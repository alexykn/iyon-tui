import { Style, TextSelector, Theme } from "@iyon/tui";
import type { StyleSpec } from "@iyon/tui";
import type { Theme as RuntimeTheme } from "@iyon/tui";

export type IyonTheme = RuntimeTheme & {
  readonly composer: StyleSpec;
  readonly footer: StyleSpec;
  readonly muted: StyleSpec;
  readonly inputBorder: "theme:input.border";
  readonly mutedColor: "theme:text.muted";
  readonly toolFinishedColor: "theme:tool.finished";
};

export function createIyonTheme(): IyonTheme {
  const theme = Theme.new()
    .withColor("surface.user", "#2d3748")
    .withColor("text.muted", "#718096")
    .withColor("surface.default", "#718096")
    .withColor("tool.running", "#a0aec0")
    .withColor("tool.finished", "#68d391")
    .withColor("tool.error", "red")
    .withColor("text.error", "red")
    .withColor("text.warning", "yellow")
    .withColor("text.heading", "#ffc457")
    .withColor("text.code", "#78c8d2")
    .withColor("diff.addition", "#68d391")
    .withColor("diff.deletion", "red")
    .withColor("diff.header", "#ffc457")
    .withColor("diff.context", "#718096")
    .withColor("diff.meta", "#718096")
    .withColor("truncation_footer", "#787a84")
    .withColor("input.border", "#add8e6")
    .withColorVariant("input.border", { states: { "iyon.agent.effort": "low" } }, "green")
    .withColorVariant("input.border", { states: { "iyon.agent.effort": "medium" } }, "yellow")
    .withColorVariant("input.border", { states: { "iyon.agent.effort": "high" } }, "magenta")
    .withColorVariant("input.border", { focused: true, states: { "iyon.agent.effort": "low" } }, "lightgreen")
    .withColorVariant("input.border", { focused: true, states: { "iyon.agent.effort": "high" } }, "lightmagenta")
    .withStyle("tool.running", Style.new().foreground("theme:tool.running"))
    .withStyle("tool.finished", Style.new().foreground("theme:tool.finished"))
    .withStyle("tool.error", Style.new().foreground("theme:tool.error"))
    .withStyle("text.muted", Style.new().foreground("theme:text.muted"))
    .withStyle("text.error", Style.new().foreground("theme:text.error"))
    .withStyle("text.warning", Style.new().foreground("theme:text.warning"))
    .withStyle("diff.addition", Style.new().foreground("theme:diff.addition"))
    .withStyle("diff.deletion", Style.new().foreground("theme:diff.deletion"))
    .withStyle("diff.header", Style.new().foreground("theme:diff.header"))
    .withStyle("diff.context", Style.new().foreground("theme:diff.context"))
    .withStyle("diff.meta", Style.new().foreground("theme:diff.meta"))
    .withTextStyle(TextSelector.heading(), Style.new().foreground("theme:text.heading"))
    .withTextStyle(TextSelector.inlineCode(), Style.new().foreground("theme:text.code"))
    .withTextStyle(TextSelector.codeBlock(), Style.new().foreground("theme:text.code"))
    .withTextStyle(TextSelector.part("codeLabel"), Style.new().foreground("theme:text.muted").dim())
    .withTextStyle(TextSelector.part("quoteMarker"), Style.new().foreground("theme:text.muted"))
    .withTextStyle(TextSelector.part("listMarker"), Style.new().foreground("theme:text.muted"))
    .withTextStyle(TextSelector.part("taskMarker"), Style.new().foreground("theme:text.muted"))
    .withTextStyle(TextSelector.part("thematicRule"), Style.new().foreground("theme:text.muted"))
    .withTextStyle(TextSelector.annotation("app", "thinking"), Style.new().foreground("theme:text.muted").italic()) as unknown as IyonTheme;

  return Object.assign(theme, {
    composer: Style.new(),
    footer: Style.new().dim(),
    muted: Style.new().dim().foreground("theme:text.muted"),
    inputBorder: "theme:input.border" as const,
    mutedColor: "theme:text.muted" as const,
    toolFinishedColor: "theme:tool.finished" as const,
  });
}
