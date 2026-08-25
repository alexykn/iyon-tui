import { Style, View } from "@iyon/tui";
import type { ToolCall, ToolResult } from "./contract.ts";

function callStyle(state: ToolCall["state"]) {
  const key = state === "failed" || state === "cancelled" ? "tool.error" : state === "pendingApproval" ? "text.warning" : state === "prepared" || state === "finished" ? "tool.finished" : "tool.running";
  return Style.new().foreground(`theme:${key}`);
}

export function renderGenericCall(call: ToolCall): View {
  const style = callStyle(call.state);
  const label = call.state === "prepared" ? "ready" : call.state === "pendingApproval" ? "waiting for approval" : call.state;
  return View.hanging(View.text("● ").style(call.pulse ? style.dim() : style).noWrap(), View.text("  ").noWrap(), View.text(`${call.name || "tool"} — ${label}`).style(style).fillWidth()).fillWidth() as unknown as View;
}

export function renderGenericResult(result: ToolResult): View {
  const title = result.isError ? "failed" : "result";
  const text = result.text ?? result.content.filter((block) => block.type === "text").map((block) => block.text).join("");
  const style = Style.new().foreground(`theme:${result.isError ? "text.error" : "text.muted"}`);
  const body = text.split(/\r?\n/u).map((line) => View.text(line).style(style).fillWidth());
  return View.vertical([View.text(`${result.toolName ?? "tool"} ${title}`).style(style).fillWidth(), ...body]).fillWidth() as unknown as View;
}

export const genericRenderer = {
  renderCall: renderGenericCall,
  renderResult: renderGenericResult,
};
