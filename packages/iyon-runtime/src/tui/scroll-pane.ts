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
import {
  OwnedBuilderRoot,
  type RetainedExecutionRuntime,
} from "./execution.ts";
import { activeExecutionScope, protocolState } from "./execution-context.ts";
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

  /** R8: shared Tui execution runtime (undefined for raw internal construction). */
  private readonly retainedRuntime?: RetainedExecutionRuntime;
  private ownedBuilderRoot?: OwnedBuilderRoot;

  constructor(
    host: NativeTuiHostContract,
    initialView?: View,
    retainedRuntime?: RetainedExecutionRuntime,
  ) {
    super("component", buildPaneHandle(host, initialView) as NativeScrollPaneHandle);
    this.currentView = initialView;
    this.retainedRuntime = retainedRuntime;
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

  /**
   * PERF-12 T13.1 R8: ownership modes mirror ViewSlot.setView — builder
   * form creates/reuses a pane-owned retained execution root; direct form
   * takes ownership after successful install. Scroll state (viewport,
   * follow-end) is NEVER touched by content rebuilds.
   */
  setContent(viewOrBuilder: View | (() => View)): void {
    if (typeof viewOrBuilder === "function") {
      if (protocolState.mutating || activeExecutionScope() !== undefined) {
        throw new Error("TUI_EXECUTION_REENTRANT_MUTATION: pane builder mutation during a retained protocol pass");
      }
      if (this.retainedRuntime === undefined) {
        throw new Error("TUI_EXECUTION_BUILDER_UNSUPPORTED: builder mode requires a Tui-created pane");
      }
      const target = {
        preparePublication: (o: View) => this.prepareSetContent(o),
      };
      if (this.ownedBuilderRoot === undefined) {
        this.ownedBuilderRoot = OwnedBuilderRoot.start(this.retainedRuntime!, viewOrBuilder, target);
      } else {
        this.ownedBuilderRoot.replaceProducer(viewOrBuilder);
      }
      return;
    }
    this.setContentDirect(viewOrBuilder);
  }

  private prepareSetContent(output: View): { commit(): void; abort(): void } | undefined {
    if (this.disposed || this.boundary === undefined) return undefined;
    // Retained preparation may refuse on a bounded/unsupported path; use the
    // complete cold materializer transactionally before giving up.
    const publication = this.boundary.prepareInstall(output) ?? this.boundary.prepareColdInstall(output);
    if (publication === undefined) return undefined;
    const setCurrent = (promoted: View): void => {
      this.currentView = promoted;
    };
    return {
      commit(): void {
        publication.commit();
        setCurrent(output);
      },
      abort(): void {
        publication.abort();
      },
    };
  }

  private setContentDirect(view: View): void {
    if (protocolState.mutating || activeExecutionScope() !== undefined) {
      throw new Error("TUI_EXECUTION_REENTRANT_MUTATION: pane mutation during a retained protocol pass");
    }
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
    // Transactional ownership transition (direct wins after successful
    // publication): dispose any owned builder root LAST.
    const root = this.ownedBuilderRoot;
    if (root !== undefined) {
      this.ownedBuilderRoot = undefined;
      root.dispose();
    }
  }

  followEnd(): void {
    if (protocolState.mutating || activeExecutionScope() !== undefined) {
      throw new Error("TUI_EXECUTION_REENTRANT_MUTATION: pane mutation during a retained protocol pass");
    }
    this.call(() => this.nativeHandle.followEnd());
  }

  /** Releases the boundary's root lease exactly once before native teardown. */
  dispose(): void {
    if (!this.disposed) {
      try {
        // Owned builder root first (SS32.2 lifecycle ordering), then boundary.
        const root = this.ownedBuilderRoot;
        if (root !== undefined) {
          this.ownedBuilderRoot = undefined;
          root.dispose();
        }
        this.boundary?.close();
      } finally {
        this.boundary = undefined;
        this.currentView = undefined;
      }
    }
    super.dispose();
  }

  nativeComponentId(): number | undefined {
    const id = this.nativeHandle.componentId();
    return id === null ? undefined : id;
  }
}
