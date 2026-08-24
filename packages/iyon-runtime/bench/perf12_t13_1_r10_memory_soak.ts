/**
 * PERF-12 T13.1 R10 — full-scale memory soak (handoff §22/§32.1 R10,
 * AMENDMENT-C §23).
 *
 * Targets:
 *   - 100,000 keyed mount/unmount cycles against a BOUNDED live set;
 *   - State subscriber counts follow live scopes (disposed scopes are not
 *     retained by their sources);
 *   - aborted pending work reclaims (abort churn interleaved every 16th
 *     cycle);
 *   - root disposal releases everything immediately.
 *
 * Synthetic driver (no native involvement): projection-less runtime — the
 * execution substrate's memory behavior is what is under test.
 */

import { RetainedExecutionRuntime } from "../src/tui/execution.ts";
import { defineView } from "../src/tui/define-view.ts";
import { invokeComponent } from "../src/tui/execution.ts";
import { state, trackedStateSubscriberCount } from "../src/tui/tracked-state.ts";
import { composeText, composeVertical } from "../src/tui/compose.ts";
import { View } from "../src/tui/values/view.ts";

const CYCLES = 100_000;
const LIVE_WINDOW = 64;

const sharedState = state("soak");
let throwCards = false;
let cardBodies = 0;

const Card = defineView(() => {
  cardBodies += 1;
  if (throwCards) throw new Error("soak-bomb");
  void sharedState.value; // subscribe while live
  return composeText(`card-${cardBodies}`);
});

/** Sliding keyed window; surviving keys keep their bodies silent. */
const keysRef = { current: [] as number[] };

const Holder = defineView(() => {
  return composeVertical((column) => {
    for (const key of keysRef.current) {
      column.child(View.key(`k${key}`, () => invokeComponent(Card, undefined as never).view));
    }
  });
});

function rss(): number {
  return process.memoryUsage.rss();
}

const runtime = new RetainedExecutionRuntime({ autoFlush: false });
const root = runtime.mountRoot(Holder, undefined as never);

console.log("rss_baseline_mb", Math.round(rss() / 1048576));

let nextKey = 0;
let abortedPasses = 0;
for (let cycle = 0; cycle < CYCLES; cycle += 1) {
  keysRef.current.push(nextKey++);
  if (keysRef.current.length > LIVE_WINDOW) keysRef.current.shift();

  if (cycle % 16 === 15) {
    // Abort churn: cards throw after mounting/staging; the batch must roll
    // back and reclaim every fresh pending scope.
    throwCards = true;
    try {
      runtime.invalidate(root);
      runtime.flush();
    } catch {
      abortedPasses += 1;
    }
    throwCards = false;
  }

  runtime.invalidate(root);
  runtime.flush();

  if (cycle % 20_000 === 19_999) {
    const subscribers = trackedStateSubscriberCount(sharedState);
    console.log(
      `cycles=${String(cycle + 1).padStart(6)} rss_mb=${String(Math.round(rss() / 1048576)).padStart(4)}`
      + ` subscribers=${subscribers} aborted_passes=${abortedPasses}`,
    );
    if (subscribers > LIVE_WINDOW * 4) {
      console.error("FAIL: subscriber count does not follow live scopes");
      process.exit(1);
    }
  }
}

// Steady state: exactly the live window's cards may be subscribed.
const steadySubscribers = trackedStateSubscriberCount(sharedState);
console.log("live_window", keysRef.current.length, "steady_subscribers", steadySubscribers);
if (steadySubscribers > LIVE_WINDOW) {
  console.error("FAIL: leaked subscriptions beyond the live window");
  process.exit(1);
}

// Root disposal releases all strong references immediately (§23).
runtime.dispose();
globalThis.gc?.();
const postDispose = trackedStateSubscriberCount(sharedState);
console.log("post_dispose_subscribers", postDispose);
console.log("rss_final_mb", Math.round(rss() / 1048576));
if (postDispose !== 0) {
  console.error("FAIL: State source retains disposed scopes");
  process.exit(1);
}
console.log(`SOAK PASS — ${CYCLES} keyed cycles, ${abortedPasses} interleaved aborts, bounded live set ${LIVE_WINDOW}`);
