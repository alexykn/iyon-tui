/**
 * PERF-12 T13.1 R0 — compose-helper fall-through parity (handoff §32.1 R0,
 * AMENDMENT-C §17.3).
 *
 * In R0 no retained-execution runtime exists, so every compose helper MUST be
 * semantically identical to ordinary eager construction (§19 fall-through).
 * These tests pin that parity per semantic family at the BridgeViewNode
 * level, so the R1 scoped arm can be added against a proven baseline:
 *
 *   - helper output vs direct output: equivalent bridge semantics;
 *   - helpers allocate fresh immutable Views every call (no hidden caching);
 *   - modifier chains match their direct equivalents;
 *   - component-handle id normalization matches View.component exactly.
 */

import { describe, expect, test } from "bun:test";
import {
  composeBackground,
  composeBorder,
  composeClampRows,
  composeComponent,
  composeContainer,
  composeContentMax,
  composeDiff,
  composeFillWidth,
  composeForeground,
  composeHanging,
  composeHorizontal,
  composePadding,
  composeSpacer,
  composeStyleSpec,
  composeStyleState,
  composeStyledText,
  composeText,
  composeTextAlign,
  composeTextAttribute,
  composeVertical,
  composeWrap,
} from "../src/tui/internal-composition.ts";
import { BRIDGE_VIEW_KIND, type BridgeViewNode } from "../src/tui/ir.ts";
import { ChildrenBuilder, View, nodeForBridge } from "../src/tui/values/view.ts";
import { Style } from "../src/tui/values/style.ts";
import { TextSpan } from "../src/tui/values/text.ts";
import { Insets } from "../src/tui/values/geometry.ts";

/** Bridge-level semantic equality: frozen normalized records compare deeply. */
function expectSameBridge(actual: View, expected: View): void {
  // NodeIds differ per construction (including nested children); everything
  // else must be identical.
  const stripIds = (_key: string, value: unknown): unknown =>
    _key === "id" ? undefined : value;
  expect(JSON.stringify(nodeForBridge(actual), stripIds))
    .toBe(JSON.stringify(nodeForBridge(expected), stripIds));
}

describe("T13.1 R0 compose helpers — §19 fall-through parity", () => {
  test("text: composed equals direct", () => {
    expectSameBridge(composeText("footer"), View.text("footer"));
    const node = nodeForBridge(composeText("hi"));
    expect(node.kind).toBe(BRIDGE_VIEW_KIND.text);
  });

  test("styledText: composed equals direct, spans preserved", () => {
    const style = Style.new().foreground("#ff8000");
    const spans = (): TextSpan[] => [TextSpan.plain("plain "), TextSpan.styled("hot", style)];
    expectSameBridge(composeStyledText(spans()), View.styledText(spans()));
  });

  test("spacer: composed equals direct", () => {
    expectSameBridge(composeSpacer(2), View.spacer(2));
  });

  test("component: handle id normalization matches View.component", () => {
    const plain = { id: 77 as never };
    expectSameBridge(composeComponent(plain), View.component(plain));

    const native = { id: 7 as never, nativeComponentId: () => 42 };
    expect(nodeForBridge(composeComponent(native)).kind).toBe(BRIDGE_VIEW_KIND.component);
    expectSameBridge(composeComponent(native), View.component(native));
  });

  test("hanging: composed equals direct with child identity", () => {
    const prefix = composeText("» ");
    const continuation = composeText("   ");
    const body = composeStyledText([TextSpan.plain("body line")]);
    const composed = composeHanging(prefix, continuation, body);
    const direct = View.hanging(
      View.text("» "),
      View.text("   "),
      View.styledText([TextSpan.plain("body line")]),
    );
    expect(nodeForBridge(composed).kind).toBe(BRIDGE_VIEW_KIND.hanging);
    const composedNode = nodeForBridge(composed) as Extract<BridgeViewNode, { kind: typeof BRIDGE_VIEW_KIND.hanging }>;
    // Child identity is the exact bridge node of the child value we passed.
    expect(composedNode.prefix).toBe(nodeForBridge(prefix));
    expect(composedNode.continuation).toBe(nodeForBridge(continuation));
    expect(composedNode.body).toBe(nodeForBridge(body));
    expectSameBridge(composed, direct);
  });

  test("vertical/horizontal: builder callback semantics equal View.vertical/horizontal", () => {
    const build = (column: ChildrenBuilder): void => {
      column.child(composeSpacer(0));
      column.gap(1);
      column.fixed(3, composeText("fixed"));
      column.flexMax(4, composeText("flexMax"));
      column.contentMax(13, composeComponent({ id: 5 as never }));
    };
    const composedColumn = composeVertical(build);
    const directColumn = View.vertical((column) => {
      column.child(View.spacer(0));
      column.gap(1);
      column.fixed(3, View.text("fixed"));
      column.flexMax(4, View.text("flexMax"));
      column.contentMax(13, View.component({ id: 5 as never }));
    });
    expectSameBridge(composedColumn, directColumn);

    const composedRow = composeHorizontal((row) => row.child(composeText("x")));
    expect(nodeForBridge(composedRow).kind).toBe(BRIDGE_VIEW_KIND.row);
    expectSameBridge(composedRow, View.horizontal([View.text("x")]));
  });

  test("contentMax/container/clampRows: composed equals direct", () => {
    const inner = composeText("long content line");
    expectSameBridge(composeContentMax(9, inner), View.contentMax(9, View.text("long content line")));
    expectSameBridge(composeContainer(inner), View.text("long content line").container());
    expectSameBridge(
      composeClampRows(inner, 3),
      View.text("long content line").clampRows(3),
    );
    expectSameBridge(
      composeClampRows(inner, 3, { kind: "ellipsis", style: Style.new().dim() }),
      View.text("long content line").clampRows(3, { kind: "ellipsis", style: Style.new().dim() }),
    );
    expectSameBridge(
      composeClampRows(inner, 3, { kind: "footer", prefix: "…+", style: Style.new().dim() }),
      View.text("long content line").clampRows(3, { kind: "footer", prefix: "…+", style: Style.new().dim() }),
    );
  });

  test("diff: pure delegate to View.diff", () => {
    expectSameBridge(composeDiff([]), View.diff([]));
    const hunks = [{
      oldRange: { start: 0, count: 2 },
      newRange: { start: 0, count: 2 },
      lines: [
        { kind: "deletion" as const, text: "- old", termination: "terminated" as const },
        { kind: "addition" as const, text: "+ new", termination: "terminated" as const },
      ],
    }];
    expectSameBridge(composeDiff(hunks), View.diff(hunks));
  });

  test("every modifier matches its public-method equivalent", () => {
    const base = (): View => View.text("mod");
    expectSameBridge(composeFillWidth(base()), base().fillWidth());
    expectSameBridge(composePadding(base(), 2), base().padding(2));
    expectSameBridge(composePadding(base(), Insets.of(1, 2, 3, 4)), base().padding(Insets.of(1, 2, 3, 4)));
    expectSameBridge(composeForeground(base(), "#00ff00"), base().foreground("#00ff00"));
    expectSameBridge(composeBackground(base(), "#0000ff"), base().background("#0000ff"));
    expectSameBridge(composeStyleSpec(base(), Style.new().dim().italic()), base().style(Style.new().dim().italic()));
    expectSameBridge(composeStyleState(base(), "iyon.agent.effort", "high"), base().styleState("iyon.agent.effort", "high"));
    expectSameBridge(composeTextAttribute(base(), "bold"), base().textAttribute("bold"));
    const border = { style: "rounded" as const, color: "#888888" };
    expectSameBridge(composeBorder(base(), border), base().border(border));
    expectSameBridge(composeWrap(base(), "noWrap"), base().wrap("noWrap"));
    expectSameBridge(composeTextAlign(base(), "center"), base().textAlign("center"));
  });

  test("modifier chain on decorated base equals its direct chain", () => {
    const spec = Style.new().dim();
    const composed = composeFillWidth(
      composeStyleSpec(
        composeStyleState(composeComponent({ id: 42 as never }), "iyon.agent.effort", "low"),
        spec,
      ),
    );
    const direct = View.component({ id: 42 as never })
      .styleState("iyon.agent.effort", "low")
      .style(spec)
      .fillWidth();

    const composedNode = nodeForBridge(composed);
    const directNode = nodeForBridge(direct);
    expect(composedNode.kind).toBe(BRIDGE_VIEW_KIND.decorated);
    expect(directNode.kind).toBe(BRIDGE_VIEW_KIND.decorated);
    if (composedNode.kind !== BRIDGE_VIEW_KIND.decorated || directNode.kind !== BRIDGE_VIEW_KIND.decorated) return;
    expect(JSON.stringify(composedNode.decoration)).toBe(JSON.stringify(directNode.decoration));
  });

  test("helpers never cache: repeated calls allocate distinct immutable Views", () => {
    const a = composeText("a");
    const b = composeText("a");
    expect(a).not.toBe(b);
    expect(nodeForBridge(a).id).not.toBe(nodeForBridge(b).id);

    const chainA = composeFillWidth(composeText("z"));
    const chainB = composeFillWidth(composeText("z"));
    expect(chainA).not.toBe(chainB);
  });
});
