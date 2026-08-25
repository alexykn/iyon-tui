import { mkdir } from "node:fs/promises";

const packageDirectory = new URL("../", import.meta.url);
const repositoryDirectory = new URL("../../", packageDirectory);
const nativeDirectory = new URL("native/", packageDirectory);
const stagedAddon = new URL("iyon-tui-native.node", nativeDirectory);

const targetKey = `${process.platform}-${process.arch}`;
const artifactByTarget: Record<string, string> = {
  "darwin-arm64": "libiyon_tui_native.dylib",
  "darwin-x64": "libiyon_tui_native.dylib",
  "linux-x64": "libiyon_tui_native.so",
  "linux-arm64": "libiyon_tui_native.so",
  "win32-x64": "iyon_tui_native.dll",
};
const artifactName = artifactByTarget[targetKey];

if (artifactName === undefined) {
  throw new Error(`unsupported iyon-tui-native staging target: ${targetKey}`);
}

const nativeFeatures = process.env.ION_NATIVE_FEATURES?.split(",").map((feature) => feature.trim()).filter(Boolean) ?? [];
const targetSuffix = nativeFeatures.length === 0 ? "" : `-${[...nativeFeatures].sort().join("-")}`;
const targetRoot = new URL(`target${targetSuffix}/`, repositoryDirectory);
const targetDirectory = new URL("release/", targetRoot);
const cargoCommand = ["cargo", "build", "--release", "-p", "iyon-tui-native"];
if (nativeFeatures.length > 0) cargoCommand.push("--features", nativeFeatures.join(","));

const cargo = Bun.spawnSync({
  cmd: cargoCommand,
  cwd: repositoryDirectory.pathname,
  // The native addon links the full TUI dependency graph. Keep the default
  // staging path reliable on constrained developer/CI machines; callers can
  // opt into more parallelism explicitly with CARGO_BUILD_JOBS.
  env: {
    ...process.env,
    CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS ?? "1",
    CARGO_TARGET_DIR: targetRoot.pathname,
  },
  stdout: "pipe",
  stderr: "pipe",
});

if (cargo.exitCode !== 0) {
  const stderr = new TextDecoder().decode(cargo.stderr);
  throw new Error(`cargo failed while building iyon-tui-native (${cargo.exitCode}):\n${stderr}`);
}

const nativeArtifact = new URL(artifactName, targetDirectory);
if (!(await Bun.file(nativeArtifact).exists())) {
  throw new Error(`cargo did not produce the expected native addon artifact: ${nativeArtifact.pathname}`);
}

await mkdir(nativeDirectory.pathname, { recursive: true });
await Bun.write(stagedAddon, Bun.file(nativeArtifact));

const addon = require(stagedAddon.pathname) as Record<string, unknown> & {
  nativeVersion?: () => string;
  tuiSmoke?: () => string;
};
if (addon.nativeVersion?.() !== "iyon-tui-native/s6" || addon.tuiSmoke?.() !== "iyon-tui/t1") {
  throw new Error(`staged addon failed the Bun load probe: ${stagedAddon.pathname}`);
}
const legacyExports = [
  "tuiViewAbiBootstrap",
  "tuiPerfAbiProbe",
  "tuiPerfAbiConformanceProbe",
];
const directFeature = nativeFeatures.includes("direct-ffi");
if (process.platform !== "win32") {
  const nm = Bun.spawnSync({
    cmd: ["nm", process.platform === "darwin" ? "-gU" : "-D", stagedAddon.pathname],
    stdout: "pipe",
    stderr: "pipe",
  });
  if (nm.exitCode === 0) {
    const symbols = new TextDecoder().decode(nm.stdout);
    const directSymbols = symbols.match(/(?:^|[\\s_])_?iyon_(?:abi_(?:probe|conformance)_|(?:runtime|view|host|axis|path|edit|style)_.*_v1)\\b/g) ?? [];
    if (directFeature && (!symbols.includes("iyon_abi_probe_noop") || !symbols.includes("iyon_runtime_noop_v1"))) {
      throw new Error("direct-ffi staged addon is missing its qualification symbol surface");
    }
    if (!directFeature && directSymbols.length > 0) {
      throw new Error(`default staged addon exposes direct-ffi symbols: ${directSymbols.slice(0, 8).join(", ")}`);
    }
  }
}
if (directFeature) {
  const missing = legacyExports.filter((name) => typeof addon[name] !== "function");
  if (missing.length > 0) throw new Error(`direct-ffi staged addon is missing qualification exports: ${missing.join(", ")}`);
} else {
  const leaked = legacyExports.filter((name) => typeof addon[name] === "function");
  if (leaked.length > 0 || typeof addon.tuiViewAbiSession !== "function") {
    throw new Error(`default staged addon has an invalid transport surface: leaked=[${leaked.join(", ")}]`);
  }
}

console.log(`staged ${stagedAddon.pathname} for ${targetKey}${directFeature ? " (direct-ffi feature)" : " (default N-API)"}`);
