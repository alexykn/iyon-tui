# AGENTS.md

## Defaults
- IMPORTANT: Be brief and concise in your replies.
- Preserve existing behavior unless the task explicitly requires a change.
- Keep changes tightly scoped and aligned with the existing architecture.
- Do not modify unrelated files.
- Do not edit `AGENTS.md` unless explicitly asked.

## Ask First
- Adding or changing third-party dependencies.
- Changes to public APIs.
- Changes spanning multiple files.

## Implementation
- Prefer the simplest solution that fits the existing design.
- Avoid unnecessary refactors, abstractions, or formatting-only changes.
- Do not silently swallow exceptions.

## Control Flow & Structure
- Prefer flat control flow. Avoid unnecessary nesting; if logic gets too deep, extract a helper or return early.
- Keep behavior local unless clearly visible friction points or boundaries show up.
- Use sensible function and module boundaries so related logic stays together, do not create oversized functions or files.

## Verification
- Run the smallest relevant checks for the files you changed before finishing.
- Run broader test/lint suites only if explicitly requested or if the change clearly warrants it.
- Do not add new tests unless explicitly asked.
- Run `bun run check:ownership` before completing any change that touches the TUI framework boundary or its public surface (see ARCHITECTURE.md).

## Python
- Use `uv` for environment and dependency management.
- Work inside `.venv`; create it with `uv venv` if missing.
- Sync dependencies with `uv sync`.
- Use `pyproject.toml` for configuration.
- Lint/fix with `uv run ruff check --fix`.
- Format with `uv run ruff format`.
- Type-check with `uv run ty check`.
- Use `pytest` for tests.
- Prefer Python 3.12+ features and native type annotations.
- Prefer `@dataclass` or Pydantic for structured data over long parameter lists.
- Prefer `ABC` over `Protocol` unless structural typing is specifically needed.

## Mandatory TUI Framework Boundary

`iyon-tui` and its TypeScript facade (`@iyon/runtime/tui` / `iyon:tui`) are a **generic TUI framework**. They are not part of the Iyon agent harness product and must not contain Iyon application concepts. This boundary applies to Rust, N-API bindings, runtime TypeScript, public types, tests, examples, and documentation.

### Framework ownership

The framework supplies reusable terminal mechanics and semantic presentation primitives:

- terminal sessions, terminal backends, headless rendering, terminal writes, resize, clock/tick scheduling, and deterministic test harnesses;
- native keyboard, paste, focus, modal, component, and interaction routing; keystrokes remain in `iyon-tui` and are not sent to TypeScript merely to reimplement native input handling;
- generic routed outputs, typed `Output<T>` channels, route registration, paste interception, and component handles;
- the generic `App`, `AppCx`, `AppHandle`, timers, lifecycle, error, update, and view kernel;
- owned semantic `View` trees and `IntoView`, including text, styled text, rows, columns, grids, hanging indents, spacers, containers, clamps, borders, padding, alignment, wrapping, sizing, overflow indicators, and component references;
- retained `Component` values, `ComponentCx`, `ComponentHandle`, capabilities, focus, key commands, paste handlers, layout notifications, and tick callbacks;
- `Scene` roots and semantic-to-physical resolution; layout, measurement, placement, wrapping, painting, cell styles, Unicode width/cell addressing, and terminal correctness;
- `History` ordered scrollback, `HistoryLayout`, `HistoryUnitId`, frozen units, live stream units, flow boundaries, tail promotion, viewport anchoring, compaction, and native scrollback transfer;
- `StreamingSource`, `TextStream`, `StreamPane`, `StreamSnapshot`, source-rooted `StreamOffset`/`StreamRange`/`StreamRevision`, snapshot validation, stream sealing, and incremental stream compilation;
- `Projection`, `ProjectionBuilder`, `Projector`, `ProjectorExt`, `Then`, `Smooth`, `SmoothConfig`, stable frontiers, restart/compaction coordinates, incremental projection, and temporal smoothing/pacing;
- semantic text IR: raw text, paragraphs, headings, lists, list items, tables, code blocks, quotes, thematic breaks, inline content, breaks, images, links, marks, text runs, provenance, origins, annotations, semantic tags, semantic keys, and semantic values;
- generic text visitors, text rewriters, rewrite projectors, traversal helpers, projection validation, and source/provenance preservation;
- `MarkdownProjector`, `MarkdownOptions`, CommonMark/GFM features, live table stabilization, `PlainTextProjector`, `TextRenderer`, `TextRenderPolicy`, soft-break policy, table sizing, task-list markers, code labels/wrapping, roles, parts, and `TextSelector` rules;
- generic `Renderer` implementations and extension points that transform semantic values into `View` trees;
- diff values and rendering: ranges, hunks, line kinds, line numbers, offsets, terminations, validation, and `DiffRenderer`;
- `Theme`, theme colors, `ColorSpec`, `StyleSpec`, `StyleRef`, selectors, style states, attributes, borders, glyphs, insets, and style resolution. Theme/style data must not encode Iyon product policy;
- generic controls such as `TextInput` and `ScrollPane`;
- generic retained slots/animations: a `ViewSlot` may receive an array of caller-supplied `View` frames and a tick interval, while Rust owns the stable slot identity, scheduling, frame selection, invalidation, and rendering. TypeScript supplies semantic frames only when application state changes; it must not drive every tick over N-API;
- the TypeScript semantic facade for all of the above, including lazy `View` construction/materialization, native handles, `History`, `TextStream`, projections/projectors, annotations, styles/themes, slots, scroll panes, text input, scenes, runtime lifecycle, routed output events, and headless inspection.

Generic Markdown, smoothing, annotation, stream, history, animation, and diff facilities are allowed because they operate on caller-supplied values and have no Iyon-specific meaning. A generic annotation may be namespaced by its caller; the framework must not hard-code a product annotation such as assistant thinking.

### Explicit prohibition

No `iyon-tui` or `@iyon/runtime/tui` code may contain or expose concepts whose meaning is specific to the Iyon agent/application, including:

- agent, assistant, model, provider, prompt, response, transcript, conversation, turn, reasoning effort, or Iyon session policy;
- tool names, tool calls, tool arguments, tool results, tool approval policy, tool cards, tool registries, or tool lifecycle presentation;
- steering/follow-up queues, queued messages, queue previews, or any agent queue semantics;
- `working`/`waiting` product status, `ConversationActivity`, assistant pipelines, assistant streams, thinking-vs-text segment kinds, assistant labels, spinner labels, or product spinner choreography;
- Iyon-specific Markdown policies, renderer defaults, viewport gutters, composer defaults, footer text, goodbye behavior, or product theme keys;
- provider/model metadata, agent lifecycle, backend event reduction, tool execution, approval decisions, transcript mutation, or application state;
- an application-specific action enum, action reducer, fixed Iyon scene, fixed Iyon layout, or product-specific default labels;
- hidden retention of application state or product policy behind generic names, `Arc`, N-API handles, callbacks, or JSON payloads.

Names such as `route`, `output`, `stream`, `history`, `activity`, `animation`, and `queue` are only valid when their semantics are genuinely generic and caller-defined. A generic route may carry an opaque caller route ID; the framework must not interpret that ID as an Iyon action.

### Correct placement of Iyon behavior

Iyon application behavior belongs in TypeScript plugin packages, principally `plugins/app/iyon` and its agent/tool/provider contributions. The plugin owns application state, event reduction, provider/backend mapping, assistant/thinking composition, steering state, working-spinner labels and choreography, tool cards and approvals, Iyon theme choices, composer policy, footer, and scene construction. It uses generic TUI handles and native primitives rather than adding product behavior to the framework.

The planned package split in `bun_refactor.md` is also mandatory: `iyon-api` is the protocol/schema surface; `iyon-core` is the native generic kernel for sessions, transcript/message state, model-turn normalization, tool-execution lifecycle, approvals, queues, cancellation, and event delivery; `iyon-tui` is the generic terminal UI framework; and `iyon`/the TypeScript plugins are product/application registrations and orchestration. `iyon-core` must not become a hidden Iyon agent loop or provider/tool registry, and `iyon-api` must remain protocol types rather than application policy. The default Iyon agent, providers, tools, and app are TypeScript contributions consuming those generic `iyon-api`, `iyon-core`, and `iyon-tui` capabilities.

Rust may retain terminal correctness, native input routing, generic component interaction, History/scrollback mechanics, projection/rendering, and performance-critical animation ticking. Rust must not decide what an agent, assistant, tool, queue, or product status means. TypeScript must not reimplement native keystroke interpretation or per-tick terminal rendering.

### Boundary review rule

Before adding or moving any TUI API, ask whether a non-agent terminal application (for example a log tailer, build watcher, editor, dashboard, or arbitrary plugin) could use it without pretending to be Iyon. If not, keep it out of `iyon-tui` and the generic facade. Any intentional public API change must preserve generic Rust/TypeScript parity, stable handle identity, source-rooted stream coordinates, native scrollback correctness, and the existing Iyon UI/UX through plugin composition.
