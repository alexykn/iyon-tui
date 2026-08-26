import type { History as HistoryContract, Scene as SceneContract } from "./types.ts";
import type { View } from "./values/view.ts";

export class Scene implements SceneContract {
  readonly body: View;
  readonly history?: HistoryContract;
  constructor(body: View, history?: HistoryContract) { this.body = body; this.history = history; }
  static from(value: SceneContract): Scene { return new Scene(value.body, value.history); }
}
