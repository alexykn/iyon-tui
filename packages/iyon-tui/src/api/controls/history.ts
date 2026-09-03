import { tuiError } from "../errors.ts";
import type { View } from "../view/view.ts";
import { nativeViewAbiSession, releaseNativeViewRef, tryRetainedMaterializeRef } from "../../transport/structural/native-view-abi.ts";
import { nativeResourceOf } from "../../transport/native/resources.ts";
import { FrameworkHandle } from "./framework-handle.ts";
import { nativeTui } from "../../transport/native/factories.ts";
import type { NativeHistoryContract } from "../../transport/native/addon.ts";

export interface History extends FrameworkHandle<"history"> {
  readonly kind: "history";
  layout(): HistoryLayout;
  push(view: View): number;
  freeze(unit: number, view: View): void;
  discardLive(unit: number): void;
  setLayout(layout: HistoryLayout): void;
}

type HistoryContract = History;

export interface HistoryLayout {
  readonly padding: number;
  readonly gap: number;
}

const historyLifetimes = new WeakMap<object, { readonly closed: boolean }>();
const HISTORY_NATIVE_TOKEN = Symbol("history-native-construction");

/**
 * Ordered scrollback handle with an explicit detached mode.
 *
 * Lifecycle:
 * - `new History()` creates caller-owned detached storage that can be used for
 *   layout and pushes without a Tui.
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
   * Unit import is identity-first through the single retained path. Retained
   * hints reuse any subtree already materialized through a framework
   * boundary. The temporary lease drains after the push because History
   * retains its own strong state; a retained refusal fails explicitly.
   */
  push(view: View): number {
    return History.callHost(this, () => {
      const ref = tryRetainedMaterializeRef(view);
      if (ref === undefined) throw new Error("HISTORY_PUSH_FAILED: View could not be materialized");
      try {
        return this.nativeAs<NativeHistoryContract>().pushRef(ref);
      } finally {
        releaseNativeViewRef(nativeViewAbiSession(), ref);
      }
    });
  }

  freeze(unit: number, view: View): void {
    History.callHost(this, () => {
      validateHistoryUnit(unit);
      // T13 (§78): same retained rule as push — freezing a live card
      // reuses its already-materialized nodes through their hints.
      const ref = tryRetainedMaterializeRef(view);
      if (ref === undefined) throw new Error("HISTORY_FREEZE_FAILED: View could not be materialized");
      try {
        this.nativeAs<NativeHistoryContract>().freezeRef(unit, ref);
      } finally {
        releaseNativeViewRef(nativeViewAbiSession(), ref);
      }
    });
  }

  discardLive(unit: number): void {
    History.callHost(this, () => {
      validateHistoryUnit(unit);
      this.nativeAs<NativeHistoryContract>().discardLive(unit);
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
export function createHistoryHandle(resource: never): HistoryContract {
  const Constructor = History as unknown as new (resource: never, token: typeof HISTORY_NATIVE_TOKEN) => HistoryContract;
  return new Constructor(resource, HISTORY_NATIVE_TOKEN);
}

/** @internal Marks a caller-owned History as unavailable after its host closes. */
export function bindHistoryLifetime(history: HistoryContract, lifetime: { readonly closed: boolean }): void {
  historyLifetimes.set(history, lifetime);
}

function validateHistoryUnit(unit: number): void {
  if (!Number.isSafeInteger(unit) || unit < 1) {
    throw new RangeError("history unit must be a positive safe integer");
  }
}
