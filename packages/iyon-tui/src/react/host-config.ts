/*
 * React 19.2 / react-reconciler 0.33 host shim.
 *
 * Keep this file deliberately boring: it is the only place that spells the
 * experimental HostConfig surface.  All native work is delegated to the
 * occurrence commit coordinator; render callbacks only create and link JS
 * candidates.
 */
import { createContext } from "react";
import type ReactReconciler from "react-reconciler";
import {
	ContinuousEventPriority,
	DefaultEventPriority,
	DiscreteEventPriority,
	NoEventPriority,
} from "react-reconciler/constants.js";
import type { RootContainer } from "./commit.ts";
import {
	type HostChild,
	HostInstance,
	HostTextInstance,
	type HostType,
	hostKindForType,
	hostNode,
	normalizeProps,
	type OccurrenceRef,
} from "./instance.ts";

export type HostContainer = RootContainer;

export interface HostContext {
	readonly root: RootContainer;
}

const NO_TIMEOUT = -1;
// The installed reconciler runtime uses the numeric constants exported by
// react-reconciler/constants.js.  In particular, its DefaultEventPriority is
// 32 (the published type declaration uses a different bit spelling), so do
// not duplicate the value or treat NoEventPriority as a schedulable default.
let currentPriority = NoEventPriority;
const publicInstances = new WeakMap<HostInstance, OccurrenceRef>();
let hostCandidateCount = 0;

export type NativeEventPriority = "discrete" | "continuous" | "default";

/** Runs a native callback at its React priority and always restores the lane. */
export function withNativeEventPriority<T>(
	priority: NativeEventPriority,
	callback: () => T,
): T {
	const previous = currentPriority;
	currentPriority =
		priority === "discrete"
			? DiscreteEventPriority
			: priority === "continuous"
				? ContinuousEventPriority
				: DefaultEventPriority;
	try {
		return callback();
	} finally {
		currentPriority = previous;
	}
}

/** @internal Test evidence for speculative HostConfig creation. */
export function hostCandidateCreations(): number {
	return hostCandidateCount;
}

function publicInstance(
	instance: HostInstance | HostTextInstance,
): OccurrenceRef {
	const node = hostNode(instance);
	const existing = publicInstances.get(node);
	if (existing !== undefined) return existing;
	const value: OccurrenceRef = {
		get kind() {
			return node.kind === 1
				? "box"
				: node.kind === 2
					? "content"
					: node.kind === 3
						? "editor"
						: node.kind === 4
							? "scroll"
							: "animation";
		},
		get lifecycle() {
			return node.lifecycle;
		},
		diagnostics: () => ({
			lifecycle: node.lifecycle,
			hasAcceptedHandle: node.accepted !== undefined,
		}),
		setOverride: (property, value) =>
			node.root.coordinator.publishOverride(node, property, value),
		clearOverride: (property) =>
			node.root.coordinator.clearOverride(node, property),
		setStyleState: (key, value) =>
			node.root.coordinator.publishStyleState(node, key, value),
		clearStyleState: (key) => node.root.coordinator.clearStyleState(node, key),
		focus: () => {
			const handle = node.accepted?.handle;
			if (handle === undefined)
				throw new Error("cannot focus an occurrence before native acceptance");
			node.root.host.focusUi([
				handle.host_namespace,
				handle.slot,
				handle.generation,
				handle.kind,
			]);
		},
		interceptPaste: (routeId) => {
			if (typeof routeId !== "string" || routeId.length === 0)
				throw new TypeError("paste routeId must be a nonempty string");
			const handle = node.accepted?.handle;
			if (handle === undefined)
				throw new Error("cannot intercept paste before occurrence acceptance");
			const intercept = node.root.host.interceptPasteUi;
			if (intercept === undefined)
				throw new Error("native occurrence paste interception is unavailable");
			intercept.call(
				node.root.host,
				[handle.host_namespace, handle.slot, handle.generation, handle.kind],
				routeId,
			);
		},
		visibleGeometry: async () => {
			const handle = node.accepted?.handle;
			if (handle === undefined)
				throw new Error(
					"cannot query geometry before the occurrence is accepted",
				);
			return node.root.host.uiVisibleGeometry([
				handle.host_namespace,
				handle.slot,
				handle.generation,
				handle.kind,
			]);
		},
		historyIdentity: () => {
			const handle = node.accepted?.handle;
			if (handle === undefined)
				throw new Error(
					"cannot query History identity before native acceptance",
				);
			if (node.rootRole !== "historyUnit")
				throw new Error("History identity requires a HistoryUnit occurrence");
			const identity = node.root.host.uiHistoryUnitIdentity([
				handle.host_namespace,
				handle.slot,
				handle.generation,
				handle.kind,
			]);
			if (identity === null)
				throw new Error(
					"History identity is not installed for the accepted unit",
				);
			return identity;
		},
	};
	publicInstances.set(node, value);
	return value;
}

type HostConfig = ReactReconciler.HostConfig<
	HostType,
	unknown,
	HostContainer,
	HostInstance,
	HostTextInstance,
	never,
	never,
	never,
	OccurrenceRef,
	HostContext,
	never,
	number,
	typeof NO_TIMEOUT,
	null
>;

/** Installed React 19 host contract (mutation mode only). */
export const hostConfig: HostConfig = {
	supportsMutation: true,
	supportsPersistence: false,
	supportsHydration: false,
	createInstance(type, props, rootContainer): HostInstance {
		hostCandidateCount += 1;
		if (
			type === "portal" &&
			(props as { readonly portalOwner?: object }).portalOwner !== rootContainer
		)
			throw new Error("cross-host portal target");
		const instance = new HostInstance(
			rootContainer,
			hostKindForType(type),
			normalizeProps(type, props),
		);
		if (type === "portal") instance.rootRole = "portal";
		if (type === "historyUnit") instance.rootRole = "historyUnit";
		return instance;
	},
	createTextInstance(text, rootContainer): HostTextInstance {
		return new HostTextInstance(rootContainer, text);
	},
	appendInitialChild(parent, child): void {
		parent.root.coordinator.appendInitialChild(parent, child);
	},
	finalizeInitialChildren(): boolean {
		return false;
	},
	shouldSetTextContent(): boolean {
		return false;
	},
	getRootHostContext(rootContainer): HostContext {
		return { root: rootContainer };
	},
	getChildHostContext(parentHostContext): HostContext {
		return parentHostContext;
	},
	getPublicInstance: publicInstance,
	prepareForCommit(container): null {
		container.coordinator.begin();
		return null;
	},
	resetAfterCommit(container): void {
		container.coordinator.finish();
	},
	preparePortalMount(): void {},
	scheduleTimeout(fn, delay): number {
		return setTimeout(fn, delay ?? 0) as unknown as number;
	},
	cancelTimeout(id): void {
		clearTimeout(id);
	},
	noTimeout: NO_TIMEOUT,
	supportsMicrotasks: true,
	scheduleMicrotask(fn): void {
		queueMicrotask(fn);
	},
	isPrimaryRenderer: true,
	warnsIfNotActing: false,
	getInstanceFromNode(): null {
		return null;
	},
	beforeActiveInstanceBlur(): void {},
	afterActiveInstanceBlur(): void {},
	prepareScopeUpdate(): void {},
	getInstanceFromScope(): null {
		return null;
	},
	detachDeletedInstance(instance): void {
		// Final JS reference cleanup follows the accepted occurrence retirement;
		// the coordinator separately revisits only deferred stale-token cleanup.
		instance.root.coordinator.detachDeletedInstance(instance);
		if (instance.lifecycle === "retired") instance.accepted = undefined;
	},
	appendChild(parent, child): void {
		parent.root.coordinator.appendChild(parent, child);
	},
	appendChildToContainer(container, child): void {
		container.coordinator.appendChildToContainer(child);
	},
	insertBefore(parent, child, before): void {
		parent.root.coordinator.insertBefore(parent, child, before as HostChild);
	},
	insertInContainerBefore(container, child, before): void {
		container.coordinator.insertInContainerBefore(child, before as HostChild);
	},
	removeChild(parent, child): void {
		parent.root.coordinator.removeChild(parent, child);
	},
	removeChildFromContainer(container, child): void {
		container.coordinator.removeChildFromContainer(child);
	},
	commitTextUpdate(instance, _oldText, newText): void {
		instance.root.coordinator.updateText(instance, newText);
	},
	commitUpdate(instance, type, _prevProps, nextProps): void {
		instance.root.coordinator.update(instance, type, nextProps);
	},
	hideInstance(instance): void {
		instance.root.coordinator.hide(instance);
	},
	hideTextInstance(instance): void {
		instance.root.coordinator.hide(instance.node);
	},
	unhideInstance(instance): void {
		instance.root.coordinator.unhide(instance);
	},
	unhideTextInstance(instance): void {
		instance.root.coordinator.unhide(instance.node);
	},
	clearContainer(container): void {
		container.coordinator.clearContainer();
	},
	NotPendingTransition: null,
	HostTransitionContext: createContext<null>(null) as never,
	setCurrentUpdatePriority(priority): void {
		currentPriority = priority;
	},
	getCurrentUpdatePriority(): number {
		return currentPriority;
	},
	resolveUpdatePriority(): number {
		return currentPriority === NoEventPriority
			? DefaultEventPriority
			: currentPriority;
	},
	resetFormInstance(): void {},
	requestPostPaintCallback(callback): void {
		queueMicrotask(() => callback(Date.now()));
	},
	shouldAttemptEagerTransition(): boolean {
		return false;
	},
	trackSchedulerEvent(): void {},
	resolveEventType(): null {
		return null;
	},
	resolveEventTimeStamp(): number {
		return Date.now();
	},
	maySuspendCommit(): boolean {
		return false;
	},
	preloadInstance(): boolean {
		return true;
	},
	startSuspendingCommit(): void {},
	suspendInstance(): void {},
	waitForCommitToBeReady(): null {
		return null;
	},
};
