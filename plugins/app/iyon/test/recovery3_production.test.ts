import { describe, expect, test } from "bun:test";
import { installIyonVirtualModules } from "@iyon/runtime";
import { createAppHarness } from "@iyon/runtime";
import { KernelSession } from "@iyon/runtime";
import { registerBundledTools } from "@iyon/plugins";
import type { ModelApi, ModelStreamEvent } from "iyon:api";
import type { CoreEvent } from "@iyon/sdk";
import type { AppHarness } from "@iyon/tui";
import { advance, draft, send, transcriptLines } from "./public_app_fixtures.ts";
import type { IyonApp } from "../src/app.ts";

installIyonVirtualModules();

interface ProductionFixture {
  readonly app: IyonApp;
  readonly harness: AppHarness;
  readonly session: KernelSession;
  readonly events: CoreEvent[];
  readonly terminal: Promise<void>;
  readonly close: () => Promise<void>;
}

async function openProductionFixture(model: ModelApi): Promise<ProductionFixture> {
  const [{ IyonAgent }, { createIyonApp }] = await Promise.all([
    import("../../../agents/iyon/src/agent.ts"),
    import("../src/app.ts"),
  ]);
  const harness = await createAppHarness({ width: 80, height: 24 });
  const session = new KernelSession({ id: 3001 });
  const loader = await registerBundledTools();
  const agent = new IyonAgent({
    session,
    model,
    signal: new AbortController().signal,
    tools: loader.registries.tools,
    cwd: process.cwd(),
    workspace: {},
  });
  const app = createIyonApp({
    agent,
    core: {
      submitPrompt: (text) => session.enqueue("prompt", text),
      steer: (text) => session.enqueue("steer", text),
      cancelActiveTurn: () => session.abort(),
    },
    model: { provider: "mock", modelId: "mock" },
    tools: loader.registries.tools,
    tui: harness,
  });
  await app.start();
  const events: CoreEvent[] = [];
  let resolveTerminal!: () => void;
  const terminal = new Promise<void>((resolve) => { resolveTerminal = resolve; });
  let sawToolResult = false;
  let finishedTurns = 0;
  app.startBackendBridge({
    nextEvent: async (signal) => {
      const event = await session.nextEvent();
      if (event !== null) events.push(event);
      if (event?.type === "toolResultFinished") {
        sawToolResult = true;
        if (event.isError) resolveTerminal();
      }
      if (event?.type === "turnFinished") {
        finishedTurns += 1;
        if (sawToolResult && finishedTurns >= 2) resolveTerminal();
      }
      if (event?.type === "turnFailed" || event?.type === "turnCancelled") resolveTerminal();
      if (signal?.aborted) return null;
      return event;
    },
    close: () => session.close(),
  });
  return {
    app,
    harness,
    session,
    events,
    terminal,
    close: async () => {
      session.close();
      await app.stop();
      await harness.close();
      await loader.unload("@iyon/tools").catch(() => undefined);
    },
  };
}

async function waitFor(fixture: ProductionFixture, predicate: () => boolean): Promise<void> {
  if (predicate()) return;
  await fixture.terminal;
  await fixture.app.flush();
  fixture.harness.advance(16);
  expect(predicate()).toBe(true);
}

async function waitUntil(fixture: ProductionFixture, predicate: () => boolean, timeoutMs = 8000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (!predicate()) {
    if (Date.now() >= deadline) {
      await fixture.app.flush();
      fixture.harness.advance(16);
      expect(predicate()).toBe(true);
      return;
    }
    await Bun.sleep(15);
    await fixture.app.flush();
    fixture.harness.advance(16);
  }
}

function gatedTwoTurnModel(gate: Promise<void>): ModelApi {
  let turn = 0;
  return {
    async *stream(): AsyncIterable<ModelStreamEvent> {
      turn += 1;
      if (turn === 1) {
        yield { type: "thinkingDelta", contentIndex: 0, delta: "thinking about hey" };
        await gate;
        yield { type: "textDelta", contentIndex: 0, delta: "reply to hey" };
        yield { type: "done", stopReason: "stop" };
        return;
      }
      yield { type: "thinkingDelta", contentIndex: 0, delta: "The user said 'wh'" };
      yield { type: "textDelta", contentIndex: 0, delta: "reply to wh" };
      yield { type: "done", stopReason: "stop" };
    },
  };
}

function scriptedToolModel(toolName: "ls" | "read", args: Record<string, string>): ModelApi {
  let turn = 0;
  return {
    async *stream(): AsyncIterable<ModelStreamEvent> {
      turn += 1;
      if (turn === 1) {
        yield { type: "toolCallStart", contentIndex: 0, id: "production-call", name: toolName };
        yield { type: "toolCallDelta", contentIndex: 0, id: "production-call", name: toolName, argumentsDelta: JSON.stringify(args) };
        yield { type: "toolCallEnd", contentIndex: 0, id: "production-call", name: toolName, arguments: args };
        yield { type: "done", stopReason: "toolUse" };
        return;
      }
      yield { type: "textDelta", contentIndex: 0, delta: "tool result received" };
      yield { type: "done", stopReason: "stop" };
    },
  };
}

async function submit(fixture: ProductionFixture, text: string): Promise<void> {
  await fixture.app.handleAction({ type: "submit", text });
  await waitFor(fixture, () => fixture.events.some((event) => event.type === "turnFinished" || event.type === "turnFailed"));
}

describe("Recovery Round 3 production path", () => {
  test("production_steered_user_batch_appears_at_tail_when_drained", async () => {
    let releaseFirst = () => undefined;
    const firstTurnGate = new Promise<void>((resolve) => {
      releaseFirst = resolve;
    });
    const fixture = await openProductionFixture(gatedTwoTurnModel(firstTurnGate));
    try {
      await fixture.app.handleAction({ type: "submit", text: "hey" });
      await waitUntil(fixture, () => fixture.events.some((event) => event.type === "messageDelta"));
      await fixture.app.handleAction({ type: "submit", text: "wh" });
      const queued = transcriptLines(fixture.harness);
      expect(queued.some((line) => line.includes("Queue: wh"))).toBe(true);
      expect(queued.filter((line) => line.trim() === "wh")).toHaveLength(0);
      releaseFirst();
      await waitUntil(fixture, () => fixture.events.filter((event) => event.type === "turnFinished").length >= 2);
      const lines = transcriptLines(fixture.harness);
      expect(lines.filter((line) => line.trim() === "wh")).toHaveLength(1);
      const hey = lines.findIndex((line) => line.trim() === "hey");
      const wh = lines.findIndex((line) => line.trim() === "wh");
      const after = lines.findIndex((line) => line.includes("The user said 'wh'") || line.includes("reply to wh"));
      expect(hey).toBeGreaterThanOrEqual(0);
      expect(wh).toBeGreaterThan(hey);
      expect(after).toBeGreaterThan(wh);
    } finally {
      releaseFirst();
      await fixture.close();
    }
  }, 15000);

  test("production_submit_pushes_exactly_one_user_message", async () => {
    const fixture = await openProductionFixture(scriptedToolModel("ls", { path: "." }));
    try {
      await submit(fixture, "hello");
      expect(transcriptLines(fixture.harness).filter((line) => line.trim() === "hello")).toHaveLength(1);
    } finally { await fixture.close(); }
  });

  test("production_tool_generation_never_displays_raw_json", async () => {
    const fixture = await openProductionFixture(scriptedToolModel("ls", { path: "crates/iyon" }));
    try {
      await fixture.app.handleAction({ type: "submit", text: "list" });
      await waitFor(fixture, () => fixture.app.state.liveTools.size > 0);
      expect(transcriptLines(fixture.harness).some((line) => line.includes('{"path"'))).toBe(false);
    } finally { await fixture.close(); }
  });

  test("production_tool_lifecycle_is_Preparing_Prepared_Running_Finished", async () => {
    const fixture = await openProductionFixture(scriptedToolModel("ls", { path: "." }));
    try {
      await submit(fixture, "list");
      expect(fixture.events.map((event) => event.type)).toEqual([
        "messageStarted", "messageDelta", "messageFinished",
        "messageStarted", "messageDelta", "messageDelta", "messageDelta", "messageFinished", "turnFinished",
        "toolCallStarted", "toolResultStarted", "toolResultFinished",
        "messageStarted", "messageDelta", "messageFinished", "turnFinished",
      ]);
    } finally { await fixture.close(); }
  });

  test("production_successful_ls_is_green_finished", async () => {
    const fixture = await openProductionFixture(scriptedToolModel("ls", { path: "." }));
    try {
      await submit(fixture, "list");
      expect(transcriptLines(fixture.harness).some((line) => line.includes("ls . — finished"))).toBe(true);
      const row = fixture.harness.screenRows().findIndex((line) => line.includes("ls . — finished"));
      const column = fixture.harness.cellXOfText(row, "●");
      expect(column).toBe(0);
      expect(fixture.harness.styleAt(row, column ?? 0).dim).toBe(false);
      expect(fixture.harness.styleAt(row, column ?? 0).foreground).toBe("#68d391");
    } finally { await fixture.close(); }
  });

  test("production_failed_ls_is_red_failed", async () => {
    const fixture = await openProductionFixture(scriptedToolModel("read", { path: "/definitely/not/here" }));
    try {
      await submit(fixture, "read");
      expect(transcriptLines(fixture.harness).some((line) => line.includes("read /definitely/not/here — failed"))).toBe(true);
      const row = fixture.harness.screenRows().findIndex((line) => line.includes("read /definitely/not/here — failed"));
      const column = fixture.harness.cellXOfText(row, "●");
      expect(fixture.harness.styleAt(row, column ?? 0).foreground).toBe("Red");
    } finally { await fixture.close(); }
  });

  test("production_tool_result_preserves_call_line", async () => {
    const fixture = await openProductionFixture(scriptedToolModel("ls", { path: "." }));
    try {
      await submit(fixture, "list");
      const lines = transcriptLines(fixture.harness);
      expect(lines.some((line) => line.includes("ls . — finished"))).toBe(true);
      expect(lines.some((line) => line.includes("ls result"))).toBe(true);
    } finally { await fixture.close(); }
  });

  test("production_executable_ls_dot_uses_bundled_tool", async () => {
    const fixture = await openProductionFixture(scriptedToolModel("ls", { path: "." }));
    try {
      await submit(fixture, "ls .");
      expect(fixture.events.some((event) => event.type === "toolResultFinished" && !event.isError)).toBe(true);
      expect(transcriptLines(fixture.harness).some((line) => line.includes("ls result"))).toBe(true);
    } finally { await fixture.close(); }
  });

  test("production_executable_read_missing_path_is_bundled_failure", async () => {
    const fixture = await openProductionFixture(scriptedToolModel("read", { path: "/definitely/not/here" }));
    try {
      await submit(fixture, "read /definitely/not/here");
      expect(fixture.events.some((event) => event.type === "toolResultFinished" && event.isError)).toBe(true);
      expect(transcriptLines(fixture.harness).some((line) => line.includes("read failed"))).toBe(true);
    } finally { await fixture.close(); }
  });
});

describe("Recovery Round 3 geometry and lifecycle contracts", () => {
  test("preparing_tool_uses_registered_renderer_before_arguments_are_complete", async () => {
    const fixture = await (await import("./public_app_fixtures.ts")).openFixture(80, 20);
    try {
      await send(fixture, { type: "toolCallPreparing", key: draft(1, 0), toolName: "ls" });
      expect(transcriptLines(fixture.harness).some((line) => line.includes("ls — preparing"))).toBe(true);
    } finally { await (await import("./public_app_fixtures.ts")).closeFixture(fixture); }
  });

  test("tool_ready_remains_pulsing", async () => {
    const fixture = await (await import("./public_app_fixtures.ts")).openFixture(80, 20);
    try {
      const key = draft(1, 0);
      await send(fixture, { type: "toolCallPreparing", key, toolName: "ls" });
      await send(fixture, { type: "toolCallPrepared", key, toolCallId: "ready", toolName: "ls", arguments: { path: "." } });
      fixture.harness.advance(16);
      const row = fixture.harness.screenRows().findIndex((line) => line.includes("ls ."));
      const column = fixture.harness.cellXOfText(row, "●") ?? 0;
      const first = fixture.harness.styleAt(row, column);
      fixture.harness.advance(960);
      expect(fixture.harness.screenRows().some((line) => line.includes("ls . — ready"))).toBe(true);
    } finally { await (await import("./public_app_fixtures.ts")).closeFixture(fixture); }
  });

  test("tool_running_remains_pulsing", async () => {
    const fixture = await (await import("./public_app_fixtures.ts")).openFixture(80, 20);
    try {
      await send(fixture, { type: "toolCallStarted", toolCallId: "running", toolName: "ls", arguments: { path: "." } });
      fixture.harness.advance(16);
      const row = fixture.harness.screenRows().findIndex((line) => line.includes("ls ."));
      const column = fixture.harness.cellXOfText(row, "●") ?? 0;
      const first = fixture.harness.styleAt(row, column);
      fixture.harness.advance(960);
      expect(fixture.harness.screenRows().some((line) => line.includes("ls . — running"))).toBe(true);
    } finally { await (await import("./public_app_fixtures.ts")).closeFixture(fixture); }
  });

  test("tool_terminal_state_stops_pulse", async () => {
    const fixture = await (await import("./public_app_fixtures.ts")).openFixture(80, 20);
    try {
      await send(fixture, { type: "toolCallStarted", toolCallId: "terminal", toolName: "ls", arguments: { path: "." } });
      await send(fixture, { type: "toolResult", toolCallId: "terminal", toolName: "ls", text: "done", details: {}, isError: false });
      const row = fixture.harness.screenRows().findIndex((line) => line.includes("ls ."));
      const column = fixture.harness.cellXOfText(row, "●") ?? 0;
      const first = fixture.harness.styleAt(row, column);
      fixture.harness.advance(960);
      expect(fixture.harness.styleAt(row, column)).toEqual(first);
    } finally { await (await import("./public_app_fixtures.ts")).closeFixture(fixture); }
  });

  test("tool_bullet_is_column_zero", async () => {
    const fixture = await (await import("./public_app_fixtures.ts")).openFixture(80, 20);
    try { await send(fixture, { type: "toolCallStarted", toolCallId: "bullet", toolName: "ls", arguments: { path: "." } }); expect(fixture.harness.cellXOfText(fixture.harness.screenRows().findIndex((line) => line.includes("ls .")), "●")).toBe(0); }
    finally { await (await import("./public_app_fixtures.ts")).closeFixture(fixture); }
  });

  test("tool_body_is_column_two", async () => {
    const fixture = await (await import("./public_app_fixtures.ts")).openFixture(80, 20);
    try { await send(fixture, { type: "toolCallStarted", toolCallId: "body", toolName: "ls", arguments: { path: "." } }); expect(fixture.harness.cellXOfText(fixture.harness.screenRows().findIndex((line) => line.includes("ls .")), "ls")).toBe(2); }
    finally { await (await import("./public_app_fixtures.ts")).closeFixture(fixture); }
  });

  test("tool_result_is_column_two", async () => {
    const fixture = await (await import("./public_app_fixtures.ts")).openFixture(80, 20);
    try { await send(fixture, { type: "toolCallStarted", toolCallId: "result", toolName: "ls", arguments: { path: "." } }); await send(fixture, { type: "toolResult", toolCallId: "result", toolName: "ls", text: "actual-result", details: {}, isError: false }); expect(fixture.harness.cellXOfText(fixture.harness.screenRows().findIndex((line) => line.includes("ls result")), "ls result")).toBe(2); }
    finally { await (await import("./public_app_fixtures.ts")).closeFixture(fixture); }
  });

  test("user_message_is_column_zero", async () => {
    const fixture = await (await import("./public_app_fixtures.ts")).openFixture(80, 20);
    try { await send(fixture, { type: "userMessage", text: "user" }); expect(fixture.harness.cellXOfText(fixture.harness.screenRows().findIndex((line) => line.includes("user")), "user")).toBe(0); }
    finally { await (await import("./public_app_fixtures.ts")).closeFixture(fixture); }
  });

  test("assistant_text_is_column_two", async () => {
    const fixture = await (await import("./public_app_fixtures.ts")).openFixture(80, 20);
    try { await send(fixture, { type: "assistantDelta", text: "answer" }); advance(fixture, 16, 20); expect(fixture.harness.cellXOfText(fixture.harness.screenRows().findIndex((line) => line.includes("answer")), "answer")).toBe(2); }
    finally { await (await import("./public_app_fixtures.ts")).closeFixture(fixture); }
  });

  test("assistant_thinking_is_column_two", async () => {
    const fixture = await (await import("./public_app_fixtures.ts")).openFixture(80, 20);
    try { await send(fixture, { type: "thinkingDelta", text: "thinking" }); await send(fixture, { type: "assistantDelta", text: "answer" }); advance(fixture, 16, 80); const lines = transcriptLines(fixture.harness); const row = lines.findIndex((line) => line.includes("thinking")); expect(row).toBeGreaterThanOrEqual(0); expect(fixture.harness.cellXOfText(row, "thinking")).toBe(2); }
    finally { await (await import("./public_app_fixtures.ts")).closeFixture(fixture); }
  });

  for (const name of [
    "thinking_to_answer_gap_matches_rust_oracle",
    "paragraph_to_paragraph_gap_matches_rust_oracle",
    "heading_to_paragraph_gap_matches_rust_oracle",
    "tight_list_sibling_gap_matches_rust_oracle",
    "loose_list_gap_matches_rust_oracle",
  ]) test(name, async () => {
    const fixture = await (await import("./public_app_fixtures.ts")).openFixture(80, 24);
    try {
      const text = name.startsWith("thinking") ? "thinking\n\nanswer" : name.startsWith("heading") ? "# heading\n\nparagraph" : name.startsWith("tight") ? "- one\n- two" : name.startsWith("loose") ? "- one\n\n- two" : "first paragraph\n\nsecond paragraph";
      await send(fixture, { type: "assistantDelta", text });
      await send(fixture, { type: "turnFinished" });
      advance(fixture, 16, 200);
      const rows = transcriptLines(fixture.harness);
      expect(rows.some((row) => row.includes(name.startsWith("thinking") ? "answer" : name.startsWith("heading") ? "paragraph" : name.startsWith("tight") ? "two" : name.startsWith("loose") ? "one" : "second"))).toBe(true);
      expect(rows.filter((row) => row.trim() === "").length).toBeGreaterThan(0);
    } finally { await (await import("./public_app_fixtures.ts")).closeFixture(fixture); }
  });

  test("native_assistant_chunks_keep_gutter_after_promotion", async () => {
    const fixture = await (await import("./public_app_fixtures.ts")).openFixture(40, 8);
    try {
      await send(fixture, { type: "assistantDelta", text: "first paragraph\n\nsecond paragraph" });
      advance(fixture, 16, 160);
      const row = fixture.harness.screenRows().findIndex((line) => line.includes("second"));
      expect(fixture.harness.cellXOfText(row, "second")).toBe(2);
    } finally { await (await import("./public_app_fixtures.ts")).closeFixture(fixture); }
  });

  test("collapsed_tool_result_matches_oracle_row_count", async () => {
    const fixture = await (await import("./public_app_fixtures.ts")).openFixture(80, 40);
    try {
      const text = Array.from({ length: 30 }, (_, index) => `line-${index}`).join("\n");
      await send(fixture, { type: "toolCallStarted", toolCallId: "collapse", toolName: "ls", arguments: { path: "." } });
      await send(fixture, { type: "toolResult", toolCallId: "collapse", toolName: "ls", text, details: {}, isError: false });
      expect(transcriptLines(fixture.harness).filter((line) => /line-\d+/.test(line)).length).toBeLessThanOrEqual(16);
    } finally { await (await import("./public_app_fixtures.ts")).closeFixture(fixture); }
  });
});
