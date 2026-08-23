import { nodeForBridge, type View } from "./values/view.ts";
import { nativeViewAbiSession, releaseNativeViewRef, tryNativeMaterialize, tryRetainedMaterializeRef } from "./native_view_abi.ts";
import { HandleBase, nativeTui } from "./handles.ts";
import type { History as HistoryContract, HistoryLayout, TextStream } from "./types.ts";

export class History extends HandleBase<ReturnType<typeof nativeTui.history>, "history"> implements HistoryContract {
  constructor(nativeHandle = nativeTui.history()) { super("history", nativeHandle); }

  layout(): HistoryLayout {
    return this.call(() => this.nativeHandle.layout() as HistoryLayout);
  }

  /**
   * PERF-12 T13 (§78): unit import is identity-first. Retained hints reuse
   * any subtree already materialized through any boundary (tool cards, chrome,
   * earlier units) with zero payload re-reads; a refused retained path falls
   * back to the cold graph and finally the N-API bridge. The temporary lease
   * drains after the push because History retains its own strong state.
   */
  push(view: View): number {
    return this.call(() => {
      const ref = tryRetainedMaterializeRef(view) ?? tryNativeMaterialize(view);
      if (ref !== undefined) {
        try {
          if (this.nativeHandle.pushRef !== undefined) return this.nativeHandle.pushRef(ref);
        } finally {
          releaseNativeViewRef(nativeViewAbiSession(), ref);
        }
      }
      return this.nativeHandle.push(nodeForBridge(view));
    });
  }

  freeze(unit: number, view: View): void {
    this.call(() => {
      // T13 (§78): same retained-first rule as push — freezing a live card
      // reuses its already-materialized nodes through their hints.
      const ref = tryRetainedMaterializeRef(view) ?? tryNativeMaterialize(view);
      if (ref !== undefined) {
        try {
          if (this.nativeHandle.freezeRef !== undefined) {
            this.nativeHandle.freezeRef(unit, ref);
            return;
          }
        } finally {
          releaseNativeViewRef(nativeViewAbiSession(), ref);
        }
      }
      this.nativeHandle.freeze(unit, nodeForBridge(view));
    });
  }

  discardLive(unit: number): void {
    this.call(() => this.nativeHandle.discardLive(unit));
  }

  pushStream(stream: TextStream): void {
    this.call(() => this.nativeHandle.pushStream((stream as unknown as { nativeObject(): object }).nativeObject()));
  }

  sealStream(stream: TextStream): void {
    this.call(() => this.nativeHandle.sealStream((stream as unknown as { nativeObject(): object }).nativeObject()));
  }

  setLayout(layout: HistoryLayout): void {
    this.call(() => this.nativeHandle.setLayout(layout));
  }

  /** Internal bridge access; not exported from the public module. */
  nativeObject(): object { this.ensureOpen(); return this.nativeHandle; }
}
