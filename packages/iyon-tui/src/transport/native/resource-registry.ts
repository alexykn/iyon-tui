import { tuiError } from "../../api/errors.ts";
import type { HandleId } from "../../api/controls/framework-handle.ts";

/**
 * Plane-neutral native resource kinds. Public planes add their own literal
 * kind without teaching this registry what the resource means.
 */
export type NativeResourceKind = string;

export type ResourceLifecycle = "live" | "disposing" | "disposed";

export interface ResourceOwner {
  readonly environment: object;
  readonly host?: object;
}

export interface ResourceRegistration {
  readonly handle: object;
  readonly resource: object;
  readonly handleId: HandleId;
  readonly kind: NativeResourceKind;
  readonly owner?: ResourceOwner;
  readonly acceptedNodeKinds?: ReadonlySet<number>;
}

export interface NativeResourceStats {
  readonly live: number;
  readonly disposing: number;
  readonly preparedLeases: number;
  readonly desiredLeases: number;
  readonly visibleLeases: number;
}

interface ResourceRecord {
  readonly handleId: HandleId;
  readonly kind: NativeResourceKind;
  readonly handleRef: WeakRef<object>;
  readonly resourceRef: WeakRef<object>;
  readonly owner?: ResourceOwner;
  readonly acceptedNodeKinds?: ReadonlySet<number>;
  readonly environment: object;
  readonly generation: number;
  lifecycle: ResourceLifecycle;
  preparedLeases: number;
  desiredLeases: number;
  visibleLeases: number;
}

interface LeaseFinalizerState {
  phase: "prepared" | "desired" | "released";
  visible: boolean;
}

interface LeaseFinalizerHeld {
  readonly registry: NativeResourceRegistry;
  readonly record: ResourceRecord;
  readonly state: LeaseFinalizerState;
}

const leaseFinalizer = new FinalizationRegistry<LeaseFinalizerHeld>((held) => {
  const { record, state } = held;
  if (state.phase === "prepared") record.preparedLeases -= 1;
  if (state.phase === "desired") record.desiredLeases -= 1;
  if (state.visible) record.visibleLeases -= 1;
  state.phase = "released";
  state.visible = false;
  held.registry.maybeFinalize(record);
});

function isWeakReferenceable(value: unknown): value is object {
  return (typeof value === "object" && value !== null) || typeof value === "function";
}

/** One short-lived H3 prepare lease for an attachment identity. */
export class PreparedResourceLease {
  private phase: "prepared" | "desired" | "released" = "prepared";
  private visible = false;
  private readonly finalizerToken = {};
  private readonly finalizerState: LeaseFinalizerState = {
    phase: "prepared",
    visible: false,
  };

  private readonly keepAlive: { readonly handle: object; readonly resource: object };

  constructor(
    private readonly registry: NativeResourceRegistry,
    private readonly record: ResourceRecord,
    handle: object,
    resource: object,
  ) {
    // Lease counts protect registry state; these strong references protect the
    // actual JS/native wrapper while a prepared, desired, or visible binding
    // still depends on it.
    this.keepAlive = { handle, resource };
    leaseFinalizer.register(
      this,
      { registry, record, state: this.finalizerState },
      this.finalizerToken,
    );
  }

  get handleId(): HandleId { return this.record.handleId; }
  get kind(): NativeResourceKind { return this.record.kind; }
  get resource(): object { return this.keepAlive.resource; }
  get generation(): number { return this.record.generation; }

  /** Commits the desired binding without making it visible yet. */
  commitDesired(): void {
    if (this.phase !== "prepared") return;
    this.phase = "desired";
    this.finalizerState.phase = "desired";
    this.record.preparedLeases -= 1;
    this.record.desiredLeases += 1;
  }

  /** Promotes this desired binding to the visible frame binding. */
  commitVisible(): void {
    if (this.phase === "prepared") this.commitDesired();
    if (this.phase !== "desired" || this.visible) return;
    this.visible = true;
    this.finalizerState.visible = true;
    this.record.visibleLeases += 1;
  }

  /** Releases a desired binding while retaining any visible binding. */
  releaseDesired(): void {
    if (this.phase !== "desired") return;
    this.phase = "released";
    this.finalizerState.phase = "released";
    this.record.desiredLeases -= 1;
    if (!this.visible) this.unregisterFinalizer();
    this.registry.maybeFinalize(this.record);
  }

  /** Releases the visible binding after a replacement frame commits. */
  releaseVisible(): void {
    if (!this.visible) return;
    this.visible = false;
    this.finalizerState.visible = false;
    this.record.visibleLeases -= 1;
    if (this.phase === "desired") return;
    this.phase = "released";
    this.finalizerState.phase = "released";
    this.unregisterFinalizer();
    this.registry.maybeFinalize(this.record);
  }

  /** Aborts a prepare lease without changing desired or visible state. */
  abort(): void {
    if (this.phase !== "prepared") return;
    this.phase = "released";
    this.finalizerState.phase = "released";
    this.record.preparedLeases -= 1;
    this.unregisterFinalizer();
    this.registry.maybeFinalize(this.record);
  }

  private unregisterFinalizer(): void {
    leaseFinalizer.unregister(this.finalizerToken);
  }
}

/**
 * A single environment-level resolver for every host-owned/native resource.
 * It is deliberately unaware of state/content/component dispatch semantics.
 */
export class NativeResourceRegistry {
  private readonly records = new Map<HandleId, ResourceRecord>();
  /** Highest retired identity; framework IDs are monotonic and never reused. */
  private retiredThrough: HandleId = 0 as HandleId;
  private readonly handles = new WeakMap<object, HandleId>();
  private readonly retiredHandles = new WeakSet<object>();
  private readonly resources = new WeakMap<object, HandleId>();
  private readonly retiredResources = new WeakSet<object>();
  private readonly finalizer = new FinalizationRegistry<HandleId>((handleId) => {
    this.finalizeUnowned(handleId);
  });
  private nextGeneration = 1;

  constructor(readonly environment: object = {}) {}

  register(registration: ResourceRegistration): void {
    if (!isWeakReferenceable(registration.handle) || !isWeakReferenceable(registration.resource)) {
      throw tuiError("validation", "native resource registration requires object handles");
    }
    if (!Number.isSafeInteger(registration.handleId) || registration.handleId < 1) {
      throw tuiError("validation", "framework handle identity must be a positive safe integer");
    }
    if (this.records.has(registration.handleId) || registration.handleId <= this.retiredThrough) {
      throw tuiError("runtime", "framework handle identity is already retired or registered", {
        id: registration.handleId,
      });
    }
    if (this.handles.has(registration.handle) || this.retiredHandles.has(registration.handle)) {
      throw tuiError("runtime", "framework value already has a native resource");
    }
    if (this.resources.has(registration.resource) || this.retiredResources.has(registration.resource)) {
      throw tuiError("runtime", "native resource is already registered", {
        id: registration.handleId,
      });
    }
    const owner = registration.owner;
    if (owner !== undefined && owner.environment !== this.environment) {
      throw tuiError("invalid-handle", "resource belongs to a different environment", {
        id: registration.handleId,
      });
    }
    if (this.nextGeneration > Number.MAX_SAFE_INTEGER) {
      throw tuiError("runtime", "native resource generation exhausted");
    }
    const acceptedNodeKinds = registration.acceptedNodeKinds === undefined
      ? undefined
      : new Set(registration.acceptedNodeKinds);
    const record: ResourceRecord = {
      handleId: registration.handleId,
      kind: registration.kind,
      handleRef: new WeakRef(registration.handle),
      resourceRef: new WeakRef(registration.resource),
      owner,
      acceptedNodeKinds,
      environment: owner?.environment ?? this.environment,
      generation: this.nextGeneration++,
      lifecycle: "live",
      preparedLeases: 0,
      desiredLeases: 0,
      visibleLeases: 0,
    };
    this.records.set(record.handleId, record);
    this.handles.set(registration.handle, record.handleId);
    this.resources.set(registration.resource, record.handleId);
    this.finalizer.register(registration.handle, record.handleId, registration.handle);
  }

  resourceForHandle(handle: object): object {
    const handleId = this.handles.get(handle);
    if (handleId === undefined) throw tuiError("disposed-handle", "framework value has no live native resource");
    return this.resourceForHandleId(handleId);
  }

  /** @internal Returns the registry identity for lifecycle checks. */
  handleIdFor(handle: object): HandleId | undefined {
    return this.handles.get(handle);
  }

  /** @internal Distinguishes retired framework values from unregistered outputs. */
  isRetiredHandle(handle: object): boolean {
    return this.retiredHandles.has(handle);
  }

  resourceForHandleId(handleId: HandleId, expectedKind?: NativeResourceKind): object {
    const record = this.records.get(handleId);
    if (record === undefined || record.lifecycle !== "live") {
      throw tuiError("disposed-handle", "framework handle has no live native resource", { id: handleId });
    }
    if (expectedKind !== undefined && record.kind !== expectedKind) {
      throw tuiError("invalid-handle", `framework resource kind must be ${expectedKind}`, {
        id: handleId,
        expectedKind,
        actualKind: record.kind,
      });
    }
    const handle = record.handleRef.deref();
    const resource = record.resourceRef.deref();
    if (handle === undefined || resource === undefined) {
      this.retireRecord(record);
      throw tuiError("disposed-handle", "framework handle has no live native resource", { id: handleId });
    }
    return resource;
  }

  /** Resolves and leases an attachment during H3 prepare. */
  prepareResolve(
    handleId: HandleId,
    expectedKind: NativeResourceKind,
    targetEnvironment: object,
    targetHost?: object,
    targetNodeKind?: number,
  ): PreparedResourceLease {
    const record = this.records.get(handleId);
    if (record === undefined || record.lifecycle === "disposed") {
      throw tuiError("disposed-handle", "framework attachment is disposed", { id: handleId });
    }
    if (record.lifecycle === "disposing") {
      throw tuiError("invalid-handle", "framework attachment is disposing", { id: handleId });
    }
    if (record.preparedLeases > 0) {
      throw tuiError("validation", "framework attachment is already prepared for another candidate", {
        id: handleId,
      });
    }
    const handle = record.handleRef.deref();
    const resource = record.resourceRef.deref();
    if (handle === undefined || resource === undefined) {
      this.retireRecord(record);
      throw tuiError("disposed-handle", "framework attachment has no live owner/resource", { id: handleId });
    }
    if (record.kind !== expectedKind) {
      throw tuiError("invalid-handle", `framework attachment kind must be ${expectedKind}`, {
        id: handleId,
        expectedKind,
        actualKind: record.kind,
      });
    }
    if (record.environment !== targetEnvironment) {
      throw tuiError("invalid-handle", "framework attachment belongs to a different environment", {
        id: handleId,
      });
    }
    if (record.owner?.host !== undefined && record.owner.host !== targetHost) {
      throw tuiError("invalid-handle", "WRONG_HOST: framework attachment belongs to a different host", {
        id: handleId,
      });
    }
    if (targetNodeKind !== undefined
      && record.acceptedNodeKinds !== undefined
      && !record.acceptedNodeKinds.has(targetNodeKind)) {
      throw tuiError(
        "validation",
        expectedKind === "state"
          ? "UNSUPPORTED_STATE_ATTACHMENT: framework state attachment is unsupported on this node kind"
          : "UNSUPPORTED_CONTENT_PORT_ATTACHMENT: framework content attachment is unsupported on this node kind",
        {
          id: handleId,
          nodeKind: targetNodeKind,
        },
      );
    }
    record.preparedLeases += 1;
    return new PreparedResourceLease(this, record, handle, resource);
  }

  beginDisposal(handleId: HandleId): void {
    const record = this.records.get(handleId);
    if (record === undefined || record.lifecycle === "disposed") return;
    if (record.preparedLeases > 0 || record.desiredLeases > 0 || record.visibleLeases > 0) {
      throw tuiError(
        "invalid-handle",
        record.kind === "state"
          ? "STATE_MOUNTED: ViewState is still attached"
          : "resource is still attached",
        { id: handleId },
      );
    }
    record.lifecycle = "disposing";
  }

  release(handleId: HandleId): void {
    const record = this.records.get(handleId);
    if (record === undefined) return;
    if (record.preparedLeases > 0 || record.desiredLeases > 0 || record.visibleLeases > 0) {
      throw tuiError("invalid-handle", "resource still has active leases", { id: handleId });
    }
    this.retireRecord(record);
  }

  /** Invalidates all host-owned resources during owner teardown. */
  invalidateHost(host: object): void {
    for (const record of [...this.records.values()]) {
      if (record.owner?.host !== host) continue;
      record.lifecycle = "disposing";
      this.maybeFinalize(record);
    }
  }

  /** Test/diagnostic registration for the first internal H3 fixtures. */
  registerInternal(
    handle: object,
    handleId: HandleId,
    kind: NativeResourceKind,
    resource: object = handle,
    owner?: ResourceOwner,
    acceptedNodeKinds?: ReadonlySet<number>,
  ): void {
    this.register({ handle, handleId, kind, resource, owner, acceptedNodeKinds });
  }

  stats(): NativeResourceStats {
    let live = 0;
    let disposing = 0;
    let preparedLeases = 0;
    let desiredLeases = 0;
    let visibleLeases = 0;
    for (const record of this.records.values()) {
      if (record.lifecycle === "live") live += 1;
      if (record.lifecycle === "disposing") disposing += 1;
      preparedLeases += record.preparedLeases;
      desiredLeases += record.desiredLeases;
      visibleLeases += record.visibleLeases;
    }
    return { live, disposing, preparedLeases, desiredLeases, visibleLeases };
  }

  /** @internal */
  maybeFinalize(record: ResourceRecord): void {
    if (record.lifecycle === "disposing"
      && record.preparedLeases === 0
      && record.desiredLeases === 0
      && record.visibleLeases === 0) {
      this.retireRecord(record);
    }
  }

  private retireRecord(record: ResourceRecord): void {
    record.lifecycle = "disposed";
    this.records.delete(record.handleId);
    if (record.handleId > this.retiredThrough) this.retiredThrough = record.handleId;
    const handle = record.handleRef.deref();
    if (handle !== undefined) {
      this.handles.delete(handle);
      this.retiredHandles.add(handle);
      this.finalizer.unregister(handle);
    }
    const resource = record.resourceRef.deref();
    if (resource !== undefined && this.resources.get(resource) === record.handleId) {
      this.resources.delete(resource);
      this.retiredResources.add(resource);
    }
  }

  private finalizeUnowned(handleId: HandleId): void {
    const record = this.records.get(handleId);
    if (record === undefined || record.handleRef.deref() !== undefined) return;
    if (record.preparedLeases === 0 && record.desiredLeases === 0 && record.visibleLeases === 0) {
      this.retireRecord(record);
      return;
    }
    // Preserve native use until the outstanding frame/prepare leases drain,
    // but prevent a finalized JS wrapper from becoming attachable again.
    record.lifecycle = "disposing";
  }
}

const ENVIRONMENT_KEY = Symbol.for("iyon:tui:resource-environment");
const REGISTRY_KEY = Symbol.for("iyon:tui:native-resource-registry");
type RuntimeGlobals = typeof globalThis & {
  [ENVIRONMENT_KEY]?: object;
  [REGISTRY_KEY]?: NativeResourceRegistry;
};
const globals = globalThis as RuntimeGlobals;
const environment = globals[ENVIRONMENT_KEY] ??= {};

/** The one JavaScript-realm environment used by framework handles. */
export function runtimeResourceEnvironment(): object {
  return environment;
}

/** The one resolver shared by native/control and structural seams. */
export function runtimeResourceRegistry(): NativeResourceRegistry {
  return globals[REGISTRY_KEY] ??= new NativeResourceRegistry(environment);
}
