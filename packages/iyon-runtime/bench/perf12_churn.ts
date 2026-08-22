/**
 * PERF-12.2b memory churn acceptance gate (PERF-12 handoff §59).
 *
 * 1,000,000 transient semantic Views are pushed through the shared runtime's
 * generated FFI path while
 *
 *   - a live rendered tree of ~200 nodes is kept installed on a real host and
 *     periodically replaced through the §18 root-lease protocol,
 *   - one transient view in every 10,000 is retained (lease never released),
 *   - periodic wide edits replace children of a 512-child live column,
 *   - periodic text replacements publish fresh text payloads.
 *
 * Every 100,000 operations: close transient state, Bun.gc(true), native full
 * maintenance sweep, Bun.gc(true), record the §89 memory snapshot. The gate
 * requires post-maintenance metadata to stay O(live + bounded slack) with no
 * linear slope across checkpoints.
 */

import { native } from "../src/native.ts";
import { linkViewAbi, type NativeAbiPointers } from "../src/tui/generated/view_abi.ts";
import type { Pointer } from "bun:ffi";
import manifest from "../src/tui/generated/view_abi_manifest.json";

const TOTAL_TRANSIENTS = Number(Bun.env.PERF12_CHURN_TOTAL ?? 1_000_000);
const CHECKPOINT_EVERY = Number(Bun.env.PERF12_CHURN_CHECKPOINT_EVERY ?? 100_000);
const RETAIN_EVERY = 10_000;
const LIVE_TREE_NODES = 200;
const WIDE_COLUMN_CHILDREN = 512;
const ROOT_REPLACE_EVERY = 50_000;
const WIDE_EDIT_EVERY = 500;

function commandText(command: string[]): string {
  return new TextDecoder().decode(Bun.spawnSync(command).stdout).trim() || "unknown";
}

function isRef(value: number): boolean {
  return value !== 0 && (value & 0x8000_0000) === 0;
}

function assertRef(value: number, what: string): number {
  if (!isRef(value)) throw new Error(`${what} returned status 0x${(value >>> 0).toString(16)}`);
  return value;
}

interface MemorySnapshot {
  readonly semantic_cache_entries: number;
  readonly semantic_cache_live: number;
  readonly native_ref_slots: number;
  readonly native_ref_pages: number;
  readonly native_ref_pages_freed: number;
  readonly leased_slots: number;
  readonly unleased_live_slots: number;
  readonly node_ref_entries: number;
  readonly scavenge_queue: number;
  readonly scavenge_processed: number;
  readonly semantic_cache_full_sweeps: number;
  readonly semantic_cache_entries_removed: number;
}

async function main(): Promise<void> {
  const bootstrap = native.tuiViewAbiBootstrap?.();
  if (bootstrap === undefined) throw new Error("native addon does not expose tuiViewAbiBootstrap");
  const symbols = linkViewAbi(bootstrap.functions as unknown as NativeAbiPointers).symbols;
  const runtime = bootstrap.runtime_ptr as Pointer;
  if (symbols.runtimeNoop(runtime) !== 1) throw new Error("bootstrap probe failed");

  const Host = native.NativeTuiHost as unknown as new (w: number, h: number, headless: boolean) => {
    dispose(): void;
    tuiViewAbiHostPointer?(): number;
    screenRows(): string[];
  };
  const host = new Host(80, 24, true);
  const hostPointerValue = host.tuiViewAbiHostPointer?.();
  if (hostPointerValue === undefined) throw new Error("host does not expose its ABI pointer");
  const hostPointer = hostPointerValue as unknown as Pointer;

  let nextNodeId = 0x20_0000;
  const nodeIdPair = (): readonly [number, number] => {
    const id = ++nextNodeId;
    return [id >>> 0, Math.floor(id / 0x1_0000_0000)];
  };

  // Scratch for borrowed ref buffers (§30): fixed small tier, reused forever.
  const childScratch = new Uint32Array(WIDE_COLUMN_CHILDREN * 2);

  interface LiveTree {
    readonly rootRef: number;
    readonly childRefs: number[];
    wideBaseRef: number;
    readonly wideChildren: number;
  }

  /** Builds a fresh live tree; returns leases for root + wide base only. */
  function buildLiveTree(prefix: string): LiveTree {
    // Wide column with 512 text children.
    const wideChildRefs: number[] = [];
    for (let index = 0; index < WIDE_COLUMN_CHILDREN; index += 1) {
      const [lo, hi] = nodeIdPair();
      wideChildRefs.push(assertRef(symbols.viewTextCreateCstring(runtime, lo, hi, `${prefix}-wide-${index}`, 0, 3, 1), "wide child"));
    }
    for (let index = 0; index < WIDE_COLUMN_CHILDREN; index += 1) {
      childScratch[index * 2] = 1;
      childScratch[index * 2 + 1] = wideChildRefs[index]!;
    }
    const [wideLo, wideHi] = nodeIdPair();
    const wideBaseRef = assertRef(symbols.viewAxisCreateBuffer(runtime, wideLo, wideHi, 2, 0, childScratch, childScratch, WIDE_COLUMN_CHILDREN), "wide column");
    // Release the wide children's temp leases: the parent owns them now.
    const wideRelease = new Uint32Array(wideChildRefs);
    if (symbols.viewReleaseMany(runtime, wideRelease, wideRelease, wideChildRefs.length) < 0) throw new Error("release failed");

    // Display column of ~199 text children plus the wide column.
    const childRefs: number[] = [];
    const displayScratch = new Uint32Array(LIVE_TREE_NODES * 2);
    for (let index = 0; index < LIVE_TREE_NODES - 1; index += 1) {
      const [lo, hi] = nodeIdPair();
      childRefs.push(assertRef(symbols.viewTextCreateCstring(runtime, lo, hi, `${prefix}-${index}`, 0, 3, 1), "tree child"));
    }
    childRefs.push(wideBaseRef);
    for (let index = 0; index < childRefs.length; index += 1) {
      displayScratch[index * 2] = 1;
      displayScratch[index * 2 + 1] = childRefs[index]!;
    }
    const [rootLo, rootHi] = nodeIdPair();
    const rootRef = assertRef(symbols.viewAxisCreateBuffer(runtime, rootLo, rootHi, 2, 0, displayScratch, displayScratch, childRefs.length), "root column");
    // Release only the display text temp leases; the wide column's lease is
    // tracked separately as the wide-edit base.
    const treeRelease = new Uint32Array(childRefs.length - 1);
    for (let index = 0; index < childRefs.length - 1; index += 1) treeRelease[index] = childRefs[index]!;
    if (symbols.viewReleaseMany(runtime, treeRelease, treeRelease, treeRelease.length) < 0) throw new Error("release failed");
    return { rootRef, childRefs, wideBaseRef, wideChildren: WIDE_COLUMN_CHILDREN };
  }

  function installRoot(tree: LiveTree): void {
    const status = symbols.hostRenderRef(runtime, hostPointer, tree.rootRef);
    if (status !== 0) throw new Error(`hostRenderRef failed with ${status}`);
  }

  let live = buildLiveTree("seed");
  installRoot(live);

  const retainedRefs: number[] = [];
  const transientRelease = new Uint32Array(1);
  const checkpoints: Record<string, unknown>[] = [];

  const snapshot = (label: string, operations: number): void => {
    Bun.gc(true);
    const maintainResult = native.tuiViewAbiMaintain?.(true) as { semantic_cache_full_sweeps: number } | undefined;
    Bun.gc(true);
    const snap = native.tuiViewRuntimeMemorySnapshot?.(true) as unknown as MemorySnapshot;
    checkpoints.push({
      record_kind: "churn_checkpoint",
      label,
      transient_views_created: operations,
      retained_views: retainedRefs.length,
      expected_live_floor: LIVE_TREE_NODES + WIDE_COLUMN_CHILDREN + retainedRefs.length,
      ...snap,
      maintain_full_sweeps_reported: maintainResult?.semantic_cache_full_sweeps ?? null,
    });
  };

  const started = Bun.nanoseconds();
  for (let operation = 1; operation <= TOTAL_TRANSIENTS; operation += 1) {
    // One transient semantic view per operation, alternating kinds.
    const [lo, hi] = nodeIdPair();
    const reference = operation % 2 === 0
      ? assertRef(symbols.viewSpacerCreate(runtime, lo, hi, operation % 5), "transient spacer")
      : assertRef(symbols.viewTextCreateCstring(runtime, lo, hi, `transient-${operation}`, 0, 3, 1), "transient text");
    if (operation % RETAIN_EVERY === 0) {
      retainedRefs.push(reference); // lease intentionally never released
    } else {
      transientRelease[0] = reference;
      if (symbols.viewReleaseMany(runtime, transientRelease, transientRelease, 1) < 0) throw new Error("transient release failed");
    }

    // Periodic wide edit: replace one child of the live wide column.
    if (operation % WIDE_EDIT_EVERY === 0) {
      const [elo, ehi] = nodeIdPair();
      const replacement = assertRef(symbols.viewTextCreateCstring(runtime, elo, ehi, `edited-${operation}`, 0, 3, 1), "wide edit child");
      const position = (operation / WIDE_EDIT_EVERY) % live.wideChildren;
      const [blo, bhi] = nodeIdPair();
      const nextWide = assertRef(symbols.viewAxisSetChild(runtime, live.wideBaseRef, blo, bhi, position, 1, replacement), "axis set child");
      // The old wide column lease transfers to the new column id; release the
      // previous unleased-live reference bookkeeping by releasing the old
      // base lease only when it is not the root-held reference.
      if (live.wideBaseRef !== live.rootRef) scratchReleaseOne(live.wideBaseRef);
      live.wideBaseRef = nextWide;
      // Replacement child is owned by the new column; drop its temp lease.
      scratchReleaseOne(replacement);
    }

    // Periodic root replacement through the §18 boundary protocol: install
    // the new root while the previous root lease is still held, then release
    // every lease the previous generation held (its edit-chain column and its
    // boundary/root lease).
    if (operation % ROOT_REPLACE_EVERY === 0) {
      const next = buildLiveTree(`gen${operation}`);
      installRoot(next);
      scratchReleaseOne(live.wideBaseRef);
      scratchReleaseOne(live.rootRef);
      live = next;
    }

    if (operation % CHECKPOINT_EVERY === 0) {
      snapshot(`after-${operation}`, operation);
    }
  }

  function scratchReleaseOne(reference: number): void {
    const buffer = new Uint32Array(1);
    buffer[0] = reference;
    if (symbols.viewReleaseMany(runtime, buffer, buffer, 1) < 0) throw new Error("scratch release failed");
  }

  snapshot("final", TOTAL_TRANSIENTS);
  const wallMs = Number(Bun.nanoseconds() - started) / 1e6;

  host.dispose();

  // Gate analysis: post-maintenance cache entries must be O(live + slack),
  // flat across checkpoints — no linear slope with historical operations.
  const numeric = checkpoints.map((checkpoint) => ({
    ops: checkpoint.transient_views_created as number,
    entries: checkpoint.semantic_cache_entries as number,
    slots: checkpoint.native_ref_slots as number,
    leased: checkpoint.leased_slots as number,
    live: checkpoint.semantic_cache_live as number,
  }));
  const steady = numeric.slice(0, -1); // exclude final (retained set complete)
  const steadyMin = Math.min(...steady.map((point) => point.entries));
  const steadyMax = Math.max(...steady.map((point) => point.entries));
  const entriesSlopePerOp = steady.length >= 2
    ? (steadyMax - steadyMin) / Math.max(1, steady.at(-1)!.ops - steady[0]!.ops)
    : 0;
  const last = numeric.at(-1)!;
  const expectedFloor = LIVE_TREE_NODES + WIDE_COLUMN_CHILDREN + retainedRefs.length;
  // Bounded slack bound: post-maintenance metadata may exceed live state only
  // by a constant, never proportionally to historical operations.
  const SLOP_BOUND = 1024;
  const noLinearSlope = steadyMax - steadyMin <= SLOP_BOUND;
  const gate = {
    record_kind: "churn_gate",
    total_transient_views: TOTAL_TRANSIENTS,
    retained_views: retainedRefs.length,
    expected_live_floor_nodes: expectedFloor,
    final_semantic_cache_entries: last.entries,
    final_semantic_cache_live: last.live,
    final_native_ref_slots: last.slots,
    final_leased_slots: last.leased,
    steady_state_entries_min: steadyMin,
    steady_state_entries_max: steadyMax,
    steady_state_entries_delta: steadyMax - steadyMin,
    steady_state_entries_slope_per_op: entriesSlopePerOp,
    slop_bound: SLOP_BOUND,
    no_linear_post_maintenance_slope: noLinearSlope,
    final_within_bounded_slack_of_live: last.entries <= expectedFloor + SLOP_BOUND,
    leased_only_roots_and_retained: last.leased <= retainedRefs.length + 8,
    wall_ms: wallMs,
    pass: false as boolean,
  };
  gate.pass = gate.no_linear_post_maintenance_slope && gate.final_within_bounded_slack_of_live && gate.leased_only_roots_and_retained;

  const provenance = {
    benchmark_version: "PERF-12",
    profile: "gate",
    git_sha: commandText(["git", "rev-parse", "HEAD"]),
    bun_version: Bun.version,
    bun_revision: commandText(["bun", "--revision"]),
    rustc_version: commandText(["rustc", "--version"]),
    target: commandText(["rustc", "-vV"]).split("\n").find((line) => line.startsWith("host:"))?.split(/\s+/)[1] ?? "unknown",
    macos_version: commandText(["sw_vers", "-productVersion"]),
    cpu_model: commandText(["sysctl", "-n", "machdep.cpu.brand_string"]),
    schema_blake3: manifest.schema_blake3,
    generator_blake3: manifest.generator_blake3,
  };
  const records = [...checkpoints, { ...gate, ...provenance }];
  const outPath = Bun.env.PERF12_CHURN_OUT ?? "packages/iyon-runtime/bench/PERF-12-churn.jsonl";
  await Bun.write(outPath, records.map((record) => JSON.stringify(record)).join("\n") + "\n");
  console.log(JSON.stringify({
    probe: "perf12_churn",
    checkpoints: checkpoints.length,
    retained: retainedRefs.length,
    gate: { ...gate, provenance: undefined },
    outPath,
  }));
}

await main();
