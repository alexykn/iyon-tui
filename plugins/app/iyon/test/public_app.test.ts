import { describe, expect, test } from "bun:test";
import type { FrontendEvent } from "../src/contracts.ts";
import {
  advance,
  closeFixture,
  draft,
  openFixture,
  send,
  toolStatusCount,
  transcriptLines,
  type PublicAppFixture,
} from "./public_app_fixtures.ts";
import type { TuiRuntime } from "@iyon/tui";

async function withFixture<T>(width: number, height: number, callback: (fixture: PublicAppFixture) => Promise<T>, withQueueIds = false): Promise<T> {
  const fixture = await openFixture(width, height, withQueueIds);
  try {
    return await callback(fixture);
  } finally {
    await closeFixture(fixture);
  }
}

async function sendAll(fixture: PublicAppFixture, events: readonly FrontendEvent[]): Promise<void> {
  for (const event of events) await send(fixture, event);
}

function position(lines: readonly string[], text: string): number {
  const index = lines.findIndex((line) => line.includes(text));
  if (index < 0) throw new Error(`missing ${text} in ${lines.join("\n")}`);
  return index;
}

function styleForText(fixture: PublicAppFixture, text: string): Readonly<Record<string, unknown>> {
  const row = fixture.harness.screenRows().findIndex((line) => line.includes(text));
  if (row < 0) throw new Error(`missing ${text} in ${fixture.harness.screenRows().join("\n")}`);
  const column = fixture.harness.cellXOfText(row, text);
  if (column === null) throw new Error(`missing cell position for ${text}`);
  return fixture.harness.styleAt(row, column);
}

describe("Iyon public native TUI", () => {
  test("submit_hello_does_not_exit_the_tui", async () => {
    await withFixture(40, 12, async ({ app, harness }) => {
      for (const key of "hello") harness.pressKey(key);
      harness.pressKey("Enter");
      const action = await harness.nextAction();
      expect(action).toEqual({ actionId: "submit", payload: "hello" });
      await app.handleAction({ type: "submit", text: action?.payload ?? "" });
      await send({ harness, app }, { type: "userMessage", text: "hello" });
      expect(await app.composer.text()).toBe("");
      expect(harness.screenRows().at(-1)).toContain("effort: Medium");
      expect(transcriptLines(harness).filter((line) => line.includes("hello"))).toHaveLength(1);
      expect(harness.exited()).toBe(false);
      expect(harness.screenRows().some((line) => line.includes("Goodbye."))).toBe(false);
    });
  });

  test("submit_hello_clears_composer_and_shows_working", async () => {
    await withFixture(40, 12, async ({ app, harness }) => {
      for (const key of "hello") harness.pressKey(key);
      harness.pressKey("Enter");
      const action = await harness.nextAction();
      await app.handleAction({ type: "submit", text: action?.payload ?? "" });
      expect(await app.composer.text()).toBe("");
      expect(harness.screenRows().some((line) => line.includes("Working"))).toBe(true);
      expect(harness.exited()).toBe(false);
    });
  });

  test("submit_hello_keeps_process_alive_when_agent_run_throws", async () => {
    await withFixture(40, 12, async ({ app, harness }) => {
      (app.agent as unknown as { run: () => Promise<void> }).run = () => {
        throw new Error("provider failed");
      };
      for (const key of "hello") harness.pressKey(key);
      harness.pressKey("Enter");
      const action = await harness.nextAction();
      await expect(app.handleAction({ type: "submit", text: action?.payload ?? "" })).resolves.toBeUndefined();
      expect(harness.exited()).toBe(false);
      expect(app.state.info.status).toBe("provider failed");
      expect(harness.screenRows().some((line) => line.includes("Goodbye."))).toBe(false);
    });
  });

  test("idle_ctrl_c_presents_goodbye_before_terminal_restore", async () => {
    await withFixture(40, 12, async ({ app, harness }) => {
      const originalExit = harness.exit.bind(harness);
      (harness as unknown as { exit: () => Promise<void> }).exit = async () => {
        expect(harness.exited()).toBe(false);
        expect(harness.screenRows().some((line) => line.includes("Goodbye."))).toBe(true);
        await originalExit();
      };
      harness.pressKey("c", ["control"]);
      const action = await harness.nextAction();
      expect(action).toEqual({ actionId: "ctrlC" });
      await app.handleAction({ type: "ctrlC" });
      expect(harness.exited()).toBe(true);
    });
  });

  test("idle_ctrl_c_leaves_goodbye_in_native_history", async () => {
    await withFixture(40, 12, async ({ app, harness }) => {
      harness.pressKey("c", ["control"]);
      await harness.nextAction();
      await app.handleAction({ type: "ctrlC" });
      expect(harness.nativeHistoryRows().some((line) => line.includes("Goodbye."))).toBe(true);
    });
  });

  test("ctrl_c_with_composer_text_clears_and_does_not_exit", async () => {
    await withFixture(40, 12, async ({ app, harness }) => {
      for (const key of "hello") harness.pressKey(key);
      harness.pressKey("\u0003");
      const action = await harness.nextAction();
      expect(action).toEqual({ actionId: "ctrlC" });
      await app.handleAction({ type: "ctrlC" });
      expect(await app.composer.text()).toBe("");
      expect(harness.exited()).toBe(false);
      expect(harness.screenRows().some((line) => line.includes("Goodbye."))).toBe(false);
    });
  });

  test("ctrl_c_during_active_work_cancels_and_does_not_exit", async () => {
    await withFixture(40, 12, async ({ app, harness }) => {
      await send({ app, harness }, { type: "turnStarted" });
      harness.pressKey("c", ["control"]);
      const action = await harness.nextAction();
      expect(action).toEqual({ actionId: "ctrlC" });
      await app.handleAction({ type: "ctrlC" });
      expect(app.state.activeTurn).toBe(false);
      expect(app.state.working).toBe(false);
      expect(harness.exited()).toBe(false);
      expect(harness.screenRows().some((line) => line.includes("Goodbye."))).toBe(false);
    });
  });

  test("ctrl_c_etx_and_control_c_both_route_to_ctrlC", async () => {
    for (const key of ["\u0003", "c"] as const) {
      await withFixture(40, 12, async ({ app, harness }) => {
        harness.pressKey(key, key === "c" ? ["control"] : undefined);
        await expect(harness.nextAction()).resolves.toEqual({ actionId: "ctrlC" });
        await app.stop();
        await harness.close();
      });
    }
  });

  test("escape_keeps_composer_visible", async () => {
    await withFixture(40, 12, async ({ app, harness }) => {
      expect(harness.screenRows().filter((line) => line.includes("─")).length).toBeGreaterThanOrEqual(2);
      expect(harness.screenRows().at(-1)).toContain("effort");
      harness.pressKey("Escape");
      const action = await harness.nextAction();
      expect(action).toEqual({ actionId: "escape" });
      expect(harness.screenRows().filter((line) => line.includes("─")).length).toBeGreaterThanOrEqual(2);
      expect(harness.screenRows().at(-1)).toContain("effort");
      expect(harness.exited()).toBe(false);
      await app.handleAction({ type: "escape" });
      expect(harness.screenRows().filter((line) => line.includes("─")).length).toBeGreaterThanOrEqual(2);
      expect(harness.screenRows().at(-1)).toContain("effort");
      expect(harness.exited()).toBe(false);
    });
  });

  test("escape_during_work_cancels_without_hiding_composer", async () => {
    await withFixture(40, 12, async ({ app, harness }) => {
      await send({ app, harness }, { type: "turnStarted" });
      harness.pressKey("Escape");
      expect(await harness.nextAction()).toEqual({ actionId: "escape" });
      expect(harness.screenRows().filter((line) => line.includes("─")).length).toBeGreaterThanOrEqual(2);
      expect(harness.screenRows().at(-1)).toContain("effort");
      await app.handleAction({ type: "escape" });
      expect(harness.screenRows().filter((line) => line.includes("─")).length).toBeGreaterThanOrEqual(2);
      expect(harness.exited()).toBe(false);
      expect(app.state.working).toBe(false);
      expect(harness.screenRows().some((line) => line.includes("Goodbye."))).toBe(false);
    });
  });

  test("escape_during_running_tool_freezes_cancelled_red", async () => {
    await withFixture(80, 20, async (fixture) => {
      await sendAll(fixture, [
        { type: "turnStarted" },
        { type: "toolCallStarted", toolCallId: "abort-call", toolName: "bash", arguments: { command: "sleep 5" } },
      ]);
      await fixture.app.handleAction({ type: "escape" });
      const lines = transcriptLines(fixture.harness);
      expect(lines.some((line) => line.includes("$ sleep 5 — cancelled"))).toBe(true);
      expect(lines.some((line) => line.includes("— running"))).toBe(false);
      const row = fixture.harness.screenRows().findIndex((line) => line.includes("$ sleep 5"));
      const column = fixture.harness.cellXOfText(row, "●");
      expect(fixture.harness.styleAt(row, column ?? 0).foreground).toBe("Red");
      expect(fixture.app.state.working).toBe(false);
    });
  });

  test("ctrl_c_during_running_tool_freezes_cancelled_without_exiting", async () => {
    await withFixture(80, 20, async (fixture) => {
      await sendAll(fixture, [
        { type: "turnStarted" },
        { type: "toolCallStarted", toolCallId: "ctrlc-abort", toolName: "bash", arguments: { command: "sleep 5" } },
      ]);
      await fixture.app.composer.setText("queued steer");
      await fixture.app.handleAction({ type: "ctrlC" });
      const lines = transcriptLines(fixture.harness);
      expect(lines.some((line) => line.includes("$ sleep 5 — cancelled"))).toBe(true);
      expect(fixture.app.state.working).toBe(false);
      expect(fixture.harness.exited()).toBe(false);
      expect(fixture.harness.screenRows().some((line) => line.includes("Goodbye."))).toBe(false);
      await fixture.app.handleAction({ type: "ctrlC" });
      expect(fixture.harness.exited()).toBe(false);
    });
  });

  test("run_loop_null_next_action_still_runs_goodbye_shutdown", async () => {
    await withFixture(40, 12, async ({ app, harness }) => {
      const tui = new Proxy(harness, {
        get(target, property, receiver) {
          if (property === "nextEvent") return async () => ({ type: "terminate", reason: "closed" });
          const value = Reflect.get(target, property, receiver);
          return typeof value === "function" ? value.bind(target) : value;
        },
      }) as unknown as TuiRuntime;
      (app as unknown as { tui: TuiRuntime }).tui = tui;
      await app.run();
      expect(app.state.goodbye).toBe(true);
      expect(harness.nativeHistoryRows().some((line) => line.includes("Goodbye."))).toBe(true);
    });
  });

  test("flushes pending assistant smoothing before a tool card", async () => {
    await withFixture(60, 20, async (fixture) => {
      await sendAll(fixture, [
        { type: "turnStarted" },
        { type: "assistantDelta", text: "assistant tail" },
        { type: "toolCallStarted", toolCallId: "boundary-tool", toolName: "bash", arguments: { command: "true" } },
      ]);
      advance(fixture, 16, 8);
      const lines = transcriptLines(fixture.harness);
      expect(position(lines, "assistant tail")).toBeLessThan(position(lines, "$ true"));
    });
  });

  test("preserves a partial assistant stream across cancellation", async () => {
    await withFixture(40, 12, async (fixture) => {
      await sendAll(fixture, [{ type: "turnStarted" }, { type: "assistantDelta", text: "cancelled assistant tail" }, { type: "turnCancelled" }]);
      advance(fixture, 16, 8);
      expect(position(transcriptLines(fixture.harness), "cancelled assistant tail")).toBeGreaterThanOrEqual(0);
      expect(fixture.app.state.working).toBe(false);
    });
  });

  test("keeps the composer below completed tool history", async () => {
    await withFixture(60, 12, async (fixture) => {
      const key = draft(1, 0);
      await sendAll(fixture, [
        { type: "turnStarted" },
        { type: "toolCallPreparing", key, toolName: "bash" },
        { type: "toolCallPrepared", key, toolCallId: "completed-tool", toolName: "bash", arguments: { command: "printf output" } },
        { type: "toolCallStarted", toolCallId: "completed-tool", toolName: "bash", arguments: { command: "printf output" } },
        { type: "toolResult", toolCallId: "completed-tool", toolName: "bash", text: "final output", details: {}, isError: false },
      ]);
      const rows = fixture.harness.screenRows();
      expect(position(rows, "final output")).toBeLessThan(rows.length - 1);
      expect(rows.some((line) => line.includes("effort"))).toBe(true);
    });
  });

  test("shows a streamed draft before execution and reuses one card", async () => {
    await withFixture(80, 20, async (fixture) => {
      const key = draft(7, 0);
      await send(fixture, { type: "turnStarted" });
      await send(fixture, { type: "toolCallPreparing", key, toolName: "bash" });
      expect(toolStatusCount(transcriptLines(fixture.harness), "bash", "preparing")).toBe(1);
      await send(fixture, { type: "toolCallPrepared", key, toolCallId: "call-b", toolName: "bash", arguments: { command: "echo b" } });
      expect(toolStatusCount(transcriptLines(fixture.harness), "echo b", "ready")).toBe(1);
      await send(fixture, { type: "toolCallStarted", toolCallId: "call-b", toolName: "bash", arguments: { command: "echo b" } });
      expect(toolStatusCount(transcriptLines(fixture.harness), "echo b", "running")).toBe(1);
      expect(transcriptLines(fixture.harness).filter((line) => line.includes("echo b") && line.includes("—")).length).toBe(1);
    });
  });

  test("keeps prepared tool order while only the started tool runs", async () => {
    await withFixture(80, 20, async (fixture) => {
      const bash = draft(7, 0);
      const read = draft(7, 1);
      await sendAll(fixture, [
        { type: "turnStarted" },
        { type: "toolCallPreparing", key: bash, toolName: "bash" },
        { type: "toolCallPrepared", key: bash, toolCallId: "call-b", toolName: "bash", arguments: { command: "echo b" } },
        { type: "toolCallPreparing", key: read, toolName: "read" },
        { type: "toolCallPrepared", key: read, toolCallId: "call-r", toolName: "read", arguments: { path: "a.txt" } },
        { type: "toolCallStarted", toolCallId: "call-r", toolName: "read", arguments: { path: "a.txt" } },
      ]);
      const lines = transcriptLines(fixture.harness);
      expect(position(lines, "read a.txt — running")).toBeGreaterThan(position(lines, "echo b — ready"));
      expect(toolStatusCount(lines, "echo b", "running")).toBe(0);
    });
  });

  test("updates a prepared approval card in place", async () => {
    await withFixture(80, 20, async (fixture) => {
      const key = draft(7, 0);
      await sendAll(fixture, [
        { type: "turnStarted" },
        { type: "toolCallPreparing", key, toolName: "read" },
        { type: "toolCallPrepared", key, toolCallId: "approval-call", toolName: "read", arguments: { path: "secrets.txt" } },
        { type: "toolApprovalRequested", approvalId: 42, toolCallId: "approval-call", toolName: "read", arguments: { path: "secrets.txt" } },
      ]);
      const lines = transcriptLines(fixture.harness);
      expect(toolStatusCount(lines, "read", "waiting for approval")).toBe(1);
      expect(lines.filter((line) => line.includes("read") && line.includes(" — ")).length).toBe(1);
    });
  });

  test("freezes a preparing tool as cancelled", async () => {
    await withFixture(80, 20, async (fixture) => {
      const key = draft(7, 0);
      await sendAll(fixture, [{ type: "turnStarted" }, { type: "toolCallPreparing", key, toolName: "read" }, { type: "turnCancelled" }]);
      const lines = transcriptLines(fixture.harness);
      expect(toolStatusCount(lines, "read", "cancelled")).toBe(1);
      expect(toolStatusCount(lines, "read", "finished")).toBe(0);
    });
  });

  test("does not mark a cancelled running tool finished", async () => {
    await withFixture(80, 20, async (fixture) => {
      const key = draft(7, 0);
      await sendAll(fixture, [
        { type: "turnStarted" },
        { type: "toolCallPreparing", key, toolName: "read" },
        { type: "toolCallPrepared", key, toolCallId: "running-call", toolName: "read", arguments: { path: "a.txt" } },
        { type: "toolCallStarted", toolCallId: "running-call", toolName: "read", arguments: { path: "a.txt" } },
        { type: "turnCancelled" },
      ]);
      const lines = transcriptLines(fixture.harness);
      expect(toolStatusCount(lines, "read", "cancelled")).toBe(1);
      expect(toolStatusCount(lines, "read", "finished")).toBe(0);
    });
  });

  test("renders an error result in the prepared card with its call line", async () => {
    await withFixture(80, 20, async (fixture) => {
      const key = draft(7, 0);
      await sendAll(fixture, [
        { type: "turnStarted" },
        { type: "toolCallPreparing", key, toolName: "read" },
        { type: "toolCallPrepared", key, toolCallId: "error-call", toolName: "read", arguments: { path: "missing.txt" } },
        { type: "toolResult", toolCallId: "error-call", toolName: "read", text: "missing", details: {}, isError: true },
      ]);
      const lines = transcriptLines(fixture.harness);
      expect(lines.filter((line) => line.includes("read failed")).length).toBe(1);
      expect(lines.filter((line) => line.includes("read") && line.includes(" — ")).length).toBe(1);
    });
  });

  test("forces a missing tool result final at turn end", async () => {
    await withFixture(80, 20, async (fixture) => {
      const key = draft(8, 0);
      await sendAll(fixture, [
        { type: "turnStarted" },
        { type: "toolCallPreparing", key, toolName: "read" },
        { type: "toolCallPrepared", key, toolCallId: "missing-result", toolName: "read", arguments: { path: "a.txt" } },
        { type: "toolCallStarted", toolCallId: "missing-result", toolName: "read", arguments: { path: "a.txt" } },
        { type: "turnFinished" },
      ]);
      expect(toolStatusCount(transcriptLines(fixture.harness), "read", "failed")).toBe(1);
      expect(fixture.app.state.liveTools.get("8:0")?.frozen).toBe(true);
    });
  });

  test("keeps long Markdown code from pinning the composer", async () => {
    await withFixture(40, 20, async (fixture) => {
      await send(fixture, { type: "assistantDelta", text: "```rust\nthis_is_a_ridiculously_long_function_call();\n```\n" });
      advance(fixture, 16, 80);
      const rows = fixture.harness.screenRows();
      expect(rows.some((line) => line.includes("this_is_a_ridiculously_long_function"))).toBe(true);
      expect(rows.some((line) => line.includes("effort"))).toBe(true);
    });
  });

  test("flushes buffered assistant text before Goodbye", async () => {
    await withFixture(40, 12, async (fixture) => {
      await send(fixture, { type: "assistantDelta", text: "buffered assistant" });
      await fixture.app.handleAction({ type: "requestExit" });
      const rows = fixture.harness.screenRows();
      expect(position(rows, "buffered assistant")).toBeLessThan(position(rows, "Goodbye."));
      expect(fixture.harness.exited()).toBe(true);
    });
  });

  test("keeps an approval prompt beside a user batch delivered after a tool", async () => {
    await withFixture(60, 20, async (fixture) => {
      const key = draft(9, 0);
      await sendAll(fixture, [
        { type: "turnStarted" },
        { type: "toolCallPreparing", key, toolName: "read" },
        { type: "toolCallPrepared", key, toolCallId: "approval-tail", toolName: "read", arguments: { path: "a.txt" } },
        { type: "toolApprovalRequested", approvalId: 9, toolCallId: "approval-tail", toolName: "read", arguments: { path: "a.txt" } },
        { type: "userMessage", text: "last user bubble" },
      ]);
      const lines = transcriptLines(fixture.harness);
      expect(position(lines, "last user bubble")).toBeGreaterThanOrEqual(0);
      expect(position(lines, "Approve read?")).toBeGreaterThanOrEqual(0);
    });
  });

  test("keeps a steered user message before the assistant stream tail", async () => {
    await withFixture(60, 20, async (fixture) => {
      await sendAll(fixture, [
        { type: "turnStarted" },
        { type: "userMessage", text: "initial user" },
        { type: "toolCallStarted", toolCallId: "steer-tool", toolName: "bash", arguments: { command: "true" } },
        { type: "toolResult", toolCallId: "steer-tool", toolName: "bash", text: "tool output", details: {}, isError: false },
        { type: "userMessage", text: "steered user" },
        { type: "assistantDelta", text: "assistant after steering" },
      ]);
      advance(fixture, 16, 120);
      const lines = transcriptLines(fixture.harness);
      expect(position(lines, "steered user")).toBeLessThan(position(lines, "assistant after steering"));
    });
  });

  test("steered_submit_does_not_enter_history_until_delivery", async () => {
    await withFixture(60, 20, async (fixture) => {
      await fixture.app.handleAction({ type: "submit", text: "hello" });
      await fixture.app.handleAction({ type: "submit", text: "jf" });
      const lines = transcriptLines(fixture.harness);
      expect(lines.some((line) => line.includes("Queue: jf"))).toBe(true);
      expect(lines.filter((line) => line.trim() === "hello")).toHaveLength(1);
      expect(lines.some((line) => line.trim() === "jf")).toBe(false);
    }, true);
  });

  test("delivered_steer_lands_at_history_tail_before_next_assistant", async () => {
    await withFixture(60, 20, async (fixture) => {
      await fixture.app.handleAction({ type: "submit", text: "hello" });
      await fixture.app.handleAction({ type: "submit", text: "jf" });
      await send(fixture, { type: "userMessage", text: "jf", queueId: 2 });
      await sendAll(fixture, [
        { type: "thinkingDelta", text: "thinking after steer" },
        { type: "assistantDelta", text: "answer after steer" },
      ]);
      advance(fixture, 16, 40);
      const lines = transcriptLines(fixture.harness);
      expect(lines.filter((line) => line.trim() === "jf")).toHaveLength(1);
      expect(position(lines, "jf")).toBeLessThan(position(lines, "thinking after steer"));
      expect(lines.some((line) => line.includes("Queue: jf"))).toBe(false);
    }, true);
  });

  test("steered_submit_during_thinking_lands_at_tail_on_delivery", async () => {
    await withFixture(60, 24, async (fixture) => {
      await fixture.app.handleAction({ type: "submit", text: "hello" });
      await send(fixture, { type: "thinkingDelta", text: "thinking about hello" });
      advance(fixture, 16, 120);
      await fixture.app.handleAction({ type: "submit", text: "jf" });
      const queued = transcriptLines(fixture.harness);
      expect(queued.some((line) => line.includes("Queue: jf"))).toBe(true);
      expect(queued.filter((line) => line.trim() === "jf")).toHaveLength(0);
      expect(position(queued, "hello")).toBeLessThan(position(queued, "thinking about hello"));

      await send(fixture, { type: "userMessage", text: "jf" });
      await sendAll(fixture, [
        { type: "thinkingDelta", text: "thinking after steer" },
        { type: "assistantDelta", text: "answer after steer" },
      ]);
      advance(fixture, 16, 120);
      const lines = transcriptLines(fixture.harness);
      expect(lines.filter((line) => line.trim() === "jf")).toHaveLength(1);
      expect(position(lines, "thinking about hello")).toBeLessThan(position(lines, "jf"));
      expect(position(lines, "jf")).toBeLessThan(position(lines, "thinking after steer"));
      expect(lines.some((line) => line.includes("Queue: jf"))).toBe(false);
    });
  });

  test("composer_paste_inserts_forwarded_text_without_reintercepting", async () => {
    await withFixture(60, 20, async (fixture) => {
      fixture.harness.paste("hello paste");
      const action = await fixture.harness.nextAction();
      expect(action?.actionId).toBe("composerPaste");
      expect(action?.payload).toBe("hello paste");
      await fixture.app.handleAction({ type: "composerPaste", text: action?.payload ?? "" });
      expect(await fixture.app.composer.text()).toBe("hello paste");
      expect(fixture.harness.screenRows().some((line) => line.includes("hello paste"))).toBe(true);
    });
  });

  test("keeps streaming assistant content contiguous while the composer collapses", async () => {
    await withFixture(40, 20, async (fixture) => {
      await send(fixture, { type: "assistantDelta", text: "assistant before composer" });
      fixture.harness.paste(Array.from({ length: 12 }, (_, index) => `line ${index}`).join("\n"));
      const action = await fixture.harness.nextAction();
      await fixture.app.handleAction({ type: "composerPaste", text: action?.payload ?? "" });
      advance(fixture, 16, 40);
      const rows = fixture.harness.screenRows();
      expect(rows.some((line) => line.includes("assistant before composer"))).toBe(true);
      expect(rows.length).toBe(20);
    });
  });

  test("consumes history slack before transferring a shrinking composer", async () => {
    await withFixture(40, 20, async (fixture) => {
      await send(fixture, { type: "userMessage", text: "history slack" });
      await send(fixture, { type: "assistantDelta", text: "history assistant" });
      fixture.harness.paste("one\ntwo\nthree\nfour\nfive");
      const action = await fixture.harness.nextAction();
      await fixture.app.handleAction({ type: "composerPaste", text: action?.payload ?? "" });
      const rows = fixture.harness.screenRows();
      expect(rows.some((line) => line.includes("history slack"))).toBe(true);
      expect(rows.length).toBe(20);
    });
  });

  test("shows multiline tool updates through the same card", async () => {
    await withFixture(60, 20, async (fixture) => {
      await sendAll(fixture, [
        { type: "turnStarted" },
        { type: "toolCallStarted", toolCallId: "update-tool", toolName: "bash", arguments: { command: "true" } },
        { type: "toolCallUpdated", toolCallId: "update-tool", update: { type: "text", text: "running\nsecond" } },
      ]);
      const lines = transcriptLines(fixture.harness);
      expect(position(lines, "running")).toBeGreaterThanOrEqual(0);
      expect(position(lines, "second")).toBeGreaterThanOrEqual(0);
      expect(lines.filter((line) => line.includes("$ true") && line.includes(" — ")).length).toBe(1);
    });
  });

  test("renders short Markdown paragraphs, lists, and tables without replacing native history", async () => {
    await withFixture(40, 20, async (fixture) => {
      await sendAll(fixture, [
        { type: "assistantDelta", text: "intro paragraph\n\n- one\n- two\n\n| A | B |\n| --- | --- |\n| 1 | 2 |" },
      ]);
      advance(fixture, 16, 100);
      const rows = fixture.harness.screenRows();
      expect(rows.some((line) => line.includes("intro paragraph"))).toBe(true);
      expect(rows.some((line) => line.includes("A"))).toBe(true);
      expect(rows.some((line) => line.includes("effort"))).toBe(true);
    });
  });

  test("shows pending steering beside the native working activity", async () => {
    await withFixture(60, 20, async (fixture) => {
      await fixture.app.handleAction({ type: "submit", text: "initial" });
      await fixture.app.handleAction({ type: "submit", text: "steer" });
      const rows = fixture.harness.screenRows();
      expect(rows.some((line) => line.includes("Queue: steer"))).toBe(true);
      expect(rows.some((line) => line.includes("Working"))).toBe(true);
      advance(fixture, 80, 5);
      expect(fixture.harness.screenRows().some((line) => line.includes("waiting"))).toBe(true);
      expect(transcriptLines(fixture.harness).some((line) => line.trim() === "steer")).toBe(false);
    });
  });

  test("thinking_deltas_do_not_rebuild_composer_chrome", async () => {
    await withFixture(40, 12, async (fixture) => {
      await fixture.app.handleAction({ type: "submit", text: "hello" });
      const before = fixture.harness.screenRows();
      const composerRows = before.filter((line) => line.includes("─")).length;
      expect(composerRows).toBeGreaterThanOrEqual(2);
      expect(before.at(-1)).toContain("effort: Medium");
      for (const chunk of ["alpha ", "beta ", "gamma ", "delta ", "epsilon"]) {
        await send(fixture, { type: "thinkingDelta", text: chunk });
      }
      const after = fixture.harness.screenRows();
      expect(after.filter((line) => line.includes("─")).length).toBe(composerRows);
      expect(after.at(-1)).toContain("effort: Medium");
      expect(fixture.harness.exited()).toBe(false);
    });
  });

  test("reasoning_effort_changes_focused_composer_border_color", async () => {
    await withFixture(40, 12, async (fixture) => {
      const medium = fixture.harness.styleAt(8, 0);
      await fixture.app.handleAction({ type: "cycleReasoningEffort" });
      const high = fixture.harness.styleAt(8, 0);
      expect(medium.foreground).toBe("Yellow");
      expect(high.foreground).toBe("LightMagenta");
    });
  });

  test("normal_working_spinner_runs_reverse", async () => {
    await withFixture(60, 20, async (fixture) => {
      await sendAll(fixture, [{ type: "turnStarted" }, { type: "userMessage", text: "prompt" }]);
      expect(fixture.harness.screenRows().some((line) => line.includes("⠞⢁ Working"))).toBe(true);
    });
  });

  test("turn_started_without_user_batch_does_not_show_working", async () => {
    await withFixture(60, 20, async (fixture) => {
      await send(fixture, { type: "turnStarted" });
      expect(fixture.harness.screenRows().some((line) => line.includes("Working"))).toBe(false);
    });
  });

  test("queued_working_spinner_runs_forward", async () => {
    await withFixture(60, 20, async (fixture) => {
      await sendAll(fixture, [
        { type: "turnStarted" },
        { type: "userMessage", text: "prompt" },
        { type: "steerQueued", text: "next" },
      ]);
      expect(fixture.harness.screenRows().some((line) => line.includes("⠞⢁ Working"))).toBe(true);
      advance(fixture, 80, 5);
      expect(fixture.harness.screenRows().some((line) => line.includes("⠋⣠ waiting"))).toBe(true);
    });
  });

  test("assistant_stream_replaces_unqueued_working_activity", async () => {
    await withFixture(60, 20, async (fixture) => {
      await sendAll(fixture, [{ type: "turnStarted" }, { type: "userMessage", text: "prompt" }]);
      advance(fixture);
      const workingRow = fixture.harness.screenRows().findIndex((line) => line.includes("Working"));
      expect(workingRow).toBeGreaterThanOrEqual(0);
      await send(fixture, { type: "assistantDelta", text: "assistant tail" });
      advance(fixture, 16, 20);
      const lines = transcriptLines(fixture.harness);
      expect(fixture.harness.screenRows().findIndex((line) => line.includes("assista"))).toBe(workingRow);
      expect(lines.some((line) => line.includes("Working"))).toBe(false);
    });
  });

  test("queued_steering_keeps_working_below_assistant_tail", async () => {
    await withFixture(60, 20, async (fixture) => {
      await sendAll(fixture, [
        { type: "turnStarted" },
        { type: "userMessage", text: "prompt" },
        { type: "assistantDelta", text: "assistant tail" },
        { type: "steerQueued", text: "second" },
      ]);
      advance(fixture, 16, 20);
      const lines = transcriptLines(fixture.harness);
      expect(position(lines, "assista")).toBeLessThan(position(lines, "Queue: second"));
      expect(position(lines, "waiting")).toBeGreaterThan(position(lines, "assista"));
    });
  });

  test("working_activity_bridges_tool_execution_and_model_wait", async () => {
    await withFixture(80, 20, async (fixture) => {
      const firstKey = draft(21, 0);
      await sendAll(fixture, [
        { type: "turnStarted" },
        { type: "userMessage", text: "prompt" },
        { type: "assistantDelta", text: "tool request" },
      ]);
      expect(fixture.harness.screenRows().some((line) => line.includes("Working"))).toBe(false);
      await send(fixture, { type: "toolCallPreparing", key: firstKey, toolCallId: "tool-gap-1", toolName: "bash" });
      await send(fixture, { type: "toolCallPrepared", key: firstKey, toolCallId: "tool-gap-1", toolName: "bash", arguments: { command: "true" } });
      expect(fixture.harness.screenRows().some((line) => line.includes("Working"))).toBe(true);
      await send(fixture, { type: "turnFinished" });
      expect(fixture.harness.screenRows().some((line) => line.includes("Working"))).toBe(true);
      await send(fixture, { type: "toolCallStarted", toolCallId: "tool-gap-1", toolName: "bash", arguments: { command: "true" } });
      await send(fixture, { type: "toolResult", toolCallId: "tool-gap-1", toolName: "bash", text: "done", details: {}, isError: false });
      expect(fixture.harness.screenRows().some((line) => line.includes("Working"))).toBe(true);
      await send(fixture, { type: "thinkingDelta", text: "next tool" });
      expect(fixture.harness.screenRows().some((line) => line.includes("Working"))).toBe(false);
      const secondKey = draft(22, 0);
      await send(fixture, { type: "toolCallPreparing", key: secondKey, toolCallId: "tool-gap-2", toolName: "bash" });
      await send(fixture, { type: "toolCallPrepared", key: secondKey, toolCallId: "tool-gap-2", toolName: "bash", arguments: { command: "true" } });
      await send(fixture, { type: "turnFinished" });
      expect(fixture.harness.screenRows().some((line) => line.includes("Working"))).toBe(true);
      await send(fixture, { type: "toolCallStarted", toolCallId: "tool-gap-2", toolName: "bash", arguments: { command: "true" } });
      await send(fixture, { type: "toolResult", toolCallId: "tool-gap-2", toolName: "bash", text: "done", details: {}, isError: false });
      expect(fixture.harness.screenRows().some((line) => line.includes("Working"))).toBe(true);
      await send(fixture, { type: "assistantDelta", text: "final" });
      advance(fixture, 16, 20);
      expect(fixture.harness.screenRows().some((line) => line.includes("Working"))).toBe(false);
    });
  });

  test("queued_working_preview_is_muted_and_italic", async () => {
    await withFixture(60, 20, async (fixture) => {
      await sendAll(fixture, [{ type: "turnStarted" }, { type: "userMessage", text: "prompt" }, { type: "steerQueued", text: "queued" }]);
      const style = styleForText(fixture, "Queue: queued");
      expect(style.foreground).toBe("#718096");
      expect(style.dim).toBe(false);
      expect(style.italic).toBe(true);
    });
  });

  test("thinking_is_muted_and_italic", async () => {
    await withFixture(60, 20, async (fixture) => {
      await sendAll(fixture, [
        { type: "thinkingDelta", text: "considering this" },
        { type: "assistantDelta", text: "Final answer" },
      ]);
      advance(fixture, 16, 100);
      const thinking = styleForText(fixture, "considering this");
      expect(thinking.foreground).toBe("#718096");
      expect(thinking.italic).toBe(true);
    });
  });

  test("thinking_and_text_preserve_markdown_semantics", async () => {
    await withFixture(60, 20, async (fixture) => {
      await send(fixture, { type: "thinkingDelta", text: "considering **this**" });
      await send(fixture, { type: "assistantDelta", text: "Final **answer**" });
      expect(() => advance(fixture, 16, 100)).not.toThrow();
      expect(styleForText(fixture, "this").bold).toBe(true);
      expect(styleForText(fixture, "answer").bold).toBe(true);
    });
  });

  test("markdown_heading_uses_heading_theme", async () => {
    await withFixture(60, 20, async (fixture) => {
      await send(fixture, { type: "assistantDelta", text: "# Heading" });
      advance(fixture, 16, 100);
      expect(styleForText(fixture, "Heading").foreground).toBe("#ffc457");
    });
  });

  test("markdown_inline_code_uses_code_theme", async () => {
    await withFixture(60, 20, async (fixture) => {
      await send(fixture, { type: "assistantDelta", text: "`inline`" });
      advance(fixture, 16, 100);
      expect(styleForText(fixture, "inline").foreground).toBe("#78c8d2");
    });
  });

  test("live_gfm_table_stabilizes", async () => {
    await withFixture(60, 20, async (fixture) => {
      await send(fixture, { type: "assistantDelta", text: "| A | B |\n| --- | --- |\n| 1 | 2 |" });
      await send(fixture, { type: "turnFinished" });
      advance(fixture, 16, 100);
      const lines = transcriptLines(fixture.harness);
      expect(lines.some((line) => line.includes("A B"))).toBe(true);
      expect(lines.some((line) => line.includes("1 2"))).toBe(true);
    });
  });

  test("tool_call_pulses_every_480ms_natively", async () => {
    await withFixture(80, 20, async (fixture) => {
      await send(fixture, { type: "toolCallStarted", toolCallId: "pulse", toolName: "bash", arguments: { command: "true" } });
      const row = position(transcriptLines(fixture.harness), "$ true");
      const column = fixture.harness.cellXOfText(row, "●");
      if (column === null) throw new Error("missing tool bullet");
      const first = fixture.harness.styleAt(row, column);
      advance(fixture, 480);
      const second = fixture.harness.styleAt(row, column);
      advance(fixture, 480);
      const third = fixture.harness.styleAt(row, column);
      expect(second.dim).not.toBe(first.dim);
      expect(third.dim).toBe(first.dim);
    });
  });

  test("tool_pulse_continues_from_preparing_into_running", async () => {
    await withFixture(80, 20, async (fixture) => {
      const key = draft(12, 0);
      await sendAll(fixture, [
        { type: "toolCallPreparing", key, toolName: "bash" },
        { type: "toolCallPrepared", key, toolCallId: "pulse-continuity", toolName: "bash", arguments: { command: "true" } },
      ]);
      advance(fixture, 240);
      await send(fixture, { type: "toolCallStarted", toolCallId: "pulse-continuity", toolName: "bash", arguments: { command: "true" } });
      const row = position(transcriptLines(fixture.harness), "$ true");
      const column = fixture.harness.cellXOfText(row, "●");
      if (column === null) throw new Error("missing tool bullet");
      const before = fixture.harness.styleAt(row, column);
      advance(fixture, 240);
      expect(fixture.harness.styleAt(row, column).dim).not.toBe(before.dim);
    });
  });

  test("tool_terminal_state_stops_pulse", async () => {
    await withFixture(80, 20, async (fixture) => {
      await send(fixture, { type: "toolCallStarted", toolCallId: "terminal-pulse", toolName: "bash", arguments: { command: "true" } });
      await send(fixture, { type: "toolResult", toolCallId: "terminal-pulse", toolName: "bash", text: "done", details: {}, isError: false });
      const row = position(transcriptLines(fixture.harness), "$ true");
      const first = fixture.harness.styleAt(row, fixture.harness.cellXOfText(row, "●") ?? 0);
      expect(first.foreground).toBe("#68d391");
      expect(first.dim).toBe(false);
      advance(fixture, 960);
      expect(fixture.harness.styleAt(row, fixture.harness.cellXOfText(row, "●") ?? 0)).toEqual(first);
    });
  });

  test("tool_result_preserves_call_line", async () => {
    await withFixture(60, 12, async (fixture) => {
      await sendAll(fixture, [
        { type: "toolCallStarted", toolCallId: "result-call", toolName: "bash", arguments: { command: "true" } },
        { type: "toolResult", toolCallId: "result-call", toolName: "bash", text: "result", details: {}, isError: false },
      ]);
      const lines = transcriptLines(fixture.harness);
      expect(position(lines, "$ true")).toBeLessThan(position(lines, "result"));
    });
  });

  test("successful_tool_result_is_muted", async () => {
    await withFixture(60, 12, async (fixture) => {
      await sendAll(fixture, [
        { type: "toolCallStarted", toolCallId: "muted-result", toolName: "bash", arguments: { command: "true" } },
        { type: "toolResult", toolCallId: "muted-result", toolName: "bash", text: "result", details: {}, isError: false },
      ]);
      expect(styleForText(fixture, "result").foreground).toBe("#718096");
    });
  });

  test("large_tool_result_retains_full_payload", async () => {
    await withFixture(80, 40, async (fixture) => {
      const text = Array.from({ length: 30 }, (_, index) => `line-${index}`).join("\n");
      await sendAll(fixture, [
        { type: "toolCallStarted", toolCallId: "large-payload", toolName: "bash", arguments: { command: "true" } },
        { type: "toolResult", toolCallId: "large-payload", toolName: "bash", text, details: {}, isError: false },
      ]);
      expect(fixture.app.state.liveTools.get("large-payload")?.result?.text).toBe(text);
    });
  });

  test("completed_tool_is_frozen_out_of_live_history", async () => {
    await withFixture(40, 8, async (fixture) => {
      const key = draft(13, 0);
      await sendAll(fixture, [
        { type: "toolCallPreparing", key, toolName: "bash" },
        { type: "toolCallPrepared", key, toolCallId: "frozen-tool", toolName: "bash", arguments: { command: "true" } },
        { type: "toolCallStarted", toolCallId: "frozen-tool", toolName: "bash", arguments: { command: "true" } },
        { type: "toolResult", toolCallId: "frozen-tool", toolName: "bash", text: "finished", details: {}, isError: false },
      ]);
      for (let index = 0; index < 30; index += 1) {
        await send(fixture, { type: "userMessage", text: `later-${index}` });
        await send(fixture, { type: "assistantDelta", text: `reply-${index}` });
        await send(fixture, { type: "turnFinished" });
        advance(fixture, 16, 8);
      }
      expect(fixture.harness.nativeHistoryRows().length).toBeGreaterThan(8);
      expect(transcriptLines(fixture.harness).some((line) => line.includes("reply-29"))).toBe(true);
    });
  });

  test("stream_compaction_preserves_root_coordinates", async () => {
    await withFixture(40, 8, async (fixture) => {
      await send(fixture, { type: "assistantDelta", text: `${"prefix ".repeat(40)}\n\n` });
      advance(fixture, 16, 160);
      await send(fixture, { type: "assistantDelta", text: "post-compaction tail" });
      advance(fixture, 16, 160);
      expect(transcriptLines(fixture.harness).some((line) => line.includes("post-compaction tail"))).toBe(true);
    });
  });

  test("stream_continues_after_native_history_promotion", async () => {
    await withFixture(40, 8, async (fixture) => {
      await send(fixture, { type: "assistantDelta", text: `${"history ".repeat(40)}\n\n` });
      advance(fixture, 16, 160);
      await send(fixture, { type: "assistantDelta", text: "native history tail" });
      advance(fixture, 16, 160);
      expect(transcriptLines(fixture.harness).some((line) => line.includes("native history tail"))).toBe(true);
    });
  });

  test("identical_user_messages_are_not_deduplicated_by_text", async () => {
    await withFixture(60, 20, async (fixture) => {
      await fixture.app.handleAction({ type: "submit", text: "same" });
      await send(fixture, { type: "turnFinished" });
      await fixture.app.handleAction({ type: "submit", text: "same" });
      await send(fixture, { type: "userMessage", text: "same" });
      await send(fixture, { type: "userMessage", text: "same" });
      expect(transcriptLines(fixture.harness).filter((line) => line.trim() === "same")).toHaveLength(2);
    });
  });

  test("local_submit_and_canonical_user_event_produce_one_user_row", async () => {
    await withFixture(60, 20, async (fixture) => {
      await fixture.app.handleAction({ type: "submit", text: "canonical" });
      await send(fixture, { type: "userMessage", text: "canonical" });
      expect(transcriptLines(fixture.harness).filter((line) => line.trim() === "canonical")).toHaveLength(1);
    });
  });

  test("identical_steers_have_distinct_queue_identity", async () => {
    await withFixture(60, 20, async (fixture) => {
      await sendAll(fixture, [
        { type: "steerQueued", text: "same steer", queueId: 1 },
        { type: "steerQueued", text: "same steer", queueId: 2 },
        { type: "userMessage", text: "same steer", queueId: 1 },
      ]);
      expect(fixture.app.state.steering).toHaveLength(1);
    });
  });

  test("delivered_steer_removes_exact_queue_item", async () => {
    await withFixture(60, 20, async (fixture) => {
      await sendAll(fixture, [
        { type: "steerQueued", text: "first", queueId: 1 },
        { type: "steerQueued", text: "second", queueId: 2 },
        { type: "userMessage", text: "first", queueId: 1 },
      ]);
      expect(fixture.app.state.steering).toEqual(["second"]);
    });
  });

});
