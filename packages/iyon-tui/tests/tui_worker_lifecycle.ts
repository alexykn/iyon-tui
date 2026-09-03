import { nativeViewAbiSession } from "../src/transport/structural/native-view-abi.ts";
import { runtimeNoop } from "../src/transport/abi/structural/generated/view_calls.ts";

// Touches the environment-local native session through the current N-API
// surface so worker teardown can be observed releasing its environment.
const session = nativeViewAbiSession();
runtimeNoop(session.symbols, session.runtime);
postMessage("ready");
