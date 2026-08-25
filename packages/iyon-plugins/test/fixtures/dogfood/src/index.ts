import { readFileSync } from "node:fs";
import process from "node:process";
import { defineTool } from "@iyon/sdk";
import { View } from "@iyon/tui";
import type { ModelStreamEvent, ProviderCapabilities, ProviderDefinition } from "iyon:api";
import type { ExtensionAPI } from "iyon:plugins";

export let runtimeProbe: { readonly npm: string; readonly file: boolean; readonly process: string; readonly network: string } | undefined;
export let selectedAgentContext: unknown;
export let customAppCreated = 0;

const capabilities: ProviderCapabilities = { reasoning: [], tools: true, streaming: true };
const provider: ProviderDefinition = {
  id: "fixture-provider",
  defaultModel: "fixture-model",
  capabilities: () => capabilities,
  create: () => ({
    async *stream(): AsyncIterable<ModelStreamEvent> {
      yield { type: "started" };
      yield { type: "textStart", contentIndex: 0 };
      yield { type: "textDelta", contentIndex: 0, delta: "fixture provider" };
      yield { type: "textEnd", contentIndex: 0, text: "fixture provider" };
      yield { type: "done", stopReason: "stop" };
    },
  }),
};

const replacement = defineTool({
  name: "read",
  description: "fixture replacement read",
  inputSchema: { type: "object", additionalProperties: false },
  execute: async () => ({ content: [{ type: "text", text: "fixture execution" }], details: { fixture: true }, isError: false }),
  renderCall: () => View.text("fixture call") as never,
  renderResult: () => View.text("fixture result") as never,
});

export function activate(api: ExtensionAPI): void | Promise<void> {
  api.tools.register(replacement, { replace: true });
  api.providers.register(provider);
  api.agents.register({
    id: "fixture-agent",
    create(context) {
      selectedAgentContext = context;
      return { run: async () => undefined, cancel: () => undefined };
    },
  });
  api.apps.register({
    id: "fixture-app",
    create() {
      customAppCreated += 1;
      return { id: "fixture-app" };
    },
  });
  api.scene.replace({ id: "fixture-replace", replace: (context) => ({ body: `replaced:${context.appId ?? "none"}` }) });
  api.scene.compose({ id: "fixture-compose", order: 10, compose: (scene) => ({ ...scene, body: `${scene.body}:composed` }) });

  const url = process.env.IYON_DOGFOOD_URL;
  if (url === undefined) throw new Error("dogfood network fixture URL is missing");
  return fetch(url).then(async (response) => {
    runtimeProbe = {
      npm: typeof defineTool,
      file: readFileSync(new URL("../package.json", import.meta.url), "utf8").includes("fixture-dogfood"),
      process: process.env.IYON_DOGFOOD_MARKER ?? "",
      network: await response.text(),
    };
  });
}
