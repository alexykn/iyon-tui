import { View } from "@iyon/tui";
import type { ToolCall, ToolResult } from "@iyon/sdk";
export function markerCall(_call: ToolCall): View { return View.text("replacement call") as unknown as View; }
export function markerResult(_result: ToolResult): View { return View.text("replacement result") as unknown as View; }
