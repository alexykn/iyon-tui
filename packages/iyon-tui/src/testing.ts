import { tuiError } from "./errors.ts";
import { Tui } from "./runtime.ts";
import { tuiTestingAccess } from "./testing-access.ts";
import type { History as HistoryHandle } from "./history.ts";
import type { TextInput as RuntimeTextInput } from "./text-input.ts";
import type { TextInput as TextInputHandle } from "./text-input.ts";
import type {
  AppHarness as AppHarnessContract,
  Output,
  TextInput as TextInputContract,
  TextInputOptions,
  ScrollPane,
  TuiEvent,
  TerminalMetadata,
  TuiOpenOptions,
  View,
  ViewSlot as ViewSlotContract,
} from "./types.ts";
import type { Theme } from "./values/theme.ts";

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

  render(scene: import("./types.ts").SceneProducer, signal?: AbortSignal): void {
    this.tui.render(scene, signal);
    tuiTestingAccess(this.tui).advance(0);
  }

  createHistory(): HistoryHandle { return this.tui.createHistory(); }
  createTextInput(options: TextInputOptions = {}): TextInputHandle { return this.tui.createTextInput(options); }
  createViewSlot(initial: View): ViewSlotContract { return this.tui.createViewSlot(initial); }
  createScrollPane(initial: View): ScrollPane { return this.tui.createScrollPane(initial); }
  bindKey(key: string, actionId: string, modifiers?: readonly string[]): void { this.tui.bindKey(key, actionId, modifiers); }
  route(output: Output<string>, actionId: string): void { this.tui.route(output, actionId); }
  interceptPaste(input: TextInputContract, actionId: string): void { this.tui.interceptPaste(input as RuntimeTextInput, actionId); }
  forwardPaste(text: string): void { this.tui.forwardPaste(text); }
  setTheme(theme: Theme): void { this.tui.setTheme(theme); }

  resize(width: number, height: number): void { this.tui.resize(width, height); }

  close(): void {
    this.tui.close();
  }

  exit(): void {
    this.tui.exit();
  }

  pressKey(key: string, modifiers?: readonly string[]): void { tuiTestingAccess(this.tui).enqueue({ type: "key", key, modifiers }); }
  paste(text: string): void { tuiTestingAccess(this.tui).enqueue({ type: "paste", text }); }
  advance(ms: number): void {
    if (!Number.isFinite(ms) || ms < 0) throw tuiError("validation", "clock advancement must be non-negative");
    this.clock += ms;
    tuiTestingAccess(this.tui).advance(ms);
  }
  screenRows(): readonly string[] { return tuiTestingAccess(this.tui).screenRows(); }
  nativeHistoryRows(): readonly string[] { return tuiTestingAccess(this.tui).nativeHistoryRows(); }
  styleAt(row: number, column: number): Readonly<Record<string, unknown>> { return tuiTestingAccess(this.tui).styleAt(row, column); }
  cellXOfText(row: number, text: string): number | null {
    return tuiTestingAccess(this.tui).cellXOfText(row, text);
  }
  exited(): boolean { return tuiTestingAccess(this.tui).exited(); }
  now(): number { return this.clock; }
}

export const createAppHarness = AppHarness.open;
