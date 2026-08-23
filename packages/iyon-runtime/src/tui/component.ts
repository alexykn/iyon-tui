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
  /**
   * PERF-12 T13 (§18/§80): this slot is a root-lease owner. The boundary
   * keeps the CURRENT content's lease alive across replacements so stable
   * subtrees stay native-live and their hints resolve during the next
   * materialization — no O(previous-tree) rebuild per update.
   */
  private boundary?: RetainedRootBoundary;

  constructor(host: NativeTuiHostContract, initialView?: View) {
    super("component", buildSlotHandle(host, initialView) as NativeViewSlotHandle);
    this.currentView = initialView;
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
  setView(view: View): void {
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
  }
  setAnimation(frames: readonly View[], intervalMs: number): void {
    this.setAnimationWithRefs(frames, intervalMs, false);
  }
  setAnimationAtCycleBoundary(frames: readonly View[], intervalMs: number): void {
    this.setAnimationWithRefs(frames, intervalMs, true);
  }
  private setAnimationWithRefs(frames: readonly View[], intervalMs: number, atCycleBoundary: boolean): void {
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
  }

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
