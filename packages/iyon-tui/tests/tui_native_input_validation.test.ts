import { expect, test } from "bun:test";
import { fileURLToPath } from "node:url";

import { nativeArtifact } from "../src/transport/native/addon.ts";

const REPOSITORY_ROOT = fileURLToPath(new URL("../../..", import.meta.url));

/**
 * Runs malformed color values in a fresh Bun process. A panic crossing the
 * Node-API boundary aborts the process before JavaScript can catch it, so an
 * in-process assertion is not sufficient for this regression.
 */
function runMalformedColorProbe(): {
  readonly state: readonly { readonly caught: boolean; readonly unchanged: boolean }[];
  readonly theme: readonly boolean[];
  readonly structural: readonly boolean[];
} {
  const script = `
    (async () => {
      const { AppHarness } = await import("./packages/iyon-tui/src/testing/index.ts");
      const { View } = await import("./packages/iyon-tui/src/index.ts");
      const { native, nativeArtifact } = await import("./packages/iyon-tui/src/transport/native/addon.ts");
      const { nativeResourceOf } = await import("./packages/iyon-tui/src/transport/native/resources.ts");
      const { nativeViewAbiSession } = await import("./packages/iyon-tui/src/transport/structural/native-view-abi.ts");
      const {
        styleAtomCreateCstring,
        viewDecoratedCreateBuffer,
        viewReleaseMany,
        viewTextCreateCstring,
      } = await import("./packages/iyon-tui/src/transport/abi/structural/generated/view_calls.ts");

      const malformed = ["#aé000", "#000é0"];
      const tui = await AppHarness.open({ width: 12, height: 4 });
      const state = tui.viewState();
      const stateResults = [];
      try {
        tui.render(() => ({ body: View.text("lane").state(state) }));
        state.setPresentation({ foreground: { type: "named", value: "red" } });
        tui.flush();
        const resource = nativeResourceOf(state);
        for (const value of malformed) {
          const before = tui.styleAt(0, 0);
          const exactStrings = Array(14).fill("");
          exactStrings[0] = value;
          let caught = false;
          try {
            // Keep the original default-N-API reproduction in the
            // subprocess: one foreground value and otherwise ignored lanes.
            resource.setPresentation(1, 0, 0, [0, 0, 0, 0, 0], exactStrings);
          } catch {
            caught = true;
          }
          tui.flush();
          stateResults.push({ caught, unchanged: JSON.stringify(before) === JSON.stringify(tui.styleAt(0, 0)) });

          const mixedBefore = tui.styleAt(0, 0);
          const strings = Array(14).fill("");
          // The preceding background lane is valid. The malformed border
          // color follows it, proving a partially decoded patch is not
          // installed when a later value fails.
          strings[1] = "green";
          strings[2] = value;
          caught = false;
          try {
            resource.setPresentation(0x6, 0, 0, [0, 0, 0, 0, 0], strings);
          } catch {
            caught = true;
          }
          tui.flush();
          stateResults.push({ caught, unchanged: JSON.stringify(mixedBefore) === JSON.stringify(tui.styleAt(0, 0)) });
        }
      } finally {
        tui.close();
      }

      const Host = native.NativeTuiHost;
      if (Host === undefined) throw new Error("NativeTuiHost is unavailable");
      const host = new Host(12, 4, true);
      const themeResults = [];
      try {
        for (const value of malformed) {
          let caught = false;
          try {
            host.setTheme({ colors: { accent: { base: value } } });
          } catch {
            caught = true;
          }
          themeResults.push(caught);
        }
      } finally {
        host.dispose();
      }

      const session = nativeViewAbiSession();
      const structuralResults = [];
      for (const [index, value] of malformed.entries()) {
        const atom = styleAtomCreateCstring(session.symbols, session.runtime, value);
        const child = viewTextCreateCstring(session.symbols, session.runtime, 10_000 + index, 0, "x", 0, 3, 1);
        const words = new Uint32Array(11);
        words[0] = 4; // foreground atom lane
        words[6] = atom;
        let caught = false;
        try {
          viewDecoratedCreateBuffer(
            session.symbols,
            session.runtime,
            11_000 + index,
            0,
            child,
            0,
            words,
            words.length,
            new Uint8Array(1),
            0,
          );
        } catch {
          caught = true;
        } finally {
          viewReleaseMany(session.symbols, session.runtime, new Uint32Array([child]), 1);
        }
        structuralResults.push(caught);
      }

      if (nativeArtifact.absolutePath.length === 0) throw new Error("native artifact path is empty");
      console.log(JSON.stringify({ state: stateResults, theme: themeResults, structural: structuralResults }));
    })().catch((error) => {
      console.error(error);
      process.exit(1);
    });
  `;
  const result = Bun.spawnSync({
    cmd: [process.execPath, "--eval", script],
    cwd: REPOSITORY_ROOT,
    env: { ...process.env, ION_TUI_NATIVE_ARTIFACT: nativeArtifact.absolutePath },
    stdout: "pipe",
    stderr: "pipe",
  });
  const stdout = new TextDecoder().decode(result.stdout).trim();
  const stderr = new TextDecoder().decode(result.stderr).trim();
  if (result.exitCode !== 0) {
    throw new Error(`malformed color subprocess exited ${result.exitCode}: ${stderr || stdout}`);
  }
  return JSON.parse(stdout) as ReturnType<typeof runMalformedColorProbe>;
}

test("native malformed RGB values are catchable and state patches stay atomic", () => {
  const result = runMalformedColorProbe();
  expect(result.state).toEqual([
    { caught: true, unchanged: true },
    { caught: true, unchanged: true },
    { caught: true, unchanged: true },
    { caught: true, unchanged: true },
  ]);
  expect(result.theme).toEqual([true, true]);
  expect(result.structural).toEqual([true, true]);
});
