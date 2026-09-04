// PRE-V5 L1-00 step 5: route-preserving trace harness.
//
// Drives structure, retained state, and content through the real native
// addon (headless host, authoritative retained route — never a mock) and
// emits one JSON document with provenance, counter deltas, and screen
// readback. Rust `perf` counters are included only when the loaded addon
// was built with the `perf-counters` feature (feature-detected).
import { TextFunnel, TextStreamSource, View } from "../src/index.ts";
import { AppHarness } from "../src/testing/index.ts";
import { native } from "../src/transport/native/addon.ts";
import { nativeViewAbiSession } from "../src/transport/structural/native-view-abi.ts";
import {
  RetainedRootBoundary,
  resetRetainedIdentityCounters,
  retainedIdentityCounterSnapshot,
} from "../src/transport/structural/retained-dag.ts";
import {
  resetWakeBrokerCounters,
  wakeBrokerCounterSnapshot,
} from "../src/runtime/wake-broker.ts";

function indexed(value: number) {
  return { type: "indexed" as const, value };
}

const provenance = {
  harness: "PRE-V5-L1-trace",
  git_sha: process.env.PRE_V5_GIT_SHA ?? "unknown",
  rustc_version: process.env.PRE_V5_RUSTC_VERSION ?? "unknown",
  target: process.env.PRE_V5_TARGET ?? "unknown",
  addon_sha256: process.env.PRE_V5_NATIVE_SHA256 ?? "unknown",
  bun_version: Bun.version,
};

const nativeRecord = native as unknown as Record<string, unknown>;
const rustCountersAvailable =
  typeof nativeRecord.tuiPerfSnapshot === "function" &&
  typeof nativeRecord.tuiPerfReset === "function";
function readRustCounters(): Record<string, number> | null {
  if (!rustCountersAvailable) return null;
  const snap = (
    native as unknown as { tuiPerfSnapshot(): Record<string, number> }
  ).tuiPerfSnapshot();
  return { ...snap };
}
function resetRustCounters(): void {
  if (!rustCountersAvailable) return;
  (native as unknown as { tuiPerfReset(): void }).tuiPerfReset();
}

// Phase 1: structural publication through the authoritative retained route.
const Host = native.NativeTuiHost;
if (Host === undefined) throw new Error("default addon does not expose NativeTuiHost");
const structuralHost = new Host(80, 24, true);
const session = nativeViewAbiSession();
if (session === undefined) throw new Error("default addon does not expose tuiViewAbiSession");
const boundary = new RetainedRootBoundary(session, () => structuralHost);
resetRetainedIdentityCounters();
resetRustCounters();
try {
  const publication = boundary.prepareInstall(
    View.vertical([
      View.text("trace structural").foreground(indexed(2)),
      View.horizontal([View.text("left"), View.text("right")]),
    ]),
  );
  if (publication === undefined) throw new Error("structural publication refused");
  publication.commit();
} finally {
  boundary.close();
  structuralHost.dispose();
}
const structural = {
  counters: retainedIdentityCounterSnapshot(),
};

// Phase 2: retained-state patch without structural republication.
const harness = await AppHarness.open({ width: 40, height: 8 });
const state = harness.viewState();
harness.render(() => ({ body: View.text("trace state").foreground(indexed(1)).state(state) }));
function styleOfRenderedText(): { row: number; column: number; style: unknown } {
  const rows = harness.screenRows();
  for (const [row, text] of rows.entries()) {
    const column = text.indexOf("trace state");
    if (column >= 0) return { row, column, style: harness.styleAt(row, column) };
  }
  throw new Error("trace text not found on screen");
}
const before = styleOfRenderedText();
state.setPresentation({ foreground: indexed(3), textAttributes: { bold: true } });
const after = styleOfRenderedText();
state.clearPresentation(["foreground"]);
const cleared = styleOfRenderedText();
const styleBefore = before.style;
const styleAfter = after.style;
const styleCleared = cleared.style;
const stateTrace = {
  row: before.row,
  column: before.column,
  styleBefore,
  styleAfter,
  styleCleared,
};

// Phase 3: content append + frame through a live port.
const source = TextStreamSource.create({ retention: { maxBytes: 64 * 1024, overflow: "drop-oldest" } });
resetWakeBrokerCounters();
const port = harness.contentPort();
const connector = port.connect(source, TextFunnel.plain());
connector.activate();
harness.render({ body: View.content(port) });
for (let index = 0; index < 50; index += 1) {
  const text = `trace line ${index}\n`;
  // Every fifth line carries a tag annotation so the annotation-record
  // copy path is exercised and observed.
  const annotations =
    index % 5 === 0
      ? [{ kind: "tag" as const, startByte: 0, endByte: 5, namespace: "trace", name: `line-${index}` }]
      : [];
  source.append(text, annotations);
}
harness.flush();
const stats = source.stats();
const content = {
  revision: stats.revision.toString(),
  retained_bytes: stats.retainedBytes.toString(),
  accepted_bytes: stats.acceptedBytes.toString(),
  copied_bytes: stats.copiedBytes.toString(),
  wake: wakeBrokerCounterSnapshot(),
  screen_rows: harness.screenRows(),
};
harness.close();
source.dispose();

console.log(
  JSON.stringify({
    ...provenance,
    structural,
    state: stateTrace,
    content,
    rust_counters: readRustCounters(),
  }),
);
