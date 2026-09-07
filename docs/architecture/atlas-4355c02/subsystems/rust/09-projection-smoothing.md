# 09 — Projection composition and smoothing

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Primary scope: `crates/iyon-tui/src/projection/` recursively
- Assignment goal: projection composition, validation, incremental state, pacing, and clocks
- Parent-added atlas documentation was treated as investigation guidance, not as source-baseline implementation.

The projection module is deliberately generic. Its module documentation explicitly says it owns projection envelopes, stability/transition/relation validation, projector composition, and generic temporal publication, while not knowing about Markdown, text IR, History, Views, or terminal geometry (`crates/iyon-tui/src/projection/mod.rs:1-6`).

The framework boundary is important here: `iyon-tui` owns generic stream coordinates, semantic transformation, smoothing, pacing, and terminal presentation mechanics. No Iyon-agent/application semantics were found in the projection module itself.

### Evidence and method

Production projection files were inspected comprehensively:

- `crates/iyon-tui/src/projection/mod.rs`
- `crates/iyon-tui/src/projection/value.rs`
- `crates/iyon-tui/src/projection/validate.rs`
- `crates/iyon-tui/src/projection/projector.rs`
- `crates/iyon-tui/src/projection/compose.rs`
- `crates/iyon-tui/src/projection/smooth.rs`

Projection-owned tests and migrated tests were inspected:

- `crates/iyon-tui/src/projection/tests.rs`
- `crates/iyon-tui/src/projection/migrated_tests.rs`
- `crates/iyon-tui/src/projection/tests/p3c_ergonomics.rs`
- `crates/iyon-tui/src/projection/tests/projection_public.rs`

Supporting source paths were inspected to establish actual production consumers and clock ownership:

- `crates/iyon-tui/src/lib.rs`
- `crates/iyon-tui/src/stream/coord.rs`
- `crates/iyon-tui/src/application/content.rs`
- `crates/iyon-tui/src/application/host.rs`
- `crates/iyon-tui/src/content/text/plain.rs`
- Projection-related implementations in Markdown, Diff, ANSI, and text visitor modules.
- `packages/iyon-tui/src/api/content/projection.ts`
- `packages/iyon-tui/src/index.ts`
- Relevant historical L1 reports, especially `reports/pre-v5-l1/L1-09-report.md`, `L1-10-report.md`, and `L1-13-report.md`, for historical context only.

No source files were modified. No test or benchmark command was executed in this investigation, so all behavioral claims below are based on source inspection and existing test assertions. Historical reports contain command results from earlier work, but those are reported as historical evidence rather than as commands run for this report.

### Key current-state conclusion

The repository contains two related but materially different projection layers:

1. **Rust projection algebra and smoothing**:
   - Strong root-coordinate envelope invariants.
   - Generic typed projector composition.
   - Explicit stable-frontier and sealed-state contracts.
   - Stateful temporal smoothing with absolute `Instant` deadlines.
   - Rust module is crate-private in production.

2. **Public TypeScript projection-shaped API**:
   - A separate, lightweight `Projection`, `ProjectionBuilder`, and `Smooth`.
   - Less restrictive span validation.
   - No visible connection to the Rust projection trait or scheduler in the inspected repository.
   - `Smooth` is merely a value object containing `through`, not a temporal state machine.

This divergence is a current architecture fact and should not be silently treated as Rust/TypeScript parity.

---

## 1. Responsibility and structure

### 1.1 Module inventory

| File | Physical LOC | Production/test | Primary responsibility |
|---|---:|---|---|
| `projection/mod.rs` | ~29 | Production | Module declarations and re-exports |
| `projection/value.rs` | ~278 | Production | `Projection<T>`, `ProjectionSpan<T>`, `ProjectionBuilder<T>`, mapping and append helpers |
| `projection/validate.rs` | ~231 | Production | Construction, relationship, and transition validators |
| `projection/projector.rs` | ~52 | Production | `Projector<Input>` trait and `ProjectorExt::then` |
| `projection/compose.rs` | ~105 | Production | Two-stage `Then<A, B>` composition and `ThenError` |
| `projection/smooth.rs` | ~438 | Production | Temporal stable-span publication, pacing, deadline state |
| `projection/tests.rs` | ~789 | Tests | Core construction, relation, transition, composition, and smoothing tests |
| `projection/migrated_tests.rs` | ~15 | Tests | Includes migrated in-crate ergonomic/public tests |
| `projection/tests/p3c_ergonomics.rs` | ~60 | Tests | Builder/mapping/default configuration ergonomics |
| `projection/tests/projection_public.rs` | ~194 | Tests | External-consumer-shaped Rust API tests |
| **Total** | **~2,191** |  |  |

Counts are approximate physical line counts based on source line numbering and include comments and blank lines. There are approximately **1,133 production lines** and **1,058 test lines** under the projection subtree. No generated source was found under this subtree.

### 1.2 `mod.rs`

`projection/mod.rs` declares:

```rust
mod compose;
mod projector;
mod smooth;
mod validate;
mod value;
```

The production re-exports are:

- `Then`, `ThenError`
- `Projector`
- `Smooth`, `SmoothConfig`, `SmoothConfigError`
- `ProjectionRelationError`
- `ProjectionTransitionError`
- `ProjectionValidationError`
- `validate_projection_relation`
- `validate_projection_transition`
- `Projection`
- `ProjectionBuilder`
- `ProjectionSpan`

The internal construction validator is re-exported as `pub(crate)` only (`mod.rs:19-29`).

### 1.3 `Projection` value layer

`value.rs` contains three central types:

- `Projection<T>` (`value.rs:12-18`)
- `ProjectionSpan<T>` (`value.rs:22-25`)
- `ProjectionBuilder<T>` (`value.rs:29-35`)

A `Projection<T>` stores:

```text
source_base
stable_through
source_end
sealed
spans: Vec<ProjectionSpan<T>>
```

Each `ProjectionSpan<T>` has:

```text
source: StreamRange
values: Vec<T>
```

The source interval is always represented in root stream coordinates. A span may contain zero values, which represents explicit elision, not an uncovered gap (`value.rs:6-10`).

The builder is the intended construction boundary:

- `new(...)`
- `emit(...)`
- `emit_many(...)`
- `elide(...)`
- `finish()`

`finish()` materializes the projection and invokes the internal validator before returning it (`value.rs:80-90`).

### 1.4 Validation layer

`validate.rs` separates three contracts:

1. **Projection construction**:
   - Is one projection internally well-formed?
2. **Projection relationship**:
   - Does an output account for a valid prefix of its input?
3. **Projection transition**:
   - Is a later snapshot a valid monotonic update of a previous snapshot?

The separation is useful because a projection can be internally valid while still violating its relationship to another projection or violating a successive-snapshot transition rule.

### 1.5 Projector layer

`projector.rs` defines the generic transformation contract:

```rust
pub trait Projector<Input> {
    type Output;
    type Error;

    fn project(
        &mut self,
        input: &Projection<Input>,
    ) -> Result<Projection<Self::Output>, Self::Error>;
    ...
}
```

A projector is mutable and may retain local parsing or transformation state. There are deliberately no `Send`, `Sync`, or storage bounds (`projector.rs:8-19`).

The trait also exposes two temporal/incremental hooks:

- `restart_from(output_from)`:
  - Returns a conservative root-coordinate restart point.
  - Default is identity.
  - Contract requires `restart_from(X) <= X` (`projector.rs:21-28`).

- `next_wakeup()`:
  - Returns an absolute `Instant` at which temporal state may change.
  - Default is `None` (`projector.rs:30-33`).

- `advance(now)`:
  - Receives a caller-supplied clock.
  - Returns whether the owner should rerun `project`.
  - Default is `false` (`projector.rs:35-39`).

`ProjectorExt` provides typed composition with `.then(...)`, but the production module only re-exports that extension trait under `#[cfg(test)]` (`mod.rs:21-22`). The trait itself exists in `projector.rs:42-52`.

### 1.6 Smooth layer

`smooth.rs` implements temporal publication of upstream-stable spans without changing values. Its explicit behavior is:

- Only spans ending at or before the input stable frontier are eligible.
- Spans are atomic; Smooth never splits a span.
- Pacing weight is `values.len()`.
- Source coordinates and display width do not determine pacing (`smooth.rs:158-162`).

`Smooth` stores the temporal and queue state needed to pace publication:

```text
config
published_end
input_sealed
input_base
input_stable
input_end
queued_through
pending: VecDeque<PendingSpan>
pending_units
credit_units
last_advance
next_wakeup
episode_active
```

(`smooth.rs:164-178`)

### 1.7 Production integration outside the primary subtree

The projection module is consumed by:

- `content/text/plain.rs`
- `content/text/markdown.rs`
- `content/text/diff.rs`
- `content/text/ansi.rs`
- `content/text/visit.rs`
- `application/content.rs`

The application layer does not generally compose these semantic projectors using `Then`. Instead, `application/content.rs` chooses a parser/projector based on `TextFunnelKind` and stores parser state in `ConnectorExecution` (`content.rs:427-454`, `content.rs:690-730`).

Smoothing is integrated through `ConnectorDelivery`, also in `application/content.rs`, rather than by inserting `Smooth` into the semantic parser chain. `ConnectorDelivery` owns a `Smooth`, a grapheme projection, source indexing metadata, and a candidate delivery frontier (`content.rs:354-421`).

---

## 2. Types, APIs and contracts

### 2.1 Stream coordinate foundation

`StreamOffset` and `StreamRange` are the coordinate primitives used by projections.

`StreamOffset` is a `u64` newtype (`crates/iyon-tui/src/stream/coord.rs:9-36`).

`StreamRange` is a half-open `[start, end)` range in root source coordinates (`coord.rs:38-51`). Text-specific consumers use UTF-8 byte offsets, while generic stream consumers may use event or record ordinals.

`StreamRange::new` asserts that `start <= end` (`coord.rs:46-50`). Thus reversed ranges panic at construction rather than becoming a recoverable projection-validation error.

### 2.2 `Projection<T>` invariants

The internal validator enforces these invariants (`validate.rs:71-118`):

1. `source_base <= stable_through <= source_end`.
2. A sealed projection has `stable_through == source_end`.
3. An empty source interval has no spans.
4. Every nonempty source interval has at least one span.
5. Every span is nonempty.
6. The first span starts exactly at `source_base`.
7. Spans are contiguous and non-overlapping.
8. No span extends beyond `source_end`.
9. The final span ends exactly at `source_end`.
10. A non-edge stable frontier must equal a span end; it cannot lie inside a span.

The explicit elision model is consequential: a projection can account for source bytes while emitting no values by storing an empty-value span. This lets downstream stages distinguish “accounted but intentionally omitted” from “missing source coverage.”

### 2.3 Builder API

`ProjectionBuilder::new` takes the entire source envelope:

```rust
ProjectionBuilder::new(
    source_base,
    stable_through,
    source_end,
    sealed,
)
```

`emit` adds one value to a source interval (`value.rs:54-62`).

`emit_many` adds any number of values (`value.rs:64-71`).

`elide` is a convenience wrapper around `emit_many(source, [])` (`value.rs:74-78`).

The builder does not enforce ordering incrementally. Invalid order, gaps, overlap, trailing coverage, or frontier placement are rejected only by `finish()`.

### 2.4 Append-only value mutation helper

`Projection::append_span` and `append_span_many` are crate-private (`value.rs:93-126`).

They are intended for append-only producers:

- Require the appended span to begin exactly at the current `source_end`.
- Assert contiguity rather than return a structured error.
- Assert that the span does not reverse.
- Push the span.
- Advance `source_end`.
- Set `stable_through` equal to the new `source_end`.
- Revalidate.

A source-wide search found no current production or test caller other than the definitions themselves. This is an available append-oriented API, but it is not an active path in the baseline.

A subtle implementation consequence is that `append_span_many` mutates `self.spans`, `source_end`, and `stable_through` before calling `validate_projection(self)`. If validation fails after those mutations, the caller retains the mutated object. The contiguity/reversal assertions also panic rather than returning `ProjectionValidationError`. Because the method is crate-private and currently unused, this is latent behavior rather than an observed active failure route.

### 2.5 Mapping APIs

`Projection<T>` offers:

- `map_ref`
- `try_map_ref`
- `map_spans`
- `try_map_spans`
- `map`

(`value.rs:182-266`)

All mapping forms preserve:

- Source base.
- Stable frontier.
- Source end.
- Sealed bit.
- Span source ranges.

`map_ref` and `map` transform individual values. `map_spans` allows a span-aware closure to produce zero or more values while retaining exactly one source span. Mapping does not alter source coverage or segmentation.

The fallible `try_map_*` variants rebuild through a builder and use `expect` after reconstruction, relying on the premise that a valid source projection and unchanged source spans remain valid.

### 2.6 Accessors and encapsulation

Projection fields are `pub(crate)`, while accessors are public within the module:

- `source_base()`
- `stable_through()`
- `source_end()`
- `is_sealed()`
- `spans()`

`ProjectionSpan` exposes:

- `source()`
- `values()`

(`value.rs:147-180`, `value.rs:268-277`)

This keeps the source envelope and span storage structurally private while allowing consumers to inspect the validated representation.

### 2.7 Relationship contract

`validate_projection_relation(input, output)` enforces a prefix relationship (`validate.rs:121-141`):

- Same `source_base`.
- Output `source_end <= input.source_end`.
- Output `stable_through <= input.stable_through`.
- A sealed output requires a sealed input.
- A sealed output must reach the input `source_end`.

It does **not** compare values or segmentation between input and output. That is intentional: projection stages are allowed to transform values, merge or split output values within their source accounting, and use different span segmentation, provided the output remains a valid source-coordinate prefix.

The relation contract permits lagging outputs. A stage can expose only a stable prefix of a still-growing input.

### 2.8 Transition contract

`validate_projection_transition(previous, next)` enforces monotonic snapshot evolution (`validate.rs:144-180`):

- A sealed previous projection can only transition to an identical sealed projection.
- `source_base` cannot regress.
- A compacted `next.source_base` cannot move past `previous.stable_through`.
- `source_end` cannot regress.
- `stable_through` cannot regress.
- A newly sealed projection must have stable frontier equal to source end.
- Values and segmentation over the previously stable overlap must remain unchanged.

The stable-prefix comparison is implemented by `same_stable_projection` (`validate.rs:183-230`). It locates the first span at or after the overlap start using `partition_point`, then checks contiguous source ends and values across the overlap.

The test comment says clipping the first span permits compaction inside a span (`tests.rs:212-214`), but the implementation still compares the complete first span’s values. It clips source-end comparison at the overlap boundary, while preserving the first span’s full value equality. This is consistent with the intended atomic-span model: a compaction boundary may fall inside an existing source span, but the values associated with the overlapping span must remain equal.

Transition validation is not automatically invoked by `Projector::project` or by `Smooth`. It is an explicit caller/test-level check. A source search found transition calls in projection tests and text incremental tests, but no production call in the runtime path.

### 2.9 `Projector<Input>`

The trait intentionally supports stateful projectors:

```text
Input projection
      |
      v
&mut Projector::project
      |
      v
Output projection
```

The `&mut self` receiver is important for:

- Markdown parser checkpoints and restart state.
- Diff parser incremental line/hunk state.
- ANSI escape parser state.
- Smooth queue/credit/deadline state.

No thread-safety constraints are imposed. The tests deliberately instantiate a projector containing `Rc<RefCell<_>>` (`tests.rs:731-766`, `projection_public.rs:120-128`).

### 2.10 `Then<A, B>`

`Then<A, B>` is a statically typed pair (`compose.rs:8-18`):

```rust
Then {
    first: A,
    second: B,
}
```

The `Projector<Input>` implementation requires:

```text
A: Projector<Input>
B: Projector<A::Output>
```

and produces `B::Output` (`compose.rs:66-72`).

`ThenError<A, B>` distinguishes four failure locations:

- `First(A)`
- `FirstRelation(ProjectionRelationError)`
- `Second(B)`
- `SecondRelation(ProjectionRelationError)`

(`compose.rs:20-27`)

The `ThenError` `Display` implementation preserves that distinction (`compose.rs:44-57`). Its `Error` implementation is available when both stage errors implement `std::error::Error` (`compose.rs:59-64`).

### 2.11 `SmoothConfig`

Defaults are:

- Tick interval: 16 ms.
- Spring: `2.0`.
- Minimum rate: 20 projected values/second.
- Maximum rate: 800 projected values/second.

(`smooth.rs:12-35`)

The configuration fields are private. Callers construct or modify configuration through:

- `SmoothConfig::new()`
- `SmoothConfig::try_from_parts(...)`
- `with_tick_interval(...)`
- `with_spring(...)`
- `with_unit_rates(...)`

Validation rejects:

- Zero tick interval.
- Non-finite spring.
- Negative spring.
- Non-finite rates.
- Negative rates.
- Minimum rate greater than maximum.
- A maximum rate of zero.
- No possible progress when spring and minimum rate are both zero.

(`smooth.rs:37-47`, `smooth.rs:124-148`)

`Smooth::new(config)` itself does not call `config.validate()`. Because configuration fields are private and the supported construction/mutation methods validate, ordinary external construction remains constrained to valid configurations. Internally, however, `Smooth::new` trusts its input.

---

## 3. Dependency and ownership map

### 3.1 Projection module dependency graph

```text
stream::StreamOffset / StreamRange
                 |
                 v
        Projection<T>
                 |
       ProjectionBuilder<T>
                 |
        validate_projection
                 |
      +----------+-----------+
      |                      |
      v                      v
 Projector<Input>       validate relation /
      |                  transition
      v
   Then<A,B>
      |
      v
    Smooth
```

The projection files depend only on:

- `std::time::{Duration, Instant}`
- `std::collections::VecDeque`
- `crate::stream::{StreamOffset, StreamRange}`

They do not depend on text, rendering, History, View, terminal backend, or native transport.

### 3.2 Runtime ownership graph

```text
Host
 └── ContentHostRegistry
      └── ConnectorRecord
           ├── ConnectorExecution
           │    ├── MarkdownProjector / DiffProjector / AnsiProjector
           │    ├── TextRenderer
           │    └── ConnectorDelivery
           │         ├── Smooth
           │         └── grapheme Projection<TextContent>
           ├── semantic projection cache
           ├── prepared paint cache
           ├── projection cache
           ├── candidate_projection
           └── committed_projection
```

`application/content.rs` owns the integration objects. Projection values own their `Vec` storage and values. `Smooth` owns only queue and clock state.

### 3.3 Ownership and lifetime

- `Projection<T>` owns its spans and values.
- `ProjectionSpan<T>` owns its value vector.
- `ProjectionBuilder<T>` owns an in-progress set of spans until `finish`.
- `Then<A, B>` owns both projector stages and drops them normally with Rust ownership.
- `Smooth` owns `PendingSpan` metadata and temporal state, but does not retain source values in its pending queue.
- `Smooth::project` clones eligible values from the input projection into a new output projection.
- `ConnectorExecution` owns mutable parser and delivery state for one Connector.
- `HostContentProjection` is retained through `Arc` and captures immutable layout/paint products and a theme snapshot.
- Candidate and committed projection products are separately retained in `ConnectorRecord`.

### 3.4 Reverse edges

The projection module is consumed by:

```text
content/text/plain.rs       -> Projection, Builder, Projector
content/text/markdown.rs    -> Projection, Builder, Projector
content/text/diff.rs        -> Projection, Builder, Projector
content/text/ansi.rs        -> Projection, Builder, Projector
content/text/visit.rs       -> Projection, Projector
application/content.rs      -> Projection, Builder, Projector, Smooth
projection tests            -> all public projection symbols
```

The actual application semantic path selects one parser branch rather than building a `Then` chain:

```text
raw Projection<TextContent>
          |
          +-- PlainTextProjector
          +-- MarkdownProjector
          +-- DiffProjector
          +-- AnsiProjector
          |
          v
semantic Projection<TextContent>
          |
          v
optional SourceAnnotationRewriter
          |
          v
TextRenderer / layout / paint
```

This dispatch is visible at `application/content.rs:690-730`.

### 3.5 Creation and destruction

Projection values are created whenever a caller calls `ProjectionBuilder::finish`. There is no explicit destruction API; Rust ownership releases them.

Connector projection state is created lazily for active/smoothed Connectors:

- `ConnectorExecution::new` constructs parser options and optional `ConnectorDelivery` (`application/content.rs:439-454`).
- `sync_connector_deadline` creates execution when a smooth Connector first needs delivery state (`content.rs:3741-3744`).
- Inactive Connector transitions clear execution and all derived caches (`content.rs:3136-3148`, `content.rs:6047-6060`).
- Connector removal drops the record and associated Arc-backed products.

---

## 4. Execution paths and state transitions

### 4.1 Projection construction path

```text
caller chooses source envelope
        |
        v
ProjectionBuilder::new
        |
        +-- emit / emit_many / elide
        |
        v
ProjectionBuilder::finish
        |
        v
validate_projection
        |
        +-- Err(ProjectionValidationError)
        |
        +-- Ok(Projection<T>)
```

The builder is the primary safe construction boundary. Direct `Projection` field construction is inaccessible outside the module because fields are `pub(crate)`.

### 4.2 Semantic text projection path

The application creates a raw projection from immutable Source snapshot chunks:

```text
HostContentSourceSnapshot
        |
        v
source_projection
        |
        v
Projection<TextContent>
        |
        v
selected Plain / Markdown / Diff / ANSI projector
        |
        v
semantic Projection<TextContent>
        |
        +-- optional SourceAnnotationRewriter
        |
        v
semantic product cache / layout
```

`source_projection` uses page-backed `RawText` values over source chunk views (`application/content.rs:611-629`). It sets the raw projection stable frontier and source end to the snapshot source end, with the snapshot sealed bit.

The application uses a second source-rooted projection for grapheme delivery (`content.rs:631-688`). It scans valid UTF-8 chunks, joins temporary cross-chunk text, segments with `unicode_segmentation::UnicodeSegmentation::grapheme_indices(true)`, and emits one `TextContent` value per grapheme.

The grapheme path retains the final grapheme of each temporary combined chunk as carry, allowing graphemes spanning Source chunk boundaries to be segmented atomically. The temporary combined buffer is then cleared (`content.rs:640-678`).

### 4.3 Stateful semantic projectors

`PlainTextProjector` validates the input text projection, groups consecutive raw spans into literal paragraphs, and calculates a stable frontier based on complete raw domains and stable barriers (`content/text/plain.rs:20-75`, `plain.rs:105-140`).

Markdown, Diff, and ANSI projectors retain parser state in `ConnectorExecution`:

```rust
markdown: Option<MarkdownProjector>,
diff: Option<DiffProjector>,
ansi: Option<AnsiProjector>,
parser_lineage: Option<ContentLineage>,
```

(`application/content.rs:427-437`)

The parser lineage uses:

```text
source_id
source_generation
content_generation
```

(`content.rs:190-205`)

When the lineage changes, parser state is reset while Smooth state and renderer reuse follow separate policies (`content.rs:457-471`).

### 4.4 Smooth first-use and input acceptance

`ConnectorDelivery::new` starts with:

- An empty `Projection<TextContent>`.
- Sentinel indexing generation/revision values.
- Zero candidate frontier.
- A fresh `Smooth`.

(`application/content.rs:367-383`)

`accept_input(snapshot)` compares only:

- `source_generation`
- `revision`
- `sealed`

If none changed, it performs no reindexing (`content.rs:386-392`).

If changed:

1. Build a full grapheme projection from the Source snapshot.
2. Call `self.smoother.project(&units)`.
3. Replace `self.units`.
4. Record current generation/revision/sealed state.
5. Copy Smooth’s published frontier into `candidate_frontier`.

(`content.rs:393-401`)

This separates input acceptance from temporal advancement. It prevents pure ticks from rescanning Source bytes, although each changed Source revision still rebuilds the entire grapheme projection.

### 4.5 Smooth update path

`Smooth::project` invokes `update(input)` and then returns the current published output (`smooth.rs:422-429`).

`update`:

1. Determines whether the previous episode was caught up using `episode_active`.
2. Records the input sealed state.
3. Moves `published_end` forward to a new source base if needed.
4. Clamps `published_end` down if the input source end regressed.
5. Rebuilds pending spans from the current published frontier.
6. If input is sealed:
   - Publishes all input.
   - Clears pending queue, credit, and deadlines.
7. If a newly arrived episode has pending work:
   - Releases the initial immediate span(s).
   - Marks the episode active if work remains.
8. If no pending work remains:
   - Clears deadlines and temporal state.
9. Otherwise leaves the episode active.

(`smooth.rs:373-407`)

The initial publication policy is span-granular, not value-granular. A zero-weight elision span may be released for free, followed by at most one weighted span in `release_immediate` (`smooth.rs:290-303`).

### 4.6 Smooth pending queue

`rebuild_pending` uses `Projection::spans_from(self.published_end)`, which skips the prefix with `partition_point` (`value.rs:172-180`, `smooth.rs:305-336`).

It queues only spans:

- Not already fully published.
- Ending at or before `input.stable_through`.

Pending weight is `span.values().len()`.

The queue stores only:

```text
source range
weight
```

not the values themselves (`smooth.rs:152-156`). Values remain in the input projection and are copied only when an output projection is materialized.

`queued_through` is maintained in `rebuild_pending`, but a source search found no read of this field after assignment. It is currently bookkeeping without an observed consumer.

### 4.7 Temporal advancement

`Smooth::advance(now)` has three major phases.

#### Idle/no-work phase

If the input is sealed or there is no pending work:

- Clear deadline.
- Clear last-advance timestamp.
- Clear credit.
- Clear episode-active state.
- Return `false`.

(`smooth.rs:237-243`)

#### Clock initialization phase

If pending work exists but no deadline has been initialized:

- Set `last_advance = Some(now)`.
- Set `next_wakeup = now + tick_interval`.
- Return `false`.

(`smooth.rs:245-248`)

This means the first scheduler observation establishes the clock; elapsed time before that observation is not credited.

`ensure_clock(now)` provides equivalent initialization for the application scheduler and deliberately does not rebase an existing clock (`smooth.rs:216-225`).

#### Due tick phase

If `now` reaches the deadline:

1. Calculate elapsed time from `last_advance`.
2. Compute rate:

```text
rate = clamp(
    pending_units * spring,
    min_units_per_second,
    max_units_per_second,
)
```

3. Add `elapsed_seconds * rate` to `credit_units`.
4. Release complete atomic spans while credit covers each span’s weight.
5. Clear all temporal state if queue drains.
6. Otherwise schedule the next deadline at `now + tick_interval`.
7. Return whether `published_end` advanced.

(`smooth.rs:254-274`)

Credit accumulates across ticks. This is why a large atomic span can eventually release even if the per-tick credit is initially insufficient.

### 4.8 Smooth output paths

`Smooth::output(input)` returns a projection from the input source base to the latest fully released span boundary (`smooth.rs:338-340`).

`output_from(input, from)`:

- Clamps output end to `min(published_end, input.source_end)`.
- Falls back to a full-prefix output if `from` is invalid or begins inside a span.
- Includes only complete spans ending at or before the published boundary.
- Constructs a projection whose:
  - `source_base = from`
  - `stable_through = effective_end`
  - `source_end = effective_end`
  - `sealed` only if input is sealed and the output reaches input source end.
- Clones values from eligible spans.

(`smooth.rs:342-370`)

`project_incremental` is a crate-private helper that calls `update` and then `output_from(input, from)` (`smooth.rs:409-419`). Its documentation says it is intended to return only newly published source spans for a caller-owned retained prefix. A repository-wide source search found no current caller. The active application path uses `ConnectorDelivery` and cached visibility rows instead.

### 4.9 Application delivery/reveal path

`ConnectorDelivery::reveal_units()` binary-searches the grapheme span list and returns the number of complete grapheme spans whose source ends are at or before Smooth’s published frontier (`application/content.rs:416-420`).

`project_text_snapshot` then uses this count to derive visible bounds from a cached `VisibilityIndex`:

```text
Smooth published frontier
        |
        v
reveal_units()
        |
        v
VisibilityIndex::reveal_bounds(...)
        |
        v
intrinsic size / visible rows / cut
```

(`application/content.rs:1206-1228`)

This is a significant distinction:

- Generic `Smooth` materializes cloned projection values.
- Application Smooth delivery uses Smooth primarily for frontier/clock state, then reveals rows from retained physical products without rebuilding semantic IR or cloning the whole surface.

### 4.10 Candidate versus committed delivery state

`ConnectorRecord` stores both:

```text
candidate_delivery_frontier
committed_delivery_frontier
```

(`application/content.rs:3012-3015`)

A delivery tick:

- Advances the mutable Smooth state.
- Updates `candidate_delivery_frontier`.
- Increments `delivery_revision`.
- Clears `candidate_projection`.
- Leaves semantic and prepared paint caches intact.
- Emits `ContentDirtyReason::DeliveryVisibility`.

(`content.rs:3646-3692`)

The candidate frontier is not immediately visible to readback consumers. Candidate projection and frontier are promoted only with the successful frame commit. Aborting a candidate restores the candidate frontier to the committed frontier (`content.rs:5134-5156`).

This was specifically hardened for delayed backend receipts. `PreparedContentConnector` captures the exact candidate `Arc` and frontier at preparation time, so a later tick or Source replacement cannot cause an older receipt to publish newer mutable state. Historical L1-13 documentation records this correction, and the current source retains the corresponding candidate/committed separation.

### 4.11 Clock ownership and scheduler path

The host owns the authoritative application clock:

```text
HostRunning::now
        |
        v
ContentHostRegistry::advance(now)
        |
        v
ConnectorDelivery::advance(now)
        |
        v
Smooth::advance(now)
```

`HostInner::advance_and_render` invokes `self.content.advance(self.now)` before normal frame preparation (`application/host.rs:2275-2285`).

`ContentHostRegistry::advance`:

1. Stores `authoritative_clock = Some(now)`.
2. If there are no active deadlines, resynchronizes currently active Connector IDs.
3. Selects only Connector IDs with `deadline <= now`.
4. Advances only those due Connectors.
5. Reindexes their next deadlines.
6. For each progressed Connector, updates candidate frontier, increments `delivery_revision`, clears candidate projection, and emits delivery dirty state.

(`application/content.rs:3586-3695`)

The scheduler does not scan every Connector for every tick. It maintains:

- `active_deadlines: HashMap<u64, Instant>`
- `active_connectors: HashSet<u64>`
- Reused scratch vectors for synchronization and due IDs

(`content.rs:3240-3250`)

`ContentHostRegistry::next_wakeup()` returns the minimum active deadline (`content.rs:3698-3700`).

`HostHandle::next_wake_ms()` combines:

- Running component deadline.
- Content delivery deadline.

It computes the minimum and converts it relative to `HostInner::now` (`application/host.rs:1243-1259`).

For asynchronous waiting, the host caps the wait at 16 ms (`host.rs:1519-1534`). For deterministic headless operation, `advance_time` increments the host clock directly (`host.rs:1405-1409`).

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic-operation to implementation-path matrix

| Semantic operation | Production path | Selection/condition | Failure/recovery behavior |
|---|---|---|---|
| Construct a projection | `ProjectionBuilder::new` → `emit`/`emit_many`/`elide` → `finish` | All Rust projection construction | Returns `ProjectionValidationError`; reversed ranges panic in `StreamRange::new` |
| Append one projection span | `Projection::append_span` / `append_span_many` | Crate-private append-only helper; no current caller found | Assertions for contiguity/reversal; validation error after mutation |
| Transform individual values | `Projection::map`, `map_ref`, `try_map_ref` | Caller chooses value transformation | Fallible variants return closure error; rebuilt validity is asserted |
| Transform span values | `map_spans`, `try_map_spans` | Caller chooses span-aware transformation | Span source coverage remains unchanged |
| Run one semantic projector | `Projector::project` | Plain/Markdown/Diff/ANSI/text visitor route | Stage-specific error propagates |
| Compose projectors | `ProjectorExt::then` → `Then::project` | Primarily tests and generic text-composition tests; no active application `.then` route found | `ThenError::{First, FirstRelation, Second, SecondRelation}` |
| Validate output/input relationship | `validate_projection_relation` | Explicit caller; enforced automatically inside `Then` | Returns relation error; no fallback |
| Validate successive snapshots | `validate_projection_transition` | Explicit tests/text incremental checks; no automatic production invocation found | Returns monotonicity/stable-prefix error |
| Smooth current projection | `Smooth::project` → `update` → `output` | Generic caller | Infallible output type; internal `expect` assumes preserved coverage |
| Advance Smooth clock | `Smooth::advance(now)` | Caller scheduler supplies absolute `Instant` | Returns `bool` only; no error channel |
| Return Smooth deadline | `Smooth::next_wakeup` | Scheduler polling | `None` when sealed, idle, or fully drained |
| Application grapheme indexing | `source_grapheme_projection` | Source revision/generation/sealed state changed | Invalid UTF-8 is treated as an internal invariant violation because Source guarantees valid UTF-8 |
| Application delivery reveal | `ConnectorDelivery::advance` + `reveal_units` + `VisibilityIndex` | Active smoothed Connector and due deadline | Candidate frontier advances; committed frontier waits for frame commit |
| Application projection cache hit | `cached_projection` | Exact `TextProjectionKey` match | Reuses immutable Arc product; deadline still synchronized |
| Application semantic cache hit | `resolve_cached_semantic` | Exact source/funnel semantic key | Avoids parser rebuilds on theme/width/delivery changes |
| Application same-key failure | `projection_failure_is_recorded` | Same projection key failed previously | Returns `PROJECTION_RETRY_BLOCKED` until source/control state changes |
| Application new Source revision after failure | `source_subscription_is_live` | New revision exceeds failed revision for inactive requested Connector | Clears retryable error and re-admits activation |
| Scheduler Connector lock failure | `ContentHostRegistry::advance` | Due Connector lock poisoned | Removes Connector from active indexes and returns typed scheduler failure |
| Host wait deadline | `HostHandle::next_wake_ms` | Running or content deadline exists | Uses minimum deadline; lock failure returns fallback 80 ms |

### 5.2 Composition failure semantics

`Then::project` is strictly ordered:

```text
first.project(input)
        |
        +-- First(error) -> stop
        |
        v
validate(input, middle)
        |
        +-- FirstRelation(error) -> stop
        |
        v
second.project(middle)
        |
        +-- Second(error) -> stop
        |
        v
validate(middle, output)
        |
        +-- SecondRelation(error) -> stop
        |
        v
Ok(output)
```

(`compose.rs:74-82`)

There is no fallback to an earlier output or to the unprojected input. Tests verify each of the four error variants (`tests.rs:588-613`).

### 5.3 Construction failure semantics

Construction validation is explicit and precise:

- `InvalidFrontier`
- `EmptySpan`
- `FirstSpanDoesNotStartAtBase`
- `GapOrOverlap`
- `SpanBeyondSourceEnd`
- `TrailingUncoveredSource`
- `StableFrontierInsideSpan`
- `SealedBeforeStableEnd`

(`validate.rs:9-21`)

The construction test covers empty projections, empty spans, missing first coverage, overlap, valid elision, and stable-frontier boundaries (`tests.rs:96-157`).

### 5.4 Smooth failure and fallback behavior

`Smooth` has an `Infallible` projector error type (`smooth.rs:422-424`). It trusts the input projection’s established validity and uses `expect("Smooth output must preserve input coverage")` after rebuilding output (`smooth.rs:368-370`).

There is one defensive alternate route in `output_from`: if the requested incremental start is outside the valid published range or starts inside a span, it falls back to a full output from the input source base (`smooth.rs:342-353`). This is a recovery path for an invalid/incompatible incremental request, not a semantic transformation fallback.

### 5.5 Application projection failure behavior

The application layer wraps semantic projection and layout failures in `anyhow::Error`, records a structured `ContentConnectorError`, and remembers the failed source revision and projection key (`application/content.rs:4117-4148`).

The same key is blocked from immediate repeated retries (`content.rs:3991-3997`). Successful cache hits clear older failures for different keys (`content.rs:3999-4011`). A later Source revision clears a retryable inactive activation error (`content.rs:6415-6426`).

No unprojected-text fallback was found in the current `project_semantic_snapshot` path. The selected parser error is propagated (`content.rs:690-730`).

### 5.6 Alternate smoothing route: application versus generic Smooth

The generic `Smooth::project` route clones output values into a newly built projection.

The application route does not use Smooth as the final row/value product. Instead:

```text
Source snapshot
   -> grapheme projection
   -> Smooth queue/deadline/frontier
   -> cached semantic/paint product
   -> VisibilityIndex reveal bounds
   -> candidate HostContentProjection
```

This is a legitimate specialization rather than a duplicate semantic implementation: application delivery needs physical row visibility and candidate/commit behavior, while generic Smooth exposes a reusable projection algebra.

### 5.7 Public Rust versus public TypeScript route

Rust projection is not externally public from the production crate:

- `lib.rs` declares `pub(crate) mod projection` (`lib.rs:37-40`).
- Root projection re-exports are `pub(crate)` and mostly test-visible (`lib.rs:77-78`, `lib.rs:105-110`).
- `ProjectorExt` is only re-exported from `projection/mod.rs` under `#[cfg(test)]`.

The migrated test comment explicitly says the tests retain old semantic assertions “without preserving an external Rust authoring facade” (`projection/migrated_tests.rs:1-3`).

The TypeScript package separately exports `Projection`, `ProjectionBuilder`, and `Smooth` from `packages/iyon-tui/src/index.ts:123-127`. No Rust native transport or runtime usage of these TypeScript classes was found in the inspected source search.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Projection-module caches

The projection module itself does not have a value cache. It has transient state in Smooth:

- Pending spans.
- Pending weighted units.
- Credit.
- Last advance time.
- Next deadline.
- Input envelope metadata.

The queue is rebuilt only when input envelope metadata changes. If source base, stable frontier, source end, and sealed state are unchanged, `rebuild_pending` returns without rebuilding (`smooth.rs:305-311`).

`Projection::spans_from` uses `partition_point`, avoiding a linear scan over already published spans (`value.rs:172-180`).

### 6.2 Application semantic cache

`application/content.rs` defines:

```rust
type SemanticProjectionCache =
    VecDeque<(SemanticProjectionKey, Arc<Projection<TextContent>>)>;
```

(`content.rs:300`)

`SemanticProjectionKey` includes:

- Source identity.
- Source generation.
- Content generation.
- Source revision.
- Source base/end.
- Sealed state.
- Funnel kind.
- ANSI hyperlink setting.

It intentionally excludes:

- Width.
- Wrap.
- Delivery revision.
- Theme revision.

(`content.rs:269-296`)

The cache capacity is two (`content.rs:261-267`). `resolve_cached_semantic` increments `SemanticProjectionRebuilds` only on a miss and maintains an MRU-like front insertion (`content.rs:302-323`).

Consequences:

- Theme changes reuse semantic IR.
- Width changes reuse semantic IR.
- Smooth ticks reuse semantic IR.
- Source revisions or semantic funnel changes rebuild semantic IR.
- `hyperlinks` is part of the semantic key, avoiding stale ANSI hyperlink semantics.

### 6.3 Prepared paint cache

`PreparedPaintCache` is keyed by:

- Semantic key.
- Theme revision.
- Width.
- Whether a finalized prefix is needed.
- Whether physical rows are needed.

(`content.rs:900-907`)

The prepared product retains:

- Layout tree.
- Text geometry cache.
- Optional physical rows.
- Visibility index.
- Optional finalized-prefix product.

(`content.rs:909-925`)

Capacity is two (`content.rs:261-267`).

Immediate non-History projections may defer physical row materialization. Smooth or History products retain physical rows because they need row visibility, scrollback, or reveal semantics (`content.rs:328-348`).

### 6.4 Projection cache

Each Connector retains a width/delivery-aware projection cache:

```rust
projection_cache: VecDeque<(TextProjectionKey, Arc<HostContentProjection>)>
```

(`content.rs:2994-2999`)

`TextProjectionKey` includes:

- Source identity and generations.
- Source revision/base/end/sealed state.
- Width.
- Wrap mode.
- Funnel kind.
- Delivery revision.
- Theme revision.
- Finalized-prefix requirement.
- Physical-row requirement.

(`content.rs:207-220`)

This key intentionally changes on each progressed delivery tick through `delivery_revision`, allowing a new candidate visibility product while semantic and prepared paint products remain reusable.

Projection cache capacity is two. A cache hit still calls `sync_connector_deadline` (`content.rs:3999-4011`).

### 6.5 Prefix proof cache

History-bound projections can retain a separate finalized-prefix proof cache:

```rust
type PrefixProofCache =
    VecDeque<(PrefixProofKey, Arc<PrefixProof>)>;
```

(`content.rs:946-948`)

The prefix cache capacity is two. Prefix proof keys include semantic identity, stable source end, and width (`content.rs:932-937`). The product is generated from a sealed stable Source prefix rather than by slicing the open projection (`content.rs:950-1019`).

### 6.6 Invalidation

#### Source changes

Source generation/revision/sealed changes invalidate:

- Grapheme indexing metadata.
- Semantic projection key.
- Projection key.
- Prepared products as dictated by the new semantic key.

A Source append wakes subscribed Connectors. Smooth Connectors also synchronize their deadlines after input (`content.rs:6427-6479`).

#### Delivery ticks

A progressed tick:

- Updates candidate frontier.
- Increments `delivery_revision`.
- Clears only `candidate_projection`.
- Preserves projection, semantic, and prepared paint caches.

(`content.rs:3672-3680`)

This is the central performance design: temporal visibility changes should not trigger parser or layout work.

#### Theme changes

Theme invalidation clears projection caches and candidate products but preserves the semantic cache (`content.rs:6580-6587` and related key logic). The prepared product key includes theme revision, so recoloring causes a paint product replacement while semantic IR remains reusable.

#### Visibility/mount state

Unmounting or hiding a Connector removes it from active deadlines and clears execution/derived state when appropriate (`content.rs:3136-3160`, `content.rs:6047-6079`).

### 6.7 Scheduling complexity

The active scheduler data structures are:

```text
active_deadlines: HashMap<ConnectorId, Instant>
active_connectors: HashSet<ConnectorId>
```

The due pass iterates only map entries whose deadlines are due (`content.rs:3603-3614`). The minimum deadline query scans active deadline values (`content.rs:3698-3700`), so it is O(D) where D is the number of active smooth Connectors, not the total Connector registry size.

The source append path uses host-grouped wake subscriptions and does not need to scan all ports or Connectors. Historical L1-10 documentation records this as the intended PERF-13 behavior.

### 6.8 Per-append and per-tick work

#### On a Source revision/input change

The current application path performs:

1. Source snapshot acquisition.
2. Full grapheme projection rebuild over retained Source chunks.
3. `Smooth::project` update.
4. Semantic projection cache lookup or parser rebuild.
5. Prepared paint/projection cache lookup or layout/paint preparation.
6. Candidate product installation.
7. Deadline synchronization.

The grapheme projection is not incrementally appended from the previous `ConnectorDelivery.units`; it is rebuilt when the indexed Source revision changes. This is correct for cross-chunk grapheme handling and Source replacement semantics, but it is not O(delta) over newly appended bytes.

#### On a pure delivery tick

The intended path performs:

1. Host supplies `Instant`.
2. Active deadline map selects due Connector IDs.
3. Smooth advances queue/credit/frontier.
4. Candidate projection is invalidated.
5. Delivery dirty record is emitted.
6. Later measurement uses cached semantic/paint products and VisibilityIndex bounds.

Historical L1-10 documentation claims zero parser, zero fresh View lowering, zero full-surface copying, and zero TypeScript transport on pure ticks. The current source structure supports those claims for the intended tick path.

#### Generic Smooth output cost

Generic `Smooth::project` still clones all values in the eligible published prefix because `output_from` iterates and clones values (`smooth.rs:354-370`). This is separate from application tick optimization, which does not use generic Smooth output as its physical row product.

### 6.9 Observed counters

The application instrumentation defines counters relevant to this subsystem:

- `SemanticProjectionRebuilds`
- `ContentDirtyRecordsMarked`
- `ContentMetricEvaluations`
- `ContentMetricChanges`
- `ContentPaintPropagations`
- `ContentDueConnectors`
- `ContentCandidateRecordsPrepared`
- `ContentSurfaceClones`

(`crates/iyon-tui/src/perf.rs:58-70`)

The source increments `SemanticProjectionRebuilds` on semantic cache misses (`content.rs:317-323`) and `ContentDueConnectors` during due scheduling (`content.rs:3610-3613`).

No runtime counters were collected during this investigation.

---

## 7. Tests, benchmarks and observability

### 7.1 Projection-owned tests

`projection/tests.rs` covers:

- Smooth publishes only stable spans and seals to identity (`tests.rs:35-46`).
- Atomic spans and accumulated credit (`tests.rs:48-75`).
- Elision has zero pacing cost (`tests.rs:77-93`).
- Exact nonempty coverage and explicit elision (`tests.rs:95-133`).
- Stable frontier boundary rules (`tests.rs:135-157`).
- Stable-prefix transition rules and tail replacement (`tests.rs:159-195`).
- Stable-prefix compaction (`tests.rs:197-228`).
- Relationship validation for lagging/sealed inputs and all relation errors (`tests.rs:230-348`).
- Incremental/batch convergence using a line-gating projector (`tests.rs:350-435`).
- Transition monotonicity and sealed mutation rejection (`tests.rs:438-517`).
- First/second projector errors and relation failures (`tests.rs:519-613`).
- Shared time and minimum deadline behavior in `Then` (`tests.rs:615-684`).
- Smooth composition and release observation (`tests.rs:686-712`).
- Smooth configuration rejection (`tests.rs:714-729`).
- Non-`Send` projectors (`tests.rs:731-767`).
- Unstable tail replacement and span merging without panic (`tests.rs:769-789`).

### 7.2 Public-shaped Rust tests

`projection/tests/projection_public.rs` uses the crate as `iyon_tui` and validates:

- A consumer can build and compose projections.
- `Rc<RefCell<_>>` projectors are accepted.
- `restart_from` backchains through `Then`.
- Projection fields are only accessible through accessors.
- Smooth deadlines are deterministic.
- Sealed input returns identity and removes the deadline.

(`projection_public.rs:101-194`)

These tests are compiled inside the private Rust crate through `migrated_tests.rs`; they are not evidence that the production crate exposes this Rust API cross-crate.

### 7.3 Text incremental tests

The projection transition validator is also used by text incremental test suites:

- `content/text/tests/markdown_incremental.rs`
- `content/text/tests/markdown_composition.rs`
- `content/text/tests/markdown_hardening.rs`

Those tests validate successive semantic projection snapshots and restart coordinates. This demonstrates that transition validation is a cross-subsystem test contract, even though it is not automatically embedded in production projector execution.

### 7.4 Application delivery tests

`application/content.rs` contains focused delivery tests including:

- `prepared_content_commit_preserves_a_newer_delivery_tick`
- `history_rows_transfer_unsealed_markdown_stream_with_smoothing`
- `smooth_history_matches_finalized_rows_through_ticks_and_receipts`
- `delivery_trace_parity_and_monotonicity`
- `two_connectors_independent_delivery_on_same_source`
- `native_ticks_perform_zero_parser_and_zero_surface_clones`
- `visible_frontier_commit_separated_from_execution_progress`
- `cold_and_disposed_connectors_clean_up_deadlines`

(`content.rs:8359-10714` test region; exact test symbols are listed by source search.)

These tests assert the important seam between Smooth state and host frame commitment:

- Candidate delivery can advance ahead of visible committed state.
- Aborted candidates roll back to committed frontier.
- A successful receipt promotes the exact prepared candidate.
- Immediate and Smooth Connectors on one Source remain independent.
- Sealed streams fully drain and remove deadlines.
- Unmounted/disposed Connectors do not keep scheduler deadlines.

### 7.5 Observability strengths

The implementation has useful state and counters for:

- Active due Connector count.
- Semantic parser rebuilds.
- Candidate preparation.
- Candidate versus committed delivery frontiers.
- Projection identity in prepared tickets.
- Projection failure code and diagnostic.
- Cleanup pending/error state.

`HostContentProjection::measurement` exposes:

- Intrinsic size.
- Physical completeness.
- Projection revision.
- Metric revision.
- Paint revision.
- Connector ID.
- Stable product identity.

(`application/content.rs:475-488`)

The monotonic `identity` is specifically used by prepared tickets to avoid pointer ABA issues after cache eviction/reuse (`content.rs:328-336`).

### 7.6 Observability gaps

- No production assertion automatically validates `validate_projection_transition` between every successive projector output.
- `Smooth` does not expose pending span count, pending units, credit, or episode state publicly.
- `queued_through` is maintained but has no observed consumer.
- There is no projection-module-specific counter for `Smooth::project` output cloning or grapheme projection rebuild work.
- `project_incremental` exists but has no active caller or route counter.
- The public TypeScript `Smooth` object exposes only `through`; it has no equivalent deadline or progress observability.

No benchmark was run for this report.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Rust projection is generic and correctly separated from product meaning

The Rust projection module contains no agent, assistant, tool, transcript, conversation, or product-status semantics. Its values are generic `T`, coordinates are generic `StreamOffset`/`StreamRange`, and Smooth rates are defined in projected values per second.

This satisfies the framework ownership boundary.

### 8.2 Rust module is production-private despite `pub` item declarations

The projection symbols are written as `pub` within a `pub(crate) mod projection`, but the module itself is inaccessible cross-crate from production `lib.rs` (`lib.rs:37-40`).

The root also re-exports projection symbols as `pub(crate)`, with key re-exports test-gated (`lib.rs:77-78`, `lib.rs:105-110`).

Therefore:

- The Rust projection API is intentionally available to internal modules and tests.
- The in-tree “public” tests are crate-internal compatibility/ergonomic tests.
- No external Rust authoring facade exists in this baseline.

### 8.3 TypeScript projection API is not Rust projection parity

The TypeScript package exports a separate API:

```ts
export class Projection {
  constructor(readonly source, readonly spans) { validateSpans(spans); }
  text(): string { ... }
  sourceRange(): ... { ... }
}

export class ProjectionBuilder {
  span(...): this { ... }
  finish(): Projection { ... }
}

export class Smooth {
  constructor(readonly through = 0) { ... }
}
```

(`packages/iyon-tui/src/api/content/projection.ts:1-34`)

Differences from Rust include:

- TS `Projection` stores a `source` object and spans, but no stable frontier or sealed bit.
- TS `ProjectionBuilder` validates only during `Projection` construction.
- TS validation permits gaps because it rejects `sourceStart < expected`, not `sourceStart != expected` (`projection.ts:28-34`).
- TS permits empty spans (`sourceEnd == sourceStart`), whereas Rust rejects empty spans for nonempty source domains.
- TS does not require the final span to reach a declared source end because it has no explicit source envelope.
- TS `Smooth` is only a validated nonnegative `through` value; it has no queue, pacing, clock, deadline, or `advance` operation.

The only direct TypeScript tests found construct and validate the TS class itself (`packages/iyon-tui/tests/tui_semantic_pipeline.test.ts:21-28`). No Rust/native transport use of these classes was found in the inspected TypeScript source.

This may be an intentionally lightweight authoring/data API, but the inspected baseline does not establish parity with Rust projection semantics.

### 8.4 Generic composition exists but is not the application semantic route

`Then` is fully implemented and tested, including nested composition and temporal delegation. However, the application’s actual text pipeline selects parser projectors by funnel kind rather than composing them through `Then` (`application/content.rs:690-730`).

Composition is active in:

- Projection-owned tests.
- Text projector composition tests.
- Generic user-defined in-crate projector tests.

No production `application/content.rs` `.then(...)` route was found.

### 8.5 Generic incremental helpers are latent, not active

The projection module contains two append/incremental facilities:

- `Projection::append_span_many` (`value.rs:105-126`)
- `Smooth::project_incremental` (`smooth.rs:409-419`)

A repository-wide source search found no current callers. The active application path instead:

- Rebuilds the complete grapheme projection when Source metadata changes.
- Uses Smooth only to advance a frontier.
- Uses `VisibilityIndex` and retained paint products for incremental physical reveal.

Thus the module offers an incremental projection API that is not the active runtime route.

### 8.6 Historical L1 reports align with current source on tick architecture

Historical `L1-10-report.md` describes:

- `ConnectorDelivery` separating input acceptance and clock advancement.
- Cached grapheme indexing.
- Active deadline indexing.
- Candidate/committed frontier separation.
- No parser or full-surface work on pure delivery ticks.

The current source contains those structures and paths (`application/content.rs:354-421`, `content.rs:3586-3695`).

Historical `L1-13-report.md` describes captured candidate projection/frontier behavior during delayed receipts. The current source’s `PreparedContentConnector`, candidate Arc matching, delivery revision, and candidate/committed frontier logic retain that shape.

These historical records are corroboration only; the current source remains authoritative.

### 8.7 Potential stale-frontier reliance

`Smooth::update` handles source-end shrinkage by clamping `published_end` down to the new source end (`smooth.rs:376-385`), but it does not independently reject or clamp a regression of `input.stable_through`.

The transition validator would reject stability regression, but `Smooth` itself does not invoke it. The generic Smooth contract therefore relies on callers supplying monotonic stable-frontier snapshots except where replacement/source-end handling is explicitly supported.

The application’s Source projection path normally sets the raw/grapheme input stable frontier to the current Source end, so the active Source route does not appear to generate a stable-frontier regression. This remains a generic-contract assumption rather than an internally enforced Smooth invariant.

### 8.8 Generic Smooth output allocation versus application tick claims

Generic Smooth output clones all eligible values into a new projection (`smooth.rs:354-370`). Application delivery avoids using that output as the physical row product and instead reuses cached rows.

Consequently, “zero copying on pure ticks” is true for the application’s optimized tick path, not a general statement that every call to `Smooth::project` is allocation-free.

---

## 9. Open questions and coverage gaps

1. **Why are `append_span` and `project_incremental` unused?**  
   They appear designed for append-only/delta routes, but no current production caller was found. It is unclear whether they are retained as future generic API, abandoned optimization scaffolding, or intentionally reserved for another subsystem.

2. **Should Smooth validate transitions itself?**  
   `Smooth` accepts an arbitrary `Projection<T>` on each call. It handles source-base/source-end changes defensively but does not invoke `validate_projection_transition`. The current active Source path supplies monotonic snapshots, but that is not enforced at the Smooth boundary.

3. **What is the intended external status of the TypeScript projection API?**  
   The TS classes are publicly exported, but no native/Rust route uses them in the inspected source. Their validation and lifecycle semantics differ substantially from Rust. It is unknown whether they are placeholders, standalone authoring helpers, or an intentionally separate transport model.

4. **Is `ProjectorExt` intentionally test-only?**  
   The trait exists but is only re-exported under `#[cfg(test)]` from `projection/mod.rs`. Generic composition is heavily tested internally, but production cross-crate users cannot access it through the private Rust module.

5. **What is the intended complexity of Source-to-grapheme indexing?**  
   `ConnectorDelivery` avoids repeated resegmentation on pure ticks, but every Source revision rebuilds the entire retained grapheme projection. The available append helper and `project_incremental` suggest a possible delta path, but no active implementation uses them.

6. **Is `queued_through` obsolete bookkeeping?**  
   It is assigned by `rebuild_pending` but has no observed read. Its intended invariant and whether it should be externally observable are unknown.

7. **Are all semantic projectors expected to use transition validation only in tests?**  
   Production projectors validate their own output projection through builders/text validators, while transition validation is explicit in tests. The runtime does not centrally enforce stable-prefix immutability across arbitrary projector calls.

8. **What is the intended behavior if `Smooth::new` receives an invalid `SmoothConfig` constructed internally?**  
   Supported public configuration constructors validate, but `Smooth::new` trusts the supplied config and does not validate.

9. **What are actual current benchmark measurements for projection and tick paths?**  
   Counters exist, and historical reports state expected zero-parser/zero-surface-clone behavior, but no fresh benchmark or runtime counter collection was performed here.

10. **How should stale delayed receipts interact with future source generations outside the tested application path?**  
    The application candidate identity and delivery-revision checks handle the observed path, but generic `Projection`/`Projector` APIs have no receipt or generation concept of their own.

No V5 disposition or migration decision is made in this report.

---

## 10. Evidence appendix

### 10.1 Primary projection files and symbols

| Path | Symbols / evidence |
|---|---|
| `crates/iyon-tui/src/projection/mod.rs` | Module boundary and exports (`:1-29`) |
| `crates/iyon-tui/src/projection/value.rs` | `Projection<T>` (`:12-18`), `ProjectionSpan<T>` (`:22-25`), `ProjectionBuilder<T>` (`:29-35`), builder methods (`:37-90`), append helpers (`:93-126`), accessors/indexing (`:136-180`), map methods (`:182-266`), span accessors (`:268-277`) |
| `crates/iyon-tui/src/projection/validate.rs` | Error enums (`:9-45`), construction validator (`:71-118`), relation validator (`:121-141`), transition validator (`:144-180`), stable-prefix comparator (`:183-230`) |
| `crates/iyon-tui/src/projection/projector.rs` | `Projector<Input>` (`:8-40`), `ProjectorExt` (`:42-52`) |
| `crates/iyon-tui/src/projection/compose.rs` | `Then<A,B>` (`:8-18`), `ThenError` (`:20-27`), composition implementation (`:66-104`) |
| `crates/iyon-tui/src/projection/smooth.rs` | Configuration (`:12-150`), `PendingSpan` (`:152-156`), `Smooth` state (`:158-178`), clock methods (`:186-274`), queue/output/update (`:276-407`), incremental helper and trait implementation (`:409-438`) |
| `crates/iyon-tui/src/projection/tests.rs` | Core projection tests (`:35-348`), composition and temporal tests (`:350-729`), non-`Send` and replacement tests (`:731-789`) |
| `crates/iyon-tui/src/projection/migrated_tests.rs` | Included migrated test modules (`:1-15`) |
| `crates/iyon-tui/src/projection/tests/p3c_ergonomics.rs` | Builder/mapping/config ergonomics (`:1-60`) |
| `crates/iyon-tui/src/projection/tests/projection_public.rs` | External-consumer-shaped composition/accessor/deadline tests (`:1-194`) |

### 10.2 Supporting Rust files

| Path | Symbols / evidence |
|---|---|
| `crates/iyon-tui/src/lib.rs` | Private projection module (`:37-40`), test-only root re-export (`:77-78`), internal projection re-exports (`:105-110`) |
| `crates/iyon-tui/src/stream/coord.rs` | `StreamOffset` (`:9-36`), `StreamRange` (`:38-51`) |
| `crates/iyon-tui/src/application/content.rs` | `SemanticProjectionKey` and caches (`:207-323`), `HostContentProjection` (`:328-352`), `ConnectorDelivery` (`:354-421`), `ConnectorExecution` (`:423-473`), Source projection (`:611-688`), semantic dispatch (`:690-730`), text projection preparation (`:1021-1256`), Connector state (`:2966-3016`), active scheduler state (`:3234-3304`), advance/deadline paths (`:3586-3774`), projection preparation/cache (`:3923-4088`), candidate cleanup (`:5134-5167`), Source wake path (`:6397-6484`) |
| `crates/iyon-tui/src/application/host.rs` | Content advance in frame preparation (`:2275-2285`), combined wake deadline (`:1243-1259`), deterministic time (`:1405-1409`), async deadline wait (`:1519-1534`) |
| `crates/iyon-tui/src/content/text/plain.rs` | Plain projector and stability (`:20-75`, `:105-140`) |
| `crates/iyon-tui/src/perf.rs` | Projection/delivery-related counters (`:58-70`, names at `:123-129`) |

### 10.3 Supporting TypeScript files

| Path | Symbols / evidence |
|---|---|
| `packages/iyon-tui/src/api/content/projection.ts` | TS `Projection`, `ProjectionBuilder`, `Smooth`, and validation (`:1-34`) |
| `packages/iyon-tui/src/index.ts` | Public TS exports (`:123-127`) |
| `packages/iyon-tui/tests/tui_semantic_pipeline.test.ts` | Standalone TS Projection construction/validation (`:21-28`) |

### 10.4 Historical documents consulted

| Path | Relevant historical evidence |
|---|---|
| `reports/pre-v5-l1/L1-09-report.md` | Parser/cache and semantic-key decoupling history (`:21-38`), historical verification (`:42-63`) |
| `reports/pre-v5-l1/L1-10-report.md` | ConnectorDelivery, deadline index, candidate/committed frontiers, tick behavior (`:7-37`), historical test evidence (`:41-72`) |
| `reports/pre-v5-l1/L1-13-report.md` | Captured candidate projection/frontier during delayed receipts (`:39-61`), historical boundary statements (`:12-30`) |
| `PRE-V5-ARCHITECTURE-REPORT.md` | Investigation discipline, current-state-first rule, projection/smoothing trace requirements, and evidence standard; consulted as historical/context material without making V5 disposition decisions |
| `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md` | Required report headings and evidence obligations (`:12-40`) |
| `docs/architecture/atlas-4355c02/README.md` | Baseline, assignment 09 scope, and atlas organization (`:1-36`) |

### 10.5 Inspected-file manifest

#### Exhaustively read in primary scope

```text
crates/iyon-tui/src/projection/compose.rs
crates/iyon-tui/src/projection/migrated_tests.rs
crates/iyon-tui/src/projection/mod.rs
crates/iyon-tui/src/projection/projector.rs
crates/iyon-tui/src/projection/smooth.rs
crates/iyon-tui/src/projection/tests.rs
crates/iyon-tui/src/projection/tests/p3c_ergonomics.rs
crates/iyon-tui/src/projection/tests/projection_public.rs
crates/iyon-tui/src/projection/validate.rs
crates/iyon-tui/src/projection/value.rs
```

#### Supporting source inspected for production wiring

```text
crates/iyon-tui/src/lib.rs
crates/iyon-tui/src/stream/coord.rs
crates/iyon-tui/src/application/content.rs
crates/iyon-tui/src/application/host.rs
crates/iyon-tui/src/content/text/plain.rs
crates/iyon-tui/src/content/text/markdown.rs
crates/iyon-tui/src/content/text/diff.rs
crates/iyon-tui/src/content/text/ansi.rs
crates/iyon-tui/src/content/text/visit.rs
crates/iyon-tui/src/perf.rs
packages/iyon-tui/src/api/content/projection.ts
packages/iyon-tui/src/index.ts
packages/iyon-tui/tests/tui_semantic_pipeline.test.ts
```

#### Indexed/search-confirmed supporting paths

```text
crates/iyon-tui/src/content/text/tests/markdown_incremental.rs
crates/iyon-tui/src/content/text/tests/markdown_composition.rs
crates/iyon-tui/src/content/text/tests/markdown_hardening.rs
```

No files under the primary projection subtree were merely indexed without inspection. No build, test, benchmark, install, or source mutation was performed for this report.