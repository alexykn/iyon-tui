import { Scene, Tui, View } from "../src/index.ts";
import { resetRetainedIdentityCounters, retainedIdentityCounterSnapshot } from "../src/retained_dag.ts";

const workload = process.env.T15_WORKLOAD ?? "plain_text";
const mode = process.env.T15_MODE ?? "shared_path";
const size = Number(process.env.T15_SIZE ?? 20);
const warmup = Number(process.env.T15_WARMUP ?? 50);
const measured = Number(process.env.T15_MEASURED ?? 1_000);

function median(values: readonly number[]): number {
  const ordered = [...values].sort((left, right) => left - right);
  return ordered[Math.floor(ordered.length / 2)]!;
}
function percentile(values: readonly number[], fraction: number): number {
  const ordered = [...values].sort((left, right) => left - right);
  return ordered[Math.min(ordered.length - 1, Math.ceil(ordered.length * fraction) - 1)]!;
}
function bootstrap(values: readonly number[], rounds = 1_000): [number, number] {
  const medians: number[] = [];
  for (let round = 0; round < rounds; round++) {
    const sample = Array.from({ length: values.length }, () => values[Math.floor(Math.random() * values.length)]!);
    medians.push(median(sample));
  }
  medians.sort((left, right) => left - right);
  return [medians[Math.floor(rounds * 0.025)]!, medians[Math.floor(rounds * 0.975)]!];
}

function leaf(kind: string, index: number): View {
  const text = `${kind}-${index}`;
  switch (kind) {
    case "styled_span_heavy": return View.text(text).bold().italic().noWrap();
    case "decoration_heavy": return View.text(text).padding(1).fillWidth();
    case "diff_heavy": return View.text(`${text}\n+ changed\n- removed`).noWrap();
    case "plain_text": return View.text(text).noWrap();
    default: return View.text(text).noWrap();
  }
}

function family(kind: string, count: number, seed: number): View {
  const leaves = Array.from({ length: count }, (_, index) => leaf(kind, seed + index));
  switch (kind) {
    case "row_heavy": return View.horizontal(leaves);
    case "column_track_heavy": return View.vertical(leaves);
    case "grid_heavy": {
      const side = Math.max(1, Math.min(32, Math.ceil(Math.sqrt(count))));
      return View.grid((grid) => {
        grid.columns(Array.from({ length: side }, () => ({ kind: "content" as const })));
        for (let row = 0; row < Math.ceil(count / side); row++) {
          grid.row((cells) => {
            for (let column = 0; column < side && row * side + column < count; column++) {
              cells.cell(leaves[row * side + column]!);
            }
          });
        }
      });
    }
    case "component_heavy": return View.vertical(leaves.map((child) => child.container().clampRows(4)));
    case "mixed_realistic": return View.vertical([
      View.text(`header-${seed}`).bold(),
      View.horizontal(leaves.slice(0, Math.min(leaves.length, 8))),
      View.vertical(leaves.slice(8)),
    ]);
    default: return leaves.length === 1 ? leaves[0]! : View.vertical(leaves);
  }
}

function makePair(seed: number): { base: View; next: View } {
  const count = Math.max(1, size);
  if (mode === "text_metadata_patch") {
    const base = View.text(`metadata-${seed}`).noWrap().textAlign("start");
    return { base, next: base.textAlign(seed % 2 === 0 ? "center" : "end") };
  }
  if (mode === "decoration_patch") {
    const base = View.text(`decoration-${seed}`).padding(1);
    return { base, next: base.padding(seed % 2 === 0 ? 2 : 1) };
  }
  const stable = family(workload, count, 0);
  if (mode === "exact_identity") return { base: stable, next: stable };
  if (mode === "rebuilt_equivalent") return { base: stable, next: family(workload, count, 0) };
  const changed = View.text(`changed-${seed}`).noWrap();
  let base = View.vertical([View.text("stable-prefix"), stable]);
  let next = View.vertical([changed, stable]);
  if (mode === "shared_deep_4" || mode === "shared_deep_16" || mode === "shared_deep_64" || mode === "shared_deep_128") {
    const depth = Number(mode.slice("shared_deep_".length));
    for (let index = 0; index < depth; index++) {
      base = base.container();
      next = next.container();
    }
  }
  if (mode === "large_shared_subtree_cutoff") {
    base = View.vertical([View.text("changed"), stable]);
    next = View.vertical([View.text(`changed-${seed}`), stable]);
  }
  return { base, next };
}

const tui = await Tui.open({ width: 80, height: 24, headless: true });
try {
  const initial = makePair(0);
  await tui.render(new Scene(initial.base));
  for (let index = 0; index < warmup; index++) {
    const pair = makePair(index);
    await tui.render(new Scene(pair.next));
  }
  resetRetainedIdentityCounters();
  const semanticConstruction: number[] = [];
  const transportAndHost: number[] = [];
  for (let index = 0; index < measured; index++) {
    const constructStart = Bun.nanoseconds();
    const pair = makePair(warmup + index);
    semanticConstruction.push(Bun.nanoseconds() - constructStart);
    const renderStart = Bun.nanoseconds();
    await tui.render(new Scene(pair.next));
    transportAndHost.push(Bun.nanoseconds() - renderStart);
  }
  const total = semanticConstruction.map((value, index) => value + transportAndHost[index]!);
  console.log(JSON.stringify({
    benchmark_version: "PERF-12-T15",
    profile: process.env.T15_PROFILE ?? "draft",
    candidate: process.env.T15_CANDIDATE ?? "napi_default",
    transport: process.env.T15_TRANSPORT ?? "generated_safe_napi",
    workload,
    size,
    mode,
    git_sha: process.env.T15_GIT_SHA ?? "unknown",
    perf7v2_sha: "e5292d62c4011610850cbdc1ba4a35f296f78e4f",
    perf11v4_result_sha: "7c670ccd99fb296b18719f62c1aa845a3e3605de",
    bun_version: Bun.version,
    bun_revision: Bun.revision,
    rustc_version: process.env.T15_RUSTC_VERSION ?? "unknown",
    target: process.env.T15_TARGET ?? "unknown",
    addon_sha256: process.env.T15_NATIVE_SHA256 ?? "unknown",
    warmup,
    measured,
    process_isolated: true,
    semantic_construction_samples_ns: semanticConstruction,
    transport_prepare_samples_ns: transportAndHost,
    native_materialize_samples_ns: [],
    host_commit_samples_ns: [],
    phase_visibility: "semantic_construction_plus_total_render_only",
    samples_ns: total,
    median_ns: median(total),
    p95_ns: percentile(total, 0.95),
    p99_ns: percentile(total, 0.99),
    median_ci95_ns: bootstrap(total),
    structural_delta: retainedIdentityCounterSnapshot(),
  }));
} finally {
  await tui.close();
}
