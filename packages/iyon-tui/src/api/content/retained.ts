import { tuiError, asTuiError } from "../errors.ts";
import { FrameworkHandle } from "../controls/framework-handle.ts";
import type { HandleId } from "../controls/framework-handle.ts";
import { releaseFrameworkHandle } from "../../runtime/handle-registry.ts";
import {
  runtimeResourceRegistry,
} from "../../transport/native/resource-registry.ts";
import { nativeResourceOf } from "../../transport/native/resources.ts";
import {
  activateContent,
  connectContent,
  contentConnectorStatus,
  contentPortMounted,
  createTextSource,
  deactivateContent,
  deactivatePort,
  disposeContentConnector,
  type NativeContentConnectorContract,
  type NativeContentPortContract,
  type NativeStateWake,
  type NativeTextSourceContract,
} from "../../transport/content/control.ts";
import { SEMANTIC_VIEW_KIND } from "../view/semantic-node.ts";
import type { TextContent } from "./text-content.ts";

export type ContentFamily = "text";

export interface TextRetentionPolicy {
  readonly maxBytes?: number;
  readonly maxLines?: number;
  readonly overflow: "drop-oldest" | "error";
}

export interface TextSourceOptions {
  readonly retention?: TextRetentionPolicy;
}

export interface ContentPortOptions {
  readonly family?: ContentFamily;
}

export type TextFunnelWrap = "word" | "grapheme" | "noWrap";

export interface TextFunnelOptions {
  readonly wrap?: TextFunnelWrap;
}

export interface Funnel<TContent = TextContent> {
  readonly kind: "text-funnel";
  readonly family: ContentFamily;
  /** Phantom content family marker; a Funnel carries no Source data. */
  readonly __content?: TContent;
}

export type ContentSource = TextStreamSource | TextBlockSource;
export type Source<TContent = TextContent> = ContentSource & { readonly content?: TContent };

export type ContentConnectorPhase =
  | "idle"
  | "waiting-for-mount"
  | "activation-pending"
  | "active"
  | "failed"
  | "disposing"
  | "disposed"
  | "blocked-geometry"
  | "unsupported-backend";

export interface ContentConnectorError {
  readonly code: string;
  readonly diagnostic: string;
}

export interface ContentConnectorStatus {
  readonly phase: ContentConnectorPhase;
  readonly requested: boolean;
  readonly visible: boolean;
  readonly projectedSourceRevision?: bigint;
  readonly error?: ContentConnectorError;
}

interface Owner {
  readonly environment: object;
  readonly host: object;
}

interface SourceOwner {
  readonly environment: object;
}

function validateTextSourceOptions(options: TextSourceOptions): void {
  if (typeof options !== "object" || options === null) {
    throw new TypeError("text source options must be an object");
  }
  for (const key of Object.keys(options)) {
    if (key !== "retention") throw new RangeError(`unknown text source option ${JSON.stringify(key)}`);
  }
  const retention = options.retention;
  if (retention === undefined) return;
  if (typeof retention !== "object" || retention === null) {
    throw new TypeError("text source retention must be an object");
  }
  if (retention.maxBytes === undefined && retention.maxLines === undefined) {
    throw new RangeError("text source retention requires maxBytes or maxLines");
  }
  for (const key of Object.keys(retention)) {
    if (key !== "maxBytes" && key !== "maxLines" && key !== "overflow") {
      throw new RangeError(`unknown text source retention option ${JSON.stringify(key)}`);
    }
  }
  for (const [name, value] of [["maxBytes", retention.maxBytes], ["maxLines", retention.maxLines]] as const) {
    if (value !== undefined && (!Number.isSafeInteger(value) || value <= 0)) {
      throw new RangeError(`text source retention ${name} must be a positive safe integer`);
    }
  }
  if (retention.overflow !== "drop-oldest" && retention.overflow !== "error") {
    throw new RangeError("text source retention overflow must be drop-oldest or error");
  }
}

function nativeWake(wake: NativeStateWake, requestWake: () => void): void {
  if (wake.schedule_environment_drain) requestWake();
}

/** Environment-owned text Source identity. Payload mutation arrives in E. */
export class TextStreamSource extends FrameworkHandle<"source"> {
  private constructor(resource: object, owner: SourceOwner) {
    try {
      super("source", resource as never, { owner });
    } catch (error) {
      try {
        (resource as NativeTextSourceContract).dispose();
      } catch (cleanupError) {
        throw new AggregateError([error, cleanupError], "Text Source registration cleanup failed");
      }
      throw error;
    }
  }

  static create(options: TextSourceOptions = {}): TextStreamSource {
    validateTextSourceOptions(options);
    return new TextStreamSource(createTextSource("stream", options), {
      environment: runtimeResourceRegistry().environment,
    });
  }
}

/** Environment-owned replacement-style text Source identity. */
export class TextBlockSource extends FrameworkHandle<"source"> {
  private constructor(resource: object, owner: SourceOwner) {
    try {
      super("source", resource as never, { owner });
    } catch (error) {
      try {
        (resource as NativeTextSourceContract).dispose();
      } catch (cleanupError) {
        throw new AggregateError([error, cleanupError], "Text Source registration cleanup failed");
      }
      throw error;
    }
  }

  static create(options: TextSourceOptions = {}): TextBlockSource {
    validateTextSourceOptions(options);
    return new TextBlockSource(createTextSource("block", options), {
      environment: runtimeResourceRegistry().environment,
    });
  }
}

/** Immutable, Source-neutral text transformation configuration. */
export class TextFunnel implements Funnel<TextContent> {
  readonly kind = "text-funnel" as const;
  readonly family = "text" as const;
  readonly mode = "plain" as const;
  readonly wrap: TextFunnelWrap;

  private constructor(options: TextFunnelOptions) {
    this.wrap = options.wrap ?? "word";
    if (this.wrap !== "word" && this.wrap !== "grapheme" && this.wrap !== "noWrap") {
      throw new RangeError("text Funnel wrap mode is invalid");
    }
    Object.freeze(this);
  }

  static plain(options: TextFunnelOptions = {}): TextFunnel {
    if (typeof options !== "object" || options === null) {
      throw new TypeError("text Funnel options must be an object");
    }
    for (const key of Object.keys(options)) {
      if (key !== "wrap") throw new RangeError(`unknown text Funnel option ${JSON.stringify(key)}`);
    }
    return new TextFunnel(options);
  }

}

function textFunnelNative(funnel: TextFunnel): object {
  return { family: funnel.family, kind: funnel.mode, wrap: funnel.wrap };
}

/** Host-owned structural ContentPort. It owns Connector membership, not data. */
export class ContentPort<TContent = TextContent> extends FrameworkHandle<"content-port"> {
  readonly kind = "content-port" as const;
  private readonly requestWake: () => void;
  private readonly assertMutationAllowed: () => void;
  private readonly owner: Owner;
  private readonly connectors = new Set<ContentConnector<TContent>>();

  private constructor(
    resource: object,
    owner: Owner,
    requestWake: () => void,
    assertMutationAllowed: () => void,
  ) {
    try {
      super("content-port", resource as never, {
        owner,
        acceptedNodeKinds: new Set([SEMANTIC_VIEW_KIND.contentHost]),
      });
    } catch (error) {
      try {
        (resource as NativeContentPortContract).dispose();
      } catch (cleanupError) {
        throw new AggregateError([error, cleanupError], "ContentPort registration cleanup failed");
      }
      throw error;
    }
    this.owner = owner;
    this.requestWake = requestWake;
    this.assertMutationAllowed = assertMutationAllowed;
  }

  connect(source: ContentSource, funnel: Funnel<TContent>): ContentConnector<TContent> {
    this.assertMutationAllowed();
    return this.call(() => {
      if (typeof source !== "object" || source === null || source.kind !== "source" || source.disposed) {
        throw new TypeError("ContentPort.connect requires a live Source");
      }
      if (!(funnel instanceof TextFunnel) || funnel.family !== "text") {
        throw tuiError("validation", "INVALID_FUNNEL: ContentPort.connect requires a text Funnel");
      }
      const sourceResource = nativeResourceOf<NativeTextSourceContract>(source, "source");
      const portResource = this.nativeAs<NativeContentPortContract>();
      const connectorResource = connectContent(portResource, sourceResource, textFunnelNative(funnel));
      const connector = ContentConnector.create(
        connectorResource,
        this.owner,
        this.requestWake,
        this.assertMutationAllowed,
        this,
        source,
      );
      this.connectors.add(connector);
      return connector;
    });
  }

  override dispose(): void {
    super.dispose();
    for (const connector of this.connectors) connector.syncNativeLifecycle();
  }

  /** @internal Reconciles Connector disposal after a host frame commit. */
  syncNativeLifecycles(): void {
    for (const connector of this.connectors) connector.syncNativeLifecycle();
  }

  /** Requests the currently selected Connector be removed at the next frame. */
  deactivate(): void {
    this.assertMutationAllowed();
    this.call(() => {
      nativeWake(deactivatePort(this.nativeAs<NativeContentPortContract>()), this.requestWake);
    });
  }

  /** Whether this Port is part of the visible committed frame. */
  mounted(): boolean {
    return this.call(() => contentPortMounted(this.nativeAs<NativeContentPortContract>()));
  }

  isMounted(): boolean { return this.mounted(); }

  connectorCount(): number {
    return this.connectors.size;
  }

  /** @internal Keeps wrapper membership aligned after Connector finalization. */
  forgetConnector(connector: ContentConnector<TContent>): void {
    this.connectors.delete(connector);
  }

  static create(
    resource: object,
    owner: Owner,
    requestWake: () => void,
    assertMutationAllowed: () => void,
  ): ContentPort {
    return new ContentPort(resource, owner, requestWake, assertMutationAllowed);
  }
}

/** Host-owned link between exactly one Source, Funnel, and ContentPort. */
export class ContentConnector<TContent = TextContent> extends FrameworkHandle<"connector"> {
  readonly kind = "connector" as const;
  private readonly nativeResource: NativeContentConnectorContract;
  private readonly requestWake: () => void;
  private readonly assertMutationAllowed: () => void;
  private readonly port: ContentPort<TContent>;
  private readonly source: ContentSource;
  private disposalRequested = false;
  private wrapperDisposed = false;

  private constructor(
    resource: object,
    owner: Owner,
    requestWake: () => void,
    assertMutationAllowed: () => void,
    port: ContentPort<TContent>,
    source: ContentSource,
  ) {
    try {
      super("connector", resource as never, { owner });
    } catch (error) {
      try {
        (resource as NativeContentConnectorContract).dispose();
      } catch (cleanupError) {
        throw new AggregateError([error, cleanupError], "Connector registration cleanup failed");
      }
      throw error;
    }
    this.nativeResource = resource as NativeContentConnectorContract;
    this.requestWake = requestWake;
    this.assertMutationAllowed = assertMutationAllowed;
    this.port = port;
    this.source = source;
  }

  static create<TContent>(
    resource: object,
    owner: Owner,
    requestWake: () => void,
    assertMutationAllowed: () => void,
    port: ContentPort<TContent>,
    source: ContentSource,
  ): ContentConnector<TContent> {
    return new ContentConnector(resource, owner, requestWake, assertMutationAllowed, port, source);
  }

  override get disposed(): boolean {
    return this.wrapperDisposed;
  }

  activate(): void {
    this.assertMutationAllowed();
    this.ensureControlOpen();
    try {
      nativeWake(activateContent(this.nativeResource), this.requestWake);
    } catch (error) {
      throw asTuiError(error);
    }
  }

  deactivate(): void {
    this.assertMutationAllowed();
    this.ensureControlOpen();
    try {
      nativeWake(deactivateContent(this.nativeResource), this.requestWake);
    } catch (error) {
      throw asTuiError(error);
    }
  }

  status(): ContentConnectorStatus {
    try {
      const status = contentConnectorStatus(this.nativeResource) as {
        readonly phase: ContentConnectorPhase;
        readonly requested: boolean;
        readonly visible: boolean;
        readonly projectedSourceRevision?: string | number;
        readonly error?: ContentConnectorError | null;
      };
      if (status.phase === "disposed") this.finalizeWrapper();
      return {
        phase: status.phase,
        requested: status.requested,
        visible: status.visible,
        ...(status.projectedSourceRevision === undefined
          ? {}
          : { projectedSourceRevision: BigInt(status.projectedSourceRevision) }),
        ...(status.error === undefined || status.error === null ? {} : { error: status.error }),
      };
    } catch (error) {
      throw asTuiError(error);
    }
  }

  override dispose(): void {
    if (this.wrapperDisposed || this.disposalRequested) return;
    this.assertMutationAllowed();
    try {
      runtimeResourceRegistry().beginDisposal(this.id);
      nativeWake(disposeContentConnector(this.nativeResource), this.requestWake);
      this.disposalRequested = true;
      const status = contentConnectorStatus(this.nativeResource) as { readonly phase: ContentConnectorPhase };
      if (status.phase === "disposed") this.finalizeWrapper();
    } catch (error) {
      runtimeResourceRegistry().cancelDisposal(this.id);
      throw asTuiError(error);
    }
  }

  /** @internal Reconciles transactional disposal after a host commit. */
  syncNativeLifecycle(): void {
    if (this.wrapperDisposed) return;
    const status = contentConnectorStatus(this.nativeResource) as { readonly phase: ContentConnectorPhase };
    if (status.phase === "disposed") this.finalizeWrapper();
  }

  private ensureControlOpen(): void {
    if (this.wrapperDisposed || this.disposalRequested) {
      throw tuiError("disposed-handle", "connector is disposing or disposed", { id: this.id });
    }
    // FrameworkHandle's private lifecycle remains the final guard for the
    // ordinary registration path; this class owns the intermediate disposing
    // phase so status remains observable until the removal frame commits.
    super.ensureOpen();
  }

  private finalizeWrapper(): void {
    if (this.wrapperDisposed) return;
    this.wrapperDisposed = true;
    this.port.forgetConnector(this);
    releaseFrameworkHandle(this);
  }

  get attachedPort(): ContentPort<TContent> { return this.port; }
  get attachedSource(): ContentSource { return this.source; }
}

/** @internal Constructs a Tui-owned ContentPort wrapper. */
export function createContentPort(
  resource: object,
  owner: Owner,
  requestWake: () => void,
  assertMutationAllowed: () => void,
): ContentPort {
  return ContentPort.create(resource, owner, requestWake, assertMutationAllowed);
}

export type ContentConnectorHandle<TContent = TextContent> = ContentConnector<TContent>;
export type ContentPortHandle<TContent = TextContent> = ContentPort<TContent>;
