import { createHash } from "node:crypto";
import type { Pointer } from "bun:ffi";
import { native } from "../../iyon-tui/src/native.ts";
import {
  hostRenderRef,
  viewReleaseMany,
  viewTextLayoutPatchPathD1,
  viewTextLayoutPatchPathD2,
  viewTextLayoutPatchPathD3,
  viewTextLayoutPatchPathD4,
} from "../../iyon-tui/src/generated/view_calls.ts";
import {
  nativePathRefForLineage,
  nativeViewAbiSession,
} from "../../iyon-tui/src/native_view_abi.ts";
import manifest from "../../iyon-tui/src/generated/view_abi_manifest.json";
import {
  NATIVE_PATH_STEP,
  NATIVE_PATH_VIEW_KIND,
  nativePathLineage,
  nodeForBridge,
  nodeIdPair,
  textLayoutAtNativePathForTransport,
  View,
  type NativePathStep,
} from "../../iyon-tui/src/values/view.ts";
import { BRIDGE_VIEW_KIND, type BridgeViewNode } from "../../iyon-tui/src/ir.ts";

const ITERATIONS = 10_000;
const REPEATS = 5;
const Host = native.NativeTuiHost as unknown as (new (width: number, height: number, headless: boolean) => {
  render(view: object): void;
  tuiViewAbiHostPointer(): number;
  dispose(): void;
}) | undefined;

type Prepared = {
  readonly baseRef: number;
  readonly hostPointer: Pointer;
  readonly pathRef: number;
  readonly calls: readonly (() => number)[];
};

function median(values: readonly number[]): number {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.floor(sorted.length / 2)]!;
}

function splitNodeId(id: number): readonly [number, number] {
  return [id >>> 0, Math.floor(id / 0x1_0000_0000)];
}

function pathNodes(root: BridgeViewNode, steps: readonly NativePathStep[]): BridgeViewNode[] {
  const nodes = [root];
  let current = root;
  for (const step of steps) {
    if (step.kind === NATIVE_PATH_STEP.columnChild) {
      if (current.kind !== BRIDGE_VIEW_KIND.column) throw new Error("benchmark column path mismatch");
      current = current.children[step.selector]!.child;
    } else if (step.kind === NATIVE_PATH_STEP.rowChild) {
      if (current.kind !== BRIDGE_VIEW_KIND.row) throw new Error("benchmark row path mismatch");
      current = current.children[step.selector]!.child;
    } else {
      throw new Error("benchmark only uses axis paths");
    }
    nodes.push(current);
  }
  return nodes;
}

function prepare(depth: number, session: NonNullable<ReturnType<typeof nativeViewAbiSession>>, host: InstanceType<NonNullable<typeof Host>>): Prepared {
  let base = View.text("path");
  const steps: NativePathStep[] = [];
  for (let level = 0; level < depth; level += 1) {
    const kind = level % 2 === 0 ? NATIVE_PATH_STEP.columnChild : NATIVE_PATH_STEP.rowChild;
    const expectedViewKind = level % 2 === 0 ? NATIVE_PATH_VIEW_KIND.column : NATIVE_PATH_VIEW_KIND.row;
    base = kind === NATIVE_PATH_STEP.columnChild
      ? View.vertical((column) => { column.child(base); })
      : View.horizontal((row) => { row.child(base); });
    steps.unshift({ kind, expectedViewKind, selector: 0 });
  }
  host.render(nodeForBridge(base));
  const [baseLow, baseHigh] = nodeIdPair(base);
  const baseRef = session.symbols.viewRefForNodeId(session.runtime, baseLow, baseHigh);
  const hostPointer = host.tuiViewAbiHostPointer() as unknown as Pointer;
  const changed = Array.from({ length: ITERATIONS }, () => textLayoutAtNativePathForTransport(base, steps, "noWrap", "center"));
  const lineage = nativePathLineage(changed[0]!);
  if (lineage === undefined) throw new Error("path benchmark lineage missing");
  const pathRef = nativePathRefForLineage(session, lineage);
  const release = new Uint32Array(1);
  const calls = changed.map((next) => {
    const nodes = pathNodes(nodeForBridge(next), steps);
    const target = splitNodeId(nodes[nodes.length - 1]!.id);
    const ancestors = nodes.slice(0, -1).reverse().map((node) => splitNodeId(node.id));
    return () => {
      const result = depth === 1
        ? viewTextLayoutPatchPathD1(session.symbols, session.runtime, baseRef, pathRef, target[0], target[1], ancestors[0]![0], ancestors[0]![1], 3, 2)
        : depth === 2
          ? viewTextLayoutPatchPathD2(session.symbols, session.runtime, baseRef, pathRef, target[0], target[1], ancestors[0]![0], ancestors[0]![1], ancestors[1]![0], ancestors[1]![1], 3, 2)
          : depth === 3
            ? viewTextLayoutPatchPathD3(session.symbols, session.runtime, baseRef, pathRef, target[0], target[1], ancestors[0]![0], ancestors[0]![1], ancestors[1]![0], ancestors[1]![1], ancestors[2]![0], ancestors[2]![1], 3, 2)
            : viewTextLayoutPatchPathD4(session.symbols, session.runtime, baseRef, pathRef, target[0], target[1], ancestors[0]![0], ancestors[0]![1], ancestors[1]![0], ancestors[1]![1], ancestors[2]![0], ancestors[2]![1], ancestors[3]![0], ancestors[3]![1], 3, 2);
      if (result >= 0x8000_0000) throw new Error(`path benchmark status 0x${result.toString(16)}`);
      const status = hostRenderRef(session.symbols, session.runtime, hostPointer, result);
      release[0] = result;
      viewReleaseMany(session.symbols, session.runtime, release, 1);
      if (status !== 0) throw new Error(`host benchmark status ${status}`);
      return result;
    };
  });
  return { baseRef, hostPointer, pathRef, calls };
}

function measure(prepared: Prepared): { median_ns_per_call: number; checksum: number } {
  const samples: number[] = [];
  let checksum = 0;
  for (let repeat = 0; repeat < REPEATS; repeat += 1) {
    const start = Bun.nanoseconds();
    for (const call of prepared.calls) checksum ^= call();
    samples.push(Number(Bun.nanoseconds() - start) / ITERATIONS);
  }
  return { median_ns_per_call: median(samples), checksum };
}

function git(command: string): string {
  const result = Bun.spawnSync(["git", ...command.split(" ")], { stdout: "pipe", stderr: "ignore" });
  return new TextDecoder().decode(result.stdout).trim();
}

const session = nativeViewAbiSession();
if (session === undefined) throw new Error("generated native View ABI is unavailable");
if (Host === undefined) throw new Error("NativeTuiHost is unavailable");

const host = new Host(40, 8, true);
const prepared = [1, 2, 3, 4].map((depth) => ({ depth, value: prepare(depth, session, host) }));
const results = Object.fromEntries(prepared.map(({ depth, value }) => [`depth_${depth}`, measure(value)]));
host.dispose();
const nativeArtifact = await Bun.file(new URL("../../../iyon-tui/native/iyon-tui-native.node", import.meta.url)).arrayBuffer();
const nativeArtifactSha256 = createHash("sha256").update(new Uint8Array(nativeArtifact)).digest("hex");
console.log(JSON.stringify({
  benchmark: "PERF-11.4-generated-path",
  bun_version: Bun.version,
  bun_revision: Bun.revision,
  git_sha: git("rev-parse HEAD"),
  git_dirty: git("status --porcelain") !== "",
  native_artifact_sha256: nativeArtifactSha256,
  schema_blake3: manifest.schema_blake3,
  generator_blake3: manifest.generator_blake3,
  abi_version: session.abi.abi_version,
  semantic_schema_version: session.abi.semantic_version,
  function_count: session.abi.function_count,
  path_depths: [1, 2, 3, 4],
  iterations: ITERATIONS,
  repeats: REPEATS,
  structural_encoding_ns: 0,
  command_words_written: 0,
  path_arrays_written: 0,
  node_id_arrays_written: 0,
  results,
}, null, 2));
