import { DiffHunk, DiffLine, DiffRange, DiffRenderer, TextSpan } from "../src/tui/index.ts";
import { Style } from "../src/tui/values/style.ts";
import type { BorderNode, ColorNode, DiffHunkNode, GridTrackNode } from "../src/tui/ir.ts";
import { replaceAxisChildForPackedTransport, spliceAxisChildrenForPackedTransport, View } from "../src/tui/values/view.ts";
import { Perf7v2View } from "./perf7v2_direct/view.ts";

export type ComparisonMode = "COLD" | "FIRST_USE" | "IDENTICAL_IDENTITY" | "SHARED_PATH" | "SHARED_DEEP" | "LARGE_SHARED_SUBTREE_CUTOFF" | "REBUILT_EQUIVALENT" | "TEXT_METADATA_PATCH" | "DECORATION_PATCH" | "WIDE_PARENT_ONE_EDIT" | "WIDE_PARENT_INSERT" | "WIDE_PARENT_REMOVE";
export type ComparisonWorkload = "plain_text_column" | "styled_span_heavy" | "row_heavy" | "column_track_heavy" | "grid_heavy" | "decoration_heavy" | "diff_heavy" | "component_heavy" | "mixed_realistic" | "long_text_wrap_only" | "long_text_one_span_edit" | "large_diff_one_hunk_edit" | "large_decoration_only_change";
export type ComparisonCase = { readonly workload: ComparisonWorkload; readonly size: number; readonly mode: ComparisonMode; readonly label: string };
export type ComparisonPair<T> = { readonly base: T; readonly next: T; readonly cold: boolean };
export type PreparedComparisonCase<T> = { readonly base: T | undefined; readonly cold: boolean; readonly next: (index: number) => T };
export type TraceCategory = "stream_append" | "no_view_change" | "view_replace" | "layout_change" | "component_update" | "history_update" | "structural_update";

type Layout<T> = { child(view: T): void; fixed(size: number, view: T): void; flex(view: T): void; flexMax(max: number, view: T): void; contentMax(max: number, view: T): void; gap(value: number): void };
type GridRow<T> = { cell(view: T): void; cellWith(spec: { readonly columnSpan?: number; readonly rowSpan?: number; readonly horizontalAlign?: "start" | "center" | "end"; readonly verticalAlign?: "top" | "center" | "bottom" }, view: T): void };
type Grid<T> = { columns(value: readonly GridTrackNode[]): void; columnGap(value: number): void; rowGap(value: number): void; row(build: (row: GridRow<T>) => void): void; rowWith(track: GridTrackNode, build: (row: GridRow<T>) => void): void };
type Overflow = { readonly kind: "none" } | { readonly kind: "ellipsis"; readonly style: ReturnType<typeof Style.new> } | { readonly kind: "footer"; readonly prefix: string; readonly style: ReturnType<typeof Style.new> };
type Factory<T> = {
  text(value: string): T; styledText(spans: readonly TextSpan[]): T; spacer(rows: number): T;
  vertical(build: (builder: Layout<T>) => void): T; horizontal(build: (builder: Layout<T>) => void): T;
  hanging(prefix: T, continuation: T, body: T): T; grid(build: (builder: Grid<T>) => void): T; diff(size?: number): T; component(id: number): T;
  contentMax(max: number, child: T): T; clampRows(max: number, child: T, overflow: Overflow): T; container(child: T): T;
  padding(value: number, child: T): T; background(color: ColorNode, child: T): T; foreground(color: ColorNode, child: T): T;
  border(child: T, border?: BorderNode): T; style(child: T, style?: ReturnType<typeof Style.new>): T; styleState(child: T, key?: string, value?: string): T;
  minMax(child: T): T; maxWidth(child: T): T; noWrap(child: T): T; textAlign(child: T, align?: "start" | "center" | "end"): T; wrap(child: T, mode: "wordThenGrapheme" | "grapheme" | "noWrap"): T;
  fitWidth(child: T): T; fillWidth(child: T): T; fitHeight(child: T): T; fillHeight(child: T): T;
};

let componentId = 42;
export function setComparisonComponentId(id: number): void { componentId = id; }
const tracks: readonly GridTrackNode[] = [{ kind: "content" }, { kind: "contentMax", max: 12 }, { kind: "fixed", size: 8 }, { kind: "flex" }, { kind: "flexMax", max: 4 }];

function diffHunks(size: number): DiffHunk[] {
  const count = Math.max(1, Math.ceil(size / 50));
  return Array.from({ length: count }, (_, index) => {
    const start = index * 4;
    return new DiffHunk(
      new DiffRange(start, 2),
      new DiffRange(start, 2),
      [DiffLine.context(start + 1, start + 1, `context-${index}`), DiffLine.deletion(start + 2, `old-${index}`), DiffLine.addition(start + 2, `new-${index}`)],
    );
  });
}

function perfDiffHunks(size: number): DiffHunkNode[] {
  const count = Math.max(1, Math.ceil(size / 50));
  return Array.from({ length: count }, (_, index) => {
    const start = index * 4;
    return {
      oldRange: { start, count: 2 },
      newRange: { start, count: 2 },
      lines: [
        { kind: "context", text: `context-${index}`, termination: "terminated" },
        { kind: "deletion", text: `old-${index}`, termination: "terminated" },
        { kind: "addition", text: `new-${index}`, termination: "terminated" },
      ],
    };
  });
}

const customBorder: BorderNode = {
  style: "double",
  edges: "all",
  color: "magenta",
  glyphs: { top: "═", right: "║", bottom: "═", left: "║", topLeft: "╔", topRight: "╗", bottomLeft: "╚", bottomRight: "╝" },
};

function currentFactory(): Factory<View> {
  const layout = (build: (builder: Layout<View>) => void, horizontal: boolean): View => {
    const adapt = (builder: { child(v: View): void; fixed(n: number, v: View): void; flex(v: View): void; flexMax(n: number, v: View): void; contentMax(n: number, v: View): void; gap(n: number): void }) => build({ child: (v) => builder.child(v), fixed: (n, v) => builder.fixed(n, v), flex: (v) => builder.flex(v), flexMax: (n, v) => builder.flexMax(n, v), contentMax: (n, v) => builder.contentMax(n, v), gap: (n) => builder.gap(n) });
    return horizontal ? View.horizontal((builder) => adapt(builder)) : View.vertical((builder) => adapt(builder));
  };
  return {
    text: View.text, styledText: View.styledText, spacer: View.spacer, vertical: (b) => layout(b, false), horizontal: (b) => layout(b, true),
    hanging: View.hanging, grid: (build) => View.grid((grid) => build({ columns: grid.columns.bind(grid), columnGap: grid.columnGap.bind(grid), rowGap: grid.rowGap.bind(grid), row: (b) => grid.row((row) => b({ cell: row.cell.bind(row), cellWith: row.cellWith.bind(row) })), rowWith: (track, b) => grid.rowWith(track, (row) => b({ cell: row.cell.bind(row), cellWith: row.cellWith.bind(row) })) })),
    diff: (size = 3) => new DiffRenderer().render(diffHunks(size)),
    component: (id) => View.component({ id: id as never }), contentMax: View.contentMax, clampRows: (max, child, overflow) => child.clampRows(max, overflow as never), container: (child) => child.container(),
    padding: (n, child) => child.padding(n), background: (c, child) => child.background(c), foreground: (c, child) => child.foreground(c), border: (child, border = customBorder) => child.border(border), style: (child, style = Style.new().bold().dim().italic().underline().reversed().strikethrough()) => child.style(style), styleState: (child, key = "phase", value = "active") => child.styleState(key, value), minMax: (child) => child.minWidth(1).maxWidth(40).minHeight(0).maxHeight(4), maxWidth: (child) => child.maxWidth(40), noWrap: (child) => child.noWrap(), textAlign: (child, align = "center") => child.textAlign(align), wrap: (child, mode) => child.wrap(mode), fitWidth: (child) => child.fitWidth(), fillWidth: (child) => child.fillWidth(), fitHeight: (child) => child.fitHeight(), fillHeight: (child) => child.fillHeight(),
  };
}
function perfFactory(): Factory<Perf7v2View> {
  const layout = (build: (builder: Layout<Perf7v2View>) => void, horizontal: boolean): Perf7v2View => {
    const adapt = (builder: { child(v: Perf7v2View): void; fixed(n: number, v: Perf7v2View): void; flex(v: Perf7v2View): void; flexMax(n: number, v: Perf7v2View): void; contentMax(n: number, v: Perf7v2View): void; gap(n: number): void }) => build({ child: (v) => builder.child(v), fixed: (n, v) => builder.fixed(n, v), flex: (v) => builder.flex(v), flexMax: (n, v) => builder.flexMax(n, v), contentMax: (n, v) => builder.contentMax(n, v), gap: (n) => builder.gap(n) });
    return horizontal ? Perf7v2View.horizontal((builder) => adapt(builder)) : Perf7v2View.vertical((builder) => adapt(builder));
  };
  return {
    text: Perf7v2View.text, styledText: Perf7v2View.styledText, spacer: Perf7v2View.spacer, vertical: (b) => layout(b, false), horizontal: (b) => layout(b, true), hanging: Perf7v2View.hanging,
    grid: (build) => Perf7v2View.grid((grid) => build({ columns: grid.columns.bind(grid), columnGap: grid.columnGap.bind(grid), rowGap: grid.rowGap.bind(grid), row: (b) => grid.row((row) => b({ cell: row.cell.bind(row), cellWith: row.cellWith.bind(row) })), rowWith: (track, b) => grid.rowWith(track, (row) => b({ cell: row.cell.bind(row), cellWith: row.cellWith.bind(row) })) })),
    diff: (size = 3) => Perf7v2View.diff(perfDiffHunks(size)),
    component: (id) => Perf7v2View.component({ id: id as never }), contentMax: Perf7v2View.contentMax, clampRows: (max, child, overflow) => child.clampRows(max, overflow), container: (child) => child.container(), padding: (n, child) => child.padding(n), background: (c, child) => child.background(c), foreground: (c, child) => child.foreground(c), border: (child, border = customBorder) => child.border(border), style: (child, style = Style.new().bold().dim().italic().underline().reversed().strikethrough()) => child.style(style), styleState: (child, key = "phase", value = "active") => child.styleState(key, value), minMax: (child) => child.minWidth(1).maxWidth(40).minHeight(0).maxHeight(4), maxWidth: (child) => child.maxWidth(40), noWrap: (child) => child.noWrap(), textAlign: (child, align = "center") => child.textAlign(align), wrap: (child, mode) => child.wrap(mode), fitWidth: (child) => child.fitWidth(), fillWidth: (child) => child.fillWidth(), fitHeight: (child) => child.fitHeight(), fillHeight: (child) => child.fillHeight(),
  };
}
function factory<T extends View | Perf7v2View>(kind: "current" | "perf7v2"): Factory<T> { return (kind === "current" ? currentFactory() : perfFactory()) as unknown as Factory<T>; }

function plain<T>(f: Factory<T>, n: number, prefix: string): T { return f.vertical((column) => { for (let i = 0; i < Math.max(1, n - 1); i += 1) column.child(f.text(`${prefix}-${i}`)); }); }
function styled<T>(f: Factory<T>, n: number): T { return f.vertical((column) => { for (let i = 0; i < Math.max(1, Math.min(n - 1, 256)); i += 1) column.child(f.textAlign(f.wrap(f.styledText([TextSpan.plain("prefix "), TextSpan.styled(`styled-${i}`, Style.new().bold().foreground(i % 2 ? "yellow" : "cyan").attribute("italic", i % 3 === 0)), TextSpan.plain(" suffix")]), i % 2 === 0 ? "wordThenGrapheme" : "grapheme"), i % 3 === 0 ? "center" : "start")); }); }
function rows<T>(f: Factory<T>, n: number): T { return f.vertical((column) => { for (let i = 0; i < Math.max(1, Math.ceil((n - 1) / 4)); i += 1) column.child(f.horizontal((row) => { row.gap(i % 3); row.child(f.text(`a-${i}`)); row.fixed(4, f.text(`b-${i}`)); row.flex(f.text(`c-${i}`)); })); }); }
function tracksTree<T>(f: Factory<T>, n: number): T { return f.vertical((column) => { for (let i = 0; i < Math.max(1, n - 1); i += 1) { const child = f.text(`track-${i}`); if (i % 4 === 0) column.fixed(8, child); else if (i % 4 === 1) column.flex(child); else if (i % 4 === 2) column.flexMax(3, child); else column.contentMax(2, child); } }); }
function gridTree<T>(f: Factory<T>, n: number): T { return f.grid((grid) => { grid.columns(tracks); grid.columnGap(1); grid.rowGap(1); for (let i = 0; i < Math.max(1, Math.ceil((n - 1) / 4)); i += 1) grid.rowWith(i % 2 ? tracks[4]! : tracks[0]!, (row) => { row.cellWith({ columnSpan: 1, horizontalAlign: "start" }, f.text(`grid-a-${i}`)); row.cellWith({ columnSpan: 2, verticalAlign: "center" }, f.text(`grid-b-${i}`)); }); }); }
function decorations<T>(f: Factory<T>, n: number): T { return f.vertical((column) => { for (let i = 0; i < Math.max(1, Math.min(n - 1, 128)); i += 1) { let child = f.text(`decorated-${i}`); child = f.padding(i % 2 ? 2 : 1, child); child = f.foreground(i % 2 ? { type: "ansi", value: 33 } : "green", child); child = f.background("theme:surface", child); child = f.border(child, i % 3 === 0 ? customBorder : { style: "plain", edges: "topBottom", color: "cyan" }); child = f.style(child, Style.new().bold().attribute("dim", i % 2 === 0)); child = f.styleState(child, "phase", i % 2 === 0 ? "a" : "b"); child = f.minMax(child); column.child(child); } }); }
function mixed<T>(f: Factory<T>, n: number): T { const each = Math.max(2, Math.floor(n / 5)); return f.vertical((column) => { column.child(plain(f, each, "mixed-text")); column.child(styled(f, each)); column.child(rows(f, each)); column.child(decorations(f, each)); column.child(f.diff(n)); }); }
function workload<T>(f: Factory<T>, name: ComparisonWorkload, n: number, suffix: string): T { switch (name) { case "plain_text_column": return plain(f, n, suffix); case "styled_span_heavy": return styled(f, n); case "row_heavy": return rows(f, n); case "column_track_heavy": return tracksTree(f, n); case "grid_heavy": return gridTree(f, n); case "decoration_heavy": return decorations(f, n); case "diff_heavy": case "large_diff_one_hunk_edit": return f.diff(n); case "component_heavy": return f.vertical((column) => { column.child(f.component(componentId)); column.child(f.text("component-tail")); }); case "mixed_realistic": return mixed(f, n); case "long_text_wrap_only": case "long_text_one_span_edit": return f.text("x".repeat(Math.min(8192, Math.max(64, n * 8)))); case "large_decoration_only_change": return f.padding(1, f.text(`${suffix}-decoration`)); } }

function deepDepth(size: number): number { return size >= 10_000 ? 128 : size >= 2_000 ? 64 : size >= 200 ? 16 : 4; }

function buildWideNext<T extends View | Perf7v2View>(kind: "current" | "perf7v2", base: T, children: readonly T[], mode: ComparisonMode, index: number, make: (values: readonly T[]) => T): T {
  const position = mode === "WIDE_PARENT_ONE_EDIT" ? index % children.length : Math.floor(children.length / 2);
  if (kind === "current") {
    const currentBase = base as View;
    if (mode === "WIDE_PARENT_ONE_EDIT") return replaceAxisChildForPackedTransport(currentBase, position, View.text(`edited-${index}`)) as T;
    if (mode === "WIDE_PARENT_INSERT") return spliceAxisChildrenForPackedTransport(currentBase, position, 0, [View.text(`inserted-${index}`)]) as T;
    return spliceAxisChildrenForPackedTransport(currentBase, position, 1, []) as T;
  }
  const next = [...children];
  if (mode === "WIDE_PARENT_ONE_EDIT") next[position] = Perf7v2View.text(`edited-${index}`) as T;
  if (mode === "WIDE_PARENT_INSERT") next.splice(position, 0, Perf7v2View.text(`inserted-${index}`) as T);
  if (mode === "WIDE_PARENT_REMOVE") next.splice(position, 1);
  return make(next);
}

export function buildComparisonPair<T extends View | Perf7v2View>(kind: "current" | "perf7v2", c: ComparisonCase, index: number): ComparisonPair<T> {
  const f = factory<T>(kind); const base = workload(f, c.workload, c.size, `base-${index}`);
  if (c.mode === "COLD" || c.mode === "FIRST_USE") return { base: workload(f, c.workload, c.size, `cold-${index}`), next: workload(f, c.workload, c.size, `cold-next-${index}`), cold: true };
  if (c.mode === "IDENTICAL_IDENTITY") return { base, next: base, cold: false };
  if (c.mode === "REBUILT_EQUIVALENT") return { base, next: workload(f, c.workload, c.size, `base-${index}`), cold: false };
  if (c.mode === "SHARED_PATH" || c.mode === "LARGE_SHARED_SUBTREE_CUTOFF") { const stable = c.mode === "SHARED_PATH" ? (c.workload === "component_heavy" ? plain(f, Math.max(2, Math.floor(c.size / 2)), `shared-${index}`) : workload(f, c.workload, Math.max(2, Math.floor(c.size / 2)), `shared-${index}`)) : plain(f, Math.max(20, c.size), `stable-${index}`); return { base: f.vertical((column) => { column.child(stable); column.child(f.text("old")); }), next: f.vertical((column) => { column.child(stable); column.child(f.text(`changed-${index}`)); }), cold: false }; }
  if (c.mode === "SHARED_DEEP") { const stable = c.workload === "component_heavy" ? plain(f, Math.max(2, Math.floor(c.size / 3)), `deep-${index}`) : workload(f, c.workload, Math.max(2, Math.floor(c.size / 3)), `deep-${index}`); const depth = deepDepth(c.size); const wrap = (suffix: string): T => { let result = f.text(suffix); for (let i = 0; i < depth; i += 1) result = f.vertical((column) => { column.child(stable); column.child(result); }); return result; }; return { base: wrap("old"), next: wrap(`new-${index}`), cold: false }; }
  if (c.mode === "TEXT_METADATA_PATCH") { const text = f.text("x".repeat(Math.min(8192, Math.max(64, c.size * 8)))); return { base: text, next: f.noWrap(text), cold: false }; }
  if (c.mode === "DECORATION_PATCH") { const text = f.text("decoration"); return { base: text, next: f.maxWidth(text), cold: false }; }
  if (c.mode === "WIDE_PARENT_ONE_EDIT" || c.mode === "WIDE_PARENT_INSERT" || c.mode === "WIDE_PARENT_REMOVE") { const children = Array.from({ length: Math.max(1, c.size) }, (_, i) => f.text(`wide-${i}`)); const make = (values: readonly T[]) => f.vertical((column) => { for (const value of values) column.child(value); }); const wideBase = make(children); return { base: wideBase, next: buildWideNext(kind, wideBase, children, c.mode, index, make), cold: false }; }
  return { base, next: f.noWrap(base), cold: false };
}

export function prepareComparisonCase<T extends View | Perf7v2View>(kind: "current" | "perf7v2", c: ComparisonCase): PreparedComparisonCase<T> {
  const f = factory<T>(kind); const full = (suffix: string) => workload(f, c.workload, c.size, suffix);
  if (c.mode === "COLD" || c.mode === "FIRST_USE") return { base: undefined, cold: true, next: (index) => full(`cold-${index}`) };
  const base = full(`base-${c.label}`);
  if (c.mode === "IDENTICAL_IDENTITY") return { base, cold: false, next: () => base };
  if (c.mode === "REBUILT_EQUIVALENT") return { base, cold: false, next: (index) => full(`rebuilt-${index}`) };
  if (c.mode === "SHARED_PATH" || c.mode === "LARGE_SHARED_SUBTREE_CUTOFF") { const stable = c.mode === "SHARED_PATH" ? (c.workload === "component_heavy" ? plain(f, Math.max(2, Math.floor(c.size / 2)), `shared-${c.label}`) : workload(f, c.workload, Math.max(2, Math.floor(c.size / 2)), `shared-${c.label}`)) : plain(f, Math.max(20, c.size), `stable-${c.label}`); return { base: f.vertical((column) => { column.child(stable); column.child(f.text("old")); }), cold: false, next: (index) => f.vertical((column) => { column.child(stable); column.child(f.text(`changed-${index}`)); }) }; }
  if (c.mode === "SHARED_DEEP") { const stable = c.workload === "component_heavy" ? plain(f, Math.max(2, Math.floor(c.size / 3)), `deep-${c.label}`) : workload(f, c.workload, Math.max(2, Math.floor(c.size / 3)), `deep-${c.label}`); const depth = deepDepth(c.size); const wrap = (suffix: string) => { let result = f.text(suffix); for (let i = 0; i < depth; i += 1) result = f.vertical((column) => { column.child(stable); column.child(result); }); return result; }; return { base: wrap("old"), cold: false, next: (index) => wrap(`new-${index}`) }; }
  if (c.mode === "TEXT_METADATA_PATCH") { const text = f.text("x".repeat(Math.min(8192, Math.max(64, c.size * 8)))); return { base: text, cold: false, next: () => f.noWrap(text) }; }
  if (c.mode === "DECORATION_PATCH") { const text = f.text("decoration"); return { base: text, cold: false, next: () => f.maxWidth(text) }; }
  if (c.mode === "WIDE_PARENT_ONE_EDIT" || c.mode === "WIDE_PARENT_INSERT" || c.mode === "WIDE_PARENT_REMOVE") { const children = Array.from({ length: Math.max(1, c.size) }, (_, i) => f.text(`wide-${i}`)); const make = (values: readonly T[]) => f.vertical((column) => { for (const value of values) column.child(value); }); const wideBase = make(children); return { base: wideBase, cold: false, next: (index) => buildWideNext(kind, wideBase, children, c.mode, index, make) }; }
  return { base, cold: false, next: () => f.noWrap(base) };
}

export function fullSchemaPair<T extends View | Perf7v2View>(kind: "current" | "perf7v2"): ComparisonPair<T> {
  const f = factory<T>(kind);
  const allAttributes = Style.new().bold().dim().italic().underline().reversed().strikethrough();
  const base = f.vertical((column) => {
    column.gap(1);
    column.child(f.wrap(f.text("文本🙂 é"), "grapheme"));
    column.fixed(2, f.textAlign(f.styledText([TextSpan.plain("styled"), TextSpan.styled(" span", allAttributes)]), "end"));
    column.flex(f.spacer(1));
    column.flexMax(3, f.hanging(f.text("prefix"), f.text("cont"), f.text("body")));
    column.contentMax(2, f.contentMax(2, f.text("content-max")));
    column.child(f.container(f.fitHeight(f.fillWidth(f.text("container")))));
    column.child(f.wrap(f.text("nowrap"), "noWrap"));
  });
  const grid = f.grid((builder) => {
    builder.columns(tracks);
    builder.columnGap(1);
    builder.rowGap(1);
    builder.row((row) => {
      row.cellWith({ columnSpan: 1, rowSpan: 2, horizontalAlign: "start", verticalAlign: "top" }, f.text("grid-start"));
      row.cellWith({ columnSpan: 2, horizontalAlign: "end", verticalAlign: "bottom" }, f.text("grid-end"));
    });
    builder.rowWith(tracks[2]!, (row) => {
      row.cellWith({ columnSpan: 2, verticalAlign: "center" }, f.text("grid-cell"));
      row.cell(f.component(42));
    });
  });
  const decorated = f.styleState(
    f.style(
      f.minMax(
        f.border(
          f.background("theme:surface", f.foreground("cyan", f.padding(1, grid))),
          customBorder,
        ),
      ),
      allAttributes,
    ),
    "phase",
    "active",
  );
  const next = f.clampRows(
    8,
    f.contentMax(7, f.vertical((column) => {
      column.child(base);
      column.child(decorated);
      column.child(f.diff(100));
      column.child(f.clampRows(2, f.text("overflow"), { kind: "footer", prefix: "… ", style: allAttributes }));
    })),
    { kind: "ellipsis", style: allAttributes },
  );
  return { base, next, cold: false };
}

export function stableNodeSnapshot(value: unknown): unknown { if (Array.isArray(value)) return value.map(stableNodeSnapshot); if (value === null || typeof value !== "object") return value; const object = value as Record<string, unknown>; return Object.fromEntries(Object.keys(object).filter((key) => key !== "id").sort().map((key) => [key, stableNodeSnapshot(object[key])])); }
export function comparisonCases(workloads: readonly ComparisonWorkload[], sizes: readonly number[]): ComparisonCase[] { const modes: readonly ComparisonMode[] = ["COLD", "FIRST_USE", "IDENTICAL_IDENTITY", "SHARED_PATH", "SHARED_DEEP", "LARGE_SHARED_SUBTREE_CUTOFF", "REBUILT_EQUIVALENT"]; return workloads.flatMap((workload) => sizes.flatMap((size) => modes.map((mode) => ({ workload, size, mode, label: `${workload}/${size}/${mode}` })))); }
export function deterministicRandom(seed: number): () => number { let state = seed >>> 0; return () => { state = (Math.imul(state, 1664525) + 1013904223) >>> 0; return state / 0x1_0000_0000; }; }
export function randomizedTree(kind: "current" | "perf7v2", seed: number, depth = 4): View | Perf7v2View {
  const f = factory(kind);
  const random = deterministicRandom(seed);
  const shared = f.styledText([TextSpan.styled(`共享-${seed}-λ`, Style.new().bold().foreground("cyan"))]);
  const build = (level: number): View | Perf7v2View => {
    if (level === 0) {
      const leaf = Math.floor(random() * 6);
      if (leaf === 0) return f.wrap(f.text(`随机-${seed}-${Math.floor(random() * 100)} ☃`), "grapheme");
      if (leaf === 1) return f.styledText([TextSpan.plain("α"), TextSpan.styled("β", Style.new().bold().attribute("italic", true))]);
      if (leaf === 2) return f.spacer(Math.floor(random() * 3));
      if (leaf === 3) return f.diff(8 + Math.floor(random() * 100));
      if (leaf === 4) return f.padding(1, shared);
      return f.textAlign(f.text("終"), "end");
    }
    const choice = Math.floor(random() * 8);
    if (choice === 0) return f.padding(1, build(level - 1));
    if (choice === 1) return f.border(build(level - 1), customBorder);
    if (choice === 2) return f.horizontal((row) => { row.gap(level % 2); row.child(build(level - 1)); row.fixed(1, build(level - 1)); });
    if (choice === 3) return f.vertical((column) => { column.child(shared); column.flex(build(level - 1)); });
    if (choice === 4) return f.hanging(build(level - 1), f.text("→"), build(level - 1));
    if (choice === 5) return f.grid((grid) => { grid.columns(tracks.slice(0, 3)); grid.rowWith(tracks[1]!, (row) => { row.cellWith({ columnSpan: 1, horizontalAlign: "start" }, build(level - 1)); row.cellWith({ columnSpan: 2, verticalAlign: "bottom" }, build(level - 1)); }); });
    if (choice === 6) return f.styleState(f.foreground("yellow", build(level - 1)), "seed", String(seed % 3));
    return f.clampRows(4, f.wrap(build(level - 1), level % 2 === 0 ? "wordThenGrapheme" : "noWrap"), { kind: "none" });
  };
  return build(depth);
}

export function randomizedRetainedPair<T extends View | Perf7v2View>(kind: "current" | "perf7v2", seed: number, depth = 4): ComparisonPair<T> {
  const f = factory<T>(kind);
  const shared = f.styledText([TextSpan.styled(`retained-${seed}-λ`, Style.new().bold().foreground("cyan"))]);
  const base = f.vertical((column) => { column.child(shared); column.child(randomizedTree(kind, seed, depth) as T); });
  const next = f.vertical((column) => { column.child(shared); column.child(f.styledText([TextSpan.styled(`retained-edit-${seed}-終`, Style.new().underline())])); });
  return { base, next, cold: false };
}
export function buildTracePair<T extends View | Perf7v2View>(kind: "current" | "perf7v2", index: number): ComparisonPair<T> & { readonly category: TraceCategory } { const slot = index % 100; const category: TraceCategory = slot < 55 ? "stream_append" : slot < 70 ? "no_view_change" : slot < 80 ? "view_replace" : slot < 88 ? "layout_change" : slot < 93 ? "component_update" : slot < 97 ? "history_update" : "structural_update"; const mode: ComparisonMode = category === "stream_append" ? "SHARED_PATH" : category === "no_view_change" ? "IDENTICAL_IDENTITY" : category === "view_replace" ? "REBUILT_EQUIVALENT" : category === "layout_change" ? "TEXT_METADATA_PATCH" : category === "component_update" ? "SHARED_PATH" : category === "history_update" ? "LARGE_SHARED_SUBTREE_CUTOFF" : "SHARED_DEEP"; return { ...buildComparisonPair<T>(kind, { workload: "mixed_realistic", size: 200, mode, label: `trace/${category}/${index}` }, index), category }; }
