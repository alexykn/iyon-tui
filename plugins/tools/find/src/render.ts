import { View } from "@iyon/tui";
import type { ToolCall, ToolResult } from "@iyon/sdk";
import { resultLines, resultStyle, resultText, statusLabel, toolCallLine, toolResultLine } from "@iyon/plugins";

export function renderFindCall(call: ToolCall<{ pattern: string; path?: string }>): View {
  return toolCallLine(call.arguments === undefined ? `find — ${statusLabel(call.state)}` : `find ${call.arguments.pattern} in ${call.arguments.path ?? "."} — ${statusLabel(call.state)}`, call.state, call.pulse) as unknown as View;
}

export function renderFindResult(result: ToolResult): View {
  const style = resultStyle(result.isError);
  return View.vertical([toolResultLine(result.isError ? "find failed" : "find result", style), ...resultLines(resultText(result), style)]).fillWidth() as unknown as View;
}
