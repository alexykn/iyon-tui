import { realpathSync } from "node:fs";
import { mkdir } from "node:fs/promises";

import { nativeArtifactName } from "../src/transport/native/artifact.ts";

const packageDirectory = new URL("../", import.meta.url);
const repositoryDirectory = new URL("../../", packageDirectory);
const nativeDirectory = new URL("native/", packageDirectory);
const stagedAddon = new URL("iyon-tui-native.node", nativeDirectory);

const targetKey = `${process.platform}-${process.arch}`;
const artifactName = nativeArtifactName(process.platform, process.arch);

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
const removedNativeClasses = ["NativeMarkdownProjector", "NativePlainProjector"];
const removedNativeMethods: Readonly<Record<string, readonly string[]>> = {
  NativeHistory: ["push", "freeze", "pushStream", "sealStream"],
  NativeTuiHost: ["render", "createViewSlot", "scrollPane"],
  NativeViewSlot: ["setView", "setAnimation", "setAnimationAtCycleBoundary", "stopAnimation"],
  NativeScrollPane: ["setContent"],
};
const nativeSurfaceOffenders: string[] = removedNativeClasses.filter((name) => addon[name] !== undefined);
for (const [className, methods] of Object.entries(removedNativeMethods)) {
  const candidate = addon[className] as { prototype?: object } | undefined;
  if (candidate?.prototype === undefined) continue;
  const prototypeNames = new Set(Object.getOwnPropertyNames(candidate.prototype));
  for (const method of methods) if (prototypeNames.has(method)) nativeSurfaceOffenders.push(`${className}.${method}`);
}
if (nativeSurfaceOffenders.length > 0) {
  throw new Error(`staged addon exposes removed native surface: ${nativeSurfaceOffenders.join(", ")}`);
}
if (addon.nativeVersion?.() !== "iyon-tui-native/s6" || addon.tuiSmoke?.() !== "iyon-tui/t1") {
  throw new Error(`staged addon failed the Bun load probe: ${stagedAddon.pathname}`);
}
const contentAbi = await import("../src/transport/content/ffi.ts");
const contentMetadata = contentAbi.contentFfiMetadata();
if (contentMetadata.artifactPath !== realpathSync(stagedAddon.pathname)) {
  throw new Error(`content ABI resolved a different artifact: ${contentMetadata.artifactPath}`);
}
const directQualificationExports = [
  "tuiViewAbiBootstrap",
  "tuiPerfAbiProbe",
  "tuiPerfAbiConformanceProbe",
];
const contentAbiSymbols = [
  "iyon_tui_perf13_abi_metadata_v1",
  "iyon_tui_source_append_utf8_v1",
  "iyon_tui_source_replace_utf8_v1",
  "iyon_tui_source_clear_v1",
  "iyon_tui_source_seal_v1",
  "iyon_tui_source_head_truncate_v1",
];
const directFeature = nativeFeatures.includes("direct-ffi");
if (process.platform !== "win32") {
  const nm = Bun.spawnSync({
    cmd: ["nm", process.platform === "darwin" ? "-gU" : "-D", stagedAddon.pathname],
    stdout: "pipe",
    stderr: "pipe",
  });
  if (nm.exitCode !== 0) {
    throw new Error(`unable to inspect staged addon symbols with nm: ${new TextDecoder().decode(nm.stderr)}`);
  }
  const symbols = new TextDecoder().decode(nm.stdout);
  const missingContentSymbols = contentAbiSymbols.filter((symbol) => !symbols.includes(symbol));
  if (missingContentSymbols.length > 0) {
    throw new Error(`staged addon is missing content ABI symbols: ${missingContentSymbols.join(", ")}`);
  }
  const directSymbols = symbols.match(/(?:^|[\s_])_?iyon_(?:abi_(?:probe|conformance)_|(?:runtime|view|host|axis|path|edit|style)_.*_v1)\b/g) ?? [];
  if (directFeature && (!symbols.includes("iyon_abi_probe_noop") || !symbols.includes("iyon_runtime_noop_v1"))) {
    throw new Error("direct-ffi staged addon is missing its qualification symbol surface");
  }
  if (!directFeature && directSymbols.length > 0) {
    throw new Error(`default staged addon exposes direct-ffi symbols: ${directSymbols.slice(0, 8).join(", ")}`);
  }
}
if (directFeature) {
  const missing = directQualificationExports.filter((name) => typeof addon[name] !== "function");
  if (missing.length > 0) throw new Error(`direct-ffi staged addon is missing qualification exports: ${missing.join(", ")}`);
} else {
  const leaked = directQualificationExports.filter((name) => typeof addon[name] === "function");
  if (leaked.length > 0 || typeof addon.tuiViewAbiSession !== "function") {
    throw new Error(`default staged addon has an invalid transport surface: leaked=[${leaked.join(", ")}]`);
  }
}

console.log(`staged ${stagedAddon.pathname} for ${targetKey}${directFeature ? " (direct-ffi feature)" : " (default N-API)"}`);
