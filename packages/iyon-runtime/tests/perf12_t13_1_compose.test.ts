/**
 * PERF-12 T13.1 Step 3 — monomorphic compose helper tests (§40 shape applied
 * at helper level).
 *
 * For every supported semantic family:
 *   - untransformed direct construction vs composed construction produce
 *     equivalent bridge semantics (§40);
 *   - exact unchanged inputs return the exact previous View/NodeId (§17);
 *   - one-field mutation changes NodeId but keeps siblings stable;
 *   - child identity mutation propagates to parents;
 *   - inactive-pass fall-through is semantically identical (§19);
 *   - modifier chains behave like their direct equivalents.
 */

import { beforeAll, describe, expect, test } from "bun:test";
import {
  ViewCompositionRoot,
  compositionCounterSnapshot,
  popCompositionPass,
  pushCompositionPass,
  resetCompositionCounters,
} from "../src/tui/composition.ts";
import {
  composeBackground,
  composeClampRows,
  composeComponent,
  composeContentMax,
  composeFillWidth,
  composeHanging,
  composeHorizontal,
  composePadding,
  composeSpacer,
  composeStyle,
  composeStyleState,
  composeStyledText,
  composeText,
  composeTextAlign,
  composeTextAttribute,
  composeVertical,
  composeWrap,
} from "../src/tui/compose.ts";
import { registerCompositionModule } from "../src/tui/composition_registry.ts";
import { BRIDGE_VIEW_KIND, type BridgeViewNode, type DecorationNode } from "../src/tui/ir.ts";
import { ChildrenBuilder, View, nodeForBridge } from "../src/tui/values/view.ts";
import { Style } from "../src/tui/values/style.ts";
import { TextSpan } from "../src/tui/values/text.ts";
import { Insets } from "../src/tui/values/geometry.ts";

/**
 * One root, many passes: each runPass evaluates a builder inside a fresh
 * composition pass over the SAME root and commits it — the exact lifecycle
 * the Step 7 boundary integration will own.
 */
class Harness {
  readonly root = new ViewCompositionRoot();
  private currentPass?: ReturnType<ViewCompositionRoot["begin"]>;

  run<T>(build: () => T): T {
    const pass = this.root.begin();
    pushCompositionPass(pass);
    this.currentPass = pass;
    try {
      return build();
    } finally {
      popCompositionPass(pass);
      this.root.commit(pass);
    }
  }
}

/** Kind-narrowed bridge accessors for assertions. */
function textNodeOf(view: View): Extract<BridgeViewNode, { kind: typeof BRIDGE_VIEW_KIND.text }> {
  const node = nodeForBridge(view);
  if (node.kind !== BRIDGE_VIEW_KIND.text) throw new Error(`expected text node, got ${node.kind}`);
  return node;
}

function decoratedOf(view: View): Extract<BridgeViewNode, { kind: typeof BRIDGE_VIEW_KIND.decorated; decoration: DecorationNode }> {
  const node = nodeForBridge(view);
  if (node.kind !== BRIDGE_VIEW_KIND.decorated) throw new Error(`expected decorated node, got ${node.kind}`);
  return node as never;
}

const MODULE = 0;

// The canonical composed chrome shape (production view.ts skeleton).
function composedChrome(footerValue: string, effort: string): View {
  return composeVertical(MODULE, 0, (column) => {
    column.child(composeSpacer(MODULE, 1, 0));
    column.contentMax(13, composeFillWidth(
      MODULE,
      4,
      composeStyle(
        MODULE,
        3,
        composeStyleState(
          MODULE,
          2,
          composeComponent(MODULE, 5, { id: 77 as never }),
          "iyon.agent.effort",
          effort,
        ),
        Style.new().foreground("theme:text.muted"),
      ),
    ));
    column.child(composeFillWidth(MODULE, 7, composeStyle(MODULE, 6, composeText(MODULE, 8, footerValue), Style.new().dim())));
  });
}

describe("T13.1 Step 3 compose helpers", () => {
  let module = -1;

  beforeAll(() => {
    resetCompositionCounters();
    module = registerCompositionModule(64);
  });

  test("composed text equals direct text semantically; repeat returns exact View", () => {
    const h = new Harness();
    const composed = h.run(() => composeText(module, 10, "footer"));
    const composedNode = textNodeOf(composed);

    // Semantic equivalence with direct construction.
    const direct = textNodeOf(View.text("footer"));
    expect(JSON.stringify({ s: composedNode.spans, w: composedNode.wrap, a: composedNode.align }))
      .toBe(JSON.stringify({ s: direct.spans, w: direct.wrap, a: direct.align }));

    const snapBefore = compositionCounterSnapshot();
    const again = h.run(() => composeText(module, 10, "footer"));
    expect(again).toBe(composed);
    expect(nodeForBridge(again)).toBe(nodeForBridge(composed));
    const snapAfter = compositionCounterSnapshot();
    expect(snapAfter.composition_exact_view_reuses - snapBefore.composition_exact_view_reuses).toBe(1);
  });

  test("changed text payload yields a new NodeId", () => {
    const h = new Harness();
    const first = h.run(() => composeText(module, 11, "Working"));
    const second = h.run(() => composeText(module, 11, "Done"));
    expect(second).not.toBe(first);
    expect(nodeForBridge(second).id).not.toBe(nodeForBridge(first).id);
  });

  test("styled spans compare per-span text and style", () => {
    const style = Style.new().foreground("#ff8000");
    const spans = (): TextSpan[] => [TextSpan.plain("plain "), TextSpan.styled("hot", style)];
    const h = new Harness();
    const first = h.run(() => composeStyledText(module, 12, spans()));
    const same = h.run(() => composeStyledText(module, 12, spans()));
    const changed = h.run(() => composeStyledText(module, 12, [TextSpan.plain("different")]));
    expect(same).toBe(first);
    expect(changed).not.toBe(first);
  });

  test("modifier chains match their direct equivalents and stabilize across passes", () => {
    const spec = Style.new().dim().italic();
    const chainOnce = (): View => composeFillWidth(
      module,
      14,
      composeStyle(
        module,
        13,
        composeStyleState(
          module,
          12,
          composeComponent(module, 15, { id: 42 as never }),
          "iyon.agent.effort",
          "low",
        ),
        spec,
      ),
    );

    const directNode = decoratedOf(
      View.component({ id: 42 as never }).styleState("iyon.agent.effort", "low").style(spec).fillWidth(),
    );

    const h = new Harness();
    const chain = h.run(chainOnce);
    const composedNode = decoratedOf(chain);
    expect(composedNode.kind).toBe(BRIDGE_VIEW_KIND.decorated);
    expect(JSON.stringify(composedNode.decoration.styleStates)).toBe(JSON.stringify(directNode.decoration.styleStates));
    expect(JSON.stringify(composedNode.decoration.width)).toBe(JSON.stringify(directNode.decoration.width));
    expect(JSON.stringify(composedNode.decoration.style.attributes)).toBe(JSON.stringify(directNode.decoration.style.attributes));

    const chainAgain = h.run(chainOnce);
    expect(chainAgain).toBe(chain);
  });

  test("one-field mutation changes only the changed path", () => {
    const h = new Harness();
    const gen1 = h.run(() => {
      const base = composeText(module, 20, "stable base");
      return { base, wrapped: composeFillWidth(module, 21, base) };
    });
    const gen2 = h.run(() => {
      const base = composeText(module, 20, "stable base");
      return { base, wrapped: composeFillWidth(module, 21, base) };
    });

    let mutatedBaseNode!: BridgeViewNode;
    const gen3 = h.run(() => {
      const base = composeText(module, 20, "CHANGED");
      mutatedBaseNode = nodeForBridge(base);
      return { base, wrapped: composeFillWidth(module, 21, base) };
    });

    expect(gen2.base).toBe(gen1.base);
    expect(gen2.wrapped).toBe(gen1.wrapped);
    expect(gen3.base).not.toBe(gen1.base);
    expect(gen3.wrapped).not.toBe(gen1.wrapped);
    expect(decoratedOf(gen3.wrapped).child).toBe(mutatedBaseNode);
  });

  test("containers compare entries, scalars, and child identity", () => {
    const h = new Harness();
    const row = h.run(() => {
      const a = composeText(module, 30, "a");
      const b = composeText(module, 31, "b");
      return composeHorizontal(module, 32, (children: ChildrenBuilder) => {
        children.gap(2);
        children.child(a);
        children.flex(b);
      });
    });

    const row2 = h.run(() => {
      const a2 = composeText(module, 30, "a");
      const b2 = composeText(module, 31, "b");
      return composeHorizontal(module, 32, (children: ChildrenBuilder) => {
        children.gap(2);
        children.child(a2);
        children.flex(b2);
      });
    });
    expect(row2).toBe(row);

    // Gap change -> new node.
    const row3 = h.run(() => {
      const a3 = composeText(module, 30, "a");
      const b3 = composeText(module, 31, "b");
      return composeHorizontal(module, 32, (children: ChildrenBuilder) => {
        children.gap(3);
        children.child(a3);
        children.flex(b3);
      });
    });
    expect(row3).not.toBe(row);
  });

  test("padding/background deltas are tracked independently per site", () => {
    const h = new Harness();
    const base = h.run(() => composeText(module, 40, "card"));

    const padded = h.run(() => {
      const p = composePadding(module, 41, base, Insets.of(0, 2, 1, 2));
      return { p, t: composeBackground(module, 42, p, "#101010") };
    });
    const again = h.run(() => {
      const p = composePadding(module, 41, base, Insets.of(0, 2, 1, 2));
      return { p, t: composeBackground(module, 42, p, "#101010") };
    });
    const retinted = h.run(() => {
      const p = composePadding(module, 41, base, Insets.of(0, 2, 1, 2));
      return { p, t: composeBackground(module, 42, p, "#202020") };
    });

    expect(again.p).toBe(padded.p);
    expect(again.t).toBe(padded.t);
    expect(retinted.p).toBe(padded.p);
    expect(retinted.t).not.toBe(padded.t);
    expect(decoratedOf(retinted.t).decoration.background).toBe("#202020");
  });

  test("clampRows/hanging/contentMax/wrap/textAlign reuse on unchanged inputs", () => {
    const overflow = {
      kind: "footer" as const,
      prefix: "\u2026 more",
      style: Style.new().foreground("theme:text.muted").italic(),
    };

    const h = new Harness();
    const clamped = h.run(() => {
      const inner = composeText(module, 50, "long output");
      return {
        clamped: composeClampRows(module, 51, inner, 16, overflow),
        hung: composeHanging(
          module,
          52,
          composeText(module, 53, "\u25cf "),
          composeText(module, 54, "  "),
          inner,
        ),
        capped: composeContentMax(module, 55, 13, inner),
        nowrap: composeWrap(module, 56, inner, "noWrap"),
        aligned: composeTextAlign(module, 57, inner, "center"),
        inner,
      };
    });

    const second = h.run(() => {
      const inner = composeText(module, 50, "long output");
      return {
        clamped: composeClampRows(module, 51, inner, 16, overflow),
        hung: composeHanging(
          module,
          52,
          composeText(module, 53, "\u25cf "),
          composeText(module, 54, "  "),
          inner,
        ),
        capped: composeContentMax(module, 55, 13, inner),
        nowrap: composeWrap(module, 56, inner, "noWrap"),
        aligned: composeTextAlign(module, 57, inner, "center"),
        inner,
      };
    });

    expect(second.clamped).toBe(clamped.clamped);
    expect(second.hung).toBe(clamped.hung);
    expect(second.capped).toBe(clamped.capped);
    expect(second.nowrap).toBe(clamped.nowrap);
    expect(second.aligned).toBe(clamped.aligned);
    expect(second.inner).toBe(clamped.inner);
    // The wrap patch produces a plain-text node carrying noWrap.
    expect(textNodeOf(clamped.nowrap).wrap).toBe(textNodeOf(View.text("x").noWrap()).wrap);
  });

  test("inactive fall-through is semantically identical to active composition", () => {
    const fallbackText = composeText(module, 60, "cold");
    expect(nodeForBridge(fallbackText).kind).toBe(BRIDGE_VIEW_KIND.text);
    const fallbackChain = composeFillWidth(module, 61, fallbackText);
    const directChain = decoratedOf(View.text("cold").fillWidth());
    expect(decoratedOf(fallbackChain).decoration.width).toBe(directChain.decoration.width);
    const fallbackAxis = composeVertical(module, 62, (children: ChildrenBuilder) => {
      children.child(fallbackText);
    });
    expect(nodeForBridge(fallbackAxis).kind).toBe(BRIDGE_VIEW_KIND.column);
  });

  test("production-shape chrome: footer-only change rebuilds only the footer path; no-op reuses root", () => {
    resetCompositionCounters();
    const h = new Harness();
    h.run(() => composedChrome("status A", "medium"));

    const snapBefore = compositionCounterSnapshot();
    const second = h.run(() => composedChrome("status B", "medium"));
    const snapAfter = compositionCounterSnapshot();

    const newViews = snapAfter.composition_new_views - snapBefore.composition_new_views;
    // Footer-only: footer text + its style wrapper + its width wrapper + the
    // root column. Everything else must be exact reuse.
    expect(newViews).toBe(4);
    expect(snapAfter.composition_exact_view_reuses - snapBefore.composition_exact_view_reuses).toBeGreaterThanOrEqual(5);

    const third = h.run(() => composedChrome("status B", "medium"));
    expect(third).toBe(second); // exact semantic no-op -> exact root reuse
  });

  test("effort style-state change keeps component identity and reuses unrelated chrome", () => {
    resetCompositionCounters();
    const h = new Harness();
    h.run(() => composedChrome("status A", "medium"));

    const snapBefore = compositionCounterSnapshot();
    h.run(() => composedChrome("status A", "high"));
    const snapAfter = compositionCounterSnapshot();

    const newViews = snapAfter.composition_new_views - snapBefore.composition_new_views;
    // Composer chain (styleState wrapper + style wrapper + fillWidth wrapper)
    // + root column; spacer, component node, and the whole footer path reused.
    expect(newViews).toBeLessThanOrEqual(4);
    expect(snapAfter.composition_exact_view_reuses - snapBefore.composition_exact_view_reuses).toBeGreaterThanOrEqual(4);
  });
});
