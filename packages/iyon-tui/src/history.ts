import { nodeForBridge } from "./view-internals.ts";
import { tuiError } from "./errors.ts";
import type { View } from "./values/view.ts";
import { nativeViewAbiSession, releaseNativeViewRef, tryNativeMaterialize, tryRetainedMaterializeRef } from "./native_view_abi.ts";
import { HandleBase, nativeResourceOf } from "./handles.ts";
import { nativeTui } from "./native-handles.ts";
import type { NativeHistoryContract } from "./native.ts";
import type { History as HistoryContract, HistoryLayout, TextStream } from "./types.ts";

const streamOwners = new WeakMap<object, WeakRef<HistoryContract>>();

function streamOwnerOf(stream: object): HistoryContract | undefined {
  const owner = streamOwners.get(stream)?.deref();
  if (owner === undefined || owner.disposed) {
    streamOwners.delete(stream);
    return undefined;
  }
  return owner;
}

function assertStreamCanAttach(stream: object): void {
  if (streamOwnerOf(stream) !== undefined) {
    throw tuiError("stream", "TUI_STREAM_ALREADY_ATTACHED: a TextStream instance already has a History attachment");
  }
}

/**
 * Ordered scrollback handle with an explicit detached mode.
 *
 * Lifecycle:
 * - `new History()` creates caller-owned detached storage that can be used for
 *   layout, pushes, and stream attachment without a Tui.
 * - `Tui.createHistory()` creates a Tui-owned history already attached to that
 *   Tui; the Tui closes factory-created histories during `close()`/`exit()`.
 *   A detached history transfers to one Tui when rendered and remains
 *   caller-owned.
 * - Callers may dispose any history early. Host-bound histories must not be
 *   used after their Tui closes; detached histories may outlive a Tui.
 * - Attachment is one-way and single-host. A history cannot transfer again or
 *   be rebound to a different Tui after attachment.
 * - `freeze` and `discardLive` require an attached history.
 */
export class History extends HandleBase<"history"> implements HistoryContract {
  constructor();
  constructor(nativeHandle?: NativeHistoryContract) { super("history", (nativeHandle ?? nativeTui.history()) as never); }

  layout(): HistoryLayout {
    return this.call(() => this.nativeAs<NativeHistoryContract>().layout() as HistoryLayout);
  }

  /**
   * PERF-12 T13 (§78): unit import is identity-first. Retained hints reuse
   * any subtree already materialized through any boundary (content blocks, chrome,
   * earlier units) with zero payload re-reads; a refused retained path falls
   * back to the cold graph and finally the N-API bridge. The temporary lease
   * drains after the push because History retains its own strong state.
   */
  push(view: View): number {
    return this.call(() => {
      const ref = tryRetainedMaterializeRef(view) ?? tryNativeMaterialize(view);
      if (ref !== undefined) {
        try {
          const nativeHandle = this.nativeAs<NativeHistoryContract>();
          if (nativeHandle.pushRef !== undefined) return nativeHandle.pushRef(ref);
        } finally {
          releaseNativeViewRef(nativeViewAbiSession(), ref);
        }
      }
      return this.nativeAs<NativeHistoryContract>().push(nodeForBridge(view));
    });
  }

  freeze(unit: number, view: View): void {
    this.call(() => {
      // T13 (§78): same retained-first rule as push — freezing a live card
      // reuses its already-materialized nodes through their hints.
      const ref = tryRetainedMaterializeRef(view) ?? tryNativeMaterialize(view);
      if (ref !== undefined) {
        try {
          const nativeHandle = this.nativeAs<NativeHistoryContract>();
          if (nativeHandle.freezeRef !== undefined) {
            nativeHandle.freezeRef(unit, ref);
            return;
          }
        } finally {
          releaseNativeViewRef(nativeViewAbiSession(), ref);
        }
      }
      this.nativeAs<NativeHistoryContract>().freeze(unit, nodeForBridge(view));
    });
  }

  discardLive(unit: number): void {
    this.call(() => this.nativeAs<NativeHistoryContract>().discardLive(unit));
  }

  pushStream(stream: TextStream): void {
    this.call(() => {
      const streamResource = nativeResourceOf<object>(stream);
      assertStreamCanAttach(stream);
      this.nativeAs<NativeHistoryContract>().pushStream(streamResource);
      streamOwners.set(stream, new WeakRef(this));
    });
  }

  sealStream(stream: TextStream): void {
    this.call(() => {
      if (streamOwnerOf(stream) !== this) {
        throw tuiError("stream", "TUI_STREAM_NOT_ATTACHED: the TextStream is not attached to this History");
      }
      this.nativeAs<NativeHistoryContract>().sealStream(nativeResourceOf<object>(stream));
    });
  }

  setLayout(layout: HistoryLayout): void {
    this.call(() => this.nativeAs<NativeHistoryContract>().setLayout(layout));
  }

}

/** @internal Wraps a host-created History without exposing its native constructor. */
export function createHistoryHandle(nativeHandle: never): History {
  const Constructor = History as unknown as new (nativeHandle: never) => History;
  return new Constructor(nativeHandle);
}
