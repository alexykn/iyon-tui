## Scope guard — read this before acting on any cut below

This document is a diagnosis, not an execution order, and it predates a decision
that overrides parts of it. Three rules bind any future simplification work:

1. **Genericity is the product, not the bloat.** `iyon-tui` is a generic TUI
   framework (see the framework boundary rules in `AGENTS.md`). Do not remove or
   thin generic capability because the Iyon harness alone does not exercise it.
   The test for a cut is not "does Iyon use this?" but "is this duplicated, dead,
   or a conversion between two representations that could be one?" Indirection
   that converts between redundant representations is a target; indirection that
   abstracts over implementations is framework surface and stays.
2. **Explicitly sanctioned exception: the generic text IR goes.** Removing the
   `content/text/` semantic text IR (Block, Inline, TextRun, Annotations,
   MarkSet, SemanticTag, provenance, visitors/rewriters, TextSelector/TextRole)
   and its double lowering into the View IR is **wanted and approved**. Content
   domains render directly into View: Markdown → MarkdownRenderer → View,
   Diff → DiffRenderer → View, later treesitter likewise, plaintext directly.
   Theming/styling addresses results by domain-scoped role keys instead of a
   generic selector model: e.g. `text.muted`, `markdown.h1.bold`,
   `markdown.code_block`, `diff.addition`. This is not a violation of rule 1:
   what is removed is a conversion layer between two internal representations,
   not framework capability. Domain-specific styling keys must remain generic in
   the sense that any caller can use a markdown/diff/plaintext renderer and its
   theme namespace; no Iyon-product policy may leak into them (AGENTS.md rule).
   When in doubt about other "generic-looking" layers, they stay unless
   explicitly listed here.
3. **No public-API removals** beyond what item 2 sanctions. There are currently
   zero external users, but the goal of this pass is compressing internals —
   making them leaner and more straightforward — not shrinking the framework's
   capability surface. Public API reshaping beyond the text-IR replacement is a
   separate, deliberate decision, not a side effect of LOC-reduction work.

Concretely this means: the `~44k / 50%` scenario below is off the table except
for the sanctioned text-IR replacement (rule 2), and items in the cut list must
each be re-validated against rules 1–3 before touching them. The realistic
target under these rules is consolidation of duplication plus the text-IR
evaporation (estimated ~25-35%, concentrated in `content/text/`, `agent/`,
stream compile/viewport duplication, host factoring, and redundant tests) — not
reduction of abstraction depth anywhere else.

The dialogue below is preserved as-is for context, with the understanding that
statements in it about cutting genericity are superseded by this guard.

---

Now I've got the full map. Here's my diagnosis of exactly where the fat lives:

  🗺️ The Redundancy Map

  1. Two text models coexisting (~4k lines of bridge)

  There are two text representations:

  System           Location
  Rich semantic IR content/text/ (Block, Inline, TextRun, Annotations, MarkSet, etc.)
  Internal View IR presentation/ir.rs (ViewKind::Text, TextView, TextSpan, etc.)

  The pipeline is: Markdown → Block/Inline → TextView → painted cells. The Block/
  Inline layer is this beautiful generic semantic IR... that then gets lowered to
  TextView which is a much simpler span model. The MarkdownProjector outputs
  TextContent::Block(Block), then TextRenderer::render_block() converts it to
  View::text(...) which wraps TextView. You're maintaining two parallel text type
  systems with conversion logic between them.

  Potential cut: Could the internal View just be the Block/Inline IR directly? Or
  conversely, could Block/Inline be simplified since it's really an intermediate that
  gets lowered to View anyway?

  2. agent/ vs kernel/ — 3,300 lines of old loop not yet deleted

  Module                                               Lines Status
  agent/ (turn, loop, tool_execution, tool_call, etc.) 3,298 Old — pub(crate)
  kernel/ (model_turn, tool, session, queue, kernel)   1,917 New — public
  runtime.rs                                           695   Still imports from agent!

  The kernel docs say "The compatibility agent still lives under crate::agent" — but
  the kernel re-exports from agent (AgentMessage, ToolCallRequest). The old loop
  still runs via runtime.rs. Once the kernel is the only path, agent/ can go.

  Potential cut: ~1,500-2,000 lines of agent/ and duplicated session state.

  3. presentation/api/ vs ir.rs — builder wrappers

  presentation/api/view.rs (631 lines) + composition.rs (495 lines) + text.rs (353
  lines) + style.rs (1,042 lines) + grid.rs (578 lines) = ~3,100 lines of public API
  builders wrapping the internal types in ir.rs (590 lines). That's a 5:1 wrapper-to-
  internal ratio. Some of this is essential ergonomics, but some is thin delegation.

  4. stream/ — 36 files doing one thing

  6,103 lines split across 36 files for what's fundamentally: append text, compile
  rows, retain prefix, reflow suffix.

  compile/ and viewport/ are two approaches to row compilation coexisting. pane/ has
  anchors/commands/index/presentation as separate files. tests/ is 9 files / 2,500
  lines.

  5. projection/ — 1,861 lines of pure algebra

  The projection algebra (Projection<T>, StreamOffset, validation, compose) is
  mathematically clean but heavy. 767 lines of tests for what's essentially offset
  math.

  6. application/host.rs — 2,581 line god object

  Host view slots, scroll panes, text inputs, streams, history binding, working
  indicators, animation, theme, resize — all in one file. Should be ~1,000 lines with
  better factoring, but it's more that each subsystem has its own mini-framework when
  a simpler approach would do.

  7. Tests — 12,854 lines

  That's 23% of iyon-tui. The stream tests alone are 2,500+ lines. The history
  projection tests are 909 lines. The presentation layout tests are 1,800+ lines.
  There's a lot of "test the same thing through different entry points."

  🎯 ~50% Target

  If I were doing this, the biggest knobs would be:

  Cut
  Delete agent/ after kernel migration
  Collapse content/text/ into something simpler (Block/Inline is beautiful but your
  View model already has spans)
  Merge compile/ and viewport/ in stream/
  Consolidate stream test files
  Remove dead perf-scaffolding after PERF work lands
  Prune projection tests to essential coverage
  Remove no-longer-needed History projection old paths
  Consolidate presentation/api/ builder boilerplate
  Merge redundant layout tests
  Trim iyon-native/tui.rs
  Various dead code / duplicate checks
  Total

  That gets you from ~88k to ~75k. Getting to ~44k (half) is more aggressive — it
  would mean things like:

  - Can the TUI framework be less generic? E.g. fewer TextRole variants, fewer
    semantic annotation features, simpler projection model.
  - Can the content Block/Inline be the View directly? No conversion layer.
  - Can layout/ and paint/ share more structure?
  - Can you accept less exhaustive tests?

  The real question is: which of those are you willing to sacrifice for the LOC
  reduction? You said no feature drops, so it's more about removing indirection
  layers and consolidation. That's doable — but half the LOC is a stretch without
  cutting abstraction depth. Maybe 60-65% of current is more realistic for "no
  feature loss, just no bloat."

  Want me to start mapping out precise cutter's notes for a specific subsystem?

────────────────────────────────────────────────────────────────────────────────────────
we probably do not need a dual representation for text. it can go directly from
markdown -> ir -> view without markdown -> ir -> ir -> view.

also the single left over ir can be made into "this is plaintext" -> "this is this
markdown thing" -> "this is this diff thing" -> "this is this treesitter thing" instead
of lowering all of them into the same generic ir. people using a markdown thing can
then just with the help of our docs theme by saying this stream pane goes through a
markdown renderer so theme markdown.h2 or something like that which removes one layer
of indirection.

no do not start doing anything i have to think about every subsystem individually and
it all comes after the performance docs are fully addressed
────────────────────────────────────────────────────────────────────────────────────────

  The user is saying:

  1. They agree the dual text representation is wasteful
  2. They envision a simpler model: each content type (markdown, diff, treesitter,
     plaintext) has its own direct renderer, rather than lowering everything into a
     generic Block/Inline IR first
  3. DON'T start implementing anything yet - they need to think through each
     subsystem individually
  4. This ALL comes AFTER the performance docs are fully addressed

  They're just thinking out loud / discussing architecture philosophy right now. They
  want me to acknowledge and engage with the idea, but not start coding.

  Let me engage thoughtfully.

  That's a strong vision. Make each content domain its own first-class thing instead
  of pouring everything through the same generic funnel.

  Right now you've got:

  Markdown → Block/Inline IR → View(TextView) → painted cells
  Diff     → Diff model   → Block/Inline IR → View(TextView) → painted cells
  Treesitter→ ???         → Block/Inline IR → View(TextView) → painted cells


  But what you're describing is:

  Markdown → MarkdownRenderer → View  (theme with markdown.h2, markdown.code_block, et
  Diff     → DiffRenderer     → View  (theme with diff.addition, diff.deletion, etc.)
  Treesitter→ TreesitterRenderer → View (theme with treesitter.keyword, etc.)
  Plaintext→ PlaintextRenderer → View


  No Block. No Inline. No TextRun. No Annotations. No MarkSet. No SemanticTag. No
  TextProvenance. No walk_block()/walk_rewrite_block(). No TextContent::Raw vs
  TextContent::Block. Each domain knows how to render itself, and theming addresses
  it by domain+role.

  That kills the entire content/text/ subsystem — maybe 6,000-7,000 lines — and
  replaces it with a handful of focused renderers that speak View natively. The
  TextRenderer/TextRenderPolicy layer vanishes. The TextSelector/TextRole
  combinatorial explosion collapses into per-domain keys.

  Themed styling becomes: theme.markdown.h2.foreground(...) not
  theme.text(TextSelector::heading().level(H2), ...).

  The smooth projector and stream row index don't care what the renderer is — they
  just see View with stream offsets.

  It's a genuinely simpler architecture. Fewer abstractions, fewer conversions, fewer
  places for bugs to hide. And the LOC drop is real because you're cutting
  translation layers, not features.

  Yeah, think it through. The performance work needs to land first anyway — and
  honestly, a simpler text model might even make some of the perf work easier (less
  pointer chasing through Block→Inline→TextRun→View).

────────────────────────────────────────────────────────────────────────────────────────
also simplifications like that will also lead to performance gains (small ones but
ones). well for me it was first get everything implemented right so the public api
shape is good and performance is good and internally parts can get swapped

now that that is done and i got a rough sketch of what the ACTUAL borders are the first
thing to do for simplification will be compressing the things within the found borders.
like the text pipeline
────────────────────────────────────────────────────────────────────────────────────────

  The user is articulating their approach:

  1. First pass: Get everything implemented and working with the right public API
     shape and good performance. This is done (the perf refactor work).
  2. Second pass: Now that they understand the actual boundaries of the system,
     compress things within those boundaries. Simplify internals without changing the
     public shape.

  The text pipeline is the first target they mentioned - collapsing the dual
  representation into a direct Markdown/Diff/etc. → View pipeline without the
  intermediate generic IR.

  They're thinking out loud and not asking me to do anything yet. They want to think
  through each subsystem individually, and all this comes after the performance docs
  are fully addressed.

  Exactly. That's the right sequencing:

  1. First iteration — make it work, make the API right, make it fast. You're
     discovering the real boundaries as you go.
  2. Second iteration — now that you know where the actual walls are, you can
     reorganize within them without breaking anything.

  The perf refactor already did this for the internal engine (layout cache, stream
  row index, History geometry, etc.) — those changes didn't change the public shape,
  they just made the internals smarter. The simplification pass is the same deal but
  aimed at structure rather than caching.

  The text pipeline is a perfect first target because:

  - The public API (View::text(...), History, StreamPane, MarkdownProjector) doesn't
    need to change
  - What changes is the guts — instead of Markdown → Block/Inline IR → View, it's
    Markdown → View
  - The Block/Inline IR evaporates, along with the conversion step, the validation,
    the walking/rewriting visitors, the TextContent enum, etc.
  - Everything downstream (layout, paint) already consumes View, so they don't care

  And yeah, the performance wins from this are secondary but real — fewer
  allocations, fewer translation steps, less pointer chasing. The main thing is
  maintainability.

  Take your time sketching it out. The perf docs need to land first, and this kind of
  restructuring is easier to reason about when you're not also fighting performance
  fires.

────────────────────────────────────────────────────────────────────────────────────────
It is something that works well with ai because ai can spit out tons of shit in short
time so overbuilding then shaving off is actually a acceptable and doable thing xD
────────────────────────────────────────────────────────────────────────────────────────

  The user is reflecting on the process - they're saying that because AI can generate
  a lot of code quickly, the approach of "overbuild first, then simplify" is actually
  viable and practical. They're acknowledging the meta-process of how this codebase
  came to be this size - an AI (or AI-assisted) process where it's easier to generate
  a lot and then cut back than to try to get it exactly right on the first pass.

  They're being self-aware about the development methodology and essentially saying
  "yeah, this bloat happened because we could generate fast, and now we're in the
  cleanup phase which is also a strength of AI - systematically going through and
  compressing."

  Haha exactly. It's the "throw a lot of clay at the wall, then carve" approach. AI
  is great at the throwing part — it'll happily spin up a 2,500-line application/
  host.rs with every feature under the sun, a full generic text IR with visitors and
  rewriters and provenance tracking, all the abstraction layers, all the defensive
  checks, all the tests for all of it.

  And then you come back and look at it and go "this is beautiful but we only needed
  half of it" — and the shaving pass is also something AI can do well, because it's
  systematic, mechanical, and benefits from the bird's-eye view of "what's actually
  reachable, what's redundant, what can be collapsed."

  The trick is knowing which half to keep. That's the human part — recognizing the
  real borders after the AI has drawn all the possible ones.

  Overbuild → survey → compress. It's a legit workflow when the marginal cost of
  generation is near zero.
