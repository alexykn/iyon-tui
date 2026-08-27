/**
 * PERF-12 T13.1 §13.3 external-consumer fixture — consumer source.
 *
 * Rules for EVERY file in this package:
 *   - import ONLY documented public iyon-tui APIs (the "@iyon/tui" root
 *     or "@iyon/tui/testing" testing entrypoint);
 *   - no internal runtime imports, no composition/compiler/plugin setup,
 *     no feature flags, no manual View memoization, no identity discipline.
 *
 * This is what a third-party library consumer's code looks like. T13.1's
 * framework bootstrap (§13) must activate retained composition for these
 * exact sources automatically; this file must never change to opt in.
 */

import { Insets, Scene, Style, View } from "@iyon/tui";
import { AppHarness } from "@iyon/tui/testing";
import type { History, ScrollPane, TextInput, TuiRuntime, View as ViewValue, ViewSlot } from "@iyon/tui";

export interface ConsumerState {
  readonly title: string;
  readonly status: string;
  readonly items: readonly string[];
  readonly showHint: boolean;
}

const TITLE_STYLE = Style.new().foreground({ type: "theme", key: "text.heading" });
const MUTED_STYLE = Style.new().foreground({ type: "theme", key: "text.muted" });

export function consumerFooterText(state: ConsumerState): string {
  return `${state.title} \u00b7 ${state.status}`;
}

/** Ordinary declarative chrome: vertical[title, hint?, list slot, composer, pane, footer]. */
export function buildConsumerBody(
  state: ConsumerState,
  handles: { composer: TextInput; listSlot: ViewSlot; pane: ScrollPane },
): ViewValue {
  return View.vertical((column) => {
    column.child(View.text(state.title).style(TITLE_STYLE).fillWidth());
    if (state.showHint) {
      column.child(View.text("type a message; ctrl+c exits").style(MUTED_STYLE).fillWidth());
    }
    column.child(handles.listSlot.view().fillWidth());
    column.contentMax(5, handles.composer.view().fillWidth());
    column.flexMax(4, handles.pane.view().fillWidth());
    column.child(View.text(consumerFooterText(state)).style(MUTED_STYLE).fillWidth());
  })
    .fillWidth()
    .padding(Insets.of(0, 1, 1, 1))
    .fillHeight();
}

/** Repeated unkeyed rows from one lexical site (occurrence identity later). */
export function buildItemRows(items: readonly string[]): ViewValue {
  return View.vertical((column) => {
    for (const item of items) {
      column.child(View.text(`- ${item}`).fillWidth());
    }
  }).fillWidth();
}

export interface ConsumerSession {
  readonly tui: AppHarness;
  readonly history: History;
  readonly composer: TextInput;
  readonly listSlot: ViewSlot;
  readonly pane: ScrollPane;
  /** R9: body-execution counter for the direct-path chrome (diagnostic). */
  readonly chromeExecutions: () => number;
  render(state: ConsumerState): void;
  close(): void;
}
/** Normal public-API session setup — nothing composition-specific anywhere. */
export async function openConsumerSession(): Promise<ConsumerSession> {
  const tui = await AppHarness.open({ width: 60, height: 16 });
  const composer = tui.createTextInput({ multiline: false });
  const listSlot = tui.createViewSlot(View.spacer(0));
  const pane = tui.createScrollPane(View.spacer(0));
  const history = tui.createHistory();
  let chromeRenderCount = 0;
  return {
    tui,
    history,
    composer,
    listSlot,
    pane,
    chromeExecutions: () => chromeRenderCount,
    render(state: ConsumerState): void {
      chromeRenderCount += 1;
      tui.render(new Scene(buildConsumerBody(state, { composer, listSlot, pane }), history));
    },
    close(): void {
      composer.dispose();
      listSlot.dispose();
      pane.dispose();
      history.dispose();
      tui.close();
    },
  };
}


// --- PERF-12 T13.1 R9: componentized consumer (public APIs ONLY) -----------
//
// Ordinary defineView/state/View.key usage with ZERO framework setup.
// The Tui-owned retained runtime activates automatically for these sources.

import { defineView as defineViewPublic } from "@iyon/tui";
import { state as createStatePublic } from "@iyon/tui";

export interface ScopedEntry {
  readonly id: string;
  readonly label: string;
}

export interface ScopedConsumer {
  readonly status: ReturnType<typeof createStatePublic<string>>;
  readonly items: ReturnType<typeof createStatePublic<readonly ScopedEntry[]>>;
  readonly headerExecutions: () => number;
  readonly cardExecutions: (id: string) => number;
  /** Renders the scoped App tree through the Tui's canonical render path. */
  readonly renderApp: () => void;
}

export function buildScopedConsumer(tui: Pick<TuiRuntime, "render">): ScopedConsumer {
  const status = createStatePublic<string>("ready");
  const items = createStatePublic<readonly ScopedEntry[]>([
    { id: "a", label: "alpha" },
    { id: "b", label: "beta" },
  ]);

  const headerCount = { n: 0 };
  const cardCounts = new Map<string, number>();

  const Header = defineViewPublic(() => {
    headerCount.n += 1;
    return View.text(`header [${status.value}]`);
  });

  const ItemCard = defineViewPublic<ScopedEntry>((entry) => {
    cardCounts.set(entry.id, (cardCounts.get(entry.id) ?? 0) + 1);
    return View.text(`item ${entry.id}: ${entry.label}`);
  });

  const Footer = defineViewPublic(() => View.text("footer"));

  const App = defineViewPublic(() =>
    View.vertical((column) => {
      column.child(Header({}));
      for (const entry of items.value) {
        column.child(View.key(entry.id, () => ItemCard(entry)));
      }
      column.child(Footer({}));
    }),
  );

  return {
    status,
    items,
    headerExecutions: () => headerCount.n,
    cardExecutions: (id) => cardCounts.get(id) ?? 0,
    renderApp: () => {
      tui.render(() => new Scene(App({})));
    },
  };
}
