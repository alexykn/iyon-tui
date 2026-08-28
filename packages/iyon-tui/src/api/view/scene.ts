import type { History as HistoryContract } from "../controls/history.ts";
import type { View } from "./view.ts";

/** Structural scene root accepted by the runtime. */
export interface SceneContract {
  readonly history?: HistoryContract;
  readonly body: View;
}

/** A scene value or a producer closure evaluated inside the retained root scope. */
export type SceneProducer = SceneContract | (() => SceneContract);

/**
 * Concrete root value for a body and optional history sideband.
 *
 * The runtime also accepts a structural SceneContract, so callers may pass
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
