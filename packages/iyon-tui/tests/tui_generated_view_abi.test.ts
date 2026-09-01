import { describe, expect, test } from "bun:test";
import { native } from "../src/transport/native/addon.ts";
import { nativeViewAbiSession } from "../src/transport/structural/native-view-abi.ts";
import { hostRenderRef, runtimeNoop, viewCommonPatchRoot, viewRefForNodeId, viewRenderRef, viewReleaseMany, viewSpacerCreate, viewTextLayoutPatchRoot } from "../src/transport/abi/structural/generated/view_calls.ts";
import { lowerColdView } from "../src/transport/structural/cold-lowering.ts";
import { nodeIdPair, View } from "../src/api/view/view.ts";

type AbiHost = NonNullable<typeof native.NativeTuiHost> extends new (...args: never[]) => infer T ? T : never;

const Host = native.NativeTuiHost as unknown as (new (width: number, height: number, headless: boolean) => AbiHost) | undefined;

describe("PERF-11 generated vertical slice", () => {
  test("loads the generated N-API session and keeps its opaque handle stable", () => {
    const session = nativeViewAbiSession();
    if (session === undefined) return;
    expect(session.abi.function_count).toBe(59);
    const addon = native as unknown as Record<string, unknown>;
    expect(addon.tuiViewAbiBootstrap).toBeUndefined();
    expect(addon.tuiPerfAbiProbe).toBeUndefined();
    expect(session.abi.transport).toBe("napi");
    expect(session.abi.generation).toBeGreaterThan(0);
    expect(session.symbols).toBeDefined();
    expect(nativeViewAbiSession()?.runtime).toBe(session.runtime);
    expect(runtimeNoop(session.symbols, session.runtime)).toBe(1);
  });

  test("renders an existing native ref through the generated host call", () => {
    if (Host === undefined) return;
    const session = nativeViewAbiSession();
    if (session === undefined) return;
    const host = new Host(8, 4, true);
    expect((host as unknown as Record<string, unknown>).tuiViewAbiHostPointer).toBeUndefined();
    const view = View.spacer(2);
    try {
      host.render(lowerColdView(view));
      const [nodeIdLow, nodeIdHigh] = nodeIdPair(view);
      const reference = viewRefForNodeId(session.symbols, session.runtime, nodeIdLow, nodeIdHigh);
      expect(hostRenderRef(session.symbols, session.runtime, host, reference)).toBe(0);
      expect(host.screenRows()).toEqual(["        ", "        ", "        ", "        "]);
      viewReleaseMany(session.symbols, session.runtime, new Uint32Array([reference]), 1);
    } finally {
      host.dispose();
    }
  });

  test("creates, resolves, publishes, and releases a native spacer", () => {
    const session = nativeViewAbiSession();
    if (session === undefined) return;
    const view = View.spacer(2);
    const [nodeIdLow, nodeIdHigh] = nodeIdPair(view);
    const reference = viewSpacerCreate(session.symbols, session.runtime, nodeIdLow, nodeIdHigh, 2);
    expect(reference).toBeGreaterThan(0);
    expect(viewRenderRef(session.symbols, session.runtime, reference)).toBe(reference);
    expect(viewReleaseMany(session.symbols, session.runtime, new Uint32Array([reference]), 1)).toBe(1);
    expect(() => viewRenderRef(session.symbols, session.runtime, reference)).toThrow();
  });

  test("patches text metadata and preserves parity with the existing host decoder", () => {
    if (Host === undefined) return;
    const session = nativeViewAbiSession();
    if (session === undefined) return;
    const host = new Host(8, 4, true);
    const direct = new Host(8, 4, true);
    const textBase = View.text("hello");
    const textChanged = textBase.noWrap().textAlign("center");
    try {
      host.render(lowerColdView(textBase));
      const [textLow, textHigh] = nodeIdPair(textBase);
      const textRef = viewRefForNodeId(session.symbols, session.runtime, textLow, textHigh);
      const [textChangedLow, textChangedHigh] = nodeIdPair(textChanged);
      const textChangedRef = viewTextLayoutPatchRoot(
        session.symbols,
        session.runtime,
        textRef,
        textChangedLow,
        textChangedHigh,
        3,
        2,
      );
      expect(textChangedRef).toBeGreaterThan(0);
      host.render(lowerColdView(textChanged));
      const directText = View.text("hello").noWrap().textAlign("center");
      direct.render(lowerColdView(directText));
      expect(host.screenRows()).toEqual(direct.screenRows());
      viewReleaseMany(session.symbols, session.runtime, new Uint32Array([textRef, textChangedRef]), 2);

      const base = View.spacer(1);
      const [baseLow, baseHigh] = nodeIdPair(base);
      const baseRef = viewSpacerCreate(session.symbols, session.runtime, baseLow, baseHigh, 1);
      const changed = base.padding(1);
      const [changedLow, changedHigh] = nodeIdPair(changed);
      const paddingWord = (1 << 16) | 1;
      const changedRef = viewCommonPatchRoot(
        session.symbols,
        session.runtime,
        baseRef,
        changedLow,
        changedHigh,
        4,
        paddingWord,
        paddingWord,
        1,
        1,
        0,
        0,
        0,
        0,
        baseRef,
      );
      expect(changedRef).toBeGreaterThan(0);
      host.render(lowerColdView(base));
      host.render(lowerColdView(changed));
      const directBase = View.spacer(1);
      direct.render(lowerColdView(directBase));
      direct.render(lowerColdView(directBase.padding(1)));
      expect(host.screenRows()).toEqual(direct.screenRows());
      viewReleaseMany(session.symbols, session.runtime, new Uint32Array([baseRef, changedRef]), 2);
    } finally {
      direct.dispose();
      host.dispose();
    }
  });
});
