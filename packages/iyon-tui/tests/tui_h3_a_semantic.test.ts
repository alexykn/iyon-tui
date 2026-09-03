import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";

import { DiffHunk, DiffLine, DiffRange, TextSpan, View } from "../src/index.ts";
import type { OverflowIndicator } from "../src/api/view/view.ts";
import { Insets } from "../src/api/view/geometry.ts";
import { StyleRef, StyleSpec } from "../src/api/presentation/style.ts";
import { themeColor } from "../src/api/presentation/theme.ts";
import {
  semanticBorderFor,
  semanticColorFor,
  semanticDecorationFor,
  semanticEmptyStyle,
  semanticOverflowFor,
  semanticStyleFor,
  semanticTextSpanFor,
} from "../src/api/presentation/semantic-style.ts";
import { PersistentSeq } from "../src/composition/persistent-seq.ts";
import {
  createSemanticViewNode,
  installSemanticNode,
  peekSemanticDerivation,
  peekSemanticGridSequenceOverride,
  peekSemanticSequenceOverride,
  SEMANTIC_VIEW_KIND,
  semanticNodeOf,
  setSemanticDerivation,
  setSemanticGridSequenceOverride,
  setSemanticSequenceOverride,
  type SemanticAxisTrack,
  type SemanticBorder,
  type SemanticColor,
  type SemanticDecoration,
  type SemanticDerivation,
  type SemanticGridTrack,
  type SemanticLayoutChild,
  type SemanticOverflowIndicator,
  type SemanticSequence,
  type SemanticStyle,
  type SemanticViewNode,
  type SemanticViewNodeDraft,
} from "../src/api/view/semantic-node.ts";
import { AppHarness } from "../src/testing/index.ts";
import {
  axisSetChildForTransport,
  axisSpliceForTransport,
  gridSetCellForTransport,
} from "../src/api/view/view.ts";

const ANSI_COLORS = new Set([
  "black",
  "red",
  "green",
  "yellow",
  "blue",
  "magenta",
  "cyan",
  "gray",
  "darkGray",
  "lightRed",
  "lightGreen",
  "lightYellow",
  "lightBlue",
  "lightMagenta",
  "lightCyan",
  "white",
]);

function semanticKindName(kind: number): string {
  switch (kind) {
    case SEMANTIC_VIEW_KIND.text: return "text";
    case SEMANTIC_VIEW_KIND.diff: return "diff";
    case SEMANTIC_VIEW_KIND.spacer: return "spacer";
    case SEMANTIC_VIEW_KIND.row: return "row";
    case SEMANTIC_VIEW_KIND.column: return "column";
    case SEMANTIC_VIEW_KIND.grid: return "grid";
    case SEMANTIC_VIEW_KIND.hanging: return "hanging";
    case SEMANTIC_VIEW_KIND.container: return "container";
    case SEMANTIC_VIEW_KIND.clamp: return "clamp";
    case SEMANTIC_VIEW_KIND.contentMax: return "contentMax";
    case SEMANTIC_VIEW_KIND.component: return "component";
    case SEMANTIC_VIEW_KIND.decorated: return "decorated";
    default: throw new TypeError(`unknown semantic kind ${kind}`);
  }
}

function semanticComparable(node: SemanticViewNode): unknown {
  const base = { id: node.id, kind: semanticKindName(node.kind) };
  switch (node.kind) {
    case SEMANTIC_VIEW_KIND.text:
      return {
        ...base,
        spans: node.spans.map((span) => ({ text: span.text, style: semanticStyleComparable(span.style) })),
        wrap: node.wrap,
        align: node.align,
      };
    case SEMANTIC_VIEW_KIND.diff:
      return {
        ...base,
        hunks: node.hunks.map((hunk) => ({
          oldRange: hunk.oldRange,
          newRange: hunk.newRange,
          lines: hunk.lines,
        })),
      };
    case SEMANTIC_VIEW_KIND.spacer:
      return { ...base, rows: node.rows };
    case SEMANTIC_VIEW_KIND.row:
    case SEMANTIC_VIEW_KIND.column:
      return { ...base, children: node.children.map(semanticLayoutChildComparable), gap: node.gap };
    case SEMANTIC_VIEW_KIND.hanging:
      return { ...base, prefix: semanticComparable(node.prefix), continuation: semanticComparable(node.continuation), body: semanticComparable(node.body) };
    case SEMANTIC_VIEW_KIND.grid:
      return {
        ...base,
        columns: node.columns,
        rows: node.rows.map((row) => ({ track: row.track, cells: row.cells.map((cell) => ({
          view: semanticComparable(cell.view),
          columnSpan: cell.columnSpan,
          rowSpan: cell.rowSpan,
          horizontalAlign: cell.horizontalAlign,
          verticalAlign: cell.verticalAlign,
        })) })),
        columnGap: node.columnGap,
        rowGap: node.rowGap,
      };
    case SEMANTIC_VIEW_KIND.container:
      return { ...base, child: semanticComparable(node.child) };
    case SEMANTIC_VIEW_KIND.clamp:
      return { ...base, child: semanticComparable(node.child), maxRows: node.maxRows, overflow: semanticOverflowComparable(node.overflow) };
    case SEMANTIC_VIEW_KIND.contentMax:
      return { ...base, child: semanticComparable(node.child), maxRows: node.maxRows };
    case SEMANTIC_VIEW_KIND.component:
      return { ...base, handle: node.handleId };
    case SEMANTIC_VIEW_KIND.decorated:
      return { ...base, child: semanticComparable(node.child), decoration: semanticDecorationComparable(node.decoration) };
  }
}

function semanticStyleComparable(style: SemanticStyle | undefined): unknown {
  if (style === undefined) return undefined;
  return {
    theme: style.theme,
    foreground: style.foreground,
    background: style.background,
    attributes: style.attributes,
  };
}

function semanticLayoutChildComparable(child: SemanticLayoutChild): unknown {
  return {
    kind: child.kind,
    ...(child.kind === "fixed" ? { size: child.size } : {}),
    ...(child.kind === "flexMax" || child.kind === "contentMax" ? { maxRows: child.maxRows } : {}),
    child: semanticComparable(child.child),
  };
}

function semanticOverflowComparable(overflow: SemanticOverflowIndicator): unknown {
  if (overflow.kind === "none") return { kind: "none" };
  return {
    kind: overflow.kind,
    ...(overflow.kind === "footer" ? { prefix: overflow.prefix } : {}),
    style: semanticStyleComparable(overflow.style),
  };
}

function semanticDecorationComparable(decoration: SemanticDecoration): unknown {
  return {
    padding: decoration.padding,
    background: decoration.background,
    foreground: decoration.foreground,
    border: decoration.border,
    style: semanticStyleComparable(decoration.style),
    styleStates: decoration.styleStates,
    width: decoration.width,
    height: decoration.height,
    minWidth: decoration.minWidth,
    maxWidth: decoration.maxWidth,
    minHeight: decoration.minHeight,
    maxHeight: decoration.maxHeight,
  };
}

function semanticDerivationComparable(derivation: SemanticDerivation): unknown {
  switch (derivation.kind) {
    case "textLayout":
      return { kind: derivation.kind, base: derivation.base.id, wrap: derivation.wrap, align: derivation.align };
    case "commonScalar":
      return { kind: derivation.kind, base: derivation.base.id, changes: derivation.changes };
    case "axisSet":
      return {
        kind: derivation.kind,
        base: derivation.base.id,
        index: derivation.index,
        track: derivation.track,
        child: derivation.child.id,
      };
    case "axisSplice":
      return {
        kind: derivation.kind,
        base: derivation.base.id,
        index: derivation.index,
        removeCount: derivation.removeCount,
        inserted: derivation.inserted.map((entry) => ({ track: entry.track, child: entry.child.id })),
      };
    case "gridCell":
      return { kind: derivation.kind, base: derivation.base.id, row: derivation.row, column: derivation.column, child: derivation.child.id };
  }
}

function completeBorder() {
  return {
    glyphs: {
      top: "─",
      right: "│",
      bottom: "─",
      left: "│",
      topLeft: "┌",
      topRight: "┐",
      bottomLeft: "└",
      bottomRight: "┘",
    },
    style: "double" as const,
    edges: "topBottom" as const,
    color: themeColor("border"),
  };
}

/** Strips dynamic semantic NodeIds so samples compare against literals. */
function withoutIds(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(withoutIds);
  if (typeof value === "object" && value !== null) {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>)
        .filter(([key]) => key !== "id")
        .map(([key, entry]) => [key, withoutIds(entry)]),
    );
  }
  return value;
}

interface SampleView {
  readonly name: string;
  readonly view: View;
  readonly expected: unknown;
}

const STYLED_SPAN_STYLE = {
  theme: "body",
  foreground: { kind: "rgb", r: 1, g: 2, b: 3 },
  background: { kind: "indexed", value: 17 },
  attributes: { bold: true, underline: true },
};

function textNode(text: string, wrap = "wordThenGrapheme", align = "start"): unknown {
  return { kind: "text", spans: [{ text }], wrap, align };
}

function sampleViews(): SampleView[] {
  const style = new StyleSpec()
    .foreground({ type: "rgb", r: 1, g: 2, b: 3 })
    .background({ type: "indexed", value: 17 })
    .bold()
    .underline();
  const styled = View.styledText([
    TextSpan.plain("plain"),
    TextSpan.styled("styled", StyleRef.theme("body", style)),
  ]).noWrap().textAlign("end");
  const diff = View.diff([
    new DiffHunk(new DiffRange(0, 2), new DiffRange(0, 2), [
      DiffLine.context(1, 1, "same", "crlf"),
      DiffLine.deletion(2, "old", "none"),
      DiffLine.addition(2, "new"),
    ]),
  ]);
  const axis = View.horizontal((builder) => {
    builder
      .child(View.text("normal"))
      .fixed(2, View.spacer(1))
      .flex(View.spacer(2))
      .flexMax(3, View.spacer(3))
      .contentMax(4, View.spacer(4))
      .gap(1);
  });
  const grid = View.grid((builder) => {
    builder
      .columns([
        { kind: "content" },
        { kind: "contentMax", max: 2 },
        { kind: "fixed", size: 3 },
        { kind: "flex" },
        { kind: "flexMax", max: 5 },
      ])
      .columnGap(1)
      .rowGap(2)
      .rowWith({ kind: "contentMax", max: 6 }, (row) => {
        row.cellWith({ columnSpan: 2, rowSpan: 2, horizontalAlign: "center", verticalAlign: "bottom" }, View.text("cell"));
        row.cell(View.spacer(1));
        row.cellWith({ horizontalAlign: "end", verticalAlign: "center" }, View.text("aligned"));
      });
  });
  const decorated = View.text("decorated")
    .padding(Insets.of(1, 2, 3, 4))
    .background(themeColor("panel"))
    .foreground({ type: "named", value: "cyan" })
    .border(completeBorder())
    .style(style)
    .styleState("mode", "active")
    .fillWidth()
    .maxHeight(5);

  return [
    {
      name: "text",
      view: View.text("text").noWrap().textAlign("center"),
      expected: { kind: "text", spans: [{ text: "text" }], wrap: "noWrap", align: "center" },
    },
    {
      name: "grapheme",
      view: View.text("grapheme").wrap("grapheme").textAlign("end"),
      expected: { kind: "text", spans: [{ text: "grapheme" }], wrap: "grapheme", align: "end" },
    },
    {
      name: "styled",
      view: styled,
      expected: {
        kind: "text",
        spans: [{ text: "plain" }, { text: "styled", style: STYLED_SPAN_STYLE }],
        wrap: "noWrap",
        align: "end",
      },
    },
    {
      name: "diff",
      view: diff,
      expected: {
        kind: "diff",
        hunks: [{
          oldRange: { start: 0, count: 2 },
          newRange: { start: 0, count: 2 },
          lines: [
            { kind: "context", text: "same", termination: "terminated", oldLine: 1, newLine: 1 },
            { kind: "deletion", text: "old", termination: "unterminated", oldLine: 2 },
            { kind: "addition", text: "new", termination: "terminated", newLine: 2 },
          ],
        }],
      },
    },
    {
      name: "spacer",
      view: View.spacer(3),
      expected: { kind: "spacer", rows: 3 },
    },
    {
      name: "axis",
      view: axis,
      expected: {
        kind: "row",
        children: [
          { kind: "normal", child: textNode("normal") },
          { kind: "fixed", size: 2, child: { kind: "spacer", rows: 1 } },
          { kind: "flex", child: { kind: "spacer", rows: 2 } },
          { kind: "flexMax", maxRows: 3, child: { kind: "spacer", rows: 3 } },
          { kind: "contentMax", maxRows: 4, child: { kind: "spacer", rows: 4 } },
        ],
        gap: 1,
      },
    },
    {
      name: "column",
      view: View.vertical([View.text("column")]),
      expected: {
        kind: "column",
        children: [{ kind: "normal", child: textNode("column") }],
        gap: 0,
      },
    },
    {
      name: "hanging",
      view: View.hanging(View.text("> "), View.text("  "), View.text("body")),
      expected: {
        kind: "hanging",
        prefix: textNode("> "),
        continuation: textNode("  "),
        body: textNode("body"),
      },
    },
    {
      name: "grid",
      view: grid,
      expected: {
        kind: "grid",
        columns: [
          { kind: "content" },
          { kind: "contentMax", max: 2 },
          { kind: "fixed", size: 3 },
          { kind: "flex" },
          { kind: "flexMax", max: 5 },
        ],
        rows: [{
          track: { kind: "contentMax", max: 6 },
          cells: [
            {
              view: textNode("cell"),
              columnSpan: 2,
              rowSpan: 2,
              horizontalAlign: "center",
              verticalAlign: "bottom",
            },
            {
              view: { kind: "spacer", rows: 1 },
              columnSpan: 1,
              rowSpan: 1,
              horizontalAlign: "start",
              verticalAlign: "top",
            },
            {
              view: textNode("aligned"),
              columnSpan: 1,
              rowSpan: 1,
              horizontalAlign: "end",
              verticalAlign: "center",
            },
          ],
        }],
        columnGap: 1,
        rowGap: 2,
      },
    },
    {
      name: "container",
      view: View.text("container").container(),
      expected: { kind: "container", child: textNode("container") },
    },
    {
      name: "clamp",
      view: View.text("clamp").clampRows(2, { kind: "ellipsis", style }),
      expected: {
        kind: "clamp",
        child: textNode("clamp"),
        maxRows: 2,
        overflow: {
          kind: "ellipsis",
          style: {
            foreground: { kind: "rgb", r: 1, g: 2, b: 3 },
            background: { kind: "indexed", value: 17 },
            attributes: { bold: true, underline: true },
          },
        },
      },
    },
    {
      name: "footer",
      view: View.text("footer").clampRows(2, { kind: "footer", prefix: "more ", style }),
      expected: {
        kind: "clamp",
        child: textNode("footer"),
        maxRows: 2,
        overflow: {
          kind: "footer",
          prefix: "more ",
          style: {
            foreground: { kind: "rgb", r: 1, g: 2, b: 3 },
            background: { kind: "indexed", value: 17 },
            attributes: { bold: true, underline: true },
          },
        },
      },
    },
    {
      name: "contentMax",
      view: View.contentMax(4, View.text("content")),
      expected: { kind: "contentMax", child: textNode("content"), maxRows: 4 },
    },
    {
      name: "decorated",
      view: decorated,
      expected: {
        kind: "decorated",
        child: textNode("decorated"),
        decoration: {
          padding: { top: 1, right: 2, bottom: 3, left: 4 },
          background: { kind: "theme", key: "panel" },
          border: {
            glyphs: {
              top: "─",
              right: "│",
              bottom: "─",
              left: "│",
              topLeft: "┌",
              topRight: "┐",
              bottomLeft: "└",
              bottomRight: "┘",
            },
            style: "double",
            edges: "topBottom",
            color: { kind: "theme", key: "border" },
          },
          style: {
            foreground: { kind: "rgb", r: 1, g: 2, b: 3 },
            background: { kind: "indexed", value: 17 },
            attributes: { bold: true, underline: true },
          },
          styleStates: { mode: "active" },
          width: "fill",
          maxHeight: 5,
        },
      },
    },
  ];
}


describe("API-H3 H3-A semantic foundation", () => {
  test("semantic nodes cover every current View family and preserve all fields", async () => {
    const samples = sampleViews();
    const tui = await AppHarness.open({ width: 20, height: 4 });
    try {
      const slot = tui.createViewSlot(View.text("component-seed"));
      try {
        const kinds = new Set(samples.map(({ view }) => semanticNodeOf(view).kind));
        kinds.add(semanticNodeOf(slot.view()).kind);
        expect([...kinds].sort((a, b) => a - b)).toEqual([
          SEMANTIC_VIEW_KIND.text,
          SEMANTIC_VIEW_KIND.diff,
          SEMANTIC_VIEW_KIND.spacer,
          SEMANTIC_VIEW_KIND.row,
          SEMANTIC_VIEW_KIND.column,
          SEMANTIC_VIEW_KIND.grid,
          SEMANTIC_VIEW_KIND.hanging,
          SEMANTIC_VIEW_KIND.container,
          SEMANTIC_VIEW_KIND.clamp,
          SEMANTIC_VIEW_KIND.contentMax,
          SEMANTIC_VIEW_KIND.component,
          SEMANTIC_VIEW_KIND.decorated,
        ].sort((a, b) => a - b));

        for (const { name, view, expected } of samples) {
          expect(withoutIds(semanticComparable(semanticNodeOf(view))), name).toEqual(expected);
        }
        expect(withoutIds(semanticComparable(semanticNodeOf(slot.view())))).toEqual({
          kind: "component",
          handle: slot.id,
        });
      } finally {
        slot.dispose();
      }
    } finally {
      tui.close();
    }
  });

  test("semantic conversion preserves shared child identity and excludes native metadata", () => {
    const child = View.text("shared");
    const view = View.horizontal([child, child]);
    const semantic = semanticNodeOf(view);
    if (semantic.kind !== SEMANTIC_VIEW_KIND.row) throw new Error("expected semantic row");

    expect(semantic.children[0]!.child).toBe(semantic.children[1]!.child);
    expect(semantic.children[0]!.child.id).toBe(semanticNodeOf(child).id);
    expect(semanticNodeOf(view)).toBe(semantic);
    expect(Object.isFrozen(semantic)).toBe(true);
    expect(Object.isFrozen(semantic.children)).toBe(true);
    expect("schema" in semantic).toBe(false);
    expect("handle" in semantic).toBe(false);

    const associated = View.text("associated");
    const associatedNode = createSemanticViewNode(900_002, {
      kind: SEMANTIC_VIEW_KIND.text,
      spans: Object.freeze([{ text: "associated" }]),
      wrap: "wordThenGrapheme",
      align: "start",
    });
    installSemanticNode(associated, associatedNode);
    expect(semanticNodeOf(associated)).toBe(associatedNode);
    expect(() => semanticNodeOf({} as View)).toThrow(/semantic value/);
  });

  test("semantic normalizers produce backend-neutral records for every public presentation form", () => {
    const colors = [
      themeColor("accent"),
      { type: "named", value: "magenta" as const },
      { type: "indexed", value: 200 },
      { type: "rgb", r: 12, g: 34, b: 56 },
    ] as const;
    expect(colors.map((color) => semanticColorFor(color))).toEqual([
      { kind: "theme", key: "accent" },
      { kind: "named", value: "magenta" },
      { kind: "indexed", value: 200 },
      { kind: "rgb", r: 12, g: 34, b: 56 },
    ]);

    const styles = [
      new StyleSpec(),
      new StyleSpec().foreground(colors[0]!).background(colors[1]!).bold().italic(),
      StyleRef.theme("named", new StyleSpec().background(colors[2]!)),
      { attributes: { reversed: false, strikethrough: true } },
    ] as const;
    expect(styles.map((style) => semanticStyleFor(style))).toEqual([
      { attributes: {} },
      {
        foreground: { kind: "theme", key: "accent" },
        background: { kind: "named", value: "magenta" },
        attributes: { bold: true, italic: true },
      },
      {
        theme: "named",
        background: { kind: "indexed", value: 200 },
        attributes: {},
      },
      { attributes: { reversed: false, strikethrough: true } },
    ]);

    const border = completeBorder();
    expect(semanticBorderFor(border)).toEqual({
      glyphs: {
        top: "─",
        right: "│",
        bottom: "─",
        left: "│",
        topLeft: "┌",
        topRight: "┐",
        bottomLeft: "└",
        bottomRight: "┘",
      },
      style: "double",
      edges: "topBottom",
      color: { kind: "theme", key: "border" },
    });

    const span = TextSpan.styled("span", StyleRef.theme("span", new StyleSpec().foreground(colors[3]!)));
    expect(semanticTextSpanFor(span)).toEqual({
      text: "span",
      style: {
        theme: "span",
        foreground: { kind: "rgb", r: 12, g: 34, b: 56 },
        attributes: {},
      },
    });

    const overflowValues: readonly OverflowIndicator[] = [
      { kind: "none" },
      { kind: "ellipsis", style: styles[1]! },
      { kind: "footer", prefix: "more ", style: styles[2]! },
    ];
    expect(overflowValues.map((overflow) => semanticOverflowFor(overflow))).toEqual([
      { kind: "none" },
      {
        kind: "ellipsis",
        style: {
          foreground: { kind: "theme", key: "accent" },
          background: { kind: "named", value: "magenta" },
          attributes: { bold: true, italic: true },
        },
      },
      {
        kind: "footer",
        prefix: "more ",
        style: {
          theme: "named",
          background: { kind: "indexed", value: 200 },
          attributes: {},
        },
      },
    ]);

    const decoration = semanticDecorationFor({
      padding: Insets.of(1, 2, 3, 4),
      background: colors[0],
      foreground: colors[1],
      border,
      style: styles[2],
      styleStates: { mode: "active" },
      width: "fill",
      height: "fit",
      minWidth: 1,
      maxWidth: 20,
      minHeight: 2,
      maxHeight: 30,
    });
    expect(decoration).toEqual({
      padding: { top: 1, right: 2, bottom: 3, left: 4 },
      background: { kind: "theme", key: "accent" },
      foreground: { kind: "named", value: "magenta" },
      border: {
        glyphs: {
          top: "─",
          right: "│",
          bottom: "─",
          left: "│",
          topLeft: "┌",
          topRight: "┐",
          bottomLeft: "└",
          bottomRight: "┘",
        },
        style: "double",
        edges: "topBottom",
        color: { kind: "theme", key: "border" },
      },
      style: {
        theme: "named",
        background: { kind: "indexed", value: 200 },
        attributes: {},
      },
      styleStates: { mode: "active" },
      width: "fill",
      height: "fit",
      minWidth: 1,
      maxWidth: 20,
      minHeight: 2,
      maxHeight: 30,
    });

    expect(() => semanticColorFor({ type: "indexed", value: 256 })).toThrow(/indexed ANSI color/);
    expect(() => semanticStyleFor({ attributes: { invalid: true } as never })).toThrow(/unknown text attribute/);
    expect(() => semanticBorderFor({ glyphs: { top: "─" } } as never)).toThrow(/border glyph/);
    expect(() => semanticDecorationFor({ padding: { top: 65_536, right: 0, bottom: 0, left: 0 } })).toThrow(/inset top/);
  });

  test("semantic normalizers snapshot caller values and freeze owned records", () => {
    const mutableRgb = { type: "rgb" as const, r: 10, g: 20, b: 30 };
    const attributes = { bold: true };
    const styleValue = { foreground: mutableRgb, attributes };
    const style = semanticStyleFor(styleValue);
    mutableRgb.r = 99;
    attributes.bold = false;
    expect(style.foreground).toEqual({ kind: "rgb", r: 10, g: 20, b: 30 });
    expect(style.attributes).toEqual({ bold: true });
    expect(Object.isFrozen(style)).toBe(true);
    expect(Object.isFrozen(style.foreground)).toBe(true);
    expect(Object.isFrozen(style.attributes)).toBe(true);

    const glyphs = {
      top: "─", right: "│", bottom: "─", left: "│",
      topLeft: "┌", topRight: "┐", bottomLeft: "└", bottomRight: "┘",
    };
    const states = { mode: "active" };
    const border = semanticBorderFor({ glyphs, color: mutableRgb });
    const decoration = semanticDecorationFor({ border: { glyphs, color: mutableRgb }, styleStates: states });
    glyphs.top = "x";
    states.mode = "changed";
    mutableRgb.g = 88;
    expect(border.glyphs?.top).toBe("─");
    expect(border.color).toEqual({ kind: "rgb", r: 99, g: 20, b: 30 });
    expect(decoration.border?.glyphs?.top).toBe("─");
    expect(decoration.styleStates).toEqual({ mode: "active" });
    expect(Object.isFrozen(border)).toBe(true);
    expect(Object.isFrozen(border.glyphs)).toBe(true);
    expect(Object.isFrozen(decoration)).toBe(true);
    expect(Object.isFrozen(decoration.border)).toBe(true);
    expect(Object.isFrozen(decoration.style)).toBe(true);
    expect(semanticEmptyStyle()).toEqual({ attributes: {} });
  });

  test("semantic derivations retain exact semantic facts and weak sidecars", () => {
    const textBase = createSemanticViewNode(1, {
      kind: SEMANTIC_VIEW_KIND.text,
      spans: Object.freeze([{ text: "base" }]),
      wrap: "wordThenGrapheme",
      align: "start",
    });
    const textDerived = createSemanticViewNode(2, {
      kind: SEMANTIC_VIEW_KIND.text,
      spans: Object.freeze([{ text: "base" }]),
      wrap: "noWrap",
      align: "end",
    });
    const decorated = createSemanticViewNode(3, {
      kind: SEMANTIC_VIEW_KIND.decorated,
      child: textDerived,
      decoration: semanticDecorationFor({ padding: 1 }),
    });
    const axis = createSemanticViewNode(4, {
      kind: SEMANTIC_VIEW_KIND.row,
      children: Object.freeze([]),
      gap: 0,
    });
    const grid = createSemanticViewNode(5, {
      kind: SEMANTIC_VIEW_KIND.grid,
      columns: Object.freeze([{ kind: "content" }]),
      rows: Object.freeze([]),
      columnGap: 0,
      rowGap: 0,
    });
    const child = createSemanticViewNode(6, {
      kind: SEMANTIC_VIEW_KIND.spacer,
      rows: 1,
    });
    const track: SemanticAxisTrack = { kind: "fixed", size: 2 };
    const derivations: SemanticDerivation[] = [
      { kind: "textLayout", base: textBase, wrap: "noWrap", align: "end" },
      { kind: "commonScalar", base: textBase, changes: { padding: { top: 1, right: 1, bottom: 1, left: 1 }, width: "fill" } },
      { kind: "axisSet", base: axis, index: 0, track, child },
      { kind: "axisSplice", base: axis, index: 0, removeCount: 1, inserted: [{ track: { kind: "normal" }, child }] },
      { kind: "gridCell", base: grid, row: 0, column: 0, child },
    ];
    const targets = [textDerived, decorated, axis, createSemanticViewNode(7, { ...axis }), createSemanticViewNode(8, { ...grid })];
    for (let index = 0; index < derivations.length; index += 1) {
      setSemanticDerivation(targets[index]!, derivations[index]!);
      expect(peekSemanticDerivation(targets[index]!)).toBe(derivations[index]);
      expect("trackWord" in derivations[index]!).toBe(false);
      expect("schema" in derivations[index]!).toBe(false);
    }
    expect(new Set(derivations.map((derivation) => derivation.kind))).toEqual(new Set([
      "textLayout", "commonScalar", "axisSet", "axisSplice", "gridCell",
    ]));
  });

  test("the semantic derivation oracle preserves every current retained fast-path fact", () => {
    const text = View.text("text").noWrap();
    const scalar = View.text("scalar").padding(1);
    const axisBase = View.horizontal([View.text("a"), View.text("b")]);
    const axisSet = axisSetChildForTransport(axisBase, 0, View.text("replacement"), { kind: "fixed", size: 4 });
    const axisSplice = axisSpliceForTransport(axisBase, 1, 1, [{ view: View.text("inserted"), track: { kind: "flexMax", maxRows: 3 } }]);
    const gridBase = View.grid([View.text("cell")]);
    const gridCell = gridSetCellForTransport(gridBase, 0, 0, View.text("new cell"));
    const semanticDerivations = [
      peekSemanticDerivation(semanticNodeOf(text)),
      peekSemanticDerivation(semanticNodeOf(scalar)),
      peekSemanticDerivation(semanticNodeOf(axisSet)),
      peekSemanticDerivation(semanticNodeOf(axisSplice)),
      peekSemanticDerivation(semanticNodeOf(gridCell)),
    ];
    if (semanticDerivations.some((derivation) => derivation === undefined)) {
      throw new Error("retained derivation fixture did not produce every derivation family");
    }
    const textDerivation = semanticDerivations[0] as Extract<SemanticDerivation, { kind: "textLayout" }>;
    const scalarDerivation = semanticDerivations[1] as Extract<SemanticDerivation, { kind: "commonScalar" }>;
    const axisSetDerivation = semanticDerivations[2] as Extract<SemanticDerivation, { kind: "axisSet" }>;
    const axisSpliceDerivation = semanticDerivations[3] as Extract<SemanticDerivation, { kind: "axisSplice" }>;
    const gridCellDerivation = semanticDerivations[4] as Extract<SemanticDerivation, { kind: "gridCell" }>;
    expect(semanticDerivationComparable(textDerivation)).toEqual({
      kind: "textLayout",
      base: textDerivation.base.id,
      wrap: "noWrap",
      align: "start",
    });
    expect(semanticDerivationComparable(scalarDerivation)).toEqual({
      kind: "commonScalar",
      base: scalarDerivation.base.id,
      changes: { padding: { top: 1, right: 1, bottom: 1, left: 1 } },
    });
    expect(semanticDerivationComparable(axisSetDerivation)).toEqual({
      kind: "axisSet",
      base: axisSetDerivation.base.id,
      index: 0,
      track: { kind: "fixed", size: 4 },
      child: axisSetDerivation.child.id,
    });
    expect(semanticDerivationComparable(axisSpliceDerivation)).toEqual({
      kind: "axisSplice",
      base: axisSpliceDerivation.base.id,
      index: 1,
      removeCount: 1,
      inserted: [{ track: { kind: "flexMax", maxRows: 3 }, child: axisSpliceDerivation.inserted[0]!.child.id }],
    });
    expect(semanticDerivationComparable(gridCellDerivation)).toEqual({
      kind: "gridCell",
      base: gridCellDerivation.base.id,
      row: 0,
      column: 0,
      child: gridCellDerivation.child.id,
    });
  });

  test("PersistentSeq satisfies the read-only semantic sequence contract without flattening at the boundary", () => {
    const sequence: SemanticSequence<number> = PersistentSeq.from([1, 2, 3]);
    expect(sequence.length).toBe(3);
    expect(sequence.get(1)).toBe(2);
    expect([...sequence.values()]).toEqual([1, 2, 3]);

    const child = createSemanticViewNode(10, { kind: SEMANTIC_VIEW_KIND.spacer, rows: 1 });
    const axis = createSemanticViewNode(11, { kind: SEMANTIC_VIEW_KIND.row, children: [], gap: 0 });
    const layout: SemanticLayoutChild = { kind: "normal", child };
    const axisSequence: SemanticSequence<SemanticLayoutChild> = PersistentSeq.from([layout]);
    setSemanticSequenceOverride(axis, { baseNode: axis, sequence: axisSequence });
    expect(peekSemanticSequenceOverride(axis)?.sequence.get(0)).toBe(layout);

    const grid = createSemanticViewNode(12, { kind: SEMANTIC_VIEW_KIND.grid, columns: [], rows: [], columnGap: 0, rowGap: 0 });
    setSemanticGridSequenceOverride(grid, {
      baseNode: grid,
      sequence: PersistentSeq.from([]),
      rowOffsets: [0],
      rowTracks: [],
      cellIndices: [],
    });
    expect(peekSemanticGridSequenceOverride(grid)?.rowOffsets).toEqual([0]);
  });

  test("semantic foundation source has no structural bridge or native-retention dependency", () => {
    const semanticNodeSource = readFileSync(new URL("../src/api/view/semantic-node.ts", import.meta.url), "utf8");
    const semanticStyleSource = readFileSync(new URL("../src/api/presentation/semantic-style.ts", import.meta.url), "utf8");
    const forbidden = /\b(?:BridgeViewNode|BRIDGE_VIEW_KIND|VIEW_BRIDGE_SCHEMA_VERSION|NativeRef|trackWord|pathRef|viewRefForNodeId)\b/u;
    expect(semanticNodeSource).not.toMatch(forbidden);
    expect(semanticStyleSource).not.toMatch(forbidden);
    expect(semanticNodeSource).not.toContain("../transport/");
    expect(semanticStyleSource).not.toContain("../transport/");
  });
});
