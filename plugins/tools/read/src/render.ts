import { View } from "@iyon/tui";
import type { ToolCall, ToolResult } from "@iyon/sdk";
import { resultLines, resultStyle, resultText, statusLabel, toolCallLine, toolResultLine } from "@iyon/plugins";

export function renderReadCall(call: ToolCall<{ path: string; offset?: number; limit?: number }>): View {
  if (call.arguments === undefined) return toolCallLine(`read — ${statusLabel(call.state)}`, call.state, call.pulse) as unknown as View;
  const { path, offset, limit } = call.arguments;
  const suffix = offset === undefined ? "" : limit === undefined ? `:${offset}` : `:${offset}-${offset + Math.max(0, limit - 1)}`;
  return toolCallLine(`read ${path}${suffix} — ${statusLabel(call.state)}`, call.state, call.pulse) as unknown as View;
}

export function renderReadResult(result: ToolResult): View {
  const style = resultStyle(result.isError);
  return View.vertical([toolResultLine(result.isError ? "read failed" : "read result", style), ...resultLines(resultText(result), style)]).fillWidth() as unknown as View;
}
