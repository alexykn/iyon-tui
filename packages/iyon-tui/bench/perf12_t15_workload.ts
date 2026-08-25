import { View } from "../src/index.ts";

export interface T15WorkloadConfig {
  readonly workload: string;
  readonly mode: string;
  readonly size: number;
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

export function makeT15Pair(
  { workload, mode, size }: T15WorkloadConfig,
  seed: number,
): { base: View; next: View } {
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
