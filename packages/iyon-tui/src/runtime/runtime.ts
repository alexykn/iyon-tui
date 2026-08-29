import { native, requireNativeClass } from "../transport/native/addon.ts";
import { nativeResourceOf } from "../transport/native/resources.ts";
import { lowerColdView } from "../transport/structural/cold-lowering.ts";
import { borderNodeFor, materializeTheme } from "../transport/structural/style-lowering.ts";
import { componentViewForHandle, View } from "../api/view/view.ts";
import { asTuiError, tuiError } from "../api/errors.ts";
import { registerRuntimeAccess } from "./access.ts";
import { Scene } from "../api/view/scene.ts";
import { bindHistoryLifetime, createHistoryHandle } from "../api/controls/history.ts";
import { createTextInput, textInputForOutput } from "../api/controls/text-input.ts";
import { createViewSlot } from "../api/controls/view-slot.ts";
import { createScrollPane } from "../api/controls/scroll-pane.ts";
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
  resize(width: number, height: number): void;
  close(): void;
  exit(): void;
  createHistory(): HistoryContract;
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
   * PERF-12 T13 (§18/§49): the scene body's root-lease boundary. It owns
   * exactly one lease on the currently installed root; previous roots stay
   * leased until a replacement is fully materialized and committed.
   */
  private boundary?: RetainedRootBoundary;

  /**
   * PERF-12 T13.1 R8: ONE retained execution runtime per Tui, created eagerly.
   * The root scene scope and every slot/pane builder root participate in this
   * single dirty queue / batch / R7 transaction protocol.
   */
  private readonly retainedRuntime: RetainedExecutionRuntime;
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
    // Bootstrap the boundary's COLD materializer (Direct decode w/o paint)
    // so prepareColdInstall can fulfill the SS32.2.3 no-paint-during-PREPARE
    // rule. Idempotent; last Tui wins (single active runtime per process is
    // the supported model).
    setRootColdMaterializer((view) => tryNativeMaterialize(view));
    const hostRef = host;
    // autoFlush stays ENABLED: tracked-state writes drain on the next
    // microtask (R5 scheduler semantics). render() additionally drains
    // pending work first so legacy/direct callers see a coherent frame.
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
        const slot = createViewSlot(hostRef as never, withoutRetainedComposition(() => View.spacer(0)), undefined);
        // This projection is framework plumbing, not a semantic operation in
        // the parent scope. User-facing control.view() calls compose through
        // the retained semantic slot; the internal projection must not.
        const view = componentViewForHandle(slot.id);
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
      flush: () => this.drainExecution(),
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
   * PERF-12 T13.1 R8 root publication target: retained prepare first; on
   * refusal a COLD MATERIALIZE-ONLY fallback (never paints during PREPARE —
   * the commit publishes via one hostRenderRef call). History sideband is
   * validated here and swapped at commit.
   */
  private prepareRootPublication(
    session: NonNullable<ReturnType<typeof nativeViewAbiSession>> | undefined,
    output: View,
  ): RootPublication | undefined {
    const historyToBind = this.stagedHistory;
    const previousHistory = this.boundHistory;
    if (session === undefined) {
      // The generated ABI is present in every supported artifact, but keep
      // the contract truthful if an older addon is loaded: builder roots can
      // still publish through the ordinary host renderer without pretending
      // that a retained ref exists.
      return {
        rootRef: 0,
        commit: (): void => {
          this.commitHistoryBinding(historyToBind, previousHistory);
          this.host.render(lowerColdView(output));
          this.currentScene = new Scene(output, historyToBind ?? this.boundHistory);
        },
        abort(): void {},
      };
    }
    this.ensureBoundary(session);
    let prepared: RootPublication | undefined = this.boundary!.prepareInstall(output);
    if (prepared === undefined) {
      // Cold materialize-only: decode WITHOUT painting (\u00a732.2.3 hard rule).
      prepared = this.boundary!.prepareColdInstall(output);
    }
    // Both retained and cold routes can refuse (for example when the native
    // session has been torn down). Report a normal preparation refusal so the
    // enclosing transaction can abort without dereferencing an absent ref.
    if (prepared === undefined) return undefined;
    // History sideband validation happens at prepare time via stageHistory;
    // commit swaps the binding before the body publishes.
    return {
      rootRef: prepared!.rootRef,
      commit: (): void => {
        this.commitHistoryBinding(historyToBind, previousHistory);
        prepared!.commit();
        this.currentScene = new Scene(output, historyToBind ?? this.boundHistory);
      },
      abort(): void {
        prepared!.abort();
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
    if (history !== undefined) {
      assertHistoryOwner(history, this);
      // Validate the handle on every producer pass, including when the
      // sideband identity is unchanged; disposal must not silently turn the
      // retained scene into a stale native reference.
      nativeResourceOf<object>(history);
    }
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
    this.stagedHistory = history ?? this.boundHistory;
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
   *                             commits once via hostRenderRef
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
      try {
        const root = OwnedBuilderRoot.start(this.retainedRuntime, producer, rootTarget);
        this.rootBuilder = root;
        this.rootScopeCreated = true;
        return;
      } catch (error) {
        this.stagedHistory = previousStagedHistory;
        throw error;
      }
    }
    const previousStagedHistory = this.stagedHistory;
    try {
      this.rootBuilder!.replaceProducer(producer);
    } catch (error) {
      this.stagedHistory = previousStagedHistory;
      throw error;
    }
  }

  private renderDirect(scene: SceneContract, signal?: AbortSignal): void {
    ensureSignal(signal);
    if (this.closed) throw tuiError("terminal", "TUI runtime is closed");
    this.drainExecution();
    this.assertNotMutating("tui.render(scene)");
    const normalized = Scene.from(scene);
    // Validate the structural body before transferring an attach-once History;
    // malformed runtime input must not partially mutate the scene sideband.
    const normalizedNode = lowerColdView(normalized.body);
    const previousHistory = this.boundHistory;
    if (normalized.history !== undefined) {
      // Validate the handle before the identity no-op below. A disposed
      // History must never be accepted merely because the same object was
      // present in the last scene.
      assertHistoryOwner(normalized.history, this);
      if (previousHistory !== undefined && previousHistory !== normalized.history) {
        throw tuiError("terminal", "TUI_HISTORY_ALREADY_BOUND: a different History instance is already attached to this Tui");
      }
      nativeResourceOf<NativeHistoryContract>(normalized.history);
    }
    // An omitted history means "keep the existing sideband" for both direct
    // and retained scene ownership modes. Compare and record that effective
    // value rather than allowing currentScene to forget a bound History.
    const effectiveHistory = normalized.history ?? previousHistory;
    if (
      this.currentScene !== undefined
      && this.currentScene.body === normalized.body
      && this.currentScene.history === effectiveHistory
    ) {
      recordNativeViewRoute("no_op");
      // Direct takeover is SEMANTIC even when no pixel can change: the direct
      // scene now owns this boundary, so the canonical builder root must be
      // relinquished NOW. Leaving it subscribed lets its State subscriptions
      // ghost-update the screen later — exactly the R8 ownership-mode ghost.
      // Projected components freeze rather than vanish: their JS scopes die
      // here while native retirement stays deferred until a successful frame
      // proves them unmounted (post-R9 invariant §32.3).
      this.stagedHistory = effectiveHistory;
      this.disposeRootBuilder();
      return;
    }
    const previousBody = this.currentScene?.body;
    const session = nativeViewAbiSession();
    const historyChanges = normalized.history !== undefined && normalized.history !== previousHistory;
    if (historyChanges && session !== undefined) {
      // Prepare the body before transferring a detached History. The native
      // history transition is attach-once, so a malformed body must not make
      // an otherwise reusable History permanently bound to this Tui.
      this.ensureBoundary(session);
      const retainedPublication = this.boundary!.prepareInstall(normalized.body);
      const publication = retainedPublication ?? this.boundary!.prepareColdInstall(normalized.body);
      if (publication !== undefined) {
        try {
          this.commitHistoryBinding(normalized.history, previousHistory);
          publication.commit();
        } catch (error) {
          // If history preparation failed before commit, release the root
          // lease acquired for the body and leave the old scene authoritative.
          publication.abort();
          throw error;
        }
        recordNativeViewRoute(retainedPublication === undefined ? "fallback" : "retained");
        this.currentScene = new Scene(normalized.body, effectiveHistory);
        this.stagedHistory = effectiveHistory;
        this.disposeRootBuilder();
        return;
      }
    }
    // If the retained/cold preflight was unavailable, keep the legacy direct
    // fallback behavior. The supported addon always has a preflight path for
    // valid public Views; this branch is only for an older/incomplete addon.
    if (historyChanges) this.commitHistoryBinding(normalized.history, previousHistory);
    // Exact-root fast path: identical body View, warm hint, one host call.
    if (
      session !== undefined
      && previousBody !== undefined
      && normalized.body === previousBody
    ) {
      this.ensureBoundary(session);
      const exact = this.boundary!.renderExact(normalized.body);
      if (exact.status === "ok") {
        recordNativeViewRoute("render_ref");
        this.currentScene = new Scene(normalized.body, effectiveHistory);
        this.stagedHistory = effectiveHistory;
        this.disposeRootBuilder();
        return;
      }
      // Miss falls through to the full retained install (§47 recovery).
    }
    let installed = false;
    if (session !== undefined) {
      this.ensureBoundary(session);
      const nextRef = this.boundary!.install(normalized.body);
      if (nextRef !== undefined) {
        recordNativeViewRoute("retained");
        installed = true;
      }
      // A refused install keeps the old root rendered (§45); the caller-level
      // fallback below re-renders authoritatively.
    }
    if (!installed) {
      // Complete cold candidate (§49): Direct N-API decode of the whole tree.
      // Nodes published by an aborted retained prefix stay valid cache entries
      // that this decode consults NodeId-first, so wasted prefix work is not
      // repeated — it shortens the fallback.
      recordNativeViewRoute("fallback");
      this.host.render(normalizedNode);
      // Adopt so future renders hit the exact-root fast path.
      if (session !== undefined) this.boundary!.adopt(normalized.body);
    }
    this.currentScene = new Scene(normalized.body, effectiveHistory);
    this.stagedHistory = effectiveHistory;
    this.disposeRootBuilder();
  }

  private ensureBoundary(session: NonNullable<ReturnType<typeof nativeViewAbiSession>>): RetainedRootBoundary {
    if (this.boundary === undefined) {
      this.boundary = new RetainedRootBoundary(session, () => this.host);
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
      return this.ownHandle(createViewSlot(this.host as never, initialView, this.retainedRuntime as never));
    } catch (error) {
      throw asTuiError(error);
    }
  }

  /** Creates a Tui-owned pane using the shared retained execution runtime. */
  createScrollPane(initialView: View): ScrollPaneContract {
    this.prepareMutation("tui.createScrollPane");
    try {
      return this.ownHandle(createScrollPane(this.host as never, initialView, this.retainedRuntime as never));
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
    const handles = [...this.ownedHandles];
    this.ownedHandles.clear();
    for (const handle of handles) handle.dispose();
  }

  private disposeRetainedExecution(): void {
    // Scope projections and builder roots own native leases/handles. They must
    // be retired while the host is still alive; otherwise State subscribers
    // and scheduled microtasks can outlive the Tui and target disposed slots.
    // Factory-created controls are disposed first while their native host and
    // the shared retained runtime are still available.
    this.disposeOwnedHandles();
    this.disposeRootBuilder();
    this.retainedRuntime.dispose();
    this.stagedHistory = undefined;
    this.boundHistory = undefined;
  }

  /**
   * Closes the host and disposes every handle created through this Tui's
   * factories. Detached History and direct TextStream values remain caller
   * owned; host-bound values must not be used after this call.
   */
  close(): void {
    if (this.closed) return;
    this.assertNotMutating("tui.close");
    this.closed = true;
    this.historyLifetime.closed = true;
    try {
      this.disposeRetainedExecution();
      this.boundary?.close();
    } finally {
      this.boundary = undefined;
      this.currentScene = undefined;
      this.host.dispose();
    }
  }

  /** Same ownership/lifecycle semantics as close(), using terminal exit. */
  exit(): void {
    if (this.closed) return;
    this.assertNotMutating("tui.exit");
    this.historyLifetime.closed = true;
    try {
      this.disposeRetainedExecution();
      this.boundary?.close();
    } finally {
      this.boundary = undefined;
      try {
        this.host.exit();
      } catch (error) {
        throw asTuiError(error);
      } finally {
        // Exit is terminal even when terminal restoration reports an error;
        // retained roots and caller-owned History liveness must not remain
        // usable after the cleanup path has run.
        this.closed = true;
      }
    }
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

function ensureSignal(signal: AbortSignal | undefined): void {
  if (signal?.aborted) throw tuiError("cancelled", "TUI render was cancelled");
}

function validateSize(width: number, height: number): void {
  if (!Number.isInteger(width) || !Number.isInteger(height)
    || width <= 0 || height <= 0 || width > 65535 || height > 65535) {
    throw asTuiError(new RangeError("terminal size must be an integer from 1 to 65535"));
  }
}
