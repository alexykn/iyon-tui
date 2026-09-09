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
	DefaultEventPriority,
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
} from "./instance.ts";

export type HostContainer = RootContainer;

export interface HostContext {
	readonly root: RootContainer;
}

export interface PublicInstance {
	readonly kind: string;
	readonly lifecycle: "candidate" | "accepted" | "retired";
	diagnostics(): {
		readonly lifecycle: string;
		readonly hasAcceptedHandle: boolean;
	};
	setOverride(
		property: string,
		value: unknown,
	): { readonly revision: number; readonly accepted: true };
	clearOverride(property: string): {
		readonly revision: number;
		readonly accepted: true;
	};
	focus(): never;
	visibleGeometry(): Promise<never>;
}

const NO_TIMEOUT = -1;
// The installed reconciler runtime uses the numeric constants exported by
// react-reconciler/constants.js.  In particular, its DefaultEventPriority is
// 32 (the published type declaration uses a different bit spelling), so do
// not duplicate the value or treat NoEventPriority as a schedulable default.
let currentPriority = NoEventPriority;
const publicInstances = new WeakMap<HostInstance, PublicInstance>();
let hostCandidateCount = 0;

/** @internal Test evidence for speculative HostConfig creation. */
export function hostCandidateCreations(): number {
	return hostCandidateCount;
}

function publicInstance(
	instance: HostInstance | HostTextInstance,
): PublicInstance {
	const node = hostNode(instance);
	const existing = publicInstances.get(node);
	if (existing !== undefined) return existing;
	const value: PublicInstance = {
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
		focus: () => {
			const error = new Error(
				"focus publication is deferred until the T4 interaction executor",
			);
			(error as Error & { code: string }).code = "T4_FOCUS_UNREALIZED";
			throw error;
		},
		visibleGeometry: () => {
			const error = new Error(
				"visible geometry is unavailable before T4 frame realization",
			);
			(error as Error & { code: string }).code = "T3_GEOMETRY_UNREALIZED";
			return Promise.reject(error);
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
	PublicInstance,
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
