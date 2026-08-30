import { tuiError } from "../api/errors.ts";
import type { HandleId } from "../api/controls/framework-handle.ts";
import type { View } from "../api/view/view.ts";
import {
  peekSemanticGridSequenceOverride,
  peekSemanticSequenceOverride,
  semanticNodeHasAttachments,
  semanticNodeOf,
  SEMANTIC_VIEW_KIND,
  type SemanticViewNode,
} from "../api/view/semantic-node.ts";
import type {
  NativeResourceRegistry,
  PreparedResourceLease,
} from "./native-resource-registry.ts";

export interface AttachmentRuntimeContext {
  readonly registry: NativeResourceRegistry;
  readonly environment: object;
  readonly host: object;
}

export interface PreparedAttachmentSet {
  readonly leases: readonly PreparedResourceLease[];
  commitDesired(): void;
  abort(): void;
}

/**
 * Desired/visible binding ledger for one host. It deliberately stores only
 * prepared resolver leases; semantic nodes remain backend-neutral and the
 * visible set is not changed until a frame commits.
 */
interface DesiredAttachmentRevision {
  readonly revision: string;
  readonly leases: readonly PreparedResourceLease[];
}

function revisionKey(value: string | number | undefined): string | undefined {
  if (value === undefined) return undefined;
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value) || value < 0) throw new TypeError("native revision must be a non-negative safe integer");
    return BigInt(value).toString();
  }
  if (!/^\d+$/u.test(value)) throw new TypeError("native revision must be a decimal integer string");
  return BigInt(value).toString();
}

function compareRevisions(left: string, right: string): number {
  const a = BigInt(left);
  const b = BigInt(right);
  return a < b ? -1 : a > b ? 1 : 0;
}

export class AttachmentBindingState {
  private desired: readonly PreparedResourceLease[] = [];
  private visible: readonly PreparedResourceLease[] = [];
  /** Desired bindings retained for frames still in backend flight. */
  private superseded: DesiredAttachmentRevision[] = [];
  private desiredRevision: string | undefined;
  private visibleRevision: string | undefined;

  commitDesired(prepared: PreparedAttachmentSet, revision?: string | number): void {
    const key = revisionKey(revision);
    if (key === undefined) {
      for (const entry of this.superseded) {
        for (const lease of entry.leases) lease.releaseDesired();
      }
      this.superseded = [];
      for (const lease of this.desired) lease.releaseDesired();
      prepared.commitDesired();
      this.desired = prepared.leases;
      this.desiredRevision = undefined;
      return;
    }

    if (this.desiredRevision !== undefined) {
      this.superseded.push({ revision: this.desiredRevision, leases: this.desired });
    } else {
      for (const lease of this.desired) lease.releaseDesired();
    }
    prepared.commitDesired();
    this.desired = prepared.leases;
    this.desiredRevision = key;
  }

  commitVisible(revision?: string | number): void {
    const key = revisionKey(revision);
    if (key === undefined) {
      this.releaseSuperseded();
      this.releaseVisibleForReplacement(this.visible);
      for (const lease of this.desired) lease.commitVisible();
      this.visible = this.desired;
      this.visibleRevision = undefined;
      return;
    }
    if (this.visibleRevision !== undefined && compareRevisions(key, this.visibleRevision) <= 0) return;

    let target = this.desiredRevision === key ? this.desired : undefined;
    if (target === undefined) {
      target = this.superseded.find((entry) => entry.revision === key)?.leases;
    }
    if (target === undefined) return;
    if (target !== this.visible) this.releaseVisibleForReplacement(this.visible);
    for (const lease of target) lease.commitVisible();
    this.visible = target;
    this.visibleRevision = key;

    const remaining: DesiredAttachmentRevision[] = [];
    for (const entry of this.superseded) {
      if (entry.leases === target) continue;
      if (compareRevisions(entry.revision, key) <= 0) {
        for (const lease of entry.leases) lease.releaseDesired();
      } else {
        remaining.push(entry);
      }
    }
    this.superseded = remaining;
  }

  dispose(): void {
    const leases = new Set<PreparedResourceLease>([
      ...this.desired,
      ...this.visible,
      ...this.superseded.flatMap((entry) => entry.leases),
    ]);
    for (const lease of leases) lease.releaseDesired();
    for (const lease of leases) lease.releaseVisible();
    this.desired = [];
    this.visible = [];
    this.superseded = [];
    this.desiredRevision = undefined;
    this.visibleRevision = undefined;
  }

  desiredCount(): number { return this.desired.length; }
  visibleCount(): number { return this.visible.length; }

  private releaseSuperseded(): void {
    for (const entry of this.superseded) {
      for (const lease of entry.leases) lease.releaseDesired();
    }
    this.superseded = [];
  }

  private releaseVisibleForReplacement(leases: readonly PreparedResourceLease[]): void {
    for (const lease of leases) lease.releaseVisible();
    if (leases !== this.desired) {
      for (const lease of leases) lease.releaseDesired();
    }
  }
}

/**
 * Validates every attachment in a semantic candidate during H3 prepare.
 * Shared ordinary subtrees are traversed once per occurrence so attachment
 * reuse is rejected while ordinary semantic DAG reuse remains legal.
 */
export function validateSemanticAttachments(
  root: SemanticViewNode,
  registry: NativeResourceRegistry,
  environment: object,
  host: object,
): void {
  prepareSemanticAttachments(root, registry, environment, host).abort();
}

export function prepareAttachmentsForView(
  view: View,
  context: AttachmentRuntimeContext | undefined,
): PreparedAttachmentSet {
  if (context === undefined) return new PreparedAttachmentSetImpl([]);
  return prepareSemanticAttachments(
    semanticNodeOf(view),
    context.registry,
    context.environment,
    context.host,
  );
}

export function prepareSemanticAttachments(
  root: SemanticViewNode,
  registry: NativeResourceRegistry,
  environment: object,
  host: object,
): PreparedAttachmentSet {
  if (!semanticNodeHasAttachments(root)) return new PreparedAttachmentSetImpl([]);
  const leases: PreparedResourceLease[] = [];
  const stateUses = new Map<number, string>();
  const contentUses = new Map<number, string>();
  const active = new Set<SemanticViewNode>();
  try {
    visit(root, "root");
    return new PreparedAttachmentSetImpl(leases);
  } catch (error) {
    for (const lease of leases) lease.abort();
    throw error;
  }

  function visit(node: SemanticViewNode, path: string): void {
    if (active.has(node)) throw tuiError("validation", "semantic View graph contains a cycle", { path });
    active.add(node);
    try {
      if (node.stateAttachment !== undefined) {
        addAttachment(
          node.stateAttachment,
          "state",
          attachmentTargetNodeKind(node),
          path,
          stateUses,
        );
      }
      if (node.contentAttachment !== undefined) {
        addAttachment(
          node.contentAttachment,
          "content",
          attachmentTargetNodeKind(node),
          path,
          contentUses,
        );
      }
      for (const [childPath, child] of childrenOf(node)) visit(child, `${path}/${childPath}`);
    } finally {
      active.delete(node);
    }
  }

  function addAttachment(
    handleId: number,
    expectedKind: "state" | "content",
    nodeKind: number,
    path: string,
    uses: Map<number, string>,
  ): void {
    const previousPath = uses.get(handleId);
    if (previousPath !== undefined) {
      throw tuiError(
        "validation",
        expectedKind === "state"
          ? "DUPLICATE_VIEW_STATE_ATTACHMENT: duplicate state attachment (ViewState) in semantic candidate"
          : "DUPLICATE_CONTENT_PORT_ATTACHMENT: duplicate content attachment in semantic candidate",
        { handleId, firstPath: previousPath, secondPath: path },
      );
    }
    uses.set(handleId, path);
    const lease = registry.prepareResolve(
      handleId as HandleId,
      expectedKind,
      environment,
      host,
      nodeKind,
    );
    leases.push(lease);
  }
}

class PreparedAttachmentSetImpl implements PreparedAttachmentSet {
  private finished = false;

  constructor(readonly leases: readonly PreparedResourceLease[]) {}

  commitDesired(): void {
    if (this.finished) return;
    this.finished = true;
    for (const lease of this.leases) lease.commitDesired();
  }

  abort(): void {
    if (this.finished) return;
    this.finished = true;
    for (const lease of this.leases) lease.abort();
  }
}

function attachmentTargetNodeKind(node: SemanticViewNode): number {
  let target = node;
  while (target.kind === SEMANTIC_VIEW_KIND.decorated) target = target.child;
  return target.kind;
}

function childrenOf(node: SemanticViewNode): ReadonlyArray<readonly [string, SemanticViewNode]> {
  switch (node.kind) {
    case SEMANTIC_VIEW_KIND.row:
    case SEMANTIC_VIEW_KIND.column: {
      const sequence = peekSemanticSequenceOverride(node)?.sequence;
      const children = sequence === undefined ? node.children : [...sequence.values()];
      return children.map((child, index) => [`child[${index}]`, child.child] as const);
    }
    case SEMANTIC_VIEW_KIND.grid: {
      const sequence = peekSemanticGridSequenceOverride(node)?.sequence;
      if (sequence !== undefined) {
        return [...sequence.values()].map((cell, index) => [`cell[${index}]`, cell.view] as const);
      }
      return node.rows.flatMap((row, rowIndex) => row.cells.map((cell, columnIndex) => [
        `row[${rowIndex}].cell[${columnIndex}]`,
        cell.view,
      ] as const));
    }
    case SEMANTIC_VIEW_KIND.hanging:
      return [["prefix", node.prefix], ["continuation", node.continuation], ["body", node.body]];
    case SEMANTIC_VIEW_KIND.container:
    case SEMANTIC_VIEW_KIND.clamp:
    case SEMANTIC_VIEW_KIND.contentMax:
    case SEMANTIC_VIEW_KIND.decorated:
      return [["child", node.child]];
    case SEMANTIC_VIEW_KIND.text:
    case SEMANTIC_VIEW_KIND.diff:
    case SEMANTIC_VIEW_KIND.spacer:
    case SEMANTIC_VIEW_KIND.component:
      return [];
  }
}
