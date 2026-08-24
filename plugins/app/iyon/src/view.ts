import { Insets, View } from "iyon:tui";
import { defineView, state } from "iyon:tui";
import type { ReasoningLevel } from "@iyon/sdk";
import type { History, State, TextInput, ViewSlot, View as ViewValue } from "@iyon/runtime/tui";
import type { IyonState, InfoState, PendingApproval } from "./contracts.ts";
import { MAX_COMPOSER_ROWS } from "./composer.ts";
import { approvalView } from "./approvals.ts";
import type { IyonTheme } from "./theme.ts";

/**
 * PERF-12 T13.1 R9 — production chrome decomposition (handoff §24.1).
 *
 * The chrome is expressed as `defineView` components reading tracked
 * `State` slices. Each component owns an execution scope: a state write
 * re-executes exactly the reading scope and skips unchanged siblings.
 * Props carry only stable handles and `State` references so shallow-equal
 * prop skipping keeps parents quiet.
 */

/** Tracked chrome slices mirrored from the reduced {@link IyonState}. */
export interface ChromeState {
  /** Footer text inputs (status/provider/model/effort as one slice). */
  readonly footerInfo: State<InfoState>;
  /** Composer effort style-state; separate so status edits skip it. */
  readonly effort: State<ReasoningLevel>;
  /** Root structural gate (goodbye clears the chrome). */
  readonly goodbye: State<boolean>;
  /** Working spinner visibility. */
  readonly activityVisible: State<boolean>;
  /** Steering queue preview texts (working row). */
  readonly steering: State<readonly string[]>;
  /** Approval card presence/content. */
  readonly pendingApproval: State<PendingApproval | undefined>;
}

export function createIyonChrome(): ChromeState {
  return {
    footerInfo: state<InfoState>({ status: "", provider: "", modelId: "", reasoningEffort: "medium" }),
    effort: state<ReasoningLevel>("medium"),
    goodbye: state(false),
    activityVisible: state(false),
    steering: state<readonly string[]>([]),
    pendingApproval: state<PendingApproval | undefined>(undefined),
  };
}

/**
 * Body-execution counters for the chrome scopes (diagnostic only, handoff
 * §28). Tests assert scoped invalidation through these; production never
 * reads them.
 */
export const chromeExecutionCounters = {
  root: 0,
  working: 0,
  approval: 0,
  composer: 0,
  footer: 0,
};

export function resetChromeExecutionCounters(): void {
  chromeExecutionCounters.root = 0;
  chromeExecutionCounters.working = 0;
  chromeExecutionCounters.approval = 0;
  chromeExecutionCounters.composer = 0;
  chromeExecutionCounters.footer = 0;
}

function infoEquals(a: InfoState, b: InfoState): boolean {
  return a.status === b.status && a.provider === b.provider && a.modelId === b.modelId && a.reasoningEffort === b.reasoningEffort;
}

function approvalEquals(a: PendingApproval | undefined, b: PendingApproval | undefined): boolean {
  if (a === undefined || b === undefined) return a === b;
  return String(a.approvalId) === String(b.approvalId)
    && String(a.toolCallId) === String(b.toolCallId)
    && a.toolName === b.toolName
    && JSON.stringify(a.arguments) === JSON.stringify(b.arguments);
}

/** Mirrors changed state slices into the tracked chrome states (publish-only). */
export function syncChromeStates(chrome: ChromeState, next: IyonState): void {
  if (!infoEquals(chrome.footerInfo.value, next.info)) chrome.footerInfo.set({ ...next.info });
  if (chrome.effort.value !== next.info.reasoningEffort) chrome.effort.set(next.info.reasoningEffort);
  if (chrome.goodbye.value !== next.goodbye) chrome.goodbye.set(next.goodbye);
  if (chrome.activityVisible.value !== next.activityVisible) chrome.activityVisible.set(next.activityVisible);
  const steering = next.steering;
  const currentSteering = chrome.steering.value;
  if (steering.length !== currentSteering.length || steering.some((value, index) => value !== currentSteering[index])) {
    chrome.steering.set([...steering]);
  }
  if (!approvalEquals(chrome.pendingApproval.value, next.pendingApproval)) chrome.pendingApproval.set(next.pendingApproval);
}

export interface IyonRootProps {
  readonly chrome: ChromeState;
  readonly composer: TextInput;
  readonly theme: IyonTheme;
  readonly working?: ViewSlot;
}

const SPINNER_FRAMES = ["⠋⣠", "⢁⡴", "⣠⠞", "⡴⠋", "⠞⢁"] as const;

export function workingFrames(waiting: boolean): ViewValue[] {
  return SPINNER_FRAMES.map((frame, index) => {
    const spinner = waiting ? frame : SPINNER_FRAMES[SPINNER_FRAMES.length - 1 - index];
    return View.text(`${spinner} ${waiting ? "waiting" : "Working"}`).noWrap();
  });
}

export function workingQueueTexts(steering: readonly string[]): { preview: string; extra: number } | undefined {
  const first = steering[0];
  if (first === undefined) return undefined;
  return { preview: first.split(/\s+/).filter(Boolean).join(" "), extra: steering.length - 1 };
}

export function workingQueueView(state: IyonState, theme: IyonTheme): ViewValue | undefined {
  const queue = workingQueueTexts(state.steering);
  if (queue === undefined) return undefined;
  const muted = (text: string) => View.text(text).noWrap().italic().foreground(theme.mutedColor);
  return View.horizontal((row) => {
    row.flex(muted(`Queue: ${queue.preview}`));
    if (queue.extra > 0) row.child(muted(` + ${queue.extra} more`));
  });
}

export function footerTextFromInfo(info: InfoState): string {
  const effort = { none: "None", minimal: "Minimal", low: "Low", medium: "Medium", high: "High", xhigh: "XHigh", max: "Max" }[info.reasoningEffort];
  return [info.provider, info.modelId, `effort: ${effort}`, info.status].filter((value) => value.length > 0).join(" · ");
}

export function footerText(state: Pick<IyonState, "info">): string {
  return footerTextFromInfo(state.info);
}

const Footer = defineView<{ chrome: ChromeState; theme: IyonTheme }>(({ chrome, theme }) => {
  chromeExecutionCounters.footer += 1;
  return View.text(footerTextFromInfo(chrome.footerInfo.value)).style(theme.footer).fillWidth();
});

const ComposerChrome = defineView<{ chrome: ChromeState; composer: TextInput; theme: IyonTheme }>(({ chrome, composer, theme }) => {
  chromeExecutionCounters.composer += 1;
  return View.component(composer)
    .style(theme.composer)
    .styleState("iyon.agent.effort", chrome.effort.value)
    .fillWidth();
});

const Working = defineView<{ chrome: ChromeState; working?: ViewSlot; theme: IyonTheme }>(({ chrome, working, theme }) => {
  chromeExecutionCounters.working += 1;
  if (!chrome.activityVisible.value || working === undefined) return View.spacer(0);
  const queue = workingQueueTexts(chrome.steering.value);
  const muted = (text: string) => View.text(text).noWrap().italic().foreground(theme.mutedColor);
  return View.horizontal((row) => {
    row.gap(4);
    row.child(View.component(working));
    if (queue !== undefined) {
      row.flex(muted(`Queue: ${queue.preview}`));
      if (queue.extra > 0) row.child(muted(` + ${queue.extra} more`));
    }
  }).fillWidth().padding(Insets.of(0, 2, 1, 2));
});

const Approval = defineView<{ chrome: ChromeState }>(({ chrome }) => {
  chromeExecutionCounters.approval += 1;
  const pending = chrome.pendingApproval.value;
  return pending === undefined ? View.spacer(0) : approvalView(pending);
});

/**
 * Root chrome body. Reads only `goodbye` directly; every other slice is read
 * inside the owning child scope, so footer/effort/activity changes never
 * re-execute this body.
 */
export const IyonRootView = defineView<IyonRootProps>((props) => {
  chromeExecutionCounters.root += 1;
  if (props.chrome.goodbye.value) return View.spacer(0);
  return View.vertical((column) => {
    column.child(Working({ chrome: props.chrome, working: props.working, theme: props.theme }));
    column.child(Approval({ chrome: props.chrome }));
    column.contentMax(MAX_COMPOSER_ROWS, ComposerChrome({ chrome: props.chrome, composer: props.composer, theme: props.theme }));
    column.child(Footer({ chrome: props.chrome, theme: props.theme }));
  }).fillWidth().fillHeight();
});

export function userBatchView(messages: readonly string[], theme: IyonTheme): ViewValue {
  return View.vertical(messages.map((message) => View.text(message).fillWidth()))
    .fillWidth()
    .border({ style: "plain", edges: "topBottom", color: theme.inputBorder });
}
