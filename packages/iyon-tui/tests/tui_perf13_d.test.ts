import { expect, test } from "bun:test";

import {
  ContentConnector,
  TextFunnel,
  TextStreamSource,
  Tui,
  View,
} from "../src/index.ts";
import { nativeResourceOf } from "../src/transport/native/resources.ts";
import { runtimeResourceRegistry } from "../src/transport/native/resource-registry.ts";

const PERF13D = "PERF-13-D content identities";

test(`${PERF13D} keeps Source ownership independent from host teardown`, async () => {
  const source = TextStreamSource.create();
  const first = await Tui.open({ width: 20, height: 4, headless: true });
  const firstPort = first.contentPort();
  const firstConnector = firstPort.connect(source, TextFunnel.plain());
  firstConnector.activate();
  expect(firstConnector.status().phase).toBe("waiting-for-mount");
  first.render({ body: View.content(firstPort) });
  expect(firstConnector.status().phase).toBe("active");
  expect(() => source.dispose()).toThrow("SOURCE_IN_USE");
  first.close();
  expect(source.disposed).toBe(false);

  const second = await Tui.open({ width: 20, height: 4, headless: true });
  try {
    const secondPort = second.contentPort();
    const secondConnector = secondPort.connect(source, TextFunnel.plain());
    secondConnector.activate();
    second.render({ body: View.content(secondPort) });
    expect(secondConnector.status().phase).toBe("active");
    secondConnector.dispose();
    second.flush();
    second.render({ body: View.text("empty") });
    secondPort.dispose();
  } finally {
    second.close();
    source.dispose();
  }
  expect(source.disposed).toBe(true);
});

test(`${PERF13D} rejects cross-host and duplicate ContentPort attachment`, async () => {
  const first = await Tui.open({ width: 20, height: 4, headless: true });
  const second = await Tui.open({ width: 20, height: 4, headless: true });
  try {
    const port = first.contentPort();
    expect(() => second.render({ body: View.content(port) })).toThrow("WRONG_HOST");
    const duplicate = View.horizontal([View.content(port), View.content(port)]);
    expect(() => first.render({ body: duplicate })).toThrow("DUPLICATE_CONTENT_PORT_ATTACHMENT");
  } finally {
    first.close();
    second.close();
  }
});

test(`${PERF13D} preserves the visible Connector across candidate failure`, async () => {
  const source = TextStreamSource.create();
  const tui = await Tui.open({ width: 20, height: 4, headless: true });
  const port = tui.contentPort();
  const first = port.connect(source, TextFunnel.plain());
  const second = port.connect(source, TextFunnel.plain());
  try {
    first.activate();
    tui.render({ body: View.content(port) });
    expect(first.status().phase).toBe("active");

    (nativeResourceOf(second) as { failNextActivation(diagnostic: string): void })
      .failNextActivation("synthetic projection failure");
    second.activate();
    expect(second.status()).toMatchObject({ requested: true, visible: false });
    expect(first.status()).toMatchObject({ phase: "active", visible: true });
    tui.flush();
    expect(first.status()).toMatchObject({ phase: "active", visible: true, requested: false });
    expect(second.status()).toMatchObject({ phase: "failed", visible: false, requested: true });

    second.activate();
    tui.flush();
    expect(second.status()).toMatchObject({ phase: "active", visible: true, requested: true });

    second.dispose();
    tui.flush();
    tui.render({ body: View.text("empty") });
    first.dispose();
    port.dispose();
    source.dispose();
  } finally {
    tui.close();
    source.dispose();
  }
});

test(`${PERF13D} exposes stable native lifecycle error codes`, async () => {
  const source = TextStreamSource.create();
  const tui = await Tui.open({ width: 20, height: 4, headless: true });
  const port = tui.contentPort();
  const connector = port.connect(source, TextFunnel.plain());
  try {
    expect(() => source.dispose()).toThrow("SOURCE_IN_USE");
    try {
      source.dispose();
    } catch (error) {
      expect((error as { nativeCode?: string }).nativeCode).toBe("ION_SOURCE_IN_USE");
    }
    tui.render({ body: View.content(port) });
    try {
      port.dispose();
      throw new Error("mounted ContentPort disposal unexpectedly succeeded");
    } catch (error) {
      expect((error as { nativeCode?: string }).nativeCode).toBe("ION_PORT_MOUNTED");
    }
    tui.render({ body: View.text("empty") });
    connector.dispose();
    port.dispose();
    source.dispose();
  } finally {
    tui.close();
    source.dispose();
  }
});

test(`${PERF13D} maps deferred cleanup through the supported error status lane`, () => {
  let phase: "active" | "disposed" = "active";
  let error: { readonly code: string; readonly diagnostic: string } | undefined = {
    code: "SOURCE_CLEANUP_PENDING",
    diagnostic: "Source membership is retained until an explicit retry",
  };
  const resource = {
    activate: () => ({ schedule_environment_drain: false }),
    deactivate: () => ({ schedule_environment_drain: false }),
    dispose: () => {
      phase = "disposed";
      return { schedule_environment_drain: false };
    },
    status: () => ({
      phase,
      requested: false,
      visible: false,
      error,
    }),
  };
  const connector = ContentConnector.create(
    resource,
    { environment: runtimeResourceRegistry().environment, host: {} },
    () => {},
    () => {},
    { forgetConnector: () => {} } as never,
    undefined as never,
  );
  try {
    const pending = connector.status();
    expect(pending).toEqual({
      phase: "active",
      requested: false,
      visible: false,
      error,
    });
    expect("cleanup_pending" in pending).toBe(false);
    expect("cleanup_error" in pending).toBe(false);

    // A normal operating failure continues to use the same supported field;
    // callers can distinguish it by code without double-applying the
    // already-accepted Source operation.
    error = { code: "PROJECTION_FAILED", diagnostic: "projection failed" };
    expect(connector.status().error).toEqual(error);
  } finally {
    connector.dispose();
  }
});
