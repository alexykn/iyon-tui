import { tuiError } from "./errors.ts";
import { Tui } from "./runtime.ts";
import type { TextInput as RuntimeTextInput } from "./text-input.ts";
import type {
  AppHarness as AppHarnessContract,
  History,
  OutputHandle,
  TextInput,
  ScrollPane,
  TuiEvent,
  TuiOpenOptions,
} from "./types.ts";

export class AppHarness implements AppHarnessContract {
  private readonly tui: Tui;
  private readonly options: { width: number; height: number };
  private clock = 0;

  private constructor(tui: Tui, options: { width: number; height: number }) {
    this.tui = tui;
    this.options = options;
  }

  static async open(options: TuiOpenOptions = {}): Promise<AppHarness> {
    const size = { width: options.width ?? 80, height: options.height ?? 24 };
    const tui = await Tui.open({ ...options, ...size, headless: true });
    return new AppHarness(tui, size);
  }

  get size(): { width: number; height: number } { return this.options; }
  nextEvent(signal?: AbortSignal): Promise<TuiEvent> { return this.tui.nextEvent(signal); }
  nextAction(signal?: AbortSignal): Promise<{ actionId: string; payload?: string } | null> { return this.tui.nextAction(signal); }

  render(scene: import("./types.ts").SceneProducer, signal?: AbortSignal): void {
    this.tui.render(scene, signal);
    this.tui.advance(0);
  }

  createHistory(): History { return this.tui.createHistory(); }
  createTextInput(options: { multiline?: boolean; border?: import("./ir.ts").BorderNode } = {}): TextInput { return this.tui.createTextInput(options); }
  createViewSlot(initial: import("./values/view.ts").View) {
    if (this.tui.createViewSlot === undefined) throw tuiError("runtime", "native view slots are unavailable");
    return this.tui.createViewSlot(initial);
  }
  createScrollPane(initial: import("./values/view.ts").View): ScrollPane {
    if (this.tui.createScrollPane === undefined) throw tuiError("runtime", "native scroll panes are unavailable");
    return this.tui.createScrollPane(initial);
  }
  bindKey(key: string, actionId: string, modifiers?: readonly string[]): void { this.tui.bindKey(key, actionId, modifiers); }
  route(output: OutputHandle<string>, actionId: string): void { this.tui.route(output, actionId); }
  interceptPaste(input: TextInput, actionId: string): void { this.tui.interceptPaste(input as RuntimeTextInput, actionId); }
  forwardPaste(text: string): void { this.tui.forwardPaste(text); }
  setTheme(theme: import("./values/theme.ts").Theme): void { this.tui.setTheme(theme); }

  resize(width: number, height: number): void {
    this.options.width = width;
    this.options.height = height;
    this.tui.resize(width, height);
  }

  close(): void {
    this.tui.close();
  }

  exit(): void {
    this.tui.exit();
  }

  pressKey(key: string, modifiers?: readonly string[]): void { this.tui.enqueue({ type: "key", key, modifiers }); }
  paste(text: string): void { this.tui.enqueue({ type: "paste", text }); }
  advance(ms: number): void {
    if (!Number.isFinite(ms) || ms < 0) throw tuiError("validation", "clock advancement must be non-negative");
    this.clock += ms;
    this.tui.advance(ms);
  }
  screenRows(): readonly string[] { return this.tui.screenRows(); }
  nativeHistoryRows(): readonly string[] { return this.tui.nativeHistoryRows(); }
  styleAt(row: number, column: number): Readonly<Record<string, unknown>> { return this.tui.styleAt(row, column); }
  cellXOfText(row: number, text: string): number | null {
    return this.tui.cellXOfText(row, text);
  }
  exited(): boolean { return this.tui.exited(); }
  now(): number { return this.clock; }
}

export const createAppHarness = AppHarness.open;
