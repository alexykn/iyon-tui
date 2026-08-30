import { expect, test } from "bun:test";

import {
  Style,
  StyleRef,
  StyleSelector,
  TextSpan,
  Theme,
  View,
} from "../src/index.ts";
import { AppHarness } from "../src/testing/index.ts";

const PERF13B = "PERF-13-B retained presentation state";

function indexed(value: number) {
  return { type: "indexed" as const, value };
}

test(`${PERF13B} applies presentation overrides without republishing structure`, async () => {
  const tui = await AppHarness.open({ width: 12, height: 3 });
  const state = tui.viewState();
  try {
    tui.render(() => ({
      body: View.text("hello").foreground(indexed(1)).state(state),
    }));
    const host = (tui as unknown as { tui: { host: { epochs(): Record<string, string> } } }).tui.host;
    const before = host.epochs();
    expect(tui.styleAt(2, 0).foreground).toBe("ansi:1");

    state.setPresentation({ foreground: indexed(3), textAttributes: { bold: true } });
    const pending = host.epochs();
    expect(pending.desired_structural_revision).toBe(before.desired_structural_revision);
    expect(pending.pending_epoch).not.toBe(pending.committed_epoch);
    expect(tui.styleAt(2, 0).foreground).toBe("ansi:3");
    expect(tui.styleAt(2, 0).bold).toBe(true);

    tui.flush();
    const committed = host.epochs();
    expect(committed.visible_frame_revision).not.toBe(before.visible_frame_revision);

    state.clearPresentation("foreground", "textAttributes");
    tui.flush();
    expect(tui.styleAt(2, 0).foreground).toBe("ansi:1");
    expect(tui.styleAt(2, 0).bold).toBe(false);
  } finally {
    tui.close();
  }
});

test(`${PERF13B} carries state through the complete cold fallback`, async () => {
  const tui = await AppHarness.open({ width: 12, height: 3 });
  const state = tui.viewState();
  try {
    const spans = ["a", "b", "c", "d", "e"].map((text) => TextSpan.plain(text));
    tui.render(() => ({ body: View.styledText(spans).state(state) }));
    state.setPresentation({ foreground: indexed(5), textAttributes: { italic: true } });
    tui.flush();
    expect(tui.screenRows()[2]).toContain("abcde");
    expect(tui.styleAt(2, 0).foreground).toBe("ansi:5");
    expect(tui.styleAt(2, 0).italic).toBe(true);
  } finally {
    tui.close();
  }
});

test(`${PERF13B} preserves attachment identity through later immutable modifiers`, async () => {
  const tui = await AppHarness.open({ width: 12, height: 3 });
  const state = tui.viewState();
  try {
    const view = View.text("identity").state(state).noWrap().foreground(indexed(1));
    tui.render(() => ({ body: view }));
    state.setPresentation({ foreground: indexed(6) });
    tui.flush();
    expect(tui.styleAt(2, 0).foreground).toBe("ansi:6");
    expect(tui.screenRows()[2]).toContain("identity");
  } finally {
    tui.close();
  }
});

test(`${PERF13B} updates an existing border without changing its box geometry`, async () => {
  const tui = await AppHarness.open({ width: 12, height: 3 });
  const state = tui.viewState();
  try {
    const border = { style: "plain" as const, edges: "all" as const };
    tui.render(() => ({ body: View.text("x").border(border).state(state) }));
    expect(tui.screenRows()[1]).toContain("x");
    const before = tui.screenRows();
    state.setPresentation({ borderColor: indexed(5), borderStyle: "rounded" });
    tui.flush();
    const after = tui.screenRows();
    expect(after.map((row) => row.length)).toEqual(before.map((row) => row.length));
    expect(after[0]).toContain("╭");
    expect(tui.styleAt(1, 0).foreground).toBe("ansi:5");
  } finally {
    tui.close();
  }
});

test(`${PERF13B} preserves unmounted overrides and distinguishes null from clear`, async () => {
  const tui = await AppHarness.open({ width: 12, height: 3 });
  const state = tui.viewState();
  try {
    state.setPresentation({ foreground: indexed(4), background: indexed(2) });
    tui.render(() => ({ body: View.text("base").foreground(indexed(1)).state(state) }));
    expect(tui.styleAt(2, 0).foreground).toBe("ansi:4");
    expect(tui.styleAt(2, 0).background).toBe("ansi:2");

    state.setPresentation({ foreground: null });
    tui.flush();
    expect(tui.styleAt(2, 0).foreground).toBe(null);
    state.clearPresentation(["foreground"]);
    tui.flush();
    expect(tui.styleAt(2, 0).foreground).toBe("ansi:1");
  } finally {
    tui.close();
  }
});

test(`${PERF13B} rejects wrong-host and duplicate state attachments during prepare`, async () => {
  const first = await AppHarness.open({ width: 12, height: 3 });
  const second = await AppHarness.open({ width: 12, height: 3 });
  const state = first.viewState();
  try {
    const attached = View.text("shared").state(state);
    first.render(() => ({ body: attached }));
    const before = first.screenRows();
    expect(() => second.render(() => ({ body: attached }))).toThrow("different host");
    expect(first.screenRows()).toEqual(before);
    expect(() => first.render(() => ({ body: View.vertical([attached, attached]) }))).toThrow("duplicate state attachment");
    expect(first.screenRows()).toEqual(before);
  } finally {
    first.close();
    second.close();
  }
});

test(`${PERF13B} preserves overrides across a same-host remount and clear reveals the new base`, async () => {
  const tui = await AppHarness.open({ width: 12, height: 3 });
  const state = tui.viewState();
  try {
    state.setPresentation({ foreground: indexed(4) });
    tui.render(() => ({ body: View.text("first").foreground(indexed(1)).state(state) }));
    expect(tui.styleAt(2, 0).foreground).toBe("ansi:4");
    tui.render(() => ({ body: View.text("second").foreground(indexed(2)).state(state) }));
    expect(tui.screenRows()[2]).toContain("second");
    expect(tui.styleAt(2, 0).foreground).toBe("ansi:4");
    state.clearPresentation(["foreground"]);
    tui.flush();
    expect(tui.styleAt(2, 0).foreground).toBe("ansi:2");
  } finally {
    tui.close();
  }
});

test(`${PERF13B} applies style-state overrides through the native selector cascade`, async () => {
  const tui = await AppHarness.open({ width: 12, height: 3 });
  const state = tui.viewState();
  try {
    const theme = Theme.new()
      .withStyle("status", Style.new())
      .withStyleVariant("status", StyleSelector.state("status", "error"), Style.new().bold());
    // The public Tui theme API remains immutable; the attached state only
    // changes the selector input and never reconstructs the semantic View.
    tui.setTheme(theme);
    tui.render(() => ({ body: View.text("state").style(StyleRef.theme("status")).state(state) }));
    expect(tui.styleAt(2, 0).bold).toBe(false);
    state.setStyleState("status", "error");
    tui.flush();
    expect(tui.styleAt(2, 0).bold).toBe(true);
    state.clearStyleState("status");
    tui.flush();
    expect(tui.styleAt(2, 0).bold).toBe(false);
  } finally {
    tui.close();
  }
});

test(`${PERF13B} rejects disposed state use after an unmounted disposal`, async () => {
  const tui = await AppHarness.open({ width: 12, height: 3 });
  const state = tui.viewState();
  try {
    state.dispose();
    expect(state.disposed).toBe(true);
    expect(() => state.setPresentation({ foreground: indexed(1) })).toThrow("disposed");
  } finally {
    tui.close();
  }
});

test(`${PERF13B} rejects component indirection and mounted disposal`, async () => {
  const tui = await AppHarness.open({ width: 12, height: 3 });
  const state = tui.viewState();
  const slot = tui.createViewSlot(View.text("slot"));
  try {
    expect(() => tui.render(() => ({ body: slot.view().state(state) }))).toThrow("unsupported");
    tui.render(() => ({ body: View.vertical([slot.view(), View.text("state").state(state)]) }));
    expect(() => state.dispose()).toThrow("attached");
  } finally {
    tui.close();
  }
});

test(`${PERF13B} rejects invalid patches atomically`, async () => {
  const tui = await AppHarness.open({ width: 12, height: 3 });
  const state = tui.viewState();
  try {
    tui.render(() => ({ body: View.text("stable").state(state) }));
    const host = (tui as unknown as { tui: { host: { epochs(): Record<string, string> } } }).tui.host;
    const before = host.epochs();
    expect(() => state.setPresentation({
      textAttributes: { bold: "yes" as unknown as boolean },
    })).toThrow("boolean");
    expect(host.epochs()).toEqual(before);
    expect(() => state.setPresentation({
      borderGlyphs: { top: "too-wide", right: "│", bottom: "─", left: "│", topLeft: "┌", topRight: "┐", bottomLeft: "└", bottomRight: "┘" },
    })).toThrow();
    expect(host.epochs()).toEqual(before);
  } finally {
    tui.close();
  }
});
