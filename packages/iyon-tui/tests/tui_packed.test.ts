import { describe, expect, test } from "bun:test";

import { native } from "../src/native.ts";
import { BRIDGE_HORIZONTAL_ALIGN, BRIDGE_VIEW_KIND, BRIDGE_WRAP_MODE, type BridgeViewNode } from "../src/ir.ts";
import { View, nodeForBridge } from "../src/values/view.ts";
import { TextSpan } from "../src/values/text.ts";
import { Style } from "../src/values/style.ts";
import { DiffHunk, DiffLine, DiffRange, DiffRenderer } from "../src/values/diff.ts";
import {
  PackedViewEncoder,
  createPackedViewEncoder,
  isPackedCacheMiss,
  packedEncoderSnapshot,
  resetPackedEncoderCounters,
  renderPackedView,
  splitSafeU64,
} from "../src/packed.ts";

type PackedHost = {
  tuiPerfPackedRender?: (words: Uint32Array, strings: string[]) => void;
  render(view: object): void;
  screenRows(): string[];
  dispose(): void;
};

const packedHost = (host: PackedHost): ((words: Uint32Array, strings: string[]) => void) => {
  if (host.tuiPerfPackedRender === undefined) throw new Error("native addon was not staged with perf-packed-benchmark");
  return (words, strings) => host.tuiPerfPackedRender!(words, strings);
};

function packedTransportAvailable(): boolean {
  const Host = native.NativeTuiHost as unknown as (new (width: number, height: number, headless: boolean) => PackedHost) | undefined;
  if (Host === undefined) return false;
  const host = new Host(1, 1, true);
  try { return host.tuiPerfPackedRender !== undefined; } finally { host.dispose(); }
}

function renderPair(view: View): readonly [string[], string[]] {
  const Host = native.NativeTuiHost as unknown as new (width: number, height: number, headless: boolean) => PackedHost;
  if (Host === undefined) throw new Error("native TUI host is unavailable");
  const direct = new Host(80, 24, true);
  const packed = new Host(80, 24, true);
  try {
    direct.render(nodeForBridge(view));
    renderPackedView(createPackedViewEncoder(), view, packedHost(packed));
    return [direct.screenRows(), packed.screenRows()];
  } finally {
    direct.dispose();
    packed.dispose();
  }
}

describe("PERF-7v2 packed retained View transaction", () => {
  test("preserves the complete semantic schema against direct decoding", () => {
    if (!packedTransportAvailable()) return;
    const style = Style.new().theme("surface").foreground("cyan").background({ type: "ansi", value: 34 }).bold().attribute("italic", false);
    const diff = new DiffRenderer().render([
      new DiffHunk(new DiffRange(0, 2), new DiffRange(0, 2), [
        DiffLine.context(1, 1, "same"),
        DiffLine.deletion(2, "old"),
        DiffLine.addition(2, "new"),
      ]),
    ]);
    const shared = View.text("shared").style(style);
    const view = View.vertical((column) => {
      column.gap(1);
      column.child(View.styledText([TextSpan.plain("a"), TextSpan.styled("b", style)]).noWrap().textAlign("center"));
      column.child(diff);
      column.child(View.spacer(1));
      column.child(View.horizontal((row) => { row.fixed(4, shared); row.flex(View.text("flex")); }));
      column.child(View.hanging(View.text("> "), View.text("  "), View.text("body")));
      column.child(View.grid({
        columns: [{ kind: "fixed", size: 10 }, { kind: "flex" }],
        rows: [{ track: { kind: "content" }, cells: [{ view: View.text("grid"), columnSpan: 2, verticalAlign: "bottom" }] }],
        columnGap: 1,
        rowGap: 1,
      }));
      column.child(View.text("decorated").padding(1).border({ style: "rounded", edges: "topBottom", color: "yellow" }).styleState("phase", "live").minWidth(2).maxWidth(20).fitHeight());
      column.child(View.text("clamped").clampRows(1, { kind: "footer", prefix: "more", style }));
      column.child(View.contentMax(1, View.text("content max")));
    });
    const [directRows, packedRows] = renderPair(view);
    expect(packedRows).toEqual(directRows);
  });

  test("preserves a registered Component node through the packed schema", () => {
    const Host = native.NativeTuiHost as unknown as new (width: number, height: number, headless: boolean) => PackedHost & { createViewSlot(initial: object): { componentId(): number | null } };
    if (Host === undefined || !packedTransportAvailable()) return;
    const direct = new Host(20, 4, true);
    const packed = new Host(20, 4, true);
    try {
      const directSlot = direct.createViewSlot(nodeForBridge(View.spacer(0)));
      const packedSlot = packed.createViewSlot(nodeForBridge(View.spacer(0)));
      const directId = directSlot.componentId();
      const packedId = packedSlot.componentId();
      if (directId === null || packedId === null) throw new Error("component registration did not produce an id");
      const directView = View.component({ id: directId as never });
      const packedView = View.component({ id: packedId as never });
      direct.render(nodeForBridge(directView));
      renderPackedView(createPackedViewEncoder(), packedView, packedHost(packed));
      expect(packed.screenRows()).toEqual(direct.screenRows());
    } finally {
      direct.dispose();
      packed.dispose();
    }
  });

  test("splits every supported safe NodeId without truncation", () => {
    for (const value of [1, 2 ** 32 - 1, 2 ** 32, 2 ** 32 + 1, Number.MAX_SAFE_INTEGER]) {
      const [low, high] = splitSafeU64(value);
      expect(high * 2 ** 32 + low).toBe(value);
    }
    for (const value of [0, -1, 2 ** 53, 1.5, Number.NaN, Number.POSITIVE_INFINITY]) {
      expect(() => splitSafeU64(value)).toThrow(RangeError);
    }
  });

  test("round-trips packed NodeIds above the u32 domain through native decoding", () => {
    if (!packedTransportAvailable()) return;
    const Host = native.NativeTuiHost as unknown as new (width: number, height: number, headless: boolean) => PackedHost;
    if (Host === undefined) throw new Error("native TUI host is unavailable");
    const host = new Host(40, 4, true);
    try {
      for (const id of [2 ** 32 - 1, 2 ** 32, 2 ** 32 + 1, Number.MAX_SAFE_INTEGER]) {
        const node = {
          id,
          schema: 1,
          kind: BRIDGE_VIEW_KIND.text,
          spans: [{ text: `wide-${id}` }],
          wrap: BRIDGE_WRAP_MODE.noWrap,
          align: BRIDGE_HORIZONTAL_ALIGN.start,
        } as unknown as BridgeViewNode;
        const transaction = new PackedViewEncoder().encodeRoots([node]);
        packedHost(host)(transaction.words, transaction.strings);
        expect(host.screenRows().some((row) => row.includes(`wide-${id}`))).toBe(true);
      }
    } finally {
      host.dispose();
    }
  });

  test("uses backward references for exact DAG duplicates", () => {
    const shared = nodeForBridge(View.text("same"));
    const root = nodeForBridge(View.vertical([View.text("left"), View.text("right")]));
    const duplicated = { ...root, children: [{ kind: 1, child: shared }, { kind: 1, child: shared }] } as unknown as BridgeViewNode;
    resetPackedEncoderCounters();
    const encoder = new PackedViewEncoder();
    encoder.encodeRoots([duplicated]);
    expect(packedEncoderSnapshot().packed_encoder_ref_records).toBe(1);
  });

  test("rejects cyclic packed DAGs before mutating the host", () => {
    if (!packedTransportAvailable()) return;
    const Host = native.NativeTuiHost as unknown as new (width: number, height: number, headless: boolean) => PackedHost;
    if (Host === undefined) throw new Error("native TUI host is unavailable");
    const host = new Host(40, 4, true);
    try {
      host.render(nodeForBridge(View.text("before")));
      const before = host.screenRows();
      const cycle = { id: 9_000_000, schema: 1, kind: BRIDGE_VIEW_KIND.container } as unknown as BridgeViewNode & { child: BridgeViewNode };
      cycle.child = cycle;
      const transaction = new PackedViewEncoder().encodeRoots([cycle]);
      expect(() => packedHost(host)(transaction.words, transaction.strings)).toThrow("cyclic");
      expect(host.screenRows()).toEqual(before);
    } finally {
      host.dispose();
    }
  });

  test("resynchronizes once after native weak-cache expiry", () => {
    const Host = native.NativeTuiHost as unknown as new (width: number, height: number, headless: boolean) => PackedHost;
    const reset = (native as typeof native & { tuiPerfResetViewBridgeCache?: () => void }).tuiPerfResetViewBridgeCache;
    if (Host === undefined || reset === undefined || !packedTransportAvailable()) return;
    const host = new Host(40, 10, true);
    const encoder = createPackedViewEncoder();
    const view = View.vertical([View.text("a"), View.text("b")]);
    try {
      const invoke = packedHost(host);
      renderPackedView(encoder, view, invoke);
      const before = host.screenRows();
      reset();
      renderPackedView(encoder, view, invoke);
      expect(host.screenRows()).toEqual(before);
    } finally {
      host.dispose();
    }
  });

  test("retries a cache miss exactly once and hard-fails a second miss", () => {
    const encoder = createPackedViewEncoder();
    const view = View.text("retry");
    let calls = 0;
    renderPackedView(encoder, view, () => {
      calls += 1;
      if (calls === 1) throw Object.assign(new Error("ION_PACKED_CACHE_MISS"), { code: "ION_PACKED_CACHE_MISS" });
    });
    expect(calls).toBe(2);
    const broken = createPackedViewEncoder();
    expect(() => renderPackedView(broken, view, () => { throw Object.assign(new Error("ION_PACKED_CACHE_MISS"), { code: "ION_PACKED_CACHE_MISS" }); })).toThrow("cold packed retry");
    expect(isPackedCacheMiss(Object.assign(new Error(), { code: "ION_PACKED_CACHE_MISS" }))).toBe(true);
  });
});
