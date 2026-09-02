import type { View } from "../../src/api/view/view.ts";
import type { NativeTuiHostContract } from "../../src/transport/native/addon.ts";
import { native } from "../../src/transport/native/addon.ts";
import {
  hostRenderRef,
  viewReleaseMany,
} from "../../src/transport/abi/structural/generated/view_calls.ts";
import { lowerColdView } from "../../src/transport/structural/cold-lowering.ts";
import { nativeViewAbiSession } from "../../src/transport/structural/native-view-abi.ts";

/** Test-only bridge for exercising the generated host-ref ABI directly. */
export function renderCold(host: NativeTuiHostContract, view: View): void {
  const session = nativeViewAbiSession();
  const reference = native.tuiViewAbiDecodeRef(lowerColdView(view));
  try {
    if (hostRenderRef(session.symbols, session.runtime, host, reference) !== 0) {
      throw new Error("native host-ref render failed");
    }
  } finally {
    viewReleaseMany(session.symbols, session.runtime, new Uint32Array([reference]), 1);
  }
}
