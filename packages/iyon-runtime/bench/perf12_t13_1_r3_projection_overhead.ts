/**
 * PERF-12 T13.1 R3 gate — projection-overhead baseline at 10/100/1,000
 * sibling scopes (handoff §32.1 R3 / §31.6; AMENDMENT-C §14.1/§20.6).
 *
 * These numbers are the GO/NO-GO INSTRUMENT THAT SCHEDULES R6B (handoff §32.1
 * Staged delivery): if per-update host work stays bounded as mounted-scope
 * counts grow, R6b's incremental mount/layout frontier can keep being
 * deferred; if it scales linearly, R6b must run.
 *
 * Measured per N (one headless host, ViewSlot-backed projections):
 *   cold_mount          mount parent + N leaf scopes (per-scope cost)
 *   noop_all            invalidate ALL N scopes; every body runs, every
 *                       output is semantically identical => ZERO installs
 *   leaf_update         mutate exactly one leaf + invalidate it + flush
 *                       (end-to-end; THE number — must stay ~flat across N)
 *
 * Output: JSONL records with provenance under bench/.
 */

import { writeFileSync } from "node:fs";
import { native } from "../src/native.ts";
import { RetainedExecutionRuntime, executionCounterSnapshot, type ViewComponent } from "../src/tui/execution.ts";
import { defineView } from "../src/tui/define-view.ts";
import { composeVertical } from "../src/tui/compose.ts";
import { View } from "../src/tui/values/view.ts";

interface LeafProps {
  readonly i: number;
}
import { ViewSlot } from "../src/tui/component.ts";

const Host = native.NativeTuiHost as
  | (new (width: number, height: number, headless: boolean) => import("../src/native.ts").NativeTuiHostContract)
  | undefined;

if (Host === undefined) {
  console.error("R3 projection-overhead benchmark requires the native addon (NativeTuiHost missing)");
  process.exit(2);
}

function median(samples: number[]): number {
  const sorted = samples.slice().sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)]!;
}

function measureNs(action: () => void): number {
  const start = process.hrtime.bigint();
  action();
  return Number(process.hrtime.bigint() - start);
}

const records: unknown[] = [];
for (const n of [10, 100, 1000]) {
  Bun.gc(true);
  // CONSTANT host geometry across N: any per-update scaling with scope count
  // must come from the scope/component machinery, not terminal layout size.
  const host = new Host!(64, 24, true);
  const heapBefore = process.memoryUsage().heapUsed;

  // N independent leaves backed by per-index labels.
  const labels: string[] = new Array(n).fill("v0");
  const LeafN: ViewComponent<LeafProps> = defineView(({ i }) => View.text(`leaf ${i} ${labels[i]}`));
  const Parent: ViewComponent<Record<string, never>> = defineView(() =>
    composeVertical((column) => {
      for (let index = 0; index < n; index += 1) column.child(LeafN({ i: index }));
    }),
  );

  const runtime = new RetainedExecutionRuntime({
    createScopeProjection: () => {
      const slot = new ViewSlot(host);
      const view = slot.view();
      return {
        view,
        install(output: View): void {
          slot.setView(output);
        },
        dispose(): void {
          slot.dispose();
        },
      };
    },
  });

  // --- cold mount ---
  const coldStart = process.hrtime.bigint();
  const root = runtime.mountRoot(Parent, {});
  const coldTotal = Number(process.hrtime.bigint() - coldStart);
  const coldPerScope = coldTotal / n;
  const heapAfterMount = process.memoryUsage().heapUsed;

  // --- exact no-op over ALL scopes (bodies run, zero installs) ---
  const noopRounds = 5;
  const noopSamples: number[] = [];
  let installsBaseline = 0;
  {
    const before = executionCounterSnapshot();
    void before;
    for (let round = 0; round < noopRounds; round += 1) {
      for (let index = 0; index < n; index += 1) runtime.invalidate(root.children[index]!.scope);
      noopSamples.push(measureNs(() => runtime.flush()));
    }
    void installsBaseline;
  }

  // --- local leaf updates (the R6b instrument) ---
  const leafRounds = Math.max(20, Math.min(200, n));
  const leafSamples: number[] = [];
  for (let round = 0; round < leafRounds; round += 1) {
    const target = (round * 37) % n; // deterministic scatter
    labels[target] = `changed-${round}`;
    const start = process.hrtime.bigint();
    runtime.invalidate(root.children[target]!.scope);
    runtime.flush();
    leafSamples.push(Number(process.hrtime.bigint() - start));
  }
  const leafMedian = median(leafSamples);

  Bun.gc(true);
  const heapDelta = process.memoryUsage().heapUsed - heapBefore;

  records.push({
    record_kind: "t13_1_r3_projection_overhead",
    profile: "smoke",
    tranche: "T13.1",
    step: "R3",
    git_sha: new TextDecoder().decode(Bun.spawnSync(["git", "rev-parse", "HEAD"]).stdout).trim(),
    bun_version: Bun.version,
    bun_revision: Bun.revision,
    scopes: n,
    cold_mount_total_ns: coldTotal,
    cold_mount_per_scope_ns: Math.round(coldPerScope),
    noop_all_flush_ns: Math.round(median(noopSamples)),
    noop_per_scope_ns: Math.round(median(noopSamples) / n),
    leaf_update_median_ns: Math.round(leafMedian),
    heap_used_delta_bytes: heapDelta,
    note: "leaf_update_median_ns is the R6b decision instrument: ~flat across N means the incremental host frontier can stay deferred",
  });

  runtime.dispose();
  host.dispose();
}

writeFileSync(
  "packages/iyon-runtime/bench/PERF-12-T13.1-R3-projection-overhead.jsonl",
  records.map((record) => JSON.stringify(record)).join("\n") + "\n",
);
console.table(records);
