import { expect, test } from "bun:test";

import { TextFunnel, TextStreamSource, View } from "../src/index.ts";
import { AppHarness } from "../src/testing/index.ts";

const PERF13H = "PERF-13-H lifetime acceptance";

async function openContentHost(source: TextStreamSource) {
  const tui = await AppHarness.open({ width: 32, height: 4 });
  const port = tui.contentPort();
  const connector = port.connect(source, TextFunnel.plain());
  connector.activate();
  tui.render({ body: View.content(port) });
  return { tui, port };
}

test(`${PERF13H} releases every shared Source subscription on multi-host teardown`, async () => {
  const source = TextStreamSource.create();
  const hosts: Array<Awaited<ReturnType<typeof openContentHost>>> = [];
  try {
    for (let index = 0; index < 8; index += 1) hosts.push(await openContentHost(source));
    source.append("shared content\n");
    for (const { tui } of hosts) {
      tui.flush();
      expect(tui.screenRows().join("\n")).toContain("shared content");
    }
  } finally {
    for (const { tui } of hosts.reverse()) tui.close();
  }
  expect(source.disposed).toBe(false);
  source.dispose();
  expect(source.disposed).toBe(true);
});

test(`${PERF13H} survives repeated host and Connector ownership cycles`, async () => {
  const source = TextStreamSource.create();
  try {
    for (let index = 0; index < 32; index += 1) {
      const { tui } = await openContentHost(source);
      source.append(`cycle ${index}\n`);
      tui.flush();
      tui.close();
    }
    expect(source.stats().revision).toBe(32n);
  } finally {
    source.dispose();
  }
  expect(source.disposed).toBe(true);
});

test(`${PERF13H} invalidates host-owned content handles after owner teardown`, async () => {
  const source = TextStreamSource.create();
  const { tui, port } = await openContentHost(source);
  const connector = port.connect(source, TextFunnel.markdown());
  try {
    tui.close();
    expect(() => connector.activate()).toThrow();
    expect(() => port.mounted()).toThrow();
    expect(source.disposed).toBe(false);
  } finally {
    source.dispose();
  }
});
