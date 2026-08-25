import { describe, expect, test } from "bun:test";
import { installIyonVirtualModules } from "@iyon/runtime";
import { createAppHarness } from "@iyon/tui";
import { discoverPackageRoot, PackageLoader, selectApp } from "@iyon/plugins";
import type { IyonApp } from "../src/app.ts";

installIyonVirtualModules();

const packageRoot = new URL("../", import.meta.url).pathname;

describe("default app package", () => {
  test("loads through the ordinary app contribution path and has a native lifecycle", async () => {
    const loader = new PackageLoader();
    const candidate = await discoverPackageRoot(packageRoot, "bundled");
    const results = await loader.load(candidate);

    expect(results[0]?.ok).toBe(true);
    expect(loader.registries.apps.get("iyon")?.id).toBe("iyon");

    const harness = await createAppHarness({ width: 60, height: 12 });
    let runs = 0;
    const submits: string[] = [];
    const selected = await selectApp(loader.registries.apps, {
      id: "iyon",
      context: { agent: { run: () => { runs += 1; } }, core: { submitTurn: (text: string) => { submits.push(text); } }, model: { provider: "mock", modelId: "mock" }, tui: harness },
    });
    const app = selected.app as IyonApp;

    await app.start();
    expect(harness.exited()).toBe(false);
    const rows = harness.screenRows();
    expect(rows.at(-1)).toContain("effort: Medium");
    expect(rows.filter((row) => row.includes("─")).length).toBeGreaterThanOrEqual(2);
    expect(rows.every((row) => !row.includes("You:") && !row.includes("Working…"))).toBe(true);
    expect(rows.slice(0, -1).some((row) => row.includes("Iyon"))).toBe(false);
    for (const key of "hello") harness.pressKey(key);
    expect(await app.composer.text()).toBe("hello");
    harness.pressKey("Enter");
    const first = await harness.nextAction();
    expect(first).toEqual({ actionId: "submit", payload: "hello" });
    await app.handleAction({ type: "submit", text: first?.payload ?? "" });
    expect(await app.composer.text()).toBe("");
    expect(runs).toBe(1);
    expect(submits).toEqual(["hello"]);
    expect(harness.screenRows().some((row) => row.includes("Working"))).toBe(true);
    const firstTurnRows = [...harness.screenRows(), ...harness.nativeHistoryRows()];
    expect(firstTurnRows.filter((row) => row.includes("hello")).length).toBe(1);
    expect(firstTurnRows.every((row) => !row.includes("You:"))).toBe(true);

    for (const key of "steer") harness.pressKey(key);
    expect(await app.composer.text()).toBe("steer");
    harness.pressKey("Enter");
    const second = await harness.nextAction();
    expect(second).toEqual({ actionId: "submit", payload: "steer" });
    await app.handleAction({ type: "submit", text: second?.payload ?? "" });
    expect(runs).toBe(1);
    expect(submits).toEqual(["hello", "steer"]);
    expect(harness.screenRows().some((row) => row.includes("Queue: steer") && row.includes("Working"))).toBe(true);
    for (let index = 0; index < 5; index += 1) harness.advance(80);
    expect(harness.screenRows().some((row) => row.includes("Queue: steer") && row.includes("waiting"))).toBe(true);
    const queuedRows = [...harness.screenRows(), ...harness.nativeHistoryRows()];
    expect(queuedRows.filter((row) => row.includes("hello")).length).toBe(1);
    expect(queuedRows.some((row) => row.trim() === "steer")).toBe(false);
    expect(queuedRows.every((row) => !row.includes("Working…"))).toBe(true);
    await app.stop();
    await harness.close();
    await loader.unload("@iyon/app-iyon");
  });
});
