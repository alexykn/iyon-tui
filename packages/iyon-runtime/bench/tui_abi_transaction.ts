import { createHash } from "node:crypto";
import type { Pointer } from "bun:ffi";
import { native } from "../src/native.ts";
import {
  editTxnAbort,
  editTxnAddTextLayout,
  editTxnBegin,
  editTxnCommitRender,
} from "../src/tui/generated/view_calls.ts";
import {
  nativePathRefForLineage,
  nativeViewAbiSession,
  releaseNativeViewRef,
} from "../src/tui/native_view_abi.ts";
import { nativeViewRefForNodeId } from "../src/tui/native_view_abi.ts";
import {
  nativePathChildLineage,
  nodeIdPair,
  nodeForBridge,
  NATIVE_PATH_STEP,
  NATIVE_PATH_VIEW_KIND,
  View,
} from "../src/tui/values/view.ts";
import manifest from "../src/tui/generated/view_abi_manifest.json";
const ITERATIONS = 2_000;
const REPEATS = 5;
const EDIT_COUNTS = [2, 4, 8, 16, 64] as const;
const Host = native.NativeTuiHost as unknown as (new (width: number, height: number, headless: boolean) => {
  render(view: object): void;
  tuiViewAbiHostPointer(): number;
  dispose(): void;
}) | undefined;

type Prepared = {
  readonly baseRef: number;
  readonly hostPointer: Pointer;
  readonly pathRefs: readonly number[];
  readonly targetIds: readonly (readonly [number, number])[];
  readonly rootId: readonly [number, number];
};

type PreparedWithSession = Prepared & { readonly session: NonNullable<ReturnType<typeof nativeViewAbiSession>> };

function median(values: readonly number[]): number {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.floor(sorted.length / 2)]!;
}

function prepare(count: number, session: NonNullable<ReturnType<typeof nativeViewAbiSession>>, host: InstanceType<NonNullable<typeof Host>>): Prepared {
  const baseChildren = Array.from({ length: count }, (_, index) => View.text(`base-${index}`));
  const changedChildren = baseChildren.map((child) => child.noWrap());
  const base = View.vertical((column) => baseChildren.forEach((child) => column.child(child)));
  const changed = View.vertical((column) => changedChildren.forEach((child) => column.child(child)));
  host.render(nodeForBridge(base));
  const baseRef = nativeViewRefForNodeId(base);
  if (baseRef === undefined) throw new Error("transaction benchmark base ref unavailable");
  const pathRefs = changedChildren.map((_, selector) => nativePathRefForLineage(session, nativePathChildLineage(base, undefined, {
    kind: NATIVE_PATH_STEP.columnChild,
    expectedViewKind: NATIVE_PATH_VIEW_KIND.column,
    selector,
  })));
  return {
    baseRef,
    hostPointer: host.tuiViewAbiHostPointer() as unknown as Pointer,
    pathRefs,
    targetIds: changedChildren.map(nodeIdPair),
    rootId: nodeIdPair(changed),
  };
}

function measure(prepared: PreparedWithSession): { median_ns_per_render: number; checksum: number } {
  const samples: number[] = [];
  let checksum = 0;
  for (let repeat = 0; repeat < REPEATS; repeat += 1) {
    const start = Bun.nanoseconds();
    for (let iteration = 0; iteration < ITERATIONS; iteration += 1) {
      const txn = editTxnBegin(prepared.session.symbols, prepared.session.runtime, prepared.baseRef, prepared.pathRefs.length);
      let failed = false;
      for (let index = 0; index < prepared.pathRefs.length; index += 1) {
        const [targetLow, targetHigh] = prepared.targetIds[index]!;
        const [rootLow, rootHigh] = prepared.rootId;
        const status = editTxnAddTextLayout(
          prepared.session.symbols,
          prepared.session.runtime,
          txn,
          prepared.pathRefs[index]!,
          1,
          targetLow,
          targetHigh,
          rootLow,
          rootHigh,
          rootLow,
          rootHigh,
          rootLow,
          rootHigh,
          rootLow,
          rootHigh,
          3,
          1,
        );
        if (status !== 0) {
          failed = true;
          break;
        }
      }
      if (failed) {
        editTxnAbort(prepared.session.symbols, prepared.session.runtime, txn);
        throw new Error("transaction benchmark add failed");
      }
      const result = editTxnCommitRender(prepared.session.symbols, prepared.session.runtime, prepared.hostPointer, txn);
      checksum ^= result;
      releaseNativeViewRef(prepared.session, result);
    }
    samples.push(Number(Bun.nanoseconds() - start) / ITERATIONS);
  }
  return { median_ns_per_render: median(samples), checksum };
}

const session = nativeViewAbiSession();
if (session === undefined) throw new Error("generated native View ABI is unavailable");
if (Host === undefined) throw new Error("NativeTuiHost is unavailable");
const host = new Host(160, 8, true);
const results: Record<string, { median_ns_per_render: number; checksum: number }> = {};
for (const count of EDIT_COUNTS) {
  const prepared = { ...prepare(count, session, host), session } as PreparedWithSession;
  results[String(count)] = measure(prepared);
}
host.dispose();
const nativeArtifact = await Bun.file(new URL("../native/iyon-tui-native.node", import.meta.url)).arrayBuffer();
const nativeArtifactSha256 = createHash("sha256").update(new Uint8Array(nativeArtifact)).digest("hex");
const git = (command: string): string => {
  const result = Bun.spawnSync(["git", ...command.split(" ")], { stdout: "pipe", stderr: "ignore" });
  return new TextDecoder().decode(result.stdout).trim();
};
console.log(JSON.stringify({
  benchmark: "PERF-11.6-native-transaction",
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
  edit_counts: EDIT_COUNTS,
  iterations: ITERATIONS,
  repeats: REPEATS,
  route: "typed transaction -> native changed-path trie -> one host commit",
  structural_encoding_ns: 0,
  command_words_written: 0,
  path_arrays_written: 0,
  node_id_arrays_written: 0,
  results,
}, null, 2));
