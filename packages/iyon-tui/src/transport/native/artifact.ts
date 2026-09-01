import { existsSync, realpathSync } from "node:fs";
import { dirname, isAbsolute, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export interface NativeArtifactLocation {
  readonly absolutePath: string;
  readonly packageBuildId: string;
  readonly platform: string;
  readonly arch: string;
}

/** Build identity shared by the Node-API and direct content-data loaders. */
export const NATIVE_PACKAGE_BUILD_ID = "iyon-tui-native/s6";

const artifactNames: Readonly<Record<string, string>> = {
  "darwin-arm64": "libiyon_tui_native.dylib",
  "darwin-x64": "libiyon_tui_native.dylib",
  "linux-arm64": "libiyon_tui_native.so",
  "linux-x64": "libiyon_tui_native.so",
  "win32-x64": "iyon_tui_native.dll",
};

/** Returns the platform artifact name used by staging and both loaders. */
export function nativeArtifactName(platform = process.platform, arch = process.arch): string {
  const artifactName = artifactNames[`${platform}-${arch}`];
  if (artifactName === undefined) {
    throw new Error(`unsupported iyon-tui-native target: ${platform}-${arch}`);
  }
  return artifactName;
}

/**
 * Resolves and canonicalizes the one native artifact used by every transport.
 * The caller URL is used so the same code works from the source tree and from
 * a published package; no transport maintains a second platform switch.
 */
export function resolveNativeArtifact(importerUrl: string | URL): NativeArtifactLocation {
  const platform = process.platform;
  const arch = process.arch;
  const targetKey = `${platform}-${arch}`;
  const artifactName = nativeArtifactName(platform, arch);
  if (artifactName === undefined) {
    throw new Error(`unsupported iyon-tui-native target: ${targetKey}`);
  }

  const importerPath = typeof importerUrl === "string" && importerUrl.startsWith("file:")
    ? fileURLToPath(importerUrl)
    : importerUrl instanceof URL
      ? fileURLToPath(importerUrl)
      : importerUrl;
  const packageRoot = resolve(dirname(importerPath), "../../../");
  const repositoryRoot = resolve(packageRoot, "../..");
  // Node-API and the direct loader must both resolve the staged Node addon.
  // Raw cargo dylib/so outputs are not valid `require()` targets and would
  // create a transport-dependent artifact choice.
  const candidates = [
    process.env.ION_TUI_NATIVE_ARTIFACT,
    resolve(packageRoot, "native/iyon-tui-native.node"),
  ].filter((candidate): candidate is string => candidate !== undefined && candidate.length > 0)
    .map((candidate) => isAbsolute(candidate) ? candidate : resolve(repositoryRoot, candidate));

  const candidate = candidates.find((path) => existsSync(path));
  if (candidate === undefined) {
    throw new Error(`iyon-tui-native Node addon is not staged for ${targetKey}; tried ${candidates.join(", ")}`);
  }
  return {
    absolutePath: realpathSync(candidate),
    packageBuildId: NATIVE_PACKAGE_BUILD_ID,
    platform,
    arch,
  };
}
