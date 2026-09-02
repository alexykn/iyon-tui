import { native } from "../src/transport/native/addon.ts";
import { viewReleaseMany } from "../src/transport/abi/structural/generated/view_calls.ts";
import { lowerColdView } from "../src/transport/structural/cold-lowering.ts";
import { nativeViewAbiSession } from "../src/transport/structural/native-view-abi.ts";
import { View } from "../src/api/view/view.ts";

const reference = native.tuiViewAbiDecodeRef(lowerColdView(View.text("worker")));
const session = nativeViewAbiSession();
viewReleaseMany(session.symbols, session.runtime, new Uint32Array([reference]), 1);
postMessage("decoded");
