import { View } from "@iyon/tui";
import type { View as ContributionView } from "@iyon/tui";
import type { ToolCall, ToolResult } from "@iyon/sdk";
import { renderDiff, resultStyle, resultText, statusLabel, toolCallLine, toolCallPreview, resultBlock, toolText } from "@iyon/plugins";
import type { EditArgs } from "./execute.ts";

export function renderEditCall(call: ToolCall<EditArgs>): ContributionView {
  const body = toolCallLine(call.arguments === undefined ? `edit — ${statusLabel(call.state)}` : `edit ${call.arguments.path} — ${statusLabel(call.state)}`, call.state, call.pulse);
  const preview = toolCallPreview(call);
  return (preview ? View.vertical([body, preview]).fillWidth() : body) as unknown as ContributionView;
}

export function renderEditResult(result: ToolResult): ContributionView {
  if (result.isError) return View.spacer(0) as unknown as ContributionView;
  const summary = toolText(resultText(result), resultStyle(false)).fillWidth();
  const diff = renderDiff(result.details);
  const body = diff ? View.vertical([summary, diff]).fillWidth() : summary;
  return resultBlock(body) as unknown as ContributionView;
}
