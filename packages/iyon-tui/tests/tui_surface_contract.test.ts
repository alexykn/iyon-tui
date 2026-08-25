import { describe, expect, test } from "bun:test";
import type { ComponentAdapter, History, Scene, TextInput, View } from "../src/index.ts";

describe("T5 iyon:tui contract", () => {
  test("models Scene as a generic body plus optional native history", () => {
    const body = { kind: "view" } as View;
    const history = undefined as History | undefined;
    const scene: Scene = { body, history };
    expect(scene.body).toBe(body);
    expect(scene.history).toBeUndefined();
  });

  test("keeps native state behind handles and callbacks explicit", () => {
    const input = undefined as TextInput | undefined;
    const adapter: ComponentAdapter = {
      view: () => ({ kind: "view" } as View),
    };
    expect(input).toBeUndefined();
    expect(adapter.view).toBeFunction();
  });
});
