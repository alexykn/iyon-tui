import { native } from "../native.ts";
import { nodeForBridge, View } from "./values/view.ts";
import { asTuiError, tuiError } from "./errors.ts";
import { requireNativeClass } from "./handles.ts";
import { Scene } from "./scene.ts";
import { History } from "./history.ts";
import { TextInput } from "./text-input.ts";
import { ViewSlot } from "./component.ts";
import { NativeScrollPane } from "./scroll-pane.ts";
import {
  nativeViewAbiSession,
  recordNativeViewRoute,
  tryNativeMaterialize,
} from "./native_view_abi.ts";
import {
  resetStyleRefCacheForThemeChange,
  RetainedRootBoundary,
  setRootColdMaterializer,
} from "./retained_dag.ts";
import type { RootPublication } from "./retained_dag.ts";
import { OwnedBuilderRoot, RetainedExecutionRuntime } from "./execution.ts";
import { protocolState } from "./execution-context.ts";
import type {
  OutputHandle,
  ScrollPane,
  Scene as SceneContract,
  History as HistoryContract,
  TerminalMetadata,
  TuiEvent,
  TuiOpenOptions,
  TuiRuntime,
} from "./types.ts";
import type { NativeTuiHostContract } from "../native.ts";

export class Tui implements TuiRuntime {
  private closed = false;
  private readonly host: NativeTuiHostContract;
  private readonly width: number;
  private readonly height: number;
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
  private rootProducerStaged?: () => SceneContract;
  private rootScopeCreated = false;
  /** History sideband: attach-once ownership per the Rust audit (SS32.2.4). */
  private stagedHistory?: HistoryContract;
  private boundHistory?: HistoryContract;

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
      createScopeProjection: () => {
        // Component scopes project as native view slots (R6a machinery).
        const slot = new ViewSlot(hostRef, View.spacer(0));
        const view = slot.view();
        return {
          view,
          install(output: View): void {
            slot.setView(output);
          },
          preparePublication(output: View) {
            return slot.prepareSetView(output);
          },
          dispose(): void {
            slot.dispose();
          },
        };
      },
    });
  }

  /**
   * PERF-12 T13.1 R8 root publication target: retained prepare first; on
   * refusal a COLD MATERIALIZE-ONLY fallback (never paints during PREPARE —
   * the commit publishes via one hostRenderRef call). History sideband is
   * validated here and swapped at commit.
   */
  private prepareRootPublication(
    session: NonNullable<ReturnType<typeof nativeViewAbiSession>>,
    output: View,
  ): RootPublication | undefined {
    this.ensureBoundary(session);
    let prepared: RootPublication | undefined = this.boundary!.prepareInstall(output);
    if (prepared === undefined) {
      // Cold materialize-only: decode WITHOUT painting (\u00a732.2.3 hard rule).
      prepared = this.boundary!.prepareColdInstall(output);
    }
    // History sideband validation happens at prepare time via stageHistory;
    // commit swaps the binding before the body publishes.
    const historyToBind = this.stagedHistory;
    const previousHistory = this.boundHistory;
    return {
      rootRef: prepared!.rootRef,
      commit: (): void => {
        // History swap BEFORE body publish mirrors the direct path's ordering
        // (set_history then install/paint). Host-fabricated histories are
        // born attached to their fabricating host (take_for_host would
        // reject re-attach); only detached handles transfer here.
        if (historyToBind !== undefined && historyToBind !== previousHistory) {
          const nativeObj = (historyToBind as unknown as { nativeObject(): object }).nativeObject() as { isDetached?: () => boolean };
          if (nativeObj.isDetached?.() !== false) {
            this.host.setHistory((historyToBind as unknown as { nativeObject(): object }).nativeObject() as never);
          }
          this.boundHistory = historyToBind;
        }
        prepared!.commit();
        this.currentScene = new Scene(output, historyToBind ?? this.boundHistory);
      },
      abort(): void {
        prepared!.abort();
      },
    };
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
    this.stagedHistory = history ?? this.boundHistory;
  }

  static async open(options: TuiOpenOptions = {}): Promise<Tui> {
    if (options.signal?.aborted) throw tuiError("cancelled", "TUI open was cancelled");
    const width = options.width ?? 80;
    const height = options.height ?? 24;
    validateSize(width, height);
    const Host = requireNativeClass(native.NativeTuiHost, "NativeTuiHost");
    try {
      const tui = new Tui(new Host(width, height, options.headless ?? false), width, height);
      if (options.theme !== undefined) await tui.setTheme(options.theme);
      return tui;
    } catch (error) {
      throw asTuiError(error);
    }
  }

  get size(): TerminalMetadata { return { width: this.width, height: this.height }; }

  /** PERF-12 T13.1 R8: the ONE retained execution runtime owned by this Tui. */
  get executionRuntime(): RetainedExecutionRuntime { return this.retainedRuntime; }

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

  /** Compatibility adapter for the pre-generic application harness. */
  async nextAction(signal?: AbortSignal): Promise<{ actionId: string; payload?: string } | null> {
    const event = await this.nextEvent(signal);
    if (event.type === "terminate") return null;
    if (event.type !== "output") return this.nextAction(signal);
    return { actionId: event.routeId, ...(event.payload === undefined ? {} : { payload: event.payload }) };
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
  render(sceneOrBuilder: SceneContract | (() => SceneContract), signal?: AbortSignal): void {
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

  private assertNotMutating(operation: string): void {
    if (protocolState.mutating || this.closed === false) {
      // Only reject when a retained protocol pass is actually running.
      if (protocolState.mutating) {
        throw tuiError("terminal", `${operation} during a retained protocol pass is forbidden`);
      }
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
      this.rootScopeCreated = true;
      const rootTarget = {
        preparePublication: (output: View): RootPublication | undefined =>
          this.prepareRootPublication(nativeViewAbiSession()!, output),
      };
      this.rootBuilder = OwnedBuilderRoot.start(this.retainedRuntime, producer, rootTarget);
      return;
    }
    this.rootBuilder!.replaceProducer(producer);
  }

  private renderDirect(scene: SceneContract, signal?: AbortSignal): void {
    ensureSignal(signal);
    if (this.closed) throw tuiError("terminal", "TUI runtime is closed");
    this.drainExecution();
    const normalized = Scene.from(scene);
    if (
      this.currentScene !== undefined
      && this.currentScene.body === normalized.body
      && this.currentScene.history === normalized.history
    ) {
      recordNativeViewRoute("no_op");
      return;
    }
    if (normalized.history !== undefined) {
      const history = (normalized.history as unknown as { nativeObject(): object }).nativeObject() as { isDetached?: () => boolean };
      if (history.isDetached?.() === true) this.host.setHistory(history as object);
    }
    const previousBody = this.currentScene?.body;
    const session = nativeViewAbiSession();
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
        this.currentScene = normalized;
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
      this.host.render(nodeForBridge(normalized.body));
      // Adopt so future renders hit the exact-root fast path.
      if (session !== undefined) this.boundary!.adopt(normalized.body);
    }
    this.currentScene = normalized;
  }

  private ensureBoundary(session: NonNullable<ReturnType<typeof nativeViewAbiSession>>): RetainedRootBoundary {
    if (this.boundary === undefined) {
      this.boundary = new RetainedRootBoundary(session, () =>
        this.host.tuiViewAbiHostPointer?.() as never,
      );
    }
    return this.boundary;
  }

  createHistory(): History { return new History(this.host.history() as never); }

  createTextInput(options: { multiline?: boolean; border?: import("./ir.ts").BorderNode } = {}): TextInput {
    return new TextInput(options, this.host.textInput(options.multiline, options.border) as never);
  }

  createViewSlot(initialView: import("./values/view.ts").View): ViewSlot {
    return new ViewSlot(this.host, initialView, this.retainedRuntime);
  }

  createScrollPane(initialView: import("./values/view.ts").View): ScrollPane {
    return new NativeScrollPane(this.host, initialView, this.retainedRuntime);
  }

  bindKey(key: string, routeId: string, modifiers?: readonly string[]): void {
    this.host.bindKey(key, modifiers, routeId);
  }

  route(output: OutputHandle<string>, routeId: string): void {
    this.host.route((output as unknown as { nativeObject: object }).nativeObject as never, routeId);
  }

  interceptPaste(input: TextInput, routeId: string): void {
    this.host.interceptPaste((input as unknown as { nativeHandle: object }).nativeHandle, routeId);
  }

  forwardPaste(text: string): void { this.host.forwardPaste(text); }

  resize(width: number, height: number): void {
    if (this.closed) throw tuiError("terminal", "TUI runtime is closed");
    validateSize(width, height);
    this.host.resize(width, height);
  }

  close(): void {
    if (this.closed) return;
    this.closed = true;
    try {
      this.boundary?.close();
    } finally {
      this.boundary = undefined;
      this.currentScene = undefined;
      this.host.dispose();
    }
  }

  exit(): void {
    if (this.closed) return;
    try {
      this.boundary?.close();
    } finally {
      this.boundary = undefined;
      this.host.exit();
      this.closed = true;
    }
  }

  setTheme(theme: import("./values/theme.ts").Theme): void {
    if (this.closed) throw tuiError("terminal", "TUI runtime is closed");
    // PERF-12 T13 theme-epoch rule: drop cached themed StyleRefs so later
    // retained materializations re-resolve against the new host theme.
    resetStyleRefCacheForThemeChange();
    this.host.setTheme(theme.materialize());
  }

  enqueue(event: { readonly type: "key"; readonly key: string; readonly modifiers?: readonly string[] } | { readonly type: "paste"; readonly text: string } | { readonly type: "resize"; readonly width: number; readonly height: number }): void {
    if (event.type === "key") this.host.dispatchKey(event.key, event.modifiers);
    if (event.type === "paste") this.host.dispatchPaste(event.text);
    if (event.type === "resize") this.resize(event.width, event.height);
  }

  screenRows(): readonly string[] { return this.host.screenRows(); }
  nativeHistoryRows(): readonly string[] { return this.host.nativeHistoryRows(); }
  styleAt(row: number, column: number): Readonly<Record<string, unknown>> {
    const style = this.host.styleAt(row, column) as Readonly<Record<string, unknown>> | null;
    if (style === null) throw tuiError("runtime", "native cell style is unavailable");
    return style;
  }
  cellXOfText(row: number, text: string): number | null { return this.host.cellXOfText(row, text); }
  exited(): boolean { return this.host.exited(); }
  advance(ms: number): void { this.host.advanceTime(ms); }
  current(): Scene | undefined { return this.currentScene; }
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
  if (!Number.isInteger(width) || !Number.isInteger(height) || width <= 0 || height <= 0) throw asTuiError(new RangeError("terminal size must be positive integers"));
}
