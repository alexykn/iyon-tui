// PRE-V5-L1 step-5 benchmark: current N-API CString round trip, separately by
// text lane. The TS materializer picks lanes by content (see retained-dag.ts
// PERF-12 T11): NUL-free spans ride the cstring family (Bun lowers strings
// natively), NUL-bearing spans ride the exact-byte utf8 family, and span
// counts above 4 ride the words+bytes buffer lane. This bench cold-publishes
// N distinct nodes per lane through a headless render and reports wall time
// and per-node cost. Changing which lane the materializer chooses later
// requires comparing against THESE numbers, not assumptions.
//
// Usage: bun packages/iyon-tui/bench/pre-v5-l1-text-lanes.ts
import { TextSpan, View } from "../src/index.ts";
import { AppHarness } from "../src/testing/index.ts";
import { StyleSpec } from "../src/index.ts";

const NODES = 300;

async function benchLane(
  label: string,
  makeBody: (index: number) => View,
): Promise<{ label: string; ms: number; perNodeUs: number; probeOk: boolean }> {
  const children: View[] = [];
  for (let index = 0; index < NODES; index += 1) children.push(makeBody(index));
  const harness = await AppHarness.open({ width: 80, height: NODES + 4 });
  const start = performance.now();
  harness.render({ body: View.vertical(children) });
  const ms = performance.now() - start;
  const rows = harness.screenRows();
  const probeOk = rows.some((row) => row.includes("node-0000"));
  harness.close();
  return { label, ms, perNodeUs: (ms * 1000) / NODES, probeOk };
}

const pad = (index: number): string => `node-${String(index).padStart(4, "0")}`;

const results = [
  // CString lane: short NUL-free single-span text.
  await benchLane("cstring-1span", (i) => View.text(`${pad(i)} plain payload`)),
  // CString lane: two styled spans, still NUL-free and within arity 4.
  await benchLane("cstring-2span-styled", (i) =>
    View.styledText([
      TextSpan.plain(`${pad(i)} a `),
      TextSpan.styled(`b-${i}`, new StyleSpec().bold()),
    ]),
  ),
  // Exact-byte utf8 lane: embedded NUL forces length-delimited transfer.
  await benchLane("utf8-nul", (i) => View.text(`${pad(i)} le\0ft`)),
  // Buffer lane: six spans exceed the fixed-arity families.
  await benchLane("buffer-6span", (i) =>
    View.styledText(
      ["a", "b", "c", "d", "e", `${pad(i)}`].map((text) => TextSpan.plain(`${text} `)),
    ),
  ),
];

console.log(JSON.stringify({ bench: "PRE-V5-L1-text-lanes", nodes: NODES, results }, null, 2));
if (results.some((r) => !r.probeOk)) {
  console.error("probe text missing from a lane render");
  process.exit(1);
}
