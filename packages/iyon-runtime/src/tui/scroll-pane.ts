import { HandleBase } from "./handles.ts";
import { native } from "../native.ts";
import type { ComponentCapabilities, ScrollPane as ScrollPaneContract } from "./types.ts";
import {
  nativeViewAbiSession,
  releaseNativeViewRef,
  tryNativeMaterialize,
  tryRetainedMaterializeRef,
} from "./native_view_abi.ts";
import { RetainedRootBoundary } from "./retained_dag.ts";
import { nodeForBridge, View } from "./values/view.ts";
import type { NativeTuiHostContract } from "../native.ts";

type NativeScrollPaneHandle = {
  dispose(): void;
  componentId(): number | null;
  setContent(view: object): void;
  setContentRef(viewRef: number): void;
  followEnd(): void;
};

/** T13: retained-first construction of the pane's initial content. */
function buildPaneHandle(host: NativeTuiHostContract, initialView?: View): object {
  const seed = initialView ?? View.spacer(0);
  const retained = tryRetainedMaterializeRef(seed) ?? tryNativeMaterialize(seed);
  if (retained !== undefined) {
    try {
      return host.scrollPaneRef(retained);
    } finally {
      releaseNativeViewRef(nativeViewAbiSession(), retained);
    }
  }
  return host.scrollPane(nodeForBridge(seed));
}

export class NativeScrollPane extends HandleBase<NativeScrollPaneHandle, "component"> implements ScrollPaneContract {
  private currentView?: View;
  /** T13 (§18/§80): the pane owns one root lease on its current content. */
  private boundary?: RetainedRootBoundary;

  constructor(host: NativeTuiHostContract, initialView?: View) {
    super("component", buildPaneHandle(host, initialView) as NativeScrollPaneHandle);
    this.currentView = initialView;
    const session = nativeViewAbiSession();
    if (session !== undefined && initialView !== undefined) {
      this.boundary = new RetainedRootBoundary(session, () => undefined, (ref) => {
        if (this.disposed) return false;
        try {
          this.nativeHandle.setContentRef(ref);
          return true;
        } catch {
          return false;
        }
      });
      this.boundary.adopt(initialView);
    }
  }

  tuiViewAbiInstallRef(viewRef: number): void { this.nativeHandle.setContentRef(viewRef); }

  view(): View {
    this.ensureOpen();
    return this.nativeComponentId() === undefined ? View.spacer(0) : View.component(this);
  }

  capabilities(): ComponentCapabilities { return this.call(() => ({ focusable: true, keys: ["up", "down", "pageup", "pagedown", "home", "end"] })); }

  setContent(view: View): void {
    this.call(() => {
      // PERF-12 T13 retained path (§80): previous content stays leased until
      // the replacement is fully materialized and committed.
      if (this.boundary !== undefined) {
        if (this.boundary.install(view) !== undefined) {
          this.currentView = view;
          return;
        }
      }
      const ref = tryNativeMaterialize(view);
      if (ref !== undefined) {
        try {
          this.nativeHandle.setContentRef(ref);
          this.currentView = view;
          return;
        } finally {
          releaseNativeViewRef(nativeViewAbiSession(), ref);
        }
      }
      this.nativeHandle.setContent(nodeForBridge(view));
      this.currentView = view;
    });
  }

  followEnd(): void { this.call(() => this.nativeHandle.followEnd()); }

  /** Releases the boundary's root lease exactly once before native teardown. */
  dispose(): void {
    if (!this.disposed) {
      try {
        this.boundary?.close();
      } finally {
        this.boundary = undefined;
      }
    }
    super.dispose();
  }

  nativeComponentId(): number | undefined {
    const id = this.nativeHandle.componentId();
    return id === null ? undefined : id;
  }
}
