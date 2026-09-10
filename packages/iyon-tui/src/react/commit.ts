import type { ContentSource, TextFunnel } from "../api/content/retained.ts";
import type {
	NativeTextSourceContract,
	NativeTuiHostContract,
} from "../transport/native/addon.ts";
import { nativeResourceOf } from "../transport/native/resources.ts";
import {
	HOST_KINDS,
	type HostKind,
	UI_BATCH_HEADER_WORDS,
	UI_BATCH_MAGIC,
	UI_BATCH_VERSION,
	UI_HANDLE_WORDS,
	UI_OPCODES,
	UI_PROPERTIES,
	uiEncodePropertyValue,
	uiPropertyValuesEqual,
} from "../transport/ui/generated/ui_schema.ts";
import {
	type NativeEventPriority,
	withNativeEventPriority,
} from "./host-config.ts";
import {
	type ContentConnectorToken,
	type ContentPortToken,
	contentTokenOwner,
	contentValuesEqual,
	eventMask,
	type HookOwner,
	type HostChild,
	HostInstance,
	type HostTextInstance,
	hostNode,
	type NormalizedContent,
	type NormalizedProperty,
	type NormalizedProps,
	normalizeProps,
	normalizePublicProperty,
	publicUiPropertyId,
	type UiEditEvent,
	type UiEvent,
	type UiHandle,
	type UiPressEvent,
	validateStyleState,
	validateStyleStateKey,
} from "./instance.ts";
import {
	ReactContentConnector,
	ReactContentPort,
	type ExplicitResourceCoordinator,
} from "./resources.ts";

export interface NativeUiEvent {
	readonly host_namespace: number;
	readonly slot: number;
	readonly generation: number;
	readonly kind: number;
	readonly mask: number;
	readonly text?: string | null;
	readonly cursor_bytes?: number | null;
	readonly key?: string | null;
	readonly revision?: number | string | null;
}

interface NormalizedNativeUiEvent
	extends Omit<NativeUiEvent, "cursor_bytes" | "key" | "revision" | "text"> {
	readonly cursor_bytes?: number;
	readonly key?: string;
	readonly revision?: number;
	readonly text?: string;
}

const NULL_HANDLE = [0, 0, 0, 0] as const;
const ALL_EDIT_REVISIONS = [0xffff_ffff, 0xffff_ffff] as const;
const textEncoder = new TextEncoder();

interface RecordValue {
	readonly section: 0 | 1 | 2 | 3 | 4;
	readonly opcode: number;
	readonly operands: readonly number[];
	readonly explicitSelection?: {
		readonly port: ReactContentPort;
		readonly connector: ReactContentConnector | undefined;
	};
}

class SidecarBuilder {
	readonly bytes: number[] = [];

	addText(value: string): [number, number] {
		if (value.length === 0 || value.includes("\0"))
			throw new RangeError("UI metadata strings must be nonempty and NUL-free");
		const encoded = textEncoder.encode(value);
		const offset = this.bytes.length;
		for (const byte of encoded) this.bytes.push(byte);
		return [offset, encoded.length];
	}

	addBytes(value: Uint8Array): [number, number] {
		const offset = this.bytes.length;
		for (const byte of value) this.bytes.push(byte);
		return [offset, value.length];
	}
}

class ContentBuilder {
	readonly bytes: number[] = [];

	add(value: string | Uint8Array): [number, number] {
		const bytes = typeof value === "string" ? textEncoder.encode(value) : value;
		const offset = this.bytes.length;
		for (const byte of bytes) this.bytes.push(byte);
		return [offset, bytes.length];
	}
}

interface Journal {
	readonly records: RecordValue[];
	readonly touched: Set<HostInstance>;
	readonly candidateRoots: Set<HostInstance>;
	readonly retired: Set<HostInstance>;
	readonly created: Map<number, HostInstance | ResourceOwner>;
	readonly contentUpdates: Map<HostInstance, PendingContentUpdate>;
	readonly sources: NativeTextSourceContract[];
	readonly sourceIndices: Map<object, number>;
	readonly tokenPorts: Map<object, PendingTokenResource>;
	readonly tokenConnectors: Map<object, PendingTokenResource>;
	readonly metadata: SidecarBuilder;
	readonly content: ContentBuilder;
}

function newJournal(): Journal {
	return {
		records: [],
		touched: new Set(),
		candidateRoots: new Set(),
		retired: new Set(),
		created: new Map(),
		contentUpdates: new Map(),
		sources: [],
		sourceIndices: new Map(),
		tokenPorts: new Map(),
		tokenConnectors: new Map(),
		metadata: new SidecarBuilder(),
		content: new ContentBuilder(),
	};
}

interface PendingTokenResource {
	readonly instance: HostInstance;
	readonly ordinal: number;
	readonly kind: "port" | "connector";
}

interface AcceptedTokenResource {
	readonly instance: HostInstance;
	readonly handle: UiHandle;
	readonly owner: HookOwner;
}

interface PendingHandle {
	readonly handle: UiHandle;
	readonly ordinal?: number;
}

interface PendingContentUpdate {
	readonly port: PendingHandle;
	readonly portToken?: ContentPortToken;
	readonly connector?: PendingHandle;
	readonly connectorToken?: ContentConnectorToken;
	readonly previousPortToken?: ContentPortToken;
	readonly previousConnectorToken?: ContentConnectorToken;
	readonly releasePreviousPortToken: boolean;
	readonly releasePreviousConnectorToken: boolean;
	readonly explicitPort?: ReactContentPort;
	readonly explicitConnector?: ReactContentConnector;
}

interface HookReleaseResources {
	readonly ports: UiHandle[];
	readonly connectors: Array<{
		readonly handle: UiHandle;
		readonly port: UiHandle | undefined;
		readonly portOwner: HookOwner | undefined;
	}>;
}

type ResourceOwner = {
	readonly instance: HostInstance | undefined;
	readonly resource: "port" | "connector" | "control";
	readonly onHandle?: (handle: UiHandle) => void;
};

export class RootContainer {
	readonly host: NativeTuiHostContract;
	readonly body: HostInstance;
	readonly coordinator: CommitCoordinator;
	readonly namespace: number;

	constructor(host: NativeTuiHostContract) {
		this.host = host;
		this.namespace = host.uiNamespace();
		const bodyHandle = host.uiBodyHandle();
		this.coordinator = new CommitCoordinator(this);
		this.body = new HostInstance(
			this,
			HOST_KINDS.box,
			normalizeProps("box", {}),
		);
		this.body.lifecycle = "accepted";
		this.body.handle = bodyHandle;
		this.body.accepted = { props: this.body.pending, handle: bodyHandle };
		this.coordinator.registerAccepted(this.body);
	}

	get children(): HostInstance[] {
		const result: HostInstance[] = [];
		let child = this.body.firstChild;
		while (child !== undefined) {
			result.push(child);
			child = child.nextSibling;
		}
		return result;
	}
}

export class CommitCoordinator implements ExplicitResourceCoordinator {
	private readonly root: RootContainer;
	private journal: Journal | undefined;
	private nextOrdinal = 1;
	private acceptedRevision = 0;
	private fault: Error | undefined;
	private cleanupMode = false;
	private readonly touchedCallbacks = new Set<HostInstance>();
	private readonly hookOwners = new Set<HookOwner>();
	private readonly pendingHookOwners = new Set<HookOwner>();
	/** Accepted correspondence used by the native event lane. */
	private readonly acceptedInstances = new Map<string, HostInstance>();
	private staleNativeEventCount = 0;
	/** Superseded tokens retained only by a stale accepted consumer. */
	private deferredHookTokens = new WeakMap<HostInstance, Set<object>>();
	private deferredTokenInstances = new WeakMap<object, HostInstance>();
	private pendingHookOwnerDrain = false;
	private tokenPorts = new WeakMap<object, AcceptedTokenResource>();
	private tokenConnectors = new WeakMap<object, AcceptedTokenResource>();
	private readonly selectedConnectors = new Map<string, UiHandle>();
	private readonly explicitPorts = new Map<string, ReactContentPort>();
	private readonly explicitConnectors = new Map<
		string,
		ReactContentConnector
	>();

	constructor(root: RootContainer) {
		this.root = root;
	}

	get isFaulted(): boolean {
		return this.fault !== undefined;
	}
	get acceptedUiRevision(): number {
		return this.acceptedRevision;
	}
	get faultError(): Error | undefined {
		return this.fault;
	}
	get staleNativeEvents(): number {
		return this.staleNativeEventCount;
	}

	createExplicitPort(family = "text"): ReactContentPort {
		if (family !== "text")
			throw new TypeError("unsupported ContentPort family");
		let handle: UiHandle | undefined;
		this.begin();
		const journal = this.requireJournal();
		const ordinal = this.nextOrdinal++;
		journal.created.set(ordinal, {
			instance: undefined,
			resource: "port",
			onHandle: (created) => {
				handle = created;
			},
		});
		journal.records.push({
			section: 2,
			opcode: UI_OPCODES.createPort,
			operands: [ordinal, 0, 2, ...NULL_HANDLE],
		});
		this.finish(true);
		if (handle === undefined)
			throw new Error("native acknowledgement omitted explicit Port");
		const port = new ReactContentPort(this, handle);
		this.explicitPorts.set(handleKey(handle), port);
		return port;
	}

	createExplicitConnector(
		port: ReactContentPort,
		source: ContentSource,
		funnel: TextFunnel,
	): ReactContentConnector {
		this.assertExplicitPort(port);
		let handle: UiHandle | undefined;
		this.begin();
		const journal = this.requireJournal();
		const ordinal = this.nextOrdinal++;
		let sourceIndex: number;
		try {
			sourceIndex = sourceIndexFor(source, journal);
		} catch (error) {
			this.abort();
			throw error;
		}
		let funnelReference: [number, number];
		try {
			funnelReference = funnelMetadata(funnel, journal.metadata);
		} catch (error) {
			this.abort();
			throw error;
		}
		journal.created.set(ordinal, {
			instance: undefined,
			resource: "connector",
			onHandle: (created) => {
				handle = created;
			},
		});
		journal.records.push({
			section: 2,
			opcode: UI_OPCODES.createConnector,
			operands: [
				ordinal,
				sourceIndex,
				...resourceRef(port.handle),
				...funnelReference,
				2,
			],
		});
		this.finish(true);
		if (handle === undefined)
			throw new Error("native acknowledgement omitted explicit Connector");
		const connector = new ReactContentConnector(
			this,
			handle,
			port,
			source,
			funnel,
		);
		this.explicitConnectors.set(handleKey(handle), connector);
		return connector;
	}

	selectExplicitConnector(
		port: ReactContentPort,
		connector: ReactContentConnector | undefined,
	): void {
		this.assertExplicitPort(port);
		if (connector !== undefined) {
			this.assertExplicitConnector(connector);
			if (connector.attachedPort !== port)
				throw new TypeError("Connector belongs to another ContentPort");
		}
		this.begin();
		const journal = this.requireJournal();
		journal.records.push(explicitSelectionRecord(port, connector));
		this.finish(true);
	}

	explicitSelectedConnector(
		port: ReactContentPort,
	): ReactContentConnector | undefined {
		this.assertExplicitPort(port);
		const selected = this.selectedConnectors.get(handleKey(port.handle));
		return selected === undefined
			? undefined
			: this.explicitConnectors.get(handleKey(selected));
	}

	deactivateExplicitConnector(connector: ReactContentConnector): void {
		this.assertExplicitConnector(connector);
		const selected = this.selectedConnectors.get(
			handleKey(connector.attachedPort.handle),
		);
		if (
			selected === undefined ||
			handleKey(selected) !== handleKey(connector.handle)
		)
			return;
		this.selectExplicitConnector(connector.attachedPort, undefined);
	}

	disposeExplicitConnector(connector: ReactContentConnector): void {
		this.assertExplicitConnector(connector);
		this.begin();
		const journal = this.requireJournal();
		journal.records.push({
			section: 2,
			opcode: UI_OPCODES.disposeConnector,
			operands: resourceRef(connector.handle),
		});
		this.finish(true);
		this.explicitConnectors.delete(handleKey(connector.handle));
		connector.markDisposed();
	}

	disposeExplicitPort(port: ReactContentPort): void {
		this.assertExplicitPort(port);
		this.begin();
		const journal = this.requireJournal();
		journal.records.push({
			section: 2,
			opcode: UI_OPCODES.disposePort,
			operands: resourceRef(port.handle),
		});
		this.finish(true);
		this.explicitPorts.delete(handleKey(port.handle));
		port.markDisposed();
	}

	explicitPortMounted(port: ReactContentPort): boolean {
		this.assertExplicitPort(port);
		const read = this.root.host.uiPortMounted;
		if (read === undefined)
			throw new Error("native UI Port status is unavailable");
		return read.call(this.root.host, resourceRef(port.handle));
	}

	explicitConnectorStatus(
		connector: ReactContentConnector,
	): import("../api/content/retained.ts").ContentConnectorStatus {
		this.assertExplicitConnector(connector);
		const read = this.root.host.uiConnectorStatus;
		if (read === undefined)
			throw new Error("native UI Connector status is unavailable");
		return read.call(
			this.root.host,
			resourceRef(connector.handle),
		) as import("../api/content/retained.ts").ContentConnectorStatus;
	}

	private assertExplicitPort(port: ReactContentPort): void {
		if (this.explicitPorts.get(handleKey(port.handle)) !== port)
			throw new Error(
				"ContentPort belongs to another React root or is disposed",
			);
	}

	private assertExplicitConnector(connector: ReactContentConnector): void {
		if (this.explicitConnectors.get(handleKey(connector.handle)) !== connector)
			throw new Error("Connector belongs to another React root or is disposed");
	}

	registerAccepted(instance: HostInstance): void {
		if (instance.handle !== undefined)
			this.acceptedInstances.set(handleKey(instance.handle), instance);
	}

	private unregisterAcceptedSubtree(instance: HostInstance): void {
		if (instance.handle !== undefined)
			this.acceptedInstances.delete(handleKey(instance.handle));
		let child = instance.firstChild;
		while (child !== undefined) {
			this.unregisterAcceptedSubtree(child);
			child = child.nextSibling;
		}
	}

	dispatchNativeEvents(
		events: readonly NativeUiEvent[],
		shouldContinue: () => boolean = () => true,
	): void {
		for (const event of events) {
			if (!shouldContinue()) return;
			const normalized = normalizeNativeUiEvent(event);
			if (normalized === undefined) continue;
			this.dispatchNativeEvent(normalized, shouldContinue);
		}
	}

	private dispatchNativeEvent(
		event: NormalizedNativeUiEvent,
		shouldContinue: () => boolean,
	): void {
		const instance = this.acceptedInstances.get(
			`${event.host_namespace}:${event.slot}:${event.generation}:${event.kind}`,
		);
		if (instance === undefined) {
			this.staleNativeEventCount += 1;
			return;
		}
		for (const [bit, name] of [
			[1, "onPress"],
			[2, "onInput"],
			[4, "onEdit"],
			[8, "onChange"],
			[16, "onSelectionChange"],
			[32, "onSubmit"],
		] as const) {
			if (!shouldContinue()) return;
			if ((event.mask & bit) === 0) continue;
			this.dispatchNativeEventCallback(instance, event, name);
		}
	}

	private dispatchNativeEventCallback(
		instance: HostInstance,
		event: NormalizedNativeUiEvent,
		name: string,
	): void {
		const callback = instance.accepted?.props.events.get(name);
		if (callback === undefined) return;
		try {
			withNativeEventPriority(nativeEventPriority(name), () =>
				callback(nativeEventPayload(event, name)),
			);
		} catch (error) {
			// Event handlers are application callbacks, not commit acceptance.
			// Report one failure and continue the owned batch so a throwing
			// handler cannot strand later native events.
			reportNativeEventError(error);
		}
	}

	drainReleasedHookOwners(): void {
		const owners = [...this.pendingHookOwners];
		this.pendingHookOwners.clear();
		for (const owner of owners) {
			if (owner.mounted || !owner.released) continue;
			try {
				owner.release?.();
			} catch (error) {
				this.pendingHookOwners.add(owner);
				this.faultFromReact(error);
			}
		}
	}

	private queueHookOwnerRelease(owner: HookOwner): void {
		if (owner.mounted || !owner.released) return;
		this.pendingHookOwners.add(owner);
		if (this.pendingHookOwnerDrain) return;
		this.pendingHookOwnerDrain = true;
		queueMicrotask(() => {
			this.pendingHookOwnerDrain = false;
			this.drainReleasedHookOwners();
		});
	}

	publishOverride(
		instance: HostInstance,
		propertyName: string,
		value: unknown,
	): { readonly revision: number; readonly accepted: true } {
		if (instance.lifecycle !== "accepted" || instance.handle === undefined)
			throw new Error("cannot override a non-accepted occurrence");
		const property = normalizePublicProperty(
			instance.kind,
			propertyName,
			value,
		);
		this.begin();
		const journal = this.requireJournal();
		journal.touched.add(instance);
		journal.records.push({
			section: 1,
			opcode: UI_OPCODES.setOverride,
			operands: [
				...nodeRef(instance),
				property.id,
				...uiEncodePropertyValue(property.name, property.value, (text) =>
					journal.metadata.addText(text),
				),
			],
		});
		this.finish();
		return { revision: this.acceptedRevision, accepted: true };
	}

	clearOverride(
		instance: HostInstance,
		propertyName: string,
	): { readonly revision: number; readonly accepted: true } {
		if (instance.lifecycle !== "accepted" || instance.handle === undefined)
			throw new Error("cannot clear an override on a non-accepted occurrence");
		const propertyId = publicUiPropertyId(instance.kind, propertyName);
		this.begin();
		const journal = this.requireJournal();
		journal.touched.add(instance);
		journal.records.push({
			section: 1,
			opcode: UI_OPCODES.clearOverride,
			operands: [...nodeRef(instance), propertyId],
		});
		this.finish();
		return { revision: this.acceptedRevision, accepted: true };
	}

	publishStyleState(
		instance: HostInstance,
		key: string,
		value: string,
	): { readonly revision: number; readonly accepted: true } {
		validateStyleState(key, value);
		if (instance.lifecycle !== "accepted" || instance.handle === undefined)
			throw new Error("cannot set a style state on a non-accepted occurrence");
		this.begin();
		const journal = this.requireJournal();
		journal.touched.add(instance);
		journal.records.push({
			section: 1,
			opcode: UI_OPCODES.setStyleState,
			operands: [
				...nodeRef(instance),
				1,
				...journal.metadata.addText(key),
				...journal.metadata.addText(value),
			],
		});
		this.finish();
		return { revision: this.acceptedRevision, accepted: true };
	}

	clearStyleState(
		instance: HostInstance,
		key: string,
	): { readonly revision: number; readonly accepted: true } {
		validateStyleStateKey(key);
		if (instance.lifecycle !== "accepted" || instance.handle === undefined)
			throw new Error(
				"cannot clear a style state on a non-accepted occurrence",
			);
		this.begin();
		const journal = this.requireJournal();
		journal.touched.add(instance);
		journal.records.push({
			section: 1,
			opcode: UI_OPCODES.clearStyleState,
			operands: [...nodeRef(instance), 1, ...journal.metadata.addText(key)],
		});
		this.finish();
		return { revision: this.acceptedRevision, accepted: true };
	}

	faultFromReact(error: unknown): void {
		this.abortCleanup();
		this.faultRoot(error);
	}

	begin(): void {
		if (this.fault !== undefined && !this.cleanupMode) throw this.fault;
		if (this.cleanupMode) {
			if (this.journal === undefined) this.journal = newJournal();
			return;
		}
		if (this.journal !== undefined)
			throw new Error("React opened a nested UI commit");
		this.nextOrdinal = 1;
		this.journal = newJournal();
	}

	beginCleanup(): void {
		this.abortCleanup();
		this.cleanupMode = true;
		this.nextOrdinal = 1;
		this.journal = newJournal();
	}

	abortCleanup(): void {
		this.cleanupMode = false;
		this.journal = undefined;
		this.touchedCallbacks.clear();
	}

	finalizeCleanup(): void {
		for (const child of this.root.children) {
			retireJsSubtree(child);
			clearJsOwnership(child);
		}
		this.tokenPorts = new WeakMap();
		this.tokenConnectors = new WeakMap();
		for (const owner of this.hookOwners) {
			owner.tokens.clear();
			owner.dependentConnectors.clear();
			owner.currentToken = undefined;
			owner.coordinator = undefined;
			owner.notifyRelease = undefined;
			owner.notifyTokenReplacement = undefined;
			owner.release = undefined;
			owner.released = true;
			owner.mounted = false;
		}
		this.hookOwners.clear();
		this.pendingHookOwners.clear();
		this.deferredHookTokens = new WeakMap();
		this.deferredTokenInstances = new WeakMap();
		this.pendingHookOwnerDrain = false;
		this.selectedConnectors.clear();
		for (const connector of this.explicitConnectors.values())
			connector.markDisposed();
		for (const port of this.explicitPorts.values()) port.markDisposed();
		this.explicitConnectors.clear();
		this.explicitPorts.clear();
		this.acceptedInstances.clear();
		this.cleanupMode = false;
		this.journal = undefined;
		this.touchedCallbacks.clear();
	}

	appendInitialChild(parent: HostInstance, child: HostChild): void {
		linkChild(parent, hostNode(child), undefined);
	}

	appendChild(parent: HostInstance, child: HostChild): void {
		this.attach(parent, hostNode(child), undefined);
	}

	appendChildToContainer(child: HostChild): void {
		this.attach(this.root.body, hostNode(child), undefined);
	}

	insertBefore(
		parent: HostInstance,
		child: HostChild,
		before: HostChild,
	): void {
		this.attach(parent, hostNode(child), hostNode(before));
	}

	insertInContainerBefore(child: HostChild, before: HostChild): void {
		this.attach(this.root.body, hostNode(child), hostNode(before));
	}

	removeChild(parent: HostInstance, child: HostChild): void {
		this.remove(parent, hostNode(child));
	}
	removeChildFromContainer(child: HostChild): void {
		this.remove(this.root.body, hostNode(child));
	}

	clearContainer(): void {
		for (const child of this.root.children) this.remove(this.root.body, child);
	}

	update(instance: HostInstance, type: string, nextProps: unknown): void {
		const next = normalizeProps(type, nextProps);
		instance.pending = next;
		this.touchedCallbacks.add(instance);
		if (instance.lifecycle === "candidate") return;
		const journal = this.requireJournal();
		journal.touched.add(instance);
		this.encodePropertyChanges(
			instance,
			instance.accepted?.props ?? instance.initial,
			next,
			journal,
		);
	}

	updateText(instance: HostTextInstance, nextText: string): void {
		instance.text = nextText;
		instance.node.pending = normalizeProps("content", { children: nextText });
		this.touchedCallbacks.add(instance.node);
		if (instance.node.lifecycle === "candidate") return;
		const journal = this.requireJournal();
		journal.touched.add(instance.node);
		const port = instance.node.port;
		if (port === undefined)
			throw new Error("accepted text instance has no ContentPort");
		const content = journal.content.add(nextText);
		journal.records.push({
			section: 4,
			opcode: UI_OPCODES.replaceLiteral,
			operands: [...resourceRef(port), 1, ...content, 0, 0],
		});
	}

	hide(instance: HostInstance): void {
		this.setHidden(instance, true);
	}
	unhide(instance: HostInstance): void {
		this.setHidden(instance, false);
	}

	private setHidden(instance: HostInstance, hidden: boolean): void {
		instance.pending = {
			...instance.pending,
			hidden,
		};
		this.touchedCallbacks.add(instance);
		if (instance.lifecycle === "candidate") return;
		const journal = this.requireJournal();
		journal.touched.add(instance);
		if (instance.accepted?.props.hidden !== hidden)
			journal.records.push({
				section: 1,
				opcode: UI_OPCODES.setHidden,
				operands: [...nodeRef(instance), hidden ? 1 : 0],
			});
	}

	finish(callerOperation = false): void {
		const journal = this.requireJournal();
		try {
			if (this.cleanupMode) return;
			this.materializeCandidates(journal);
			if (journal.records.length === 0) {
				this.promoteWithoutNative(journal);
				return;
			}
			const words = encodeWords(
				this.root.namespace,
				this.acceptedRevision,
				journal.records,
				journal.metadata.bytes.length,
				journal.content.bytes.length,
				journal.sources.length,
				journal.created.size,
			);
			const ack = this.root.host.commitUiV1(
				words,
				Uint8Array.from(journal.metadata.bytes),
				Uint8Array.from(journal.content.bytes),
				journal.sources,
			);
			this.acceptAcknowledgement(ack, journal);
		} catch (error) {
			if (!callerOperation || !isRecoverableCallerRejection(error))
				this.faultRoot(error);
			throw error;
		} finally {
			this.cleanupMode = false;
			this.journal = undefined;
			this.touchedCallbacks.clear();
		}
	}

	abort(): void {
		this.journal = undefined;
		this.touchedCallbacks.clear();
	}

	private attach(
		parent: HostInstance,
		child: HostInstance,
		before: HostInstance | undefined,
	): void {
		this.validateHistoryAttach(parent, child);
		if (child === before) return;
		if (child.parent === parent && child.nextSibling === before) return;
		if (
			child.rootRole === "portal" &&
			child.lifecycle === "accepted" &&
			child.portalOwner !== undefined &&
			child.portalOwner !== parent
		)
			throw new Error("portal root owner transfer is unsupported");
		const previousParent = child.parent;
		linkChild(parent, child, before);
		if (
			child.lifecycle === "candidate" &&
			previousParent?.lifecycle !== "candidate"
		)
			this.requireJournal().candidateRoots.add(child);
		if (
			child.lifecycle === "candidate" &&
			(previousParent === undefined || previousParent.lifecycle === "candidate")
		)
			return;
		if (child.rootRole === "portal" || child.rootRole === "historyUnit") return;
		const journal = this.requireJournal();
		const anchor = nativeAnchor(before);
		journal.touched.add(child);
		journal.records.push({
			section: 0,
			opcode: UI_OPCODES.insertBefore,
			operands: [
				...nodeRef(parent),
				...nodeRef(child),
				...(anchor === undefined ? NULL_HANDLE : nodeRef(anchor)),
			],
		});
	}

	private validateHistoryAttach(
		parent: HostInstance,
		child: HostInstance,
	): void {
		if (child.rootRole !== "historyUnit") return;
		if (parent !== this.root.body)
			throw new Error("HistoryUnit must be a root-level child");
		if (child.lifecycle === "accepted" && child.parent === this.root.body)
			throw new Error("HistoryUnit reorder is unsupported");
	}

	private remove(parent: HostInstance, child: HostInstance): void {
		if (child.parent !== parent)
			throw new Error("React removed an occurrence from the wrong parent");
		const journal = this.requireJournal();
		unlinkChild(child);
		if (child.lifecycle === "candidate") {
			const journal = this.journal;
			journal?.candidateRoots.delete(child);
			if (journal !== undefined) this.releaseTokenSubtree(child, journal);
			if (this.cleanupMode) retireJsSubtree(child);
			return;
		}
		this.releaseTokenSubtree(child, journal);
		if (journal.retired.has(child)) return;
		journal.retired.add(child);
		journal.touched.add(child);
		if (child.rootRole === "historyUnit") {
			journal.records.push({
				section: 0,
				opcode: UI_OPCODES.historyAction,
				operands: [...nodeRef(child), 2],
			});
		} else {
			journal.records.push({
				section: 0,
				opcode:
					child.rootRole === "portal"
						? UI_OPCODES.retireRoot
						: UI_OPCODES.retireSubtree,
				operands: nodeRef(child),
			});
		}
		if (this.cleanupMode) {
			this.unregisterAcceptedSubtree(child);
			retireJsSubtree(child);
			clearJsOwnership(child);
		}
	}

	private materializeCandidates(journal: Journal): void {
		const roots = candidateRoots(journal.candidateRoots);
		const anchors = candidateAnchors(roots);
		for (const child of roots) {
			if (child.lifecycle !== "candidate" || child.parent === undefined)
				continue;
			this.materializeSubtree(child, journal);
			if (child.rootRole === "portal" || child.rootRole === "historyUnit")
				continue;
			const parent = child.parent;
			const anchor = anchors.get(child);
			journal.records.push({
				section: 0,
				opcode: UI_OPCODES.insertBefore,
				operands: [
					...nodeRef(parent),
					...nodeRef(child),
					...(anchor === undefined ? NULL_HANDLE : nodeRef(anchor)),
				],
			});
		}
	}

	private materializeSubtree(instance: HostInstance, journal: Journal): void {
		if (instance.lifecycle !== "candidate") return;
		const nodeOrdinal = this.allocate(instance, journal);
		instance.localOrdinal = nodeOrdinal;
		this.encodeInitialCreation(instance, nodeOrdinal, journal);
		this.encodeInitialResources(instance, nodeOrdinal, journal);
		for (const property of instance.pending.properties.values())
			this.encodeSetDeclared(instance, property, journal);
		this.encodeStyleStateChanges(
			instance,
			new Map(),
			instance.pending.styleStates,
			journal,
		);
		if (instance.pending.hidden)
			journal.records.push({
				section: 1,
				opcode: UI_OPCODES.setHidden,
				operands: [...localRef(nodeOrdinal, 1), 1],
			});
		const mask = eventMask(instance.pending.events);
		if (mask !== 0)
			journal.records.push({
				section: 3,
				opcode: UI_OPCODES.setSubscriptions,
				operands: [...localRef(nodeOrdinal, 1), mask, 0],
			});
		this.encodeInitialChildren(instance, nodeOrdinal, journal);
		journal.touched.add(instance);
	}

	private encodeInitialCreation(
		instance: HostInstance,
		nodeOrdinal: number,
		journal: Journal,
	): void {
		if (instance.rootRole === "portal" || instance.rootRole === "historyUnit") {
			journal.records.push({
				section: 0,
				opcode: UI_OPCODES.createRoot,
				operands: [
					nodeOrdinal,
					instance.rootRole === "historyUnit" ? 3 : 2,
					...(instance.rootRole === "historyUnit"
						? NULL_HANDLE
						: nodeRef(instance.parent ?? this.root.body)),
					...(instance.rootRole === "historyUnit"
						? historyRootConfig(instance.pending, journal.metadata)
						: [0, 0]),
				],
			});
		} else {
			journal.records.push({
				section: 0,
				opcode: UI_OPCODES.createNode,
				operands: [nodeOrdinal, instance.kind],
			});
		}
		if (instance.rootRole !== "historyUnit") return;
		const action = instance.pending.historyUnit?.action ?? 0;
		if (action === 0) return;
		journal.records.push({
			section: 0,
			opcode: UI_OPCODES.historyAction,
			operands: [...localRef(nodeOrdinal, 1), action],
		});
	}

	private encodeInitialChildren(
		instance: HostInstance,
		nodeOrdinal: number,
		journal: Journal,
	): void {
		let child = instance.firstChild;
		while (child !== undefined) {
			this.materializeSubtree(child, journal);
			child = child.nextSibling;
		}
		child = instance.firstChild;
		while (child !== undefined) {
			if (child.rootRole !== "portal" && child.rootRole !== "historyUnit")
				journal.records.push({
					section: 0,
					opcode: UI_OPCODES.insertBefore,
					operands: [
						...localRef(nodeOrdinal, 1),
						...nodeRef(child),
						...NULL_HANDLE,
					],
				});
			child = child.nextSibling;
		}
	}

	private encodeInitialResources(
		instance: HostInstance,
		nodeOrdinal: number,
		journal: Journal,
	): void {
		const content = instance.pending.content;
		if (content !== undefined)
			this.encodeContentResource(instance, nodeOrdinal, content, journal);
		if (instance.pending.control !== undefined)
			this.encodeControlResource(instance, nodeOrdinal, journal);
	}

	private encodeContentResource(
		instance: HostInstance,
		nodeOrdinal: number,
		content: NormalizedContent,
		journal: Journal,
	): void {
		if (content.explicitPort !== undefined) {
			const explicitPort = content.explicitPort as ReactContentPort;
			this.assertExplicitPort(explicitPort);
			instance.port = explicitPort.handle;
			instance.explicitPort = explicitPort;
			journal.records.push({
				section: 0,
				opcode: UI_OPCODES.attachPort,
				operands: [
					...localRef(nodeOrdinal, 1),
					...resourceRef(explicitPort.handle),
				],
			});
			if (content.explicitConnector !== undefined) {
				const explicitConnector =
					content.explicitConnector as ReactContentConnector;
				this.assertExplicitConnector(explicitConnector);
				if (explicitConnector.attachedPort !== explicitPort)
					throw new TypeError("Connector belongs to another ContentPort");
				instance.connector = explicitConnector.handle;
				instance.explicitConnector = explicitConnector;
				journal.records.push(
					explicitSelectionRecord(explicitPort, explicitConnector),
				);
			}
			return;
		}
		const token = content.portToken;
		const port = this.preparePort(
			instance,
			token,
			journal,
			localRef(nodeOrdinal, 1),
		);
		instance.port = port.handle;
		if (token !== undefined) instance.portToken = token;
		journal.records.push({
			section: 0,
			opcode: UI_OPCODES.attachPort,
			operands: [...localRef(nodeOrdinal, 1), ...resourceRef(port.handle)],
		});
		if (
			token !== undefined &&
			content.mode === "lazy" &&
			content.connectorToken === undefined
		)
			this.deselectSelectedConnector(port.handle, journal);
		this.encodeInitialContent(instance, content, journal);
	}

	private preparePort(
		instance: HostInstance,
		token: ContentPortToken | undefined,
		journal: Journal,
		owner: number[],
	): PendingHandle {
		if (token !== undefined) this.ensureTokenAuthority(token);
		const accepted =
			token === undefined ? undefined : this.tokenPorts.get(token);
		if (accepted !== undefined) return { handle: accepted.handle };
		const pending =
			token === undefined ? undefined : journal.tokenPorts.get(token);
		if (pending !== undefined)
			return {
				handle: localHandle(pending.ordinal, 2),
				ordinal: pending.ordinal,
			};
		const ordinal = this.allocateResource(instance, "port", journal);
		const ownership = token === undefined ? [1, ...owner] : [2, ...NULL_HANDLE];
		journal.records.push({
			section: 2,
			opcode: UI_OPCODES.createPort,
			operands: [ordinal, 0, ...ownership],
		});
		if (token !== undefined)
			journal.tokenPorts.set(token, { instance, ordinal, kind: "port" });
		return { handle: localHandle(ordinal, 2), ordinal };
	}

	private encodeInitialContent(
		instance: HostInstance,
		content: NormalizedContent,
		journal: Journal,
	): void {
		if (content.explicitConnector !== undefined) return;
		if (content.mode === "literal")
			this.encodeLiteralResource(instance.port, content, journal);
		if (
			content.mode === "source" ||
			(content.mode === "lazy" && content.source !== undefined)
		) {
			const connector = this.encodeConnectorResource(
				instance,
				content.source,
				content.funnel,
				journal,
				content.connectorToken,
			);
			instance.connector = connector.handle;
			if (content.connectorToken !== undefined)
				instance.connectorToken = content.connectorToken;
		}
	}

	private encodeLiteralResource(
		port: UiHandle | undefined,
		content: NormalizedContent,
		journal: Journal,
	): void {
		if (port === undefined)
			throw new Error("literal ContentPort was not allocated");
		const bytes = journal.content.add(content.text ?? "");
		journal.records.push({
			section: 4,
			opcode: UI_OPCODES.replaceLiteral,
			operands: [...resourceRef(port), 1, ...bytes, 0, 0],
		});
		journal.records.push(
			...literalFunnelRecords(port, content.funnel, journal.metadata),
		);
	}

	private encodeConnectorResource(
		instance: HostInstance,
		source: ContentSource | undefined,
		funnel: TextFunnel,
		journal: Journal,
		connectorToken?: ContentConnectorToken,
		port: UiHandle = instance.port as UiHandle,
	): PendingHandle {
		if (source === undefined || instance.port === undefined)
			throw new Error("source ContentPort is incomplete");
		if (connectorToken !== undefined) this.ensureTokenAuthority(connectorToken);
		const sourceIndex = sourceIndexFor(source, journal);
		const acceptedConnector =
			connectorToken === undefined
				? undefined
				: this.tokenConnectors.get(connectorToken)?.handle;
		const pendingConnector =
			connectorToken === undefined
				? undefined
				: journal.tokenConnectors.get(connectorToken);
		const existingConnector =
			acceptedConnector ??
			(pendingConnector === undefined
				? undefined
				: localHandle(pendingConnector.ordinal, 3));
		if (existingConnector !== undefined) {
			journal.records.push({
				section: 2,
				opcode: UI_OPCODES.selectConnector,
				operands: [...resourceRef(port), ...resourceRef(existingConnector)],
			});
			return {
				handle: existingConnector,
				ordinal: pendingConnector?.ordinal,
			};
		}
		const connectorOrdinal = this.allocateResource(
			instance,
			"connector",
			journal,
		);
		const connector = localHandle(connectorOrdinal, 3);
		if (connectorToken !== undefined)
			journal.tokenConnectors.set(connectorToken, {
				instance,
				ordinal: connectorOrdinal,
				kind: "connector",
			});
		journal.records.push({
			section: 2,
			opcode: UI_OPCODES.createConnector,
			operands: [
				connectorOrdinal,
				sourceIndex,
				...resourceRef(port),
				...funnelMetadata(funnel, journal.metadata),
				connectorToken === undefined ? 1 : 2,
			],
		});
		journal.records.push({
			section: 2,
			opcode: UI_OPCODES.selectConnector,
			operands: [...resourceRef(port), ...resourceRef(connector)],
		});
		return { handle: connector, ordinal: connectorOrdinal };
	}

	private encodeControlResource(
		instance: HostInstance,
		nodeOrdinal: number,
		journal: Journal,
	): void {
		const control = instance.pending.control;
		if (control === undefined)
			throw new Error("control resource is incomplete");
		const controlOrdinal = this.allocateResource(instance, "control", journal);
		const controlKind =
			instance.kind === HOST_KINDS.editor
				? 1
				: instance.kind === HOST_KINDS.scroll
					? 2
					: 3;
		const config = controlMetadata(instance.kind, control, journal.metadata);
		const handle = localHandle(controlOrdinal, 4);
		journal.records.push({
			section: 0,
			opcode: UI_OPCODES.createControl,
			operands: [
				controlOrdinal,
				controlKind,
				1,
				...localRef(nodeOrdinal, 1),
				...config,
			],
		});
		journal.records.push({
			section: 0,
			opcode: UI_OPCODES.attachControl,
			operands: [...localRef(nodeOrdinal, 1), ...resourceRef(handle)],
		});
		instance.control = handle;
		if (instance.kind === HOST_KINDS.editor && control.value !== undefined) {
			const bytes = journal.content.add(control.value);
			journal.records.push({
				section: 4,
				opcode: UI_OPCODES.replaceEditorContent,
				operands: [...resourceRef(handle), ...bytes, ...ALL_EDIT_REVISIONS],
			});
		}
	}

	private promoteWithoutNative(journal: Journal): void {
		for (const instance of this.touchedCallbacks) {
			if (instance.lifecycle === "retired") continue;
			if (instance.accepted !== undefined)
				instance.accepted = {
					props: instance.pending,
					handle: instance.accepted.handle,
				};
		}
		for (const instance of journal.retired) {
			this.clearSelectionSubtree(instance);
			this.unregisterAcceptedSubtree(instance);
			retireJsSubtree(instance);
		}
	}

	private acceptAcknowledgement(ack: Uint32Array, journal: Journal): void {
		validateAcknowledgement(ack, journal.created.size);
		this.assignCreatedHandles(ack, journal);
		this.applyContentUpdates(journal, ack);
		this.acceptedRevision = (ack[1] ?? 0) + (ack[2] ?? 0) * 0x1_0000_0000;
		this.promoteTouched(journal);
		// Only accepted selection operations change caller-owned selection.
		// An unrelated React prop update must not restore an earlier Connector.
		for (const record of journal.records) {
			const selection = record.explicitSelection;
			if (selection !== undefined)
				this.replaceSelection(
					undefined,
					selection.port.handle,
					selection.connector?.handle,
				);
		}
		for (const [token, pending] of journal.tokenPorts)
			this.acceptTokenResource(
				this.tokenPorts,
				token,
				pending.instance,
				acknowledgementHandle(ack, pending.ordinal),
			);
		for (const [token, pending] of journal.tokenConnectors)
			this.acceptTokenResource(
				this.tokenConnectors,
				token,
				pending.instance,
				acknowledgementHandle(ack, pending.ordinal),
			);
		for (const instance of journal.retired) {
			this.clearSelectionSubtree(instance);
			this.unregisterAcceptedSubtree(instance);
			retireJsSubtree(instance);
		}
	}

	private applyContentUpdates(journal: Journal, ack: Uint32Array): void {
		for (const [instance, update] of journal.contentUpdates) {
			const previousPort = instance.port;
			const previousExplicitPort = instance.explicitPort;
			const previousConnectorToken = instance.connectorToken;
			instance.port = this.acknowledgePending(update.port, ack);
			instance.portToken = update.portToken;
			instance.connector =
				update.connector === undefined
					? undefined
					: this.acknowledgePending(update.connector, ack);
			instance.connectorToken = update.connectorToken;
			instance.explicitPort = update.explicitPort;
			instance.explicitConnector = update.explicitConnector;
			if (
				previousConnectorToken !== undefined &&
				previousConnectorToken !== update.connectorToken &&
				this.deferredTokenInstances.get(previousConnectorToken) === instance
			)
				queueMicrotask(() => this.releaseDeferredHookTokens(instance));
			this.replaceSelection(
				previousExplicitPort === undefined ? previousPort : undefined,
				instance.explicitPort === undefined ? instance.port : undefined,
				instance.connector,
			);
			if (
				update.previousPortToken !== undefined &&
				update.previousPortToken !== update.portToken &&
				update.releasePreviousPortToken
			)
				this.forgetTokenResource(this.tokenPorts, update.previousPortToken);
			if (
				update.previousConnectorToken !== undefined &&
				update.previousConnectorToken !== update.connectorToken &&
				update.releasePreviousConnectorToken
			)
				this.forgetTokenResource(
					this.tokenConnectors,
					update.previousConnectorToken,
				);
		}
	}

	private forgetTokenResource(
		resources: WeakMap<object, AcceptedTokenResource>,
		token: object,
	): void {
		const resource = resources.get(token);
		if (resource !== undefined) {
			resource.owner.tokens.delete(token);
			if (
				(token as { readonly kind?: string }).kind === "content-connector-token"
			) {
				const connectorToken = token as ContentConnectorToken;
				tokenOwner(connectorToken.port).dependentConnectors.delete(
					connectorToken,
				);
			}
		}
		resources.delete(token);
	}

	private acknowledgePending(
		pending: PendingHandle,
		ack: Uint32Array,
	): UiHandle {
		return pending.ordinal === undefined
			? pending.handle
			: acknowledgementHandle(ack, pending.ordinal);
	}

	private acceptTokenResource(
		resources: WeakMap<object, AcceptedTokenResource>,
		token: object,
		instance: HostInstance,
		handle: UiHandle,
	): void {
		const owner = tokenOwner(token);
		if (owner.coordinator !== undefined && owner.coordinator !== this)
			throw new Error("Content token is already owned by another React root");
		owner.coordinator = this;
		resources.set(token, { instance, handle, owner });
		owner.tokens.add(token);
		if (
			(token as { readonly kind?: string }).kind === "content-connector-token"
		) {
			const connectorToken = token as ContentConnectorToken;
			tokenOwner(connectorToken.port).dependentConnectors.add(connectorToken);
		}
		this.hookOwners.add(owner);
		owner.notifyRelease = () => this.queueHookOwnerRelease(owner);
		owner.notifyTokenReplacement = (replaced) =>
			this.releaseReplacedHookToken(owner, replaced);
		if (owner.release === undefined)
			owner.release = () => this.releaseHookOwner(owner);
	}

	private ensureTokenAuthority(token: object): void {
		const owner = tokenOwner(token);
		if (owner.released)
			throw new Error("Content token belongs to an unmounted hook owner");
		if (owner.coordinator !== undefined && owner.coordinator !== this)
			throw new Error("Content token is already owned by another React root");
	}

	private assignCreatedHandles(ack: Uint32Array, journal: Journal): void {
		for (const [ordinal, owner] of journal.created) {
			const handle = acknowledgementHandle(ack, ordinal);
			if (owner instanceof HostInstance) owner.handle = handle;
			else {
				if (owner.instance !== undefined)
					this.assignResource(owner.instance, handle, owner.resource);
				owner.onHandle?.(handle);
			}
		}
	}

	private promoteTouched(journal: Journal): void {
		for (const instance of journal.touched) this.promoteInstance(instance);
	}

	private promoteInstance(instance: HostInstance): void {
		if (instance.lifecycle === "candidate") {
			instance.lifecycle = "accepted";
			if (instance.rootRole === "portal")
				instance.portalOwner = instance.parent;
			const handle = instance.handle;
			if (handle === undefined)
				throw new Error("native acknowledgement omitted a node handle");
			instance.accepted = { props: instance.pending, handle };
			this.registerAccepted(instance);
		} else if (
			instance.lifecycle !== "retired" &&
			instance.accepted !== undefined
		) {
			instance.accepted = {
				props: instance.pending,
				handle: instance.accepted.handle,
			};
		}
		this.refreshPortTokenOwner(instance);
		this.refreshConnectorTokenOwner(instance);
		if (instance.explicitPort === undefined)
			this.replaceSelection(instance.port, instance.port, instance.connector);
	}

	private replaceSelection(
		previousPort: UiHandle | undefined,
		nextPort: UiHandle | undefined,
		connector: UiHandle | undefined,
	): void {
		if (
			previousPort !== undefined &&
			(nextPort === undefined ||
				handleKey(previousPort) !== handleKey(nextPort))
		)
			this.clearSelectedConnector(previousPort);
		if (nextPort === undefined) return;
		if (connector === undefined) this.clearSelectedConnector(nextPort);
		else this.selectedConnectors.set(handleKey(nextPort), connector);
	}

	private clearSelectedConnector(port: UiHandle): void {
		this.selectedConnectors.delete(handleKey(port));
	}

	private deselectSelectedConnector(port: UiHandle, journal: Journal): void {
		if (port.host_namespace === 0) return;
		if (this.selectedConnectors.get(handleKey(port)) === undefined) return;
		journal.records.push({
			section: 2,
			opcode: UI_OPCODES.selectConnector,
			operands: [...resourceRef(port), ...NULL_HANDLE],
		});
	}

	private clearSelectionSubtree(instance: HostInstance): void {
		// Native detachment retires an occurrence-owned Port and its selected
		// Connector.  An explicit Port survives with its selected Connector;
		// retain that accepted selection until the hook owner deliberately
		// deselects or disposes it.
		if (
			instance.portToken === undefined &&
			instance.explicitPort === undefined &&
			instance.port !== undefined
		)
			this.clearSelectedConnector(instance.port);
		let child = instance.firstChild;
		while (child !== undefined) {
			this.clearSelectionSubtree(child);
			child = child.nextSibling;
		}
	}

	private refreshPortTokenOwner(instance: HostInstance): void {
		const token = instance.portToken;
		const resource =
			token === undefined ? undefined : this.tokenPorts.get(token);
		if (
			token !== undefined &&
			resource !== undefined &&
			instance.port !== undefined
		)
			this.tokenPorts.set(token, {
				instance,
				handle: resource.handle,
				owner: resource.owner,
			});
	}

	private refreshConnectorTokenOwner(instance: HostInstance): void {
		const token = instance.connectorToken;
		const resource =
			token === undefined ? undefined : this.tokenConnectors.get(token);
		if (
			token !== undefined &&
			resource !== undefined &&
			instance.connector !== undefined
		) {
			this.tokenConnectors.set(token, {
				instance,
				handle: resource.handle,
				owner: resource.owner,
			});
			this.moveDeferredHookToken(token, instance);
		}
	}

	private assignResource(
		instance: HostInstance,
		handle: UiHandle,
		resource?: ResourceOwner["resource"],
	): void {
		if (resource === "port") instance.port = handle;
		else if (resource === "connector") instance.connector = handle;
		else if (resource === "control") instance.control = handle;
	}

	private encodePropertyChanges(
		instance: HostInstance,
		previous: NormalizedProps,
		next: NormalizedProps,
		journal: Journal,
	): void {
		this.encodePropertyFields(instance, previous, next, journal);
		this.encodeStyleStateChanges(
			instance,
			previous.styleStates,
			next.styleStates,
			journal,
		);
		if (previous.hidden !== next.hidden)
			journal.records.push({
				section: 1,
				opcode: UI_OPCODES.setHidden,
				operands: [...nodeRef(instance), next.hidden ? 1 : 0],
			});
		const previousEvents = eventMask(previous.events);
		const nextEvents = eventMask(next.events);
		if (previousEvents !== nextEvents)
			journal.records.push({
				section: 3,
				opcode: UI_OPCODES.setSubscriptions,
				operands: [...nodeRef(instance), nextEvents, 0],
			});
		this.encodeContentChanges(
			instance,
			previous.content,
			next.content,
			journal,
		);
		this.encodeControlledEditorChange(instance, previous, next, journal);
		this.encodeHistoryUnitChange(instance, previous, next, journal);
	}

	private encodeHistoryUnitChange(
		instance: HostInstance,
		previous: NormalizedProps,
		next: NormalizedProps,
		journal: Journal,
	): void {
		if (instance.rootRole !== "historyUnit") return;
		const before = previous.historyUnit;
		const after = next.historyUnit;
		if (after === undefined)
			throw new Error("HistoryUnit lost its root configuration");
		if (before !== undefined && before.flowBoundary !== after.flowBoundary)
			throw new Error("HistoryUnit identity and flow boundary are immutable");
		const previousAction = before?.action ?? 0;
		if (previousAction === 1 && after.action === 0)
			throw new Error("a frozen HistoryUnit cannot become live again");
		if (previousAction === after.action || after.action === 0) return;
		journal.records.push({
			section: 0,
			opcode: UI_OPCODES.historyAction,
			operands: [...nodeRef(instance), after.action],
		});
	}

	private encodePropertyFields(
		instance: HostInstance,
		previous: NormalizedProps,
		next: NormalizedProps,
		journal: Journal,
	): void {
		for (const name of new Set([
			...previous.properties.keys(),
			...next.properties.keys(),
		])) {
			const before = previous.properties.get(name);
			const after = next.properties.get(name);
			if (before === undefined && after === undefined) continue;
			if (
				before !== undefined &&
				after !== undefined &&
				uiPropertyValuesEqual(name, before.value, after.value)
			)
				continue;
			if (after === undefined)
				journal.records.push({
					section: 1,
					opcode: UI_OPCODES.resetDeclared,
					operands: [...nodeRef(instance), UI_PROPERTIES[name]],
				});
			else this.encodeSetDeclared(instance, after, journal);
		}
	}

	private encodeStyleStateChanges(
		instance: HostInstance,
		previous: ReadonlyMap<string, string>,
		next: ReadonlyMap<string, string>,
		journal: Journal,
	): void {
		const keys = new Set([...previous.keys(), ...next.keys()]);
		for (const key of keys) {
			const before = previous.get(key);
			const after = next.get(key);
			if (before === after) continue;
			if (after === undefined) {
				journal.records.push({
					section: 1,
					opcode: UI_OPCODES.clearStyleState,
					operands: [...nodeRef(instance), 0, ...journal.metadata.addText(key)],
				});
			} else {
				journal.records.push({
					section: 1,
					opcode: UI_OPCODES.setStyleState,
					operands: [
						...nodeRef(instance),
						0,
						...journal.metadata.addText(key),
						...journal.metadata.addText(after),
					],
				});
			}
		}
	}

	private encodeControlledEditorChange(
		instance: HostInstance,
		previous: NormalizedProps,
		next: NormalizedProps,
		journal: Journal,
	): void {
		const control = instance.control;
		const nextValue = next.control?.value;
		if (
			instance.kind === HOST_KINDS.editor &&
			control !== undefined &&
			next.control?.controlled === true &&
			previous.control?.value !== nextValue &&
			nextValue !== undefined
		) {
			const bytes = journal.content.add(nextValue);
			journal.records.push({
				section: 4,
				opcode: UI_OPCODES.replaceEditorContent,
				operands: [...resourceRef(control), ...bytes, ...ALL_EDIT_REVISIONS],
			});
		}
	}

	private encodeSetDeclared(
		instance: HostInstance,
		property: NormalizedProperty,
		journal: Journal,
	): void {
		journal.records.push({
			section: 1,
			opcode: UI_OPCODES.setDeclared,
			operands: [
				...nodeRef(instance),
				property.id,
				...uiEncodePropertyValue(property.name, property.value, (value) =>
					journal.metadata.addText(value),
				),
			],
		});
	}

	private encodeContentChanges(
		instance: HostInstance,
		previous: NormalizedContent | undefined,
		next: NormalizedContent | undefined,
		journal: Journal,
	): void {
		if (
			next === undefined ||
			instance.port === undefined ||
			contentValuesEqual(previous, next)
		)
			return;
		const explicitChanged =
			previous?.explicitPort !== next.explicitPort ||
			previous?.explicitConnector !== next.explicitConnector;
		if (explicitChanged) {
			if (next.explicitPort !== undefined)
				this.encodeExplicitContentChange(instance, next, journal);
			else this.encodePortChange(instance, previous, next, journal);
			return;
		}
		if (previous?.portToken !== next.portToken) {
			this.encodePortChange(instance, previous, next, journal);
			return;
		}
		const port = instance.port;
		if (next.mode === "literal") {
			let releaseConnector = false;
			if (previous?.mode !== "literal") {
				this.deselectCurrentConnector(instance, journal);
				releaseConnector = this.disposeCurrentConnector(
					instance,
					journal,
					true,
				);
			}
			this.encodeLiteralUpdate(port, previous, next, journal);
			this.recordContentUpdate(
				instance,
				previous,
				next,
				{ port: { handle: port } },
				journal,
				false,
				releaseConnector,
			);
			return;
		}
		if (
			next.mode === "source" ||
			(next.mode === "lazy" && next.source !== undefined)
		) {
			const releaseConnector = this.disposeCurrentConnector(
				instance,
				journal,
				false,
			);
			const connector = this.encodeConnectorResource(
				instance,
				next.source,
				next.funnel,
				journal,
				next.connectorToken,
				port,
			);
			this.recordContentUpdate(
				instance,
				previous,
				next,
				{ port: { handle: port }, connector },
				journal,
				false,
				releaseConnector,
			);
			return;
		}
		let releaseConnector = false;
		if (previous?.mode !== "literal") {
			this.deselectCurrentConnector(instance, journal);
			releaseConnector = this.disposeCurrentConnector(instance, journal, true);
		}
		this.recordContentUpdate(
			instance,
			previous,
			next,
			{ port: { handle: port } },
			journal,
			false,
			releaseConnector,
		);
	}

	private encodeExplicitContentChange(
		instance: HostInstance,
		next: NormalizedContent,
		journal: Journal,
	): void {
		const port = next.explicitPort as ReactContentPort;
		this.assertExplicitPort(port);
		if (instance.port === undefined)
			throw new Error("ContentPort is unavailable");
		if (handleKey(instance.port) !== handleKey(port.handle)) {
			journal.records.push({
				section: 0,
				opcode: UI_OPCODES.attachPort,
				operands: [...nodeRef(instance), ...NULL_HANDLE],
			});
			journal.records.push({
				section: 0,
				opcode: UI_OPCODES.attachPort,
				operands: [...nodeRef(instance), ...resourceRef(port.handle)],
			});
		}
		const connector = next.explicitConnector as
			| ReactContentConnector
			| undefined;
		if (connector !== undefined) {
			this.assertExplicitConnector(connector);
			if (connector.attachedPort !== port)
				throw new TypeError("Connector belongs to another ContentPort");
			journal.records.push(explicitSelectionRecord(port, connector));
		}
		journal.contentUpdates.set(instance, {
			port: { handle: port.handle },
			portToken: undefined,
			connector:
				connector === undefined ? undefined : { handle: connector.handle },
			connectorToken: undefined,
			previousPortToken: undefined,
			previousConnectorToken: undefined,
			releasePreviousPortToken: false,
			releasePreviousConnectorToken: false,
			explicitPort: port,
			explicitConnector: connector,
		});
	}

	private encodePortChange(
		instance: HostInstance,
		previous: NormalizedContent | undefined,
		next: NormalizedContent,
		journal: Journal,
	): void {
		if (instance.port === undefined)
			throw new Error("ContentPort is unavailable");
		const port = this.preparePort(
			instance,
			next.portToken,
			journal,
			nodeRef(instance),
		);
		if (previous?.portToken !== undefined)
			this.deselectCurrentConnector(instance, journal);
		journal.records.push({
			section: 0,
			opcode: UI_OPCODES.attachPort,
			operands: [...nodeRef(instance), ...NULL_HANDLE],
		});
		journal.records.push({
			section: 0,
			opcode: UI_OPCODES.attachPort,
			operands: [...nodeRef(instance), ...resourceRef(port.handle)],
		});
		let connector: PendingHandle | undefined;
		if (next.mode === "literal") {
			this.encodeLiteralResource(port.handle, next, journal);
		} else if (next.source !== undefined) {
			connector = this.encodeConnectorResource(
				instance,
				next.source,
				next.funnel,
				journal,
				next.connectorToken,
				port.handle,
			);
		}
		this.recordContentUpdate(
			instance,
			previous,
			next,
			{ port, connector },
			journal,
			false,
			false,
		);
	}

	private encodeLiteralUpdate(
		port: UiHandle,
		previous: NormalizedContent | undefined,
		next: NormalizedContent,
		journal: Journal,
	): void {
		if (previous?.mode !== "literal" || previous.text !== next.text) {
			const bytes = journal.content.add(next.text ?? "");
			journal.records.push({
				section: 4,
				opcode: UI_OPCODES.replaceLiteral,
				operands: [...resourceRef(port), 1, ...bytes, 0, 0],
			});
		}
		if (
			previous === undefined ||
			funnelSignature(previous.funnel) !== funnelSignature(next.funnel)
		)
			journal.records.push(
				...literalFunnelRecords(port, next.funnel, journal.metadata),
			);
	}

	private deselectCurrentConnector(
		instance: HostInstance,
		journal: Journal,
	): void {
		if (instance.connector === undefined || instance.port === undefined) return;
		if (instance.connector.host_namespace !== 0)
			journal.records.push({
				section: 2,
				opcode: UI_OPCODES.selectConnector,
				operands: [...resourceRef(instance.port), ...NULL_HANDLE],
			});
	}

	private disposeCurrentConnector(
		instance: HostInstance,
		journal: Journal,
		alreadyDeselected: boolean,
	): boolean {
		if (instance.connector === undefined || instance.port === undefined)
			return false;
		const token = instance.connectorToken;
		if (token !== undefined && !tokenOwner(token).released) return false;
		if (instance.connector.host_namespace !== 0) {
			if (!alreadyDeselected) this.deselectCurrentConnector(instance, journal);
			journal.records.push({
				section: 2,
				opcode: UI_OPCODES.disposeConnector,
				operands: resourceRef(instance.connector),
			});
		}
		return true;
	}

	private recordContentUpdate(
		instance: HostInstance,
		previous: NormalizedContent | undefined,
		next: NormalizedContent,
		resources: {
			readonly port: PendingHandle;
			readonly connector?: PendingHandle;
		},
		journal: Journal,
		releasePreviousPortToken: boolean,
		releasePreviousConnectorToken: boolean,
	): void {
		journal.contentUpdates.set(instance, {
			port: resources.port,
			portToken: next.portToken,
			connector: resources.connector,
			connectorToken: next.connectorToken,
			previousPortToken: previous?.portToken,
			previousConnectorToken: previous?.connectorToken,
			releasePreviousPortToken,
			releasePreviousConnectorToken,
		});
	}

	private allocate(instance: HostInstance, journal: Journal): number {
		const ordinal = this.nextOrdinal++;
		journal.created.set(ordinal, instance);
		return ordinal;
	}

	private allocateResource(
		instance: HostInstance,
		resource: ResourceOwner["resource"],
		journal: Journal,
	): number {
		const ordinal = this.nextOrdinal++;
		journal.created.set(ordinal, { instance, resource });
		return ordinal;
	}

	private requireJournal(): Journal {
		if (this.journal === undefined)
			throw new Error("React mutation callback ran outside prepareForCommit");
		return this.journal;
	}

	private releaseTokenSubtree(instance: HostInstance, journal: Journal): void {
		if (instance.portToken !== undefined) {
			journal.tokenPorts.delete(instance.portToken);
		}
		if (instance.connectorToken !== undefined) {
			journal.tokenConnectors.delete(instance.connectorToken);
		}
		let child = instance.firstChild;
		while (child !== undefined) {
			const next = child.nextSibling;
			this.releaseTokenSubtree(child, journal);
			child = next;
		}
	}

	private faultRoot(error: unknown): void {
		const fault = error instanceof Error ? error : new Error(String(error));
		this.fault = fault;
		for (const child of this.root.children) retireSpeculativeSubtree(child);
	}

	/**
	 * React calls this after a deleted host instance has completed its native
	 * retirement.  Revisit only superseded tokens that were retained because a
	 * stale consumer still referenced them; ordinary detached consumers remain
	 * owned by their hook lifecycle.
	 */
	detachDeletedInstance(instance: HostInstance): void {
		if (this.deferredHookTokens.get(instance) === undefined) return;
		queueMicrotask(() => this.releaseDeferredHookTokens(instance));
	}

	private releaseDeferredHookTokens(instance: HostInstance): void {
		const tokens = this.deferredHookTokens.get(instance);
		if (tokens === undefined) return;
		for (const token of [...tokens]) {
			const resource = this.tokenConnectors.get(token);
			if (resource === undefined) {
				this.clearDeferredHookToken(token);
				continue;
			}
			const owner = resource.owner;
			this.releaseReplacedHookToken(owner, token);
			if (this.tokenConnectors.get(token) === undefined)
				this.clearDeferredHookToken(token);
		}
	}

	private deferHookToken(token: object, instance: HostInstance): void {
		let tokens = this.deferredHookTokens.get(instance);
		if (tokens === undefined) {
			tokens = new Set();
			this.deferredHookTokens.set(instance, tokens);
		}
		tokens.add(token);
		this.deferredTokenInstances.set(token, instance);
	}

	private clearDeferredHookToken(token: object): void {
		const instance = this.deferredTokenInstances.get(token);
		if (instance !== undefined) {
			const tokens = this.deferredHookTokens.get(instance);
			tokens?.delete(token);
		}
		this.deferredTokenInstances.delete(token);
	}

	private moveDeferredHookToken(token: object, instance: HostInstance): void {
		const previous = this.deferredTokenInstances.get(token);
		if (previous === undefined || previous === instance) return;
		this.deferredHookTokens.get(previous)?.delete(token);
		this.deferHookToken(token, instance);
	}

	/**
	 * Release the native resource for a connector token that was replaced by a
	 * committed hook dependency change.  This is deliberately separate from
	 * consumer selection: an inactive connector owned by another live hook is
	 * not touched merely because a Port selected a different connector.
	 */
	private releaseReplacedHookToken(owner: HookOwner, token: object): void {
		if (
			owner.coordinator !== this ||
			this.cleanupMode ||
			this.fault !== undefined ||
			!owner.mounted
		)
			return;
		if (
			(token as { readonly kind?: string }).kind !== "content-connector-token"
		)
			return;
		const resource = this.tokenConnectors.get(token);
		if (resource === undefined || resource.owner !== owner) return;
		// A stale render can leave an older token in a consumer closure even
		// after its hook has produced a replacement.  The replacement callback
		// may release only a token that the accepted occurrence no longer uses;
		// ordinary consumer detachment remains an owner-lifecycle concern.
		if (
			resource.instance.lifecycle === "accepted" &&
			resource.instance.connectorToken === token &&
			resource.instance.connector !== undefined &&
			handleKey(resource.instance.connector) === handleKey(resource.handle)
		) {
			this.deferHookToken(token, resource.instance);
			return;
		}
		const connectorToken = token as ContentConnectorToken;
		const portResource = this.tokenPorts.get(connectorToken.port);
		const selected = portResource?.handle;
		const selectedConnector =
			selected === undefined
				? undefined
				: this.selectedConnectors.get(handleKey(selected));
		let selectedByPort = false;
		try {
			this.begin();
			const journal = this.requireJournal();
			if (
				selected !== undefined &&
				selectedConnector !== undefined &&
				handleKey(selectedConnector) === handleKey(resource.handle)
			) {
				selectedByPort = true;
				journal.records.push({
					section: 2,
					opcode: UI_OPCODES.selectConnector,
					operands: [...resourceRef(selected), ...NULL_HANDLE],
				});
			}
			journal.records.push({
				section: 2,
				opcode: UI_OPCODES.disposeConnector,
				operands: resourceRef(resource.handle),
			});
			this.finish();
		} catch (error) {
			this.faultFromReact(error);
			return;
		}
		this.forgetTokenResource(this.tokenConnectors, token);
		this.clearDeferredHookToken(token);
		if (selectedByPort && selected !== undefined)
			this.clearSelectedConnector(selected);
		this.queueHookOwnerRelease(tokenOwner(connectorToken.port));
	}

	private releaseHookOwner(owner: HookOwner): void {
		if (!this.canReleaseHookOwner(owner)) return;
		const resources = this.collectHookResources(owner);
		if (resources.ports.length === 0 && resources.connectors.length === 0) {
			// A Port can be held alive by a Connector owned by another hook.  Do
			// not put this owner back into the render-wide pending queue: the
			// dependent Connector release explicitly wakes it when the dependency
			// is gone.  Re-scanning here on every render both hides the dependency
			// edge and can turn a blocked cleanup into unbounded work.
			if (owner.tokens.size === 0) this.severHookOwner(owner);
			return;
		}
		const dependentOwners = resources.connectors
			.map((connector) => connector.portOwner)
			.filter((dependent): dependent is HookOwner => dependent !== undefined);
		const deselected = this.disposeHookResources(resources);
		for (const token of owner.tokens) {
			this.clearDeferredHookToken(token);
			this.forgetTokenResource(this.tokenPorts, token);
			this.forgetTokenResource(this.tokenConnectors, token);
		}
		this.severHookOwner(owner);
		for (const dependent of dependentOwners)
			this.queueHookOwnerRelease(dependent);
		for (const port of deselected) this.clearSelectedConnector(port);
	}

	private severHookOwner(owner: HookOwner): void {
		owner.tokens.clear();
		owner.dependentConnectors.clear();
		owner.currentToken = undefined;
		owner.release = undefined;
		owner.notifyRelease = undefined;
		owner.notifyTokenReplacement = undefined;
		owner.coordinator = undefined;
		this.hookOwners.delete(owner);
		this.pendingHookOwners.delete(owner);
	}

	private canReleaseHookOwner(owner: HookOwner): boolean {
		return !owner.mounted && owner.released && !this.cleanupMode && !this.fault;
	}

	private collectHookResources(owner: HookOwner): HookReleaseResources {
		const ports: UiHandle[] = [];
		const connectors: HookReleaseResources["connectors"] = [];
		for (const token of owner.tokens) {
			this.collectTokenResources(token, owner, ports, connectors);
		}
		return { ports, connectors };
	}

	private collectTokenResources(
		token: object,
		owner: HookOwner,
		ports: UiHandle[],
		connectors: HookReleaseResources["connectors"],
	): void {
		const port = this.tokenPorts.get(token);
		if (port?.owner === owner) {
			const blocked = [...owner.dependentConnectors].some((dependent) => {
				const resource = this.tokenConnectors.get(dependent);
				return resource !== undefined && resource.owner !== owner;
			});
			if (!blocked) ports.push(port.handle);
		}
		const connector = this.tokenConnectors.get(token);
		if (connector?.owner !== owner) return;
		const connectorToken = token as ContentConnectorToken;
		const portResource = this.tokenPorts.get(connectorToken.port);
		connectors.push({
			handle: connector.handle,
			port: portResource?.handle,
			portOwner: tokenOwner(connectorToken.port),
		});
	}

	private disposeHookResources(resources: HookReleaseResources): UiHandle[] {
		this.begin();
		const journal = this.requireJournal();
		const deselected: UiHandle[] = [];
		for (const connector of resources.connectors) {
			const selected =
				connector.port === undefined
					? undefined
					: this.selectedConnectors.get(handleKey(connector.port));
			if (
				connector.port !== undefined &&
				selected !== undefined &&
				handleKey(selected) === handleKey(connector.handle)
			) {
				deselected.push(connector.port);
				journal.records.push({
					section: 2,
					opcode: UI_OPCODES.selectConnector,
					operands: [...resourceRef(connector.port), ...NULL_HANDLE],
				});
			}
		}
		for (const connector of resources.connectors)
			journal.records.push({
				section: 2,
				opcode: UI_OPCODES.disposeConnector,
				operands: resourceRef(connector.handle),
			});
		for (const port of resources.ports)
			journal.records.push({
				section: 2,
				opcode: UI_OPCODES.disposePort,
				operands: resourceRef(port),
			});
		this.finish();
		return deselected;
	}
}

function tokenOwner(token: object): HookOwner {
	const owner = contentTokenOwner(token);
	if (owner === undefined)
		throw new TypeError("Content token is not a live qualified React token");
	return owner;
}

function handleKey(handle: UiHandle): string {
	return `${handle.host_namespace}:${handle.slot}:${handle.generation}:${handle.kind}`;
}

const NATIVE_EVENT_MASK = 1 | 2 | 4 | 8 | 16 | 32;

function nativeEventPriority(name: string): NativeEventPriority {
	// The current native event schema contains application input only. Key,
	// paste, submit, and cursor actions are discrete; continuous pointer and
	// scroll events can be assigned their own lane when the schema grows them.
	if (name === "onScroll" || name === "onPointerMove") return "continuous";
	if (
		name === "onPress" ||
		name === "onInput" ||
		name === "onEdit" ||
		name === "onChange" ||
		name === "onSelectionChange" ||
		name === "onSubmit"
	)
		return "discrete";
	return "default";
}

function normalizeNativeUiEvent(
	event: NativeUiEvent,
): NormalizedNativeUiEvent | undefined {
	if (!isSafeU32(event.host_namespace) || event.host_namespace === 0)
		throw new TypeError("native UI event has an invalid host namespace");
	if (!isSafeU32(event.slot) || event.slot === 0)
		throw new TypeError("native UI event has an invalid occurrence slot");
	if (!isSafeU32(event.generation) || event.generation === 0)
		throw new TypeError("native UI event has an invalid occurrence generation");
	if (event.kind !== HOST_KINDS.box)
		throw new TypeError("native UI event target is not an occurrence");
	if (!isSafeU32(event.mask) || (event.mask & ~NATIVE_EVENT_MASK) !== 0)
		throw new TypeError("native UI event has an invalid subscription mask");
	const text = nullableString(event.text, "text");
	const key = nullableString(event.key, "key");
	const cursorBytes = nullableNonNegativeInteger(
		event.cursor_bytes,
		"cursor_bytes",
	);
	const revision = nullableRevision(event.revision);
	if (event.mask === 0) return undefined;
	if ((event.mask & 1) !== 0 && key === undefined)
		throw new TypeError("native press event is missing its key");
	if (
		(event.mask & (2 | 4 | 8 | 16 | 32)) !== 0 &&
		(text === undefined || cursorBytes === undefined || revision === undefined)
	)
		throw new TypeError(
			"native editor event is missing its snapshot or revision",
		);
	return {
		host_namespace: event.host_namespace,
		slot: event.slot,
		generation: event.generation,
		kind: 1,
		mask: event.mask,
		text,
		cursor_bytes: cursorBytes,
		key,
		revision,
	};
}

function isSafeU32(value: number): boolean {
	return Number.isSafeInteger(value) && value >= 0 && value <= 0xffff_ffff;
}

function nullableString(
	value: string | null | undefined,
	name: string,
): string | undefined {
	if (value === undefined || value === null) return undefined;
	if (typeof value !== "string")
		throw new TypeError(`native UI event ${name} must be a string or null`);
	return value;
}

function nullableNonNegativeInteger(
	value: number | null | undefined,
	name: string,
): number | undefined {
	if (value === undefined || value === null) return undefined;
	if (!Number.isSafeInteger(value) || value < 0)
		throw new TypeError(
			`native UI event ${name} must be a non-negative integer`,
		);
	return value;
}

function nullableRevision(
	value: number | string | null | undefined,
): number | undefined {
	if (value === undefined || value === null) return undefined;
	if (typeof value === "number") {
		if (!Number.isSafeInteger(value) || value < 0)
			throw new TypeError(
				"native UI event revision must be a non-negative integer",
			);
		return value;
	}
	if (!/^\d+$/u.test(value))
		throw new TypeError(
			"native UI event revision must be a decimal integer string",
		);
	const revision = BigInt(value);
	if (revision > BigInt(Number.MAX_SAFE_INTEGER))
		throw new RangeError(
			"native UI event revision exceeds JavaScript precision",
		);
	return Number(revision);
}

function nativeEventPayload(
	event: NormalizedNativeUiEvent,
	name: string,
): UiEvent {
	const target = {
		host_namespace: event.host_namespace,
		slot: event.slot,
		generation: event.generation,
		kind: event.kind,
	};
	if (name === "onPress") {
		const payload: UiPressEvent = {
			type: "press",
			target: Object.freeze(target),
			key: event.key ?? undefined,
		};
		return Object.freeze(payload);
	}
	const text = event.text;
	const cursorBytes = event.cursor_bytes;
	if (text === undefined || cursorBytes === undefined)
		throw new Error("validated native editor event has no snapshot");
	const editTypes: Readonly<Record<string, UiEditEvent["type"]>> = {
		onInput: "input",
		onEdit: "edit",
		onChange: "change",
		onSelectionChange: "selectionChange",
		onSubmit: "submit",
	};
	const payload: UiEditEvent = {
		type: editTypes[name] ?? "submit",
		target: Object.freeze(target),
		text,
		cursorBytes,
		key: event.key ?? undefined,
		...(event.revision === undefined ? {} : { revision: event.revision }),
	};
	return Object.freeze(payload);
}

function reportNativeEventError(error: unknown): void {
	try {
		const reportError = (
			globalThis as unknown as {
				reportError?: (error: unknown) => void;
			}
		).reportError;
		if (typeof reportError === "function") reportError(error);
		else console.error(error);
	} catch {
		// A broken diagnostic sink cannot change accepted native state or stop
		// delivery of later owned events.
	}
}

function explicitSelectionRecord(
	port: ReactContentPort,
	connector: ReactContentConnector | undefined,
): RecordValue {
	return {
		section: 2,
		opcode: UI_OPCODES.selectConnector,
		operands: [
			...resourceRef(port.handle),
			...(connector === undefined
				? NULL_HANDLE
				: resourceRef(connector.handle)),
		],
		explicitSelection: { port, connector },
	};
}

/** Native admission rejections are caller errors only for explicit resource
 * operations. Malformed acknowledgements, transport failures and invariant
 * violations still fault the root under the renderer contract. */
function isRecoverableCallerRejection(error: unknown): boolean {
	const message = error instanceof Error ? error.message : String(error);
	return /native UI commit rejected \(detail [3-7], record \d+\)/u.test(
		message,
	);
}

function linkChild(
	parent: HostInstance,
	child: HostInstance,
	before: HostInstance | undefined,
): void {
	if (before !== undefined && before.parent !== parent)
		throw new Error("React supplied an anchor from another parent");
	if (child.parent !== undefined) unlinkChild(child);
	const previous =
		before === undefined ? parent.lastChild : before.previousSibling;
	child.parent = parent;
	child.previousSibling = previous;
	child.nextSibling = before;
	if (previous === undefined) parent.firstChild = child;
	else previous.nextSibling = child;
	if (before === undefined) parent.lastChild = child;
	else before.previousSibling = child;
}

function candidateRoots(
	candidates: ReadonlySet<HostInstance>,
): Set<HostInstance> {
	const roots = new Set<HostInstance>();
	for (const candidate of candidates) {
		let root = candidate;
		const seen = new Set<HostInstance>();
		while (root.parent?.lifecycle === "candidate") {
			if (seen.has(root))
				throw new Error("candidate ownership links contain a cycle");
			seen.add(root);
			root = root.parent;
		}
		roots.add(root);
	}
	return roots;
}

function nativeAnchor(
	before: HostInstance | undefined,
): HostInstance | undefined {
	let anchor = before;
	while (
		anchor !== undefined &&
		(anchor.lifecycle === "candidate" ||
			anchor.rootRole === "portal" ||
			anchor.rootRole === "historyUnit")
	)
		anchor = anchor.nextSibling;
	return anchor;
}

function candidateAnchors(
	roots: ReadonlySet<HostInstance>,
): Map<HostInstance, HostInstance | undefined> {
	const anchors = new Map<HostInstance, HostInstance | undefined>();
	const suffixes = new Map<HostInstance, HostInstance | undefined>();
	for (const root of roots) {
		if (root.rootRole === "portal" || root.rootRole === "historyUnit") continue;
		const skipped: HostInstance[] = [];
		let anchor = root.nextSibling;
		while (
			anchor !== undefined &&
			(anchor.lifecycle === "candidate" ||
				anchor.rootRole === "portal" ||
				anchor.rootRole === "historyUnit")
		) {
			if (suffixes.has(anchor)) {
				anchor = suffixes.get(anchor);
				break;
			}
			skipped.push(anchor);
			anchor = anchor.nextSibling;
		}
		for (const sibling of skipped) suffixes.set(sibling, anchor);
		anchors.set(root, anchor);
	}
	return anchors;
}

function unlinkChild(child: HostInstance): void {
	const parent = child.parent;
	if (parent === undefined) return;
	if (child.previousSibling === undefined)
		parent.firstChild = child.nextSibling;
	else child.previousSibling.nextSibling = child.nextSibling;
	if (child.nextSibling === undefined) parent.lastChild = child.previousSibling;
	else child.nextSibling.previousSibling = child.previousSibling;
	child.parent = undefined;
	child.previousSibling = undefined;
	child.nextSibling = undefined;
}

function retireJsSubtree(instance: HostInstance): void {
	let child = instance.firstChild;
	while (child !== undefined) {
		const next = child.nextSibling;
		retireJsSubtree(child);
		child = next;
	}
	instance.lifecycle = "retired";
	instance.handle = undefined;
	instance.accepted = undefined;
	instance.initial.events.clear();
	instance.pending = { ...instance.pending, events: new Map() };
	instance.port = undefined;
	instance.connector = undefined;
	instance.control = undefined;
	instance.portalOwner = undefined;
}

function retireSpeculativeSubtree(instance: HostInstance): void {
	if (instance.accepted === undefined) {
		retireJsSubtree(instance);
		return;
	}
	let child = instance.firstChild;
	while (child !== undefined) {
		const next = child.nextSibling;
		retireSpeculativeSubtree(child);
		child = next;
	}
}

function clearJsOwnership(instance: HostInstance): void {
	let child = instance.firstChild;
	while (child !== undefined) {
		const next = child.nextSibling;
		clearJsOwnership(child);
		child = next;
	}
	instance.portToken = undefined;
	instance.connectorToken = undefined;
	instance.port = undefined;
	instance.connector = undefined;
	instance.control = undefined;
	instance.portalOwner = undefined;
}

function nodeRef(instance: HostInstance): number[] {
	if (instance.handle === undefined) {
		if (instance.localOrdinal === undefined)
			throw new Error("candidate has no local ordinal");
		return localRef(instance.localOrdinal, 1);
	}
	return [
		instance.handle.host_namespace,
		instance.handle.slot,
		instance.handle.generation,
		1,
	];
}

function validateAcknowledgement(
	ack: Uint32Array,
	expectedCreated: number,
): void {
	if (ack.length < 8) throw new Error("native UI acknowledgement is truncated");
	if ((ack[0] ?? 0) !== 0) {
		throw new Error(
			`native UI commit rejected (detail ${ack[5] ?? 0}, record ${ack[4] ?? 0})`,
		);
	}
	const createdCount = ack[3] ?? 0;
	if (createdCount !== expectedCreated)
		throw new Error(
			"native UI acknowledgement creation count does not match the journal",
		);
	if (ack.length < 8 + createdCount * UI_HANDLE_WORDS)
		throw new Error("native UI acknowledgement has truncated handles");
}

function acknowledgementHandle(ack: Uint32Array, ordinal: number): UiHandle {
	const at = 8 + (ordinal - 1) * UI_HANDLE_WORDS;
	const host_namespace = ack[at];
	const slot = ack[at + 1];
	const generation = ack[at + 2];
	const kind = ack[at + 3];
	if (
		host_namespace === undefined ||
		slot === undefined ||
		generation === undefined ||
		kind === undefined
	) {
		throw new Error(
			`native UI acknowledgement omitted creation ordinal ${ordinal}`,
		);
	}
	return { host_namespace, slot, generation, kind };
}

function resourceRef(handle: UiHandle): number[] {
	return handle.host_namespace === 0
		? localRef(handle.slot, handle.kind)
		: [handle.host_namespace, handle.slot, handle.generation, handle.kind];
}

function localRef(ordinal: number, kind: number): number[] {
	return [0, ordinal, 0, kind];
}
function localHandle(ordinal: number, kind: number): UiHandle {
	return { host_namespace: 0, slot: ordinal, generation: 0, kind };
}
function sourceIndexFor(source: ContentSource, journal: Journal): number {
	const native = nativeResourceOf<NativeTextSourceContract>(source, "source");
	const object = native as unknown as object;
	const existing = journal.sourceIndices.get(object);
	if (existing !== undefined) return existing;
	const index = journal.sources.length;
	journal.sourceIndices.set(object, index);
	journal.sources.push(native);
	return index;
}

function funnelMetadata(
	funnel: TextFunnel,
	metadata: SidecarBuilder,
): [number, number] {
	const delivery = funnel.delivery.kind === "smooth" ? "1" : "0";
	return metadata.addText(
		`${funnel.mode}:${funnel.wrap}:${funnel.hyperlinks ? "1" : "0"}:${delivery}`,
	);
}

function funnelSignature(funnel: TextFunnel): string {
	return `${funnel.mode}:${funnel.wrap}:${funnel.hyperlinks ? "1" : "0"}:${funnel.delivery.kind === "smooth" ? "1" : "0"}`;
}

function literalFunnelRecords(
	port: UiHandle,
	funnel: TextFunnel,
	metadata: SidecarBuilder,
): RecordValue[] {
	return [
		{
			section: 2,
			opcode: UI_OPCODES.setLiteralFunnel,
			operands: [...resourceRef(port), ...funnelMetadata(funnel, metadata)],
		},
	];
}

function controlMetadata(
	kind: HostKind,
	control: NonNullable<NormalizedProps["control"]>,
	metadata: SidecarBuilder,
): [number, number] {
	if (kind === HOST_KINDS.editor)
		return metadata.addBytes(Uint8Array.of(control.multiline ? 1 : 0));
	if (kind !== HOST_KINDS.animation || control.intervalMs === undefined)
		return [0, 0];
	const bytes = new Uint8Array(4);
	new DataView(bytes.buffer).setUint32(0, control.intervalMs, true);
	return metadata.addBytes(bytes);
}

function historyRootConfig(
	props: NormalizedProps,
	metadata: SidecarBuilder,
): [number, number] {
	const config = props.historyUnit;
	if (config === undefined)
		throw new Error("HistoryUnit is missing its root configuration");
	const bytes = new Uint8Array(10);
	bytes[0] = config.flowBoundary;
	return metadata.addBytes(bytes);
}

function encodeWords(
	namespace: number,
	revision: number,
	records: readonly RecordValue[],
	metadataLength: number,
	contentLength: number,
	sourceCount: number,
	localCount: number,
): Uint32Array {
	const sections: number[][] = [[], [], [], [], []];
	for (const record of records) {
		const section = sections[record.section];
		if (section === undefined)
			throw new Error(`unknown UI section ${record.section}`);
		section.push(record.opcode, record.operands.length + 2, ...record.operands);
	}
	const total =
		UI_BATCH_HEADER_WORDS +
		sections.reduce((sum, section) => sum + section.length, 0);
	const words = new Uint32Array(total);
	words.set(
		[
			UI_BATCH_MAGIC,
			UI_BATCH_VERSION,
			total,
			namespace,
			revision >>> 0,
			Math.floor(revision / 0x1_0000_0000) >>> 0,
			0,
			...sections.map((section) => section.length),
			metadataLength,
			contentLength,
			sourceCount,
			0,
		],
		0,
	);
	words[6] = localCount;
	let offset = UI_BATCH_HEADER_WORDS;
	for (const section of sections) {
		words.set(section, offset);
		offset += section.length;
	}
	return words;
}
