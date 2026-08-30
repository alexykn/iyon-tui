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
export class AttachmentBindingState {
  private desired: readonly PreparedResourceLease[] = [];
  private visible: readonly PreparedResourceLease[] = [];

  commitDesired(prepared: PreparedAttachmentSet): void {
    for (const lease of this.desired) lease.releaseDesired();
    prepared.commitDesired();
    this.desired = prepared.leases;
  }

  commitVisible(): void {
    for (const lease of this.visible) lease.releaseVisible();
    for (const lease of this.desired) lease.commitVisible();
    this.visible = this.desired;
  }

  dispose(): void {
    for (const lease of this.desired) lease.releaseDesired();
    for (const lease of this.visible) lease.releaseVisible();
    this.desired = [];
    this.visible = [];
  }

  desiredCount(): number { return this.desired.length; }
  visibleCount(): number { return this.visible.length; }
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
