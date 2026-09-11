import { realpathSync } from "node:fs";
import { mkdir } from "node:fs/promises";

import { nativeArtifactName } from "../src/transport/native/artifact.ts";

const packageDirectory = new URL("../", import.meta.url);
const repositoryDirectory = new URL("../../", packageDirectory);
const nativeDirectory = new URL("native/", packageDirectory);
const stagedAddon = new URL("iyon-tui-native.node", nativeDirectory);

const targetKey = `${process.platform}-${process.arch}`;
const artifactName = nativeArtifactName(process.platform, process.arch);

const nativeFeatures =
	process.env.ION_NATIVE_FEATURES?.split(",")
		.map((feature) => feature.trim())
		.filter(Boolean) ?? [];
const targetSuffix =
	nativeFeatures.length === 0 ? "" : `-${[...nativeFeatures].sort().join("-")}`;
const targetRoot = new URL(`target${targetSuffix}/`, repositoryDirectory);
const targetDirectory = new URL("release/", targetRoot);
const cargoCommand = ["cargo", "build", "--release", "-p", "iyon-tui-native"];
if (nativeFeatures.length > 0)
	cargoCommand.push("--features", nativeFeatures.join(","));

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
	throw new Error(
		`cargo failed while building iyon-tui-native (${cargo.exitCode}):\n${stderr}`,
	);
}

const nativeArtifact = new URL(artifactName, targetDirectory);
if (!(await Bun.file(nativeArtifact).exists())) {
	throw new Error(
		`cargo did not produce the expected native addon artifact: ${nativeArtifact.pathname}`,
	);
}

await mkdir(nativeDirectory.pathname, { recursive: true });
await Bun.write(stagedAddon, Bun.file(nativeArtifact));

const addon = require(stagedAddon.pathname) as Record<string, unknown> & {
	nativeVersion?: () => string;
	tuiSmoke?: () => string;
};
const removedNativeClasses = [
	"NativeMarkdownProjector",
	"NativePlainProjector",
	"NativeHistory",
	"NativeViewSlot",
	"NativeScrollPane",
];
const removedNativeMethods: Readonly<Record<string, readonly string[]>> = {
	NativeTuiHost: [
		"render",
		"createViewSlotRef",
		"scrollPaneRef",
		"setDesiredViewRef",
		"viewState",
		"tuiViewAbiHostPointer",
	],
};
const removedNativeExports = [
	"bootstrap",
	"tuiViewAbiBootstrap",
	"runtimeNoop",
	"viewStatusDetail",
	"viewRenderRef",
	"hostRenderRef",
	"tuiPerfAbiProbe",
	"tuiPerfAbiConformanceProbe",
	"tuiViewEnvironmentCount",
] as const;
const nativeSurfaceOffenders: string[] = removedNativeClasses.filter(
	(name) => addon[name] !== undefined,
);
for (const name of removedNativeExports)
	if (addon[name] !== undefined) nativeSurfaceOffenders.push(name);
for (const [className, methods] of Object.entries(removedNativeMethods)) {
	const candidate = addon[className] as { prototype?: object } | undefined;
	if (candidate?.prototype === undefined) continue;
	const prototypeNames = new Set(
		Object.getOwnPropertyNames(candidate.prototype),
	);
	for (const method of methods)
		if (prototypeNames.has(method))
			nativeSurfaceOffenders.push(`${className}.${method}`);
}
if (nativeSurfaceOffenders.length > 0) {
	throw new Error(
		`staged addon exposes removed native surface: ${nativeSurfaceOffenders.join(", ")}`,
	);
}
if (
	addon.nativeVersion?.() !== "iyon-tui-native/s6" ||
	addon.tuiSmoke?.() !== "iyon-tui/t1"
) {
	throw new Error(
		`staged addon failed the Bun load probe: ${stagedAddon.pathname}`,
	);
}
const contentAbi = await import("../src/transport/content/ffi.ts");
const contentMetadata = contentAbi.contentFfiMetadata();
if (contentMetadata.artifactPath !== realpathSync(stagedAddon.pathname)) {
	throw new Error(
		`content ABI resolved a different artifact: ${contentMetadata.artifactPath}`,
	);
}
// Source direct FFI is part of the canonical addon. The deleted native View
// ABI had a separate pointer/bootstrap qualification surface; no second UI
// transport or feature-gated View qualification mode is allowed.
const contentAbiSymbols = [
	"iyon_tui_perf13_abi_metadata_v1",
	"iyon_tui_source_append_utf8_v1",
	"iyon_tui_source_replace_utf8_v1",
	"iyon_tui_source_clear_v1",
	"iyon_tui_source_seal_v1",
	"iyon_tui_source_head_truncate_v1",
];
const removedViewAbiSymbols = [
	"iyon_abi_probe_noop",
	"iyon_abi_probe_u32_8",
	"iyon_abi_probe_i32_4",
	"iyon_abi_probe_buffer",
	"iyon_abi_probe_cstring",
	"iyon_abi_conformance_u8_8_v1",
	"iyon_abi_conformance_u16_8_v1",
	"iyon_abi_conformance_u32_8_v1",
	"iyon_abi_conformance_u32_16_v1",
	"iyon_abi_conformance_i32_4_v1",
	"iyon_abi_conformance_f32_4_v1",
	"iyon_abi_conformance_f64_4_v1",
	"iyon_abi_conformance_pointer_v1",
	"iyon_abi_conformance_buffer_v1",
	"iyon_abi_conformance_cstring_v1",
	"iyon_runtime_noop_v1",
	"iyon_view_status_detail_v1",
	"iyon_view_render_ref_v1",
	"iyon_host_render_ref_v1",
] as const;
if (process.platform !== "win32") {
	const nm = Bun.spawnSync({
		cmd: [
			"nm",
			process.platform === "darwin" ? "-gU" : "-D",
			stagedAddon.pathname,
		],
		stdout: "pipe",
		stderr: "pipe",
	});
	if (nm.exitCode !== 0) {
		throw new Error(
			`unable to inspect staged addon symbols with nm: ${new TextDecoder().decode(nm.stderr)}`,
		);
	}
	const symbols = new TextDecoder().decode(nm.stdout);
	const missingContentSymbols = contentAbiSymbols.filter(
		(symbol) => !symbols.includes(symbol),
	);
	if (missingContentSymbols.length > 0) {
		throw new Error(
			`staged addon is missing content ABI symbols: ${missingContentSymbols.join(", ")}`,
		);
	}
	const leakedViewAbiSymbols = removedViewAbiSymbols.filter((symbol) =>
		symbols.includes(symbol),
	);
	if (leakedViewAbiSymbols.length > 0) {
		throw new Error(
			`staged addon exposes removed View ABI probe symbols: ${leakedViewAbiSymbols.join(", ")}`,
		);
	}
}

const buildKind = nativeFeatures.includes("perf-counters")
	? "perf-counters instrumentation"
	: "default N-API";
console.log(`staged ${stagedAddon.pathname} for ${targetKey} (${buildKind})`);
