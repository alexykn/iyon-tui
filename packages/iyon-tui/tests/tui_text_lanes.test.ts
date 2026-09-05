import { expect, test } from "bun:test";

import { StyleSpec, TextSpan, View } from "../src/index.ts";
import { AppHarness } from "../src/testing/index.ts";

const LANES = "PRE-V5-L1 text lane equivalence characterization";

async function renderBody(body: View): Promise<string[]> {
  const harness = await AppHarness.open({ width: 40, height: 10 });
  try {
    harness.render({ body });
    const rows = harness.screenRows().map((row) => row.trimEnd());
    harness.close();
    return rows;
  } catch (error) {
    harness.close();
    throw error;
  }
}

test(`${LANES} fixed and buffer lanes render identical text`, async () => {
  // Same visible text, different span splits: four spans travel the
  // fixed-arity lane while five take the words+bytes buffer lane.
  const four = View.styledText(
    ["alpha ", "beta ", "gamma ", "delta"].map((text) => TextSpan.plain(text)),
  );
  const five = View.styledText(
    ["alpha ", "beta ", "gam", "ma ", "delta"].map((text) => TextSpan.plain(text)),
  );
  const fourRows = await renderBody(four);
  const fiveRows = await renderBody(five);
  expect(fiveRows).toEqual(fourRows);
  expect(fourRows.some((row) => row.includes("alpha beta gamma delta"))).toBe(true);
});

test(`${LANES} styled spans render across lanes`, async () => {
  const styled = View.styledText([
    TextSpan.plain("plain "),
    TextSpan.styled("bold", new StyleSpec().bold()),
  ]);
  const rows = await renderBody(styled);
  expect(rows.some((row) => row.includes("plain") && row.includes("bold"))).toBe(true);
});

test(`${LANES} NUL, empty, trailing newline, and Unicode survive the utf8 lane`, async () => {
  const nul = await renderBody(View.text("left\0right"));
  expect(nul.some((row) => row.includes("left") && row.includes("right"))).toBe(true);
  // Re-render stability (cache-first path) for the same content.
  const harness = await AppHarness.open({ width: 40, height: 10 });
  try {
    const body = View.text("left\0right");
    harness.render({ body });
    const first = harness.screenRows().map((row) => row.trimEnd());
    harness.render({ body });
    const second = harness.screenRows().map((row) => row.trimEnd());
    expect(second).toEqual(first);
  } finally {
    harness.close();
  }

  const empty = await renderBody(View.text(""));
  expect(empty.every((row) => row === "")).toBe(true);

  const trailing = await renderBody(View.text("line\n"));
  expect(trailing.some((row) => row.includes("line"))).toBe(true);

  const unicode = await renderBody(View.text("héllo 🌍 Û"));
  expect(unicode.some((row) => row.includes("héllo") && row.includes("🌍"))).toBe(true);
});
