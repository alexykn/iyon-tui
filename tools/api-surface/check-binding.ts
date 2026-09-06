import { readFileSync } from "node:fs";
import { join, relative, resolve } from "node:path";

// L1-01 binding seam gate: the native crate may only reach the core crate
// through `iyon_tui::binding`, the binding export set must equal the blessed
// list below, and the core crate must not offer authoring posture
// (publishable manifest, prelude, or a wholesale binding re-export at root).
const ROOT = resolve(import.meta.dir, "../..");
const NATIVE_SRC = join(ROOT, "crates/iyon-tui-native/src");
const CORE_LIB = join(ROOT, "crates/iyon-tui/src/lib.rs");
const CORE_MANIFEST = join(ROOT, "crates/iyon-tui/Cargo.toml");
const BINDING = join(ROOT, "crates/iyon-tui/src/binding/mod.rs");
const ROOT_ALLOWED_PUBLIC_MODULES = new Set(["binding", "perf_bench"]);

// blend of §5.3 lanes; the check compares sets, order is documentary.
const BLESSED_BINDING = new Set([
  "AnsiColor", "BorderEdges", "BorderGlyphs", "BorderSpec", "BorderStyle",
  "ColorSpec", "ContentAnnotationRecord", "ContentDelivery", "ContentFamily",
  "ContentMutationResult", "Counter", "DiffHunk", "DiffLine", "DiffLineNumber",
  "DiffLineOffset", "DiffLineTermination", "DiffRange", "lower_diff_hunks",
  "FormatId", "GeometryAlignment", "GridCellSpec", "GridTrack", "History",
  "HistoryLayout", "HorizontalAlign", "HostCellStyle", "HostContentConnector",
  "HostContentFunnel", "HostContentPort", "HostContentSource", "HostHistory",
  "HostScrollPane", "HostTextInput", "HostViewSlot", "HostViewState",
  "Insets", "Key", "KeyStroke", "LanguageId", "Modifiers", "intern_style_atom",
  "NativeCommonPatch", "NativeTextPage", "Output", "OverflowIndicator",
  "RetainedPathStep", "SemanticTag", "SmoothConfig",
  "StyleRef", "StyleSelector", "StyleSpec", "StyleStateKey", "StyleStateValue", "TextAttribute", "TextAttributeSpec", "TextFunnelKind",
  "TextInput", "TextOrigin", "TextPart", "TextRole", "TextSelector",
  "TextSourceKind", "TextSpan", "TextWrapMode", "Theme", "ThemeColor", "ThemeKey",
  "TuiEnvironment", "TuiHost", "VerticalAlign", "View",
  "ViewStateGeometryPatch", "ViewStateGeometryProperty",
  "ViewStatePresentationPatch", "ViewStatePresentationProperty",
  "ViewStateSizeMode", "WakeDisposition",
  "WeakView", "WrapMode", "add", "inc", "reset", "snapshot",
  "grid_cell_spec_column_span", "grid_cell_spec_horizontal_align", "grid_cell_spec_new",
  "grid_cell_spec_row_span", "grid_cell_spec_vertical_align", "grid_track_content",
  "grid_track_content_max", "grid_track_fixed", "grid_track_flex", "grid_track_flex_max",
  "text_span_plain", "text_span_styled", "view_clamp_rows", "view_downgrade",
  "view_hanging", "view_native_axis_from_children", "view_native_axis_set_child",
  "view_native_axis_splice", "view_native_component", "view_native_container",
  "view_native_content_host", "view_native_grid_final", "view_native_grid_set_cell",
  "view_native_patched", "view_native_replace_at_path", "view_native_state_attachment_id",
  "view_native_state_attachment_ids", "view_native_state_capable", "view_native_text_final",
  "view_native_with_content_attachment", "view_native_with_state_attachment", "view_spacer",
  "view_styled_text", "view_text", "view_text_plain", "view_try_replace_retained_children",
  "view_try_retained_child", "view_try_with_text_layout_patch",
  "view_try_with_text_layout_patch_path", "view_try_with_text_layout_patch_path_with_nodes",
  "view_upgrade",
]);

let failed = false;
function fail(check: string, detail: string): void {
  failed = true;
  console.log(`FAIL ${check} — ${detail}`);
}
function pass(check: string, detail: string): void {
  console.log(`PASS ${check} — ${detail}`);
}

function walkRs(dir: string): string[] {
  const out: string[] = [];
  const entries = [...new Bun.Glob("**/*.rs").scanSync({ cwd: dir })];
  for (const entry of entries) out.push(join(dir, entry));
  return out;
}

// 1. Allowlist: every core path named in the native crate goes via binding.
const offSeam: string[] = [];
for (const file of walkRs(NATIVE_SRC)) {
  const text = readFileSync(file, "utf8");
  for (const match of text.matchAll(/\biyon_tui::([A-Za-z_][A-Za-z0-9_]*)/g)) {
    if (match[1] !== "binding") offSeam.push(`${relative(ROOT, file)}: iyon_tui::${match[1]}`);
  }
}
if (offSeam.length > 0) fail("binding-seam", `native code reaches past binding: ${offSeam.join("; ")}`);
else pass("binding-seam", "all native core imports go through iyon_tui::binding");

// 2. The binding export set equals the blessed list (no silent widening).
const bindingSrc = readFileSync(BINDING, "utf8").replace(/\/\/[^\n]*/g, "");
const exported = new Set<string>();
for (const stmt of bindingSrc.matchAll(/pub use [^;]+;/g)) {
  const text = stmt[0];
  const inner = /::\{([^}]*)\}/.exec(text);
  if (inner) {
    for (const name of (inner[1] ?? "").split(",")) {
      const trimmed = name.trim();
      if (trimmed) exported.add(trimmed);
    }
    continue;
  }
  // Single-name re-exports (`pub use crate::SmoothConfig;`).
  const single = /pub use [\w:]+::(\w+)\s*;/.exec(text);
  if (single?.[1]) exported.add(single[1]);
}
const added = [...exported].filter((n) => !BLESSED_BINDING.has(n)).sort();
const removed = [...BLESSED_BINDING].filter((n) => !exported.has(n)).sort();
if (added.length > 0 || removed.length > 0) {
  fail("binding-surface", `drift — added [${added}] removed [${removed}]; amend the blessed list deliberately`);
} else {
  pass("binding-surface", `${exported.size} binding exports match the blessed list`);
}

// The unsupported seam must not accidentally grow back into the removed
// authoring facade.  Operation-specific native constructors remain allowed;
// these names are the old trait/renderer entry points themselves.
const forbiddenAuthoring = ["IntoView", "Renderer", "DiffRenderer"].filter((name) =>
  exported.has(name),
);
if (forbiddenAuthoring.length > 0) {
  fail("binding-no-authoring-names", `forbidden exports remain: ${forbiddenAuthoring.join(", ")}`);
} else {
  pass("binding-no-authoring-names", "removed Rust authoring traits/renderers are not exported");
}

// 3. The unsupported binding is the only public runtime seam. The one
// feature-gated benchmark module is an explicit tooling exception: it is
// hidden from documentation and is not an authoring API. Keeping this check on the
// source root (rather than only on the mapping snapshot) makes a newly added
// `pub use` or public semantic module fail immediately.
const lib = readFileSync(CORE_LIB, "utf8");
const manifest = readFileSync(CORE_MANIFEST, "utf8");
const posture: string[] = [];
const leakedRootUses = [...lib.matchAll(/^\s*pub\s+use\b[^\n]*/gm)].map((m) => m[0]!.trim());
if (leakedRootUses.length > 0) {
  posture.push(`crate root publishes unsupported re-exports: ${leakedRootUses.join("; ")}`);
}
for (const match of lib.matchAll(/^\s*pub\s+mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;/gm)) {
  const name = match[1]!;
  if (!ROOT_ALLOWED_PUBLIC_MODULES.has(name)) {
    posture.push(`crate root publishes unsupported module ${name}`);
    continue;
  }
  if (name !== "binding") {
    const declarationStart = match.index ?? 0;
    const context = lib.slice(Math.max(0, declarationStart - 160), declarationStart);
    if (!/#\[\s*doc\s*\(\s*hidden\s*\)\s*\]/u.test(context)) {
      posture.push(`internal tooling module ${name} is not #[doc(hidden)]`);
    }
    if (name === "perf_bench" && !/#\[\s*cfg\(\s*feature\s*=\s*"perf-counters"\s*\)\s*\]/u.test(context)) {
      posture.push("internal tooling module perf_bench is not feature-gated");
    }
  }
}
if (/(^|\n)\s*pub\s+mod\s+prelude\b/.test(lib)) posture.push("pub mod prelude is still offered");
if (!/^\s*publish\s*=\s*false\s*$/m.test(manifest)) posture.push("core manifest is not publish = false");
if (/pub\s+use\s+(?:crate::|self::)?binding::/.test(lib)) posture.push("binding is re-exported wholesale at the crate root");
if (posture.length > 0) fail("binding-no-authoring", posture.join("; "));
else pass("binding-no-authoring", "no prelude, unpublished manifest, no root binding re-export");

if (failed) {
  console.log("\nBINDING CHECKS FAILED");
  process.exit(1);
}
console.log("\nALL BINDING CHECKS PASSED");
