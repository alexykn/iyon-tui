/**
 * Standalone TUI ownership gates (IYON-TUI-REPOSITORY-SEPARATION-HANDOFF §7).
 *
 * Machine-checks the generic framework, native bridge, public surface, and
 * external-consumer fixture. Run with `bun run check:ownership`. Every
 * failure names the file and rule; no gate relies on prose alone.
 */

import { readFileSync, existsSync, statSync, readdirSync } from "node:fs";
import { createHash } from "node:crypto";
import { join, relative, resolve, dirname } from "node:path";

const ROOT = resolve(import.meta.dir, "../..");
let failed = false;

function pass(name: string, detail?: string): void {
  console.log(`PASS ${name}${detail ? ` — ${detail}` : ""}`);
}
function fail(name: string, detail: string): void {
  failed = true;
  console.log(`FAIL ${name} — ${detail}`);
}

function walk(dir: string, out: string[] = []): string[] {
  for (const entry of Array.from(readdirSync(dir)).sort()) {
    if (entry === "node_modules" || entry.startsWith(".")) continue;
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) walk(path, out);
    else if (path.endsWith(".ts")) out.push(path);
  }
  return out;
}

/** Resolve a relative TS specifier to a file path, or null when unresolvable. */
function resolveRelative(fromFile: string, specifier: string): string | null {
  const clean = specifier.replace(/[?#].*$/, "");
  const base = resolve(dirname(fromFile), clean);
  for (const candidate of [base, `${base}.ts`, join(base, "index.ts")]) {
    if (existsSync(candidate) && statSync(candidate).isFile()) return candidate;
  }
  return null;
}

/** All module specifiers referenced by a TS source (static, dynamic, bare). */
function specifiersOf(source: string): string[] {
  const result: string[] = [];
  const pattern = /\bfrom\s+["']([^"']+)["']|\bimport\s*(?:\(\s*)?["']([^"']+)["']/gu;
  for (const match of source.matchAll(pattern)) result.push(match[1] ?? match[2]!);
  return result;
}

// ---------------------------------------------------------------------------
// Gate 1: Rust dependency direction
// ---------------------------------------------------------------------------

function rustDependencyGate(): void {
  const meta = JSON.parse(
    new TextDecoder().decode(
      Bun.spawnSync(["cargo", "metadata", "--format-version", "1", "--no-deps"], { cwd: ROOT }).stdout,
    ),
  );
  const byName = new Map<string, string[]>();
  for (const pkg of meta.packages as { name: string; dependencies: { name: string }[] }[]) {
    if (!byName.has(pkg.name)) byName.set(pkg.name, pkg.dependencies.map((d) => d.name));
  }

  function closure(rootName: string): Set<string> {
    const seen = new Set<string>();
    const queue = [rootName];
    while (queue.length > 0) {
      const name = queue.pop()!;
      if (seen.has(name)) continue;
      seen.add(name);
      for (const dep of byName.get(name) ?? []) {
        if (byName.has(dep)) queue.push(dep);
      }
    }
    return seen;
  }

  const forbidden = ["iyon-core", "iyon-api"];
  const tuiClosure = closure("iyon-tui");
  const leaked = forbidden.filter((name) => tuiClosure.has(name));
  if (leaked.length > 0) fail("rust-dependency-direction", `closure(iyon-tui) reaches ${leaked.join(", ")}`);
  else pass("rust-dependency-direction", `closure(iyon-tui) excludes ${forbidden.join(" and ")}`);

  const tuiNativePaths = [
    "crates/iyon-tui-native/src/tui.rs",
    "crates/iyon-tui-native/src/tui",
    "crates/iyon-tui-native/src/generated",
    "crates/iyon-tui-native/tests/generated_view_abi.rs",
  ];
  const offenders: string[] = [];
  for (const path of tuiNativePaths) {
    const full = join(ROOT, path);
    if (!existsSync(full)) continue;
    const files = statSync(full).isDirectory()
      ? Array.from(new Bun.Glob("**/*.rs").scanSync({ cwd: full })).map((f) => join(full, f))
      : [full];
    for (const file of files) {
      if (/\biyon_(core|api)\b/.test(readFileSync(file, "utf8"))) offenders.push(relative(ROOT, file));
    }
  }
  if (offenders.length > 0) fail("tui-native-module-purity", `references iyon_core/iyon_api: ${offenders.join(", ")}`);
  else pass("tui-native-module-purity", "TUI-native modules reference no application crate");

  const tuiRustOffenders = Array.from(
    new Bun.Glob("**/*.rs").scanSync({ cwd: join(ROOT, "crates/iyon-tui") }),
  ).filter((f) => /\biyon_(core|api)\b/.test(readFileSync(join(ROOT, "crates/iyon-tui", f), "utf8")));
  if (tuiRustOffenders.length > 0) fail("framework-rust-purity", `crates/iyon-tui references app crates: ${tuiRustOffenders.join(", ")}`);
  else pass("framework-rust-purity", "crates/iyon-tui sources reference no application crate");
}

// ---------------------------------------------------------------------------
// Gate 2: TypeScript import direction
// ---------------------------------------------------------------------------

const FRAMEWORK_SRC = join(ROOT, "packages/iyon-tui/src");
const NATIVE_CONTRACT = resolve(ROOT, "packages/iyon-tui/src/transport/native/addon.ts");
const PUBLIC_CONTRACT_PATHS = [
  "api/controls/framework-handle.ts",
  "api/controls/history.ts",
  "api/controls/output.ts",
  "api/controls/scroll-pane.ts",
  "api/controls/text-input.ts",
  "api/controls/text-stream.ts",
  "api/controls/view-slot.ts",
  "api/content/stream-snapshot.ts",
  "api/content/text-content.ts",
  "api/content/text.ts",
  "api/errors.ts",
  "api/extensions/traits/component.ts",
  "api/extensions/traits/projector.ts",
  "api/extensions/traits/renderer.ts",
  "api/extensions/traits/streaming-source.ts",
  "api/extensions/traits/text-rewriter.ts",
  "api/extensions/traits/text-visitor.ts",
  "api/presentation/style.ts",
  "api/presentation/theme.ts",
  "api/view/geometry.ts",
  "api/view/scene.ts",
  "api/view/view.ts",
  "composition/define-view.ts",
  "composition/tracked-state.ts",
  "runtime/events.ts",
  "runtime/runtime.ts",
  "testing/index.ts",
] as const;

function publicContractSource(): string {
  return PUBLIC_CONTRACT_PATHS
    .map((path) => readFileSync(join(FRAMEWORK_SRC, path), "utf8"))
    .join("\n");
}

function tsImportGate(): void {
  const files = walk(FRAMEWORK_SRC);
  const violations: string[] = [];
  const seams: string[] = [];

  for (const file of files) {
    const source = readFileSync(file, "utf8");
    const specifiers = specifiersOf(source);
    for (const spec of specifiers) {
      if (!spec.startsWith(".") && !spec.startsWith("/")) {
        if (/^(bun|node):/.test(spec)) continue;
        violations.push(`${relative(ROOT, file)} -> "${spec}"`);
        continue;
      }
      const resolved = resolveRelative(file, spec);
      if (resolved === null) {
        violations.push(`${relative(ROOT, file)} -> "${spec}" (unresolved)`);
        continue;
      }
      if (resolved === NATIVE_CONTRACT) {
        seams.push(`${relative(ROOT, file)} -> transport/native/addon.ts`);
        continue;
      }
      if (resolved.startsWith(FRAMEWORK_SRC)) continue;
      violations.push(`${relative(ROOT, file)} -> "${spec}" escapes framework`);
    }
  }
  if (violations.length > 0) fail("framework-ts-import-direction", violations.join("; "));
  else pass("framework-ts-import-direction", `${files.length} files import only framework modules (+${seams.length} recorded native-contract seams)`);

}

// ---------------------------------------------------------------------------
// Gate 3: H2 CUT 2 composition/structural ownership
// ---------------------------------------------------------------------------

function cut2OwnershipGate(): void {
  const required = [
    "composition/child-owner.ts",
    "composition/compose.ts",
    "composition/define-view.ts",
    "composition/execution-context.ts",
    "composition/execution.ts",
    "composition/persistent-seq.ts",
    "composition/tracked-state.ts",
    "transport/structural/component-view.ts",
    "transport/structural/ir.ts",
    "transport/structural/native-view-abi.ts",
    "transport/structural/policy.ts",
    "transport/structural/retained-dag.ts",
    "transport/structural/style-lowering.ts",
    "transport/structural/view-bridge.ts",
  ];
  const legacy = [
    "child-owner.ts",
    "compose.ts",
    "define-view.ts",
    "execution-context.ts",
    "execution.ts",
    "internal-composition.ts",
    "persistent_seq.ts",
    "tracked-state.ts",
    "component-facade.ts",
    "ir.ts",
    "native_view_abi.ts",
    "native_view_policy.ts",
    "retained_dag.ts",
    "style-internals.ts",
    "view-internals.ts",
  ];
  const missing = required.filter((path) => !existsSync(join(FRAMEWORK_SRC, path)));
  const stale = legacy.filter((path) => existsSync(join(FRAMEWORK_SRC, path)));
  const offenders: string[] = [];
  if (missing.length > 0) offenders.push(`missing CUT 2 owners: ${missing.join(", ")}`);
  if (stale.length > 0) offenders.push(`legacy root owners remain: ${stale.join(", ")}`);

  const compositionRootImports = new Set([
    "runtime.ts",
    "tui.ts",
    "native.ts",
    "native-handles.ts",
    "handles.ts",
    "handle-registry.ts",
    "component.ts",
    "history.ts",
    "text-input.ts",
    "stream.ts",
    "scroll-pane.ts",
  ]);
  for (const file of walk(join(FRAMEWORK_SRC, "composition"))) {
    for (const specifier of specifiersOf(readFileSync(file, "utf8"))) {
      if (!specifier.startsWith(".")) continue;
      const resolved = resolveRelative(file, specifier);
      if (resolved === null || !resolved.startsWith(FRAMEWORK_SRC)) continue;
      const target = relative(FRAMEWORK_SRC, resolved);
      if (compositionRootImports.has(target)) {
        offenders.push(`${relative(ROOT, file)} imports live runtime/native owner ${target}`);
      }
    }
  }

  for (const file of walk(join(FRAMEWORK_SRC, "api"))) {
    for (const specifier of specifiersOf(readFileSync(file, "utf8"))) {
      if (!specifier.startsWith(".")) continue;
      const resolved = resolveRelative(file, specifier);
      if (resolved !== null && relative(FRAMEWORK_SRC, resolved).startsWith("transport/abi/")) {
        offenders.push(`${relative(ROOT, file)} imports generated structural ABI directly`);
      }
    }
  }

  if (offenders.length > 0) {
    fail("h2-cut2-ownership", offenders.join("; "));
  } else {
    pass("h2-cut2-ownership", "composition and structural transport have dedicated owners with no legacy root peers");
  }
}

// ---------------------------------------------------------------------------
// Gate 4: H2 CUT 3 runtime/native/control ownership
// ---------------------------------------------------------------------------

function cut3OwnershipGate(): void {
  const required = [
    "api/controls/framework-handle.ts",
    "api/controls/history.ts",
    "api/controls/scroll-pane.ts",
    "api/controls/text-input.ts",
    "api/controls/text-stream.ts",
    "api/controls/view-slot.ts",
    "runtime/handle-registry.ts",
    "runtime/runtime.ts",
    "transport/native/addon.ts",
    "transport/native/factories.ts",
    "transport/native/resources.ts",
  ];
  const legacy = [
    "component.ts",
    "handle-registry.ts",
    "handles.ts",
    "history.ts",
    "native-handles.ts",
    "native.ts",
    "runtime.ts",
    "scroll-pane.ts",
    "stream.ts",
    "text-input.ts",
    "tui.ts",
  ];
  const missing = required.filter((path) => !existsSync(join(FRAMEWORK_SRC, path)));
  const stale = legacy.filter((path) => existsSync(join(FRAMEWORK_SRC, path)));
  const offenders: string[] = [];
  if (missing.length > 0) offenders.push(`missing CUT 3 owners: ${missing.join(", ")}`);
  if (stale.length > 0) offenders.push(`legacy runtime/native/control roots remain: ${stale.join(", ")}`);

  const compositionForbidden = ["runtime/", "transport/native/", "api/controls/"];
  for (const file of walk(join(FRAMEWORK_SRC, "composition"))) {
    for (const specifier of specifiersOf(readFileSync(file, "utf8"))) {
      if (!specifier.startsWith(".")) continue;
      const resolved = resolveRelative(file, specifier);
      if (resolved === null || !resolved.startsWith(FRAMEWORK_SRC)) continue;
      const target = relative(FRAMEWORK_SRC, resolved);
      const semanticHandleContract = target === "api/controls/framework-handle.ts";
      if (!semanticHandleContract && compositionForbidden.some((prefix) => target.startsWith(prefix))) {
        offenders.push(`${relative(ROOT, file)} imports live runtime/native/control owner ${target}`);
      }
    }
  }

  for (const file of walk(join(FRAMEWORK_SRC, "transport/native"))) {
    for (const specifier of specifiersOf(readFileSync(file, "utf8"))) {
      if (!specifier.startsWith(".")) continue;
      const resolved = resolveRelative(file, specifier);
      if (resolved === null || !resolved.startsWith(FRAMEWORK_SRC)) continue;
      const target = relative(FRAMEWORK_SRC, resolved);
      if (target.startsWith("runtime/")) {
        offenders.push(`${relative(ROOT, file)} imports runtime ownership from transport/native: ${target}`);
      }
    }
  }

  const frameworkHandle = readFileSync(join(FRAMEWORK_SRC, "api/controls/framework-handle.ts"), "utf8");
  const runtimeRegistry = readFileSync(join(FRAMEWORK_SRC, "runtime/handle-registry.ts"), "utf8");
  const nativeResources = readFileSync(join(FRAMEWORK_SRC, "transport/native/resources.ts"), "utf8");
  const addon = readFileSync(join(FRAMEWORK_SRC, "transport/native/addon.ts"), "utf8");
  const index = readFileSync(join(FRAMEWORK_SRC, "index.ts"), "utf8");
  if (!/export\s+abstract\s+class\s+FrameworkHandle[\s\S]*#frameworkHandleBrand/u.test(frameworkHandle)) {
    offenders.push("api/controls/framework-handle.ts: missing nominal public handle implementation");
  }
  if (!/registerFrameworkHandle[\s\S]*disposeFrameworkResource/u.test(frameworkHandle)) {
    offenders.push("api/controls/framework-handle.ts: public handle does not delegate runtime lifecycle");
  }
  if (!/new\s+WeakMap<object,\s*object>\(\)[\s\S]*nativeResourceOf/u.test(nativeResources)) {
    offenders.push("transport/native/resources.ts: raw native-resource registry is missing");
  }
  if (/new\s+WeakMap<object,\s*object>\(\)/u.test(runtimeRegistry)) {
    offenders.push("runtime/handle-registry.ts: raw native resources remain in runtime ownership");
  }
  if (!/require\("\.\.\/\.\.\/\.\.\/native\/iyon-tui-native\.node"\)/u.test(addon)) {
    offenders.push("transport/native/addon.ts: addon loading is not owned by transport/native");
  }
  if (/\b(?:transport\/native|transport\/structural|runtime\/handle-registry)\//u.test(index)) {
    offenders.push("index.ts: private runtime/native/transport path is exported");
  }

  if (offenders.length > 0) {
    fail("h2-cut3-ownership", offenders.join("; "));
  } else {
    pass("h2-cut3-ownership", "controls, live runtime ownership, and raw native access have dedicated owners with no legacy root peers");
  }
}

// ---------------------------------------------------------------------------
// Gate 5: H2 CUT 4 root cleanup and contract ownership
// ---------------------------------------------------------------------------

function cut4RootCleanupGate(): void {
  const rootFiles = Array.from(readdirSync(FRAMEWORK_SRC))
    .filter((entry) => entry.endsWith(".ts"))
    .sort();
  const offenders: string[] = [];
  const unexpectedRootFiles = rootFiles.filter((entry) => entry !== "index.ts");
  if (unexpectedRootFiles.length > 0) {
    offenders.push(`root implementation files remain: ${unexpectedRootFiles.join(", ")}`);
  }

  const removedResidue = [
    "types.ts",
    "style-internals.ts",
    "view-internals.ts",
    "component-facade.ts",
    "internal-composition.ts",
    "composition/internal-composition.ts",
    "runtime/tui.ts",
  ];
  const staleResidue = removedResidue.filter((path) => existsSync(join(FRAMEWORK_SRC, path)));
  if (staleResidue.length > 0) offenders.push(`ambiguous or forwarding modules remain: ${staleResidue.join(", ")}`);

  const forbiddenImportFiles = walk(FRAMEWORK_SRC).filter((file) =>
    specifiersOf(readFileSync(file, "utf8")).some((specifier) => /(?:^|\/)types\.ts$/u.test(specifier)),
  );
  if (forbiddenImportFiles.length > 0) {
    offenders.push(`legacy types.ts imports remain: ${forbiddenImportFiles.map((file) => relative(ROOT, file)).join(", ")}`);
  }

  const forbiddenDirectories = ["shared", "common", "misc", "utils"];
  const escapeHatches = forbiddenDirectories.filter((name) => existsSync(join(FRAMEWORK_SRC, name)));
  if (escapeHatches.length > 0) offenders.push(`architectural escape-hatch directories remain: ${escapeHatches.join(", ")}`);

  const owners: readonly [string, RegExp][] = [
    ["api/presentation/style.ts", /export\s+interface\s+StyleSpecValue\b/u],
    ["api/content/text.ts", /export\s+interface\s+TextSelectorValue\b/u],
    ["api/view/scene.ts", /export\s+type\s+SceneProducer\b/u],
    ["api/controls/history.ts", /export\s+interface\s+History\b/u],
    ["api/controls/text-input.ts", /export\s+interface\s+TextInput\b/u],
    ["api/controls/text-stream.ts", /export\s+interface\s+TextStream\b/u],
    ["api/controls/view-slot.ts", /export\s+interface\s+ViewSlot\b/u],
    ["api/controls/scroll-pane.ts", /export\s+interface\s+ScrollPane\b/u],
    ["api/extensions/traits/component.ts", /export\s+interface\s+ComponentAdapter\b/u],
    ["runtime/events.ts", /export\s+type\s+TuiEvent\b/u],
    ["runtime/runtime.ts", /export\s+interface\s+TuiRuntime\b/u],
  ];
  for (const [path, pattern] of owners) {
    const full = join(FRAMEWORK_SRC, path);
    if (!existsSync(full) || !pattern.test(readFileSync(full, "utf8"))) {
      offenders.push(`${path}: semantic contract has no explicit owner`);
    }
  }

  const index = readFileSync(join(FRAMEWORK_SRC, "index.ts"), "utf8");
  if (/from\s+["']\.\/(?:types|style-internals|view-internals|component-facade)[^"']*["']/u.test(index)) {
    offenders.push("index.ts: root barrel still forwards an eliminated module");
  }

  if (offenders.length > 0) {
    fail("h2-cut4-root-cleanup", offenders.join("; "));
  } else {
    pass("h2-cut4-root-cleanup", "root contains only the curated barrel; mixed contracts and forwarding residue have explicit owners");
  }
}

// ---------------------------------------------------------------------------
// Gate 6: H2 CUT 5 import-direction enforcement
// ---------------------------------------------------------------------------

function cut5ImportBoundaryGate(): void {
  const offenders: string[] = [];
  for (const file of walk(FRAMEWORK_SRC)) {
    const owner = relative(FRAMEWORK_SRC, file).replaceAll("\\", "/");
    const production = !owner.startsWith("testing/");
    for (const specifier of specifiersOf(readFileSync(file, "utf8"))) {
      if (!specifier.startsWith(".") && !specifier.startsWith("/")) continue;
      const resolved = resolveRelative(file, specifier);
      if (resolved === null || !resolved.startsWith(FRAMEWORK_SRC)) continue;
      const target = relative(FRAMEWORK_SRC, resolved).replaceAll("\\", "/");

      if (production && target.startsWith("testing/")) {
        offenders.push(`${relative(ROOT, file)} imports production code from testing: ${target}`);
      }
      if (owner.startsWith("composition/") && (
        target.startsWith("runtime/")
        || target.startsWith("transport/native/")
        || target.startsWith("testing/")
      )) {
        offenders.push(`${relative(ROOT, file)} imports live host/native/testing ownership: ${target}`);
      }
      if (owner.startsWith("api/") && (
        target.startsWith("transport/abi/")
        || target.includes("/generated/")
      )) {
        offenders.push(`${relative(ROOT, file)} imports generated ABI from semantic API: ${target}`);
      }
    }
  }

  if (offenders.length > 0) {
    fail("h2-cut5-import-boundaries", offenders.join("; "));
  } else {
    pass("h2-cut5-import-boundaries", "production does not import testing, composition avoids live host/native seams, and API code avoids generated ABI");
  }
}

// ---------------------------------------------------------------------------
// Gate 7: H3-B composition/structural transport seam
// ---------------------------------------------------------------------------

function h3bCompositionTransportGate(): void {
  const offenders: string[] = [];
  for (const file of walk(join(FRAMEWORK_SRC, "composition"))) {
    for (const specifier of specifiersOf(readFileSync(file, "utf8"))) {
      if (!specifier.startsWith(".")) continue;
      const resolved = resolveRelative(file, specifier);
      if (resolved === null || !resolved.startsWith(FRAMEWORK_SRC)) continue;
      const target = relative(FRAMEWORK_SRC, resolved).replaceAll("\\", "/");
      if (target.startsWith("transport/")) {
        offenders.push(`${relative(ROOT, file)} imports structural transport: ${target}`);
      }
    }
  }
  if (offenders.length > 0) {
    fail("h3b-composition-transport-seam", offenders.join("; "));
  } else {
    pass("h3b-composition-transport-seam", "composition imports no structural/native transport implementation");
  }
}

// ---------------------------------------------------------------------------
// Gate 8: H3-C structural transport/composition seam
// ---------------------------------------------------------------------------

function h3cStructuralCompositionGate(): void {
  const offenders: string[] = [];
  for (const file of walk(join(FRAMEWORK_SRC, "transport/structural"))) {
    for (const specifier of specifiersOf(readFileSync(file, "utf8"))) {
      if (!specifier.startsWith(".")) continue;
      const resolved = resolveRelative(file, specifier);
      if (resolved === null || !resolved.startsWith(FRAMEWORK_SRC)) continue;
      const target = relative(FRAMEWORK_SRC, resolved).replaceAll("\\", "/");
      if (target.startsWith("composition/")) {
        offenders.push(`${relative(ROOT, file)} imports composition implementation: ${target}`);
      }
    }
  }
  if (offenders.length > 0) {
    fail("h3c-structural-composition-seam", offenders.join("; "));
  } else {
    pass("h3c-structural-composition-seam", "structural transport imports no composition implementation");
  }
}

// ---------------------------------------------------------------------------
// Gate 9: H2 CUT 5 root publication boundary
// ---------------------------------------------------------------------------

function cut5RootPublicationGate(): void {
  const indexPath = join(FRAMEWORK_SRC, "index.ts");
  const index = readFileSync(indexPath, "utf8");
  const allowedTargets = new Set([
    "runtime/events.ts",
    "runtime/runtime.ts",
    "composition/define-view.ts",
    "composition/tracked-state.ts",
  ]);
  const offenders: string[] = [];

  if (/\bexport\s+(?:type\s+)?\*/u.test(index)) {
    offenders.push("index.ts: wildcard export hides the public/private boundary");
  }

  const exportPattern = /export(?:\s+type)?\s*\{([\s\S]*?)\}\s*from\s*["']([^"']+)["']/gu;
  for (const match of index.matchAll(exportPattern)) {
    const clause = match[1]!;
    const specifier = match[2]!;
    const resolved = resolveRelative(indexPath, specifier);
    if (resolved === null) {
      offenders.push(`index.ts: unresolved public export target ${specifier}`);
      continue;
    }
    const target = relative(FRAMEWORK_SRC, resolved).replaceAll("\\", "/");
    if (!target.startsWith("api/") && !allowedTargets.has(target)) {
      offenders.push(`index.ts: private module is root-exported: ${target}`);
    }
    if (target.startsWith("transport/") || target.startsWith("testing/")) {
      offenders.push(`index.ts: bridge/native/generated/testing module is root-exported: ${target}`);
    }

    for (const item of clause.split(",")) {
      const exportedName = item
        .replace(/\/\/.*$/u, "")
        .trim()
        .replace(/^type\s+/u, "")
        .split(/\s+as\s+/u)
        .pop()!
        .trim();
      if (/^(?:Bridge|Native)[A-Z]/u.test(exportedName)) {
        offenders.push(`index.ts: bridge/native symbol is root-exported: ${exportedName}`);
      }
    }
  }

  if (offenders.length > 0) {
    fail("h2-cut5-root-publication", offenders.join("; "));
  } else {
    pass("h2-cut5-root-publication", "root exports are explicit semantic/API owners and do not publish bridge, native, generated, or testing modules");
  }
}

// ---------------------------------------------------------------------------
// Gate 8: H2 CUT 5 module identity and alias guard
// ---------------------------------------------------------------------------

function cut5ModuleIdentityGate(): void {
  const offenders: string[] = [];
  for (const configPath of ["tsconfig.json", "packages/iyon-tui/tsconfig.json"]) {
    const config = JSON.parse(readFileSync(join(ROOT, configPath), "utf8")) as {
      compilerOptions?: { baseUrl?: unknown; paths?: Record<string, unknown> };
    };
    const compilerOptions = config.compilerOptions ?? {};
    if (compilerOptions.baseUrl !== undefined) {
      offenders.push(`${configPath}: compilerOptions.baseUrl creates an alternate module root`);
    }
    if (compilerOptions.paths !== undefined && Object.keys(compilerOptions.paths).length > 0) {
      offenders.push(`${configPath}: compilerOptions.paths creates an alternate module identity`);
    }
  }

  for (const file of walk(FRAMEWORK_SRC)) {
    const targetSpellings = new Map<string, Set<string>>();
    for (const specifier of specifiersOf(readFileSync(file, "utf8"))) {
      if (specifier === "@iyon/tui" || specifier.startsWith("@iyon/tui/")) {
        offenders.push(`${relative(ROOT, file)} imports the package from inside its own source: ${specifier}`);
        continue;
      }
      if (specifier.startsWith("/")) {
        offenders.push(`${relative(ROOT, file)} uses an absolute local import: ${specifier}`);
        continue;
      }
      if (!specifier.startsWith(".")) continue;
      const resolved = resolveRelative(file, specifier);
      if (resolved === null || !resolved.startsWith(FRAMEWORK_SRC)) continue;
      const target = relative(FRAMEWORK_SRC, resolved).replaceAll("\\", "/");
      const spellings = targetSpellings.get(target) ?? new Set<string>();
      spellings.add(specifier);
      targetSpellings.set(target, spellings);
    }
    for (const [target, spellings] of targetSpellings) {
      if (spellings.size > 1) {
        offenders.push(`${relative(ROOT, file)} has alternate spellings for ${target}: ${[...spellings].join(", ")}`);
      }
    }
  }

  if (offenders.length > 0) {
    fail("h2-cut5-module-identity", offenders.join("; "));
  } else {
    pass("h2-cut5-module-identity", "framework source has no path aliases, package self-imports, absolute imports, or duplicate local spellings");
  }
}

// ---------------------------------------------------------------------------
// Gate 9: H2 CUT 5 package/publication and future-plane guard
// ---------------------------------------------------------------------------

function cut5PackagePublicationGate(): void {
  const offenders: string[] = [];
  const futurePaths = [
    "transport/state",
    "transport/content",
    "transport/abi/state",
    "transport/abi/content",
  ];
  for (const path of futurePaths) {
    if (existsSync(join(FRAMEWORK_SRC, path))) offenders.push(`future PERF-13 ownership slot is prematurely implemented or published: ${path}`);
  }

  const manifests: readonly [string, Record<string, string>][] = [
    ["package.json", {
      ".": "./packages/iyon-tui/src/index.ts",
      "./testing": "./packages/iyon-tui/src/testing/index.ts",
      "./native-stage": "./packages/iyon-tui/scripts/stage-native.ts",
    }],
    ["packages/iyon-tui/package.json", {
      ".": "./src/index.ts",
      "./testing": "./src/testing/index.ts",
      "./native-stage": "./scripts/stage-native.ts",
    }],
  ];
  for (const [path, expected] of manifests) {
    const manifest = JSON.parse(readFileSync(join(ROOT, path), "utf8")) as { exports?: Record<string, unknown> };
    const actual = manifest.exports;
    if (actual === undefined) {
      offenders.push(`${path}: package exports are missing`);
      continue;
    }
    for (const key of Object.keys(actual)) {
      if (!(key in expected)) offenders.push(`${path}: undocumented deep export ${key}`);
    }
    for (const [key, value] of Object.entries(expected)) {
      if (actual[key] !== value) offenders.push(`${path}: export ${key} is ${JSON.stringify(actual[key])}, expected ${JSON.stringify(value)}`);
    }
    for (const key of Object.keys(actual)) {
      if (/\.\/(?:state|content|transport|abi)(?:\/|$)/u.test(key)) {
        offenders.push(`${path}: future/private plane is published as ${key}`);
      }
    }
  }

  if (offenders.length > 0) {
    fail("h2-cut5-package-publication", offenders.join("; "));
  } else {
    pass("h2-cut5-package-publication", "package exports are limited to documented entrypoints and future state/content planes remain unpublished");
  }
}

// ---------------------------------------------------------------------------
// Gate 10: S6 safe N-API transport boundary
// ---------------------------------------------------------------------------

function napiTransportGate(): void {
  const files = walk(FRAMEWORK_SRC);
  const forbidden = /bun:ffi|linkSymbols|NativeAbiPointers|tuiViewAbiBootstrap|runtime_ptr|host_ptr/u;
  const offenders = files
    .filter((file) => forbidden.test(readFileSync(file, "utf8")))
    .map((file) => relative(ROOT, file));
  const nativeContract = readFileSync(NATIVE_CONTRACT, "utf8");
  if (/NativeViewAbiBootstrap|tuiViewAbiBootstrap|Pointer|runtime_ptr|host_ptr/u.test(nativeContract)) {
    offenders.push(relative(ROOT, NATIVE_CONTRACT));
  }
  if (offenders.length > 0) {
    fail("safe-napi-ts-boundary", `unsafe transport surface in ${offenders.join(", ")}`);
  } else {
    pass("safe-napi-ts-boundary", `${files.length} active framework files use no Bun FFI or raw pointer contract`);
  }

  const generatedNapi = join(ROOT, "crates/iyon-tui-native/src/generated/view_abi_napi.rs");
  const manifest = join(ROOT, "packages/iyon-tui/src/transport/abi/structural/generated/view_abi_manifest.json");
  const cargo = readFileSync(join(ROOT, "crates/iyon-tui-native/Cargo.toml"), "utf8");
  if (!existsSync(generatedNapi) || !existsSync(manifest) || !/direct-ffi\s*=\s*\[\]/u.test(cargo)) {
    fail("generated-napi-lowering", "generated N-API methods, manifest, or feature-gated direct qualification surface is missing");
  } else {
    pass("generated-napi-lowering", "canonical ABI emits safe N-API methods and keeps direct qualification feature-gated");
  }
}

// ---------------------------------------------------------------------------
// Gate 7: Standalone external-consumer fixture
// ---------------------------------------------------------------------------

function consumerFixtureGate(): void {
  const fixtureRoot = join(ROOT, "packages/tui-consumer-fixture/src");
  const allowedEntrypoints = new Set(["@iyon/tui", "@iyon/tui/testing"]);
  const violations: string[] = [];
  for (const file of walk(fixtureRoot)) {
    for (const spec of specifiersOf(readFileSync(file, "utf8"))) {
      if (!spec.startsWith(".") && !spec.startsWith("/")) {
        if (!allowedEntrypoints.has(spec) && !/^(bun|node):/.test(spec)) {
          violations.push(`${relative(ROOT, file)} -> "${spec}"`);
        }
        continue;
      }
      const resolved = resolveRelative(file, spec);
      if (resolved !== null && !resolved.startsWith(fixtureRoot)) {
        violations.push(`${relative(ROOT, file)} -> "${spec}" escapes fixture`);
      }
    }
  }
  const fixturePackageRoot = join(ROOT, "packages/tui-consumer-fixture");
  for (const file of walk(fixturePackageRoot)) {
    for (const specifier of specifiersOf(readFileSync(file, "utf8"))) {
      if (specifier.startsWith("@iyon/tui/") && specifier !== "@iyon/tui/testing") {
        violations.push(`${relative(ROOT, file)} -> undocumented deep import "${specifier}"`);
      }
    }
  }
  const packageManifest = JSON.parse(readFileSync(join(fixturePackageRoot, "package.json"), "utf8"));
  const dependencies = Object.keys(packageManifest.dependencies ?? {}).sort();
  if (dependencies.length !== 1 || dependencies[0] !== "@iyon/tui") {
    violations.push(`package dependencies are [${dependencies.join(", ")}]`);
  }
  if (violations.length > 0) fail("standalone-consumer-public-entrypoint", violations.join("; "));
  else pass("standalone-consumer-public-entrypoint", "fixture source and dependency manifest use only documented @iyon/tui root/testing entrypoints");
}

// ---------------------------------------------------------------------------
// Gate 8: H1B theme/style semantic guard
// ---------------------------------------------------------------------------

async function themeStyleSemanticGate(): Promise<void> {
  const privateFiles = new Set([
    "transport/structural/ir.ts",
    "transport/native/addon.ts",
    "transport/structural/native-view-abi.ts",
    "transport/structural/retained-dag.ts",
    "transport/structural/style-lowering.ts",
    "transport/structural/cold-lowering.ts",
    "transport/structural/encoding.ts",
    "transport/structural/view-bridge.ts",
  ]);
  const offenders: string[] = [];
  for (const file of walk(FRAMEWORK_SRC)) {
    if (privateFiles.has(relative(FRAMEWORK_SRC, file))) continue;
    const source = readFileSync(file, "utf8");
    if (/["'`]theme:/u.test(source)) offenders.push(relative(ROOT, file));
  }
  const fixtureRoot = join(ROOT, "packages/tui-consumer-fixture/src");
  for (const file of walk(fixtureRoot)) {
    if (/["'`]theme:/u.test(readFileSync(file, "utf8"))) offenders.push(relative(ROOT, file));
  }

  const mod = await import(join(FRAMEWORK_SRC, "index.ts"));
  const styleSpec = mod.StyleSpec as { readonly prototype: object } | undefined;
  const styleSpecTheme = styleSpec !== undefined
    && typeof (styleSpec.prototype as { readonly theme?: unknown }).theme === "function";
  if (styleSpecTheme) offenders.push("packages/iyon-tui/src/api/presentation/style.ts: StyleSpec.theme");

  const styleTypes = readFileSync(join(FRAMEWORK_SRC, "api/presentation/style.ts"), "utf8");
  if (/interface StyleSpecValue\s*\{[^}]*\btheme\??\s*:/u.test(styleTypes)) {
    offenders.push("packages/iyon-tui/src/api/presentation/style.ts: StyleSpecValue.theme");
  }

  if (offenders.length > 0) {
    fail("h1b-theme-style-semantics", `public theme/style convention remains: ${offenders.join(", ")}`);
  } else {
    pass("h1b-theme-style-semantics", "public styles use ColorSpec/StyleRef semantics; theme: lowering remains private");
  }
}

// ---------------------------------------------------------------------------
// Gate 9: H1C opaque framework-handle guard
// ---------------------------------------------------------------------------

function opaqueHandleGate(): void {
  const frameworkHandle = readFileSync(join(FRAMEWORK_SRC, "api/controls/framework-handle.ts"), "utf8");
  const handles = `${frameworkHandle}\n${readFileSync(join(FRAMEWORK_SRC, "runtime/handle-registry.ts"), "utf8")}\n${readFileSync(join(FRAMEWORK_SRC, "transport/native/resources.ts"), "utf8")}`;
  const index = readFileSync(join(FRAMEWORK_SRC, "index.ts"), "utf8");
  const offenders: string[] = [];

  if (!/export\s+abstract\s+class\s+FrameworkHandle[\s\S]*#frameworkHandleBrand/u.test(frameworkHandle)) {
    offenders.push("api/controls/framework-handle.ts: missing nominal FrameworkHandle brand");
  }
  if (!/new\s+WeakMap<object,\s*object>\(\)[\s\S]*function\s+nativeResourceOf/u.test(handles)) {
    offenders.push("transport/native/resources.ts: missing private native-resource registry");
  }
  if (/\bNativeHandle(?:Id)?\b/u.test(index)) {
    offenders.push("index.ts: NativeHandle/NativeHandleId remains a consumer export");
  }
  const sourceOffenders = walk(FRAMEWORK_SRC)
    .filter((file) => /\b(?:nativeObject|nativeHandle)\s*\(/u.test(readFileSync(file, "utf8")))
    .map((file) => relative(ROOT, file));
  if (sourceOffenders.length > 0) offenders.push(`public native unwrap methods: ${sourceOffenders.join(", ")}`);
  const castOffenders = walk(FRAMEWORK_SRC)
    .filter((file) => /as\s+unknown\s+as\s*\{[^}]*\bnative(?:Handle|Object)\b/u.test(readFileSync(file, "utf8")))
    .map((file) => relative(ROOT, file));
  if (castOffenders.length > 0) offenders.push(`untyped native unwrap casts: ${castOffenders.join(", ")}`);

  if (offenders.length > 0) {
    fail("h1c-opaque-handles", offenders.join("; "));
  } else {
    pass("h1c-opaque-handles", "framework handles are nominal and native resources unwrap through the private registry");
  }
}

// ---------------------------------------------------------------------------
// Gate 10: H1D control-construction/lifecycle guard
// ---------------------------------------------------------------------------

function controlLifecycleGate(): void {
  const runtime = readFileSync(join(FRAMEWORK_SRC, "runtime/runtime.ts"), "utf8");
  const textInput = readFileSync(join(FRAMEWORK_SRC, "api/controls/text-input.ts"), "utf8");
  const component = readFileSync(join(FRAMEWORK_SRC, "api/controls/view-slot.ts"), "utf8");
  const scrollPane = readFileSync(join(FRAMEWORK_SRC, "api/controls/scroll-pane.ts"), "utf8");
  const nativeHandles = readFileSync(join(FRAMEWORK_SRC, "transport/native/factories.ts"), "utf8");
  const offenders: string[] = [];

  if (!/setContent\(view: View \| \(\(\) => View\)\)/u.test(scrollPane)) {
    offenders.push("api/controls/scroll-pane.ts: ScrollPane.setContent does not expose builder support");
  }
  if (!/private\s+constructor\((?:resource|nativeHandle):\s*never(?:,|\))/u.test(textInput)
    || !/TEXT_INPUT_NATIVE_TOKEN/u.test(textInput)
    || /new\s+TextInput\s*\(/u.test(textInput)) {
    offenders.push("api/controls/text-input.ts: TextInput has a direct consumer constructor");
  }
  if (!/private\s+constructor\(host: never/u.test(component) || /new\s+ViewSlot\s*\(/u.test(runtime)) {
    offenders.push("api/controls/view-slot.ts/runtime/runtime.ts: ViewSlot is not Tui-factory-only");
  }
  if (!/private\s+constructor\(host: never/u.test(scrollPane) || /new\s+NativeScrollPane\s*\(/u.test(runtime)) {
    offenders.push("api/controls/scroll-pane.ts/runtime/runtime.ts: ScrollPane is not Tui-factory-only");
  }
  if (/nativeTui\.textInput/u.test(nativeHandles) || !/disposeOwnedHandles\(\)/u.test(runtime)) {
    offenders.push("runtime/runtime.ts/transport/native/factories.ts: Tui-owned control lifecycle is not centralized");
  }

  if (offenders.length > 0) {
    fail("h1d-control-lifecycle", offenders.join("; "));
  } else {
    pass("h1d-control-lifecycle", "controls use canonical Tui factories, builder contracts, and deterministic owner disposal");
  }
}

// ---------------------------------------------------------------------------
// Gate 11: H1E component-composition facade guard
// ---------------------------------------------------------------------------

function componentFacadeGate(): void {
  const view = readFileSync(join(FRAMEWORK_SRC, "api/view/view.ts"), "utf8");
  const facade = readFileSync(join(FRAMEWORK_SRC, "transport/structural/component-view.ts"), "utf8");
  const compose = readFileSync(join(FRAMEWORK_SRC, "composition/compose.ts"), "utf8");
  const runtime = readFileSync(join(FRAMEWORK_SRC, "runtime/runtime.ts"), "utf8");
  const contracts = publicContractSource();
  const component = readFileSync(join(FRAMEWORK_SRC, "api/controls/view-slot.ts"), "utf8");
  const scrollPane = readFileSync(join(FRAMEWORK_SRC, "api/controls/scroll-pane.ts"), "utf8");
  const textInput = readFileSync(join(FRAMEWORK_SRC, "api/controls/text-input.ts"), "utf8");
  const index = readFileSync(join(FRAMEWORK_SRC, "index.ts"), "utf8");
  const fixtureRoot = join(ROOT, "packages/tui-consumer-fixture/src");
  const stripComments = (source: string): string => source
    .replace(/\/\*[\s\S]*?\*\//gu, "")
    .replace(/\/\/.*$/gmu, "");
  const activeSources = [view, facade, compose, component, scrollPane, textInput];
  const offenders: string[] = [];

  if (/\bstatic\s+component\s*\(/u.test(stripComments(view))) {
    offenders.push("api/view/view.ts: View.component remains public");
  }
  if (activeSources.some((source) => /\bView\.component\s*\(/u.test(stripComments(source)))) {
    offenders.push("framework controls/composition still construct View.component directly");
  }
  if (!/export\s+function\s+componentViewFor\s*\(/u.test(facade)) {
    offenders.push("transport/structural/component-view.ts: missing private component placement lowering");
  }
  if (!/componentViewFor\(slot\)/u.test(runtime) || /\bslot\.view\(\)/u.test(runtime)) {
    offenders.push("runtime/runtime.ts: internal component projection participates in parent composition");
  }
  for (const [name, source] of [["api/controls/view-slot.ts", component], ["api/controls/scroll-pane.ts", scrollPane], ["api/controls/text-input.ts", textInput]] as const) {
    if (!/composeComponent\(this\)/u.test(source)) offenders.push(`${name}: control.view() bypasses retained composition`);
  }
  if (/\bexport\s+(?:class|interface|type)\s+Component\b/u.test(contracts) || /\bexport\s+class\s+Component\b/u.test(component)) {
    offenders.push("Component is still exposed as a concrete or structural root abstraction");
  }
  if (/\bexport\s*\{[^}]*\bComponent\b/u.test(index)) {
    offenders.push("index.ts: concrete Component remains a root export");
  }
  if (walk(fixtureRoot).some((file) => /\bView\.component\s*\(/u.test(stripComments(readFileSync(file, "utf8"))))) {
    offenders.push("standalone fixture still uses View.component instead of control.view()");
  }

  if (offenders.length > 0) {
    fail("h1e-component-facade", offenders.join("; "));
  } else {
    pass("h1e-component-facade", "controls compose through view() and placement lowering remains private");
  }
}

// ---------------------------------------------------------------------------
// Gate 12: H1F typed output/event semantics guard
// ---------------------------------------------------------------------------

function typedOutputEventGate(): void {
  const output = readFileSync(join(FRAMEWORK_SRC, "api/controls/output.ts"), "utf8");
  const component = readFileSync(join(FRAMEWORK_SRC, "api/extensions/traits/component.ts"), "utf8");
  const textInput = readFileSync(join(FRAMEWORK_SRC, "api/controls/text-input.ts"), "utf8");
  const runtime = readFileSync(join(FRAMEWORK_SRC, "runtime/runtime.ts"), "utf8");
  const testing = readFileSync(join(FRAMEWORK_SRC, "testing/index.ts"), "utf8");
  const events = readFileSync(join(FRAMEWORK_SRC, "runtime/events.ts"), "utf8");
  const index = readFileSync(join(FRAMEWORK_SRC, "index.ts"), "utf8");
  const contracts = output + component + textInput + runtime + testing + events;
  const offenders: string[] = [];

  if (!/export\s+class\s+Output<[^>]+>[\s\S]*#outputBrand[\s\S]*declare\s+private\s+readonly\s+outputType[\s\S]*private\s+constructor/u.test(output)) {
    offenders.push("api/controls/output.ts: missing opaque typed Output<T> identity");
  }
  if (/OutputHandle\b/u.test(contracts)) {
    offenders.push("legacy OutputHandle remains in the framework facade");
  }
  const outputClass = output.match(/export\s+class\s+Output<T>[\s\S]*?\n\}/u)?.[0] ?? "";
  if (/export\s+type\s+Output\s*=/u.test(output) || /\bpayload\b/u.test(outputClass)) {
    offenders.push("api/controls/output.ts: Output is still a record or exposes a fake payload");
  }
  if (!/submitted\(\):\s*Output<string>/u.test(textInput)) {
    offenders.push("TextInput.submitted() does not expose Output<string>");
  }
  if (!/route\(output:\s*Output<string>,/u.test(runtime) || !/route\(output:\s*Output<string>,/u.test(testing)) {
    offenders.push("runtime route contracts do not consume typed Output<string>");
  }
  if (!/emit<T>\(output:\s*Output<T>,\s*payload:\s*T\)/u.test(component)) {
    offenders.push("ComponentContext.emit does not separate typed channel and payload");
  }
  if (/readonly\s+output:\s*Output\b/u.test(contracts)) {
    offenders.push("InteractionResult still carries a record-shaped output field");
  }
  if (existsSync(join(FRAMEWORK_SRC, "output.ts"))) offenders.push("output.ts: parallel string-keyed OutputRouter remains");
  if (/OutputRouter|RouteConflict|keyEvent|pasteEvent|resizeEvent|terminateEvent/u.test(index)) {
    offenders.push("index.ts: standalone router or test-input event constructors remain exported");
  }
  if (!/export\s+type\s+TuiEvent\s*=\s*OutputEvent\s*\|\s*TerminateEvent/u.test(events)) {
    offenders.push("runtime/events.ts: routed output/termination event union is missing");
  }

  if (offenders.length > 0) {
    fail("h1f-typed-output-events", offenders.join("; "));
  } else {
    pass("h1f-typed-output-events", "Output<T> is opaque, payloads are separate, and test-input constructors are not runtime exports");
  }
}

// ---------------------------------------------------------------------------
// Gate 13: H1G false-alias and compatibility removal guard
// ---------------------------------------------------------------------------

function falseAliasGate(): void {
  const stream = readFileSync(join(FRAMEWORK_SRC, "api/controls/text-stream.ts"), "utf8");
  const runtime = readFileSync(join(FRAMEWORK_SRC, "runtime/runtime.ts"), "utf8");
  const testing = readFileSync(join(FRAMEWORK_SRC, "testing/index.ts"), "utf8");
  const native = readFileSync(join(FRAMEWORK_SRC, "transport/native/addon.ts"), "utf8");
  const index = readFileSync(join(FRAMEWORK_SRC, "index.ts"), "utf8");
  const tests = join(ROOT, "packages/iyon-tui/tests");
  const stage = readFileSync(join(ROOT, "packages/iyon-tui/scripts/stage-native.ts"), "utf8");
  const publicSources = stream + runtime + testing + index;
  const offenders: string[] = [];

  if (/\bStreamPane\b/u.test(publicSources)) offenders.push("StreamPane remains a TypeScript TextStream alias");
  if (/\b(?:TuiOperation|TuiFailure)\b/u.test(publicSources)) offenders.push("no-op TuiOperation/TuiFailure aliases remain public");
  if (/\bnextAction\b/u.test(runtime + testing)) offenders.push("nextAction remains in the public runtime or harness facade");
  if (walk(tests).some((file) => /\bnextAction\b/u.test(readFileSync(file, "utf8")))) offenders.push("framework tests still use nextAction");
  if (/\b(?:nextAction|waitForAction)\s*\(/u.test(native)) offenders.push("native host contract still declares compatibility action aliases");
  if (/\b(?:next_action|wait_for_action)\s*\(/u.test(readFileSync(join(ROOT, "crates/iyon-tui/src/application/host.rs"), "utf8"))) {
    offenders.push("Rust host still exposes compatibility action aliases");
  }
  if (/\b(?:next_action|wait_for_action)\s*\(/u.test(readFileSync(join(ROOT, "crates/iyon-tui-native/src/tui.rs"), "utf8"))) {
    offenders.push("native binding still exposes compatibility action aliases");
  }
  if (/\b(?:export\s+const\s+tuiSmoke|export\s*\{[^}]*\btuiSmoke\b)/u.test(index)) offenders.push("application-facing tuiSmoke remains a root export");
  if (!/addon\.tuiSmoke\?\.\(\)/u.test(stage)) offenders.push("native staging smoke probe was removed with the root marker");

  if (offenders.length > 0) {
    fail("h1g-false-aliases", offenders.join("; "));
  } else {
    pass("h1g-false-aliases", "false aliases and compatibility action/smoke exports are removed while native staging remains probed");
  }
}

// ---------------------------------------------------------------------------
// Gate 14: H1H root and testing-subpath hygiene guard
// ---------------------------------------------------------------------------

function rootAndTestingSurfaceGate(): void {
  const index = readFileSync(join(FRAMEWORK_SRC, "index.ts"), "utf8");
  const runtime = readFileSync(join(FRAMEWORK_SRC, "runtime/runtime.ts"), "utf8");
  const testing = readFileSync(join(FRAMEWORK_SRC, "testing/index.ts"), "utf8");
  const packageManifest = JSON.parse(readFileSync(join(ROOT, "packages/iyon-tui/package.json"), "utf8")) as {
    exports?: Record<string, unknown>;
  };
  const workspaceManifest = JSON.parse(readFileSync(join(ROOT, "package.json"), "utf8")) as {
    exports?: Record<string, unknown>;
  };
  const forbiddenRootNames = /\b(?:AppHarness|createAppHarness|NativeOutputHandle|NativeViewSlot|NativeScrollPane|(?:Renderer|Projector|TextVisitor|TextRewriter|StreamingSource)Adapter|ComponentAdapterBridge|FocusController|InteractionRouter)\b/u;
  const testOnlyMethods = /^\s+(?:enqueue|screenRows|nativeHistoryRows|styleAt|cellXOfText|advance|current|exited)\s*\(/mu;
  const offenders: string[] = [];

  if (forbiddenRootNames.test(index)) offenders.push("implementation, adapter, interaction, or testing names remain root-exported");
  if (testOnlyMethods.test(runtime)) offenders.push("Tui still declares direct test/inspection methods");
  if (typeof packageManifest.exports?.["./testing"] !== "string") offenders.push("packages/iyon-tui does not export ./testing");
  if (packageManifest.exports?.["./testing"] !== "./src/testing/index.ts") offenders.push("packages/iyon-tui ./testing does not target src/testing/index.ts");
  if (workspaceManifest.exports?.["./testing"] !== "./packages/iyon-tui/src/testing/index.ts") offenders.push("workspace ./testing export is not aligned with the package");
  if (!/export\s+class\s+AppHarness\b/u.test(testing) || !/export\s+const\s+createAppHarness\b/u.test(testing)) {
    offenders.push("testing subpath does not expose AppHarness and createAppHarness");
  }
  if (/export\s+(?:const|function|class)\s+tuiTestingAccess\b/u.test(testing)) {
    offenders.push("testing subpath exposes the private Tui testing-access seam");
  }
  if (existsSync(join(FRAMEWORK_SRC, "interaction.ts"))) offenders.push("unused TypeScript interaction facade remains");

  if (offenders.length > 0) {
    fail("h1h-root-testing-surface", offenders.join("; "));
  } else {
    pass("h1h-root-testing-surface", "the root is semantic, testing helpers live under ./testing, and Tui test hooks are private");
  }
}

// ---------------------------------------------------------------------------
// Gate 15: H1I runtime contract and authoritative size guard
// ---------------------------------------------------------------------------

function runtimeContractGate(): void {
  const runtime = readFileSync(join(FRAMEWORK_SRC, "runtime/runtime.ts"), "utf8");
  const testing = readFileSync(join(FRAMEWORK_SRC, "testing/index.ts"), "utf8");
  const runtimeBody = runtime.match(/export interface TuiRuntime\s*\{([\s\S]*?)\n\}/u)?.[1] ?? "";
  const candidates = [
    "createHistory",
    "createTextInput",
    "createViewSlot",
    "createScrollPane",
    "interceptPaste",
    "forwardPaste",
    "setTheme",
  ];
  const optional = candidates.filter((name) => new RegExp(`\\b${name}\\?\\s*\\(`, "u").test(runtimeBody));
  const offenders: string[] = [];

  if (optional.length > 0) offenders.push(`TuiRuntime keeps optional methods: ${optional.join(", ")}`);
  if (/private readonly (?:width|height)\s*:/u.test(runtime)) offenders.push("Tui stores terminal dimensions as readonly open-time values");
  if (!/this\.host\.resize\(width, height\);\s*this\.width = width;\s*this\.height = height;/su.test(runtime)) {
    offenders.push("Tui does not publish dimensions after a successful host resize");
  }
  if (/private readonly options|this\.options\.(?:width|height)/u.test(testing)) {
    offenders.push("AppHarness keeps an independent mutable size record");
  }
  if (!/get size\(\): TerminalMetadata\s*\{\s*return this\.tui\.size;\s*\}/su.test(testing)) {
    offenders.push("AppHarness size is not delegated to the authoritative Tui size");
  }

  if (offenders.length > 0) {
    fail("h1i-runtime-contract", offenders.join("; "));
  } else {
    pass("h1i-runtime-contract", "runtime capabilities are required and Tui/AppHarness expose one authoritative post-resize size");
  }
}

// ---------------------------------------------------------------------------
// Gate 16: H1J public contract parity guard
// ---------------------------------------------------------------------------

function contractParityGate(): void {
  const sources = new Map([
    ["contracts", publicContractSource()],
    ["runtime/runtime.ts", readFileSync(join(FRAMEWORK_SRC, "runtime/runtime.ts"), "utf8")],
    ["api/controls/history.ts", readFileSync(join(FRAMEWORK_SRC, "api/controls/history.ts"), "utf8")],
    ["api/controls/text-input.ts", readFileSync(join(FRAMEWORK_SRC, "api/controls/text-input.ts"), "utf8")],
    ["api/controls/text-stream.ts", readFileSync(join(FRAMEWORK_SRC, "api/controls/text-stream.ts"), "utf8")],
    ["api/controls/view-slot.ts", readFileSync(join(FRAMEWORK_SRC, "api/controls/view-slot.ts"), "utf8")],
    ["api/controls/scroll-pane.ts", readFileSync(join(FRAMEWORK_SRC, "api/controls/scroll-pane.ts"), "utf8")],
    ["testing/index.ts", readFileSync(join(FRAMEWORK_SRC, "testing/index.ts"), "utf8")],
    ["view.ts", readFileSync(join(FRAMEWORK_SRC, "api/view/view.ts"), "utf8")],
    ["style.ts", readFileSync(join(FRAMEWORK_SRC, "api/presentation/style.ts"), "utf8")],
    ["theme.ts", readFileSync(join(FRAMEWORK_SRC, "api/presentation/theme.ts"), "utf8")],
    ["text.ts", readFileSync(join(FRAMEWORK_SRC, "api/content/text.ts"), "utf8")],
    ["style-lowering.ts", readFileSync(join(FRAMEWORK_SRC, "transport/structural/style-lowering.ts"), "utf8")],
    ["native-view-abi.ts", readFileSync(join(FRAMEWORK_SRC, "transport/structural/native-view-abi.ts"), "utf8")],
  ]);
  const index = readFileSync(join(FRAMEWORK_SRC, "index.ts"), "utf8");
  const native = readFileSync(join(ROOT, "crates/iyon-tui-native/src/tui.rs"), "utf8");
  const rustOutput = readFileSync(join(ROOT, "crates/iyon-tui/src/output/handle.rs"), "utf8");
  const rustComponent = readFileSync(join(ROOT, "crates/iyon-tui/src/component/mod.rs"), "utf8");
  const runtime = sources.get("runtime/runtime.ts")!;
  const types = sources.get("contracts")!;
  const theme = sources.get("theme.ts")!;
  const view = sources.get("view.ts")!;
  const style = sources.get("style.ts")!;
  const text = sources.get("text.ts")!;
  const styleInternals = sources.get("style-lowering.ts")!;
  const nativeViewAbi = sources.get("native-view-abi.ts")!;
  const kernel = readFileSync(join(ROOT, "crates/iyon-tui/src/application/kernel.rs"), "utf8");
  const offenders: string[] = [];

  const implementations: readonly [string, RegExp][] = [
    ["runtime/runtime.ts", /export\s+class\s+Tui\s+implements\s+TuiRuntime\b/u],
    ["api/controls/history.ts", /export\s+class\s+History[\s\S]*implements\s+HistoryContract\b/u],
    ["api/controls/text-input.ts", /export\s+class\s+TextInput[\s\S]*implements\s+TextInputContract\b/u],
    ["api/controls/text-stream.ts", /export\s+class\s+TextStream[\s\S]*implements\s+TextStreamContract\b/u],
    ["api/controls/view-slot.ts", /export\s+class\s+ViewSlot[\s\S]*implements\s+ViewSlotContract\b/u],
    ["api/controls/scroll-pane.ts", /export\s+class\s+NativeScrollPane[\s\S]*implements\s+ScrollPaneContract\b/u],
    ["testing/index.ts", /export\s+class\s+AppHarness\s+implements\s+AppHarnessContract\b/u],
  ];
  for (const [file, pattern] of implementations) {
    if (!pattern.test(sources.get(file)!)) offenders.push(`${file}: implementation no longer declares contract parity`);
  }

  if (!/interface\s+ScrollPane[\s\S]*setContent\(view:\s*View\s*\|\s*\(\(\)\s*=>\s*View\)\):\s*void/u.test(types)) {
    offenders.push("ScrollPane contract does not include retained builder content");
  }
  if (/\b(?:ThemeDefinition|ThemeStyleEntry|ThemeColorEntry)\b/u.test(index)) {
    offenders.push("index.ts: native-bound ThemeDefinition records remain root-exported");
  }
  if (!/interface\s+AppHarness[\s\S]*now\(\):\s*number/u.test(types)) {
    offenders.push("AppHarness contract omits its public deterministic clock accessor");
  }
  if (!/setContent\(viewOrBuilder:\s*View\s*\|\s*\(\(\)\s*=>\s*View\)\):\s*void/u.test(sources.get("api/controls/scroll-pane.ts")!)) {
    offenders.push("ScrollPane implementation does not include retained builder content");
  }
  if (!/interface\s+TextInputOptions\s*\{[\s\S]*border\?:\s*BorderSpec/u.test(types)
    || !/const\s+border\s*=\s*options\.border[\s\S]*host\.textInput\(options\.multiline,\s*border\)/u.test(runtime)) {
    offenders.push("TextInput border semantics are not present in both contract and host lowering");
  }
  if (/materialize\(\)\s*:\s*ThemeDefinition/u.test(theme) || !/themeDefinitionFor\(theme\)/u.test(runtime)) {
    offenders.push("Theme lowering is still public or bypasses its private projection seam");
  }

  const semanticAttributes = ["bold", "dim", "italic", "underline", "reversed", "strikethrough"];
  for (const attribute of semanticAttributes) {
    if (!types.includes(`"${attribute}"`)) offenders.push(`api/presentation/style.ts: missing TextAttribute ${attribute}`);
    if (!native.includes(`"${attribute}" => Some(`)) offenders.push(`native/tui.rs: missing TextAttribute ${attribute} lowering`);
  }
  if (!/attributes:\s*Readonly<Partial<Record<TextAttribute,\s*boolean>>>/u.test(types)) {
    offenders.push("StyleSpecValue attributes are not closed to the native vocabulary");
  }
  if (!/attribute\(name:\s*TextAttribute/u.test(style) || !/textAttribute\(name:\s*TextAttribute/u.test(view)) {
    offenders.push("StyleSpec/View text-attribute methods are not aligned with TextAttribute");
  }
  if (!/function\s+styleAttributesFor[\s\S]*validateTextAttribute\(name\)/u.test(styleInternals)) {
    offenders.push("style lowering does not validate the closed native text-attribute vocabulary");
  }
  if (!/optional_u16_value\(insets, "top"\)/u.test(native)
    || !/optional_u16_value\(insets, "right"\)/u.test(native)
    || !/optional_u16_value\(insets, "bottom"\)/u.test(native)
    || !/optional_u16_value\(insets, "left"\)/u.test(native)) {
    offenders.push("NativeTextStream does not honor optional stream inset fields");
  }
  if (!/impl\s+NativeTextInput[\s\S]*?self\.alive\.swap\(false,\s*Ordering::AcqRel\)[\s\S]*?host\.retire\(\)/u.test(native)) {
    offenders.push("NativeTextInput disposal does not request deferred component retirement");
  }
  if (!/export\s+function\s+tryNativeMaterialize[\s\S]*?tuiViewAbiDecodeRef/u.test(nativeViewAbi)) {
    offenders.push("cold materialization does not have a paint-free native direct-decoder fallback");
  }
  if (!kernel.includes("self.reap_retired_components();")) {
    offenders.push("successful frame preparation does not reap deferred component retirements");
  }

  const textRoles = ["paragraph", "heading", "blockQuote", "list", "listItem", "codeBlock", "table", "tableRow", "tableCell", "thematicBreak", "rawBlock", "container", "strong", "emphasis", "strikethrough", "underline", "superscript", "subscript", "smallCaps", "inlineCode", "link", "image", "rawInline"];
  const textParts = ["listMarker", "taskMarker", "quoteMarker", "codeLabel", "tableRule", "thematicRule", "imageFallback"];
  for (const role of textRoles) {
    if (!types.includes(`"${role}"`)) offenders.push(`api/content/text.ts: missing TextRole ${role}`);
    if (!native.includes(`"${role}" =>`)) offenders.push(`native/tui.rs: missing TextRole ${role} lowering`);
  }
  for (const part of textParts) {
    if (!types.includes(`"${part}"`)) offenders.push(`api/content/text.ts: missing TextPart ${part}`);
    if (!native.includes(`"${part}" =>`)) offenders.push(`native/tui.rs: missing TextPart ${part} lowering`);
  }
  if (!/roles\?:\s*readonly\s*TextRole\[\]/u.test(types) || !/parts\?:\s*readonly\s*TextPart\[\]/u.test(types)) {
    offenders.push("TextSelectorValue roles/parts are not closed semantic vocabularies");
  }
  if (!/role\(role:\s*TextRole\)/u.test(text) || !/part\(part:\s*TextPart\)/u.test(text)) {
    offenders.push("TextSelector methods do not use the closed semantic vocabularies");
  }
  if (!/validateTextName\(namespace, "annotation namespace"\)/u.test(text)
    || !/validateTextName\(language, "language"\)/u.test(text)
    || !/validateTextName\(origin, "text origin"\)/u.test(text)
    || !/validateTextName\(format, "text format"\)/u.test(text)) {
    offenders.push("TextSelector does not validate native semantic-name dimensions");
  }

  if (!/export\s+interface\s+BorderGlyphs\s*\{[\s\S]*topLeft:\s*string[\s\S]*bottomRight:\s*string/u.test(types)) {
    offenders.push("BorderGlyphs does not name the complete native border record");
  }
  if (/static\s+(?:__rawGrid|axisSetChildForTransport|axisSpliceForTransport|gridSetCellForTransport|__composedAxis|textLayoutAtNativePathForTransport|textLayoutTransactionForTransport)\b/u.test(view)) {
    offenders.push("View still exposes retained transport constructors as public statics");
  }
  if (!/createViewSlot\(initialView:\s*View\):\s*ViewSlotContract/u.test(runtime)
    || !/createViewSlot\(initial:\s*View\):\s*ViewSlotContract/u.test(sources.get("testing/index.ts")!)) {
    offenders.push("ViewSlot factory signatures expose implementation classes instead of semantic contracts");
  }
  if (!/pub\s+struct\s+Output<T:/u.test(rustOutput) || !/pub\s+trait\s+Component/u.test(rustComponent)) {
    offenders.push("Rust typed Output or Component semantic reference is missing");
  }
  if (!/isRetainedConstruction\(\)/u.test(view) || !/OwnedBuilderRoot\.start/u.test(sources.get("api/controls/view-slot.ts")!)
    || !/OwnedBuilderRoot\.start/u.test(sources.get("api/controls/scroll-pane.ts")!)) {
    offenders.push("facade controls no longer use the shared retained composition architecture");
  }
  if (!/renderCanonical[\s\S]*renderDirect/u.test(runtime)) {
    offenders.push("direct and retained render ownership paths were merged");
  }

  if (offenders.length > 0) {
    fail("h1j-contract-parity", offenders.join("; "));
  } else {
    pass("h1j-contract-parity", "TypeScript contracts, native lowering, Rust semantic references, and retained paths remain aligned");
  }
}

// ---------------------------------------------------------------------------
// Gate 17: Public API surface guard
// ---------------------------------------------------------------------------

const BANNED_SURFACE_NAMES = [
  "Agent",
  "Assistant",
  "Provider",
  "Prompt",
  "ModelTurn",
  "ToolCall",
  "ToolExecution",
  "Approval",
  "Conversation",
  "Transcript",
  "KernelSession",
  "Steering",
  "ReasoningEffort",
];

async function publicSurfaceGate(): Promise<void> {
  // TypeScript facade exports vs frozen S0 snapshot.
  const baselinePath = join(ROOT, "docs/repository-separation/s0/api-surface.json");
  const baseline = JSON.parse(readFileSync(baselinePath, "utf8"));
  const packageManifest = JSON.parse(readFileSync(join(ROOT, "packages/iyon-tui/package.json"), "utf8")) as {
    name?: string;
  };
  const typescriptSnapshot = baseline.typescriptTui as {
    currentPackage?: string;
    currentSubpath?: string;
    source?: string;
    sourceSha256?: string;
  };
  const identityErrors: string[] = [];
  if (packageManifest.name !== "@iyon/tui") identityErrors.push(`package name is ${JSON.stringify(packageManifest.name)}`);
  if (typescriptSnapshot.currentPackage !== "@iyon/tui") identityErrors.push("snapshot currentPackage is not @iyon/tui");
  if (typescriptSnapshot.currentSubpath !== ".") identityErrors.push("snapshot currentSubpath is not the package root");
  if (typescriptSnapshot.source !== "packages/iyon-tui/src/index.ts") identityErrors.push("snapshot source is not the canonical TUI root");
  if (typescriptSnapshot.source === undefined || !existsSync(join(ROOT, typescriptSnapshot.source))) {
    identityErrors.push("snapshot source does not exist");
  } else {
    const sourceHash = createHash("sha256").update(readFileSync(join(ROOT, typescriptSnapshot.source))).digest("hex");
    if (typescriptSnapshot.sourceSha256 !== sourceHash) identityErrors.push("snapshot sourceSha256 is stale");
  }
  if (identityErrors.length > 0) fail("tui-package-identity", identityErrors.join("; "));
  else pass("tui-package-identity", "@iyon/tui is the canonical framework package and snapshot root");
  const mod = await import(join(FRAMEWORK_SRC, "index.ts"));
  const values = Object.keys(mod).sort();
  const typeExports: string[] = [];
  const source = readFileSync(join(FRAMEWORK_SRC, "index.ts"), "utf8");
  for (const block of source.matchAll(/export\s+type\s*\{(.*?)\}\s*from/gs)) {
    for (const item of block[1]!.split(",")) {
      const name = item.replace(/\/\/.*$/, "").trim();
      if (name) typeExports.push(name.split(" as ").pop()!.trim());
    }
  }
  const types = [...new Set(typeExports)].sort();

  const expectedValues: string[] = baseline.typescriptTui.valueExports;
  const expectedTypes: string[] = baseline.typescriptTui.typeExports;
  const addedValues = values.filter((v) => !expectedValues.includes(v));
  const removedValues = expectedValues.filter((v) => !values.includes(v));
  const addedTypes = types.filter((v) => !expectedTypes.includes(v));
  const removedTypes = expectedTypes.filter((v) => !types.includes(v));

  if ([...addedValues, ...removedValues, ...addedTypes, ...removedTypes].length > 0) {
    fail(
      "ts-surface-snapshot",
      `drift vs S0 snapshot — added values [${addedValues}] removed values [${removedValues}] added types [${addedTypes}] removed types [${removedTypes}]; update docs/repository-separation/s0/api-surface.json deliberately`,
    );
  } else {
    pass("ts-surface-snapshot", `${values.length} value + ${types.length} type exports match the frozen S0 snapshot`);
  }

  const bannedRe = new RegExp(`^(${BANNED_SURFACE_NAMES.join("|")})$`, "i");
  const surfaceHits = [...values, ...types].filter((name) => bannedRe.test(name));
  if (surfaceHits.length > 0) fail("ts-surface-banned-names", `application-specific exports: ${surfaceHits.join(", ")}`);
  else pass("ts-surface-banned-names", "no application concepts in the TypeScript TUI surface");

  // Rust mapping surface vs committed snapshot.
  const mappingPath = join(ROOT, "tools/api-surface/mappings/iyon-tui.toml");
  const ids = [...readFileSync(mappingPath, "utf8").matchAll(/^item_id\s*=\s*"([^"]+)"/gm)].map((m) => m[1]!).sort();
  const snapshotPath = join(ROOT, "tools/ownership/snapshots/iyon-tui-rust-surface.txt");
  const snapshotIds = existsSync(snapshotPath)
    ? readFileSync(snapshotPath, "utf8").split("\n").map((l) => l.trim()).filter(Boolean).sort()
    : [];
  const drifted = ids.length !== snapshotIds.length || ids.some((id, i) => id !== snapshotIds[i]);
  if (drifted) {
    fail("rust-surface-snapshot", `mapping drift vs tools/ownership/snapshots/iyon-tui-rust-surface.txt (${ids.length} records); regenerate deliberately`);
  } else {
    pass("rust-surface-snapshot", `${ids.length} mapped Rust items match the committed snapshot`);
  }
  const lastSegment = (id: string) => id.split(/[.:]/).pop() ?? id;
  const bannedSet = new Set(BANNED_SURFACE_NAMES.map((n) => n.toLowerCase()));
  const rustHits = ids.filter((id) => bannedSet.has(lastSegment(id).toLowerCase()));
  if (rustHits.length > 0) fail("rust-surface-banned-names", `application-specific mapped items: ${rustHits.join(", ")}`);
  else pass("rust-surface-banned-names", "no application concepts in the mapped iyon-tui Rust surface");
}

// ---------------------------------------------------------------------------

rustDependencyGate();
tsImportGate();
cut2OwnershipGate();
cut3OwnershipGate();
cut4RootCleanupGate();
cut5ImportBoundaryGate();
h3bCompositionTransportGate();
h3cStructuralCompositionGate();
cut5RootPublicationGate();
cut5ModuleIdentityGate();
cut5PackagePublicationGate();
napiTransportGate();
consumerFixtureGate();
await themeStyleSemanticGate();
opaqueHandleGate();
controlLifecycleGate();
componentFacadeGate();
typedOutputEventGate();
falseAliasGate();
rootAndTestingSurfaceGate();
runtimeContractGate();
contractParityGate();
await publicSurfaceGate();

if (failed) {
  console.log("\nOWNERSHIP CHECKS FAILED");
  process.exit(1);
}
console.log("\nALL OWNERSHIP CHECKS PASSED");
