// DO NOT EDIT. Generated from tools/tui-abi/view_abi.toml.
// schema_blake3 = 2b797eccd4c6c803a51937b1344f29c27e6289ae5b4765a0a76bf082cb201fbe
// generator_blake3 = 581e146de3ee31e0ceb7b1292ca9a5ca487fb0ada2aa235857505a55520467fa
import type { Pointer } from "bun:ffi";
import type { linkViewAbi } from "./view_abi";
import { viewSpacerCreate } from "./view_calls";
import type { ViewAbiSymbols } from "./view_calls";

const ERROR_BIT = 0x8000_0000;

export interface MaterializeTx {
  readonly symbols: ViewAbiSymbols;
  readonly runtime: Pointer;
}

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

export interface BridgeSpacerMaterializeNode {
  readonly id: number;
  readonly rows: number;
}

/** PERF-12 §74 status detail kind for this materializer: "none". */
export const SPACER_STATUS_DETAIL = "none" as const;

export function materializeSpacer(node: BridgeSpacerMaterializeNode, tx: MaterializeTx): number {
  const [nodeIdLow, nodeIdHigh] = splitNodeId(node.id);
  return viewSpacerCreate(tx.symbols, tx.runtime, nodeIdLow, nodeIdHigh, node.rows);
}
