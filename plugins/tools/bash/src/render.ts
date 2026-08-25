import { View } from "@iyon/tui";
import type { ToolCall, ToolResult } from "@iyon/sdk";
import { resultLines, resultStyle, resultText, statusLabel, toolCallLine, toolResultLine } from "@iyon/plugins";

export function renderBashCall(call: ToolCall<{ command: string }>): View {
  const label = call.arguments === undefined
    ? `${call.name} — ${statusLabel(call.state)}`
    : `$ ${call.arguments.command} — ${statusLabel(call.state)}`;
  return toolCallLine(label, call.state, call.pulse) as unknown as View;
}

export function renderBashResult(result: ToolResult): View {
  const style = resultStyle(result.isError);
  const children = [toolResultLine(result.isError ? "bash failed" : "bash result", style), ...resultLines(resultText(result), style)];
  const fullOutputPath = typeof result.details === "object" && result.details !== null && typeof (result.details as { fullOutputPath?: unknown }).fullOutputPath === "string" ? (result.details as { fullOutputPath: string }).fullOutputPath : undefined;
  if (fullOutputPath) children.push(toolResultLine(`[Full output: ${fullOutputPath}]`, style.theme("text.warning")));
  return View.vertical(children).fillWidth() as unknown as View;
}
