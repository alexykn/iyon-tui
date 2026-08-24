/**
 * S1 ownership gates (IYON-TUI-REPOSITORY-SEPARATION-HANDOFF §7).
 *
 * Machine-checks framework/application ownership while both still live in one
 * checkout. Run with `bun run check:ownership`. Every failure names the file
 * and rule; no gate relies on prose alone.
 */

import { readFileSync, existsSync, statSync, readdirSync } from "node:fs";
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
  return [
    ...[...source.matchAll(/(?:^|\s)from\s+"([^"]+)"/g)].map((m) => m[1]!),
    ...[...source.matchAll(/import\(\s*"([^"]+)"\s*\)/g)].map((m) => m[1]!),
    ...[...source.matchAll(/(?:^|\s)import\s+"([^"]+)"/g)].map((m) => m[1]!),
  ];
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

  // The mixed native crate is module-gated until S3 splits it.
  const tuiNativePaths = [
    "crates/iyon-native/src/tui.rs",
    "crates/iyon-native/src/tui",
    "crates/iyon-native/src/generated",
    "crates/iyon-native/tests/generated_view_abi.rs",
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

  const appNativeFiles = Array.from(new Bun.Glob("src/*.rs").scanSync({ cwd: join(ROOT, "crates/iyon-native") }))
    .filter((f) => !f.startsWith("tui"))
    .map((f) => join(ROOT, "crates/iyon-native", f));
  const appOffenders = appNativeFiles.filter((f) => /\bcrate::tui\b/.test(readFileSync(f, "utf8")));
  if (appOffenders.length > 0)
    fail("app-native-module-purity", `application native modules import TUI ABI: ${appOffenders.map((f) => relative(ROOT, f)).join(", ")}`);
  else pass("app-native-module-purity", "application native modules reference no TUI module");

  const tuiRustOffenders = Array.from(
    new Bun.Glob("**/*.rs").scanSync({ cwd: join(ROOT, "crates/iyon-tui") }),
  ).filter((f) => /\biyon_(core|api)\b/.test(readFileSync(join(ROOT, "crates/iyon-tui", f), "utf8")));
  if (tuiRustOffenders.length > 0) fail("framework-rust-purity", `crates/iyon-tui references app crates: ${tuiRustOffenders.join(", ")}`);
  else pass("framework-rust-purity", "crates/iyon-tui sources reference no application crate");
}

// ---------------------------------------------------------------------------
// Gate 2: TypeScript import direction
// ---------------------------------------------------------------------------

const FRAMEWORK_SRC = join(ROOT, "packages/iyon-runtime/src/tui");
const NATIVE_CONTRACT = resolve(ROOT, "packages/iyon-runtime/src/native.ts");

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
      if (resolved.startsWith(FRAMEWORK_SRC)) continue;
      if (resolved === NATIVE_CONTRACT) {
        seams.push(`${relative(ROOT, file)} -> ../native.ts`);
        continue;
      }
      violations.push(`${relative(ROOT, file)} -> "${spec}" escapes framework`);
    }
  }
  if (violations.length > 0) fail("framework-ts-import-direction", violations.join("; "));
  else pass("framework-ts-import-direction", `${files.length} files import only framework modules (+${seams.length} recorded native-contract seams)`);

  // Application production sources must use public TUI entrypoints only.
  const appRoots = [
    "plugins",
    "packages/iyon-cli/src",
    "packages/iyon-plugins/src",
    "packages/iyon-sdk/src",
  ].map((p) => join(ROOT, p));
  const bannedInternals =
    /retained_dag|view_abi|native_view_policy|internal-composition|tui-execution|execution-context|persistent_seq|packed(_v[34])?_meta/;
  const subpathImport = /^@iyon\/runtime\/tui\/.+$/;

  const appViolations: string[] = [];
  let appFilesChecked = 0;
  const seenAppFiles = new Set<string>();
  for (const root of appRoots) {
    if (!existsSync(root)) continue;
    for (const file of walk(root)) {
      if (file.includes("/test/") || file.includes("/tests/") || file.endsWith(".test.ts")) continue;
      seenAppFiles.add(file);
    }
  }
  for (const file of seenAppFiles) {
    appFilesChecked += 1;
    const source = readFileSync(file, "utf8");
    const specs = specifiersOf(source);
    for (const spec of specs) {
      const rel = relative(ROOT, file);
      // Path-pattern rules apply to bare specifiers only; relative imports are
      // judged by where they actually resolve.
      if (!spec.startsWith(".") && !spec.startsWith("/")) {
        if (bannedInternals.test(spec)) appViolations.push(`${rel} -> "${spec}"`);
        else if (subpathImport.test(spec)) appViolations.push(`${rel} -> "${spec}"`);
        continue;
      }
      const resolved = resolveRelative(file, spec);
      if (resolved !== null && resolved.startsWith(FRAMEWORK_SRC) && resolved !== join(FRAMEWORK_SRC, "index.ts")) {
        appViolations.push(`${rel} -> "${spec}" (non-public framework path)`);
      }
    }
  }
  if (appViolations.length > 0) fail("app-ts-public-entrypoints-only", appViolations.join("; "));
  else pass("app-ts-public-entrypoints-only", `${appFilesChecked} application source files use public TUI surfaces only`);

  // Runtime non-TUI sources may enter the framework only through tui/index.ts,
  // except virtual-modules.ts — the recorded S4/S5 bundler-compatibility seam.
  const runtimeRoot = join(ROOT, "packages/iyon-runtime/src");
  const runtimeViolations: string[] = [];
  const runtimeSeams: string[] = [];
  for (const file of walk(runtimeRoot)) {
    if (file.startsWith(FRAMEWORK_SRC)) continue;
    if (file.endsWith(".test.ts")) continue;
    const rel = relative(ROOT, file);
    const isVirtualModuleSeam =
      rel === "packages/iyon-runtime/src/virtual-modules.ts" ||
      rel === "packages/iyon-runtime/src/virtual-modules.d.ts";
    const specs = specifiersOf(readFileSync(file, "utf8"));
    for (const spec of specs) {
      if (!spec.startsWith(".")) continue;
      const resolved = resolveRelative(file, spec);
      if (resolved !== null && resolved.startsWith(FRAMEWORK_SRC) && resolved !== join(FRAMEWORK_SRC, "index.ts")) {
        if (isVirtualModuleSeam) runtimeSeams.push(`virtual-modules.ts -> "${spec}"`);
        else runtimeViolations.push(`${rel} -> "${spec}"`);
      }
    }
  }
  if (runtimeViolations.length > 0) fail("runtime-ts-public-entrypoints-only", runtimeViolations.join("; "));
  else
    pass(
      "runtime-ts-public-entrypoints-only",
      `runtime non-TUI sources enter via tui/index.ts only (${runtimeSeams.length} recorded virtual-module alias seams)`,
    );
}

// ---------------------------------------------------------------------------
// Gate 3: Public API surface guard
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
await publicSurfaceGate();

if (failed) {
  console.log("\nOWNERSHIP CHECKS FAILED");
  process.exit(1);
}
console.log("\nALL OWNERSHIP CHECKS PASSED");
