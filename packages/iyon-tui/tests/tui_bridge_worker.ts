import { native } from "../src/native.ts";
import { nodeForBridge, View } from "../src/values/view.ts";

const Host = native.NativeTuiHost;
if (Host === undefined) throw new Error("native TUI host is unavailable");
const host = new Host(12, 2, true);
host.render(nodeForBridge(View.text("worker")));
host.dispose();
postMessage("decoded");
