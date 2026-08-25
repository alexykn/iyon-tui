/**
 * PERF-12 T13.1 R6a — framework-owned binding between a `Tui` instance and a
 * retained execution runtime (handoff §32.1 R6a, AMENDMENT-C §14).
 *
 * Every scope projection is backed by the existing production machinery:
 * `Tui.createViewSlot` → native ComponentSlot + revision + cached snapshot +
 * RetainedRootBoundary lease. NO new host dependency graphs — local scope
 * updates swap their sub-DAG root through the slot and the native side
 * propagates damage itself (empirically verified: setView alone repaints).
 *
 * Ownership: this is private iyon-tui infrastructure. Consumers receive
 * retained execution automatically through the canonical boundary APIs
 * (R8/R11); they never construct or configure this runtime themselves
 * (handoff §4.1/§25).
 */

import { RetainedExecutionRuntime, type RetainedExecutionRuntimeOptions } from "./execution.ts";
import { View } from "./values/view.ts";
import type { Tui } from "./runtime.ts";

export type TuiExecutionRuntimeOptions = Omit<RetainedExecutionRuntimeOptions, "createScopeProjection">;

/**
 * Creates a retained execution runtime whose scope projections are backed by
 * `tui.createViewSlot` — stable native component identity, revision-driven
 * content swaps, and root leases via the existing PERF-12 boundary.
 */
export function bindExecutionRuntime(
  tui: Tui,
  options: TuiExecutionRuntimeOptions = {},
): RetainedExecutionRuntime {
  return new RetainedExecutionRuntime({
    ...options,
    createScopeProjection: () => {
      const slot = tui.createViewSlot(View.spacer(0));
      const view = slot.view();
      return {
        view,
        install(output: View): void {
          // Commit publication runs with the framework's internal publication
          // token; use the ordinary setter rather than exposing an execution-
          // only mutation method on ViewSlot.
          slot.setView(output);
        },
        preparePublication(output: View): { commit(): void; abort(): void } | undefined {
          // R7: delegate to the slot's own RetainedRootBoundary — ownership
          // stays inside the boundary (no split-brain between slot and lease
          // tables). `undefined` means the boundary is unavailable (no native
          // session); the runtime falls back to the legacy per-scope install.
          return slot.prepareSetView(output);
        },
        dispose(): void {
          slot.dispose();
        },
      };
    },
  });
}
