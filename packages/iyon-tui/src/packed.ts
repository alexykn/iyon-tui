import {
  BRIDGE_DIFF_LINE_KIND,
  BRIDGE_DIFF_LINE_TERMINATION,
  BRIDGE_GRID_TRACK_KIND,
  BRIDGE_LAYOUT_CHILD_KIND,
  BRIDGE_OVERFLOW_KIND,
  BRIDGE_VIEW_KIND,
  PACKED_VIEW,
  type BorderNode,
  type BridgeDiffHunkNode,
  type BridgeGridCellNode,
  type BridgeGridRowNode,
  type BridgeGridTrackNode,
  type BridgeLayoutChild,
  type BridgeOverflowIndicatorNode,
  type BridgeViewNode,
  type ColorNode,
  type DecorationNode,
  type StyleNode,
} from "./ir.ts";
import { nodeForBridge, type View } from "./values/view.ts";

const HEADER_WORDS = 5;
const U32 = 0x1_0000_0000;
const MAX_SAFE = Number.MAX_SAFE_INTEGER;
const EMPTY_STRINGS: string[] = [];
const ATTRIBUTE_BITS = new Map([
  ["bold", 1],
  ["dim", 2],
  ["italic", 4],
  ["underline", 8],
  ["reversed", 16],
  ["strikethrough", 32],
]);

type PackedTransaction = { readonly words: Uint32Array; readonly strings: string[] };
type PackedInvoke = (words: Uint32Array, strings: string[]) => void;
export type PackedRenderHooks = {
  readonly encodingStarted?: () => void;
  readonly encodingFinished?: () => void;
  readonly nativeStarted?: () => void;
  readonly nativeFinished?: () => void;
};

type PackedCounters = {
  packed_encoder_nodes_visited: number;
  packed_encoder_ref_records: number;
  packed_encoder_def_records: number;
  packed_encoder_words_used: number;
  packed_encoder_strings: number;
  packed_encoder_string_bytes: number;
  packed_encoder_buffer_grows: number;
  packed_encoder_ref_packet_hits: number;
  packed_encoder_cache_resyncs: number;
  packed_encoder_cold_retries: number;
};

const counters: PackedCounters = {
  packed_encoder_nodes_visited: 0,
  packed_encoder_ref_records: 0,
  packed_encoder_def_records: 0,
  packed_encoder_words_used: 0,
  packed_encoder_strings: 0,
  packed_encoder_string_bytes: 0,
  packed_encoder_buffer_grows: 0,
  packed_encoder_ref_packet_hits: 0,
  packed_encoder_cache_resyncs: 0,
  packed_encoder_cold_retries: 0,
};

export function resetPackedEncoderCounters(): void {
  for (const key of Object.keys(counters) as (keyof PackedCounters)[]) counters[key] = 0;
}

export function packedEncoderSnapshot(): Record<string, number> {
  return { ...counters };
}

export function isPackedCacheMiss(error: unknown): boolean {
  if (typeof error !== "object" || error === null) return false;
  const candidate = error as { readonly code?: unknown; readonly message?: unknown };
  return candidate.code === "ION_PACKED_CACHE_MISS"
    || (typeof candidate.message === "string" && candidate.message.includes("ION_PACKED_CACHE_MISS"));
}

export function splitSafeU64(value: number): readonly [number, number] {
  if (!Number.isSafeInteger(value) || value <= 0 || value > MAX_SAFE) {
    throw new RangeError("packed NodeId must be a positive safe integer");
  }
  return [value % U32, Math.floor(value / U32)];
}

class WordWriter {
  private buffer = new Uint32Array(256);
  private cursor = HEADER_WORDS;

  reset(): void {
    this.cursor = HEADER_WORDS;
  }

  get position(): number {
    return this.cursor;
  }

  get capacity(): number {
    return this.buffer.length;
  }

  push(value: number): void {
    if (!Number.isInteger(value) || value < 0 || value >= U32) throw new RangeError("packed word must be a uint32");
    this.ensure(1);
    this.buffer[this.cursor++] = value;
  }

  reserve(): number {
    const position = this.cursor;
    this.push(0);
    return position;
  }

  patch(position: number, value: number): void {
    if (!Number.isInteger(value) || value < 0 || value >= U32 || position < 0 || position >= this.cursor) {
      throw new RangeError("invalid packed word patch");
    }
    this.buffer[position] = value;
  }

  finish(rootCount: number): PackedTransaction {
    this.patch(0, PACKED_VIEW.magic);
    this.patch(1, PACKED_VIEW.version);
    this.patch(2, 1);
    this.patch(3, this.cursor);
    this.patch(4, rootCount);
    counters.packed_encoder_words_used += this.cursor;
    return { words: this.buffer.subarray(0, this.cursor), strings: [] };
  }

  private ensure(additional: number): void {
    const required = this.cursor + additional;
    if (required <= this.buffer.length) return;
    let size = this.buffer.length;
    while (size < required) size *= 2;
    const next = new Uint32Array(size);
    next.set(this.buffer);
    this.buffer = next;
    counters.packed_encoder_buffer_grows += 1;
  }
}

export class PackedViewEncoder {
  private knownNodes = new WeakSet<object>();
  private refPackets = new WeakMap<object, Uint32Array>();
  private readonly writer = new WordWriter();
  private readonly strings: string[] = [];
  private readonly stringIndices = new Map<string, number>();
  private readonly seenThisTransaction = new Map<number, BridgeViewNode>();
  private definedThisTransaction: BridgeViewNode[] = [];

  resetKnownNativeState(): void {
    this.knownNodes = new WeakSet<object>();
    this.refPackets = new WeakMap<object, Uint32Array>();
  }

  scratchCapacity(): number {
    return this.writer.capacity;
  }

  encodeRoots(roots: readonly BridgeViewNode[], forceCold = false): PackedTransaction {
    if (roots.length === 0) throw new RangeError("packed transaction requires at least one root");
    this.writer.reset();
    this.strings.length = 0;
    this.stringIndices.clear();
    this.seenThisTransaction.clear();
    this.definedThisTransaction = [];
    for (const root of roots) this.encodeNode(root, forceCold);
    const transaction = this.writer.finish(roots.length);
    return { words: transaction.words, strings: this.strings };
  }

  commitSuccessfulDefinitions(): void {
    for (const node of this.definedThisTransaction) this.knownNodes.add(node);
    counters.packed_encoder_strings += this.strings.length;
    counters.packed_encoder_string_bytes += this.strings.reduce((sum, value) => sum + value.length, 0);
    this.definedThisTransaction = [];
  }

  render(root: BridgeViewNode, invoke: PackedInvoke, hooks: PackedRenderHooks = {}): void {
    let needsColdRetry = false;
    if (this.knownNodes.has(root)) {
      let packet = this.refPackets.get(root);
      if (packet === undefined) {
        hooks.encodingStarted?.();
        packet = this.singleRootRefPacket(root.id);
        hooks.encodingFinished?.();
        this.refPackets.set(root, packet);
      }
      counters.packed_encoder_ref_packet_hits += 1;
      try {
        hooks.nativeStarted?.();
        invoke(packet, EMPTY_STRINGS);
        hooks.nativeFinished?.();
        return;
      } catch (error) {
        hooks.nativeFinished?.();
        if (!isPackedCacheMiss(error)) throw error;
        counters.packed_encoder_cache_resyncs += 1;
        this.resetKnownNativeState();
        needsColdRetry = true;
      }
    }

    if (!needsColdRetry) {
      let encodingOpen = false;
      try {
        hooks.encodingStarted?.();
        encodingOpen = true;
        const transaction = this.encodeRoots([root]);
        hooks.encodingFinished?.();
        encodingOpen = false;
        hooks.nativeStarted?.();
        invoke(transaction.words, transaction.strings);
        hooks.nativeFinished?.();
        this.commitSuccessfulDefinitions();
        return;
      } catch (error) {
        if (encodingOpen) hooks.encodingFinished?.();
        hooks.nativeFinished?.();
        if (!isPackedCacheMiss(error)) throw error;
        counters.packed_encoder_cache_resyncs += 1;
        this.resetKnownNativeState();
      }
    }

    counters.packed_encoder_cold_retries += 1;
    hooks.encodingStarted?.();
    const cold = this.encodeRoots([root], true);
    hooks.encodingFinished?.();
    try {
      hooks.nativeStarted?.();
      invoke(cold.words, cold.strings);
      hooks.nativeFinished?.();
    } catch (error) {
      hooks.nativeFinished?.();
      if (isPackedCacheMiss(error)) throw new Error("ION_PACKED_CACHE_MISS: cold packed retry missed a persistent reference", { cause: error });
      throw error;
    }
    this.commitSuccessfulDefinitions();
  }

  private singleRootRefPacket(id: number): Uint32Array {
    const [low, high] = splitSafeU64(id);
    const packet = new Uint32Array(HEADER_WORDS + 4);
    packet[0] = PACKED_VIEW.magic;
    packet[1] = PACKED_VIEW.version;
    packet[2] = 1;
    packet[3] = packet.length;
    packet[4] = 1;
    packet[5] = PACKED_VIEW.ref;
    packet[6] = 4;
    packet[7] = low;
    packet[8] = high;
    return packet;
  }

  private encodeNode(node: BridgeViewNode, forceCold: boolean): void {
    counters.packed_encoder_nodes_visited += 1;
    const previous = this.seenThisTransaction.get(node.id);
    if (previous !== undefined) {
      if (previous !== node) throw new Error(`packed NodeId ${node.id} belongs to different bridge objects`);
      this.emitRef(node.id);
      return;
    }
    this.seenThisTransaction.set(node.id, node);
    if (!forceCold && this.knownNodes.has(node)) {
      this.emitRef(node.id);
      return;
    }

    counters.packed_encoder_def_records += 1;
    const start = this.writer.position;
    this.writer.push(PACKED_VIEW.def);
    const lengthPosition = this.writer.reserve();
    this.writeNodeId(node.id);
    this.writer.push(node.kind);
    switch (node.kind) {
      case BRIDGE_VIEW_KIND.text: this.encodeText(node.spans, node.wrap, node.align); break;
      case BRIDGE_VIEW_KIND.diff: this.encodeDiff(node.hunks); break;
      case BRIDGE_VIEW_KIND.spacer: this.writer.push(node.rows); break;
      case BRIDGE_VIEW_KIND.row:
      case BRIDGE_VIEW_KIND.column: this.encodeAxis(node.children, node.gap, forceCold); break;
      case BRIDGE_VIEW_KIND.hanging:
        this.encodeNode(node.prefix, forceCold);
        this.encodeNode(node.continuation, forceCold);
        this.encodeNode(node.body, forceCold);
        break;
      case BRIDGE_VIEW_KIND.grid: this.encodeGrid(node, forceCold); break;
      case BRIDGE_VIEW_KIND.container: this.encodeNode(node.child, forceCold); break;
      case BRIDGE_VIEW_KIND.clamp:
        if (node.maxRows === undefined || node.overflow === undefined) throw new TypeError("packed clamp is missing overflow fields");
        this.writer.push(node.maxRows);
        this.encodeOverflow(node.overflow);
        this.encodeNode(node.child, forceCold);
        break;
      case BRIDGE_VIEW_KIND.contentMax:
        this.writer.push(node.maxRows);
        this.encodeNode(node.child, forceCold);
        break;
      case BRIDGE_VIEW_KIND.component: this.writeNodeId(node.handle); break;
      case BRIDGE_VIEW_KIND.decorated:
        this.encodeNode(node.child, forceCold);
        this.encodeDecoration(node.decoration);
        break;
      default: assertNever(node);
    }
    this.writer.patch(lengthPosition, this.writer.position - start);
    this.definedThisTransaction.push(node);
  }

  private emitRef(id: number): void {
    counters.packed_encoder_ref_records += 1;
    this.writer.push(PACKED_VIEW.ref);
    this.writer.push(4);
    this.writeNodeId(id);
  }

  private writeNodeId(id: number): void {
    const [low, high] = splitSafeU64(id);
    this.writer.push(low);
    this.writer.push(high);
  }

  private encodeText(spans: readonly { readonly text: string; readonly style?: StyleNode }[], wrap: number, align: number): void {
    this.writer.push(wrap);
    this.writer.push(align);
    this.writer.push(spans.length);
    for (const span of spans) {
      this.writer.push(this.string(span.text));
      this.writer.push(span.style === undefined ? 0 : 1);
      if (span.style !== undefined) this.encodeStyle(span.style);
    }
  }

  private encodeDiff(hunks: readonly BridgeDiffHunkNode[]): void {
    this.writer.push(hunks.length);
    for (const hunk of hunks) {
      this.writeSafePair(hunk.oldRange.start, hunk.oldRange.count);
      this.writeSafePair(hunk.newRange.start, hunk.newRange.count);
      this.writer.push(hunk.lines.length);
      for (const line of hunk.lines) {
        this.writer.push(line.kind);
        this.writer.push(this.string(line.text));
        this.writer.push(line.termination);
        if (line.kind === BRIDGE_DIFF_LINE_KIND.context) {
          this.writeSafe(line.oldLine);
          this.writeSafe(line.newLine);
        } else if (line.kind === BRIDGE_DIFF_LINE_KIND.addition) this.writeSafe(line.newLine);
        else this.writeSafe(line.oldLine);
      }
    }
  }

  private encodeAxis(children: readonly BridgeLayoutChild[], gap: number, forceCold: boolean): void {
    this.writer.push(gap);
    this.writer.push(children.length);
    for (const child of children) {
      this.writer.push(child.kind);
      this.writer.push("size" in child ? child.size : 0);
      this.writer.push("maxRows" in child ? child.maxRows : 0);
      this.encodeNode(child.child, forceCold);
    }
  }

  private encodeGrid(node: Extract<BridgeViewNode, { kind: typeof BRIDGE_VIEW_KIND.grid }>, forceCold: boolean): void {
    this.writer.push(node.columns.length);
    for (const track of node.columns) this.encodeTrack(track);
    this.writer.push(node.rows.length);
    for (const row of node.rows) {
      this.encodeTrack(row.track);
      this.writer.push(row.cells.length);
      for (const cell of row.cells) {
        this.writer.push(cell.columnSpan);
        this.writer.push(cell.rowSpan);
        this.writer.push(cell.horizontalAlign);
        this.writer.push(cell.verticalAlign);
        this.encodeNode(cell.view, forceCold);
      }
    }
    this.writer.push(node.columnGap);
    this.writer.push(node.rowGap);
  }

  private encodeTrack(track: BridgeGridTrackNode): void {
    this.writer.push(track.kind);
    this.writer.push("max" in track ? track.max : "size" in track ? track.size : 0);
  }

  private encodeOverflow(overflow: BridgeOverflowIndicatorNode | undefined): void {
    const value = overflow ?? { kind: BRIDGE_OVERFLOW_KIND.none };
    this.writer.push(value.kind);
    if (value.kind === BRIDGE_OVERFLOW_KIND.ellipsis) this.encodeStyle(value.style);
    if (value.kind === BRIDGE_OVERFLOW_KIND.footer) {
      this.writer.push(this.string(value.prefix));
      this.encodeStyle(value.style);
    }
  }

  private encodeDecoration(decoration: DecorationNode): void {
    let flags = 0;
    if (decoration.padding !== undefined) flags |= PACKED_VIEW.decorationPadding;
    if (decoration.background !== undefined) flags |= PACKED_VIEW.decorationBackground;
    if (decoration.foreground !== undefined) flags |= PACKED_VIEW.decorationForeground;
    if (decoration.border !== undefined) flags |= PACKED_VIEW.decorationBorder;
    flags |= PACKED_VIEW.decorationStyle;
    if (decoration.styleStates !== undefined) flags |= PACKED_VIEW.decorationStates;
    if (decoration.width !== undefined) flags |= PACKED_VIEW.decorationWidth;
    if (decoration.height !== undefined) flags |= PACKED_VIEW.decorationHeight;
    if (decoration.minWidth !== undefined) flags |= PACKED_VIEW.decorationMinWidth;
    if (decoration.maxWidth !== undefined) flags |= PACKED_VIEW.decorationMaxWidth;
    if (decoration.minHeight !== undefined) flags |= PACKED_VIEW.decorationMinHeight;
    if (decoration.maxHeight !== undefined) flags |= PACKED_VIEW.decorationMaxHeight;
    this.writer.push(flags);
    if (decoration.padding !== undefined) {
      this.writer.push(decoration.padding.top); this.writer.push(decoration.padding.right);
      this.writer.push(decoration.padding.bottom); this.writer.push(decoration.padding.left);
    }
    if (decoration.background !== undefined) this.encodeColor(decoration.background);
    if (decoration.foreground !== undefined) this.encodeColor(decoration.foreground);
    if (decoration.border !== undefined) this.encodeBorder(decoration.border);
    this.encodeStyle(decoration.style);
    if (decoration.styleStates !== undefined) {
      const states = Object.entries(decoration.styleStates);
      this.writer.push(states.length);
      for (const [key, value] of states) { this.writer.push(this.string(key)); this.writer.push(this.string(value)); }
    }
    if (decoration.width !== undefined) this.writer.push(decoration.width === "fit" ? PACKED_VIEW.ruleFit : PACKED_VIEW.ruleFill);
    if (decoration.height !== undefined) this.writer.push(decoration.height === "fit" ? PACKED_VIEW.ruleFit : PACKED_VIEW.ruleFill);
    if (decoration.minWidth !== undefined) this.writer.push(decoration.minWidth);
    if (decoration.maxWidth !== undefined) this.writer.push(decoration.maxWidth);
    if (decoration.minHeight !== undefined) this.writer.push(decoration.minHeight);
    if (decoration.maxHeight !== undefined) this.writer.push(decoration.maxHeight);
  }

  private encodeBorder(border: BorderNode): void {
    let flags = 0;
    if (border.glyphs !== undefined) flags |= PACKED_VIEW.borderGlyphs;
    if (border.color !== undefined) flags |= PACKED_VIEW.borderColor;
    if (border.style !== undefined) flags |= PACKED_VIEW.borderStyle;
    if (border.edges !== undefined) flags |= PACKED_VIEW.borderEdges;
    this.writer.push(flags);
    if (border.glyphs !== undefined) {
      for (const key of ["top", "right", "bottom", "left", "topLeft", "topRight", "bottomLeft", "bottomRight"]) {
        const value = border.glyphs[key];
        if (typeof value !== "string") throw new TypeError(`border glyph ${key} is required`);
        this.writer.push(this.string(value));
      }
    }
    if (border.color !== undefined) this.encodeColor(border.color);
    if (border.style !== undefined) this.writer.push(border.style === "plain" ? PACKED_VIEW.borderStylePlain : border.style === "rounded" ? PACKED_VIEW.borderStyleRounded : PACKED_VIEW.borderStyleDouble);
    if (border.edges !== undefined) this.writer.push(border.edges === "all" ? PACKED_VIEW.borderEdgesAll : PACKED_VIEW.borderEdgesTopBottom);
  }

  private encodeStyle(style: StyleNode): void {
    let flags = 0;
    if (style.theme !== undefined) flags |= PACKED_VIEW.styleTheme;
    if (style.foreground !== undefined) flags |= PACKED_VIEW.styleForeground;
    if (style.background !== undefined) flags |= PACKED_VIEW.styleBackground;
    this.writer.push(flags);
    if (style.theme !== undefined) this.writer.push(this.string(style.theme));
    if (style.foreground !== undefined) this.encodeColor(style.foreground);
    if (style.background !== undefined) this.encodeColor(style.background);
    let present = 0;
    let truth = 0;
    for (const [name, enabled] of Object.entries(style.attributes)) {
      const bit = ATTRIBUTE_BITS.get(name);
      if (bit === undefined) throw new TypeError(`unknown text attribute ${name}`);
      present |= bit;
      if (enabled) truth |= bit;
    }
    this.writer.push(present);
    this.writer.push(truth);
  }

  private encodeColor(color: ColorNode): void {
    if (typeof color === "string") {
      this.writer.push(PACKED_VIEW.colorString);
      this.writer.push(this.string(color));
      return;
    }
    if (color.type !== "ansi" || !Number.isInteger(color.value) || color.value < 0 || color.value > 255) throw new TypeError("invalid ANSI color");
    this.writer.push(PACKED_VIEW.colorAnsi);
    this.writer.push(color.value);
  }

  private string(value: string): number {
    if (typeof value !== "string") throw new TypeError("packed string table value must be a string");
    const existing = this.stringIndices.get(value);
    if (existing !== undefined) return existing;
    const index = this.strings.length;
    this.strings.push(value);
    this.stringIndices.set(value, index);
    return index;
  }

  private writeSafe(value: number | undefined): void {
    if (value === undefined) throw new TypeError("packed diff line number is required");
    this.writeNodeId(value);
  }

  private writeSafePair(left: number, right: number): void {
    this.writeNonNegativeSafe(left);
    this.writeNonNegativeSafe(right);
  }

  private writeNonNegativeSafe(value: number): void {
    if (!Number.isSafeInteger(value) || value < 0 || value > MAX_SAFE) throw new RangeError("packed number must be a non-negative safe integer");
    const low = value % U32;
    const high = Math.floor(value / U32);
    this.writer.push(low);
    this.writer.push(high);
  }
}

export function createPackedViewEncoder(): PackedViewEncoder {
  return new PackedViewEncoder();
}

export function renderPackedView(encoder: PackedViewEncoder, view: View, invoke: PackedInvoke, hooks: PackedRenderHooks = {}): void {
  hooks.encodingStarted?.();
  let node: BridgeViewNode;
  try {
    node = nodeForBridge(view);
  } finally {
    hooks.encodingFinished?.();
  }
  encoder.render(node, invoke, hooks);
}

function assertNever(value: never): never {
  throw new Error(`unsupported packed view kind ${(value as { readonly kind?: unknown }).kind ?? "unknown"}`);
}
