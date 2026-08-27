import type { History as HistoryContract, Scene as SceneContract } from "./types.ts";
import type { View } from "./values/view.ts";

/**
 * Concrete root value for a body and optional history sideband.
 *
 * The runtime also accepts a structural `SceneContract`, so callers may pass
 * plain `{ body, history? }` values without constructing this class. Direct
 * scene values and retained scene producers keep their distinct ownership
 * semantics at the render boundary.
 */
export class Scene implements SceneContract {
  readonly body: View;
  readonly history?: HistoryContract;
  constructor(body: View, history?: HistoryContract) { this.body = body; this.history = history; }

  /** Normalize a structural scene value to the concrete root representation. */
  static from(value: SceneContract): Scene { return new Scene(value.body, value.history); }
}
