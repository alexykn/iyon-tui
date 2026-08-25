/**
 * Standalone TUI ownership gates (IYON-TUI-REPOSITORY-SEPARATION-HANDOFF §7).
 *
 * Machine-checks the generic framework, native bridge, public surface, and
 * external-consumer fixture. Run with `bun run check:ownership`. Every
 * failure names the file and rule; no gate relies on prose alone.
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
const NATIVE_CONTRACT = resolve(ROOT, "packages/iyon-tui/src/native.ts");

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

}

// ---------------------------------------------------------------------------
// Gate 2: Standalone external-consumer fixture
// ---------------------------------------------------------------------------

function consumerFixtureGate(): void {
  const fixtureRoot = join(ROOT, "packages/tui-consumer-fixture/src");
  const violations: string[] = [];
  for (const file of walk(fixtureRoot)) {
    for (const spec of specifiersOf(readFileSync(file, "utf8"))) {
      if (!spec.startsWith(".") && !spec.startsWith("/")) {
        if (spec !== "@iyon/tui" && !/^(bun|node):/.test(spec)) {
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
  const packageManifest = JSON.parse(readFileSync(join(ROOT, "packages/tui-consumer-fixture/package.json"), "utf8"));
  const dependencies = Object.keys(packageManifest.dependencies ?? {}).sort();
  if (dependencies.length !== 1 || dependencies[0] !== "@iyon/tui") {
    violations.push(`package dependencies are [${dependencies.join(", ")}]`);
  }
  if (violations.length > 0) fail("standalone-consumer-public-entrypoint", violations.join("; "));
  else pass("standalone-consumer-public-entrypoint", "fixture source and dependency manifest use only @iyon/tui");
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
consumerFixtureGate();
await publicSurfaceGate();

if (failed) {
  console.log("\nOWNERSHIP CHECKS FAILED");
  process.exit(1);
}
console.log("\nALL OWNERSHIP CHECKS PASSED");
