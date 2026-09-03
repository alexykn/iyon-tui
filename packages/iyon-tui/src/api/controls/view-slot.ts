import { FrameworkHandle } from "./framework-handle.ts";
import type { ComponentHandle } from "./framework-handle.ts";
import type { ComponentCapabilities } from "../extensions/traits/component.ts";
import {
  nativeViewAbiSession,
  releaseNativeViewRef,
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

/**
 * PERF-12 T13: builds the native slot for the initial content through the
 * single retained materialization path (identity-first, hint-driven); the
 * temporary lease always drains because the slot natively retains its own
 * strong copy. A retained refusal fails explicitly (PRE-V5-R0).
 */
function buildSlotHandle(
  host: NativeTuiHostContract,
  initialView: View | undefined,
  attachmentContext: AttachmentRuntimeContext | undefined,
): object {
  if (initialView !== undefined) prepareAttachmentsForView(initialView, attachmentContext).abort();
  const seed = initialView ?? View.spacer(0);
  const retained = tryRetainedMaterializeRef(seed);
  if (retained === undefined) {
    throw new Error("TUI_VIEW_SLOT_INITIALIZATION_FAILED: structural content could not be materialized");
  }
  try {
    return host.createViewSlotRef(retained);
  } finally {
    releaseNativeViewRef(nativeViewAbiSession(), retained);
  }
}

const ANIMATION_REF_SCRATCH = new WeakMap<object, Uint32Array>();

export interface ViewSlot extends ComponentHandle {
  readonly kind: "component";
  capabilities(): ComponentCapabilities;
  setView(view: View | (() => View)): void;
  setAnimation(frames: readonly View[], intervalMs: number): void;
  setAnimationAtCycleBoundary(frames: readonly View[], intervalMs: number): void;
  stopAnimation(view: View): void;
  revision(): number;
}

type ViewSlotContract = ViewSlot;

interface ViewSlotImplementation extends ViewSlotContract {
  prepareSetView(view: View): { commit(): void; abort(): void } | undefined;
}

const VIEW_SLOT_NATIVE_TOKEN = Symbol("view-slot-native-construction");

type NativeViewSlotHandle = {
  dispose(): void;
  revision(): number;
  componentId(): number | null;
  setViewRef(viewRef: number): void;
  setAnimationRef1(ref0: number, intervalMs: number): void;
  setAnimationRef2(ref0: number, ref1: number, intervalMs: number): void;
  setAnimationRef3(ref0: number, ref1: number, ref2: number, intervalMs: number): void;
  setAnimationRef4(ref0: number, ref1: number, ref2: number, ref3: number, intervalMs: number): void;
  setAnimationRef1AtCycleBoundary(ref0: number, intervalMs: number): void;
  setAnimationRef2AtCycleBoundary(ref0: number, ref1: number, intervalMs: number): void;
  setAnimationRef3AtCycleBoundary(ref0: number, ref1: number, ref2: number, intervalMs: number): void;
  setAnimationRef4AtCycleBoundary(ref0: number, ref1: number, ref2: number, ref3: number, intervalMs: number): void;
  setAnimationRefs(refs: Uint32Array, usedCount: number, intervalMs: number): void;
  setAnimationRefsAtCycleBoundary(refs: Uint32Array, usedCount: number, intervalMs: number): void;
  stopAnimationRef(viewRef: number): void;
};

/**
 * Tui-owned retained component slot.
 *
 * Construct with `Tui.createViewSlot()` only. The owning Tui disposes factory
 * slots during `close()`/`exit()`; callers may dispose them earlier. Their
 * builder root, View identity, animation state, and root lease remain owned by
 * the slot until disposal or an ownership transition. A slot may be mounted at
 * one location in the retained View graph; duplicate component nodes are
 * rejected, and reusing the handle does not create independent instances.
 * Builder mode is
 * available only through the Tui-created shared execution runtime. Direct
 * `setView(View)` takes ownership after successful publication; animation also
 * relinquishes any builder root only after installation succeeds. The handle
 * must not be used after its owning Tui closes.
 */
export class ViewSlot extends FrameworkHandle<"component"> implements ViewSlotContract {
  private currentView?: View;
  /** R7: mutable cell so transaction closures can promote currentView on commit. */
  private currentViewSet = (view: View): void => {
    this.currentView = view;
  };
  /**
   * PERF-12 T13 (§18/§80): this slot is a root-lease owner. The boundary
   * keeps the CURRENT content's lease alive across replacements so stable
   * subtrees stay native-live and their hints resolve during the next
   * materialization — no O(previous-tree) rebuild per update.
   */
  private boundary?: RetainedRootBoundary;

  /** R8: shared Tui execution runtime (undefined for raw internal construction — builder mode unsupported there). */
  #retainedRuntime?: RetainedExecutionRuntime;
  private ownedBuilderRoot?: OwnedBuilderRoot;
  private readonly attachmentContext: AttachmentRuntimeContext | undefined;
  private readonly attachmentBindings = new AttachmentBindingState();

  private constructor(host: never, initialView?: View, retainedRuntime?: never, token?: typeof VIEW_SLOT_NATIVE_TOKEN, attachmentContext?: object) {
    if (token !== VIEW_SLOT_NATIVE_TOKEN) throw new TypeError("ViewSlot native construction is private");
    const nativeHost = host as unknown as NativeTuiHostContract;
    const executionRuntime = retainedRuntime as unknown as RetainedExecutionRuntime | undefined;
    const runtimeAttachments = attachmentContext as AttachmentRuntimeContext | undefined;
    super("component", buildSlotHandle(nativeHost, initialView, runtimeAttachments) as never);
    this.currentView = initialView;
    this.attachmentContext = runtimeAttachments;
    this.#retainedRuntime = executionRuntime;
    const session = nativeViewAbiSession();
    const seed = initialView ?? View.spacer(0);
    this.boundary = new RetainedRootBoundary(session, () => undefined, (ref) => {
      if (this.disposed) return false;
      // Native exceptions remain visible to the retained transaction rather
      // than being converted into an unrelated error.
      this.nativeAs<NativeViewSlotHandle>().setViewRef(ref);
      return true;
    });
    // Adopt the initial content so the boundary owns its root lease.
    this.boundary.adopt(seed);
    if (initialView !== undefined) {
      const attachments = prepareAttachmentsForView(initialView, this.attachmentContext);
      this.attachmentBindings.commitDesired(attachments);
      this.attachmentBindings.commitVisible();
    }
  }

  tuiViewAbiInstallRef(viewRef: number): void { this.nativeAs<NativeViewSlotHandle>().setViewRef(viewRef); }

  view(): View {
    this.ensureOpen();
    return composeComponent(this);
  }
  capabilities(): ComponentCapabilities { return this.call(() => ({ ticks: true })); }
  revision(): number { return this.call(() => this.nativeAs<NativeViewSlotHandle>().revision()); }
  /**
   * PERF-12 T13.1 R8 (handoff §32.2.4): direct and builder forms are
   * OWNERSHIP MODES.
   *
   *   setView(() => View)  — builder takes ownership: a retained execution
   *       root owned by THIS slot renders the producer output; State reads
   *       subscribe automatically (no further setView calls needed).
   *   setView(view)        — DIRECT takes ownership: any previous builder
   *       root is disposed AFTER the direct view is successfully installed
   *       (transactional order: prepare/commit new, then release old).
   */
  setView(viewOrBuilder: View | (() => View)): void {
    if (typeof viewOrBuilder === "function") {
      this.setViewBuilder(viewOrBuilder);
      return;
    }
    this.setViewDirect(viewOrBuilder);
  }

  private assertUserMutationAllowed(operation: string): void {
    if ((protocolState.mutating && !protocolState.internalPublication) || activeExecutionScope() !== undefined) {
      throw new Error(`TUI_EXECUTION_REENTRANT_MUTATION: ${operation} during a retained protocol pass`);
    }
  }

  private assertBuilderAllowed(): void {
    this.ensureOpen();
    this.assertUserMutationAllowed("slot builder mutation");
    if (this.#retainedRuntime === undefined) {
      throw new Error(
        "TUI_EXECUTION_BUILDER_UNSUPPORTED: builder mode requires a Tui-created slot (shared execution runtime)",
      );
    }
  }

  private setViewBuilder(build: () => View): void {
    this.assertBuilderAllowed();
    const target = {
      preparePublication: (o: import("../view/view.ts").View) => this.prepareSetView(o),
    };
    if (this.ownedBuilderRoot === undefined) {
      const runtime = this.#retainedRuntime!;
      this.ownedBuilderRoot = OwnedBuilderRoot.start(runtime, build, target);
    } else {
      this.ownedBuilderRoot.replaceProducer(build);
    }
  }

  /** Direct ownership: install now, then dispose any owned builder root. */
  private setViewDirect(view: View): void {
    this.assertUserMutationAllowed("slot mutation");
    this.call(() => {
      const attachments = prepareAttachmentsForView(view, this.attachmentContext);
      try {
        // PERF-12 T13 retained path: identity-first install through the slot's
        // own §18 boundary. The previous content remains leased until the
        // replacement commits; a retained refusal fails explicitly.
        const publication = this.boundary!.prepareInstall(view);
        if (publication === undefined) {
          throw new Error("TUI_VIEW_SLOT_UPDATE_FAILED: structural content could not be materialized");
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
    // Transactional ownership transition (direct wins only after successful
    // publication): dispose any owned builder root LAST.
    this.disposeOwnedBuilder();
  }

  private disposeOwnedBuilder(): void {
    const root = this.ownedBuilderRoot;
    if (root !== undefined) {
      this.ownedBuilderRoot = undefined;
      root.dispose();
    }
  }
  /**
   * PERF-12 T13.1 R7: transactional variant of {@link setView}. Delegates to
   * the slot's own RetainedRootBoundary — ownership stays inside the boundary
   * (no split-brain). The returned publication's commit installs the prepared
   * root and its abort leaves the old content installed and leased.
   */
  prepareSetView(view: View): { commit(): void; abort(): void } | undefined {
    if (this.disposed) return undefined;
    const attachments = prepareAttachmentsForView(view, this.attachmentContext);
    // A retained preparation can refuse for an unsupported-kind reason; the
    // refusal surfaces as an explicit failure, never a second transport.
    let publication: ReturnType<RetainedRootBoundary["prepareInstall"]>;
    try {
      publication = this.boundary!.prepareInstall(view);
    } catch (error) {
      attachments.abort();
      throw error;
    }
    if (publication === undefined) {
      attachments.abort();
      return undefined;
    }
    const setCurrentView = (promoted: View): void => this.currentViewSet(promoted);
    const attachmentBindings = this.attachmentBindings;
    return {
      commit(): void {
        try {
          publication.commit();
          attachmentBindings.commitDesired(attachments);
          attachmentBindings.commitVisible();
          setCurrentView(view);
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

  setAnimation(frames: readonly View[], intervalMs: number): void {
    this.setAnimationWithRefs(frames, intervalMs, false);
  }
  setAnimationAtCycleBoundary(frames: readonly View[], intervalMs: number): void {
    this.setAnimationWithRefs(frames, intervalMs, true);
  }
  private setAnimationWithRefs(frames: readonly View[], intervalMs: number, atCycleBoundary: boolean): void {
    this.assertUserMutationAllowed("slot animation mutation");
    this.call(() => {
      if (frames.length === 0) throw new Error("native view slot animation requires at least one frame");
      for (const frame of frames) prepareAttachmentsForView(frame, this.attachmentContext).abort();
      // Small animations can stay scalar. Large animations write acquired refs
      // directly into the reusable native buffer; do not stage a second JS
      // number[] copy of the frame list.
      const scalarRefs: number[] | undefined = frames.length <= 4 ? [] : undefined;
      let scratch: Uint32Array | undefined;
      let acquiredCount = 0;
      try {
        if (scalarRefs === undefined) {
          scratch = this.animationScratch(frames.length);
        }
        for (const [index, frame] of frames.entries()) {
          // T13: animation frames ride retained identity too — stable frame
          // View objects hit their hints on every cycle instead of rebuilding.
          const ref = tryRetainedMaterializeRef(frame);
          if (ref === undefined) {
            throw new Error("TUI_VIEW_SLOT_ANIMATION_UPDATE_FAILED: structural content could not be materialized");
          }
          if (scratch !== undefined) scratch[index] = ref;
          else scalarRefs!.push(ref);
          acquiredCount += 1;
        }
        if (scalarRefs !== undefined && this.setFixedAnimationRefs(scalarRefs, intervalMs, atCycleBoundary)) return;
        if (scratch === undefined) {
          scratch = this.animationScratch(scalarRefs!.length);
          scratch.set(scalarRefs!);
        }
        if (atCycleBoundary) this.nativeAs<NativeViewSlotHandle>().setAnimationRefsAtCycleBoundary(scratch, acquiredCount, intervalMs);
        else this.nativeAs<NativeViewSlotHandle>().setAnimationRefs(scratch, acquiredCount, intervalMs);
      } finally {
        if (scratch !== undefined) {
          for (let index = 0; index < acquiredCount; index += 1) releaseNativeViewRef(nativeViewAbiSession(), scratch[index]!);
        } else {
          for (const ref of scalarRefs ?? []) releaseNativeViewRef(nativeViewAbiSession(), ref);
        }
      }
    });
    // Animation is an ownership transition just like direct setView: the
    // builder relinquishes control only after the native animation install
    // succeeds, so a failed install leaves the builder authoritative.
    this.disposeOwnedBuilder();
  }

  private animationScratch(requiredLength: number): Uint32Array {
    let scratch = ANIMATION_REF_SCRATCH.get(this.nativeAs<NativeViewSlotHandle>() as object);
    if (scratch === undefined || scratch.length < requiredLength) {
      scratch = new Uint32Array(Math.max(requiredLength, 4));
      ANIMATION_REF_SCRATCH.set(this.nativeAs<NativeViewSlotHandle>() as object, scratch);
    }
    return scratch;
  }
  private setFixedAnimationRefs(refs: readonly number[], intervalMs: number, atCycleBoundary: boolean): boolean {
    const native = this.nativeAs<NativeViewSlotHandle>();
    if (atCycleBoundary) {
      switch (refs.length) {
        case 1: native.setAnimationRef1AtCycleBoundary(refs[0]!, intervalMs); return true;
        case 2: native.setAnimationRef2AtCycleBoundary(refs[0]!, refs[1]!, intervalMs); return true;
        case 3: native.setAnimationRef3AtCycleBoundary(refs[0]!, refs[1]!, refs[2]!, intervalMs); return true;
        case 4: native.setAnimationRef4AtCycleBoundary(refs[0]!, refs[1]!, refs[2]!, refs[3]!, intervalMs); return true;
        default: return false;
      }
    }
    switch (refs.length) {
      case 1: native.setAnimationRef1(refs[0]!, intervalMs); return true;
      case 2: native.setAnimationRef2(refs[0]!, refs[1]!, intervalMs); return true;
      case 3: native.setAnimationRef3(refs[0]!, refs[1]!, refs[2]!, intervalMs); return true;
      case 4: native.setAnimationRef4(refs[0]!, refs[1]!, refs[2]!, refs[3]!, intervalMs); return true;
      default: return false;
    }
  }

  stopAnimation(view: View): void {
    this.assertUserMutationAllowed("slot animation mutation");
    this.call(() => {
      prepareAttachmentsForView(view, this.attachmentContext).abort();
      const ref = tryRetainedMaterializeRef(view);
      if (ref === undefined) {
        throw new Error("TUI_VIEW_SLOT_ANIMATION_UPDATE_FAILED: structural content could not be materialized");
      }
      try {
        this.nativeAs<NativeViewSlotHandle>().stopAnimationRef(ref);
      } finally {
        releaseNativeViewRef(nativeViewAbiSession(), ref);
      }
    });
    this.disposeOwnedBuilder();
  }

  /** Releases owned execution state and the root lease before native teardown. */
  dispose(): void {
    if (!this.disposed) {
      // A disposed slot must not leave its builder root subscribed to State or
      // queued in the shared runtime. Dispose the producer first, while the
      // native slot is still a valid target, then release its root lease.
      const root = this.ownedBuilderRoot;
      if (root !== undefined) {
        this.ownedBuilderRoot = undefined;
        root.dispose();
      }
      try {
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

/** @internal Constructs a slot for the owning Tui and retained runtime. */
export function createViewSlot(
  host: never,
  initialView: View,
  retainedRuntime?: never,
  attachmentContext?: object,
): ViewSlotImplementation {
  const Constructor = ViewSlot as unknown as new (
    host: never,
    initialView: View,
    retainedRuntime?: never,
    token?: typeof VIEW_SLOT_NATIVE_TOKEN,
    attachmentContext?: object,
  ) => ViewSlotImplementation;
  return new Constructor(host, initialView, retainedRuntime, VIEW_SLOT_NATIVE_TOKEN, attachmentContext);
}
