import { describe, expect, test } from "bun:test";

import {
  DiffHunk,
  DiffLine,
  DiffRange,
  MarkdownProjector,
  ProjectionBuilder,
  TextContent,
  Theme,
  StyleSpec,
  View,
} from "../src/index.ts";

describe("T5 semantic text pipeline", () => {
  test("preserves origins through projection and rewrite", () => {
    const content = TextContent.markdown("**hello**").withOrigin({ format: "markdown", source: "fixture" });
    const projection = new MarkdownProjector().project(content);
    expect(projection.text()).toBe("hello");
    expect(content.rewrite((text) => text.toUpperCase()).origin.source).toBe("fixture");
  });

  test("validates source spans and renders diffs as Views", () => {
    const content = TextContent.plain("abc");
    const projection = new ProjectionBuilder(content).span(0, 3, "abc").finish();
    expect(projection.sourceRange()).toEqual({ start: 0, end: 3 });
    const hunk = new DiffHunk(new DiffRange(1, 0), new DiffRange(1, 1), [new DiffLine("addition", "new")]);
    expect(hunk.render()).toBeInstanceOf(View);
    expect(() => new ProjectionBuilder(content).span(2, 1, "bad").finish()).toThrow(/projection/);
  });

  test("theme selection remains a semantic style lookup", () => {
    const theme = Theme.new().withStyle("emphasis", new StyleSpec().bold());
    expect(theme.style("emphasis").value.attributes.bold).toBe(true);
  });
});
