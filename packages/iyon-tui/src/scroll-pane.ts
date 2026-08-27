import { FrameworkHandle } from "./types.ts";
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
import { composeComponent } from "./compose.ts";
import { nodeForBridge } from "./view-internals.ts";
import { View } from "./values/view.ts";
import type { NativeTuiHostContract } from "./native.ts";

const SCROLL_PANE_NATIVE_TOKEN = Symbol("scroll-pane-native-construction");

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

/**
 * Tui-owned retained scrolling component.
 *
 * Construct with `Tui.createScrollPane()` only. The owning Tui disposes
 * factory panes during `close()`/`exit()`; callers may dispose them earlier.
 * Their content root and scroll state remain with that handle. Builder content
 * uses the Tui-owned shared retained execution runtime, while direct content
 * and `followEnd()` are explicit ownership-preserving mutations. A pane may
 * be mounted at one location in the retained View graph; duplicate component
 * nodes are rejected and do not create independent panes. The handle must not
 * be used after its Tui closes.
 */
export class NativeScrollPane extends FrameworkHandle<"component"> implements ScrollPaneContract {
  private currentView?: View;
  /** T13 (§18/§80): the pane owns one root lease on its current content. */
  private boundary?: RetainedRootBoundary;

  /** R8: shared Tui execution runtime (undefined for raw internal construction). */
  #retainedRuntime?: RetainedExecutionRuntime;
  private ownedBuilderRoot?: OwnedBuilderRoot;

  private constructor(host: never, initialView?: View, retainedRuntime?: never, token?: typeof SCROLL_PANE_NATIVE_TOKEN) {
    if (token !== SCROLL_PANE_NATIVE_TOKEN) throw new TypeError("ScrollPane native construction is private");
    const nativeHost = host as unknown as NativeTuiHostContract;
    const executionRuntime = retainedRuntime as unknown as RetainedExecutionRuntime | undefined;
    super("component", buildPaneHandle(nativeHost, initialView) as never);
    this.currentView = initialView;
    this.#retainedRuntime = executionRuntime;
    const session = nativeViewAbiSession();
    if (session !== undefined && initialView !== undefined) {
      this.boundary = new RetainedRootBoundary(session, () => undefined, (ref) => {
        if (this.disposed) return false;
        try {
          this.nativeAs<NativeScrollPaneHandle>().setContentRef(ref);
          return true;
        } catch {
          return false;
        }
      });
      this.boundary.adopt(initialView);
    }
  }

  tuiViewAbiInstallRef(viewRef: number): void { this.nativeAs<NativeScrollPaneHandle>().setContentRef(viewRef); }

  view(): View {
    this.ensureOpen();
    return composeComponent(this);
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
      this.ensureOpen();
      if (protocolState.mutating || activeExecutionScope() !== undefined) {
        throw new Error("TUI_EXECUTION_REENTRANT_MUTATION: pane builder mutation during a retained protocol pass");
      }
      if (this.#retainedRuntime === undefined) {
        throw new Error("TUI_EXECUTION_BUILDER_UNSUPPORTED: builder mode requires a Tui-created pane");
      }
      const target = {
        preparePublication: (o: View) => this.prepareSetContent(o),
      };
      if (this.ownedBuilderRoot === undefined) {
        this.ownedBuilderRoot = OwnedBuilderRoot.start(this.#retainedRuntime!, viewOrBuilder, target);
      } else {
        this.ownedBuilderRoot.replaceProducer(viewOrBuilder);
      }
      return;
    }
    this.setContentDirect(viewOrBuilder);
  }

  private prepareSetContent(output: View): { commit(): void; abort(): void } | undefined {
    if (this.disposed) return undefined;
    if (this.boundary === undefined) {
      // Older addons may not expose the retained ABI. Keep builder ownership
      // valid through a transactional native semantic publication.
      return {
        commit: (): void => {
          this.nativeAs<NativeScrollPaneHandle>().setContent(nodeForBridge(output));
          this.currentView = output;
        },
        abort(): void {},
      };
    }
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
          this.nativeAs<NativeScrollPaneHandle>().setContentRef(ref);
          // The direct decoder returns a temporary lease, while the pane
          // owns the installed content. Keep the boundary's root lease in
          // sync before releasing that temporary lease.
          this.boundary?.adopt(view);
          this.currentView = view;
          return;
        } finally {
          releaseNativeViewRef(nativeViewAbiSession(), ref);
        }
      }
      this.nativeAs<NativeScrollPaneHandle>().setContent(nodeForBridge(view));
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
    this.call(() => this.nativeAs<NativeScrollPaneHandle>().followEnd());
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
}

/** @internal Constructs a pane for the owning Tui and retained runtime. */
export function createScrollPane(
  host: never,
  initialView: View,
  retainedRuntime?: never,
): NativeScrollPane {
  const Constructor = NativeScrollPane as unknown as new (host: never, initialView: View, retainedRuntime?: never, token?: typeof SCROLL_PANE_NATIVE_TOKEN) => NativeScrollPane;
  return new Constructor(host, initialView, retainedRuntime, SCROLL_PANE_NATIVE_TOKEN);
}
