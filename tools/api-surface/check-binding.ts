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

// blend of §5.3 lanes; the check compares sets, order is documentary.
const BLESSED_BINDING = new Set([
  "AnsiColor", "BorderEdges", "BorderGlyphs", "BorderSpec", "BorderStyle",
  "ColorSpec", "ContentAnnotationRecord", "ContentDelivery", "ContentFamily",
  "ContentMutationResult", "Counter", "DiffHunk", "DiffLine", "DiffLineNumber",
  "DiffLineOffset", "DiffLineTermination", "DiffRange", "DiffRenderer",
  "FormatId", "GeometryAlignment", "GridCellSpec", "GridTrack", "History",
  "HistoryLayout", "HorizontalAlign", "HostCellStyle", "HostContentConnector",
  "HostContentFunnel", "HostContentPort", "HostContentSource", "HostHistory",
  "HostScrollPane", "HostTextInput", "HostViewSlot", "HostViewState",
  "Insets", "IntoView", "Key", "KeyStroke", "LanguageId", "Modifiers",
  "NativeCommonPatch", "Output", "OverflowIndicator", "Renderer",
  "RetainedPathStep", "SemanticTag", "SmoothConfig",
  "StyleRef", "StyleSelector", "StyleSpec", "TextAttribute", "TextFunnelKind",
  "TextInput", "TextOrigin", "TextPart", "TextRole", "TextSelector",
  "TextSourceKind", "TextSpan", "TextWrapMode", "Theme", "ThemeColor",
  "TuiEnvironment", "TuiHost", "VerticalAlign", "View",
  "ViewStateGeometryPatch", "ViewStateGeometryProperty",
  "ViewStatePresentationPatch", "ViewStatePresentationProperty",
  "ViewStateSizeMode", "ViewStateTextAttributes", "WakeDisposition",
  "WeakView", "WrapMode", "add", "inc", "reset", "snapshot",
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

// 3. Forbidden authoring posture on the core crate.
const lib = readFileSync(CORE_LIB, "utf8");
const manifest = readFileSync(CORE_MANIFEST, "utf8");
const posture: string[] = [];
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
