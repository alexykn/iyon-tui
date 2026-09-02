import { TextFunnel, TextStreamSource, View } from "../src/index.ts";
import { AppHarness } from "../src/testing/index.ts";
import { wakeBrokerCounterSnapshot, resetWakeBrokerCounters } from "../src/runtime/wake-broker.ts";

const iterations = Number(process.env.PERF13_H_ITERATIONS ?? 1_000);
if (!Number.isSafeInteger(iterations) || iterations <= 0) {
  throw new Error("PERF13_H_ITERATIONS must be a positive safe integer");
}

const source = TextStreamSource.create({
  retention: { maxBytes: 64 * 1024, overflow: "drop-oldest" },
});
const harness = await AppHarness.open({ width: 80, height: 24 });
resetWakeBrokerCounters();
try {
  const port = harness.contentPort();
  const connector = port.connect(source, TextFunnel.plain());
  connector.activate();
  harness.render({ body: View.content(port) });

  const appendStart = Bun.nanoseconds();
  for (let index = 0; index < iterations; index += 1) source.append(`line ${index}\n`);
  const appendNs = Bun.nanoseconds() - appendStart;

  const frameStart = Bun.nanoseconds();
  harness.flush();
  const frameNs = Bun.nanoseconds() - frameStart;
  const stats = source.stats();
  const wake = wakeBrokerCounterSnapshot();

  console.log(JSON.stringify({
    benchmark: "PERF-13-H-content",
    iterations,
    append_ns: appendNs,
    frame_ns: frameNs,
    source: {
      revision: stats.revision.toString(),
      retained_bytes: stats.retainedBytes.toString(),
      chunk_count: stats.chunkCount,
      accepted_bytes: stats.acceptedBytes.toString(),
      copied_bytes: stats.copiedBytes.toString(),
      dropped_head_bytes: stats.droppedHeadBytes.toString(),
    },
    wake,
  }));
} finally {
  harness.close();
  source.dispose();
}
