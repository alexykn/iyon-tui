import { HandleBase, requireNativeClass } from "./handles.ts";
import { native } from "../native.ts";
import type { Component as ComponentContract, ComponentCapabilities, ViewSlot as ViewSlotContract } from "./types.ts";
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

/**
 * PERF-12 T13: builds the native slot for the initial content. Retained
 * materialization first (identity-first, hint-driven), then the cold FFI
 * graph, then the N-API bridge; the temporary lease always drains because
 * the slot natively retains its own strong copy.
 */
function buildSlotHandle(host: NativeTuiHostContract, initialView?: View): object {
  const seed = initialView ?? View.spacer(0);
  const retained = tryRetainedMaterializeRef(seed) ?? tryNativeMaterialize(seed);
  if (retained !== undefined) {
    try {
      return host.createViewSlotRef(retained);
    } finally {
      releaseNativeViewRef(nativeViewAbiSession(), retained);
    }
  }
  return host.createViewSlot(nodeForBridge(seed));
}

const ANIMATION_REF_SCRATCH = new WeakMap<object, Uint32Array>();

type NativeViewSlotHandle = {
  dispose(): void;
  revision(): number;
  componentId(): number | null;
  setView(view: object): void;
  setViewRef(viewRef: number): void;
  setAnimation(frames: object[], intervalMs: number): void;
  setAnimationAtCycleBoundary(frames: object[], intervalMs: number): void;
  setAnimationRef1?(ref0: number, intervalMs: number): void;
  setAnimationRef2?(ref0: number, ref1: number, intervalMs: number): void;
  setAnimationRef3?(ref0: number, ref1: number, ref2: number, intervalMs: number): void;
  setAnimationRef4?(ref0: number, ref1: number, ref2: number, ref3: number, intervalMs: number): void;
  setAnimationRef1AtCycleBoundary?(ref0: number, intervalMs: number): void;
  setAnimationRef2AtCycleBoundary?(ref0: number, ref1: number, intervalMs: number): void;
  setAnimationRef3AtCycleBoundary?(ref0: number, ref1: number, ref2: number, intervalMs: number): void;
  setAnimationRef4AtCycleBoundary?(ref0: number, ref1: number, ref2: number, ref3: number, intervalMs: number): void;
  setAnimationRefs(refs: Uint32Array, usedCount: number, intervalMs: number): void;
  setAnimationRefsAtCycleBoundary(refs: Uint32Array, usedCount: number, intervalMs: number): void;
  stopAnimation(view: object): void;
  stopAnimationRef(viewRef: number): void;
};

export class ViewSlot extends HandleBase<NativeViewSlotHandle, "component"> implements ViewSlotContract {
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
  private readonly retainedRuntime?: RetainedExecutionRuntime;
  private ownedBuilderRoot?: OwnedBuilderRoot;

  constructor(
    host: NativeTuiHostContract,
    initialView?: View,
    retainedRuntime?: RetainedExecutionRuntime,
  ) {
    super("component", buildSlotHandle(host, initialView) as NativeViewSlotHandle);
    this.currentView = initialView;
    this.retainedRuntime = retainedRuntime;
    const session = nativeViewAbiSession();
    if (session !== undefined && initialView !== undefined) {
      this.boundary = new RetainedRootBoundary(session, () => undefined, (ref) => {
        if (this.disposed || this.nativeHandle.setViewRef === undefined) return false;
        try {
          this.nativeHandle.setViewRef(ref);
          return true;
        } catch {
          return false;
        }
      });
      // Adopt the initial content so the boundary owns its root lease.
      this.boundary.adopt(initialView);
    }
  }

  tuiViewAbiInstallRef(viewRef: number): void { this.nativeHandle.setViewRef(viewRef); }

  view(): View {
    this.ensureOpen();
    return this.nativeComponentId() === undefined ? View.spacer(0) : View.component(this);
  }
  capabilities(): ComponentCapabilities { return this.call(() => ({})); }
  revision(): number { return this.call(() => this.nativeHandle.revision()); }
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
    this.assertUserMutationAllowed("slot builder mutation");
    if (this.retainedRuntime === undefined) {
      throw new Error(
        "TUI_EXECUTION_BUILDER_UNSUPPORTED: builder mode requires a Tui-created slot (shared execution runtime)",
      );
    }
  }

  private setViewBuilder(build: () => View): void {
    this.assertBuilderAllowed();
    const target = {
      preparePublication: (o: import("./values/view.ts").View) => this.prepareSetView(o),
    };
    if (this.ownedBuilderRoot === undefined) {
      const runtime = this.retainedRuntime!;
      this.ownedBuilderRoot = OwnedBuilderRoot.start(runtime, build, target);
    } else {
      this.ownedBuilderRoot.replaceProducer(build);
    }
  }

  /** Direct ownership: install now, then dispose any owned builder root. */
  private setViewDirect(view: View): void {
    this.assertUserMutationAllowed("slot mutation");
    this.call(() => {
      // PERF-12 T13 retained path: identity-first install through the slot's
      // own §18 boundary. Previous content stays leased until the replacement
      // is fully materialized and committed; failure keeps the old content.
      if (this.boundary !== undefined) {
        if (this.boundary.install(view) !== undefined) {
          this.currentView = view;
          return;
        }
        // Refused → complete fallback below; old content still installed.
      }
      const ref = tryNativeMaterialize(view);
      if (ref !== undefined) {
        try {
          this.nativeHandle.setViewRef(ref);
          this.currentView = view;
          return;
        } finally {
          releaseNativeViewRef(nativeViewAbiSession(), ref);
        }
      }
      this.nativeHandle.setView(nodeForBridge(view));
      this.currentView = view;
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
   * (no split-brain). Returns `undefined` when no boundary is available
   * (caller falls back to {@link setView}); otherwise returns a publication
   * whose commit publishes the prepared root and whose abort leaves the old
   * content installed and leased.
   */
  prepareSetView(view: View): { commit(): void; abort(): void } | undefined {
    if (this.disposed || this.boundary === undefined) return undefined;
    // A retained preparation can refuse for a budget/unsupported-kind
    // reason. Complete cold materialization is still a valid transactional
    // fallback, and must happen before publication rather than turning a
    // renderable update into an unnecessary abort.
    const publication = this.boundary.prepareInstall(view) ?? this.boundary.prepareColdInstall(view);
    if (publication === undefined) return undefined;
    const setCurrentView = (promoted: View): void => this.currentViewSet(promoted);
    return {
      commit(): void {
        publication.commit();
        setCurrentView(view);
      },
      abort(): void {
        publication.abort();
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
          const ref = tryRetainedMaterializeRef(frame) ?? tryNativeMaterialize(frame);
          if (ref === undefined) {
            this.setAnimationBridge(frames, intervalMs, atCycleBoundary);
            return;
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
        if (atCycleBoundary) this.nativeHandle.setAnimationRefsAtCycleBoundary(scratch, acquiredCount, intervalMs);
        else this.nativeHandle.setAnimationRefs(scratch, acquiredCount, intervalMs);
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
    let scratch = ANIMATION_REF_SCRATCH.get(this.nativeHandle as object);
    if (scratch === undefined || scratch.length < requiredLength) {
      scratch = new Uint32Array(Math.max(requiredLength, 4));
      ANIMATION_REF_SCRATCH.set(this.nativeHandle as object, scratch);
    }
    return scratch;
  }
  private setAnimationBridge(frames: readonly View[], intervalMs: number, atCycleBoundary: boolean): void {
    if (atCycleBoundary) this.nativeHandle.setAnimationAtCycleBoundary(frames.map(nodeForBridge), intervalMs);
    else this.nativeHandle.setAnimation(frames.map(nodeForBridge), intervalMs);
  }

  private setFixedAnimationRefs(refs: readonly number[], intervalMs: number, atCycleBoundary: boolean): boolean {
    if (atCycleBoundary) {
      switch (refs.length) {
        case 1:
          if (this.nativeHandle.setAnimationRef1AtCycleBoundary === undefined) return false;
          this.nativeHandle.setAnimationRef1AtCycleBoundary(refs[0]!, intervalMs);
          return true;
        case 2:
          if (this.nativeHandle.setAnimationRef2AtCycleBoundary === undefined) return false;
          this.nativeHandle.setAnimationRef2AtCycleBoundary(refs[0]!, refs[1]!, intervalMs);
          return true;
        case 3:
          if (this.nativeHandle.setAnimationRef3AtCycleBoundary === undefined) return false;
          this.nativeHandle.setAnimationRef3AtCycleBoundary(refs[0]!, refs[1]!, refs[2]!, intervalMs);
          return true;
        case 4:
          if (this.nativeHandle.setAnimationRef4AtCycleBoundary === undefined) return false;
          this.nativeHandle.setAnimationRef4AtCycleBoundary(refs[0]!, refs[1]!, refs[2]!, refs[3]!, intervalMs);
          return true;
      }
      return false;
    }
    switch (refs.length) {
      case 1:
        if (this.nativeHandle.setAnimationRef1 === undefined) return false;
        this.nativeHandle.setAnimationRef1(refs[0]!, intervalMs);
        return true;
      case 2:
        if (this.nativeHandle.setAnimationRef2 === undefined) return false;
        this.nativeHandle.setAnimationRef2(refs[0]!, refs[1]!, intervalMs);
        return true;
      case 3:
        if (this.nativeHandle.setAnimationRef3 === undefined) return false;
        this.nativeHandle.setAnimationRef3(refs[0]!, refs[1]!, refs[2]!, intervalMs);
        return true;
      case 4:
        if (this.nativeHandle.setAnimationRef4 === undefined) return false;
        this.nativeHandle.setAnimationRef4(refs[0]!, refs[1]!, refs[2]!, refs[3]!, intervalMs);
        return true;
      default:
        return false;
    }
  }

  stopAnimation(view: View): void {
    this.assertUserMutationAllowed("slot animation mutation");
    this.call(() => {
      const ref = tryRetainedMaterializeRef(view) ?? tryNativeMaterialize(view);
      if (ref !== undefined) {
        try {
          this.nativeHandle.stopAnimationRef(ref);
          return;
        } finally {
          releaseNativeViewRef(nativeViewAbiSession(), ref);
        }
      }
      this.nativeHandle.stopAnimation(nodeForBridge(view));
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
      }
    }
    super.dispose();
  }
  nativeComponentId(): number | undefined {
    const id = this.nativeHandle.componentId();
    return id === null ? undefined : id;
  }
}

export class Component extends HandleBase<NativeViewSlotHandle, "component"> implements ComponentContract {
  constructor() {
    const NativeViewSlot = requireNativeClass(native.NativeViewSlot, "NativeViewSlot");
    super("component", new NativeViewSlot(nodeForBridge(View.spacer(0))));
  }
  view(): View {
    this.ensureOpen();
    return this.nativeComponentId() === undefined ? View.spacer(0) : View.component(this);
  }
  capabilities(): ComponentCapabilities { return this.call(() => ({})); }
  revision(): number { return this.call(() => this.nativeHandle.revision()); }
  nativeComponentId(): number | undefined {
    const id = this.nativeHandle.componentId();
    return id === null ? undefined : id;
  }
}
