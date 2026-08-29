import {
  createViewFromSemanticNode,
  updateSemanticViewNode,
  viewNodeId,
  type HorizontalAlign,
  type View,
  type WrapMode,
} from "../../api/view/view.ts";
import {
  SEMANTIC_VIEW_KIND,
  semanticNodeOf,
  type SemanticViewNode,
} from "../../api/view/semantic-node.ts";
import { horizontalAlignCode, wrapModeCode } from "./encoding.ts";

/** Native path selectors and lineage are physical retained metadata. */
export interface NativePathStep {
  readonly kind: number;
  readonly expectedViewKind: number;
  readonly selector: number;
}

/** Lineage metadata for retained-path constructions; references NodeIds only. */
export interface NativePathLineage {
  readonly baseNodeId: number;
  readonly parent?: NativePathLineage;
  readonly step?: NativePathStep;
  readonly depth: number;
}

export interface NativeTextLayoutTransactionEdit {
  readonly lineage: NativePathLineage;
  readonly nodeIds: readonly number[];
  readonly wrap: number;
  readonly align: number;
}

export const NATIVE_PATH_VIEW_KIND = Object.freeze({
  text: 1,
  row: 2,
  column: 3,
  grid: 4,
  hanging: 5,
  container: 6,
  clampRows: 7,
});

export const NATIVE_PATH_STEP = Object.freeze({
  containerChild: 1,
  clampChild: 2,
  rowViewportChild: 3,
  columnChild: 4,
  rowChild: 5,
  gridCell: 6,
  hangingPrefix: 7,
  hangingContinuation: 8,
  hangingBody: 9,
});

const nativePathLineages = new WeakMap<View, NativePathLineage>();
const nativeTextLayoutTransactions = new WeakMap<View, readonly NativeTextLayoutTransactionEdit[]>();

/** @internal Retained path patch constructor. */
export function textLayoutAtNativePathForTransport(
  view: View,
  steps: readonly NativePathStep[],
  wrap: WrapMode,
  align: HorizontalAlign,
): View {
  if (steps.length > 4) throw new RangeError("native retained path depth must be at most 4");
  const nextNode = patchSemanticTextPath(viewNode(view), steps, wrap, align);
  const lineage = nativePathLineageForSteps(view, steps);
  const result = createViewFromSemanticNode(nextNode);
  nativePathLineages.set(result, freezeNativePathLineage(lineage));
  return result;
}

/** @internal Retained transaction constructor for multiple text patches. */
export function textLayoutTransactionForTransport(
  view: View,
  edits: readonly {
    readonly steps: readonly NativePathStep[];
    readonly wrap: WrapMode;
    readonly align: HorizontalAlign;
  }[],
): View {
  if (edits.length < 2 || edits.length > 256) {
    throw new RangeError("native text transaction must contain 2 through 256 edits");
  }
  const seen = new Set<string>();
  let node = viewNode(view);
  for (const edit of edits) {
    if (edit.steps.length > 4) throw new RangeError("native retained transaction path depth must be at most 4");
    const key = edit.steps.map((step) => `${step.kind}:${step.expectedViewKind}:${step.selector}`).join("/");
    if (!seen.add(key)) throw new RangeError("native text transaction paths must be distinct");
    node = patchSemanticTextPath(node, edit.steps, edit.wrap, edit.align);
  }
  const result = createViewFromSemanticNode(node);
  nativeTextLayoutTransactions.set(result, buildTransactionEdits(view, node, edits));
  return result;
}

/** Returns the one-time retained path lineage attached during construction. */
export function nativePathLineage(view: View): NativePathLineage | undefined {
  return nativePathLineages.get(view);
}

/** Internal construction helper used by path-aware retained tests/builders. */
export function nativePathChildLineage(
  base: View,
  parent: NativePathLineage | undefined,
  step: NativePathStep,
): NativePathLineage {
  const baseNodeId = viewNodeId(base);
  if (parent !== undefined && parent.baseNodeId !== baseNodeId) throw new Error("native path lineage base mismatch");
  const immutableStep = Object.freeze({ ...step });
  return Object.freeze({ baseNodeId, parent, step: immutableStep, depth: (parent?.depth ?? 0) + 1 });
}

/** Attaches a root/child path lineage without retaining any child View. */
export function attachNativePathLineage(view: View, lineage: NativePathLineage): void {
  if (lineage.baseNodeId === viewNodeId(view)) throw new Error("native path lineage base must be the previous root");
  nativePathLineages.set(view, freezeNativePathLineage(lineage));
}

/** Returns the construction-time transaction metadata for a View. */
export function nativeTextLayoutTransaction(view: View): readonly NativeTextLayoutTransactionEdit[] | undefined {
  return nativeTextLayoutTransactions.get(view);
}

function viewNode(view: View): SemanticViewNode {
  return semanticNodeOf(view);
}

function freezeNativePathLineage(lineage: NativePathLineage): NativePathLineage {
  const parent = lineage.parent === undefined ? undefined : freezeNativePathLineage(lineage.parent);
  const step = lineage.step === undefined ? undefined : Object.freeze({ ...lineage.step });
  return Object.freeze({ baseNodeId: lineage.baseNodeId, parent, step, depth: lineage.depth });
}

function nativePathLineageForSteps(view: View, steps: readonly NativePathStep[]): NativePathLineage {
  let lineage: NativePathLineage = Object.freeze({ baseNodeId: viewNodeId(view), depth: 0 });
  for (const step of steps) lineage = nativePathChildLineage(view, lineage, step);
  return lineage;
}

function buildTransactionEdits(
  base: View,
  finalNode: SemanticViewNode,
  edits: readonly {
    readonly steps: readonly NativePathStep[];
    readonly wrap: WrapMode;
    readonly align: HorizontalAlign;
  }[],
): readonly NativeTextLayoutTransactionEdit[] {
  return Object.freeze(edits.map((edit) => {
    const nodes_ = semanticPathNodesForTransaction(finalNode, edit.steps);
    if (nodes_ === undefined || nodes_[nodes_.length - 1]?.kind !== SEMANTIC_VIEW_KIND.text) {
      throw new TypeError("native text transaction path does not terminate at text");
    }
    let lineage: NativePathLineage = Object.freeze({ baseNodeId: viewNodeId(base), depth: 0 });
    for (const step of edit.steps) lineage = nativePathChildLineage(base, lineage, step);
    return Object.freeze({
      lineage,
      nodeIds: Object.freeze(nodes_.slice().reverse().map((entry) => entry.id)),
      wrap: wrapModeCode(edit.wrap),
      align: horizontalAlignCode(edit.align),
    });
  }));
}

function patchSemanticTextPath(
  node: SemanticViewNode,
  steps: readonly NativePathStep[],
  wrap: WrapMode,
  align: HorizontalAlign,
): SemanticViewNode {
  const step = steps[0];
  if (step === undefined) {
    if (node.kind !== SEMANTIC_VIEW_KIND.text) throw new TypeError("native retained text path must terminate at text");
    return updateSemanticViewNode(node, { wrap, align });
  }
  if (semanticPathViewKind(node.kind) !== step.expectedViewKind) {
    throw new TypeError("native retained path expected view kind does not match semantic node");
  }
  const tail = steps.slice(1);
  switch (step.kind) {
    case NATIVE_PATH_STEP.containerChild:
    case NATIVE_PATH_STEP.clampChild: {
      if (step.selector !== 0 || (node.kind !== SEMANTIC_VIEW_KIND.container && node.kind !== SEMANTIC_VIEW_KIND.clamp && node.kind !== SEMANTIC_VIEW_KIND.contentMax)) {
        throw new RangeError("native retained single-child path is invalid");
      }
      return updateSemanticViewNode(node, { child: patchSemanticTextPath(node.child, tail, wrap, align) });
    }
    case NATIVE_PATH_STEP.columnChild:
    case NATIVE_PATH_STEP.rowChild: {
      const expected = step.kind === NATIVE_PATH_STEP.columnChild ? SEMANTIC_VIEW_KIND.column : SEMANTIC_VIEW_KIND.row;
      if (node.kind !== expected) throw new TypeError("native retained axis path kind is invalid");
      if (!Number.isInteger(step.selector) || step.selector < 0 || step.selector >= node.children.length) throw new RangeError("native retained axis path selector is out of range");
      const children = node.children.map((child, index) => index === step.selector
        ? { ...child, child: patchSemanticTextPath(child.child, tail, wrap, align) }
        : child);
      return updateSemanticViewNode(node, { children });
    }
    case NATIVE_PATH_STEP.gridCell: {
      if (node.kind !== SEMANTIC_VIEW_KIND.grid || !Number.isInteger(step.selector) || step.selector < 0) throw new TypeError("native retained grid path kind is invalid");
      let remaining = step.selector;
      let changed = false;
      const rows = node.rows.map((row) => ({
        ...row,
        cells: row.cells.map((cell) => {
          if (changed || remaining !== 0) {
            if (!changed) remaining -= 1;
            return cell;
          }
          changed = true;
          return { ...cell, view: patchSemanticTextPath(cell.view, tail, wrap, align) };
        }),
      }));
      if (!changed || remaining !== 0) throw new RangeError("native retained grid path selector is out of range");
      return updateSemanticViewNode(node, { rows });
    }
    case NATIVE_PATH_STEP.hangingPrefix:
    case NATIVE_PATH_STEP.hangingContinuation:
    case NATIVE_PATH_STEP.hangingBody: {
      if (node.kind !== SEMANTIC_VIEW_KIND.hanging || step.selector !== 0) throw new TypeError("native retained hanging path is invalid");
      const key = step.kind === NATIVE_PATH_STEP.hangingPrefix ? "prefix" : step.kind === NATIVE_PATH_STEP.hangingContinuation ? "continuation" : "body";
      return updateSemanticViewNode(node, { [key]: patchSemanticTextPath(node[key], tail, wrap, align) });
    }
    default: throw new TypeError("unknown native retained path step");
  }
}

function semanticPathNodesForTransaction(
  root: SemanticViewNode,
  steps: readonly NativePathStep[],
): SemanticViewNode[] | undefined {
  const collected = [root];
  let current = root;
  for (const step of steps) {
    if (semanticPathViewKind(current.kind) !== step.expectedViewKind) return undefined;
    switch (step.kind) {
      case NATIVE_PATH_STEP.containerChild:
      case NATIVE_PATH_STEP.clampChild:
      case NATIVE_PATH_STEP.rowViewportChild:
        if (step.selector !== 0 || (current.kind !== SEMANTIC_VIEW_KIND.container && current.kind !== SEMANTIC_VIEW_KIND.clamp && current.kind !== SEMANTIC_VIEW_KIND.contentMax)) return undefined;
        current = current.child;
        break;
      case NATIVE_PATH_STEP.columnChild:
      case NATIVE_PATH_STEP.rowChild: {
        const expected = step.kind === NATIVE_PATH_STEP.columnChild ? SEMANTIC_VIEW_KIND.column : SEMANTIC_VIEW_KIND.row;
        if (current.kind !== expected) return undefined;
        const child = current.children[step.selector];
        if (child === undefined) return undefined;
        current = child.child;
        break;
      }
      case NATIVE_PATH_STEP.gridCell: {
        if (current.kind !== SEMANTIC_VIEW_KIND.grid || step.selector < 0) return undefined;
        let remaining = step.selector;
        let found: SemanticViewNode | undefined;
        for (const row of current.rows) {
          for (const cell of row.cells) {
            if (remaining === 0) found = cell.view;
            remaining -= 1;
          }
        }
        if (found === undefined || remaining >= 0) return undefined;
        current = found;
        break;
      }
      case NATIVE_PATH_STEP.hangingPrefix:
      case NATIVE_PATH_STEP.hangingContinuation:
      case NATIVE_PATH_STEP.hangingBody:
        if (current.kind !== SEMANTIC_VIEW_KIND.hanging || step.selector !== 0) return undefined;
        current = step.kind === NATIVE_PATH_STEP.hangingPrefix ? current.prefix : step.kind === NATIVE_PATH_STEP.hangingContinuation ? current.continuation : current.body;
        break;
      default: return undefined;
    }
    collected.push(current);
  }
  return collected;
}

function semanticPathViewKind(kind: SemanticViewNode["kind"]): number {
  switch (kind) {
    case SEMANTIC_VIEW_KIND.text: return NATIVE_PATH_VIEW_KIND.text;
    case SEMANTIC_VIEW_KIND.row: return NATIVE_PATH_VIEW_KIND.row;
    case SEMANTIC_VIEW_KIND.column: return NATIVE_PATH_VIEW_KIND.column;
    case SEMANTIC_VIEW_KIND.grid: return NATIVE_PATH_VIEW_KIND.grid;
    case SEMANTIC_VIEW_KIND.hanging: return NATIVE_PATH_VIEW_KIND.hanging;
    case SEMANTIC_VIEW_KIND.container: return NATIVE_PATH_VIEW_KIND.container;
    case SEMANTIC_VIEW_KIND.clamp:
    case SEMANTIC_VIEW_KIND.contentMax: return NATIVE_PATH_VIEW_KIND.clampRows;
    default: return 0;
  }
}
