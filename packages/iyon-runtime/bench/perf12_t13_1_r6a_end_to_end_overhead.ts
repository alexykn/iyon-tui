/**
 * PERF-12 T13.1 R6a — END-TO-END local-update latency at 10/100/1,000
 * sibling scopes through a REAL headless Tui (handoff §32.1 R6a gate:
 * "overhead at current mounted-scope counts measured and recorded as the
 * R6b decision input").
 *
 * Complements perf12_t13_1_r3_projection_overhead.ts (JS-side) by including
 * the native slot install + damage propagation in the measured path:
 *
 *   state write -> scheduled flush -> scope body -> semantic reuse check ->
 *   projection install (native revision swap) -> host damage/paint
 *
 * CONSTANT host geometry across N. Visibility is verified once per N outside
 * the timing window.
 */

import { writeFileSync } from "node:fs";
import { Tui } from "../src/tui/runtime.ts";
import { Scene } from "../src/tui/scene.ts";
import { View } from "../src/tui/values/view.ts";
import { RetainedExecutionRuntime, type ViewComponent } from "../src/tui/execution.ts";
import { defineView } from "../src/tui/define-view.ts";
import { composeVertical } from "../src/tui/compose.ts";

interface LeafProps {
  readonly i: number;
}

const records: unknown[] = [];

function median(samples: number[]): number {
  const sorted = samples.slice().sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)]!;
}

for (const n of [10, 100, 1000]) {
  Bun.gc(true);
  const tui = await Tui.open({ width: 64, height: 24, headless: true }); // constant geometry
  const heapBefore = process.memoryUsage().heapUsed;

  const labels: string[] = new Array(n).fill("v0");
  const Leaf: ViewComponent<LeafProps> = defineView(({ i }) => View.text(`leaf ${i} ${labels[i]}`));
  const Parent: ViewComponent<Record<string, never>> = defineView(() =>
    composeVertical((column) => {
      for (let index = 0; index < n; index += 1) column.child(Leaf({ i: index }));
    }),
  );

  const runtime = new RetainedExecutionRuntime({
    createScopeProjection: () => {
      const slot = tui.createViewSlot(View.spacer(0));
      const view = slot.view();
      return {
        view,
        install(output: View): void {
          slot.setView(output);
        },
        preparePublication(output: View) {
          return slot.prepareSetView(output);
        },
        dispose(): void {
          slot.dispose();
        },
      };
    },
  });

  // Cold mount.
  const coldStart = process.hrtime.bigint();
  const root = runtime.mountRoot(Parent, {});
  const coldTotal = Number(process.hrtime.bigint() - coldStart);

  // Local leaf updates BEFORE any scene render: isolates execution + slot
  // install cost (no mounted-scene resolve/damage in the path).
  const rounds = Math.max(20, Math.min(200, n));
  const preRenderSamples: number[] = [];
  let visible = false;
  let round = 0;
  for (; round < rounds; round += 1) {
    const target = (round * 37) % n;
    labels[target] = `pre-${round}`;
    const start = process.hrtime.bigint();
    runtime.invalidate(root.children[target]!.scope);
    runtime.flush();
    preRenderSamples.push(Number(process.hrtime.bigint() - start));
  }

  // Mount the scene: fresh wrapper embedding every stable projection.
  const sceneStart = process.hrtime.bigint();
  tui.render(
    new Scene(
      View.vertical((column) => {
        for (const record of root.children) column.child(record.scope.projection!.view);
      }),
    ),
  );
  const sceneRenderNs = Number(process.hrtime.bigint() - sceneStart);

  // Visibility proof (deterministic): update an ON-SCREEN leaf (index 1),
  // re-render the UNCHANGED parent composite, expect the new text.
  labels[1] = "vis-check";
  runtime.invalidate(root.children[1]!.scope);
  runtime.flush();
  tui.render(new Scene(root.currentOutput!));
  visible = tui.screenRows().some((row: string) => row.includes("vis-check"));

  // Local leaf updates POST scene render: now each install propagates through
  // the mounted component forest (AMENDMENT-C §5.3 resolver scan — the
  // measured R6b trigger input).
  const postRenderSamples: number[] = [];
  for (let index = 0; index < rounds; index += 1) {
    const target = (round * 37 + index * 53) % n;
    labels[target] = `post-${index}`;
    const start = process.hrtime.bigint();
    runtime.invalidate(root.children[target]!.scope);
    runtime.flush();
    postRenderSamples.push(Number(process.hrtime.bigint() - start));
  }

  Bun.gc(true);
  const heapDelta = process.memoryUsage().heapUsed - heapBefore;

  records.push({
    record_kind: "t13_1_r6a_end_to_end_overhead",
    profile: "smoke",
    tranche: "T13.1",
    step: "R6a",
    git_sha: new TextDecoder().decode(Bun.spawnSync(["git", "rev-parse", "HEAD"]).stdout).trim(),
    bun_version: Bun.version,
    bun_revision: Bun.revision,
    scopes: n,
    cold_mount_total_ns: coldTotal,
    cold_mount_per_scope_ns: Math.round(coldTotal / n),
    leaf_update_pre_render_median_ns: Math.round(median(preRenderSamples)),
    initial_scene_render_ns: Math.round(sceneRenderNs),
    leaf_update_post_scene_render_median_ns: Math.round(median(postRenderSamples)),
    leaf_update_visible_through_unchanged_scene: visible,
    heap_used_delta_bytes: heapDelta,
    note: "post_scene_render phase measures the AMENDMENT-C SS5.3 resolver gap: per-update cost scales with MOUNTED scope count once a scene references them; this curve IS the R6b decision evidence",
  });

  runtime.dispose();
  await tui.close();
}

writeFileSync(
  "packages/iyon-runtime/bench/PERF-12-T13.1-R6a-end-to-end-overhead.jsonl",
  records.map((record) => JSON.stringify(record)).join("\n") + "\n",
);
console.table(records);
