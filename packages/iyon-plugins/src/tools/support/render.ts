import { parseUnifiedDiff } from "./diff.ts";
import type { DiffHunk as ParsedDiffHunk } from "./diff.ts";
import { DiffHunk, DiffLine, DiffRange, DiffRenderer, Style, View } from "@iyon/tui";
import type { ToolCall, ToolLifecycleState, ToolResult } from "@iyon/sdk";

export const MAX_COLLAPSED_TOOL_ROWS = 16;

export function collapseResultView(view: View): View {
  return view.clampRows(MAX_COLLAPSED_TOOL_ROWS, {
    kind: "footer",
    prefix: "… more lines (full result retained)",
    style: Style.new().foreground("theme:truncation_footer").italic().dim(),
  });
}

export function toolStyle(state: ToolLifecycleState) {
  const key = state === "preparing" || state === "running" ? "tool.running" : state === "pendingApproval" ? "text.warning" : state === "failed" || state === "cancelled" ? "tool.error" : "tool.finished";
  return Style.new().foreground(`theme:${key}`);
}

export function resultStyle(isError: boolean) {
  return Style.new().foreground(`theme:${isError ? "tool.error" : "text.muted"}`);
}

export function toolText(value: string, style: ReturnType<typeof Style.new>): View {
  return View.text(value).style(style);
}

export function toolCallLine(value: string, state: ToolLifecycleState, pulse = false): View {
  const style = toolStyle(state);
  const bullet = pulse ? style.dim() : style;
  return View.hanging(toolText("● ", bullet).noWrap(), View.text("  ").noWrap(), toolText(value, style).fillWidth()).fillWidth();
}

export function toolResultLine(value: string, style: ReturnType<typeof Style.new>): View {
  return View.hanging(View.text("  ").noWrap(), View.text("  ").noWrap(), toolText(value, style).fillWidth()).fillWidth();
}

export function resultLines(value: string, style: ReturnType<typeof Style.new>): View[] {
  return value.split(/\r?\n/u).map((line) => toolResultLine(line, style));
}

export function resultBlock(body: View): View {
  return View.hanging(View.text("  ").noWrap(), View.text("  ").noWrap(), body).fillWidth();
}

export function resultText(result: ToolResult): string {
  return result.text ?? result.content.filter((block) => block.type === "text").map((block) => block.text).join("");
}

export function statusLabel(state: ToolLifecycleState): string {
  if (state === "prepared") return "ready";
  if (state === "pendingApproval") return "waiting for approval";
  return state;
}

export function toolCallPreview<T>(call: ToolCall<T>): View | undefined {
  if (!call.showArgPreview || call.state === "failed" || call.state === "cancelled") return undefined;
  const value = call.arguments === undefined ? undefined : JSON.stringify(call.arguments, null, 2);
  if (value === undefined) return undefined;
  return View.vertical(value.split("\n").map((line) => toolResultLine(line, toolStyle(call.state)))).fillWidth();
}

export function renderDiff(details: unknown): View | undefined {
  const diff = typeof details === "object" && details !== null && typeof (details as { diff?: unknown }).diff === "string" ? (details as { diff: string }).diff : undefined;
  if (!diff) return undefined;
  try { return renderHunks(parseUnifiedDiff(diff)); } catch { return View.vertical(diff.split("\n").map((line) => toolText(line, Style.new().theme("diff.meta")).fillWidth())).fillWidth(); }
}

export function renderHunks(hunks: readonly ParsedDiffHunk[]): View {
  return new DiffRenderer().render(hunks.map(toSemanticHunk));
}

function toSemanticHunk(hunk: ParsedDiffHunk): DiffHunk {
  return new DiffHunk(
    toSemanticRange(hunk.oldStart, hunk.oldCount),
    toSemanticRange(hunk.newStart, hunk.newCount),
    hunk.lines.map((line) => new DiffLine(line.kind, line.text, line.termination ?? "lf")),
  );
}

function toSemanticRange(start: number, count: number): DiffRange {
  const offset = count === 0 ? start : start - 1;
  return new DiffRange(offset, count);
}
