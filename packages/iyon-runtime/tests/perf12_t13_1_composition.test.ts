/**
 * PERF-12 T13.1 Step 2 — composition runtime unit tests (§48 Step 2).
 *
 * Synthetic internal helpers drive the real runtime primitives the way the
 * Step 3 monomorphic compose helpers will: resolve slot -> reuse candidate ->
 * compare immediate semantics -> stage previous or new View. Uses real View
 * values and the lookup-only bridge accessor so NodeId identity claims are
 * checked against the actual semantic DAG.
 */

import { beforeAll, describe, expect, test } from "bun:test";
import {
  COMPOSITION_DUPLICATE_KEY,
  CompositionError,
  ViewCompositionRoot,
  activeCompositionPass,
  compositionCounterSnapshot,
  noteExactViewReuse,
  noteNewView,
  popCompositionPass,
  pushCompositionPass,
  resetCompositionCounters,
  slotReuseCandidate,
  stageSlotValue,
  withCompositionScope,
  type CompositionSlot,
  type ViewCompositionPass,
} from "../src/tui/composition.ts";
import { registerCompositionModule } from "../src/tui/composition_registry.ts";
import { BRIDGE_VIEW_KIND } from "../src/tui/ir.ts";
import { View, nodeForBridge } from "../src/tui/values/view.ts";

// --- Synthetic helper layer (the Step 3 shape in miniature). ---------------

interface Ctx {
  root: ViewCompositionRoot;
  pass: ViewCompositionPass;
}

function makeCtx(root: ViewCompositionRoot): Ctx {
  const pass = root.begin();
  const ctx = { root, pass };
  pushCompositionPass(pass);
  return ctx;
}

function endCtx(ctx: Ctx): void {
  popCompositionPass(ctx.pass);
}

/** Composed text factory: exact semantic repeat returns the exact View. */
function composeText(ctx: Ctx, moduleId: number, siteId: number, text: string): View {
  const slot = ctx.root.currentPositionalSlot(ctx.pass, moduleId, siteId);
  const previous = slotReuseCandidate(slot);
  if (previous !== undefined) {
    const node = nodeForBridge(previous);
    if (node.kind === BRIDGE_VIEW_KIND.text && node.spans.length === 1 && node.spans[0]!.text === text) {
      stageSlotValue(slot, previous);
      noteExactViewReuse();
      return previous;
    }
  }
  const view = View.text(text);
  stageSlotValue(slot, view);
  noteNewView();
  return view;
}

/** Composed keyed factory around any builder (the future View.key shape). */
function composeKeyed(ctx: Ctx, moduleId: number, siteId: number, key: string | number, build: () => View): View {
  const group = ctx.root.keyedGroup(ctx.pass, moduleId, siteId, key);
  return withCompositionScope(ctx.pass, group.scope, build);
}

// --- Tests. -----------------------------------------------------------------

describe("T13.1 Step 2 composition runtime", () => {
  let moduleA = -1;

  beforeAll(() => {
    resetCompositionCounters();
    moduleA = registerCompositionModule(64);
    void moduleA;
  });

  test("module registry assigns dense process-local ids", () => {
    const first = registerCompositionModule(3);
    const second = registerCompositionModule(7);
    expect(second).toBe(first + 1);
  });

  test("unregistered modules and out-of-range sites are deterministic errors", () => {
    const root = new ViewCompositionRoot();
    const ctx = makeCtx(root);
    expect(() => composeText(ctx, 9999, 0, "x")).toThrow(CompositionError);
    const registered = registerCompositionModule(2);
    composeText(ctx, registered, 0, "ok");
    composeText(ctx, registered, 1, "ok");
    expect(() => composeText(ctx, registered, 2, "beyond")).toThrow(CompositionError);
    try {
      composeText(ctx, 9999, 0, "x");
    } catch (error) {
      expect((error as CompositionError).code).toBe("TUI_COMPOSITION_MODULE");
    }
    endCtx(ctx);
    root.abort(ctx.pass);
  });

  test("exact semantic repeat returns the exact previous View and NodeId", () => {
    resetCompositionCounters();
    const root = new ViewCompositionRoot();
    const ctx = makeCtx(root);
    const first = composeText(ctx, 0, 0, "footer");
    endCtx(ctx);
    root.commit(ctx.pass);

    const ctx2 = makeCtx(root);
    const second = composeText(ctx2, 0, 0, "footer");
    endCtx(ctx2);
    root.commit(ctx2.pass);

    expect(second).toBe(first);
    expect(nodeForBridge(second)).toBe(nodeForBridge(first));
    const snap = compositionCounterSnapshot();
    expect(snap.composition_exact_view_reuses).toBe(1);
    expect(snap.composition_new_views).toBe(1);
    expect(snap.composition_commits).toBeGreaterThanOrEqual(2);
    expect(snap.composition_modules_touched).toBe(2);
    expect(snap.composition_positional_slot_hits).toBeGreaterThanOrEqual(1);
  });

  test("changed semantics produce a new immutable View and NodeId", () => {
    resetCompositionCounters();
    const root = new ViewCompositionRoot();
    const ctx = makeCtx(root);
    const before = composeText(ctx, 0, 1, "Working");
    endCtx(ctx);
    root.commit(ctx.pass);

    const ctx2 = makeCtx(root);
    const after = composeText(ctx2, 0, 1, "Done");
    endCtx(ctx2);
    root.commit(ctx2.pass);

    expect(after).not.toBe(before);
    expect(nodeForBridge(after).id).not.toBe(nodeForBridge(before).id);
    expect(nodeForBridge(after).kind).toBe(BRIDGE_VIEW_KIND.text);
    expect(compositionCounterSnapshot().composition_new_views).toBe(2);
  });

  test("unrelated lexical sites keep identity while another site changes", () => {
    resetCompositionCounters();
    const root = new ViewCompositionRoot();
    const ctx = makeCtx(root);
    const stable = composeText(ctx, 0, 5, "composer");
    composeText(ctx, 0, 6, "v1");
    endCtx(ctx);
    root.commit(ctx.pass);

    const ctx2 = makeCtx(root);
    const stable2 = composeText(ctx2, 0, 5, "composer");
    composeText(ctx2, 0, 6, "v2");
    endCtx(ctx2);
    root.commit(ctx2.pass);

    expect(stable2).toBe(stable);
    expect(compositionCounterSnapshot().composition_exact_view_reuses).toBe(1);
  });

  test("occurrence identity maps visits to slots by index; shrink releases tail on commit", () => {
    resetCompositionCounters();
    const root = new ViewCompositionRoot();
    const ctx = makeCtx(root);
    const three = [composeText(ctx, 0, 7, "a"), composeText(ctx, 0, 7, "b"), composeText(ctx, 0, 7, "c")];
    endCtx(ctx);
    root.commit(ctx.pass);

    const ctx2 = makeCtx(root);
    const two = [composeText(ctx2, 0, 7, "a"), composeText(ctx2, 0, 7, "b")];
    endCtx(ctx2);
    root.commit(ctx2.pass);

    expect(two[0]).toBe(three[0]);
    expect(two[1]).toBe(three[1]);
    const snap = compositionCounterSnapshot();
    expect(snap.composition_removed_positional_slots).toBe(1);
    expect(snap.composition_positional_slot_misses).toBe(3);
    expect(snap.composition_positional_slot_hits).toBe(2);
  });

  test("aborted shrink keeps the committed tail", () => {
    resetCompositionCounters();
    const root = new ViewCompositionRoot();
    const ctx = makeCtx(root);
    const committed = [composeText(ctx, 0, 8, "a"), composeText(ctx, 0, 8, "b")];
    endCtx(ctx);
    root.commit(ctx.pass);

    const ctx2 = makeCtx(root);
    composeText(ctx2, 0, 8, "a");
    endCtx(ctx2);
    root.abort(ctx2.pass);

    const ctx3 = makeCtx(root);
    const again = [composeText(ctx3, 0, 8, "a"), composeText(ctx3, 0, 8, "b")];
    endCtx(ctx3);
    root.commit(ctx3.pass);
    expect(again[0]).toBe(committed[0]);
    expect(again[1]).toBe(committed[1]);
    expect(compositionCounterSnapshot().composition_removed_positional_slots).toBe(0);
  });

  test("aborted repeated-site growth releases pending tail shells", () => {
    resetCompositionCounters();
    const root = new ViewCompositionRoot();
    const initial = makeCtx(root);
    composeText(initial, 0, 30, "stable");
    endCtx(initial);
    root.commit(initial.pass);

    for (let attempt = 0; attempt < 8; attempt += 1) {
      const failed = makeCtx(root);
      for (let occurrence = 0; occurrence < 32; occurrence += 1) composeText(failed, 0, 30, `pending-${attempt}-${occurrence}`);
      endCtx(failed);
      root.abort(failed.pass);
    }

    const committed = makeCtx(root);
    composeText(committed, 0, 30, "stable");
    endCtx(committed);
    root.commit(committed.pass);
    // If aborted shells survived, the successful one-occurrence commit would
    // report a large removed tail. It must not.
    expect(compositionCounterSnapshot().composition_removed_positional_slots).toBe(0);
  });

  test("conditional absence breaks continuity: reappearing site is a fresh mount (§25.5)", () => {
    resetCompositionCounters();
    const root = new ViewCompositionRoot();
    const ctx = makeCtx(root);
    const mounted = composeText(ctx, 0, 9, "working");
    endCtx(ctx);
    root.commit(ctx.pass);

    // Site absent for one committed pass (a different site runs instead).
    const ctx2 = makeCtx(root);
    composeText(ctx2, 0, 10, "spacer");
    endCtx(ctx2);
    root.commit(ctx2.pass);

    // Reappearance: same payload, but NOT continuously mounted -> fresh View.
    resetCompositionCounters();
    const ctx3 = makeCtx(root);
    const returned = composeText(ctx3, 0, 9, "working");
    endCtx(ctx3);
    root.commit(ctx3.pass);

    expect(returned).not.toBe(mounted);
    expect(nodeForBridge(returned).id).not.toBe(nodeForBridge(mounted).id);
    expect(compositionCounterSnapshot().composition_new_views).toBe(1);
  });

  test("keyed groups: same key resumes identity, reorder preserves items, removal releases on commit", () => {
    resetCompositionCounters();
    const root = new ViewCompositionRoot();
    const ctx = makeCtx(root);
    const alpha = composeKeyed(ctx, 0, 11, "alpha", () => composeText(ctx, 0, 12, "alpha-row"));
    const beta = composeKeyed(ctx, 0, 11, "beta", () => composeText(ctx, 0, 12, "beta-row"));
    endCtx(ctx);
    root.commit(ctx.pass);

    // Reorder: beta visited before alpha; identities follow keys.
    const ctx2 = makeCtx(root);
    const beta2 = composeKeyed(ctx2, 0, 11, "beta", () => composeText(ctx2, 0, 12, "beta-row"));
    const alpha2 = composeKeyed(ctx2, 0, 11, "alpha", () => composeText(ctx2, 0, 12, "alpha-row"));
    endCtx(ctx2);
    root.commit(ctx2.pass);
    expect(beta2).toBe(beta);
    expect(alpha2).toBe(alpha);

    // Removal: alpha absent this pass -> its group is released at commit.
    const ctx3 = makeCtx(root);
    const beta3 = composeKeyed(ctx3, 0, 11, "beta", () => composeText(ctx3, 0, 12, "beta-row"));
    endCtx(ctx3);
    root.commit(ctx3.pass);
    expect(beta3).toBe(beta);
    const snap = compositionCounterSnapshot();
    expect(snap.composition_removed_keyed_slots).toBe(1);
    expect(snap.composition_keyed_slot_hits).toBeGreaterThanOrEqual(3);
  });

  test("committed absence releases an entire positional/keyed site surface", () => {
    resetCompositionCounters();
    const root = new ViewCompositionRoot();
    const ctx = makeCtx(root);
    composeText(ctx, 0, 27, "one");
    composeText(ctx, 0, 27, "two");
    composeKeyed(ctx, 0, 28, "gone", () => composeText(ctx, 0, 29, "nested"));
    endCtx(ctx);
    root.commit(ctx.pass);

    // An empty successful pass unmounts the previously active site surface.
    const empty = makeCtx(root);
    endCtx(empty);
    root.commit(empty.pass);
    const snap = compositionCounterSnapshot();
    expect(snap.composition_removed_positional_slots).toBeGreaterThanOrEqual(3);
    expect(snap.composition_removed_keyed_slots).toBeGreaterThanOrEqual(1);
  });

  test("key change resets logical lifetime; duplicate keys are a deterministic error", () => {
    resetCompositionCounters();
    const root = new ViewCompositionRoot();
    const ctx = makeCtx(root);
    const original = composeKeyed(ctx, 0, 13, "a", () => composeText(ctx, 0, 14, "same payload"));
    endCtx(ctx);
    root.commit(ctx.pass);

    const ctx2 = makeCtx(root);
    const renamed = composeKeyed(ctx2, 0, 13, "b", () => composeText(ctx2, 0, 14, "same payload"));
    endCtx(ctx2);
    root.commit(ctx2.pass);
    expect(renamed).not.toBe(original);

    const ctx3 = makeCtx(root);
    composeKeyed(ctx3, 0, 13, "dup", () => View.text("x"));
    expect(() => composeKeyed(ctx3, 0, 13, "dup", () => View.text("y"))).toThrow(CompositionError);
    try {
      composeKeyed(ctx3, 0, 13, "dup", () => View.text("y"));
    } catch (error) {
      expect((error as CompositionError).code).toBe(COMPOSITION_DUPLICATE_KEY);
    }
    endCtx(ctx3);
    root.abort(ctx3.pass);
    expect(compositionCounterSnapshot().composition_duplicate_key_errors).toBeGreaterThanOrEqual(1);
  });

  test("abort retains the committed keyed map exactly (§11.7) and releases pending state", () => {
    const root = new ViewCompositionRoot();
    const ctx = makeCtx(root);
    const committed = composeKeyed(ctx, 0, 15, "keep", () => composeText(ctx, 0, 16, "committed"));
    endCtx(ctx);
    root.commit(ctx.pass);

    const ctx2 = makeCtx(root);
    composeKeyed(ctx2, 0, 15, "keep", () => composeText(ctx2, 0, 16, "pending change"));
    composeKeyed(ctx2, 0, 15, "fresh", () => composeText(ctx2, 0, 16, "new"));
    endCtx(ctx2);
    root.abort(ctx2.pass);

    // Retry from the same application state: committed identity intact.
    const ctx3 = makeCtx(root);
    const retried = composeKeyed(ctx3, 0, 15, "keep", () => composeText(ctx3, 0, 16, "committed"));
    endCtx(ctx3);
    root.commit(ctx3.pass);
    expect(retried).toBe(committed);
  });

  test("nested keyed scopes isolate inner site occurrences (§39)", () => {
    const root = new ViewCompositionRoot();
    const ctx = makeCtx(root);
    const toolA = composeKeyed(ctx, 0, 17, "tool-a", () => composeText(ctx, 0, 18, "inner"));
    const toolB = composeKeyed(ctx, 0, 17, "tool-b", () => composeText(ctx, 0, 18, "inner"));
    endCtx(ctx);
    root.commit(ctx.pass);
    // Same inner lexical site executed inside two keyed groups: independent
    // slots, therefore distinct Views despite identical payload.
    expect(toolA).not.toBe(toolB);

    const ctx2 = makeCtx(root);
    const toolA2 = composeKeyed(ctx2, 0, 17, "tool-a", () => composeText(ctx2, 0, 18, "inner"));
    endCtx(ctx2);
    root.commit(ctx2.pass);
    expect(toolA2).toBe(toolA);
  });

  test("same key at different lexical sites and different roots is independent", () => {
    const root = new ViewCompositionRoot();
    const ctx = makeCtx(root);
    const site1 = composeKeyed(ctx, 0, 19, "k", () => composeText(ctx, 0, 20, "one"));
    const site2 = composeKeyed(ctx, 0, 21, "k", () => composeText(ctx, 0, 22, "two"));
    endCtx(ctx);
    root.commit(ctx.pass);
    expect(site1).not.toBe(site2);

    const otherRoot = new ViewCompositionRoot();
    const ctxOther = makeCtx(otherRoot);
    const elsewhere = composeKeyed(ctxOther, 0, 19, "k", () => composeText(ctxOther, 0, 20, "one"));
    endCtx(ctxOther);
    otherRoot.commit(ctxOther.pass);
    expect(elsewhere).not.toBe(site1);
  });

  test("failure after staging: exception propagates, committed composition untouched, retry succeeds", () => {
    resetCompositionCounters();
    const root = new ViewCompositionRoot();
    const ctx = makeCtx(root);
    const good = composeText(ctx, 0, 23, "stable");
    endCtx(ctx);
    root.commit(ctx.pass);

    // Simulate a builder that stages then fails downstream: the runtime's
    // abort path must release the staged pending value.
    const ctx2 = makeCtx(root);
    composeText(ctx2, 0, 23, "changed");
    endCtx(ctx2);
    root.abort(ctx2.pass);

    const ctx3 = makeCtx(root);
    const retried = composeText(ctx3, 0, 23, "stable");
    endCtx(ctx3);
    root.commit(ctx3.pass);
    expect(retried).toBe(good);
    expect(compositionCounterSnapshot().composition_aborts).toBe(1);
  });

  test("async builders are rejected (§52.9)", () => {
    const root = new ViewCompositionRoot();
    const ctx = makeCtx(root);
    const unused: CompositionSlot = root.currentPositionalSlot(ctx.pass, 0, 24);
    void unused;
    expect(() => withCompositionScope(ctx.pass, ctx.pass.topScope, () => Promise.resolve(1))).toThrow(
      CompositionError,
    );
    endCtx(ctx);
    root.abort(ctx.pass);
  });

  test("active pass context nests and restores", () => {
    const outerRoot = new ViewCompositionRoot();
    const outer = outerRoot.begin();
    pushCompositionPass(outer);
    expect(activeCompositionPass()).toBe(outer);
    const innerRoot = new ViewCompositionRoot();
    const inner = innerRoot.begin();
    pushCompositionPass(inner);
    expect(activeCompositionPass()).toBe(inner);
    popCompositionPass(inner);
    expect(activeCompositionPass()).toBe(outer);
    popCompositionPass(outer);
    expect(activeCompositionPass()).toBeUndefined();
    innerRoot.dispose();
  });

  test("multi-root isolation: identical addresses hold independent committed Views", () => {
    const rootA = new ViewCompositionRoot();
    const ctxA = makeCtx(rootA);
    const a = composeText(ctxA, 0, 25, "shared shape");
    endCtx(ctxA);
    rootA.commit(ctxA.pass);

    const rootB = new ViewCompositionRoot();
    const ctxB = makeCtx(rootB);
    const b = composeText(ctxB, 0, 25, "shared shape");
    endCtx(ctxB);
    rootB.commit(ctxB.pass);

    expect(a).not.toBe(b);
    // Root A re-renders: unaffected by root B's activity.
    const ctxA2 = makeCtx(rootA);
    const a2 = composeText(ctxA2, 0, 25, "shared shape");
    endCtx(ctxA2);
    rootA.commit(ctxA2.pass);
    expect(a2).toBe(a);
  });

  test("dispose releases committed Views and refuses new passes (§25.6)", () => {
    const root = new ViewCompositionRoot();
    const ctx = makeCtx(root);
    composeText(ctx, 0, 26, "doomed");
    endCtx(ctx);
    root.commit(ctx.pass);
    root.dispose();
    expect(() => root.begin()).toThrow(CompositionError);
  });

  test("one root rejects concurrent passes while independent roots may nest", () => {
    const root = new ViewCompositionRoot();
    const pass = root.begin();
    expect(() => root.begin()).toThrow(CompositionError);
    root.abort(pass);
    const next = root.begin();
    root.abort(next);
  });

  test("state machine guards: double commit and commit-after-abort throw", () => {
    const root = new ViewCompositionRoot();
    const ctx = makeCtx(root);
    endCtx(ctx);
    root.commit(ctx.pass);
    expect(() => root.commit(ctx.pass)).toThrow(CompositionError);
    const ctx2 = makeCtx(root);
    endCtx(ctx2);
    root.abort(ctx2.pass);
    expect(() => root.commit(ctx2.pass)).toThrow(CompositionError);
  });
});
