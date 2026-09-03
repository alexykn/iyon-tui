import type { View } from "../../src/api/view/view.ts";
import type { NativeTuiHostContract } from "../../src/transport/native/addon.ts";
import {
  hostRenderRef,
  viewReleaseMany,
} from "../../src/transport/abi/structural/generated/view_calls.ts";
import { tryRetainedMaterializeRef } from "../../src/transport/structural/native-view-abi.ts";
import { nativeViewAbiSession } from "../../src/transport/structural/native-view-abi.ts";

/**
 * Test-only full-publication helper. It renders through the single
 * authoritative retained path (materialize + host render), giving
 * differential tests a same-architecture reference for incremental edit
 * operations (path patches, transactions, axis builders).
 */
export function renderRetained(host: NativeTuiHostContract, view: View): void {
  const session = nativeViewAbiSession();
  const reference = tryRetainedMaterializeRef(view);
  if (reference === undefined) {
    throw new Error("retained test materialization refused the view");
  }
  try {
    if (hostRenderRef(session.symbols, session.runtime, host, reference) !== 0) {
      throw new Error("native host-ref render failed");
    }
  } finally {
    viewReleaseMany(session.symbols, session.runtime, new Uint32Array([reference]), 1);
  }
}
