import { FrameworkHandle } from "./framework-handle.ts";
import type { ComponentHandle } from "./framework-handle.ts";
import type { ComponentCapabilities } from "../extensions/traits/component.ts";
import {
  nativeViewAbiSession,
  releaseNativeViewRef,
  tryNativeMaterialize,
  tryRetainedMaterializeRef,
} from "../../transport/structural/native-view-abi.ts";
import { RetainedRootBoundary } from "../../transport/structural/retained-dag.ts";
import {
  OwnedBuilderRoot,
  type RetainedExecutionRuntime,
} from "../../composition/execution.ts";
import { activeExecutionScope, protocolState } from "../../composition/execution-context.ts";
import { composeComponent } from "../../composition/compose.ts";
import { View } from "../view/view.ts";
import type { NativeTuiHostContract } from "../../transport/native/addon.ts";
import {
  AttachmentBindingState,
  prepareAttachmentsForView,
  type AttachmentRuntimeContext,
} from "../../runtime/attachments.ts";

export interface ScrollPane extends ComponentHandle {
  readonly kind: "component";
  capabilities(): ComponentCapabilities;
  setContent(view: View | (() => View)): void;
  followEnd(): void;
}

type ScrollPaneContract = ScrollPane;

const SCROLL_PANE_NATIVE_TOKEN = Symbol("scroll-pane-native-construction");

type NativeScrollPaneHandle = {
  dispose(): void;
  componentId(): number | null;
  setContentRef(viewRef: number): void;
  followEnd(): void;
};

/** T13: retained-first construction of the pane's initial content. */
function buildPaneHandle(
  host: NativeTuiHostContract,
  initialView: View | undefined,
  attachmentContext: AttachmentRuntimeContext | undefined,
): object {
  if (initialView !== undefined) prepareAttachmentsForView(initialView, attachmentContext).abort();
  const seed = initialView ?? View.spacer(0);
  const retained = tryRetainedMaterializeRef(seed) ?? tryNativeMaterialize(seed);
  if (retained === undefined) {
    throw new Error("TUI_SCROLL_PANE_INITIALIZATION_FAILED: structural content could not be materialized");
  }
  try {
    return host.scrollPaneRef(retained);
  } finally {
    releaseNativeViewRef(nativeViewAbiSession(), retained);
  }
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
  private readonly attachmentContext: AttachmentRuntimeContext | undefined;
  private readonly attachmentBindings = new AttachmentBindingState();

  private constructor(host: never, initialView?: View, retainedRuntime?: never, token?: typeof SCROLL_PANE_NATIVE_TOKEN, attachmentContext?: object) {
    if (token !== SCROLL_PANE_NATIVE_TOKEN) throw new TypeError("ScrollPane native construction is private");
    const nativeHost = host as unknown as NativeTuiHostContract;
    const executionRuntime = retainedRuntime as unknown as RetainedExecutionRuntime | undefined;
    const runtimeAttachments = attachmentContext as AttachmentRuntimeContext | undefined;
    super("component", buildPaneHandle(nativeHost, initialView, runtimeAttachments) as never);
    this.currentView = initialView;
    this.attachmentContext = runtimeAttachments;
    this.#retainedRuntime = executionRuntime;
    const session = nativeViewAbiSession();
    const seed = initialView ?? View.spacer(0);
    this.boundary = new RetainedRootBoundary(session, () => undefined, (ref) => {
      if (this.disposed) return false;
      // Native failures must remain visible to the retained transaction;
      // returning false here would hide a broken pane installation.
      this.nativeAs<NativeScrollPaneHandle>().setContentRef(ref);
      return true;
    });
    this.boundary.adopt(seed);
    if (initialView !== undefined) {
      const attachments = prepareAttachmentsForView(initialView, this.attachmentContext);
      this.attachmentBindings.commitDesired(attachments);
      this.attachmentBindings.commitVisible();
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
    const attachments = prepareAttachmentsForView(output, this.attachmentContext);
    // Retained preparation may refuse on a bounded/unsupported path; use the
    // complete cold materializer transactionally before reporting failure.
    let publication: ReturnType<RetainedRootBoundary["prepareInstall"]>;
    try {
      publication = this.boundary!.prepareInstall(output) ?? this.boundary!.prepareColdInstall(output);
    } catch (error) {
      attachments.abort();
      throw error;
    }
    if (publication === undefined) {
      attachments.abort();
      return undefined;
    }
    const setCurrent = (promoted: View): void => {
      this.currentView = promoted;
    };
    const attachmentBindings = this.attachmentBindings;
    return {
      commit(): void {
        try {
          publication.commit();
          attachmentBindings.commitDesired(attachments);
          attachmentBindings.commitVisible();
          setCurrent(output);
        } catch (error) {
          attachments.abort();
          throw error;
        }
      },
      abort(): void {
        try {
          publication.abort();
        } finally {
          attachments.abort();
        }
      },
    };
  }

  private setContentDirect(view: View): void {
    if (protocolState.mutating || activeExecutionScope() !== undefined) {
      throw new Error("TUI_EXECUTION_REENTRANT_MUTATION: pane mutation during a retained protocol pass");
    }
    this.call(() => {
      const attachments = prepareAttachmentsForView(view, this.attachmentContext);
      try {
        // PERF-12 T13 retained path (§80): previous content stays leased until
        // the replacement is fully materialized and committed. Capacity misses
        // use the canonical cold materialization transaction; there is no
        // parallel native-view mutation route.
        const publication = this.boundary!.prepareInstall(view)
          ?? this.boundary!.prepareColdInstall(view);
        if (publication === undefined) {
          throw new Error("TUI_SCROLL_PANE_UPDATE_FAILED: structural content could not be materialized");
        }
        publication.commit();
        this.attachmentBindings.commitDesired(attachments);
        this.attachmentBindings.commitVisible();
        this.currentView = view;
      } catch (error) {
        attachments.abort();
        throw error;
      }
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
        this.attachmentBindings.dispose();
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
  attachmentContext?: object,
): NativeScrollPane {
  const Constructor = NativeScrollPane as unknown as new (
    host: never,
    initialView: View,
    retainedRuntime?: never,
    token?: typeof SCROLL_PANE_NATIVE_TOKEN,
    attachmentContext?: object,
  ) => NativeScrollPane;
  return new Constructor(host, initialView, retainedRuntime, SCROLL_PANE_NATIVE_TOKEN, attachmentContext);
}
