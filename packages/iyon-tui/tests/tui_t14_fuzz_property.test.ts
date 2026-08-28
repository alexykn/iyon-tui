import { describe, expect, test } from "bun:test";
import { native } from "../src/native.ts";
import { nativeViewAbiSession } from "../src/transport/structural/native-view-abi.ts";
import { viewRenderRef } from "../src/transport/abi/structural/generated/view_calls.ts";
import { nodeForBridge } from "../src/transport/structural/view-bridge.ts";
import { View } from "../src/api/view/view.ts";

const Host = native.NativeTuiHost as unknown as
  (new (width: number, height: number, headless: boolean) => {
    render(view: object): void;
    screenRows(): string[];
    dispose(): void;
  }) | undefined;

function malformed(seed: number): object {
  const cycle: Record<string, unknown> = { id: seed, kind: seed % 32 };
  cycle.child = cycle;
  switch (seed % 6) {
    case 0: return {};
    case 1: return { id: -seed, kind: 999 };
    case 2: return { id: seed, kind: "text", text: 42 };
    case 3: return { id: seed, kind: 2, children: [null, { bad: true }] };
    case 4: return { id: seed, kind: 3, rows: -1, columns: "wide" };
    default: return cycle;
  }
}

describe("PERF-12 T14 malformed-boundary properties", () => {
  test("malformed views and invalid refs do not partially mutate the host", () => {
    if (Host === undefined) return;
    const session = nativeViewAbiSession();
    if (session === undefined) return;
    const host = new Host(12, 4, true);
    try {
      host.render(nodeForBridge(View.text("stable")));
      const baseline = host.screenRows();
      for (let seed = 1; seed <= 100; seed++) {
        try {
          host.render(malformed(seed));
        } catch {
          // Invalid input is expected; the host must remain authoritative.
        }
        expect(host.screenRows(), `malformed seed ${seed}`).toEqual(baseline);
        try {
          viewRenderRef(session.symbols, session.runtime, 0xffff_ffff - seed);
        } catch {
          // Invalid NativeRefs are expected to fail before host mutation.
        }
      }
    } finally {
      host.dispose();
    }
  });
});
