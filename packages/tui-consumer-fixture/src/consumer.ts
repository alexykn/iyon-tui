/**
 * PERF-12 T13.1 §13.3 external-consumer fixture — consumer source.
 *
 * Rules for EVERY file in this package:
 *   - import ONLY documented public iyon-tui APIs (the "@iyon/runtime/tui"
 *     package specifier);
 *   - no internal runtime imports, no composition/compiler/plugin setup,
 *     no feature flags, no manual View memoization, no identity discipline.
 *
 * This is what a third-party library consumer's code looks like. T13.1's
 * framework bootstrap (§13) must activate retained composition for these
 * exact sources automatically; this file must never change to opt in.
 */

import { Insets, Scene, Style, Tui, View } from "@iyon/runtime/tui";
import type { History, ScrollPane, TextInput, View as ViewValue, ViewSlot } from "@iyon/runtime/tui";

export interface ConsumerState {
  readonly title: string;
  readonly status: string;
  readonly items: readonly string[];
  readonly showHint: boolean;
}

const TITLE_STYLE = Style.new().foreground("theme:text.heading");
const MUTED_STYLE = Style.new().foreground("theme:text.muted");

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
    column.child(View.component(handles.listSlot).fillWidth());
    column.contentMax(5, View.component(handles.composer).fillWidth());
    column.flexMax(4, View.component(handles.pane).fillWidth());
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
  readonly tui: Tui;
  readonly history: History;
  readonly composer: TextInput;
  readonly listSlot: ViewSlot;
  readonly pane: ScrollPane;
  render(state: ConsumerState): void;
  close(): void;
}
/** Normal public-API session setup — nothing composition-specific anywhere. */
export async function openConsumerSession(): Promise<ConsumerSession> {
  const tui = await Tui.open({ width: 60, height: 16, headless: true });
  const composer = tui.createTextInput({ multiline: false });
  const listSlot = tui.createViewSlot(View.spacer(0));
  const pane = tui.createScrollPane(View.spacer(0));
  const history = tui.createHistory();
  return {
    tui,
    history,
    composer,
    listSlot,
    pane,
    render(state: ConsumerState): void {
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
