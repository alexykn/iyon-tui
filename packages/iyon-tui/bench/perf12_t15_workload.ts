import { View } from "../src/index.ts";
import { BRIDGE_VIEW_KIND } from "../src/transport/structural/ir.ts";
import {
  axisSetChildForTransport,
  axisSpliceForTransport,
  gridSetCellForTransport,
  NATIVE_PATH_VIEW_KIND,
  textLayoutAtNativePathForTransport,
  type NativePathStep,
} from "../src/api/view/view.ts";

export interface T15WorkloadConfig {
  readonly workload: string;
  readonly mode: string;
  readonly size: number;
}

export interface T15Scenario {
  readonly initial: View;
  next(seed: number): View;
}

function leaf(kind: string, index: number): View {
  const text = `${kind}-${index}`;
  switch (kind) {
    case "styled_span_heavy": return View.text(text).bold().italic().noWrap();
    case "decoration_heavy": return View.text(text).padding(1).fillWidth();
    case "diff_heavy": return View.text(`${text}\n+ changed\n- removed`).noWrap();
    case "plain_text_column":
    case "plain_text":
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
    case "plain_text_column":
    case "plain_text":
    default: return leaves.length === 1 ? leaves[0]! : View.vertical(leaves);
  }
}

function wideAxis(count: number): View {
  return View.vertical(Array.from({ length: count }, (_, index) => View.text(`wide-${index}`).noWrap()));
}

function wideGrid(count: number): View {
  return family("grid_heavy", count, 0);
}

function wrapSharedPath(changed: View, stable: View, depth: number): View {
  let root = View.vertical([changed, stable]);
  for (let index = 0; index < depth; index++) root = root.container();
  return root;
}

export function makeT15Scenario({ workload, mode, size }: T15WorkloadConfig): T15Scenario {
  const count = Math.max(1, size);

  if (mode === "wide_axis_set" || mode === "wide_axis_splice") {
    let current = wideAxis(count);
    const index = Math.floor(count / 2);
    return {
      initial: current,
      next: (seed) => {
        const child = View.text(`wide-${seed}`).noWrap();
        const next = mode === "wide_axis_set"
          ? axisSetChildForTransport(current, index, child)
          : axisSpliceForTransport(current, index, 1, [{ view: child }]);
        current = next;
        return next;
      },
    };
  }

  if (mode === "wide_grid_cell") {
    let current = wideGrid(count);
    return {
      initial: current,
      next: (seed) => {
        const next = gridSetCellForTransport(current, 0, 0, View.text(`cell-${seed}`).noWrap());
        current = next;
        return next;
      },
    };
  }

  if (mode === "path_scalar") {
    let current = View.vertical(Array.from({ length: Math.max(4, Math.min(count, 128)) }, (_, index) => View.text(`path-${index}`).noWrap()));
    const step: NativePathStep = { kind: 4, expectedViewKind: NATIVE_PATH_VIEW_KIND.column, selector: Math.floor(Math.max(4, Math.min(count, 128)) / 2) };
    return {
      initial: current,
      next: (seed) => {
        const next = textLayoutAtNativePathForTransport(
          current,
          [step],
          seed % 2 === 0 ? "noWrap" : "wordThenGrapheme",
          seed % 2 === 0 ? "center" : "end",
        );
        current = next;
        return next;
      },
    };
  }

  if (mode === "text_metadata_patch") {
    const text = View.text("metadata").noWrap().textAlign("start");
    return {
      initial: text,
      next: (seed) => text.textAlign(seed % 2 === 0 ? "center" : "end"),
    };
  }

  if (mode === "decoration_patch") {
    const text = View.text("decoration");
    return {
      initial: text.padding(1),
      next: (seed) => text.padding(seed % 2 === 0 ? 2 : 1),
    };
  }

  const stable = family(workload, count, 0);
  if (mode === "exact_identity") {
    return { initial: stable, next: () => stable };
  }
  if (mode === "rebuilt_equivalent") {
    return { initial: stable, next: () => family(workload, count, 0) };
  }

  const depth = mode.startsWith("shared_deep_")
    ? Number(mode.slice("shared_deep_".length))
    : mode === "large_shared_subtree_cutoff"
      ? 0
      : 0;
  return {
    initial: wrapSharedPath(leaf(workload, 0), stable, depth),
    next: (seed) => wrapSharedPath(leaf(workload, seed), stable, depth),
  };
}

/** Compatibility helper for isolated one-pair probes. */
export function makeT15Pair(
  config: T15WorkloadConfig,
  seed: number,
): { base: View; next: View } {
  const scenario = makeT15Scenario(config);
  return { base: scenario.initial, next: scenario.next(seed) };
}
