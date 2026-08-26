const BRANCH_FACTOR = 32;

/**
 * PERF-12 §91} structural counters (compile-time-cheap arm: plain field
 * increments on already-executing mutation paths, no scans, no allocation).
 * They prove the O(log₃₂ N) asymptotic of retained wide edits independently
 * of timing noise: one `set` at width 100,000 clones a bounded handful of
 * nodes regardless of width. `nodes_cloned` counts every PersistentSeqNode
 * allocated by a mutation (branches and leaves); `items_iterated` counts
 * leaf item copies those allocations touch.
 */
export const persistentSeqCounters = {
  nodes_cloned: 0,
  branches_cloned: 0,
  items_iterated: 0,
};

export function resetPersistentSeqCounters(): void {
  persistentSeqCounters.nodes_cloned = 0;
  persistentSeqCounters.branches_cloned = 0;
  persistentSeqCounters.items_iterated = 0;
}

function countLeafClone<T>(items: readonly T[]): T[] {
  const copy = [...items];
  persistentSeqCounters.nodes_cloned += 1;
  persistentSeqCounters.items_iterated += copy.length;
  return copy;
}

function countBranchClone<T>(children: readonly PersistentSeqNode<T>[]): PersistentSeqNode<T>[] {
  persistentSeqCounters.nodes_cloned += 1;
  persistentSeqCounters.branches_cloned += 1;
  return [...children];
}

function countLeafMerge<T>(left: readonly T[], right: readonly T[]): T[] {
  const merged = [...left, ...right];
  persistentSeqCounters.items_iterated += merged.length;
  return merged;
}

export type SeqAggregate = number;

export type PersistentSeqNode<T> =
  | {
      readonly kind: "leaf";
      readonly items: readonly T[];
      readonly length: number;
      readonly height: 0;
      readonly aggregate: SeqAggregate;
    }
  | {
      readonly kind: "branch";
      readonly children: readonly PersistentSeqNode<T>[];
      readonly sizes: readonly number[];
      readonly length: number;
      readonly height: number;
      readonly aggregate: SeqAggregate;
    };

type Aggregate<T> = (value: T) => SeqAggregate;

type InsertResult<T> = readonly [PersistentSeqNode<T>, ...PersistentSeqNode<T>[]];

type Frame<T> = { readonly node: PersistentSeqNode<T>; index: number };

function freezeNode<T>(node: PersistentSeqNode<T>): PersistentSeqNode<T> {
  return Object.freeze(node);
}

function leaf<T>(items: readonly T[], aggregate: Aggregate<T>): PersistentSeqNode<T> {
  let flags = 0;
  for (const item of items) flags |= aggregate(item);
  return freezeNode({ kind: "leaf", items: Object.freeze([...items]), length: items.length, height: 0, aggregate: flags });
}

function branch<T>(children: readonly PersistentSeqNode<T>[]): PersistentSeqNode<T> {
  if (children.length === 0) return leaf([], () => 0);
  const sizes: number[] = [];
  let length = 0;
  let flags = 0;
  let height = 0;
  for (const child of children) {
    length += child.length;
    sizes.push(length);
    flags |= child.aggregate;
    height = Math.max(height, child.height + 1);
  }
  return freezeNode({
    kind: "branch",
    children: Object.freeze([...children]),
    sizes: Object.freeze(sizes),
    length,
    height,
    aggregate: flags,
  });
}

function build<T>(items: readonly T[], aggregate: Aggregate<T>): PersistentSeqNode<T> {
  if (items.length <= BRANCH_FACTOR) return leaf(items, aggregate);
  let level: PersistentSeqNode<T>[] = [];
  for (let index = 0; index < items.length; index += BRANCH_FACTOR) {
    level.push(leaf(items.slice(index, index + BRANCH_FACTOR), aggregate));
  }
  while (level.length > BRANCH_FACTOR) {
    const next: PersistentSeqNode<T>[] = [];
    for (let index = 0; index < level.length; index += BRANCH_FACTOR) {
      next.push(branch(level.slice(index, index + BRANCH_FACTOR)));
    }
    level = next;
  }
  return level.length === 1 ? level[0]! : branch(level);
}

function childIndex(sizes: readonly number[], index: number): number {
  let low = 0;
  let high = sizes.length;
  while (low < high) {
    const middle = (low + high) >>> 1;
    if (index < sizes[middle]!) high = middle;
    else low = middle + 1;
  }
  return low;
}

function setNode<T>(node: PersistentSeqNode<T>, index: number, value: T, aggregate: Aggregate<T>): PersistentSeqNode<T> {
  if (node.kind === "leaf") {
    const items = countLeafClone(node.items);
    items[index] = value;
    return leaf(items, aggregate);
  }
  const child = childIndex(node.sizes, index);
  const offset = child === 0 ? index : index - node.sizes[child - 1]!;
  const children = countBranchClone(node.children);
  children[child] = setNode(children[child]!, offset, value, aggregate);
  return branch(children);
}

function insertNode<T>(node: PersistentSeqNode<T>, index: number, value: T, aggregate: Aggregate<T>): InsertResult<T> {
  if (node.kind === "leaf") {
    const items = countLeafClone(node.items);
    items.splice(index, 0, value);
    if (items.length <= BRANCH_FACTOR) return [leaf(items, aggregate)];
    return [leaf(items.slice(0, BRANCH_FACTOR), aggregate), leaf(items.slice(BRANCH_FACTOR), aggregate)];
  }
  const child = index === node.length ? node.children.length - 1 : childIndex(node.sizes, index);
  const offset = child === 0 ? index : index - node.sizes[child - 1]!;
  const inserted = insertNode(node.children[child]!, offset, value, aggregate);
  const children = countBranchClone(node.children).slice(0, child).concat(inserted, node.children.slice(child + 1));
  if (children.length <= BRANCH_FACTOR) return [branch(children)];
  return [branch(children.slice(0, BRANCH_FACTOR)), branch(children.slice(BRANCH_FACTOR))];
}

function removeNode<T>(node: PersistentSeqNode<T>, index: number, aggregate: Aggregate<T>): PersistentSeqNode<T> {
  if (node.kind === "leaf") {
    const items = countLeafClone(node.items);
    items.splice(index, 1);
    return leaf(items, aggregate);
  }
  const child = childIndex(node.sizes, index);
  const offset = child === 0 ? index : index - node.sizes[child - 1]!;
  const children = countBranchClone(node.children);
  children[child] = removeNode(children[child]!, offset, aggregate);
  if (children[child]!.length === 0) children.splice(child, 1);
  return children.length === 0 ? leaf([], aggregate) : branch(children);
}

function normalizeRoot<T>(node: PersistentSeqNode<T>): PersistentSeqNode<T> {
  let current = node;
  while (current.kind === "branch" && current.children.length === 1) current = current.children[0]!;
  return current;
}

function splitNode<T>(node: PersistentSeqNode<T>, index: number, aggregate: Aggregate<T>): readonly [PersistentSeqNode<T>, PersistentSeqNode<T>] {
  if (node.kind === "leaf") return [leaf(countLeafClone(node.items.slice(0, index)), aggregate), leaf(countLeafClone(node.items.slice(index)), aggregate)];
  const child = index === node.length ? node.children.length : childIndex(node.sizes, index);
  if (child === node.children.length) return [node, leaf([], aggregate)];
  const offset = child === 0 ? index : index - node.sizes[child - 1]!;
  const childHeight = node.children[child]!.height;
  const [leftChild, rightChild] = splitNode(node.children[child]!, offset, aggregate);
  const left = wrapToHeight(leftChild, childHeight);
  const right = wrapToHeight(rightChild, childHeight);
  const leftChildren = countBranchClone(node.children).slice(0, child);
  if (left.length !== 0) leftChildren.push(left);
  const rightChildren = [...(right.length === 0 ? [] : [right]), ...node.children.slice(child + 1)];
  return [
    leftChildren.length === 0 ? leaf([], aggregate) : branch(leftChildren),
    rightChildren.length === 0 ? leaf([], aggregate) : branch(rightChildren),
  ];
}

function wrapToHeight<T>(node: PersistentSeqNode<T>, height: number): PersistentSeqNode<T> {
  let current = node;
  while (current.height < height) current = branch([current]);
  return current;
}

function concatNodes<T>(left: PersistentSeqNode<T>, right: PersistentSeqNode<T>, aggregate: Aggregate<T>): readonly PersistentSeqNode<T>[] {
  if (left.kind === "leaf" && right.kind === "leaf") {
    const items = countLeafMerge(left.items, right.items);
    if (items.length <= BRANCH_FACTOR) return [leaf(items, aggregate)];
    return [leaf(items.slice(0, BRANCH_FACTOR), aggregate), leaf(items.slice(BRANCH_FACTOR), aggregate)];
  }
  if (left.kind === "branch" && right.kind === "branch" && left.height === right.height) {
    const boundary = concatNodes(left.children[left.children.length - 1]!, right.children[0]!, aggregate);
    const children = [...left.children.slice(0, -1), ...boundary, ...right.children.slice(1)];
    if (children.length <= BRANCH_FACTOR) return [branch(children)];
    return [branch(children.slice(0, BRANCH_FACTOR)), branch(children.slice(BRANCH_FACTOR))];
  }
  const height = Math.max(left.height, right.height);
  return concatNodes(wrapToHeight(left, height), wrapToHeight(right, height), aggregate);
}

export class PersistentSeq<T> {
  readonly #root: PersistentSeqNode<T>;
  readonly #aggregate: Aggregate<T>;

  private constructor(root: PersistentSeqNode<T>, aggregate: Aggregate<T>) {
    this.#root = root;
    this.#aggregate = aggregate;
  }

  static empty<T>(aggregate: Aggregate<T> = () => 0): PersistentSeq<T> {
    return new PersistentSeq(leaf([], aggregate), aggregate);
  }

  static from<T>(items: readonly T[], aggregate: Aggregate<T> = () => 0): PersistentSeq<T> {
    return new PersistentSeq(build(items, aggregate), aggregate);
  }

  get length(): number { return this.#root.length; }
  get height(): number { return this.#root.height; }
  get aggregate(): SeqAggregate { return this.#root.aggregate; }
  get root(): PersistentSeqNode<T> { return this.#root; }

  get(index: number): T | undefined {
    if (!Number.isInteger(index) || index < 0 || index >= this.length) return undefined;
    let node = this.#root;
    let offset = index;
    while (node.kind === "branch") {
      const child = childIndex(node.sizes, offset);
      if (child > 0) offset -= node.sizes[child - 1]!;
      node = node.children[child]!;
    }
    return node.items[offset];
  }

  set(index: number, value: T): PersistentSeq<T> {
    if (!Number.isInteger(index) || index < 0 || index >= this.length) throw new RangeError("persistent sequence index out of range");
    return new PersistentSeq(setNode(this.#root, index, value, this.#aggregate), this.#aggregate);
  }

  append(value: T): PersistentSeq<T> { return this.insert(this.length, value); }

  insert(index: number, value: T): PersistentSeq<T> {
    if (!Number.isInteger(index) || index < 0 || index > this.length) throw new RangeError("persistent sequence insert index out of range");
    if (this.length === 0) return new PersistentSeq(leaf([value], this.#aggregate), this.#aggregate);
    const inserted = insertNode(this.#root, index, value, this.#aggregate);
    return new PersistentSeq(normalizeRoot(inserted.length === 1 ? inserted[0]! : branch(inserted)), this.#aggregate);
  }

  remove(index: number): PersistentSeq<T> {
    if (!Number.isInteger(index) || index < 0 || index >= this.length) throw new RangeError("persistent sequence remove index out of range");
    return new PersistentSeq(normalizeRoot(removeNode(this.#root, index, this.#aggregate)), this.#aggregate);
  }

  splice(index: number, removeCount: number, ...inserted: readonly T[]): PersistentSeq<T> {
    if (!Number.isInteger(index) || index < 0 || index > this.length) throw new RangeError("persistent sequence splice index out of range");
    if (!Number.isInteger(removeCount) || removeCount < 0 || index + removeCount > this.length) throw new RangeError("persistent sequence splice count out of range");
    const [left, remainder] = this.split(index);
    const [, right] = remainder.split(removeCount);
    return left.concat(PersistentSeq.from(inserted, this.#aggregate)).concat(right);
  }

  split(index: number): readonly [PersistentSeq<T>, PersistentSeq<T>] {
    if (!Number.isInteger(index) || index < 0 || index > this.length) throw new RangeError("persistent sequence split index out of range");
    const [left, right] = splitNode(this.#root, index, this.#aggregate);
    return [new PersistentSeq(normalizeRoot(left), this.#aggregate), new PersistentSeq(normalizeRoot(right), this.#aggregate)];
  }

  concat(other: PersistentSeq<T>): PersistentSeq<T> {
    if (other.length === 0) return this as PersistentSeq<T>;
    if (this.length === 0) return other as PersistentSeq<T>;
    const nodes = concatNodes(this.#root, other.#root, this.#aggregate);
    return new PersistentSeq(normalizeRoot(nodes.length === 1 ? nodes[0]! : branch(nodes)), this.#aggregate);
  }

  toArray(): readonly T[] { return [...this]; }

  *[Symbol.iterator](): Iterator<T> {
    const stack: Frame<T>[] = [{ node: this.#root, index: 0 }];
    while (stack.length > 0) {
      const frame = stack[stack.length - 1]!;
      if (frame.node.kind === "leaf") {
        if (frame.index >= frame.node.items.length) { stack.pop(); continue; }
        yield frame.node.items[frame.index++]!;
        continue;
      }
      if (frame.index >= frame.node.children.length) { stack.pop(); continue; }
      stack.push({ node: frame.node.children[frame.index++]!, index: 0 });
    }
  }
}

export const PERSISTENT_SEQ_BRANCH_FACTOR = BRANCH_FACTOR;
