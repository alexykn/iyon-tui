import { describe, expect, test } from "bun:test";
import { Scene, View } from "@iyon/tui";
import { SceneExtensions } from "../src/scene-extensions.ts";

describe("Scene extensions", () => {
  test("composes in order and supports full replacement", async () => {
    const extensions = new SceneExtensions();
    const body = View.text("base");
    extensions.compose({ id: "one", order: 2, compose: (scene) => new Scene(scene.body as never, scene.history as never) });
    extensions.compose({ id: "two", order: 1, compose: (scene) => new Scene(scene.body as never, scene.history as never) });
    extensions.replace({ id: "three", order: 3, replace: () => new Scene(View.text("replacement")) });
    const result = await extensions.apply(new Scene(body));
    expect(result.body).toBeDefined();
    expect((result.body as { readonly value?: string }).value).toBeUndefined();
  });
});
