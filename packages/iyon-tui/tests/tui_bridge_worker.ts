import { native } from "../src/transport/native/addon.ts";
import { lowerColdView } from "../src/transport/structural/cold-lowering.ts";
import { View } from "../src/api/view/view.ts";

const Host = native.NativeTuiHost;
if (Host === undefined) throw new Error("native TUI host is unavailable");
const host = new Host(12, 2, true);
host.render(lowerColdView(View.text("worker")));
host.dispose();
postMessage("decoded");
