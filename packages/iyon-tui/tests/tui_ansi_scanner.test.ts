import { expect, test } from "bun:test";

import { TextFunnel, TextStreamSource, View } from "../src/index.ts";
import { AppHarness } from "../src/testing/index.ts";

const ANSI = "PRE-V5-L1 ANSI/UTF-8 scanner boundary characterization";

async function renderParts(parts: string[], funnel = TextFunnel.ansi()): Promise<string[]> {
  const harness = await AppHarness.open({ width: 40, height: 8 });
  const source = TextStreamSource.create({
    retention: { maxBytes: 64 * 1024, overflow: "drop-oldest" },
  });
  const port = harness.contentPort();
  const connector = port.connect(source, funnel);
  connector.activate();
  harness.render({ body: View.content(port) });
  for (const part of parts) source.append(part);
  harness.flush();
  const rows = harness.screenRows().map((row) => row.trimEnd());
  harness.close();
  source.dispose();
  return rows;
}

test(`${ANSI} every append split through escape sequences renders identically`, async () => {
  const doc = "\x1b[1;31mRED\x1b[0m plain \x1b[4mUL\x1b[0m\nsecond\n";
  const reference = await renderParts([doc]);
  expect(reference.some((row) => row.includes("RED") && row.includes("plain"))).toBe(true);
  for (let split = 1; split < doc.length; split += 1) {
    const rows = await renderParts([doc.slice(0, split), doc.slice(split)]);
    expect(rows).toEqual(reference);
  }
});

test(`${ANSI} SGR intent reaches styles while unsafe sequences are consumed`, async () => {
  const harness = await AppHarness.open({ width: 40, height: 8 });
  const source = TextStreamSource.create({
    retention: { maxBytes: 64 * 1024, overflow: "drop-oldest" },
  });
  try {
    const port = harness.contentPort();
    const connector = port.connect(source, TextFunnel.ansi());
    connector.activate();
    harness.render({ body: View.content(port) });
    source.append("\x1b[1mRED\x1b[0m");
    harness.flush();

    const rows = harness.screenRows();
    let found = false;
    for (const [row, text] of rows.entries()) {
      const column = text.indexOf("RED");
      if (column >= 0) {
        found = true;
        const style = harness.styleAt(row, column) as { bold?: unknown };
        expect(style.bold).toBe(true);
      }
    }
    expect(found).toBe(true);
  } finally {
    harness.close();
    source.dispose();
  }

  // Unsafe cursor/window operation: consumed, surrounding text joined.
  expect(await renderParts(["a\x1b[2Jb\n"])).toContain("ab");
  // Unfinished trailing escape: text survives, no crash.
  expect(await renderParts(["abc\x1b["])).toContain("abc");
  // OSC 8 hyperlink consumed with links on and off; visible text identical.
  const link = "\x1b]8;;http://example\x07LINK\x1b]8;;\x07\n";
  expect(await renderParts([link])).toContain("LINK");
  expect(await renderParts([link], TextFunnel.ansi({ hyperlinks: false }))).toContain("LINK");
  // BEL and ST terminators are equivalent.
  const bel = await renderParts(["\x1b]8;;http://example\x07LINK\x1b]8;;\x07\n"]);
  const st = await renderParts(["\x1b]8;;http://example\x1b\\LINK\x1b]8;;\x1b\\\n"]);
  expect(st).toEqual(bel);
});

test(`${ANSI} a UTF-8 continuation byte matching C1 CSI blanks new output until removed`, async () => {
  // U+00DB "Û" is bytes C3 9B: the 0x9B trail byte hits the scanner's C1-CSI
  // arm even in plain text with no escape anywhere nearby. The projection
  // fails safely (no panic, no corruption) but the connector holds its last
  // committed frame, so every newer revision is suppressed while the byte
  // remains in the domain. Any scanner repair must change this fixture
  // explicitly; do not "fix" it by weakening the assertions.
  const harness = await AppHarness.open({ width: 40, height: 8 });
  const source = TextStreamSource.create({
    retention: { maxBytes: 64 * 1024, overflow: "drop-oldest" },
  });
  try {
    const port = harness.contentPort();
    const connector = port.connect(source, TextFunnel.ansi());
    connector.activate();
    harness.render({ body: View.content(port) });
    const visible = (): string[] =>
      harness.screenRows().map((row) => row.trimEnd()).filter((row) => row.length > 0);

    // Control without a 0x9B byte renders normally.
    source.append("\x1b[1mé\x1b[0m\n");
    harness.flush();
    expect(visible()).toEqual(["é"]);

    // The poison byte suppresses the new revision; committed output holds.
    source.append("Û bad\n");
    harness.flush();
    expect(visible()).toEqual(["é"]);

    // A clean revision (poison byte gone) recovers the connector.
    source.replace("healed\n");
    harness.flush();
    expect(visible()).toEqual(["healed"]);
  } finally {
    harness.close();
    source.dispose();
  }

  // A domain whose first content carries the byte never commits anything.
  expect(await renderParts(["Û\n"])).toEqual(["", "", "", "", "", "", "", ""]);
});
