# 08 — Semantic Content

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `iyon-tui`
- Branch: `main`
- Source revision: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Primary scope: `crates/iyon-tui/src/content/` recursively
- Supporting seam inspection:
  - `crates/iyon-tui/src/text.rs`
  - `crates/iyon-tui/src/lib.rs`
  - `crates/iyon-tui/src/application/content.rs`
  - `crates/iyon-tui/src/binding/mod.rs`
  - selected native and TypeScript ingress files for reverse-edge confirmation

I read `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`, `docs/architecture/atlas-4355c02/README.md`, `AGENTS.md`, and the semantic-content portions of `PRE-V5-ARCHITECTURE-REPORT.md` before drawing conclusions. The historical report was used as context and as an evidence checklist; this report describes current source and does not make V5 migration/disposition decisions.

### Scope boundaries

This report covers:

- Semantic text IR:
  - root raw text
  - blocks
  - inline values
  - marks
  - code/raw portals
  - images and links
  - tables, lists, quotes, headings, breaks, thematic breaks
- Semantic annotations and selectors
- Provenance and source witnesses
- Markdown, plain-text, ANSI, and text-diff projectors
- Text visitors and persistent rewriters
- Semantic text renderer and renderer policies
- Structured diff model and direct diff renderer
- Incremental parsing, restart/stability behavior, cache behavior, and failure semantics
- Current application/content seam that owns parser and renderer lifetime

It does not attempt to census the entire presentation, History, stream, native, or TypeScript subsystems. Those are referenced only where necessary to prove a content boundary or consumer.

### Evidence categories

- **Current source fact:** directly observed in the inspected Rust source.
- **Static inference:** behavior inferred from call chains and data structures without executing tests.
- **Historical/contextual:** derived from architecture documentation and labeled as such.
- **Observed execution:** none in this investigation. No build, test, benchmark, or running service was executed.

### High-level finding

The repository contains a substantial, generic semantic text system already:

```text
raw Source bytes
    ↓
Projection<TextContent>
    ↓
MarkdownProjector / PlainTextProjector / DiffProjector / AnsiProjector
    ↓
immutable TextContent IR
    ↓
TextRenderer
    ↓
geometry-independent View tree + semantic StyleFacts
    ↓
ViewCompiler / terminal layout / paint
```

The strongest architectural seam is between semantic projection and width-dependent rendering. The weakest seam is that the semantic `TextRun` structure contains an optional `crate::StyleRef`, and the renderer is implemented directly in terms of `View`, `TextSpan`, `StyleFacts`, and presentation factories. Thus the IR is largely backend-neutral but not completely independent of the current presentation API.

A second consequential finding is that there are two distinct diff implementations:

1. `content/text/diff.rs`: unified-diff source text parsed into ordinary semantic paragraph/inlines with tags and styles.
2. `content/diff/model.rs` plus `content/diff/render.rs`: validated structured hunks lowered directly to a `View`.

They share visual theme keys but do not share one semantic diff IR.

---

## 1. Responsibility and structure

### 1.1 Module inventory

| Path | Approximate physical size | Public surface | Primary responsibility | Plane | Hot path |
|---|---:|---|---|---|---|
| `crates/iyon-tui/src/content/mod.rs` | 10 lines | Internal module | Groups semantic text and structured diff modules | Content | No |
| `crates/iyon-tui/src/content/render.rs` | 9 lines | Test-only trait | Minimal test renderer abstraction from semantic values to `View` | Mixed/test | No |
| `crates/iyon-tui/src/content/diff/mod.rs` | 12 lines | Re-exports via binding | Structured diff model and native-host renderer | Content/mixed | Conditional |
| `crates/iyon-tui/src/content/diff/model.rs` | ~395 production, ~163 tests | Types re-exported through `binding` | Ranges, line coordinates, line kinds, termination, hunk validation | Content | Conditional |
| `crates/iyon-tui/src/content/diff/render.rs` | ~88 production, ~190 tests | `lower_diff_hunks` native-host seam | Direct structured diff-to-`View` lowering | Mixed | Conditional/native |
| `crates/iyon-tui/src/content/text/mod.rs` | 53 lines | Internal module; selected binding re-exports | Text IR module composition and exports | Content | No |
| `crates/iyon-tui/src/content/text/content.rs` | 124 lines | Internal/public-within-crate types | `RawText` and `TextContent` roots | Content | Yes |
| `crates/iyon-tui/src/content/text/annotations.rs` | 176 lines | Types reachable through internal text API and binding-selected types | Canonical tags, keys, values, and annotation sets | Content | Yes |
| `crates/iyon-tui/src/content/text/provenance.rs` | 221 lines | Internal/public-within-crate types | `TextRun`, `LiteralText`, exact/derived/synthetic provenance | Content | Yes |
| `crates/iyon-tui/src/content/text/origin.rs` | ~289 production, ~90 tests | `TextOrigin` selected by binding | Projector origin metadata and recursive origin stamping | Content | Yes |
| `crates/iyon-tui/src/content/text/inline.rs` | 389 lines | Internal/public-within-crate types | Inline kinds, marks, links, images, inline collections | Content | Yes |
| `crates/iyon-tui/src/content/text/block.rs` | ~582 production, ~100 tests | Internal/public-within-crate types | Block kinds, lists, tables, cells, code blocks, structural validation | Content | Yes |
| `crates/iyon-tui/src/content/text/errors.rs` | 167 lines | Internal/public-within-crate error types | IR, projection, naming, provenance, table, and source errors | Content | Conditional |
| `crates/iyon-tui/src/content/text/source.rs` | ~408 production, ~52 tests | Crate-private | Raw projection domains, source witnesses, lazy multi-page assembly | Content | Yes |
| `crates/iyon-tui/src/content/text/validate.rs` | 153 lines | Validation functions re-exported internally | Projection and nested provenance validation | Content | Yes at projector seams |
| `crates/iyon-tui/src/content/text/visit.rs` | 365 lines | Visitor and rewriter APIs | Recursive traversal, persistent rewriting, projection adapter | Content | Conditional |
| `crates/iyon-tui/src/content/text/plain.rs` | 143 lines | `PlainTextProjector` | Claims raw domains as literal paragraphs with hard breaks | Content | Conditional |
| `crates/iyon-tui/src/content/text/markdown_options.rs` | 112 lines | `MarkdownOptions` | CommonMark/GFM and live-table policy | Content | No |
| `crates/iyon-tui/src/content/text/markdown.rs` | 1,433 lines | `MarkdownProjector` and error type | Pulldown CommonMark/GFM parsing, incremental restart and caching | Content | Yes/conditional |
| `crates/iyon-tui/src/content/text/diff.rs` | ~230 production, ~83 tests | `DiffProjector` | Unified-diff text to styled semantic paragraph | Content | Conditional |
| `crates/iyon-tui/src/content/text/ansi.rs` | ~562 production, ~126 tests | `AnsiProjector` | Safe ANSI/SGR/OSC 8 interpretation into semantic text runs | Content | Conditional |
| `crates/iyon-tui/src/content/text/style.rs` | 556 production, ~466 tests | `TextRole`, `TextPart`, selectors, policies | Semantic style vocabulary and translation to ordinary style facts | Mixed/content-presentation | Yes during rendering |
| `crates/iyon-tui/src/content/text/render/mod.rs` | ~546 production | `TextRenderer` is crate-private | Text IR to retained `View`, semantic sequence caches | Mixed | Yes |
| `crates/iyon-tui/src/content/text/render/block.rs` | 523 lines | Crate-private renderer machinery | Block lowering, list/table/code/quote structure, block cache | Mixed | Yes |
| `crates/iyon-tui/src/content/text/render/inline.rs` | 114 lines | Crate-private renderer machinery | Inline/literal lowering and semantic facts on spans | Mixed | Yes |
| `crates/iyon-tui/src/content/text/render/identity.rs` | 146 lines | Crate-private | Inherited semantic render context and style-fact stamping | Mixed | Yes |
| `crates/iyon-tui/src/content/text/render/policy.rs` | 183 lines | `TextRenderPolicy` and policy enums | Structural renderer policy: gaps, wraps, labels, markers, table tracks | Content/rendering | Conditional |
| `crates/iyon-tui/src/content/text/render/source_format.rs` | 177 test lines | Test-only | Proves non-Markdown source origin specialization | Test | No |
| `crates/iyon-tui/src/content/text/render/structured.rs` | 919 test lines | Test-only | End-to-end structured renderer behavior | Test | No |
| `crates/iyon-tui/src/content/text/render/tests.rs` | 765 test lines | Test-only | Style facts, cache identity, structural sharing, renderer semantics | Test | No |
| `crates/iyon-tui/src/content/text/migrated_tests.rs` | 40 lines | Test-only include wrapper | Includes former integration tests inside owning crate | Test | No |
| `crates/iyon-tui/src/content/text/tests/*.rs` | ~2,300 lines total | Test-only | Public IR, Markdown, incremental, provenance, and pulldown behavior | Test | No |

### 1.2 Approximate LOC method

The estimates above use source line ranges and file endpoints from the tracked files. Blank lines, comments, imports, and test scaffolding are included in the physical estimates. For files containing an inline `#[cfg(test)]` module, production and test estimates are separated approximately by the beginning of the test module. No generated files are included.

Aggregate estimate for the assigned content tree:

- Production/current implementation: approximately **8,000 physical lines**
- Tests and test-only support: approximately **5,500 physical lines**
- Generated code: none inside `crates/iyon-tui/src/content/`

The largest production implementation is `markdown.rs` (~1,433 lines), followed by the renderer (`render/mod.rs`, `render/block.rs`) and the block/IR definitions.

### 1.3 Primary and secondary responsibilities

The directory is not a single renderer. It contains several separable layers:

1. **Value layer**
   - `TextContent`, `RawText`
   - `Block`, `Inline`, `TextRun`, `LiteralText`
   - lists, tables, code blocks, images, links, marks

2. **Metadata layer**
   - `Annotations`
   - `SemanticTag`, `SemanticKey`, `SemanticValue`
   - `TextOrigin`
   - `TextProvenance`

3. **Source/projection layer**
   - `RawDomain`
   - `validate_text_projection`
   - Markdown, plain, diff, and ANSI projectors

4. **Transformation layer**
   - `TextVisitor`
   - `TextRewriter`
   - `RewriteProjector`

5. **Semantic styling layer**
   - `TextRole`, `TextPart`
   - `TextSelector`
   - `TextFacts`
   - `TextRenderPolicy`

6. **Presentation lowering layer**
   - `TextRenderer`
   - semantic-to-`View` lowering
   - persistent sequence and block caches

7. **Separate structured diff layer**
   - validated line/range/hunk model
   - direct hunk-to-`View` lowering

---

## 2. Types, APIs and contracts

### 2.1 Module exports and actual API visibility

`crates/iyon-tui/src/text.rs` is a thin re-export:

```rust
pub use crate::content::text::*;
```

However, `lib.rs` declares `mod text;`, not `pub mod text;`. Consequently, the Rust text API is not an ordinary external public crate-root API in this baseline. `lib.rs` exposes many of the names only as `pub(crate)` aliases for internal use and tests. The native-facing `binding` module selectively re-exports:

- structured diff types and, under `native-host`, `lower_diff_hunks`
- `FormatId`, `LanguageId`, `SemanticTag`, `TextOrigin`
- `TextPart`, `TextRole`, `TextSelector`
- presentation types needed by native ingress

The complete semantic text authoring surface is therefore principally used inside the Rust kernel and through the TypeScript/native facade, not as a public external Rust module.

### 2.2 Root content values

#### `RawText`

`content/text/content.rs:7-89`

`RawText` stores:

```rust
page: Arc<str>,
start: u32,
len: u32,
```

It is a byte slice into an owning retained page. `RawText::new` owns the entire page; `from_page_slice` is crate-private and allows `Source` chunk views to retain slices without copying. Equality is text-based, not identity-based (`content.rs:15-20`).

Important contracts:

- `text()` indexes the page by byte offsets (`content.rs:45-50`).
- `len()` is byte length, not character count.
- `exact_slice(owner, local)` checks owner length, local bounds, and UTF-8 boundaries before constructing a witnessed `TextRun` (`content.rs:62-88`).
- Exact source slicing requires the supplied `StreamRange` length to equal the root text byte length.

Potential capacity boundary: the page length is cast from `usize` to `u32` in `RawText::new` (`content.rs:23-30`). The surrounding source-store limits appear intended to keep payloads below that bound, but the constructor itself does not return an overflow error.

#### `TextContent`

`content/text/content.rs:91-118`

The closed root set is:

```rust
pub enum TextContent {
    Raw(RawText),
    Block(Block),
}
```

There is deliberately no root-level `Inline` or `LiteralText` variant. Raw text is expected to be claimed by a projector or retained as a raw source value. Structured semantic values are block-rooted.

### 2.3 Block IR

`content/text/block.rs:5-581`

The block model is immutable and mostly Arc-backed.

#### Block kinds

`BlockKind` (`block.rs:410-433`) includes:

- `Paragraph(InlineContent)`
- `Heading { level, content }`
- `BlockQuote { blocks }`
- `List(List)`
- `CodeBlock(CodeBlock)`
- `Table(Table)`
- `ThematicBreak`
- `RawBlock { format, body }`
- `Container { blocks }`

`Block` is an Arc-backed immutable wrapper (`block.rs:467-581`) with:

- `Block::new`
- constructors for every block kind
- `kind()`
- annotations
- `with_annotations`
- `map_annotations`
- `as_code_block`, `as_list`, `as_container`
- `ptr_eq`
- crate-private `identity_ptr`

`identity_ptr` is an allocator-address key only; caches retain the owning `Block` to prevent pointer-ABA aliasing.

#### Heading

`HeadingLevel` (`block.rs:5-28`) is validated to 1–6. `H1` through `H6` constants are provided.

#### Lists

`List`, `ListItem`, `ListMarker`, `NumberStyle`, and `NumberDelimiter` (`block.rs:30-180`) represent generic list semantics.

- Bullet and ordered markers are distinct.
- Ordered lists carry `start`, number style, and delimiter.
- `tight` is semantic list spacing metadata.
- List items contain zero or more blocks and optional task state.
- `ListItem::task` stores `checked: Some(bool)`.

`List::with_number_style` and `with_delimiter` reject bullet lists with `InvalidListConfiguration` (`block.rs:107-131`).

One permissive edge is that `List::ordered(start, ...)` does not reject `start == 0`; Markdown-originated lists normally supply valid ordered starts.

#### Tables

`Table`, `TableColumn`, `TableRow`, and `TableCell` (`block.rs:190-408`) hold:

- optional caption blocks
- column alignment
- header row count
- rows and cells
- row/column spans
- cell-specific alignment
- row/cell annotations

`Table::new` validates immediately (`block.rs:198-212`), using `compute_cell_columns` (`block.rs:215-267`) to detect:

- header count beyond row count
- cells that do not fit the declared column schema
- overlapping row/column spans
- row spans beyond the table

`cell_start_columns()` is crate-private and relies on the construction invariant (`block.rs:274-276`).

### 2.4 Inline IR

`content/text/inline.rs:5-389`

#### Inline kinds

`InlineKind` (`inline.rs:216-224`) includes:

- `Text(TextRun)`
- `Break(BreakKind)`
- `Image(Image)`
- `RawInline { format, body }`

`Inline` is Arc-backed immutable state with:

```rust
kind: InlineKind,
marks: MarkSet,
annotations: Annotations,
```

`Inline` supports:

- `text`
- `break_`
- `image`
- `raw`
- `kind`
- `marks`
- `annotations`
- `as_text`
- `with_mark`
- `with_marks`
- `with_annotations`
- `map_annotations`
- `ptr_eq`

#### Breaks

`BreakKind` distinguishes `Soft` and `Hard` (`inline.rs:5-10`).

The parser preserves soft breaks in Markdown IR; the renderer later applies `SoftBreakPolicy::Space` or `LineBreak`.

#### Marks and links

`Mark` (`inline.rs:71-84`) includes:

- emphasis
- strong
- strikethrough
- underline
- superscript
- subscript
- small caps
- code
- link with `LinkTarget`

`MarkSet` (`inline.rs:86-125`) canonicalizes by sorting and deduplicating. It rejects more than one `Mark::Link` with `DuplicateLinkMark`. Its sorted Arc slice enables binary-search lookup.

The canonicalization makes mark order irrelevant. Tests in `document_public.rs:500-512` verify that differently ordered strong/emphasis sets compare equal and duplicate link marks fail.

#### Images

`Image` (`inline.rs:356-389`) retains destination, optional title, and semantic alt inline content. The renderer currently displays the alt content as an image fallback rather than terminal image data.

#### Inline collections

`InlineContent` (`inline.rs:127-214`) is immutable ordered `Arc<[Inline]>` storage with conversion from `Inline`, `TextRun`, `&str`, and `String`. It supports immutable `with_mark` transformations and convenience `strong`, `emphasis`, and `code` methods.

### 2.5 Provenance and literal text

`content/text/provenance.rs:7-221`

#### `TextProvenance`

```rust
pub enum TextProvenance {
    Exact(StreamRange),
    Derived(StreamRange),
    Synthetic,
}
```

Semantics:

- `Exact(range)`: display text is intended to equal source bytes in the range.
- `Derived(range)`: display text is transformed from source bytes in the range.
- `Synthetic`: display text has no source bytes.

#### `TextRun`

`TextRun` contains:

```rust
text: Arc<str>,
provenance: TextProvenance,
annotations: Annotations,
style: Option<crate::StyleRef>,
```

The style field is documented as semantic style intent, not a native style ID (`provenance.rs:24-27`). Nevertheless, its type is a presentation-layer `StyleRef`, which creates a source-level dependency on the current presentation API.

Constructors:

- `TextRun::exact` checks UTF-8 boundaries and byte-length equality but has no source witness (`provenance.rs:59-80`).
- `TextRun::derived` stores arbitrary transformed text and source range.
- `TextRun::synthetic` stores text with no source mapping.

The exact constructor cannot prove byte equality. This is explicitly documented and enforced conceptually by validation (`provenance.rs:60-64`, `validate.rs:24-28`).

`split_at`:

- rejects non-character boundaries
- splits exact ranges proportionally by byte offset
- preserves the same source range for derived runs
- preserves synthetic provenance
- copies annotations and style to both pieces (`provenance.rs:148-176`)

The test at `document_public.rs:443-466` verifies UTF-8 splitting, exact range behavior, synthetic boundary errors, and derived split semantics.

#### `LiteralText`

`LiteralText` (`provenance.rs:179-221`) is an Arc-backed ordered list of `TextRun`s for code/raw nested-language portals.

- `from_exact` constructs one witnessed exact run.
- `runs()` exposes the retained run list.
- `text()` concatenates runs.
- Empty means no runs or all runs have empty text.

### 2.6 Annotations

`content/text/annotations.rs:5-176`

#### `SemanticTag` and `SemanticKey`

Both hold validated namespace/name pairs as `Arc<str>`:

```rust
SemanticTag { namespace, name }
SemanticKey  { namespace, name }
```

`validate_name` requires nonempty, whitespace-free names (`errors.rs:159-167`).

Both implement ordering, equality, hashing, and display as `namespace:name`.

#### `SemanticValue`

`SemanticValue` supports:

- `Bool(bool)`
- `Integer(i64)`
- `Text(Arc<str>)`
- `TextList(Arc<[Arc<str>]>)`

There are conversions from `bool`, `i64`, `String`, and `&str`. There is no convenience `From<Vec<String>>` visible in this module.

#### `Annotations`

`Annotations` contains sorted immutable arrays:

```rust
tags: Arc<[SemanticTag]>,
properties: Arc<[(SemanticKey, SemanticValue)]>,
```

Contracts:

- `with_tag` is idempotent and keeps tags sorted (`annotations.rs:117-131`).
- `with_property` replaces an existing key and sorts properties (`annotations.rs:133-150`).
- `contains_tag` uses binary search.
- `property` uses binary search.
- Arbitrary caller namespaces are supported; framework-specific meanings are not hard-coded here.

### 2.7 Origin metadata

`content/text/origin.rs:13-288`

`TextOrigin` is a validated named projector identity:

- `MARKDOWN = "markdown"`
- `PLAIN_TEXT = "plain-text"`
- `ANSI = "ansi"`
- `DIFF = "diff"`

Custom origins are allowed with `TextOrigin::new`, subject to name validation.

Origin is stored as a regular semantic annotation property under a cached key:

```text
namespace: iyon-tui
name: origin
```

`Annotations::with_origin` and `Annotations::origin` are implemented at `origin.rs:106-130`.

Origin helpers exist for:

- `Annotations`
- `Block`
- `Inline`
- `ListItem`
- `TableRow`
- `TableCell`

`stamp_block_origin` recursively stamps nested blocks, inline content, list items, table rows/cells, and image alt content (`origin.rs:195-287`).

Important exception: code-block and raw-block bodies are not recursively stamped as run annotations:

```rust
BlockKind::CodeBlock(code) => BlockKind::CodeBlock(code),
BlockKind::RawBlock { format, body } => BlockKind::RawBlock { format, body },
```

The renderer instead propagates the containing block's origin through `RenderContext` into body style facts. Therefore:

- `block.origin()` can identify a Markdown code block.
- its body runs may not individually carry an origin annotation.
- rendered body spans can still match origin-aware selectors because inherited render context adds the origin.

### 2.8 Validation contracts

`content/text/errors.rs` and `validate.rs`

#### `TextIrError`

Covers:

- invalid semantic names
- exact length mismatch
- invalid heading levels
- table shape/span errors
- invalid source slices
- duplicate link marks
- invalid list configuration
- non-character-boundary splits

#### `TextProjectionError`

Wraps projection validation and adds:

- raw values must be sole span values
- raw byte length mismatch
- nested source range outside owning span
- exact run length mismatch

#### `validate_text_content`

`validate.rs:5-22` validates a standalone value against an owner range.

#### `validate_text_projection`

`validate.rs:24-61`:

1. validates ordinary projection structure through `validate_projection`
2. enforces the Raw-only span invariant
3. recursively validates every structured block
4. validates nested Exact/Derived provenance containment

Exact runs are checked for length equality. Derived runs are checked for containment but not length equality.

The source witness limitation remains: validation cannot prove that an `Exact` run's bytes equal the source bytes when only a `StreamRange` is retained. `RawDomain::exact_runs` provides that stronger witness during parser construction.

### 2.9 Visitors and rewriters

`content/text/visit.rs:10-365`

#### `TextVisitor`

The visitor traverses:

- raw values
- blocks
- inline content
- inline values
- literal bodies
- text runs

`walk_block` reaches nested list items, table captions, rows, cells, code bodies, raw bodies, block quotes, and containers (`visit.rs:35-70`). `walk_inline` reaches image alt content and raw inline literal bodies.

#### `TextRewriter`

`TextRewriter` has override points for:

- content
- block
- inline
- inline content
- block vectors
- literal text

The default methods recurse through the generic walk helpers.

#### `RewriteProjector`

`RewriteProjector` preserves projection envelope metadata:

- source spans
- stable frontier
- source end
- sealed state

It rewrites each value, rebuilds the projection, and validates the result (`visit.rs:159-190`).

#### Persistent identity preservation

`walk_rewrite_block` retains the original `Block` when rebuilt kind and annotations are equal (`visit.rs:246-253`).

`walk_rewrite_blocks` preserves the original vector when every child retains pointer identity (`visit.rs:263-275`).

`walk_rewrite_inline_content` preserves the original `InlineContent` when every inline retains pointer identity (`visit.rs:284-300`).

This is exercised by `document_public.rs:500-545`, `:576-651`, and `:789-798`. No-op rewriting over 10,000 blocks is explicitly tested to preserve the root pointer.

One special case is documented in `SourceAnnotationRewriter::rewrite_inline` (`application/content.rs:1361-1383`): if a direct `rewrite_inline` call causes one run to split into multiple pieces, that method retains only the first piece because the enclosing vector boundary is unavailable. The application path overrides `rewrite_inline_content` and expands all pieces correctly (`application/content.rs:1386-1405`).

---

## 3. Dependency and ownership map

### 3.1 Forward dependency graph

```text
Projection<TextContent>
    │
    ├── validate_text_projection
    │
    ├── PlainTextProjector
    │       └── RawDomain
    │
    ├── MarkdownProjector
    │       ├── pulldown-cmark
    │       ├── RawDomain
    │       ├── Block / Inline / TextRun
    │       └── ProjectionBuilder
    │
    ├── DiffProjector
    │       ├── RawDomain
    │       ├── TextRun / annotations / StyleRef
    │       └── ProjectionBuilder
    │
    └── AnsiProjector
            ├── RawDomain
            ├── TextRun / StyleRef / LinkTarget
            └── ProjectionBuilder

TextContent IR
    │
    ├── TextVisitor / TextRewriter
    │
    └── TextRenderer
            ├── TextRenderPolicy
            ├── RenderContext / TextFacts
            ├── presentation::factory
            └── View

View
    └── presentation layout / paint / backend
```

### 3.2 Application/content ownership graph

```text
HostContentSource
    └── HostContentSourceSnapshot
            └── Arc<StoredSource>
                    └── chunk views / source annotations

HostContentConnector
    └── ConnectorExecution
            ├── MarkdownProjector / DiffProjector / AnsiProjector
            ├── ConnectorDelivery / Smooth
            ├── TextRenderer
            └── parser lineage

snapshot + funnel
    └── source_projection
            └── semantic projector
                    └── SourceAnnotationRewriter
                            └── Projection<TextContent>

Projection<TextContent> + TextRenderer + Theme + width
    └── View lowering / layout / rows
```

`ConnectorExecution` is created per connector/funnel in `application/content.rs:423-454`. It owns parser state, optional smoothing state, and the renderer. Deactivation drops that execution state, so inactive connectors do not retain parsers, delivery state, or semantic lowering caches.

`HostContentSourceSnapshot` owns an Arc-backed source-store snapshot (`application/content.rs:108-187`). The content projector borrows this snapshot while constructing projection products.

### 3.3 Ownership and lifetime

#### Semantic values

- `Block`, `Inline`, `TextRun`, and collection types are immutable and cheap to clone through `Arc`.
- Parser-local mutable state is owned by each projector instance.
- Projected `Projection<TextContent>` owns its semantic values and source ranges.
- `TextRenderer` owns view-lowering caches and is retained by `ConnectorExecution`.

#### Parser state

- `MarkdownProjector` owns:
  - options
  - required restart frontier
  - last stable frontier
  - checkpoint ranges
  - cached parsed domains
  - optional test counters
- `DiffProjector` owns:
  - source continuation identity
  - hunk-state boolean
  - completed inline prefix
  - completed source end
- `AnsiProjector` owns:
  - source continuation identity
  - completed ANSI state
  - completed inline prefix
  - completed source end

#### Cache ownership

- `RawDomain` owns page witnesses and lazy assembled buffers.
- `BlockLoweringCache` is owned by `TextRenderer`.
- Semantic sequence/raw/edge caches are also owned by `TextRenderer`.
- Application semantic projection cache is owned by the content runtime, not by the source or projector modules.
- Theme/layout/physical-row caches are owned by application/content or presentation layers.

### 3.4 Dependency boundary observations

The semantic modules depend on:

- `crate::stream::{StreamOffset, StreamRange}`
- `crate::projection::{Projection, ProjectionBuilder, Projector}`
- `crate::presentation::api::StyleRef` through `TextRun`
- `crate::presentation` factories only in renderer modules
- `pulldown-cmark` only in Markdown parsing

The semantic IR itself does not depend on History, terminal backend, clock, or application state. The renderer does depend on presentation `View` and style APIs by design.

The most important source-level coupling is:

```text
TextRun
    └── Option<crate::StyleRef>
```

This means an alternative host can reuse most semantic values but cannot treat `TextRun` as entirely free of the current presentation vocabulary without either supporting `StyleRef` or translating it.

---

## 4. Execution paths and state transitions

### 4.1 Raw source to semantic projection

`application/content.rs:611-629` builds a `Projection<TextContent>` from source chunk views:

1. `HostContentSourceSnapshot::chunk_views()` supplies retained page slices.
2. Each chunk becomes `RawText::from_page_slice`.
3. Each chunk is emitted as one `TextContent::Raw`.
4. The projection preserves absolute source coordinates.
5. Projection construction validates the envelope.

For direct application content, `project_semantic_snapshot` (`application/content.rs:690-730`) then:

1. prepares parser lineage
2. builds the raw projection
3. selects one funnel projector
4. projects raw domains
5. optionally applies `SourceAnnotationRewriter`

Structured spans are passed through unchanged by all four projectors. This is the “hard barrier” contract: a front-end projector claims raw spans but does not parse already-structured semantic values.

### 4.2 Plain text route

`PlainTextProjector::project` (`plain.rs:20-75`):

1. validates the input projection
2. copies non-raw spans and values unchanged
3. groups consecutive raw spans into a `RawDomain`
4. creates one paragraph block for the entire raw domain
5. converts each source newline into `Inline::break_(BreakKind::Hard)`
6. creates exact runs for non-newline segments
7. stamps `TextOrigin::PLAIN_TEXT`
8. rebuilds output with computed stable frontier

The raw domain grouping permits chunk-independent behavior and preserves exact source witnesses across source-store page boundaries.

### 4.3 Markdown route

`MarkdownProjector::project` (`markdown.rs:142-264`):

1. validates input.
2. rejects insufficient retained restart context if `required_restart_from` is ahead of the input base.
3. builds a provisional output with a conservative stable frontier.
4. passes already-structured spans through.
5. groups consecutive raw spans into `RawDomain`.
6. parses each domain through cached or uncached pulldown parsing.
7. updates stability based on barriers, open blocks, reference definitions, and table state.
8. finalizes stable frontier snapped to a semantic span boundary.
9. stores source checkpoints for future `restart_from`.

The projector's public state transition is:

```text
new projector
    ↓
first raw domain parse
    ↓
cache stable parsed prefix
    ↓
append/revision with same retained base
    ├── exact cache hit
    ├── parse only suffix after stable cache
    ├── restart farther back for references/open syntax
    └── reject if required context was compacted away
    ↓
sealed input
    └── stable_through == source_end
```

#### Markdown event handling

`Builder::event` (`markdown.rs:673-703`) handles:

- start/end tags
- text
- code text
- inline HTML
- block HTML
- soft/hard breaks
- thematic rules
- task markers

Unsupported parser events such as math, footnote references, and unsupported tags produce explicit `ParserInvariant` errors.

The parser uses frame types for paragraphs, block quotes, lists, list items, code, HTML, tables, rows, cells, images, and root (`markdown.rs:590-653`).

#### Source provenance conversion

`Builder::inline_text` (`markdown.rs:1050-1074`) compares parser event text with `RawDomain::source_slice`:

- if equal, it uses `RawDomain::exact_runs`
- otherwise it creates a `Derived` run mapped to the parser range

This preserves exact source witnesses wherever pulldown emits unchanged source text while correctly representing entity decoding and other transformations as derived.

#### Root list behavior

At `markdown.rs:863-881`, a root list is emitted as one list block per item rather than one list block containing all items. The source comment explains that this keeps a closed item stable while a later item grows. Nested lists remain structurally nested.

#### Tables

At `markdown.rs:982-1001`:

- a table can be downgraded to a raw pipe paragraph when live-table stabilization says the table is still open
- otherwise ragged GFM rows are normalized to the declared schema width
- short rows are padded with empty cells
- extra cells are dropped
- the generic `Table::new` validator remains strict

#### Live table stability

`MarkdownOptions::with_live_table_stabilization` is explicitly not GFM grammar (`markdown_options.rs:9-13`, `:63-71`). It is an incremental streaming policy. `following_line_closes_table` (`markdown.rs:1335-1356`) closes a live table only when a following blank/non-pipe line proves that no more pipe rows can arrive.

### 4.4 Text diff projector route

`content/text/diff.rs:1-230`

`DiffProjector` treats unified diff as source-format content, not as a structured `DiffHunk`.

Call path:

```text
RawDomain
    ↓
line_ranges
    ↓
parse_single_diff_line
    ↓
Inline::Text(TextRun) / Inline::Break(Hard)
    ↓
Block::paragraph
    ↓
TextContent::Block
```

`parse_single_diff_line` classifies:

- `@@ ` as hunk header
- `\ No newline` as metadata
- `+` inside a hunk, excluding `+++`, as addition
- `-` inside a hunk, excluding `---`, as deletion
- leading space inside a hunk as context
- all other lines as metadata

The marker is synthetic and the body is exact source text:

```rust
marker → TextRun::synthetic(...)
body   → RawDomain::exact_runs(...)
```

Both marker and body receive a `SemanticTag` under namespace `diff`, and a themed `StyleRef`:

- `diff.header`
- `diff.meta`
- `diff.addition`
- `diff.deletion`
- `diff.context`

Every source newline becomes a hard break.

Continuation state is line-oriented:

- complete lines append to `completed_inlines`
- a partial final line is held as trailing inlines
- the next larger snapshot resumes from `completed_end`
- source-base or monotonicity mismatch resets parser state

This route does not parse hunk line numbers, ranges, or termination metadata.

### 4.5 ANSI projector route

`content/text/ansi.rs:22-453`

`AnsiProjector` parses ANSI as content syntax:

- SGR attributes/colors
- OSC 8 hyperlinks
- arbitrary cursor/window/control sequences are consumed
- unsafe controls never reach the terminal backend

Call path:

```text
RawDomain
    ↓
byte scanner
    ├── newline → Hard break
    ├── CSI/SGR → state transition
    ├── OSC 8 → link state transition
    └── ordinary bytes → exact TextRun with current style/link
    ↓
Block::paragraph
```

State includes:

- foreground/background `ColorSpec`
- bold, dim, italic, underline
- reversed
- strikethrough
- optional `LinkTarget`

`push_segment` creates a themed `StyleRef` containing the current `StyleSpec` (`ansi.rs:353-374`). This preserves ANSI style intent in semantic runs but uses the presentation `StyleRef` type.

Incremental behavior:

- complete lines and complete escapes are appended to retained state
- incomplete CSI/OSC/ESC at an unsealed boundary is held and not leaked
- incomplete controls at seal are consumed/dropped
- source-base/continuation checks reset state if the input is not a valid append

### 4.6 Text renderer route

The renderer is `TextRenderer` (`content/text/render/mod.rs:167-210`), crate-private and retained per application connector.

The block path is:

```text
TextContent::Block
    ↓
TextRenderer::lower_block
    ↓
BlockLoweringCache lookup by block identity + inherited context
    ↓ miss
lower_block_uncached
    ├── paragraph → styled text
    ├── heading → styled text + heading facts
    ├── quote → hanging "> " marker
    ├── list → hanging list/task markers
    ├── code → optional label + wrapped literal body
    ├── table → Grid
    ├── thematic break → generated rule text
    ├── raw block → literal body
    └── container → column
```

The inline path (`render/inline.rs:10-114`) turns inline values into `TextSpan`s and applies:

- inherited context facts
- node annotations
- origin
- language/format
- mark roles
- local `TextRun` style override if present

### 4.7 Renderer state and contexts

`RenderContext` (`render/identity.rs:8-92`) tracks:

- complete ancestor role path
- origin
- list kind
- task state
- table section
- language
- format

The complete ancestor role path is in the block cache key. A depth alone would be insufficient because selectors can distinguish different ancestor role sequences (`identity.rs:8-20`).

`semantic_view_facts` and `part_facts` combine:

- inherited ancestor roles
- current role or generated part
- scalar dimensions
- annotations

`stamp_view` and `stamp_text` attach the reserved `text_style_ref()` to a `View` (`identity.rs:120-125`).

### 4.8 Application connector path

`ConnectorExecution::new` (`application/content.rs:439-454`) selects parser state from `HostContentFunnel`:

- Markdown creates `MarkdownProjector` with GFM and live-table stabilization
- Diff creates `DiffProjector`
- ANSI creates `AnsiProjector` configured for hyperlinks
- Plain has no persistent projector because `PlainTextProjector` is stateless
- all connectors create a persistent `TextRenderer`
- smooth connectors create `ConnectorDelivery`

`prepare_for_snapshot` (`application/content.rs:457-472`) detects source lineage changes:

```text
(source_id, source_generation, content_generation)
```

If lineage changes after a prior snapshot, it drops Markdown/Diff/ANSI parser instances and starts a new logical document. Smooth delivery is deliberately retained according to its replacement policy, and the renderer remains reusable.

### 4.9 Semantic compilation

`compile_semantic_content` (`application/content.rs:746-782`):

1. lowers the semantic projection to a `View` through `TextRenderer::lower_semantic_iter`
2. creates a `ViewCompiler`
3. constructs a width-constrained layout tree
4. compiles rows with a `TextGeometryCache`

`layout_semantic_content` (`application/content.rs:784-797`) performs the layout-only route used when physical rows can be deferred.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation to production path

| Semantic operation | Production path | Selection condition | Result | Failure/recovery behavior |
|---|---|---|---|---|
| Preserve raw source | `source_projection` → `TextContent::Raw` | Source chunk ingress | Raw semantic span | Raw span must be sole value and byte length must equal source range |
| Plain text claim | `PlainTextProjector` | `TextFunnelKind::Plain` | Paragraph with exact runs and hard breaks | Projection errors returned; structured barriers passed through |
| Markdown parse | `MarkdownProjector` | `TextFunnelKind::Markdown` | Blocks/inlines/tables/lists/code/raw portals | Explicit invalid nesting/parser invariant errors; cache/restart on incremental input |
| Unified diff text | `DiffProjector` | `TextFunnelKind::Diff` | Paragraph with tagged/styled line fragments | Malformed lines retained as metadata/plain lines rather than failing |
| ANSI text | `AnsiProjector` | `TextFunnelKind::Ansi` | Paragraph with style/link-aware runs | Unsafe controls consumed; incomplete unsealed controls held; malformed controls dropped at seal |
| Structured diff | `DiffHunk` + `lower_diff_hunks` | Native direct diff view ingress | Header/marker/payload View tree | Invalid ranges, coordinates, UTF-8, framing, or hunk counts reject the operation |
| Generic structured rendering | `TextRenderer` | Any `TextContent::Block` | View tree with semantic facts | Validated IR assumed; table validity checked at construction |
| Source annotation application | `SourceAnnotationRewriter` | Source has annotations | Annotated/split semantic runs | Exact/derived ranges mapped; synthetic runs unaffected |

### 5.2 Hard barriers

Every text projector checks whether a span is raw. Non-raw spans are copied unchanged:

- Markdown: `markdown.rs:170-180`
- Plain: `plain.rs:35-44`
- Diff: `diff.rs:131-137`
- ANSI: `ansi.rs:97-104`

This permits front-end composition such as:

```text
raw source
    ↓ outer projector claims selected raw spans
structured code/table/container spans
    ↓ later projector sees them as barriers
only remaining raw spans are claimed
```

`document_public.rs:374-440` explicitly tests structured blocks as hard barriers.

### 5.3 Markdown failures

Failures are explicit rather than silently masked:

- `InsufficientRestartContext` when a retained source base is after required reference context (`markdown.rs:151-157`)
- `InvalidNesting` when frame or mark stacks do not match
- `InvalidSourceMap` for invalid source coordinate conversions
- `ParserInvariant` for unsupported pulldown events/tags
- `TextIrError` for invalid table or semantic construction

Malformed but supported Markdown is generally normalized by pulldown rather than treated as an error.

### 5.4 Diff text failures

The text diff projector deliberately has permissive semantics. Its module-level documentation states:

> Malformed diff syntax is retained as styled metadata/plain lines instead of making a frame fail.

The parser does not validate hunk counts, line numbers, or source range relationships. It only classifies lines by prefixes and hunk state.

This is materially different from the structured `DiffHunk` model.

### 5.5 Structured diff failures

`DiffHunk::new` validates immediately (`content/diff/model.rs:286-298`).

Validation checks:

- range arithmetic overflow
- one-based line coordinate construction
- line coordinate kind consistency
- sequential coordinate progression
- old/new consumed count equality
- final maximum coordinates without successor overflow

Diagnostics are represented by `DiffValidationError` (`model.rs:209-276`).

### 5.6 ANSI failures and safety

ANSI controls that could move the cursor, clear the screen, switch modes, or write directly to the terminal are consumed by the parser. `try_consume_escape` explicitly consumes all other two-byte ESC commands (`ansi.rs:377-390`).

Unterminated CSI/OSC sequences:

- unsealed: text before the escape is emitted, incomplete escape is held
- sealed: text before the escape is emitted, incomplete escape is dropped

The tests at `ansi.rs:602-663` and `:665-687` cover this behavior.

### 5.7 Source/provenance failures

`validate_text_projection` rejects:

- mixed Raw and structured values in one span
- Raw values whose byte length differs from the owning source span
- nested Exact/Derived ranges outside the owner
- Exact text length mismatch
- invalid projection structure

It does not prove Exact byte equality because `TextRun::exact` has no source witness. The stronger source-witness path is only available while operating on `RawDomain`.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 RawDomain materialization

`RawDomain` (`source.rs:13-407`) is designed to avoid unnecessary assembly.

For one source page:

- stores an Arc page and subrange directly
- `text()` borrows the existing page slice

For multiple pages:

- retains `RawPiece` witnesses
- stores a lazy `OnceLock<Arc<str>>`
- only assembles when parser text access requires a contiguous buffer

`newline_offsets` can scan all retained pieces without materializing the entire domain (`source.rs:179-197`).

`prefix` and `suffix` construct narrowed domains while preserving source witnesses (`source.rs:199-278`).

`exact_runs` splits at every retained raw piece boundary (`source.rs:376-397`), preserving page-backed exact provenance. The test at `source.rs:414-460` verifies that suffix materialization does not materialize the original full domain and that exact runs remain split across two pieces.

### 6.2 Markdown parser cache

`MarkdownProjector` stores a vector of `CachedDomain` records:

```rust
source_base
stable_end
spans
has_reference_context
```

The cache is not globally bounded in `markdown.rs`; application lifetime and connector replacement bound it indirectly. The cache supports:

- exact stable-domain hits
- suffix-only parsing after cached stable prefixes
- full reparsing when reference context is present
- prefix proof by re-parsing a candidate stable region

Counters under `test-util`:

```rust
parser_invocations
parser_bytes
```

are exposed by `parser_work()` (`markdown.rs:128-133`).

The hardening test `markdown_hardening.rs:322-341` checks that 1,000 incremental blocks do not cause unconstrained full-source reparsing by asserting parser bytes are less than 100 times final source size.

### 6.3 TextRenderer block cache

`BlockLoweringCache` (`render/block.rs:12-63`):

- key: `block_ptr + RenderContextKey`
- capacity: 4
- eviction: clear all entries when capacity is reached
- entry owns both the source `Block` and lowered `View`

The context key includes all inherited semantic dimensions that affect facts. This avoids stale style-context reuse when the same immutable block is rendered under different ancestor roles, origins, languages, or formats.

### 6.4 TextRenderer semantic lowering cache

`SemanticLoweringCache` (`render/mod.rs:144-165`) stores:

- raw view cache
- edge/spacing views
- up to four persistent semantic sequences

#### Raw cache

`RawCacheKey`:

```text
page_ptr, start, len
```

The entry owns the source page Arc to prevent pointer reuse from producing stale cache hits (`render/mod.rs:35-47`, `:482-520`).

Capacity: 1,024, then clear-all eviction.

#### Edge cache

`EdgeCacheKey`:

```text
child ViewId
gap
predecessor list marker/tightness
```

Capacity: 2,048, then clear-all eviction (`render/mod.rs:523-544`).

#### Semantic sequence cache

`SemanticItemKey` includes:

- raw page pointer/start/length and owning page Arc
- block pointer and owning Block

Sequence entries retain:

- item key persistent sequence
- edge key persistent sequence
- child sequence
- final View

Capacity: four sequences (`render/mod.rs:152-158`).

The append algorithm:

1. finds a matching cached sequence by first item
2. reuses matching prefix items and edge products
3. detects the first mismatch
4. bulk-builds the rebuilt suffix
5. concatenates persistent sequence roots
6. stores the new sequence
7. drops stale tail items on shortening

The renderer tests at `render/tests.rs:547-685` cover:

- slice rendering equivalence
- append updates only the tail
- owner retention across cache eviction
- shortening without stale tail leakage

### 6.5 Application semantic projection cache

`SemanticProjectionKey` (`application/content.rs:269-300`) includes:

- source ID
- source generation
- content generation
- source revision
- source base/end
- sealed state
- funnel kind
- hyperlink option

It excludes:

- theme
- width
- wrap mode
- delivery tick
- viewport

`resolve_cached_semantic` (`application/content.rs:302-324`) has capacity two. Theme-only changes hit this cache and do not re-run parsing. A semantic rebuild increments `Counter::SemanticProjectionRebuilds` (`application/content.rs:317`).

### 6.6 Width/theme/layout keys

`TextProjectionKey` (`application/content.rs:207-258`) adds:

- width
- wrap mode
- funnel kind
- delivery revision
- theme revision
- finalized-prefix requirement
- physical-row requirement

This separates semantic IR caching from layout/paint products.

The prepared paint cache includes theme revision and width. Layout trees are retained separately from themed physical rows (`application/content.rs:909-925`).

### 6.7 Delivery and smoothing

`ConnectorDelivery` (`application/content.rs:354-421`) owns:

- `Smooth`
- grapheme projection
- indexed source generation/revision/sealed state
- candidate frontier

On a source change, `accept_input` builds a grapheme projection and submits it to `Smooth` (`application/content.rs:386-401`).

On a pure tick, `advance` only advances the smoother (`application/content.rs:404-410`). It does not rebuild source projections, reparse Markdown/diff/ANSI, or lower semantic text.

`source_grapheme_projection` uses `unicode_segmentation` and preserves a single trailing grapheme across chunk boundaries (`application/content.rs:631-687`).

### 6.8 Finalized-prefix products

`prove_finalized_prefix` (`application/content.rs:950-1000`) renders a sealed stable prefix independently of the open document. This is important for Markdown, where an open trailing block may render differently until syntax closes.

The finalized-prefix product is deliberately separate from open-document rows and can be transferred into History-like consumers once the existing stable-prefix proof succeeds.

### 6.9 Per-append/frame/width work

| Event | Work observed statically |
|---|---|
| New source append | Source snapshot → raw projection → selected semantic projector; cache lookup may avoid parser work |
| Markdown append with stable prefix | Cache hit or suffix parse; possible candidate prefix proof |
| Diff/ANSI append | Stateful projector parses only after completed source end; retained completed prefix is reused |
| Pure smoothing tick | `Smooth::advance`; no source parse or semantic projection rebuild |
| Width change | Semantic projection cache remains valid; renderer/layout/paint product keys change |
| Theme change | Semantic projection and geometry-independent View remain valid; paint resolution uses new theme and prepared theme key |
| Frame | Application consumes prepared products; no parser work unless content is dirty or key misses |
| Source replacement/clear | Connector parser instances reset through lineage; renderer persists |

No performance counters are defined inside `crates/iyon-tui/src/content/`; semantic rebuild counters and prepared-product behavior are in `application/content.rs`.

---

## 7. Tests, benchmarks and observability

### 7.1 Test inventory

The assigned tree contains substantial behavioral coverage.

#### Core model tests

- `content/diff/model.rs:396-558`
  - offset/number semantics
  - range overflow
  - line payload and termination
  - valid hunk sequences
  - empty old/new ranges
  - maximum coordinates
  - coordinate/count rejection
- `content/diff/render.rs:129-277`
  - headers and markers
  - payload preservation
  - unterminated metadata
  - narrow wrapping
  - default/application theme behavior
  - multiple hunk order
- `content/text/block.rs:583-683`
  - table placement
  - column spans
  - row spans
  - overlap rejection
- `content/text/origin.rs:290-379`
  - origin validation
  - replacement of origin property
  - coexistence with arbitrary annotations
  - recursive stamping
  - table row/cell origin

#### Public IR and visitor tests

`content/text/tests/document_public.rs`:

- generic projector composition and nested code/diagram portals (`:309-372`)
- structured hard barriers (`:374-440`)
- provenance/source validation (`:442-498`)
- canonical marks and annotations (`:500-545`)
- persistent rewriting and structural sharing (`:576-651`)
- nested visitor reachability through lists, tables, images, and literal portals (`:653-700`)
- table construction invariants (`:702-752`)
- multi-value span segmentation (`:754-787`)
- no-op large-tree preservation (`:789-798`)

#### Markdown tests

- `markdown_smoke.rs`
  - CommonMark block parsing and exact code bodies (`:28-48`)
  - independently selectable extensions (`:50-66`)
  - GFM table/strike/task mapping (`:73-133`)
  - CommonMark does not claim GFM extensions (`:136-166`)
  - plain projector claim behavior (`:169-177`)
- `markdown_incremental.rs`
  - setext heading remains mutable until seal (`:68-77`)
  - references and incremental seal convergence (`:79-89`)
  - origin preservation through cached parses (`:91-117`)
  - GFM table/task/strike incremental convergence (`:119-170`)
  - live table stable-frontier behavior (`:172-216`)
  - table after list (`:218-247`)
  - nested thematic break in block quote (`:249-269`)
  - every-character live Markdown prefix validation (`:271-324`)
  - GFM examples 202/203/204 (`:343-376`)
  - live stabilization versus strict GFM (`:378-399`)
- `markdown_hardening.rs`
  - raw transport segmentation independence (`:130-151`)
  - restart including reference context (`:161-181`)
  - stable closed blocks inside raw domains (`:183-197`)
  - unresolved references remain mutable (`:199-225`)
  - nonzero root coordinates (`:228-256`)
  - Smooth composition (`:258-284`)
  - Unicode chunk invariance (`:286-320`)
  - stable cache parser-work bound (`:322-341`)
- `pulldown_characterization.rs`
  - event ranges, code, list, reference, table, break, task, HTML, and rule shape

#### ANSI tests

`content/text/ansi.rs:576-687`:

- incremental equals one-shot
- incomplete escape held until completion
- incomplete escape dropped safely on seal
- unsafe control sequences never leak to output

#### Renderer tests

- `render/tests.rs`
  - framework semantic defaults
  - source-origin specialization
  - conjunction selectors
  - inherited roles
  - annotation locality
  - language/format selectors
  - generated parts
  - local style precedence
  - block cache identity
  - semantic sequence append/shorten/eviction behavior
- `render/structured.rs`
  - table-to-single-grid structure
  - spans and logical columns
  - shared column alignment
  - table caption
  - narrow table safety
  - header styling
  - task/list marker identity
  - list continuation prefixes
  - nested lists and quotes
  - code labels/wrapping
  - raw inline and code language selectors
  - GFM end-to-end render fixture

### 7.2 Test-only renderer trait

`content/render.rs:3-9` defines:

```rust
pub(crate) trait Renderer<Input: ?Sized> {
    fn render(&self, input: &Input) -> View;
}
```

It is compiled only under `#[cfg(test)]` through `content/mod.rs:5-10`. The production `TextRenderer` remains crate-private and does not expose a public generic renderer extension trait.

### 7.3 Observability

Observed observability hooks:

- `MarkdownProjector::parser_work()` under `test-util` (`markdown.rs:128-133`)
- `Counter::SemanticProjectionRebuilds` incremented by application semantic cache miss (`application/content.rs:317`)
- native direct diff tests verify cache-first behavior and invalid input codes in `crates/iyon-tui-native/src/tui/view_abi.rs:3123-3260`, especially `:5575-5672`

There are no content-tree-local benchmark files. Benchmark and counter aggregation is outside the assigned directory.

### 7.4 Validation status

No tests or benchmarks were executed in this investigation. All test claims above are based on source inspection.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Semantic text is mostly presentation-independent, but not fully

The IR types themselves are largely generic:

- `Block`
- `Inline`
- `InlineKind`
- `Mark`
- `MarkSet`
- `TextProvenance`
- `Annotations`
- `TextOrigin`
- `TextRole`
- `TextPart`

They do not contain terminal geometry, physical cells, viewport state, or backend handles.

However, `TextRun` contains `Option<crate::StyleRef>` (`provenance.rs:18-28`). ANSI parsing and source annotation application use this field. This means the IR has a direct type-level dependency on the presentation API even though the style is documented as host-independent intent.

This is not the same as containing a native style ID; the style is theme-resolvable and not backend-materialized. It is nevertheless a coupling that an alternate host would have to account for.

### 8.2 Semantic styling becomes physical styling at paint

The conversion boundary is:

```text
TextRole/TextPart/TextOrigin/annotations
    ↓ TextFacts
StyleRef::theme("__iyon_tui.text")
    ↓ ThemeResolver / StyleContext
PhysicalStyle
    ↓
PhysicalRow / terminal cells
```

`TextFacts` is encoded into ordinary `StyleFacts` using reserved keys (`style.rs:336-555`). `TextSelector` is explicitly a facade over the ordinary `StyleSelector`, not a separate text theme engine (`style.rs:96-107`).

The renderer attaches semantic facts; Theme resolution occurs later. This supports recoloring without parsing source again.

### 8.3 Theme changes do not reparse semantic content

Application semantic keys exclude theme. Prepared paint keys include theme revision. Consequently:

- source/funnel changes can rebuild semantic IR
- theme changes reuse semantic IR
- width changes reuse semantic IR
- theme and width changes can rebuild layout/paint products without invoking Markdown/diff/ANSI parsers

This is explicit in `application/content.rs:302-304` and the `SemanticProjectionKey`/`TextProjectionKey` split.

### 8.4 Structural policy versus application policy

`TextRenderPolicy` controls structural behavior:

- block and table gaps
- soft-break conversion
- table track sizing
- task/list marker arrangement
- code labels
- code wrapping

It does not directly encode colors or semantic styling (`render/policy.rs:46-50`).

Colors and role-specific styling belong to Theme:

- framework defaults are resolved through the framework theme resolver
- application overrides enter through `Theme::with_text_style`
- local `TextRun` styles are applied as local `StyleRef` overrides

Tests in `style.rs:739-842` and `render/tests.rs:475-498` verify framework defaults, application overrides, and local precedence.

### 8.5 Origin is generic and caller-extensible

The source has built-in origins for Markdown, plain text, ANSI, and diff, but `TextOrigin::new` accepts arbitrary names. Generic selectors can specialize by origin without hard-coding application meaning.

The annotation model also permits arbitrary caller namespaces. This agrees with the framework-boundary requirement that generic content may be namespaced by callers but must not encode product-specific meanings.

### 8.6 Direct and parsed diff are parallel routes

The two diff systems are architecturally distinct:

#### Parsed text diff

- `content/text/diff.rs`
- input: `Projection<TextContent>` containing raw unified diff text
- output: `Projection<TextContent>` containing paragraph blocks
- semantic representation: tags, styles, exact body runs, synthetic markers
- incremental and source-aware
- used by `TextFunnelKind::Diff`

#### Structured direct diff

- `content/diff/model.rs`
- input: validated `DiffHunk` values
- output: `View`
- semantic representation: typed ranges, coordinates, line kinds, terminations
- direct native ingress
- used by `View.diff` / native diff creation
- exposed through `binding::lower_diff_hunks`

The native direct route is proven by `crates/iyon-tui-native/src/tui/view_abi.rs:3123-3260`:

```text
words + bytes payload
    ↓ parse_and_build_diff
DiffRange / DiffLine / DiffHunk validation
    ↓
binding::lower_diff_hunks
    ↓
View publication
```

The parsed text route does not produce `DiffHunk`, and the structured route does not use `TextRenderer`.

### 8.7 Direct structured diff styling is hard-coded to generic theme keys

`content/diff/render.rs:33-85` uses:

- `diff.header`
- `diff.meta`
- `diff.context`
- `diff.addition`
- `diff.deletion`

This is generic diff policy rather than product policy. It resolves through Theme at paint time, as verified by tests at `diff/render.rs:186-258`.

### 8.8 Markdown source-origin stamping has a body exception

`MarkdownProjector::finish` stamps each emitted root block with `TextOrigin::MARKDOWN` (`markdown.rs:1250-1256`). `stamp_block_origin` recursively stamps most nested content but deliberately leaves code/raw literal bodies untouched (`origin.rs:211-215`).

Renderer context propagation compensates for this at render time. A consumer inspecting body-run annotations directly should not assume every nested run has a Markdown origin.

### 8.9 Source annotations are applied after parsing, not inside parsers

`project_semantic_snapshot` first parses semantic content, then applies `SourceAnnotationRewriter` if source annotations exist (`application/content.rs:723-729`).

This preserves a clean separation:

```text
source-store annotations
    ↓ post-projection rewriter
semantic run annotations/style intent
```

The rewriter uses provenance ranges:

- Exact ranges map byte-for-byte
- Derived ranges use proportional source-to-display splitting
- Synthetic runs are not source-annotated

This is a consequential seam for future consumers because caller annotations remain attached to semantic runs without being resolved into physical styles prematurely.

### 8.10 Content-to-presentation dependency is explicit

`content/text/render/*` imports:

- `crate::presentation::factory`
- `View`
- `TextSpan`
- `StyleRef`
- grid and layout types

This is an intentional semantic-to-presentation lowering layer, not a parser dependency. The parser and IR modules do not import terminal layout.

---

## 9. Open questions and coverage gaps

1. **External Rust visibility**
   - The semantic text module is private at `lib.rs:49`, while selected names are re-exported through `binding`.
   - The intended long-term public authoring surface appears to be the TypeScript/native facade, but this report does not census all generated TypeScript transport behavior.

2. **Exact style-neutrality contract**
   - `TextRun::style` is documented as host-independent semantic intent, but its concrete type is `crate::StyleRef`.
   - It is unresolved whether alternate hosts are expected to understand this type directly or whether it is intended to be extracted into a more abstract semantic style representation.

3. **Direct diff versus diff projector convergence**
   - The two diff routes share visual style-key names but have different semantic models and metadata fidelity.
   - No source evidence shows a planned unification in the current baseline.

4. **Direct projector replacement semantics**
   - Markdown, Diff, and ANSI continuation checks use source base/end and internal state.
   - Application-level `ContentLineage` resets parser instances on source replacement.
   - A direct caller using the projector APIs without application lineage could present a same-base replacement that looks like an append. The public projector contract does not itself carry a source revision identifier.

5. **RawText u32 length bound**
   - `RawText::new` casts `usize` byte length to `u32` without returning overflow.
   - The source-store payload limits may make this unreachable in normal operation, but the standalone constructor does not establish that bound through its type or error.

6. **Derived provenance mapping precision**
   - `SourceAnnotationRewriter::source_local_offset` uses proportional byte mapping for derived runs (`application/content.rs:1416-1426`).
   - This is deterministic but cannot represent exact character-level correspondence for arbitrary transformations.

7. **Code/raw body origin visibility**
   - Container origin is available through render context, but body-run annotations are not recursively stamped.
   - Consumers that inspect semantic annotations rather than rendered facts may observe different origin coverage.

8. **Markdown reference-cache lifetime**
   - `MarkdownProjector`'s internal `caches` vector is not bounded in the inspected implementation.
   - The application connector lifetime bounds it in practice, but a long-lived direct projector may retain an expanding set of source-domain products.

9. **No executed validation**
   - This report did not execute Rust tests or benchmarks.
   - The behavioral claims are based on source and test-code inspection.

10. **Test files not all read line-by-line**
    - Production files under `content/` were inspected comprehensively.
    - Test files were indexed exhaustively and selected key suites were inspected in detail; `render/structured.rs`, `render/tests.rs`, `markdown_composition.rs`, `pulldown_characterization.rs`, and `text_origin.rs` were not all read line-by-line in this session.

11. **No product/application consumers assumed**
    - The repository contains generic content/funnel names and TypeScript APIs, but this report does not infer application-specific meaning for them.

---

## 10. Evidence appendix

### 10.1 Primary source manifest inspected

#### Content roots

- `crates/iyon-tui/src/content/mod.rs`
- `crates/iyon-tui/src/content/render.rs`
- `crates/iyon-tui/src/content/diff/mod.rs`
- `crates/iyon-tui/src/content/diff/model.rs`
- `crates/iyon-tui/src/content/diff/render.rs`

#### Semantic text IR and metadata

- `crates/iyon-tui/src/content/text/mod.rs`
- `crates/iyon-tui/src/content/text/content.rs`
- `crates/iyon-tui/src/content/text/annotations.rs`
- `crates/iyon-tui/src/content/text/provenance.rs`
- `crates/iyon-tui/src/content/text/origin.rs`
- `crates/iyon-tui/src/content/text/inline.rs`
- `crates/iyon-tui/src/content/text/block.rs`
- `crates/iyon-tui/src/content/text/errors.rs`
- `crates/iyon-tui/src/content/text/source.rs`
- `crates/iyon-tui/src/content/text/validate.rs`
- `crates/iyon-tui/src/content/text/visit.rs`

#### Projectors

- `crates/iyon-tui/src/content/text/plain.rs`
- `crates/iyon-tui/src/content/text/markdown_options.rs`
- `crates/iyon-tui/src/content/text/markdown.rs`
- `crates/iyon-tui/src/content/text/diff.rs`
- `crates/iyon-tui/src/content/text/ansi.rs`

#### Styling and renderer

- `crates/iyon-tui/src/content/text/style.rs`
- `crates/iyon-tui/src/content/text/render/mod.rs`
- `crates/iyon-tui/src/content/text/render/block.rs`
- `crates/iyon-tui/src/content/text/render/inline.rs`
- `crates/iyon-tui/src/content/text/render/identity.rs`
- `crates/iyon-tui/src/content/text/render/policy.rs`

#### Test-only renderer modules

- `crates/iyon-tui/src/content/text/render/source_format.rs`
- `crates/iyon-tui/src/content/text/render/structured.rs`
- `crates/iyon-tui/src/content/text/render/tests.rs`
- `crates/iyon-tui/src/content/text/migrated_tests.rs`

#### Text test suites

- `crates/iyon-tui/src/content/text/tests/document_public.rs`
- `crates/iyon-tui/src/content/text/tests/markdown_composition.rs`
- `crates/iyon-tui/src/content/text/tests/markdown_hardening.rs`
- `crates/iyon-tui/src/content/text/tests/markdown_incremental.rs`
- `crates/iyon-tui/src/content/text/tests/markdown_smoke.rs`
- `crates/iyon-tui/src/content/text/tests/pulldown_characterization.rs`
- `crates/iyon-tui/src/content/text/tests/text_origin.rs`

### 10.2 Supporting source inspected

- `crates/iyon-tui/src/text.rs`
- `crates/iyon-tui/src/lib.rs`
- `crates/iyon-tui/src/application/content.rs`
- `crates/iyon-tui/src/binding/mod.rs`
- `crates/iyon-tui-native/src/content_ffi.rs`
- `crates/iyon-tui-native/src/tui/view_abi.rs`
- `packages/iyon-tui/src/api/content/text-content.ts`
- `packages/iyon-tui/src/api/content/diff.ts`
- `packages/iyon-tui/src/api/content/retained.ts`
- `packages/iyon-tui/src/api/content/annotations.ts`
- `packages/iyon-tui/src/api/content/projection.ts`
- `crates/iyon-tui/Cargo.toml`

### 10.3 Exact symbol references

#### IR and metadata

- `RawText`: `content/text/content.rs:7-89`
- `TextContent`: `content/text/content.rs:91-118`
- `BreakKind`: `content/text/inline.rs:5-10`
- `Mark`, `MarkSet`: `content/text/inline.rs:71-125`
- `InlineContent`: `content/text/inline.rs:127-214`
- `InlineKind`, `Inline`: `content/text/inline.rs:216-354`
- `Image`: `content/text/inline.rs:356-389`
- `BlockKind`, `Block`: `content/text/block.rs:410-581`
- `Table` validation: `content/text/block.rs:198-277`
- `TextProvenance`, `TextRun`, `LiteralText`: `content/text/provenance.rs:7-221`
- `SemanticTag`, `SemanticKey`, `SemanticValue`, `Annotations`: `content/text/annotations.rs:5-176`
- `TextOrigin`: `content/text/origin.rs:13-130`
- recursive origin stamping: `content/text/origin.rs:195-287`

#### Validation and rewriting

- `TextIrError`, `TextProjectionError`: `content/text/errors.rs:5-167`
- `validate_text_content`: `content/text/validate.rs:5-22`
- `validate_text_projection`: `content/text/validate.rs:24-61`
- nested validation: `content/text/validate.rs:63-152`
- `TextVisitor`: `content/text/visit.rs:10-91`
- `TextRewriter`: `content/text/visit.rs:93-157`
- `RewriteProjector`: `content/text/visit.rs:111-190`
- persistent rewrite helpers: `content/text/visit.rs:192-365`

#### Projectors

- `PlainTextProjector`: `content/text/plain.rs:9-143`
- `MarkdownOptions`: `content/text/markdown_options.rs:3-112`
- `MarkdownProjector`: `content/text/markdown.rs:78-264`
- Markdown cache/restart: `content/text/markdown.rs:266-462`
- Markdown domain parsing: `content/text/markdown.rs:552-588`
- Markdown event builder: `content/text/markdown.rs:590-1270`
- live table helpers: `content/text/markdown.rs:1272-1395`
- `DiffProjector`: `content/text/diff.rs:26-230`
- text diff classification: `content/text/diff.rs:159-229`
- `AnsiProjector`: `content/text/ansi.rs:22-120`
- ANSI state/parser: `content/text/ansi.rs:126-561`

#### Styling and renderer

- `TextRole`, `TextPart`: `content/text/style.rs:27-73`
- `TextSelector`: `content/text/style.rs:96-305`
- Theme integration: `content/text/style.rs:307-334`
- `TextFacts`: `content/text/style.rs:480-555`
- `TextRenderer`: `content/text/render/mod.rs:167-546`
- semantic sequence lowering: `content/text/render/mod.rs:213-398`
- raw/edge lowering caches: `content/text/render/mod.rs:482-544`
- block cache: `content/text/render/block.rs:12-83`
- block lowering: `content/text/render/block.rs:85-409`
- inline lowering: `content/text/render/inline.rs:10-114`
- inherited context/facts: `content/text/render/identity.rs:8-146`
- structural policy: `content/text/render/policy.rs:3-183`

#### Structured diff

- range and number types: `content/diff/model.rs:3-90`
- line kinds/termination/coordinates: `content/diff/model.rs:92-207`
- validation errors: `content/diff/model.rs:209-276`
- `DiffHunk`: `content/diff/model.rs:278-394`
- direct lowering: `content/diff/render.rs:5-87`

#### Application seam

- content snapshot/source projection: `crates/iyon-tui/src/application/content.rs:108-187`, `:611-688`
- semantic keys/cache: `application/content.rs:207-324`
- delivery state: `application/content.rs:354-421`
- connector execution: `application/content.rs:423-472`
- semantic projectors: `application/content.rs:690-730`
- renderer policy: `application/content.rs:732-744`
- semantic compilation: `application/content.rs:746-797`
- finalized-prefix proof: `application/content.rs:927-1000`
- source annotation rewriter: `application/content.rs:1258-1426`

### 10.4 Boundary references

- Internal text re-export: `crates/iyon-tui/src/text.rs:1-8`
- private crate-root text module and internal aliases: `crates/iyon-tui/src/lib.rs:48-104`
- selected native binding exports: `crates/iyon-tui/src/binding/mod.rs:18-31`, `:65-78`
- native structured diff parse/lower path: `crates/iyon-tui-native/src/tui/view_abi.rs:3123-3260`
- native direct diff tests: `crates/iyon-tui-native/src/tui/view_abi.rs:5575-5672`
- TypeScript diff counterpart: `packages/iyon-tui/src/api/content/diff.ts:1-103`
- TypeScript funnel kinds: `packages/iyon-tui/src/api/content/retained.ts:111-145`, `:471-495`

### 10.5 Commands/search methods

Read-only repository searches were performed using:

- recursive file discovery under `crates/iyon-tui/src/content/`
- symbol/reference search for:
  - `DiffProjector`
  - `DiffHunk`
  - `lower_diff_hunks`
  - `MarkdownProjector`
  - `PlainTextProjector`
  - `AnsiProjector`
  - `TextRenderer`
  - `SourceAnnotationRewriter`
- source line inspection for all assigned production modules
- test-function inventory and selected behavioral-test inspection

No source, configuration, generated artifact, dependency, or running service was changed.