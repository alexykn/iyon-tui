import { createAppHarness, installIyonVirtualModules } from "@iyon/runtime";
import type { AppHarness } from "@iyon/tui";
import { registerBundledTools } from "@iyon/plugins";
import type { IyonApp } from "../src/app.ts";
import type { FrontendEvent, ToolDraftKey, ToolRendererContribution } from "../src/contracts.ts";

installIyonVirtualModules();
const { createIyonApp } = await import("../src/app.ts");
const bundledTools = await registerBundledTools();
const tools = {
  get(name: string): ToolRendererContribution | undefined {
    return bundledTools.registries.tools.get(name) as unknown as ToolRendererContribution | undefined;
  },
};

export const draft = (messageId: number, contentIndex: number): ToolDraftKey => ({ messageId, contentIndex });

export interface PublicAppFixture {
  readonly app: IyonApp;
  readonly harness: AppHarness;
}

export async function openFixture(width: number, height: number, withQueueIds = false): Promise<PublicAppFixture> {
  const harness = await createAppHarness({ width, height });
  let nextQueueId = 0;
  const app = createIyonApp({
    agent: { run: async () => undefined, cancel: async () => undefined },
    core: {
      submitPrompt: async () => withQueueIds ? ++nextQueueId : undefined,
      steer: async () => withQueueIds ? ++nextQueueId : undefined,
      cancelActiveTurn: async () => undefined,
    },
    model: { provider: "mock", modelId: "mock" },
    tools,
    tui: harness,
  });
  await app.start();
  return { app, harness };
}

export async function closeFixture(fixture: PublicAppFixture): Promise<void> {
  await fixture.app.stop();
  await fixture.harness.close();
}

export async function send(fixture: PublicAppFixture, event: FrontendEvent): Promise<void> {
  await fixture.app.handleAction({ type: "backend", event });
}

export function transcriptLines(harness: AppHarness): string[] {
  return [...harness.screenRows(), ...harness.nativeHistoryRows()];
}

export function advance(fixture: PublicAppFixture, milliseconds = 16, steps = 1): void {
  for (let index = 0; index < steps; index += 1) fixture.harness.advance(milliseconds);
}

export function toolStatusCount(lines: readonly string[], tool: string, status: string): number {
  return lines.filter((line) => line.includes(tool) && line.includes(`— ${status}`)).length;
}
