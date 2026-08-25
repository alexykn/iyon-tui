import { mkdir } from "node:fs/promises";

const packageDirectory = new URL("../", import.meta.url);
const repositoryDirectory = new URL("../../", packageDirectory);
const targetDirectory = new URL("target/release/", repositoryDirectory);
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
const cargoCommand = ["cargo", "build", "--release", "-p", "iyon-tui-native"];
if (nativeFeatures.length > 0) cargoCommand.push("--features", nativeFeatures.join(","));

const cargo = Bun.spawnSync({
  cmd: cargoCommand,
  cwd: repositoryDirectory.pathname,
  // The native addon links the full TUI dependency graph. Keep the default
  // staging path reliable on constrained developer/CI machines; callers can
  // opt into more parallelism explicitly with CARGO_BUILD_JOBS.
  env: { ...process.env, CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS ?? "1" },
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

const addon = require(stagedAddon.pathname) as {
  nativeVersion?: () => string;
  tuiSmoke?: () => string;
};
if (addon.nativeVersion?.() !== "iyon-tui-native/s3" || addon.tuiSmoke?.() !== "iyon-tui/t1") {
  throw new Error(`staged addon failed the Bun load probe: ${stagedAddon.pathname}`);
}

console.log(`staged ${stagedAddon.pathname} for ${targetKey}`);
