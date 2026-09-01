import { asTuiError, tuiError } from "../api/errors.ts";
import { Tui } from "../runtime/runtime.ts";
import { runtimeAccess } from "../runtime/access.ts";
import type { Output } from "../api/controls/output.ts";
import type { History as HistoryContract } from "../api/controls/history.ts";
import type { ScrollPane as ScrollPaneContract } from "../api/controls/scroll-pane.ts";
import type { TextInput as TextInputContract, TextInputOptions } from "../api/controls/text-input.ts";
import type { ViewSlot as ViewSlotContract } from "../api/controls/view-slot.ts";
import type { ViewState as ViewStateContract } from "../api/view/retained-state.ts";
import type { ContentPort as ContentPortContract, ContentPortOptions } from "../api/content/retained.ts";
import { TextContent } from "../api/content/text-content.ts";
import type { SceneProducer } from "../api/view/scene.ts";
import type { View } from "../api/view/view.ts";
import type { TuiEvent } from "../runtime/events.ts";
import type { TuiRuntime, TerminalMetadata, TuiOpenOptions } from "../runtime/runtime.ts";
import type { Theme } from "../api/presentation/theme.ts";

interface AppHarnessContract extends TuiRuntime {
  createHistory(): HistoryContract;
  viewState(): ViewStateContract;
  contentPort(options?: ContentPortOptions | typeof TextContent): ContentPortContract;
  createTextInput(options?: TextInputOptions): TextInputContract;
  createViewSlot(initial: View): ViewSlotContract;
  createScrollPane(initial: View): ScrollPaneContract;
  pressKey(key: string, modifiers?: readonly string[]): void;
  paste(text: string): void;
  advance(ms: number): void;
  screenRows(): readonly string[];
  nativeHistoryRows(): readonly string[];
  styleAt(row: number, column: number): Readonly<Record<string, unknown>>;
  cellXOfText(row: number, text: string): number | null;
  exited(): boolean;
  now(): number;
}

export class AppHarness implements AppHarnessContract {
  private readonly tui: Tui;
  private clock = 0;

  private constructor(tui: Tui) {
    this.tui = tui;
  }

  static async open(options: TuiOpenOptions = {}): Promise<AppHarness> {
    const tui = await Tui.open({ ...options, headless: true });
    return new AppHarness(tui);
  }

  get size(): TerminalMetadata { return this.tui.size; }
  nextEvent(signal?: AbortSignal): Promise<TuiEvent> { return this.tui.nextEvent(signal); }

  render(scene: SceneProducer, signal?: AbortSignal): void {
    this.tui.render(scene, signal);
    this.callTesting(() => runtimeAccess(this.tui).advance(0));
  }

  flush(): void { this.tui.flush(); }
  onRuntimeError(listener: Parameters<TuiRuntime["onRuntimeError"]>[0]): () => void {
    return this.tui.onRuntimeError(listener);
  }

  createHistory(): HistoryContract { return this.tui.createHistory(); }
  viewState(): ViewStateContract { return this.tui.viewState(); }
  contentPort(options: ContentPortOptions | typeof TextContent = {}): ContentPortContract {
    return this.tui.contentPort(options);
  }
  createTextInput(options: TextInputOptions = {}): TextInputContract { return this.tui.createTextInput(options); }
  createViewSlot(initial: View): ViewSlotContract { return this.tui.createViewSlot(initial); }
  createScrollPane(initial: View): ScrollPaneContract { return this.tui.createScrollPane(initial); }
  bindKey(key: string, actionId: string, modifiers?: readonly string[]): void { this.tui.bindKey(key, actionId, modifiers); }
  route(output: Output<string>, actionId: string): void { this.tui.route(output, actionId); }
  interceptPaste(input: TextInputContract, actionId: string): void { this.tui.interceptPaste(input, actionId); }
  forwardPaste(text: string): void { this.tui.forwardPaste(text); }
  setTheme(theme: Theme): void { this.tui.setTheme(theme); }

  resize(width: number, height: number): void { this.tui.resize(width, height); }

  close(): void {
    this.tui.close();
  }

  exit(): void {
    this.tui.exit();
  }

  pressKey(key: string, modifiers?: readonly string[]): void {
    this.callTesting(() => {
      const access = runtimeAccess(this.tui);
      access.flush();
      access.enqueue({ type: "key", key, modifiers });
    });
  }
  paste(text: string): void {
    this.callTesting(() => {
      const access = runtimeAccess(this.tui);
      access.flush();
      access.enqueue({ type: "paste", text });
    });
  }
  advance(ms: number): void {
    if (!Number.isSafeInteger(ms) || ms < 0 || ms > Number.MAX_SAFE_INTEGER - this.clock) {
      throw tuiError("validation", "clock advancement must keep the deterministic clock within safe integer range");
    }
    // Keep the public deterministic clock transactional: a failed native
    // advancement must not make now() report time that was never applied.
    this.callTesting(() => {
      const access = runtimeAccess(this.tui);
      access.flush();
      access.advance(ms);
    });
    this.clock += ms;
  }
  screenRows(): readonly string[] {
    return this.inspect((access) => access.screenRows());
  }
  nativeHistoryRows(): readonly string[] {
    return this.inspect((access) => access.nativeHistoryRows());
  }
  styleAt(row: number, column: number): Readonly<Record<string, unknown>> {
    return this.inspect((access) => access.styleAt(row, column));
  }
  cellXOfText(row: number, text: string): number | null {
    return this.inspect((access) => access.cellXOfText(row, text));
  }
  exited(): boolean { return this.callTesting(() => runtimeAccess(this.tui).exited()); }
  now(): number { return this.clock; }

  private inspect<R>(operation: (access: ReturnType<typeof runtimeAccess>) => R): R {
    return this.callTesting(() => {
      const access = runtimeAccess(this.tui);
      // Flush retained/native zero-time work before a deterministic snapshot.
      // This keeps headless inspection coherent without exposing testing-only
      // clock control to application code.
      access.flush();
      access.advance(0);
      return operation(access);
    });
  }

  private callTesting<R>(operation: () => R): R {
    try {
      return operation();
    } catch (error) {
      throw asTuiError(error);
    }
  }
}

export const createAppHarness = AppHarness.open;
