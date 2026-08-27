import { nodeForBridge } from "./view-internals.ts";
import { tuiError } from "./errors.ts";
import type { View } from "./values/view.ts";
import { nativeViewAbiSession, releaseNativeViewRef, tryNativeMaterialize, tryRetainedMaterializeRef } from "./native_view_abi.ts";
import { nativeResourceOf } from "./handles.ts";
import { FrameworkHandle } from "./types.ts";
import { nativeTui } from "./native-handles.ts";
import type { NativeHistoryContract } from "./native.ts";
import type { History as HistoryContract, HistoryLayout, TextStream } from "./types.ts";

const streamOwners = new WeakMap<object, WeakRef<HistoryContract>>();
const historyStreams = new WeakMap<object, Set<WeakRef<object>>>();
const streamLifetimes = new WeakMap<object, { readonly closed: boolean }>();
const historyLifetimes = new WeakMap<object, { readonly closed: boolean }>();
const HISTORY_NATIVE_TOKEN = Symbol("history-native-construction");

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
export class History extends FrameworkHandle<"history"> implements HistoryContract {
  constructor();
  constructor(resource?: NativeHistoryContract, token?: typeof HISTORY_NATIVE_TOKEN) {
    let nativeResource: NativeHistoryContract;
    if (resource === undefined) {
      if (token !== undefined) throw new TypeError("invalid History construction token");
      nativeResource = nativeTui.history();
    } else {
      if (token !== HISTORY_NATIVE_TOKEN) throw new TypeError("History native construction is private");
      nativeResource = resource;
    }
    super("history", nativeResource as never);
  }

  layout(): HistoryLayout {
    return History.callHost(this, () => this.nativeAs<NativeHistoryContract>().layout() as HistoryLayout);
  }

  /**
   * PERF-12 T13 (§78): unit import is identity-first. Retained hints reuse
   * any subtree already materialized through any boundary (content blocks, chrome,
   * earlier units) with zero payload re-reads; a refused retained path falls
   * back to the cold graph and finally the N-API bridge. The temporary lease
   * drains after the push because History retains its own strong state.
   */
  push(view: View): number {
    return History.callHost(this, () => {
      const ref = tryRetainedMaterializeRef(view) ?? tryNativeMaterialize(view);
      if (ref !== undefined) {
        try {
          const nativeHandle = this.nativeAs<NativeHistoryContract>();
          return nativeHandle.pushRef(ref);
        } finally {
          releaseNativeViewRef(nativeViewAbiSession(), ref);
        }
      }
      return this.nativeAs<NativeHistoryContract>().push(nodeForBridge(view));
    });
  }

  freeze(unit: number, view: View): void {
    History.callHost(this, () => {
      validateHistoryUnit(unit);
      // T13 (§78): same retained-first rule as push — freezing a live card
      // reuses its already-materialized nodes through their hints.
      const ref = tryRetainedMaterializeRef(view) ?? tryNativeMaterialize(view);
      if (ref !== undefined) {
        try {
          const nativeHandle = this.nativeAs<NativeHistoryContract>();
          nativeHandle.freezeRef(unit, ref);
          return;
        } finally {
          releaseNativeViewRef(nativeViewAbiSession(), ref);
        }
      }
      this.nativeAs<NativeHistoryContract>().freeze(unit, nodeForBridge(view));
    });
  }

  discardLive(unit: number): void {
    History.callHost(this, () => {
      validateHistoryUnit(unit);
      this.nativeAs<NativeHistoryContract>().discardLive(unit);
    });
  }

  pushStream(stream: TextStream): void {
    History.callHost(this, () => {
      const streamResource = nativeResourceOf<object>(stream);
      assertStreamCanAttach(stream);
      this.nativeAs<NativeHistoryContract>().pushStream(streamResource);
      streamOwners.set(stream, new WeakRef(this));
      let streams = historyStreams.get(this);
      if (streams === undefined) {
        streams = new Set();
        historyStreams.set(this, streams);
      }
      streams.add(new WeakRef(stream));
      const lifetime = historyLifetimes.get(this);
      if (lifetime === undefined) streamLifetimes.delete(stream);
      else streamLifetimes.set(stream, lifetime);
    });
  }

  sealStream(stream: TextStream): void {
    History.callHost(this, () => {
      if (streamOwnerOf(stream) !== this) {
        throw tuiError("stream", "TUI_STREAM_NOT_ATTACHED: the TextStream is not attached to this History");
      }
      this.nativeAs<NativeHistoryContract>().sealStream(nativeResourceOf<object>(stream));
    });
  }

  setLayout(layout: HistoryLayout): void {
    History.callHost(this, () => this.nativeAs<NativeHistoryContract>().setLayout(layout));
  }

  private static callHost<R>(history: History, operation: () => R): R {
    return history.call(() => {
      if (historyLifetimes.get(history)?.closed === true) {
        throw tuiError("terminal", "History is bound to a closed Tui runtime");
      }
      return operation();
    });
  }

}

/** @internal Wraps a host-created History without exposing its native constructor. */
export function createHistoryHandle(resource: never): History {
  const Constructor = History as unknown as new (resource: never, token: typeof HISTORY_NATIVE_TOKEN) => History;
  return new Constructor(resource, HISTORY_NATIVE_TOKEN);
}

/** @internal Marks a caller-owned History as unavailable after its host closes. */
export function bindHistoryLifetime(history: HistoryContract, lifetime: { readonly closed: boolean }): void {
  historyLifetimes.set(history, lifetime);
  const streams = historyStreams.get(history);
  if (streams === undefined) return;
  for (const reference of streams) {
    const stream = reference.deref();
    if (stream === undefined) streams.delete(reference);
    else streamLifetimes.set(stream, lifetime);
  }
}

/** @internal Rejects mutations through a stream whose host runtime has closed. */
export function assertTextStreamUsable(stream: object): void {
  if (streamLifetimes.get(stream)?.closed === true) {
    throw tuiError("terminal", "TextStream is attached to a closed Tui runtime");
  }
}

function validateHistoryUnit(unit: number): void {
  if (!Number.isSafeInteger(unit) || unit < 1) {
    throw new RangeError("history unit must be a positive safe integer");
  }
}
