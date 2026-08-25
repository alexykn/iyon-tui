import { View } from "@iyon/tui";
import type { ToolCall, ToolResult } from "@iyon/sdk";
import { renderDiff, resultBlock, resultStyle, resultText, statusLabel, toolCallLine, toolText } from "@iyon/plugins";

export function renderWriteCall(call: ToolCall<{ path: string }>): View {
  return toolCallLine(call.arguments === undefined ? `write — ${statusLabel(call.state)}` : `write ${call.arguments.path} — ${statusLabel(call.state)}`, call.state, call.pulse) as unknown as View;
}

export function renderWriteResult(result: ToolResult): View {
  if (result.isError) return View.spacer(0) as unknown as View;
  const summary = toolText(resultText(result), resultStyle(false)).fillWidth();
  const diff = renderDiff(result.details);
  const body = diff ? View.vertical([summary, diff]).fillWidth() : summary;
  return resultBlock(body) as unknown as View;
}
