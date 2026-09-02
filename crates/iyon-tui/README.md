# iyon-tui

A streaming-first terminal UI framework for Rust.

`iyon-tui` is purpose-built for applications that consume incrementally-produced, semantically-structured text — log tailers, build watchers, and live data pipelines. It is not a general-purpose widget library.

- **Streaming-first.** Append text, compile incrementally, render without re-parsing the full buffer.
- **Semantic text IR.** Headings, lists, tables, strikethrough, code blocks — not just styled strings.
- **Source-mapped.** Every span tracks its origin offset. Projects are incremental and cacheable.
- **Format-agnostic renderer.** Same `TextRenderer` works for Markdown, plain text, and custom projectors.
- **Composable `View` tree.** Text, rows, columns, grids, hanging indents, containers, components.

---

## Quick start

```rust
use iyon_tui::{App, AppCx, History, View, TextInput};

App::new(
    |cx| {
        let input = cx.register(TextInput::new().multiline(true));
        Ok(input)
    },
    |_input, _action, _cx| Ok(()),
    |input| View::component(*input).fill(),
)
.with_history(History::new())
.run()?;
```

---

## Core concepts

### View

`View` is an owned, backend-neutral composition tree. Everything is a `View`.

```rust
use iyon_tui::{View, IntoView, ColorSpec, Insets, StyleSpec, TextSpan, BorderSpec, TextAttribute};

// Text with typed API (preserves text methods until conversion)
View::text("hello").bold().no_wrap().fill_width();

// Styled spans
View::styled_text([
    TextSpan::plain("plain "),
    TextSpan::styled("bold", StyleSpec::new().bold()),
]);

// Horizontal row
View::horizontal(|row| {
    row.child("● ");
    row.flex(View::text("command").fill_width());
    row.gap(1);
});

// Vertical column
View::vertical(|col| {
    col.child("first");
    col.child("second");
    col.gap(1);
});

// Hanging indent (prefix once, continuation on wrapped rows)
View::hanging("● ", "  ", "long command body");

// Grid
View::grid(|grid| {
    grid.row(|row| {
        row.cell("Name");
        row.cell("Value");
    });
});

// Spacer
View::spacer(1);

// Container (structural boundary)
View::text("inner").container().padding(Insets::all(1));

// Clamp rows
View::text("long text").clamp_rows(3, OverflowIndicator::Ellipsis);

// Sizing
View::text("fill").fill_width();
View::text("fit").fit_width();   // default
View::text("grow").fill_height();
View::text("bounded").min_width(10).max_width(80);

// Styling chain
View::text("styled")
    .foreground(ColorSpec::theme("text.warning"))
    .bold()
    .italic()
    .underline()
    .strikethrough()
    .text_attribute(TextAttribute::Dim, true)
    .padding(Insets::all(1))
    .background(ColorSpec::ansi(237))
    .border(BorderSpec::rounded().color(ColorSpec::ansi(4)))
    .fill_width();
```

`Text` is a typed wrapper that stays `Text` until you call `.container()` or `.into_view()`:

```rust
let text: View = View::text("hello").no_wrap().bold().into_view();
let container: View = View::text("hello").container();
```

Everything implements `IntoView`: `View`, `Text`, `String`, `&str`, and your own types.

**View construction is immediate** — closures run once and the result is owned data. There is no retained builder, no deferred layout.

---

### Component

`Component` is the trait for retained-state widgets. Components own mutable state, receive typed outputs, and produce a `View` on demand.

```rust
use iyon_tui::{Component, ComponentCx, View, Key, Output};

struct Counter(u64);

impl Component for Counter {
    fn view(&self) -> View {
        View::text(format!("Count: {}", self.0))
    }

    fn capabilities(&self, cx: &mut ComponentCx<'_, Self>) {
        cx.bind_key(Key::Char('+'), |counter| {
            counter.0 += 1;
        });
    }
}
```

Register by value, interact by handle:

```rust
let handle: ComponentHandle<Counter> = cx.register(Counter(0));
let count = cx.with_component(handle, |c| c.0);
cx.with_component_mut(handle, |c| c.0 += 1);
```

Components can emit typed outputs that route to the application action enum:

```rust
#[derive(Debug)]
enum Action { Incremented(u64) }

// In capabilities:
cx.output(Output::new(), |counter| Action::Incremented(counter.0));

// In the output event handler:
let action = EventCx::output(Output::new(), Action::Incremented(42));
```

Render a component in the View tree:

```rust
View::component(handle).fill_width();
```

---

### App

Owns application state, framework runtime, and event dispatch.

```rust
use iyon_tui::{App, AppCx, View, History, Theme};

App::new(
    |cx: &mut AppCx<'_, Action>| -> Result<MyState, Error> {
        // init — return initial state, register components
        let component = cx.register(MyComponent::new());
        Ok(MyState { component })
    },
    |state, action: Action, cx: &mut AppCx<'_, Action>| {
        // update — handle actions
        match action {
            Action::Something => { /* ... */ }
        }
        Ok(())
    },
    |state| -> View {
        // view — produce the current view tree
        View::component(state.component).fill()
    },
)
.with_history(History::new())
.with_theme(Theme::new())
.run()?;
```

**`AppCx`** gives you:
- `cx.register(component)` — register a component, get a handle
- `cx.history()` — access the History model
- `cx.history_mut()` — mutable History access
- `cx.bind_key(keystroke, action)` — global keybindings
- `cx.route(output, action)` — route component outputs to actions
- `cx.exit()` — signal graceful shutdown
- `cx.forward_paste(text)` — forward pasted text to a component
- `cx.intercept_paste(component, action)` — capture paste events
- `cx.with_component(handle, fn)` — read component state
- `cx.with_component_mut(handle, fn)` — mutate component state
- `cx.remove_component(handle)` — unregister a component
- `cx.remove_route(output)` — remove a route
- `cx.theme()` / `cx.theme_mut()` — access the theme
- `cx.add_timer(interval, action)` — schedule recurring actions

Async support via `AppHandle`:

```rust
let handle = app.handle();
handle.send(IyonAction::Something)?;              // sync send
handle.send_async(IyonAction::Something).await?;   // async send
```

Error types:

```rust
pub enum RunError<ApplicationError> {
    Application(ApplicationError),
    Runtime(RuntimeError),
}
```

---

### History

Ordered scrollback with static units, live component-backed units, and flow
boundaries. Streaming content is a normal ContentPort/Connector occurrence;
History does not own a second stream runtime.

```rust
use iyon_tui::{FlowBoundary, History, HistoryLayout, Insets, View};

let mut history = History::new();
history.set_layout(HistoryLayout::from_parts(Insets::new(0, 0, 1, 0), 1));
history.push(View::text("hello").fill_width())?;
history.push_with_boundary(View::text("continued"), FlowBoundary::AttachToPrevious)?;
```

---

### Text pipeline

```rust
use iyon_tui::{
    MarkdownOptions, MarkdownProjector, PlainTextProjector,
    TextContent, TextRenderPolicy, TextRenderer,
    TextSelector, TextRole, TextPart, Theme, StyleSpec, ColorSpec,
    Projector, Projection, Renderer, SoftBreakPolicy,
};

// 1. Choose a projector
let mut md = MarkdownProjector::new(MarkdownOptions::gfm());
// or: let plain = PlainTextProjector;

// 2. Project raw text to semantic IR
let input = Projection::from_raw("**bold** and ~~strikethrough~~");
let semantic: Projection<TextContent> = md.project(&input)?;

// 3. Configure the renderer
let renderer = TextRenderer::with_policy(
    TextRenderPolicy::new()
        .with_soft_break(SoftBreakPolicy::LineBreak)
        .with_block_gap(1)
        .with_table_column_sizing(TableColumnSizing::Content)
        .with_table_column_gap(1)
        .with_task_list_marker(TaskListMarkerPolicy::TaskOnly)
        .with_code_block_label(CodeBlockLabelPolicy::Language)
        .with_code_block_gap(0)
        .with_code_wrap(WrapMode::NoWrap),
);

// 4. Render semantic IR to View
for span in semantic.spans() {
    let view = Renderer::render(&renderer, span.values());
    // use the view
}
```

Semantic text IR types:

- `TextContent::Block(Block)` — paragraphs, headings, lists, code blocks, tables, quotes, breaks
- `TextContent::Raw(text)` — raw unparsed text
- `BlockKind::Paragraph`, `Heading`, `BlockQuote`, `List`, `CodeBlock`, `Table`, `ThematicBreak`, `RawBlock`
- `InlineKind::Text`, `Break`, `Image`, `RawInline`
- `Mark::Emphasis`, `Strong`, `Strikethrough`, `Underline`, `Superscript`, `Subscript`, `SmallCaps`, `Code`, `Link`

Theme with `TextSelector`:

```rust
let theme = Theme::new()
    .with_text_style(
        TextSelector::heading().level(1),
        StyleSpec::new().bold().foreground(ColorSpec::theme("heading")),
    )
    .with_text_style(
        TextSelector::role(TextRole::Strikethrough),
        StyleSpec::new().strikethrough(),
    )
    .with_text_style(
        TextSelector::role(TextRole::InlineCode),
        StyleSpec::new().foreground(ColorSpec::ansi(2)),
    )
    .with_text_style(
        TextSelector::part(TextPart::CodeLabel),
        StyleSpec::new().dim(),
    );
```

---

### Markdown support

With `MarkdownOptions::gfm()`:

| Feature | Example | Rendered |
|---------|---------|----------|
| Heading | `# H1` – `###### H6` | `TextRole::Heading` |
| Bold | `**text**` | `Mark::Strong` |
| Italic | `*text*` | `Mark::Emphasis` |
| Strikethrough | `~~text~~` | `Mark::Strikethrough` |
| Inline code | `` `code` `` | `Mark::Code` |
| Link | `[label](url)` | `Mark::Link` |
| Bullet list | `- item` | `BlockKind::List(Bullet)` |
| Ordered list | `1. item` | `BlockKind::List(Ordered)` |
| Task list | `- [x] done` | `ListItem::checked(Some(true))` |
| Code block | ` ```rust ` | `BlockKind::CodeBlock` |
| Table | `\| col \| col \|` | `BlockKind::Table` |
| Block quote | `> quote` | `BlockKind::BlockQuote` |
| Thematic break | `---` | `BlockKind::ThematicBreak` |
| Image | `![alt](url)` | `InlineKind::Image` |
| Soft break | (newline) | `BreakKind::Soft` |
| Hard break | trailing `\` | `BreakKind::Hard` |

`MarkdownOptions::commonmark()` is the default (no extensions). Build custom sets:

```rust
MarkdownOptions::gfm()
    .with_tables(true)
    .with_strikethrough(true)
    .with_task_lists(true)
    .with_live_table_stabilization(true);  // streaming table stabilization
```

`PlainTextProjector` treats all input as paragraphs.

---

### Custom projectors

Implement `Projector<TextContent>` to add new input formats:

```rust
use iyon_tui::{Projector, Projection, TextContent};

struct MyProjector;

impl Projector<TextContent> for MyProjector {
    type Output = TextContent;
    type Error = MyError;

    fn project(&mut self, input: &Projection<TextContent>)
        -> Result<Projection<TextContent>, Self::Error>
    {
        // Transform raw text spans into semantic blocks
    }

    fn restart_from(&self, output_from: StreamOffset) -> StreamOffset {
        // Where to resume re-parsing after compaction
        output_from
    }
}
```

Compose projectors with `Then`:

```rust
use iyon_tui::{Then, ProjectorExt};

let pipeline = MarkdownProjector::new(options)
    .then(MyRewriter::new())
    .then(AnotherPass);
```

Or chain with `Smooth` for temporal pacing:

```rust
use iyon_tui::{Smooth, SmoothConfig, Projector};

let mut pipeline = Smooth::new(SmoothConfig::default())
    .then(markdown)
    .then(rewriter);
```

---

### Text rewriting / annotation

Walk and transform the text IR:

```rust
use iyon_tui::{
    TextRewriter, RewriteProjector, TextVisitor,
    Inline, InlineKind, InlineContent, Block, TextRun,
    walk_content, walk_rewrite_inline, walk_rewrite_content,
};

// Visitor — read-only traversal
struct MyVisitor;
impl TextVisitor for MyVisitor {
    fn visit_inline(&mut self, inline: &Inline) {
        // inspect inlines
    }
    fn visit_block(&mut self, block: &Block) {
        // inspect blocks
    }
}
walk_content(&mut MyVisitor, &text_content);

// Rewriter — transform IR
struct MyRewriter;
impl TextRewriter for MyRewriter {
    type Error = std::convert::Infallible;

    fn rewrite_inline(&mut self, inline: Inline) -> Result<Inline, Self::Error> {
        // Modify or replace inlines
        walk_rewrite_inline(self, inline)
    }
    fn rewrite_inline_content(&mut self, content: InlineContent) -> Result<InlineContent, Self::Error> {
        walk_rewrite_content(self, content)
    }
}

// Convert a rewriter to a projector for use in pipelines
let projector = MyRewriter.into_projector();
```


### TextInput control

```rust
use iyon_tui::{TextInput, TextChange};

// Create
let input = TextInput::new()
    .multiline(true)
    .single_line(false)
    .placeholder("Type something...")
    .border(BorderSpec::plain().edges(BorderEdges::TOP_BOTTOM));

// Read state
let text = cx.with_component(handle, |input| input.text().to_owned());
let is_empty = cx.with_component(handle, |input| input.is_empty());
let cursor = cx.with_component(handle, |input| input.cursor());

// Mutation
cx.with_component_mut(handle, |input| input.clear());

// Output (text changes)
cx.route(
    cx.with_component(handle, TextInput::submitted)?,
    Action::Submit,
);
```

---

### Diff rendering

```rust
use iyon_tui::{
    DiffRenderer, DiffHunk, DiffLine, DiffLineKind,
    DiffLineNumber, DiffLineOffset, DiffRange,
};

let renderer = DiffRenderer::default();
let hunks = [
    DiffHunk::new(
        DiffRange::new(DiffLineOffset::new(0), 2).unwrap(),
        DiffRange::new(DiffLineOffset::new(0), 2).unwrap(),
        [
            DiffLine::context(
                DiffLineNumber::new(1).unwrap(),
                DiffLineNumber::new(1).unwrap(),
                "  same line",
            ),
            DiffLine::deletion(
                DiffLineNumber::new(2).unwrap(),
                "-removed line",
            ),
            DiffLine::addition(
                DiffLineNumber::new(2).unwrap(),
                "+added line",
            ),
        ],
    ).unwrap(),
];
let view: View = renderer.render(&hunks);
```

---

### Renderer trait

Converts a semantic value into a `View`. Renderers do not receive terminal geometry, parser state, clocks, or stream lifecycle.

```rust
use iyon_tui::{Renderer, View, TextContent, TextRenderer};

let renderer = TextRenderer::new();
let view: View = Renderer::render(&renderer, &text_content);
```

`TextRenderer` implements `Renderer<[TextContent]>`. Custom renderers implement the trait for their own input types.

### Scene

Terminal semantic root. Resolves a `View` tree against a theme and terminal size into physical rows. Applications typically do not interact with `Scene` directly — the `App` framework manages it.

```rust
use iyon_tui::{Scene, View, Theme};

let scene = Scene::new(terminal_size, theme, view);
scene.render();
```

---

### ScrollPane

A scrollable container for a single child view.

```rust
use iyon_tui::ScrollPane;

let pane = ScrollPane::new(View::text("scrollable content"));
pane.set_content(updated_view);
View::component(pane).fill();
```

---

### Projection

Root-coordinate projection algebra for incremental, source-mapped transformations.

```rust
use iyon_tui::{Projection, ProjectionBuilder, ProjectionSpan, StreamOffset, StreamRange};

// Build a projection
let input = ProjectionBuilder::new(
    StreamOffset::ZERO,  // source_base
    StreamOffset::ZERO,  // stable_through
    offset(100),         // source_end
    false,               // sealed
)
.emit(StreamRange::new(offset(0), offset(5)), "hello")
.emit(StreamRange::new(offset(5), offset(11)), " world")
.finish()?;

// Read
assert_eq!(input.source_base(), StreamOffset::ZERO);
assert_eq!(input.source_end(), offset(11));
for span in input.spans() {
    let range: StreamRange = span.source();
    let values: &[&str] = span.values();
}

// Transform
let mapped = input.map_ref(|s| s.to_uppercase());
let owned = input.map(|s| s.len());
```

`ProjectionSpan` is the unit of stability. Spans before `stable_through` are considered finalized.

---

### Projector trait

```rust
use iyon_tui::{Projector, Projection, StreamOffset};

struct MyProjector;

impl<T> Projector<T> for MyProjector {
    type Output = T;
    type Error = MyError;

    fn project(&mut self, input: &Projection<T>)
        -> Result<Projection<Self::Output>, Self::Error>
    { /* ... */ }

    fn restart_from(&self, output_from: StreamOffset) -> StreamOffset
    { output_from }
}
```

`ProjectorExt` adds `.then()` for composing projectors in sequence.

---

### Smooth (temporal pacing)

Controls how fast streamed units progress through the pipeline, independent of arrival rate. Useful when input arrives in bursts but should appear at a human-readable pace.

```rust
use iyon_tui::{Smooth, SmoothConfig};

let mut smooth = Smooth::new(SmoothConfig::default());
let paced = smooth.project(&bursty_input)?;

// Advance time
let changed = smooth.advance(now);  // returns true if more tokens are ready

// Check when to wake
let next = smooth.next_wakeup();
```

---

### Theme

```rust
use iyon_tui::{Theme, ColorSpec, StyleSpec, TextSelector, TextRole, TextPart};

let theme = Theme::new()
    .with_text_style(
        TextSelector::role(TextRole::Heading).level(1),
        StyleSpec::new().bold().foreground(ColorSpec::theme("heading")),
    )
    .with_text_style(
        TextSelector::role(TextRole::Link),
        StyleSpec::new().underline().foreground(ColorSpec::ansi(4)),
    )
    .with_text_style(
        TextSelector::role(TextRole::Strikethrough),
        StyleSpec::new().strikethrough(),
    )
    .with_text_style(
        TextSelector::part(TextPart::CodeLabel),
        StyleSpec::new().dim().foreground(ColorSpec::theme("text.muted")),
    );
```

---

### Testing utilities (feature: `test-util`)

```rust
#[cfg(feature = "test-util")]
use iyon_tui::testing::{
    start, AppHarness, compile_view_lines, cell_x_of_text,
    style_at_text, PaintedFlags,
};

// Run an App in a headless terminal
let mut harness: AppHarness<_, _, _, _, _> = start(app, 80, 20)?;
harness.step()?;
let lines: Vec<String> = harness.screen_lines();
assert!(lines[0].contains("expected"));

// Compile a View directly
let lines = compile_view_lines(&view, 80);
assert_eq!(lines[0], "Hello");

// Find pixel positions
let x = cell_x_of_text(&view, 80, "needle");
let flags = style_at_text(&view, 80, &theme, "needle");
assert!(flags.bold);
Input simulation:
harness.handle().send(Action::Something)?;
harness.step()?;
```

---

### Prelude

```rust
use iyon_tui::prelude::*;

// Includes: App, AppCx, Block, Component, ComponentCx,
// DiffHunk, DiffLine, DiffLineKind, DiffLineNumber,
// DiffLineOffset, DiffRange, DiffRenderer, EventCx,
// History, HistoryLayout, Inline, InlineContent,
// IntoView, MarkdownProjector, Output, PlainTextProjector,
// Projection, Projector, ProjectorExt, Renderer, Scene,
// ScrollPane, Smooth, TextContent, TextInput,
// TextOrigin, TextRenderPolicy, TextRenderer, TextSelector,
// Theme, View
```

---

### All public types

**Application** — `App`, `AppCx`, `AppHandle`, `AppClosed`, `AppSendError`, `RunError`, `RuntimeError`, `TimerHandle`

**Component** — `Component` trait, `ComponentCx`, `ComponentHandle<C>`

**View / composition** — `View`, `IntoView`, `Text`, `TextSpan`, `Horizontal`, `Vertical`, `Grid`, `GridCellSpec`, `GridRow`, `GridTrack`, `HorizontalAlign`, `WrapMode`, `VerticalAlign`, `OverflowIndicator`

**Styling** — `StyleSpec`, `StyleRef`, `StyleSelector`, `StyleStateKey`, `StyleStateValue`, `ColorSpec`, `AnsiColor`, `ThemeColor`, `ThemeKey`, `TextAttribute`, `TextAttributeSpec`, `Insets`, `BorderSpec`, `BorderEdges`, `BorderGlyphs`, `BorderGlyphError`, `BorderStyle`

**Semantic text** — `TextContent`, `RawText`, `Block`, `BlockKind`, `HeadingLevel`, `CodeBlock`, `List`, `ListItem`, `ListMarker`, `NumberDelimiter`, `NumberStyle`, `Alignment`, `Table`, `TableColumn`, `TableRow`, `TableCell`, `Inline`, `InlineContent`, `InlineKind`, `TextRun`, `TextProvenance`, `LiteralText`, `Mark`, `MarkSet`, `BreakKind`, `FormatId`, `Image`, `LanguageId`, `LinkTarget`, `Annotations`, `SemanticKey`, `SemanticTag`, `SemanticValue`, `TextOrigin`

**Text processing** — `TextIrError`, `TextProjectionError`, `validate_text_content`, `validate_text_projection`, `TextVisitor` trait, `TextRewriter` trait, `RewriteProjector`, `RewriteProjectionError`, `walk_block`, `walk_content`, `walk_inline`, `walk_inline_content`, `walk_literal`, `walk_rewrite_block`, `walk_rewrite_blocks`, `walk_rewrite_content`, `walk_rewrite_inline`, `walk_rewrite_inline_content`, `walk_rewrite_literal`

**Text render** — `TextRenderer`, `TextRenderPolicy`, `SoftBreakPolicy`, `TableColumnSizing`, `TaskListMarkerPolicy`, `CodeBlockLabelPolicy`, `TextRole`, `TextPart`, `TextSelector`, `TextListKind`, `TextTaskState`, `TextTableSection`, `Renderer` trait

**Projectors** — `MarkdownProjector`, `MarkdownOptions`, `MarkdownProjectionError`, `PlainTextProjector`

**Projection** — `Projection<T>`, `ProjectionBuilder<T>`, `ProjectionSpan<T>`, `Projector` trait, `ProjectorExt`, `Then<A, B>`, `ThenError<A, B>`, `Smooth`, `SmoothConfig`, `SmoothConfigError`, `ProjectionRelationError`, `ProjectionTransitionError`, `ProjectionValidationError`, `validate_projection_relation`, `validate_projection_transition`

**Coordinates** — `stream::StreamOffset`, `stream::StreamRange`

**History** — `History`, `HistoryLayout`, `HistoryUnitId`, `HistoryError`, `FlowBoundary`

**Controls** — `TextInput`, `TextChange`

**Diff** — `DiffRenderer`, `DiffHunk`, `DiffLine`, `DiffLineKind`, `DiffLineNumber`, `DiffLineOffset`, `DiffLineTermination`, `DiffRange`, `DiffValidationError`

**Interaction** — `Key`, `KeyStroke`, `MediaKey`, `ModifierKey`, `Modifiers`, `InteractionResult`

**Output** — `Output<T>`, `OutputRouter<A>`, `EventCx`, `RouteConflict`

**Theme** — `Theme`

**Scene** — `Scene`

**Scroll** — `ScrollPane`

---

## Status

Pre-release. The public API is under active development. Breaking changes expected until 1.0.