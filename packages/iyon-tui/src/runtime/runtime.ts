import { native, requireNativeClass } from "../transport/native/addon.ts";
import { nativeResourceOf } from "../transport/native/resources.ts";
import { borderNodeFor, materializeTheme } from "../transport/structural/style-lowering.ts";
import { componentViewForHandle, View } from "../api/view/view.ts";
import { retainSemanticAttachmentReference, semanticNodeOf } from "../api/view/semantic-node.ts";
import { asTuiError, tuiError } from "../api/errors.ts";
import { registerRuntimeAccess } from "./access.ts";
import { runtimeEnvironment } from "./environment.ts";
import { RuntimeErrorChannel, type RuntimeErrorReporter } from "./error-channel.ts";
import {
  AttachmentBindingState,
  prepareSemanticAttachments,
  validateSemanticAttachments,
  type AttachmentRuntimeContext,
} from "./attachments.ts";
import type { NativeHostCommit, RuntimeHostRegistration } from "./wake-broker.ts";
import { Scene } from "../api/view/scene.ts";
import { bindHistoryLifetime, createHistoryHandle } from "../api/controls/history.ts";
import { createTextInput, textInputForOutput } from "../api/controls/text-input.ts";
import { createViewSlot } from "../api/controls/view-slot.ts";
import { createScrollPane } from "../api/controls/scroll-pane.ts";
import { createViewState, type ViewState as ViewStateContract } from "../api/view/retained-state.ts";
import { createContentPort as createContentPortHandle, type ContentPort as ContentPortContract, type ContentPortOptions } from "../api/content/retained.ts";
import { createContentPort as createContentPortResource } from "../transport/content/control.ts";
import { TextContent } from "../api/content/text-content.ts";
import {
  nativeViewAbiSession,
  recordNativeViewRoute,
  tryNativeMaterialize,
} from "../transport/structural/native-view-abi.ts";
import {
  resetStyleRefCacheForThemeChange,
  RetainedRootBoundary,
  setRootColdMaterializer,
} from "../transport/structural/retained-dag.ts";
import type { RootPublication } from "../transport/structural/retained-dag.ts";
import { OwnedBuilderRoot, RetainedExecutionRuntime } from "../composition/execution.ts";
import { activeExecutionScope, protocolState, withoutRetainedComposition } from "../composition/execution-context.ts";
import type { Output } from "../api/controls/output.ts";
import type { History as HistoryContract } from "../api/controls/history.ts";
import type { ScrollPane as ScrollPaneContract } from "../api/controls/scroll-pane.ts";
import type { TextInput as TextInputContract, TextInputOptions } from "../api/controls/text-input.ts";
import type { ViewSlot as ViewSlotContract } from "../api/controls/view-slot.ts";
import type { SceneContract, SceneProducer } from "../api/view/scene.ts";
import type { TuiEvent } from "./events.ts";
import { themeDefinitionFor } from "../api/presentation/theme.ts";
import type { Theme } from "../api/presentation/theme.ts";
import type { NativeHistoryContract, NativeTuiHostContract, NativeTuiOutputContract } from "../transport/native/addon.ts";

/** Current terminal dimensions reported by a runtime. */
export interface TerminalMetadata {
  readonly width: number;
  readonly height: number;
}

export interface TuiOpenOptions {
  readonly width?: number;
  readonly height?: number;
  readonly headless?: boolean;
  readonly signal?: AbortSignal;
  readonly theme?: Theme;
}

export interface TuiRuntime {
  /** The current terminal size; it changes only after a successful resize. */
  readonly size: TerminalMetadata;
  nextEvent(signal?: AbortSignal): Promise<TuiEvent>;
  /**
   * Render either a structural scene value or a retained scene producer.
   * Direct values take over the root immediately; producers own the retained
   * root and remain subscribed to tracked state. These paths are distinct.
   */
  render(scene: SceneProducer, signal?: AbortSignal): void;
  /** Flushes through the host epoch captured at the barrier entry. */
  flush(): void;
  /**
   * Registers a host runtime-error listener; returning true consumes the
   * record and suppresses the fallback reporter. The returned function removes
   * the listener.
   */
  onRuntimeError(listener: RuntimeErrorReporter): () => void;
  resize(width: number, height: number): void;
  close(): void;
  exit(): void;
  createHistory(): HistoryContract;
  viewState(): ViewStateContract;
  contentPort(options?: ContentPortOptions | typeof TextContent): ContentPortContract;
  createTextInput(options?: TextInputOptions): TextInputContract;
  createViewSlot(initial: View): ViewSlotContract;
  createScrollPane(initial: View): ScrollPaneContract;
  bindKey(key: string, routeId: string, modifiers?: readonly string[]): void;
  route(output: Output<string>, routeId: string): void;
  interceptPaste(input: TextInputContract, routeId: string): void;
  forwardPaste(text: string): void;
  setTheme(theme: Theme): void;
}

type OwnedHandle = { dispose(): void };
const historyOwners = new WeakMap<object, Tui>();

function assertHistoryOwner(history: HistoryContract, owner: Tui): void {
  const currentOwner = historyOwners.get(history);
  if (currentOwner !== undefined && currentOwner !== owner) {
    throw tuiError("terminal", "TUI_HISTORY_ALREADY_BOUND: a History instance is already attached to a different Tui");
  }
}

function claimHistoryOwner(history: HistoryContract, owner: Tui): void {
  assertHistoryOwner(history, owner);
  historyOwners.set(history, owner);
}

export class Tui implements TuiRuntime {
  private closed = false;
  private readonly host: NativeTuiHostContract;
  private width: number;
  private height: number;
  private currentScene?: Scene;
  /**
   * PERF-13-A root boundary: desired and visible roots keep independent leases
   * during a failed or superseded frame transition.
   */
  private boundary?: RetainedRootBoundary;

  /**
   * PERF-12 T13.1 R8: ONE retained execution runtime per Tui, created eagerly.
   * The root scene scope and every slot/pane builder root participate in this
   * single dirty queue / batch / R7 transaction protocol.
   */
  private readonly retainedRuntime: RetainedExecutionRuntime;
  private readonly runtimeEnvironment = runtimeEnvironment();
  private readonly runtimeErrors = new RuntimeErrorChannel();
  private readonly hostRegistration: RuntimeHostRegistration;
  private readonly attachmentContext: AttachmentRuntimeContext;
  private runtimeErrorListener: RuntimeErrorReporter | undefined;
  private readonly attachmentBindings = new AttachmentBindingState();
  /** Root execution scope (created on first canonical render). */
  private rootBuilder?: OwnedBuilderRoot;
  private rootScopeCreated = false;
  /** History sideband: attach-once ownership per the Rust audit (SS32.2.4). */
  private stagedHistory?: HistoryContract;
  private boundHistory?: HistoryContract;
  /** Handles created by this Tui are closed with the owning host. */
  private readonly ownedHandles = new Set<OwnedHandle>();
  /** Shared liveness token for caller-owned histories attached to this Tui. */
  private readonly historyLifetime = { closed: false };

  private constructor(host: NativeTuiHostContract, width: number, height: number) {
    this.host = host;
    this.width = width;
    this.height = height;
    const owner = new WeakRef(this);
    this.hostRegistration = this.runtimeEnvironment.registerHost(
      host,
      this.runtimeErrors,
      (commit) => owner.deref()?.commitVisibleAfterDrain(commit),
    );
    this.attachmentContext = {
      registry: this.runtimeEnvironment.resources,
      environment: this.runtimeEnvironment.token,
      host: this.hostRegistration.token,
    };
    // Bootstrap the boundary's COLD materializer (Direct decode w/o paint)
    // so prepareColdInstall can fulfill the SS32.2.3 no-paint-during-PREPARE
    // rule. Idempotent; last Tui wins (single active runtime per process is
    // the supported model).
    setRootColdMaterializer((view) => tryNativeMaterialize(view));
    const hostRef = host;
    // autoFlush stays enabled: tracked-state writes drain on the next
    // microtask (retained scheduler semantics). render() additionally drains
    // pending work first so callers see a coherent frame.
    this.retainedRuntime = new RetainedExecutionRuntime({
      // Scene sideband state is WIP alongside the root output. A failed
      // evaluation/preparation must not leave a history staged for a future
      // commit after the committed root stayed unchanged.
      onBatchAbort: () => {
        this.stagedHistory = this.boundHistory;
      },
      createScopeProjection: () => {
        // Component scopes project as native view slots (R6a machinery).
        // The seed is framework plumbing, not a user semantic construction;
        // do not consume a parent scope's semantic slot for it.
        const slot = createViewSlot(
          hostRef as never,
          withoutRetainedComposition(() => View.spacer(0)),
          undefined,
          this.attachmentContext,
        );
        // This projection is framework plumbing, not a semantic operation in
        // the parent scope. User-facing control.view() calls compose through
        // the retained semantic slot; the internal projection must not.
        const view = componentViewForHandle(slot.id);
        retainSemanticAttachmentReference(semanticNodeOf(view), slot);
        return {
          view,
          target: {
            preparePublication(output: View) {
              return slot.prepareSetView(output);
            },
          },
          dispose(): void {
            slot.dispose();
          },
        };
      },
    });
    registerRuntimeAccess(this, {
      flush: () => this.flush(),
      enqueue: (event) => {
        if (event.type === "key") this.host.dispatchKey(event.key, event.modifiers);
        if (event.type === "paste") this.host.dispatchPaste(event.text);
        if (event.type === "resize") this.resize(event.width, event.height);
      },
      screenRows: () => this.host.screenRows(),
      nativeHistoryRows: () => this.host.nativeHistoryRows(),
      styleAt: (row, column) => {
        const style = this.host.styleAt(row, column) as Readonly<Record<string, unknown>> | null;
        if (style === null) throw tuiError("runtime", "native cell style is unavailable");
        return style;
      },
      cellXOfText: (row, text) => this.host.cellXOfText(row, text),
      advance: (milliseconds) => this.host.advanceTime(milliseconds),
      exited: () => this.host.exited(),
    });
  }

  /**
   * PERF-13-A root publication target: retained prepare first; on refusal a
   * COLD MATERIALIZE-ONLY fallback (never paints during PREPARE). H3 commit
   * installs desired structure; the environment frame barrier performs the
   * visible host update. History sideband is validated here and swapped at
   * desired commit.
   */
  private prepareRootPublication(
    session: ReturnType<typeof nativeViewAbiSession>,
    output: View,
  ): RootPublication {
    const historyToBind = this.stagedHistory;
    const previousHistory = this.boundHistory;
    const attachments = prepareSemanticAttachments(
      semanticNodeOf(output),
      this.runtimeEnvironment.resources,
      this.runtimeEnvironment.token,
      this.hostRegistration.token,
    );
    // Allocate the desired scene record during prepare so H3 commit only
    // promotes already-owned state and cannot fail on this bookkeeping step.
    const nextScene = new Scene(output, historyToBind ?? previousHistory);
    let prepared: RootPublication | undefined;
    try {
      this.ensureBoundary(session);
      prepared = this.boundary!.prepareDesiredInstall(output);
      if (prepared === undefined) {
        // Cold materialize-only: decode without painting before the desired
        // structural publication. This is the canonical capacity fallback.
        prepared = this.boundary!.prepareColdInstall(output);
      }
    } catch (error) {
      attachments.abort();
      throw error;
    }
    if (prepared === undefined) {
      attachments.abort();
      throw new Error("TUI_ROOT_PREPARATION_FAILED: no structural publication was prepared");
    }
    // History sideband validation happens at prepare time via stageHistory;
    // commit swaps the binding before the body publishes. Structural commit
    // installs only desired state; visibility is promoted by the frame drain.
    return {
      rootRef: prepared.rootRef,
      route: prepared.route,
      commit: (): void => {
        try {
          this.commitHistoryBinding(historyToBind, previousHistory);
          prepared!.commit();
          const desiredRevision = this.host.epochs().desired_structural_revision;
          this.attachmentBindings.commitDesired(attachments, desiredRevision);
          this.hostRegistration.markPending();
          this.currentScene = nextScene;
        } catch (error) {
          attachments.abort();
          throw error;
        }
      },
      abort(): void {
        try {
          prepared!.abort();
        } finally {
          attachments.abort();
        }
      },
    };
  }

  private commitHistoryBinding(
    historyToBind: HistoryContract | undefined,
    previousHistory: HistoryContract | undefined,
  ): void {
    // History swap BEFORE body publish mirrors the direct path's ordering
    // (set_history then install/paint). Host-fabricated histories are born
    // attached to their fabricating host; only detached handles transfer here.
    if (historyToBind === undefined || historyToBind === previousHistory) return;
    assertHistoryOwner(historyToBind, this);
    const nativeObj = nativeResourceOf<NativeHistoryContract>(historyToBind);
    if (nativeObj.isDetached() !== false) this.host.setHistory(nativeObj);
    claimHistoryOwner(historyToBind, this);
    bindHistoryLifetime(historyToBind, this.historyLifetime);
    this.boundHistory = historyToBind;
  }

  private stageHistoryBinding(history?: HistoryContract): void {
    if (
      history !== undefined &&
      this.boundHistory !== undefined &&
      this.boundHistory !== history
    ) {
      // Rust ownership model: History transitions detached -> attached ONCE
      // (take_for_host rejects already-attached handles; scene replacement
      // orphans the old value). Different handle after binding is therefore
      // a deterministic API error, not a swappable pointer.
      throw tuiError("terminal", "TUI_HISTORY_ALREADY_BOUND: a different History instance is already attached to this Tui");
    }
    const effectiveHistory = history ?? this.boundHistory;
    if (effectiveHistory !== undefined) {
      assertHistoryOwner(effectiveHistory, this);
      // Validate the handle on every producer pass, including when the
      // sideband identity is unchanged; disposal must not silently turn the
      // retained scene into a stale native reference.
      nativeResourceOf<object>(effectiveHistory);
    }
    this.stagedHistory = effectiveHistory;
  }

  static async open(options: TuiOpenOptions = {}): Promise<Tui> {
    if (options.signal?.aborted) throw tuiError("cancelled", "TUI open was cancelled");
    const width = options.width ?? 80;
    const height = options.height ?? 24;
    validateSize(width, height);
    const Host = requireNativeClass(native.NativeTuiHost, "NativeTuiHost");
    let host: NativeTuiHostContract | undefined;
    let tui: Tui | undefined;
    try {
      host = new Host(width, height, options.headless ?? false);
      tui = new Tui(host, width, height);
      if (options.theme !== undefined) tui.setTheme(options.theme);
      return tui;
    } catch (error) {
      const primary = asTuiError(error);
      try {
        if (tui !== undefined) tui.close();
        else host?.dispose();
      } catch (cleanupError) {
        throw new AggregateError([primary, asTuiError(cleanupError)], "TUI open cleanup failed");
      }
      throw primary;
    }
  }

  /** Current dimensions from the last successfully completed resize. */
  get size(): TerminalMetadata { return { width: this.width, height: this.height }; }

  async nextEvent(signal?: AbortSignal): Promise<TuiEvent> {
    if (signal?.aborted) throw tuiError("cancelled", "TUI event wait was cancelled");
    if (this.closed) return { type: "terminate", reason: "closed" };
    const output = signal === undefined
      ? await this.host.waitForOutput()
      : await this.pollOutput(signal);
    if (signal?.aborted) throw tuiError("cancelled", "TUI event wait was cancelled");
    if (output === null) return { type: "terminate", reason: "closed" };
    return {
      type: "output",
      routeId: output.route_id,
      ...(output.payload === null || output.payload === undefined ? {} : { payload: output.payload }),
    };
  }

  private async pollOutput(signal: AbortSignal): Promise<{ route_id: string; payload?: string | null } | null> {
    while (!this.closed) {
      if (signal.aborted) throw tuiError("cancelled", "TUI event wait was cancelled");
      this.host.pollTerminal();
      const output = this.host.nextOutput();
      if (output !== null) return output;
      await waitForAbortableDelay(Math.min(Math.max(this.host.nextWakeMs(), 1), 16), signal);
    }
    return null;
  }

  /**
   * PERF-12 T13 (§49/§77-B1): production scene router.
   *
   * ```text
   * same body object          → no-op (identity cutoff above the bridge)
   * warm root hint            → §20 exact-root fast path (one host render)
   * otherwise                 → §18 boundary install: ensureNative walks the
   *                             changed semantic frontier, children-first, and
   *                             commits once through the root publication
   *                             target
   * refused / over-budget     → complete cold path (Direct N-API decode), per
   *                             §49: constructors published before the budget
   *                             abort remain valid cache entries the decode
   *                             reuses through its NodeId-first consult
   * ```
   *
   * The pre-T13 recipe cascade (render_ref/scalar/path/structural/edit-tx)
   * is gone: identity hints replace path-lineage recipes, and derivation
   * patches ride inside ensureNative (§27).
   */
  /**
   * PERF-12 T13.1 R8 canonical recurring form (handoff §32.2.3): the Tui
   * owns the root execution scope — closure IDENTITY is irrelevant. Each
   * call replaces the producer and re-drives the root through the same R7
   * prepare/commit protocol as every other boundary participant. State reads
   * inside `builder` subscribe the root scope automatically, so tracked
   * state changes re-render WITHOUT calling render again.
   */
  render(sceneOrBuilder: SceneProducer, signal?: AbortSignal): void {
    if (typeof sceneOrBuilder === "function") {
      this.renderCanonical(sceneOrBuilder, signal);
      return;
    }
    this.renderDirect(sceneOrBuilder, signal);
  }

  /** Drains pending retained-execution work so callers see a coherent frame. */
  private drainExecution(): void {
    if (!this.closed) this.retainedRuntime.flush();
  }

  /**
   * Explicit read-your-writes barrier. H3 publication is accepted first;
   * this second step asks the environment broker to commit the latest visible
   * frame and surfaces any stored runtime failure synchronously.
   */
  flush(): void {
    this.ensureOpen();
    this.retainedRuntime.flush();
    this.hostRegistration.flush();
  }

  onRuntimeError(listener: RuntimeErrorReporter): () => void {
    this.ensureOpen();
    if (typeof listener !== "function") throw new TypeError("runtime error listener must be a function");
    this.runtimeErrorListener = listener;
    this.runtimeErrors.setReporter(listener);
    return () => {
      if (this.runtimeErrorListener !== listener) return;
      this.runtimeErrorListener = undefined;
      this.runtimeErrors.setReporter(undefined);
    };
  }

  private ensureOpen(): void {
    if (this.closed) throw tuiError("terminal", "TUI runtime is closed");
  }

  private prepareMutation(operation: string): void {
    this.ensureOpen();
    this.drainExecution();
    this.assertNotMutating(operation);
  }

  private assertNotMutating(operation: string): void {
    if ((protocolState.mutating && !protocolState.internalPublication) || activeExecutionScope() !== undefined) {
      throw tuiError("terminal", `${operation} during a retained protocol pass is forbidden`);
    }
  }

  private renderCanonical(builder: () => SceneContract, signal?: AbortSignal): void {
    ensureSignal(signal);
    if (this.closed) throw tuiError("terminal", "TUI runtime is closed");
    this.drainExecution();
    this.assertNotMutating("tui.render(builder)");

    const producer = (): View => {
      const scene = Scene.from(builder());
      this.stageHistoryBinding(scene.history);
      return scene.body;
    };

    if (!this.rootScopeCreated) {
      const rootTarget = {
        preparePublication: (output: View): RootPublication | undefined =>
          this.prepareRootPublication(nativeViewAbiSession(), output),
        needsPublication: (_output: View): boolean => this.stagedHistory !== this.boundHistory,
      };
      // Do not mark the boundary live until the initial body/materialization
      // succeeds. A failed first render must leave the Tui retryable rather
      // than making the next render dereference an absent root builder.
      const previousStagedHistory = this.stagedHistory;
      let root: OwnedBuilderRoot;
      try {
        root = OwnedBuilderRoot.start(this.retainedRuntime, producer, rootTarget);
      } catch (error) {
        this.stagedHistory = previousStagedHistory;
        throw error;
      }
      this.rootBuilder = root;
      this.rootScopeCreated = true;
      // H3 commit has already accepted the desired root. A frame failure is a
      // later visibility error and must not roll the desired sideband back.
      this.flush();
      return;
    }
    const previousStagedHistory = this.stagedHistory;
    try {
      this.rootBuilder!.replaceProducer(producer);
    } catch (error) {
      this.stagedHistory = previousStagedHistory;
      throw error;
    }
    // As above, only producer/evaluation failure restores staged sideband;
    // frame failure leaves the newly accepted desired revision retryable.
    this.flush();
  }

  private renderDirect(scene: SceneContract, signal?: AbortSignal): void {
    ensureSignal(signal);
    if (this.closed) throw tuiError("terminal", "TUI runtime is closed");
    this.drainExecution();
    this.assertNotMutating("tui.render(scene)");
    const normalized = Scene.from(scene);
    // Validate the semantic body before transferring an attach-once History;
    // malformed runtime input must not partially mutate the scene sideband.
    semanticNodeOf(normalized.body);
    const previousHistory = this.boundHistory;
    if (normalized.history !== undefined) {
      assertHistoryOwner(normalized.history, this);
      if (previousHistory !== undefined && previousHistory !== normalized.history) {
        throw tuiError("terminal", "TUI_HISTORY_ALREADY_BOUND: a different History instance is already attached to this Tui");
      }
      nativeResourceOf<NativeHistoryContract>(normalized.history);
    }
    const effectiveHistory = normalized.history ?? previousHistory;
    if (effectiveHistory !== undefined) nativeResourceOf<NativeHistoryContract>(effectiveHistory);
    if (
      this.currentScene !== undefined
      && this.currentScene.body === normalized.body
      && this.currentScene.history === effectiveHistory
    ) {
      validateSemanticAttachments(
        semanticNodeOf(normalized.body),
        this.runtimeEnvironment.resources,
        this.runtimeEnvironment.token,
        this.hostRegistration.token,
      );
      recordNativeViewRoute("no_op");
      this.stagedHistory = effectiveHistory;
      this.disposeRootBuilder();
      this.flush();
      return;
    }

    const session = nativeViewAbiSession();
    const previousStagedHistory = this.stagedHistory;
    this.stagedHistory = effectiveHistory;
    let publication: RootPublication;
    try {
      publication = this.prepareRootPublication(session, normalized.body);
      publication.commit();
    } catch (error) {
      this.stagedHistory = previousStagedHistory;
      throw error;
    }
    recordNativeViewRoute(publication.route ?? "retained");
    this.stagedHistory = effectiveHistory;
    this.disposeRootBuilder();
    this.flush();
  }

  private ensureBoundary(session: ReturnType<typeof nativeViewAbiSession>): RetainedRootBoundary {
    if (this.boundary === undefined) {
      this.boundary = new RetainedRootBoundary(
        session,
        () => this.host,
        undefined,
        { deferHostCommit: true },
      );
    }
    return this.boundary;
  }

  /** Creates a Tui-owned History already attached to this host. */
  createHistory(): HistoryContract {
    this.prepareMutation("tui.createHistory");
    try {
      const history = createHistoryHandle(this.host.history() as never);
      claimHistoryOwner(history, this);
      bindHistoryLifetime(history, this.historyLifetime);
      return this.ownHandle(history);
    } catch (error) {
      throw asTuiError(error);
    }
  }

  /** Creates a Tui-owned retained presentation state record. */
  viewState(): ViewStateContract {
    this.prepareMutation("tui.viewState");
    const resource = this.host.viewState();
    const registration = this.hostRegistration;
    try {
      return this.ownHandle(createViewState(
        resource,
        {
          environment: this.runtimeEnvironment.token,
          host: registration.token,
        },
        () => registration.markPending(),
        assertViewStateMutationAllowed,
      ));
    } catch (error) {
      try {
        resource.dispose();
      } catch (cleanupError) {
        throw new AggregateError([asTuiError(error), asTuiError(cleanupError)], "ViewState creation cleanup failed");
      }
      throw asTuiError(error);
    }
  }

  /** Creates a Tui-owned, host-bound ContentPort. */
  contentPort(
    options: ContentPortOptions | typeof TextContent = {},
  ): ContentPortContract {
    this.prepareMutation("tui.contentPort");
    const family = options === TextContent
      ? "text"
      : options !== null && typeof options === "object"
        ? options.family ?? "text"
        : undefined;
    if (options !== TextContent && options !== null && typeof options === "object") {
      for (const key of Object.keys(options)) {
        if (key !== "family") throw tuiError("validation", `unknown ContentPort option ${JSON.stringify(key)}`);
      }
    }
    if (family !== "text") throw tuiError("validation", "unsupported ContentPort family");
    try {
      return this.ownHandle(createContentPortHandle(
        createContentPortResource(this.host, family),
        {
          environment: this.runtimeEnvironment.token,
          host: this.hostRegistration.token,
        },
        () => this.hostRegistration.markPending(),
        () => this.assertNotMutating("content control"),
      ));
    } catch (error) {
      throw asTuiError(error);
    }
  }

  /** Creates a Tui-owned, host-bound TextInput with all options applied. */
  createTextInput(options: TextInputOptions = {}): TextInputContract {
    this.prepareMutation("tui.createTextInput");
    const border = options.border === undefined ? undefined : borderNodeFor(options.border);
    try {
      return this.ownHandle(createTextInput(this.host.textInput(options.multiline, border) as never));
    } catch (error) {
      throw asTuiError(error);
    }
  }

  /** Creates a Tui-owned slot using the shared retained execution runtime. */
  createViewSlot(initialView: View): ViewSlotContract {
    this.prepareMutation("tui.createViewSlot");
    try {
      return this.ownHandle(createViewSlot(
        this.host as never,
        initialView,
        this.retainedRuntime as never,
        this.attachmentContext,
      ));
    } catch (error) {
      throw asTuiError(error);
    }
  }

  /** Creates a Tui-owned pane using the shared retained execution runtime. */
  createScrollPane(initialView: View): ScrollPaneContract {
    this.prepareMutation("tui.createScrollPane");
    try {
      return this.ownHandle(createScrollPane(
        this.host as never,
        initialView,
        this.retainedRuntime as never,
        this.attachmentContext,
      ));
    } catch (error) {
      throw asTuiError(error);
    }
  }

  bindKey(key: string, routeId: string, modifiers?: readonly string[]): void {
    this.prepareMutation("tui.bindKey");
    try {
      this.host.bindKey(key, modifiers, routeId);
    } catch (error) {
      throw asTuiError(error);
    }
  }

  route(output: Output<string>, routeId: string): void {
    this.prepareMutation("tui.route");
    const input = textInputForOutput(output);
    if (input === undefined || input.disposed || !this.ownedHandles.has(input)) {
      throw tuiError("invalid-handle", "output is not owned by this Tui");
    }
    try {
      this.host.route(nativeResourceOf<NativeTuiOutputContract>(output), routeId);
    } catch (error) {
      throw asTuiError(error);
    }
  }

  interceptPaste(input: TextInputContract, routeId: string): void {
    this.prepareMutation("tui.interceptPaste");
    if (!this.ownedHandles.has(input)) {
      throw tuiError("invalid-handle", "text input is not owned by this Tui");
    }
    try {
      this.host.interceptPaste(nativeResourceOf<object>(input), routeId);
    } catch (error) {
      throw asTuiError(error);
    }
  }

  forwardPaste(text: string): void {
    this.prepareMutation("tui.forwardPaste");
    try {
      this.host.forwardPaste(text);
    } catch (error) {
      throw asTuiError(error);
    }
  }

  /** Resize the host, publishing the new size only after it succeeds. */
  resize(width: number, height: number): void {
    this.ensureOpen();
    validateSize(width, height);
    this.prepareMutation("tui.resize");
    try {
      this.host.resize(width, height);
      this.width = width;
      this.height = height;
    } catch (error) {
      throw asTuiError(error);
    }
  }

  private disposeRootBuilder(): void {
    const root = this.rootBuilder;
    this.rootBuilder = undefined;
    this.rootScopeCreated = false;
    root?.dispose();
  }

  private ownHandle<T extends OwnedHandle>(handle: T): T {
    this.ownedHandles.add(handle);
    return handle;
  }

  private disposeOwnedHandles(): void {
    let pending = [...this.ownedHandles];
    this.ownedHandles.clear();
    let finalErrors: unknown[] = [];
    // Controls can own attachment leases for ViewState values. Retry handles
    // that were temporarily in use after dependent controls have had a chance
    // to release their leases, while preserving every final error.
    for (let pass = 0; pending.length > 0 && pass <= pending.length; pass += 1) {
      const failed: OwnedHandle[] = [];
      const passErrors: unknown[] = [];
      for (const handle of pending) {
        try {
          handle.dispose();
        } catch (error) {
          failed.push(handle);
          passErrors.push(error);
        }
      }
      if (failed.length === pending.length) {
        finalErrors = passErrors;
        break;
      }
      pending = failed;
    }
    if (finalErrors.length === 1) throw finalErrors[0];
    if (finalErrors.length > 1) throw new AggregateError(finalErrors, "TUI owned-handle cleanup failed");
  }

  private commitVisibleAfterDrain(commit?: NativeHostCommit): void {
    const revision = commit?.visible_structural_revision;
    this.boundary?.commitVisible(revision);
    this.attachmentBindings.commitVisible(revision);
    for (const handle of this.ownedHandles) {
      const owned = handle as OwnedHandle & {
        syncNativeLifecycle?: () => void;
        syncNativeLifecycles?: () => void;
      };
      owned.syncNativeLifecycle?.();
      owned.syncNativeLifecycles?.();
    }
  }

  private disposeRetainedExecution(): void {
    // Scope projections, attachment bindings, and builder roots own native
    // leases/handles. Retire them before the host so queued environment work
    // cannot outlive the Tui and target disposed resources.
    const errors: unknown[] = [];
    const attempt = (cleanup: () => void): void => {
      try {
        cleanup();
      } catch (error) {
        errors.push(error);
      }
    };
    attempt(() => this.hostRegistration.dispose());
    // Owner death is the only content-plane cascade. It releases host-owned
    // Port/Connector native records before individual wrapper disposal runs.
    attempt(() => this.host.disposeContentResources());
    this.runtimeErrorListener = undefined;
    this.runtimeErrors.setReporter(undefined);
    attempt(() => this.attachmentBindings.dispose());
    attempt(() => this.host.clearViewStateBindings());
    attempt(() => this.runtimeEnvironment.resources.invalidateHost(this.hostRegistration.token));
    attempt(() => this.disposeOwnedHandles());
    attempt(() => this.disposeRootBuilder());
    attempt(() => this.retainedRuntime.dispose());
    const boundary = this.boundary;
    this.boundary = undefined;
    attempt(() => boundary?.close());
    this.stagedHistory = undefined;
    this.boundHistory = undefined;
    if (errors.length === 1) throw errors[0];
    if (errors.length > 1) throw new AggregateError(errors, "TUI retained-runtime cleanup failed");
  }

  /**
   * Closes the host and disposes every handle created through this Tui's
   * factories. Detached History and environment-owned Sources remain caller
   * owned; host-bound values must not be used after this call.
   */
  close(): void {
    if (this.closed) return;
    this.assertNotMutating("tui.close");
    this.closed = true;
    this.historyLifetime.closed = true;
    const errors: unknown[] = [];
    try {
      this.disposeRetainedExecution();
    } catch (error) {
      errors.push(error);
    }
    this.currentScene = undefined;
    try {
      this.host.dispose();
    } catch (error) {
      errors.push(asTuiError(error));
    }
    if (errors.length === 1) throw errors[0];
    if (errors.length > 1) throw new AggregateError(errors, "TUI close cleanup failed");
  }

  /** Same ownership/lifecycle semantics as close(), using terminal exit. */
  exit(): void {
    if (this.closed) return;
    this.assertNotMutating("tui.exit");
    this.historyLifetime.closed = true;
    const errors: unknown[] = [];
    let exitFailed = false;
    try {
      // The final exit frame must run while its desired ContentPort bindings
      // still exist. Retained-resource teardown follows the native final
      // frame, unlike close(), which never prepares another frame.
      this.host.exit();
    } catch (error) {
      exitFailed = true;
      errors.push(asTuiError(error));
    }
    try {
      this.disposeRetainedExecution();
    } catch (error) {
      errors.push(error);
    }
    if (exitFailed) {
      try {
        this.host.dispose();
      } catch (cleanupError) {
        errors.push(asTuiError(cleanupError));
      }
    }
    // Exit is terminal even when terminal restoration reports an error;
    // retained roots and caller-owned History liveness must not remain usable
    // after the cleanup path has run.
    this.closed = true;
    this.currentScene = undefined;
    if (errors.length === 1) throw errors[0];
    if (errors.length > 1) throw new AggregateError(errors, "TUI exit cleanup failed");
  }

  setTheme(theme: Theme): void {
    this.prepareMutation("tui.setTheme");
    const definition = themeDefinitionFor(theme);
    const lowered = materializeTheme(definition);
    try {
      this.host.setTheme(lowered);
    } catch (error) {
      throw asTuiError(error);
    } finally {
      // PERF-12 T13 theme-epoch rule: drop cached themed StyleRefs so later
      // retained materializations re-resolve against the new host theme. The
      // native host may have applied the theme before a presentation failure,
      // so this cleanup also belongs on the exceptional path.
      resetStyleRefCacheForThemeChange();
    }
  }

}

function waitForAbortableDelay(milliseconds: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      signal.removeEventListener("abort", onAbort);
      resolve();
    }, milliseconds);
    const onAbort = () => {
      clearTimeout(timer);
      reject(tuiError("cancelled", "TUI event wait was cancelled"));
    };
    signal.addEventListener("abort", onAbort, { once: true });
  });
}

function assertViewStateMutationAllowed(): void {
  if ((protocolState.mutating && !protocolState.internalPublication) || activeExecutionScope() !== undefined) {
    throw tuiError("terminal", "ViewState mutation during a retained protocol pass is forbidden");
  }
}

function ensureSignal(signal: AbortSignal | undefined): void {
  if (signal?.aborted) throw tuiError("cancelled", "TUI render was cancelled");
}

function validateSize(width: number, height: number): void {
  if (!Number.isInteger(width) || !Number.isInteger(height)
    || width <= 0 || height <= 0 || width > 65535 || height > 65535) {
    throw asTuiError(new RangeError("terminal size must be an integer from 1 to 65535"));
  }
}
