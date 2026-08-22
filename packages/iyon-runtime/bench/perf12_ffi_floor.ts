/**
 * PERF-12.0 direct-call floor probe (PERF-12 handoff §83).
 *
 * Not a second architecture implementation. Measures the engine-native FFI
 * call floor on the pinned Bun 1.4 revision using the already-generated
 * runtimeNoop plus representative existing 11v3 generated call shapes:
 *
 *   - noop chains of 1/2/4/8/16/32/64 calls in one JS operation
 *   - one scalar constructor           (view_spacer_create)
 *   - one fixed-arity Row constructor  (view_row_create_2)
 *   - one small ref-buffer constructor (view_axis_create_buffer, 4 children)
 *   - two retained patches             (view_text_layout_patch_root,
 *                                       view_common_patch_root)
 *
 * Timing discipline follows the tiny-case rules of §102: tiny cases batch
 * 1,000 operations per timed block; block medians are recorded. The probe
 * emits PERF-12-ffi-floor.jsonl records and a final decision record that
 * compares the measured floor against the expected changed-frontier budget
 * derived from the frozen PERF-11v4 results.
 *
 * Same-image rule (§61): all pointers come from the N-API addon bootstrap and
 * are bound with Bun linkSymbols/CFunction inside this process; no library is
 * dlopened.
 */

import { CFunction, linkSymbols, type Pointer } from "bun:ffi";
import { native } from "../src/native.ts";
import { linkViewAbi, type NativeAbiPointers } from "../src/tui/generated/view_abi.ts";
import manifest from "../src/tui/generated/view_abi_manifest.json";

const BATCH_OPS = 1_000;
const WARMUP_BLOCKS = Number(Bun.env.PERF12_FLOOR_WARMUP_BLOCKS ?? 30);
const MEASURED_BLOCKS = Number(Bun.env.PERF12_FLOOR_MEASURED_BLOCKS ?? 50);
if (!Number.isSafeInteger(WARMUP_BLOCKS) || WARMUP_BLOCKS < 10) throw new Error("PERF12_FLOOR_WARMUP_BLOCKS must be an integer >= 10");
if (!Number.isSafeInteger(MEASURED_BLOCKS) || MEASURED_BLOCKS < 10) throw new Error("PERF12_FLOOR_MEASURED_BLOCKS must be an integer >= 10");

const STATUS_BIT = 0x8000_0000;
const FIRST_PROBE_NODE_ID = 0x10_0000;

function commandText(command: string[]): string {
  return new TextDecoder().decode(Bun.spawnSync(command).stdout).trim() || "unknown";
}

function sha256(path: string): string {
  return commandText(["shasum", "-a", "256", path]).split(/\s+/)[0] ?? "unknown";
}

function percentile(samples: readonly number[], percentage: number): number {
  const sorted = [...samples].sort((left, right) => left - right);
  return sorted[Math.ceil((sorted.length - 1) * percentage / 100)] ?? 0;
}

interface ShapeStats {
  readonly median_ns: number;
  readonly p95_ns: number;
  readonly p99_ns: number;
  readonly min_ns: number;
  readonly max_ns: number;
}

function stats(perOpSamples: readonly number[]): ShapeStats {
  return {
    median_ns: percentile(perOpSamples, 50),
    p95_ns: percentile(perOpSamples, 95),
    p99_ns: percentile(perOpSamples, 99),
    min_ns: perOpSamples.reduce((left, right) => Math.min(left, right), Number.POSITIVE_INFINITY),
    max_ns: perOpSamples.reduce((left, right) => Math.max(left, right), 0),
  };
}

function isRef(value: number): boolean {
  return value !== 0 && (value & STATUS_BIT) === 0;
}

function assertRef(value: number, shape: string): number {
  if (!isRef(value)) throw new Error(`shape ${shape} returned status 0x${(value >>> 0).toString(16)} instead of a NativeRef`);
  return value;
}

/** Runs one measured shape: warmup blocks, then measured blocks of BATCH_OPS ops each. */
function measure(op: () => void): ShapeStats {
  for (let block = 0; block < WARMUP_BLOCKS; block += 1) {
    for (let index = 0; index < BATCH_OPS; index += 1) op();
  }
  const blockPerOp: number[] = [];
  for (let block = 0; block < MEASURED_BLOCKS; block += 1) {
    const started = Bun.nanoseconds();
    for (let index = 0; index < BATCH_OPS; index += 1) op();
    const elapsed = Bun.nanoseconds() - started;
    blockPerOp.push(elapsed / BATCH_OPS);
  }
  return stats(blockPerOp);
}

function provenance(): Record<string, unknown> {
  return {
    benchmark_version: "PERF-12",
    profile: "probe",
    git_sha: commandText(["git", "rev-parse", "HEAD"]),
    perf7v2_sha: "e5292d62c4011610850cbdc1ba4a35f296f78e4f",
    bun_version: Bun.version,
    bun_revision: commandText(["bun", "--revision"]),
    rustc_version: commandText(["rustc", "--version"]),
    target: commandText(["rustc", "-vV"]).split("\n").find((line) => line.startsWith("host:"))?.split(/\s+/)[1] ?? "unknown",
    macos_version: commandText(["sw_vers", "-productVersion"]),
    cpu_model: commandText(["sysctl", "-n", "machdep.cpu.brand_string"]),
    native_artifact_sha256: sha256(new URL("../native/iyon-native.node", import.meta.url).pathname),
    schema_blake3: manifest.schema_blake3,
    generator_blake3: manifest.generator_blake3,
    batch_ops_per_timed_block: BATCH_OPS,
    warmup_blocks: WARMUP_BLOCKS,
    measured_blocks: MEASURED_BLOCKS,
    warmup_ops: WARMUP_BLOCKS * BATCH_OPS,
    measured_ops: MEASURED_BLOCKS * BATCH_OPS,
  };
}

async function main(): Promise<void> {
  // Pure u32 -> u32 engine-native call floor (same probe function as PERF-11).
  const perfProbe = native.tuiPerfAbiProbe?.() as { noop_ptr: Pointer } | undefined;
  if (perfProbe === undefined) throw new Error("native addon does not expose tuiPerfAbiProbe");
  const pureNoop = CFunction({ ptr: perfProbe.noop_ptr, args: ["u32"], returns: "u32" });

  // Generated same-image View ABI used by the realistic call shapes.
  const bootstrap = native.tuiViewAbiBootstrap?.();
  if (bootstrap === undefined) throw new Error("native addon does not expose tuiViewAbiBootstrap");
  if (
    bootstrap.abi_name !== "iyon_tui_view"
    || bootstrap.abi_version !== 1
    || !Number.isSafeInteger(bootstrap.generation)
    || bootstrap.generation < 1
  ) {
    throw new Error("native View ABI bootstrap metadata is incompatible");
  }
  const symbols = linkViewAbi(bootstrap.functions as unknown as NativeAbiPointers).symbols;
  const runtime = bootstrap.runtime_ptr as Pointer;
  if (symbols.runtimeNoop(runtime) !== 1) throw new Error("native View ABI bootstrap probe failed");

  let nextNodeId = FIRST_PROBE_NODE_ID;
  const createdRefs: number[] = [];
  const drainScratch = new Uint32Array(1_024);

  /** Drains accumulated temporary leases untimed so repeated constructor shapes stay bounded. */
  function drain(): void {
    if (createdRefs.length === 0) return;
    const count = Math.min(createdRefs.length, drainScratch.length);
    for (let index = 0; index < count; index += 1) drainScratch[index] = createdRefs[index]!;
    const released = symbols.viewReleaseMany(runtime, drainScratch, drainScratch, count);
    if (released < 0) throw new Error(`viewReleaseMany failed with ${released}`);
    createdRefs.length = 0;
  }

  const records: Record<string, unknown>[] = [];
  const provenanceBlock = provenance();

  // --- noop chains ------------------------------------------------------------
  for (const chainLength of [1, 2, 4, 8, 16, 32, 64] as const) {
    let sink = 1;
    const measurement = measure(() => {
      for (let index = 0; index < chainLength; index += 1) sink = pureNoop(sink);
    });
    if ((sink & 1) === 0 && sink < 8) throw new Error("noop chain was optimized away");
    records.push({
      record_kind: "ffi_floor_shape",
      shape: `runtime_noop_x${chainLength}`,
      family: "noop_chain",
      calls_per_operation: chainLength,
      ...provenanceBlock,
      ...measurement,
      ns_per_call_median: measurement.median_ns / chainLength,
      samples_count: MEASURED_BLOCKS,
    });
  }

  // Generated dispatch floor: one full generated wrapper call including native
  // status recording (runtimeNoop takes only the runtime pointer).
  records.push({
    record_kind: "ffi_floor_shape",
    shape: "generated_runtime_noop_x1",
    family: "generated_dispatch_floor",
    calls_per_operation: 1,
    ...provenanceBlock,
    ...measure(() => {
      symbols.runtimeNoop(runtime);
    }),
    samples_count: MEASURED_BLOCKS,
  });

  // --- fixture refs shared by realistic call shapes ---------------------------
  let childA = assertRef(symbols.viewSpacerCreate(runtime, nextNodeId >>> 0, 0, 1), "child A");
  nextNodeId += 1;
  const childB = assertRef(symbols.viewSpacerCreate(runtime, nextNodeId >>> 0, 0, 1), "child B");
  nextNodeId += 1;
  const childC = assertRef(symbols.viewSpacerCreate(runtime, nextNodeId >>> 0, 0, 1), "child C");
  nextNodeId += 1;
  const childD = assertRef(symbols.viewSpacerCreate(runtime, nextNodeId >>> 0, 0, 1), "child D");
  nextNodeId += 1;

  const baseRow = assertRef(
    symbols.viewRowCreate2(runtime, nextNodeId >>> 0, 0, 1, 1, childA, 1, childB),
    "base row",
  );
  nextNodeId += 1;
  const baseText = assertRef(
    symbols.viewTextCreateCstring(runtime, nextNodeId >>> 0, 0, "floor-probe", 0, 3, 1),
    "base text",
  );
  nextNodeId += 1;

  // --- scalar constructor -------------------------------------------------------
  {
    const measurement = measure(() => {
      const id = nextNodeId++;
      createdRefs.push(assertRef(symbols.viewSpacerCreate(runtime, id >>> 0, 0, 1), "spacer_create"));
    });
    records.push({ record_kind: "ffi_floor_shape", shape: "scalar_constructor_view_spacer_create", family: "scalar_constructor", calls_per_operation: 1, ...provenanceBlock, ...measurement, samples_count: MEASURED_BLOCKS });
    drain();
  }

  // --- fixed-arity Row constructor -----------------------------------------------
  {
    const measurement = measure(() => {
      const id = nextNodeId++;
      createdRefs.push(assertRef(symbols.viewRowCreate2(runtime, id >>> 0, 0, 1, 1, childA, 1, childB), "row_create_2"));
    });
    records.push({ record_kind: "ffi_floor_shape", shape: "fixed_arity_view_row_create_2", family: "fixed_arity_constructor", calls_per_operation: 1, ...provenanceBlock, ...measurement, samples_count: MEASURED_BLOCKS });
    drain();
  }

  // --- small ref-buffer constructor -----------------------------------------------
  {
    const scratch = new Uint32Array(8);
    scratch[0] = 1; scratch[1] = childA;
    scratch[2] = 1; scratch[3] = childB;
    scratch[4] = 1; scratch[5] = childC;
    scratch[6] = 1; scratch[7] = childD;
    const measurement = measure(() => {
      const id = nextNodeId++;
      createdRefs.push(assertRef(symbols.viewAxisCreateBuffer(runtime, id >>> 0, 0, 2, 0, scratch, scratch, 4), "axis_create_buffer"));
    });
    records.push({
      record_kind: "ffi_floor_shape",
      shape: "ref_buffer_view_axis_create_buffer_4_children",
      family: "ref_buffer_constructor",
      calls_per_operation: 1,
      ref_words_written_per_operation: 8,
      ...provenanceBlock,
      ...measurement,
      samples_count: MEASURED_BLOCKS,
    });
    drain();
  }

  // --- retained patch: text layout --------------------------------------------------
  {
    const measurement = measure(() => {
      const id = nextNodeId++;
      createdRefs.push(assertRef(symbols.viewTextLayoutPatchRoot(runtime, baseText, id >>> 0, 0, 3, 1), "text_layout_patch_root"));
    });
    records.push({ record_kind: "ffi_floor_shape", shape: "retained_patch_view_text_layout_patch_root", family: "retained_patch", calls_per_operation: 1, payload_bytes_resent: 0, ...provenanceBlock, ...measurement, samples_count: MEASURED_BLOCKS });
    drain();
  }

  // --- retained patch: common scalars -------------------------------------------------
  {
    const paddingWord = 1 | (1 << 16); // one changed padding word (top/left = 1)
    const measurement = measure(() => {
      const id = nextNodeId++;
      createdRefs.push(assertRef(symbols.viewCommonPatchRoot(runtime, baseRow, id >>> 0, 0, 4, paddingWord, 0, 0, 0, 0, 0, 0, 0, baseRow), "common_patch_root"));
    });
    records.push({ record_kind: "ffi_floor_shape", shape: "retained_patch_view_common_patch_root", family: "retained_patch", calls_per_operation: 1, payload_bytes_resent: 0, ...provenanceBlock, ...measurement, samples_count: MEASURED_BLOCKS });
    drain();
  }

  // --- decision --------------------------------------------------------------------------
  const byShape = new Map(records.map((record) => [String(record.shape), record]));
  const pureOneCall = Number(byShape.get("runtime_noop_x1")!.median_ns);
  const generatedOneCall = Number(byShape.get("generated_runtime_noop_x1")!.median_ns);
  const worstCommonShapeMedian = Math.max(
    Number(byShape.get("scalar_constructor_view_spacer_create")!.median_ns),
    Number(byShape.get("fixed_arity_view_row_create_2")!.median_ns),
    Number(byShape.get("ref_buffer_view_axis_create_buffer_4_children")!.median_ns),
    Number(byShape.get("retained_patch_view_text_layout_patch_root")!.median_ns),
    Number(byShape.get("retained_patch_view_common_patch_root")!.median_ns),
  );

  // Expected changed-frontier budget from the frozen PERF-11v4 authoritative run
  // (direct_7v2 total medians for representative retained modes; see
  // PERF-11v4-benchmark-report.md). PERF-12 replaces the prior native boundary
  // cost with roughly frontier-size direct calls, so the projected direct-call
  // cost must stay well inside the observed budget. Threshold: the projected
  // cost at the frontier size must be <= 25% of the corresponding observed total.
  const frontierBudgetCases = [
    { case: "small SHARED_PATH (one-leaf edit)", frontier_nodes: 8, prior_total_median_ns: 48_417 },
    { case: "plain-text SHARED_DEEP depth 16", frontier_nodes: 32, prior_total_median_ns: 210_999 },
    { case: "plain-text SHARED_DEEP depth 128", frontier_nodes: 128, prior_total_median_ns: 101_561_750 },
    { case: "realistic trace per operation (upper bound)", frontier_nodes: 200, prior_total_median_ns: 1_182_125 },
  ];
  const projections = frontierBudgetCases.map((entry) => {
    const projectedNs = entry.frontier_nodes * worstCommonShapeMedian;
    return {
      ...entry,
      projected_direct_ffi_cost_ns: projectedNs,
      share_of_prior_budget: projectedNs / entry.prior_total_median_ns,
      within_25pct_budget: projectedNs <= entry.prior_total_median_ns * 0.25,
    };
  });
  const callFloorBelow100ns = pureOneCall < 100 && generatedOneCall < 250;
  const decisionPass = callFloorBelow100ns && projections.every((projection) => projection.within_25pct_budget);
  records.push({
    record_kind: "ffi_floor_decision",
    section: "PERF-12 handoff §83",
    ...provenanceBlock,
    pure_noop_one_call_median_ns: pureOneCall,
    generated_runtime_noop_median_ns: generatedOneCall,
    call_floor_within_gate: callFloorBelow100ns,
    worst_common_shape_median_ns: worstCommonShapeMedian,
    frontier_projections: projections,
    decision_threshold: "pure noop floor < 100 ns/call and generated dispatch < 250 ns/call; projected frontier cost <= 25% of frozen PERF-11v4 direct_7v2 total medians",
    decision: decisionPass ? "GO" : "STOP",
    stop_condition_applies: !decisionPass ? "§83: direct FFI call floor consumes the expected retained-operation budget" : null,
  });

  drain();
  const outPath = Bun.env.PERF12_FLOOR_OUT ?? "packages/iyon-runtime/bench/PERF-12-ffi-floor.jsonl";
  await Bun.write(outPath, `${records.map((record) => JSON.stringify(record)).join("\n")}\n`);
  console.log(JSON.stringify({
    probe: "perf12_ffi_floor",
    shapes: records.length - 1,
    pure_noop_one_call_median_ns: pureOneCall,
    generated_runtime_noop_median_ns: generatedOneCall,
    worst_common_shape_median_ns: worstCommonShapeMedian,
    decision: decisionPass ? "GO" : "STOP",
    outPath,
  }));
}

await main();
