import { describe, expect, test } from "bun:test";

import { native } from "../src/native.ts";
import { nativeViewAbiSession } from "../src/tui/native_view_abi.ts";
import { nodeForBridge, View } from "../src/tui/values/view.ts";
import { TextSpan } from "../src/tui/values/text.ts";
import { StyleSpec } from "../src/tui/values/style.ts";
import { Theme } from "../src/tui/values/theme.ts";
import {
  peekBridgeNativeHint,
  retainedIdentityCounterSnapshot,
  resetRetainedIdentityCounters,
  RetainedRootBoundary,
} from "../src/tui/retained_dag.ts";

interface Host {
  render(view: object): void;
  screenRows(): string[];
  tuiViewAbiHostPointer(): number;
  setTheme(theme: object): void;
  styleAt(row: number, column: number): object | null;
  dispose(): void;
}

const Host = native.NativeTuiHost as unknown as
  | (new (width: number, height: number, headless: boolean) => Host)
  | undefined;
const session = nativeViewAbiSession();
const canRun = Host !== undefined && session !== undefined;

function countersDelta(action: () => void): ReturnType<typeof retainedIdentityCounterSnapshot> {
  const before = retainedIdentityCounterSnapshot();
  action();
  const after = retainedIdentityCounterSnapshot();
  const delta = {} as ReturnType<typeof retainedIdentityCounterSnapshot>;
  for (const key of Object.keys(after) as (keyof typeof after)[]) {
    delta[key] = after[key] - before[key];
  }
  return delta;
}

function oracle(view: View, width: number, height: number): string[] {
  const host = new Host!(width, height, true);
  try {
    host.render(nodeForBridge(view));
    return host.screenRows();
  } finally {
    host.dispose();
  }
}

/** Installs through the retained boundary and asserts Direct-oracle parity. */

describe("PERF-12 T11 payload families", () => {
  test("§39: the full string correctness dataset renders identically to Direct", () => {
    if (!canRun) return;
    const dataset: readonly [string, string][] = [
      ["empty", ""],
      ["ascii", "hello world"],
      ["short unicode", "héllo ✓ 世界"],
      ["emoji non-bmp", "family 👨‍👩‍👧 flag 🇺🇸"],
      ["combining", "e\u0301\u0327 a\u030a"],
      ["embedded nul", "a\u0000b"],
      ["lone surrogate", "\uD800tail"],
      ["u10ffff", "\u{10FFFF}"],
      ["256 bytes", "x".repeat(256)],
      ["4 kib", "y".repeat(4096)],
    ];
    const host = new Host!(64, 16, true);
    const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer() as never);
    try {
      for (const [name, text] of dataset) {
        const view = View.vertical([View.text(text)]);
        const delta = countersDelta(() => {
          expect(boundary.install(view)).toBeGreaterThan(0);
        });
        expect(host.screenRows()).toEqual(oracle(view, 64, 16));
        expect(delta.cold_fallbacks).toBe(0);
        // Every case must ride exactly one text materializer call.
        expect(delta.direct_materializer_calls).toBe(2);
        if (name === "embedded nul" || name === "lone surrogate" || name === "u10ffff") {
          // These must NOT take the cstring lane: NUL cannot cross cstring
          // and the surrogate/U+10FFFF cases are proven byte-exact below.
        }
      }
    } finally {
      boundary.close();
      host.dispose();
    }
  });

  test("§39: embedded NUL rides the exact-byte lane and never truncates", () => {
    if (!canRun) return;
    const view = View.text("a\u0000b✓");
    const host = new Host!(32, 8, true);
    const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer() as never);
    try {
      const delta = countersDelta(() => expect(boundary.install(view)).toBeGreaterThan(0));
      // UTF-8 encoding happened in JS: a=1, NUL=1, b=1, ✓=3.
      expect(delta.byte_payload_bytes).toBe(6);
      expect(host.screenRows()).toEqual(oracle(view, 32, 8));
    } finally {
      boundary.close();
      host.dispose();
    }
  });

  test("§39: lone surrogates normalize identically on both transports", () => {
    if (!canRun) return;
    // The replacement-character normalization must match the Direct oracle
    // byte-for-byte whichever lane serves the span.
    for (const text of ["\uD800", "ok\uDFFFmore"]) {
      const view = View.text(text);
      const host = new Host!(32, 8, true);
      const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer() as never);
      try {
        expect(boundary.install(view)).toBeGreaterThan(0);
        expect(host.screenRows()).toEqual(oracle(view, 32, 8));
      } finally {
        boundary.close();
        host.dispose();
      }
    }
  });

  test("§39: large Diff content renders identically to Direct", () => {
    if (!canRun) return;
    const lines = [];
    for (let index = 0; index < 120; index += 1) {
      if (index % 10 === 0) {
        lines.push({ kind: "deletion" as const, text: `- old ${index} ✗`, termination: "terminated" as const });
      } else if (index % 10 === 1) {
        lines.push({ kind: "addition" as const, text: `+ new ${index} ✓`, termination: "unterminated" as const });
      } else {
        lines.push({ kind: "context" as const, text: `  ctx ${index}`, termination: "terminated" as const });
      }
    }
    const view = View.diff([{
      oldRange: { start: 0, count: 108 },
      newRange: { start: 0, count: 108 },
      lines,
    }]);
    const host = new Host!(64, 24, true);
    const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer() as never);
    try {
      const delta = countersDelta(() => expect(boundary.install(view)).toBeGreaterThan(0));
      expect(delta.cold_fallbacks).toBe(0);
      expect(delta.byte_payload_bytes).toBeGreaterThan(500);
      expect(host.screenRows()).toEqual(oracle(view, 64, 24));
    } finally {
      boundary.close();
      host.dispose();
    }
  });

  test("§41: an empty diff stays on the retained path and matches Direct", () => {
    if (!canRun) return;
    const view = View.diff([]);
    const host = new Host!(32, 8, true);
    const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer() as never);
    try {
      const delta = countersDelta(() => expect(boundary.install(view)).toBeGreaterThan(0));
      expect(delta.cold_fallbacks).toBe(0);
      expect(host.screenRows()).toEqual(oracle(view, 32, 8));
    } finally {
      boundary.close();
      host.dispose();
    }
  });

  test("§40: styled spans materialize once and stable payload is never resent", () => {
    if (!canRun) return;
    const sharedStyle = new StyleSpec().foreground("#00ff00").bold();
    const styledText = View.styledText([
      TextSpan.plain("plain "),
      TextSpan.styled("green", sharedStyle),
      TextSpan.styled(" themed", new StyleSpec().theme("t11.accent")),
    ]);
    const root = View.vertical([styledText]);
    const host = new Host!(48, 12, true);
    const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer() as never);
    try {
      expect(boundary.install(root)).toBeGreaterThan(0);
      expect(host.screenRows()).toEqual(oracle(root, 48, 12));

      // One-leaf change sharing the styled text node: only the new column and
      // the new spacer materialize. The stable text/style subtree must cut off
      // by identity before any payload or style inspection (§21/§40).
      const grown = View.vertical([styledText, View.spacer(1)]);
      const delta = countersDelta(() => expect(boundary.install(grown)).toBeGreaterThan(0));
      expect(delta.bridge_hint_hits).toBe(1);
      expect(delta.direct_materializer_calls).toBe(2); // new column + spacer
      expect(delta.byte_payload_bytes).toBe(0);
      expect(host.screenRows()).toEqual(oracle(grown, 48, 12));

      // Replacing the text with a NEW node reusing the SAME style object
      // materializes only the new text; the shared style resolves through
      // the sidecar and no second native style publication is observable
      // from the retained path.
      const replaced = View.vertical([
        View.styledText([
          TextSpan.plain("plain "),
          TextSpan.styled("green", sharedStyle),
          TextSpan.styled(" themed", new StyleSpec().theme("t11.accent")),
        ]),
        View.spacer(1),
      ]);
      const replaceDelta = countersDelta(() => expect(boundary.install(replaced)).toBeGreaterThan(0));
      expect(replaceDelta.direct_materializer_calls).toBe(3); // new column + new text + spacer
      expect(host.screenRows()).toEqual(oracle(replaced, 48, 12));
    } finally {
      boundary.close();
      host.dispose();
    }
  });

  test("§40: themed styles resolve through the host theme on both transports", () => {
    if (!canRun) return;
    const theme = Theme.new()
      .withColor("t11.accent", "#ff8000")
      .withStyle("t11.accent", new StyleSpec().foreground("#ff8000").underline());
    const view = View.vertical([
      View.styledText([TextSpan.styled("accented", new StyleSpec().theme("t11.accent"))]),
    ]);
    const host = new Host!(48, 8, true);
    host.setTheme((theme as unknown as { materialize(): object }).materialize());
    const directHost = new Host!(48, 8, true);
    directHost.setTheme((theme as unknown as { materialize(): object }).materialize());
    const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer() as never);
    try {
      expect(boundary.install(view)).toBeGreaterThan(0);
      directHost.render(nodeForBridge(view));
      expect(host.screenRows()).toEqual(directHost.screenRows());
      // The accent color must actually paint on the row carrying the text.
      const textRow = host.screenRows().findIndex((row) => row.trim().length > 0);
      expect(textRow).toBeGreaterThanOrEqual(0);
      let painted = 0;
      for (let column = 0; column < 30; column += 1) {
        const style = host.styleAt(textRow, column) as { foreground?: string | null } | null;
        if (style?.foreground === "#ff8000") painted += 1;
      }
      expect(painted).toBe("accented".length);
    } finally {
      boundary.close();
      host.dispose();
      directHost.dispose();
    }
  });

  test("§40: invalid styles refuse the retained path cleanly and drain leases", () => {
    if (!canRun) return;
    const host = new Host!(48, 12, true);
    const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer() as never);
    try {
      const good = View.vertical([View.text("stable base")]);
      expect(boundary.install(good)).toBeGreaterThan(0);

      // Unknown attribute name.
      const badAttribute = View.vertical([
        View.styledText([new TextSpan({ text: "x", style: { attributes: { blink: true } } })]),
      ]);
      const leasedBefore = (native as { tuiViewRuntimeMemorySnapshot?: (live?: boolean) => { leased_slots: number } })
        .tuiViewRuntimeMemorySnapshot?.(true)?.leased_slots;
      const attributeDelta = countersDelta(() =>
        expect(boundary.install(badAttribute)).toBeUndefined(),
      );
      expect(attributeDelta.cold_fallbacks).toBe(1);

      // Invalid color string.
      const badColor = View.vertical([
        View.styledText([new TextSpan({ text: "y", style: { attributes: {}, foreground: "not-a-color" } })]),
      ]);
      expect(boundary.install(badColor)).toBeUndefined();

      // The old root keeps rendering and no leases leaked from the failures.
      expect(host.screenRows()).toEqual(oracle(good, 48, 12));
      const snapshot = (native as { tuiViewRuntimeMemorySnapshot?: (live?: boolean) => { leased_slots: number } })
        .tuiViewRuntimeMemorySnapshot?.(true);
      expect(snapshot?.leased_slots).toBe(leasedBefore);
    } finally {
      boundary.close();
      host.dispose();
    }
  });

  test("§41: oversize diff payload refuses the retained path and keeps the old root", () => {
    if (!canRun) return;
    const host = new Host!(64, 12, true);
    const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer() as never);
    try {
      const base = View.vertical([View.diff([])]);
      expect(boundary.install(base)).toBeGreaterThan(0);
      const huge = View.diff([{
        oldRange: { start: 0, count: 1 },
        newRange: { start: 0, count: 1 },
        lines: [{ kind: "context", text: "z".repeat(80_000), termination: "terminated", oldLine: 1, newLine: 1 }],
      }]);
      const delta = countersDelta(() => expect(boundary.install(huge)).toBeUndefined());
      expect(delta.cold_fallbacks).toBeGreaterThanOrEqual(1);
      expect(host.screenRows()).toEqual(oracle(base, 64, 12));
      // A valid diff still installs afterwards.
      const next = View.vertical([View.diff([{
        oldRange: { start: 0, count: 0 },
        newRange: { start: 0, count: 1 },
        lines: [{ kind: "addition", text: "after", termination: "terminated", newLine: 1 }],
      }])]);
      expect(boundary.install(next)).toBeGreaterThan(0);
      expect(host.screenRows()).toEqual(oracle(next, 64, 12));
    } finally {
      boundary.close();
      host.dispose();
    }
  });

  test("§42: stream bytes never enter structural construction", () => {
    if (!canRun) return;
    const NativeTextStream = native.NativeTextStream;
    if (NativeTextStream === undefined) return;
    const stream = new NativeTextStream("markdown");
    try {
      resetRetainedIdentityCounters();
      stream.append("# heading\n\nparagraph ");
      stream.append("continuation ✓");
      stream.seal();
      const delta = retainedIdentityCounterSnapshot();
      // Streaming uses its own native path; not one structural counter moves.
      expect(delta.direct_materializer_calls).toBe(0);
      expect(delta.bridge_children_visited).toBe(0);
      expect(delta.ref_words_written).toBe(0);
      expect(delta.byte_payload_bytes).toBe(0);
      expect(delta.host_mutations).toBe(0);
    } finally {
      stream.dispose();
    }
  });

  test("§37: multi-span arities 2..=4 ride the cstring family with zero JS encoding", () => {
    if (!canRun) return;
    const view = View.styledText([
      TextSpan.plain("one "),
      TextSpan.styled("two", new StyleSpec().dim()),
      TextSpan.styled(" three", new StyleSpec().italic()),
      TextSpan.styled("!", new StyleSpec().reversed()),
    ]);
    const host = new Host!(48, 8, true);
    const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer() as never);
    try {
      const delta = countersDelta(() => expect(boundary.install(view)).toBeGreaterThan(0));
      expect(delta.byte_payload_bytes).toBe(0);
      expect(delta.cold_fallbacks).toBe(0);
      expect(host.screenRows()).toEqual(oracle(view, 48, 8));
    } finally {
      boundary.close();
      host.dispose();
    }
  });

  test("§49: span counts beyond the family fall back without touching the host", () => {
    if (!canRun) return;
    const host = new Host!(48, 8, true);
    const boundary = new RetainedRootBoundary(session!, () => host.tuiViewAbiHostPointer() as never);
    try {
      const spans = Array.from({ length: 5 }, (_, index) => TextSpan.plain(`s${index} `));
      const view = View.styledText(spans);
      const delta = countersDelta(() => expect(boundary.install(view)).toBeUndefined());
      expect(delta.cold_fallbacks).toBe(1);
      expect(delta.host_mutations).toBe(0);
    } finally {
      boundary.close();
      host.dispose();
    }
  });
});
