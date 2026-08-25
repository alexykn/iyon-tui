import type { ModelApi, ModelErrorKind, ModelRequest, ModelStreamEvent } from "iyon:api";
import type { JsonValue } from "@iyon/sdk";
import { buildRequestBody } from "./serialize.ts";
import { createStreamState, flushToolCalls, normalizeChunk } from "./normalize.ts";
import { parseSse } from "./sse.ts";

export const DEFAULT_BASE_URL = "https://openrouter.ai/api/v1";
export const DEFAULT_MODEL = "meta/muse-spark-1.2-contributor";

export interface OpenRouterProviderConfig {
  readonly apiKey: string;
  readonly model?: string;
  readonly baseUrl?: string;
  readonly title?: string;
  readonly fetch?: typeof fetch;
  readonly sleep?: (ms: number) => Promise<void>;
}

export interface OpenRouterFactoryConfig extends Partial<OpenRouterProviderConfig> {
  readonly credentials?: import("@iyon/sdk").CredentialStore;
}

export class OpenRouterProvider implements ModelApi {
  private readonly config: Required<Pick<OpenRouterProviderConfig, "apiKey" | "model" | "baseUrl">> & OpenRouterProviderConfig;

  constructor(config: OpenRouterProviderConfig) {
    this.config = { ...config, model: config.model ?? DEFAULT_MODEL, baseUrl: config.baseUrl ?? DEFAULT_BASE_URL };
  }

  async *stream(request: ModelRequest, options: { readonly signal?: AbortSignal } = {}): AsyncIterable<ModelStreamEvent> {
    const response = await this.sendWithRetry(request, options.signal);
    if (!response.body) throw providerError("OpenRouter returned an empty response stream", "provider");
    const state = createStreamState();
    yield { type: "started" };
    for await (const data of parseSse(response.body)) {
      if (data === "[DONE]") continue;
      let parsed: JsonValue;
      try { parsed = JSON.parse(data) as JsonValue; } catch (error) { throw providerError(`invalid chat chunk json: ${error instanceof Error ? error.message : "parse failure"}`, "provider"); }
      for (const event of normalizeChunk(parsed, state)) yield event;
    }
    if (state.anyToolCall && state.stopReason === "stop") state.stopReason = "toolUse";
    for (const event of flushToolCalls(state)) yield event;
    yield { type: "done", stopReason: state.stopReason };
  }

  private async sendWithRetry(request: ModelRequest, signal?: AbortSignal): Promise<Response> {
    const fetcher = this.config.fetch ?? fetch;
    const sleep = this.config.sleep ?? ((ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms)));
    let lastError: unknown;
    for (let attempt = 0; attempt <= 2; attempt += 1) {
      try {
        const headers: Record<string, string> = {
          authorization: `Bearer ${this.config.apiKey}`,
          accept: "text/event-stream",
          "content-type": "application/json",
        };
        const title = this.config.title ?? process.env.OPENROUTER_TITLE;
        if (title?.trim()) headers["x-openrouter-title"] = title;
        const response = await fetcher(`${this.config.baseUrl}/chat/completions`, {
          method: "POST", headers, body: JSON.stringify(buildRequestBody(request, this.config.model)), signal,
        });
        if (response.ok) return response;
        const error = await httpError(response, "openrouter");
        if (!retryable(response.status) || attempt === 2) throw error;
        lastError = error;
      } catch (error) {
        lastError = error;
        if (isModelError(error) && !retryableKind(error.kind)) throw error;
        if (attempt === 2) throw error;
      }
      await sleep(300 * 2 ** attempt);
    }
    throw lastError ?? providerError("openrouter request failed", "unknown");
  }
}

export async function createOpenRouterProvider(config: OpenRouterFactoryConfig = {}): Promise<OpenRouterProvider> {
  const { resolveApiKey } = await import("./auth.ts");
  const apiKey = await resolveApiKey(config);
  if (!apiKey) throw providerError("OpenRouter credentials are unavailable", "authentication");
  return new OpenRouterProvider({ ...config, apiKey });
}

async function httpError(response: Response, provider: string): Promise<Error & { readonly kind: ModelErrorKind }> {
  const body = (await response.text()).slice(0, 512);
  let detail = body;
  try {
    const parsed = JSON.parse(body) as Record<string, unknown>;
    const error = parsed.error;
    detail = typeof error === "string" ? error : typeof parsed.message === "string" ? parsed.message : body;
  } catch { /* retain bounded body text */ }
  const kind: ModelErrorKind = response.status === 401 || response.status === 403 ? "authentication" : response.status === 429 ? "rateLimited" : response.status === 400 ? "invalidRequest" : response.status >= 500 ? "transport" : "provider";
  return providerError(`${provider} request failed (${response.status}): ${detail}`, kind);
}

function retryable(status: number): boolean { return status === 429 || status >= 500; }
function retryableKind(kind: ModelErrorKind): boolean { return kind === "transport" || kind === "rateLimited"; }
function isModelError(error: unknown): error is { readonly kind: ModelErrorKind } { return !!error && typeof error === "object" && typeof (error as { kind?: unknown }).kind === "string"; }
function providerError(message: string, kind: ModelErrorKind): Error & { readonly kind: ModelErrorKind } { return Object.assign(new Error(message), { kind }); }
