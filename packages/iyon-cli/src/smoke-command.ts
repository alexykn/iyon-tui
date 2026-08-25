import {
  echoBuffer,
  echoJson,
  echoString,
  nativeVersion,
} from "iyon:api";
import {
  CancellationProbe,
  EventQueueProbe,
  NativeCounter,
  asyncSleep,
  nativeCounterStats,
  resetNativeCounterStats,
  runWithAbortSignal,
} from "iyon:core";
import { tuiSmoke } from "@iyon/tui";

const wait = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms));

function assert(condition: boolean, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}

function assertDeepEqual(actual: unknown, expected: unknown, message: string): void {
  assert(JSON.stringify(actual) === JSON.stringify(expected), message);
}

async function assertRejects(operation: Promise<unknown>, pattern: RegExp): Promise<void> {
  try {
    await operation;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    assert(pattern.test(message), `rejection did not match ${pattern}: ${message}`);
    return;
  }
  throw new Error(`expected rejection matching ${pattern}`);
}

async function assertCounterFinalization(): Promise<void> {
  resetNativeCounterStats();
  let counter: InstanceType<typeof NativeCounter> | undefined = new NativeCounter();
  assert(counter.increment() === 1, "NativeCounter increment probe failed");
  assert(counter.value() === 1, "NativeCounter value probe failed");
  counter = undefined;

  for (let attempt = 0; attempt < 40; attempt += 1) {
    Bun.gc(true);
    await wait(5);
    const stats = nativeCounterStats();
    if (stats.live === 0 && stats.finalized > 0) {
      return;
    }
  }

  throw new Error("NativeCounter finalizer did not run within the bounded polling window");
}

export async function runSmokeCommand(): Promise<{
  ok: true;
  native: string;
  tui: string;
  concurrent: number;
  event: string;
}> {
  assert(nativeVersion() === "iyon-native/t1", "native version probe failed");
  assert(tuiSmoke === "iyon:tui/t1", "TUI smoke marker failed");

  const jsonValue = { nested: [null, true, 42, "text"] };
  assertDeepEqual(echoJson(jsonValue), jsonValue, "JSON round trip failed");
  const largeString = "x".repeat(1024 * 1024);
  assert(echoString(largeString) === largeString, "large string transfer failed");
  assertDeepEqual(
    [...echoBuffer(Buffer.from([0, 1, 2, 255]))],
    [0, 1, 2, 255],
    "Buffer transfer failed",
  );

  assert((await asyncSleep(0)) === "slept", "async success probe failed");
  await assertRejects(asyncSleep(0xffffffff), /invalid input/);

  const concurrent = await Promise.all(Array.from({ length: 100 }, () => asyncSleep(0)));
  assert(concurrent.length === 100, "concurrent future probe failed");

  const controller = new AbortController();
  const cancellationProbe = new CancellationProbe();
  const cancellation = runWithAbortSignal(controller.signal, {
    run: () => cancellationProbe.run(10_000),
    cancel: () => cancellationProbe.cancel(),
  });
  controller.abort();
  await assertRejects(cancellation, /cancelled/);

  await assertCounterFinalization();

  const queue = new EventQueueProbe();
  await queue.send({ id: 1 });
  await queue.send({ id: 2 });
  assertDeepEqual(await queue.nextEvent(), { id: 1 }, "first queue event was not preserved");
  assertDeepEqual(await queue.nextEvent(), { id: 2 }, "second queue event was not preserved");
  const waiting = queue.nextEvent();
  queue.close();
  assert((await waiting) === null, "closed queue did not resolve to null");
  await assertRejects(queue.send({ after: "close" }), /closed/);

  return {
    ok: true,
    native: nativeVersion(),
    tui: tuiSmoke,
    concurrent: concurrent.length,
    event: "fifo-and-close",
  };
}
