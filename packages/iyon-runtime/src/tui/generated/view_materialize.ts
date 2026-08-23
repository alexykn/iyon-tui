// DO NOT EDIT. Generated from tools/tui-abi/view_abi.toml.
// schema_blake3 = 7c7f9480cf8950965436de870da6d9a135bc346bd1e78aa74cb702874f0cf498
// generator_blake3 = 5fa933f670b4b38bdf04e8e5b6635342d3a75e6781cceb14283f73c575d4ed4a
import type { Pointer } from "bun:ffi";
import type { linkViewAbi } from "./view_abi";
import { viewAxisCreateBuffer, viewColumnCreate0, viewColumnCreate1, viewColumnCreate2, viewColumnCreate3, viewColumnCreate4, viewRowCreate0, viewRowCreate1, viewRowCreate2, viewRowCreate3, viewRowCreate4, viewSpacerCreate } from "./view_calls";
import type { ViewAbiSymbols } from "./view_calls";
import { BRIDGE_LAYOUT_CHILD_KIND, type BridgeLayoutChild } from "../ir.ts";
import { RetainedFastFallbackError, ensureNative } from "../retained_dag.ts";
import { MAX_DIRECT_AXIS_REFS } from "../native_view_policy.ts";
import type { MaterializeTx } from "../retained_dag.ts";
export type { MaterializeTx };

const ERROR_BIT = 0x8000_0000;

function splitNodeId(id: number): [number, number] {
  return [id >>> 0, Math.floor(id / 0x1_0000_0000)];
}

export interface MaterializeStatus {
  readonly ok: boolean;
  readonly reference: number;
  readonly status: number;
}

export function decodeMaterializeStatus(result: number): MaterializeStatus {
  if (result === 0 || (result & ERROR_BIT) !== 0) return { ok: false, reference: 0, status: result >>> 0 };
  return { ok: true, reference: result, status: 0 };
}

const TRACK_CONTENT_MAX = 2;
const TRACK_FIXED = 3;
const TRACK_FLEX = 4;
const TRACK_FLEX_MAX = 5;

function layoutTrackWord(child: BridgeLayoutChild): number {
  switch (child.kind) {
    case BRIDGE_LAYOUT_CHILD_KIND.normal:
      return 0;
    case BRIDGE_LAYOUT_CHILD_KIND.fixed:
      return TRACK_FIXED | (child.size << 8);
    case BRIDGE_LAYOUT_CHILD_KIND.flex:
      return TRACK_FLEX | (1 << 8);
    case BRIDGE_LAYOUT_CHILD_KIND.flexMax:
      return TRACK_FLEX_MAX | (child.maxRows << 8);
    case BRIDGE_LAYOUT_CHILD_KIND.contentMax:
      return TRACK_CONTENT_MAX | (child.maxRows << 8);
  }
}

/** PERF-12 §74 status detail kind for this materializer: "none". */
export const SPACER_STATUS_DETAIL = "none" as const;

export interface BridgeSpacerMaterializeNode {
  readonly id: number;
  readonly rows: number;
}

export function materializeSpacer(node: BridgeSpacerMaterializeNode, tx: MaterializeTx): number {
  const [nodeIdLow, nodeIdHigh] = splitNodeId(node.id);
  return viewSpacerCreate(tx.symbols, tx.runtime, nodeIdLow, nodeIdHigh, node.rows);
}

/** PERF-12 §74 status detail kind for this materializer: "none". */
export const ROW_STATUS_DETAIL = "none" as const;

export interface BridgeRowMaterializeNode {
  readonly id: number;
  readonly gap: number;
  readonly children: readonly BridgeLayoutChild[];
}

export function materializeRow(node: BridgeRowMaterializeNode, tx: MaterializeTx): number {
  const [nodeIdLow, nodeIdHigh] = splitNodeId(node.id);
  const children = node.children;
  switch (children.length) {
    case 0: return viewRowCreate0(tx.symbols, tx.runtime, nodeIdLow, nodeIdHigh, node.gap);
    case 1: return viewRowCreate1(tx.symbols, tx.runtime, nodeIdLow, nodeIdHigh, node.gap, layoutTrackWord(children[0]), ensureNative(children[0].child, tx));
    case 2: return viewRowCreate2(tx.symbols, tx.runtime, nodeIdLow, nodeIdHigh, node.gap, layoutTrackWord(children[0]), ensureNative(children[0].child, tx), layoutTrackWord(children[1]), ensureNative(children[1].child, tx));
    case 3: return viewRowCreate3(tx.symbols, tx.runtime, nodeIdLow, nodeIdHigh, node.gap, layoutTrackWord(children[0]), ensureNative(children[0].child, tx), layoutTrackWord(children[1]), ensureNative(children[1].child, tx), layoutTrackWord(children[2]), ensureNative(children[2].child, tx));
    case 4: return viewRowCreate4(tx.symbols, tx.runtime, nodeIdLow, nodeIdHigh, node.gap, layoutTrackWord(children[0]), ensureNative(children[0].child, tx), layoutTrackWord(children[1]), ensureNative(children[1].child, tx), layoutTrackWord(children[2]), ensureNative(children[2].child, tx), layoutTrackWord(children[3]), ensureNative(children[3].child, tx));
    default: {
      // Single enforcement point: axisRefScratch refuses arities above the
      // retained cap (Sections 30/50) and counts the fallback.
      const scratch = tx.axisRefScratch(children.length);
      let offset = 0;
      for (let index = 0; index < children.length; index++) {
        const child = children[index];
        scratch[offset++] = layoutTrackWord(child);
        scratch[offset++] = ensureNative(child.child, tx);
      }
      tx.noteRefWords(offset);
      return viewAxisCreateBuffer(tx.symbols, tx.runtime, nodeIdLow, nodeIdHigh, 1, node.gap, scratch, children.length);
    }
  }
}

/** PERF-12 §74 status detail kind for this materializer: "none". */
export const COLUMN_STATUS_DETAIL = "none" as const;

export interface BridgeColumnMaterializeNode {
  readonly id: number;
  readonly gap: number;
  readonly children: readonly BridgeLayoutChild[];
}

export function materializeColumn(node: BridgeColumnMaterializeNode, tx: MaterializeTx): number {
  const [nodeIdLow, nodeIdHigh] = splitNodeId(node.id);
  const children = node.children;
  switch (children.length) {
    case 0: return viewColumnCreate0(tx.symbols, tx.runtime, nodeIdLow, nodeIdHigh, node.gap);
    case 1: return viewColumnCreate1(tx.symbols, tx.runtime, nodeIdLow, nodeIdHigh, node.gap, layoutTrackWord(children[0]), ensureNative(children[0].child, tx));
    case 2: return viewColumnCreate2(tx.symbols, tx.runtime, nodeIdLow, nodeIdHigh, node.gap, layoutTrackWord(children[0]), ensureNative(children[0].child, tx), layoutTrackWord(children[1]), ensureNative(children[1].child, tx));
    case 3: return viewColumnCreate3(tx.symbols, tx.runtime, nodeIdLow, nodeIdHigh, node.gap, layoutTrackWord(children[0]), ensureNative(children[0].child, tx), layoutTrackWord(children[1]), ensureNative(children[1].child, tx), layoutTrackWord(children[2]), ensureNative(children[2].child, tx));
    case 4: return viewColumnCreate4(tx.symbols, tx.runtime, nodeIdLow, nodeIdHigh, node.gap, layoutTrackWord(children[0]), ensureNative(children[0].child, tx), layoutTrackWord(children[1]), ensureNative(children[1].child, tx), layoutTrackWord(children[2]), ensureNative(children[2].child, tx), layoutTrackWord(children[3]), ensureNative(children[3].child, tx));
    default: {
      // Single enforcement point: axisRefScratch refuses arities above the
      // retained cap (Sections 30/50) and counts the fallback.
      const scratch = tx.axisRefScratch(children.length);
      let offset = 0;
      for (let index = 0; index < children.length; index++) {
        const child = children[index];
        scratch[offset++] = layoutTrackWord(child);
        scratch[offset++] = ensureNative(child.child, tx);
      }
      tx.noteRefWords(offset);
      return viewAxisCreateBuffer(tx.symbols, tx.runtime, nodeIdLow, nodeIdHigh, 2, node.gap, scratch, children.length);
    }
  }
}

