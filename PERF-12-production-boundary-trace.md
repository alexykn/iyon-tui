# PERF-12 — Production boundary trace (§77 inventory)

**Status:** complete trace of the real production View-bearing surface, prepared as the T13 prerequisite (`§77`: "Trace actual production source before implementation").
**Scope:** every place the production application constructs, stores, replaces, or renders a `View`, in both the deprecated Rust app layer (`crates/iyon/*`) and the current TypeScript plugin layer (`plugins/*`, backed by the generic facade in `packages/iyon-runtime/src/tui/*`).
**Companion documents:** `PERF-12-retained-dag-direct-ffi-handoff.md` (architecture + tranche records), referenced from the T13 record.

---

## 1. The two production layers

### 1.1 Deprecated app layer — `crates/iyon/src/tui/*` (Rust)

The legacy layer is a direct consumer of the generic `iyon-tui` framework. Its shape is the "goal" the TS layer reimplements:

```text
lib.rs → tui/mod.rs
  backend.rs        agent/core bridge: BackendCommands, tool draft keys,
                    ToolUpdatePresentation, event translation into IyonAction
  state.rs          IyonState: composer/steering components, ConversationState
                    (user batch, working spinner, per-tool LiveTool{unit,
                    ComponentHandle<ConversationActivity>, ScrollPane}, stream),
                    paste store, approvals; ALL History mutations happen here
  theme.rs          iyon_theme(): Theme with colors/styles/text-styles;
                    AGENT_EFFORT StyleStateKey; VIEWPORT_GUTTER insets
  controller.rs     IyonAction enum
  components/       retained components: ConversationActivity (working spinner
                    + tool cards + pulse), UserBatch, SteeringQueuePanel
  transcript/       AssistantStream (StreamingSource: pacing atoms +
                    AssistantPipeline{Smooth → MarkdownProjector(GFM, live
                    tables) → PipeTableRewriter → thinking annotation rewriter}),
                    semantic.rs TuiFormatter (TimelineItem → View), pipeline.rs
                    (assistant_render_policy / assistant_presentation chunks)
  tools/            registry + per-tool renderers (bash/read/write/edit/grep/
                    find/ls/generic) and unified_diff.rs parser feeding DiffHunk
```

App instantiation (legacy): `iyon_tui::App::new(|cx| …)` → `IyonState::init(cx, …)`
registers components (`TextInput`, `SteeringQueuePanel`, later `UserBatch`,
`ConversationActivity`, `ScrollPane`) via `cx.register`, routes outputs
(`cx.route(composer.submitted, SubmitTurn)`), binds keys, intercepts paste.
Each frame the kernel calls `IyonState::view()` which builds a small chrome tree:

```text
View::vertical(|column| {
    working component (if any)   // View::component(ConversationActivity)
    content_max(MAX_COMPOSER_ROWS=13, composer)   // styled by style_state(AGENT_EFFORT, effort)
    footer text                  // provider · model · effort · status
}).fill_width().fill_height()
```

History is retained native state owned by the kernel; the chrome view never
contains conversation content — that lives in History units.

### 1.2 Current app layer — `plugins/app/iyon` (+ `plugins/tools/*`, `plugins/agents/*`)

Plugin activation: `api.apps.register({ id: "iyon", create })` →
`createIyonApp(dependencies)` (`plugins/app/iyon/src/index.ts`). Dependencies:
an agent (`plugins/agents/iyon`), core commands, model metadata, an optional
tool renderer resolver (`plugins/tools/*/render.ts` contributions).

`app.ts` (`IyonAppImpl`) owns the whole View-bearing surface:

| Handle | Created | Role |
|---|---|---|
| `historyHandle: HistoryHandle` | `new History()` then replaced by `tui.createHistory()` in `start()` | retained conversation scrollback |
| `composerHandle: TextInputHandle` | `new TextInput({multiline:true})` then replaced by `tui.createTextInput({multiline:true, border:{plain, topBottom, theme:input.border}})` | input |
| `workingHandle: ViewSlot` | `tui.createViewSlot(View.spacer(0))` | working spinner animation target |
| `toolSlots: Map<key, ViewSlot>` | `tui.createViewSlot(view)` per live tool card | live tool card (pulse animation) |
| `toolPanes: Map<key, ScrollPane>` | `tui.createScrollPane(View.spacer(0))` per live tool | streaming tool output |
| `assistantStream: NativeAssistantStream` | on first assistant/thinking delta | native TextStream pushed into History |

Scene construction per render (`view.ts` `createIyonView`, mirroring legacy
`state.view()`):

```text
View.vertical(column => {
    working row      (View.component(workingHandle) + queue preview) | spacer
    approval view    | spacer
    contentMax(13, View.component(composer).style(theme.composer)
                        .styleState("iyon.agent.effort", effort))
    footer text      .style(theme.footer)
}).fillWidth().fillHeight()
```

Render entry: `dispatch(action)` / `handleAction` mutate a reduced `IyonState`,
serialize through a single `historyMutation` promise chain
(`appendHistory` → History mutations, then `renderCurrentScene`).
`renderCurrentScene` computes a `bodyKey` string from the state (goodbye flag,
footer text, effort, pending approval, activity visibility, steering list,
live-tool statuses) and skips `tui.render(...)` entirely when unchanged
(calling `advance?.(0)` so animations still tick) — otherwise renders
`new Scene(body, this.history)`.

---

## 2. Theming and styling, and how they relate

**Theme** (`theme.ts` vs legacy `theme.rs`): built once at app start.
TS: `Theme.new().withColor(key, color).withColorVariant(key, {states:{…},
focused}, color).withStyle(key, StyleSpec).withTextStyle(selector, StyleSpec)`
— identical key set to legacy `Theme::new().with_color(...)`.
Pushed to native exactly once: `tui.setTheme(theme)` → host `set_theme`
(lowered through `lower_theme` in `crates/iyon-native/src/tui.rs`).
After that, **no JS object ever carries resolved colors**: views reference
theme entries by string key (`"theme:text.muted"`, `"theme:input.border"`,
`Style.new().theme("diff.meta")`), and the native side resolves them against
the host theme at materialization/paint time. The reasoning-effort selector is
a *style-state variant* on one theme key (`input.border` varies by
`iyon.agent.effort` × focused), selected at render time by
`View.component(composer).styleState("iyon.agent.effort", level)` — not by
rebuilding views with different colors.

**Styles on views:** `View.text(...).style(StyleSpec)` merges the spec into the
decorated node's `decoration.style`; `.foreground/.background/.border/.bold()/…`
do the same via `decorate()`. Themed text styling for markdown comes from
`withTextStyle(TextSelector.heading()/inlineCode()/codeBlock()/part("codeLabel"|…)/annotation("app","thinking"))`
— the same selector set as legacy (`TextSelector::heading()`, `part(TextPart::…)`,
`annotation(thinking_tag())`). PERF-12 T11's `STYLE_REF_CACHE` already resolves
these decoration styles to generation-scoped `StyleRef`s during retained
materialization; the authoritative style table remains the native runtime's (§40).

Relationship summary: Theme = named colors/styles/state variants installed on
the host once; StyleSpec = caller-side value referencing those names; the View
DAG stores only references; native resolves references late. A theme change
therefore requires no structural retransmission — only new paint-time
resolution. (Note for T13: nothing invalidates JS-side cached StyleRefs on
`setTheme`; today the app sets the theme once before first render.)

---

## 3. Streaming panes, history, and the assistant stream

History is the retained subsystem (native `crates/iyon-tui/src/history/model.rs`):
`push`, `freeze(unit, view)`, `discard_live`, `push_stream[_with_boundary]`,
`update_stream`, `seal_stream`, `set_layout({padding,gap})`. The TS facade
(`history.ts`) already prefers ref-based entry points when the retained path
can materialize the unit view (`pushRef`, `freezeRef` via
`tryNativeMaterialize`), releasing the temporary lease after the call —
History natively retains its own strong state per unit. Legacy used the same
operations plus `FlowBoundary::AttachToPrevious` for collapsed duplicate tool
results (TS collapses via `collapseResultView` instead and dedupes by
`last_completed_tool`-equivalent bookkeeping).

Per-conversation-element lifecycle as actually driven by `app.ts`:

1. **User message / batch.** First message opens a *live* unit:
   `slot = tui.createViewSlot(userBatchView([text]))`;
   `unit = history.push(View.component(slot).fillWidth())`. Subsequent queued
   user messages mutate the slot in place: `slot.setView(userBatchView(messages,…))`.
   On the first assistant/thinking/tool delta the batch is **frozen**:
   `history.freeze(unit, userBatchView(all messages))`, slot disposed. Frozen
   units are plain Views retained by History forever after.

2. **Assistant stream.** `NativeAssistantStream` wraps one native `TextStream`
   constructed with `{ projector: "markdown", presentation: { insets
   {0,2,0,2} }, pacing: { minUnitsPerSecond: 40, maxUnitsPerSecond: 800 } }`.
   `history.pushStream(stream)` inserts it as a live unit; every delta is
   `stream.append(text, kind === "thinking" ? [{namespace:"app",name:"thinking"}] : [])`
   (with a `\n\n` normalization between thinking→text segment transitions);
   turn end calls `stream.seal()` + `history.sealStream(stream)`.
   All smoothing/markdown/table/thinking-styling work happens natively
   (`SmoothConfig` pacing, MarkdownProjector with live-table stabilization,
   annotation-driven themed italic muted thinking) — identical pipeline shape
   to legacy `AssistantPipeline` (Smooth → MarkdownProjector → PipeTableRewriter
   → thinking rewriter → TextRenderer policy: block gap 1, soft break =
   LineBreak, table columns content-sized, task-only markers, language code
   labels, no-wrap code). Stream bytes never enter the structural View bridge
   (PERF-12 §42, counter-proven in the T11 record).

3. **Tool cards.** Draft lifecycle (`preparing → arguments → prepared → started
   → updated → result/approval → frozen`) is tracked in `ToolCardStore` (pure
   state); rendering goes through `updateToolSlot`:
   - live card = `ViewSlot` holding the call line view; while active it pulses
     via `slot.setAnimation([view, pulsed], 480)` (or `stopAnimation(view)`).
   - output pane = `ScrollPane(View.spacer(0))`; updates are
     `pane.setContent(updateView); pane.followEnd()`.
   - both are mounted once into History inside a vertical unit:
     `history.push(vertical[component(slot).fillWidth, flexMax(16, component(pane)).fillWidth])`.
   - terminal result freezes the unit:
     `history.freeze(unit, vertical[callLine, resultView].fillWidth())`, then
     slot + pane are disposed.
   Without native slots (headless/test fallback) cards mount directly as
   static `history.push(view)` once.

4. **Working spinner.** One persistent `workingHandle` ViewSlot outside
   History. `renderCurrentScene` drives it: frames from `workingFrames(waiting)`
   (5 braille spinner frames × "waiting"/"Working") at 80 ms; when the waiting
   flag flips it swaps via `setAnimationAtCycleBoundary` to avoid restarting
   mid-cycle. This mirrors legacy `ConversationActivity` pulsing but moves the
   tick ownership from a component to the shared slot-animation machinery.

---

## 4. Components

Component handles (`ViewSlot`, `ScrollPane`, `TextInput`) are referenced inside
View trees via `View.component(handle)` (legacy `View::component(handle)`),
which lowers to a native component id reference — the tree stays semantic; the
component's current content lives natively. Capabilities: TextInput exposes
`submitted()` output routed via `tui.route(output, routeId)`; keys are bound
app-level (`ctrlC`, `escape`, `shift+tab`, raw `\u0003`); paste interception
routes through `interceptPaste`. Approval decisions in legacy were a routed
component Output; in TS they are plain actions (`approve/reject`) — no View
involvement beyond the approval card view.

---

## 5. Diff rendering

Both layers parse unified diffs at the application edge and lower them through
the framework's semantic diff values:

- Legacy: `tools/unified_diff.rs` parses into `iyon_tui::DiffHunk{DiffRange,
  DiffLine{kind,text,termination}}`; renderers emit themed lines.
- TS: `packages/iyon-plugins/src/tools/support/render.ts` `parseUnifiedDiff` →
  `DiffRenderer().render(hunks)` (the canonical lowering used by the T11
  retained lane), wrapped by `collapseResultView` =
  `clampRows(16, overflow footer "… more lines (full result retained)",
  themed truncation style)` — identical constants and footer semantics to
  legacy `MAX_COLLAPSED_TOOL_ROWS` / `OverflowIndicator::Footer`.

Diff views appear inside tool results (edit tool: `renderDiff(result.details)`)
and therefore inside frozen History units and live ScrollPane contents. The
retained diff constructor (`view_diff_create_buffer`) plus DiffRenderer cover
this surface end-to-end since T11.

## 6. Markdown rendering

Markdown exists in exactly two places, both outside the structural bridge:

1. Assistant stream: native `TextStream` with `projector:"markdown"` (see §3).
2. Static re-renders: none in TS production — finalized assistant content is
   never rebuilt as a View DAG; it stays in its sealed stream unit. (Legacy
   could rebuild `TimelineItem::AssistantMessage` through the full pipeline;
   TS relies on the sealed stream remaining in History.)

The generic `MarkdownProjector`/`PlainTextProjector` facade objects exist for
tests/tools but are not on the production render path.

## 7. Stream smoothing

Native-owned end to end: `SmoothConfig` (min/max units-per-second, spring,
tick interval) is parsed from the TS options object and executed inside the
host's stream pacing (`crates/iyon-tui/src/application/host.rs`). The app only
appends and seals; `advance(ms)` on the Tui runtime (called by
`renderCurrentScene` even on body-key hits) drives time for pacing/animations
in headless mode. No per-frame JS involvement — this matches PERF-12 §81's
requirement that animation/smoothing must not create a hidden per-frame bridge.

## 8. Tools and how they construct views from the APIs

Tool plugins (`plugins/tools/{bash,read,write,edit,grep,find,ls}`) register
execution + optional renderers via `api.tools.register`. Renderers build views
from shared support helpers (`@iyon/plugins`):

```text
toolCallLine(label, state, pulse)  hanging("● ", "  ", text(state-style))   bullet line
toolCallPreview(call)              JSON argument preview lines (hanging)
toolResultLine / resultLines       hanging("  ", "  ", text(style))
resultBlock(body)                  two-space hanging indent block
resultStyle(isError)/toolStyle     foreground theme:<tool.error|text.muted|tool.running|…>
collapseResultView(view)           clampRows(16, themed truncation footer)
renderDiff(details)                unified-diff → DiffRenderer
```

Generic fallbacks live in `packages/iyon-runtime/src/tools/generic.ts`
(`renderGenericCall/renderGenericResult`). Every produced view uses only:
text, styled/decorated text, vertical/hanging layout, clamp rows, and (for
edit) DiffRenderer hunks — i.e. kinds fully covered by T7–T11 materializers
except `clamp`/overflow-footer and `container`, which currently route to the
complete fallback until their T13 materializers/routing land.

## 9. Where the View bridge is actually entered (the §77 boundary list)

Tracing the TS facade down to native, production renders cross these
boundaries, each of which PERF-12 T13 must own explicitly:

| # | Boundary | Entry points (files) | Current transport behavior |
|---|---|---|---|
| B1 | Scene root (`Tui.render`) | `runtime.ts` `Tui.render(Scene)`; app calls it per state change | Holds `currentNativeRef` for the previous scene body and walks the 11v3 route cascade: `render_ref` → scalar patch → path-scalar → structural → edit transaction → text-create → cold → boundary-create → **Direct decode fallback** (`host.render(nodeForBridge(body))`). No BridgeNativeHint sidecars, no ceiling-gated NodeId promotion, no MaterializeTx. Previous ref released only after success ✓. |
| B2 | History units | `history.ts` `push/freeze/pushStream/sealStream/setLayout` | Per-call cold materialization via `tryNativeMaterialize` (cold FFI graph) then `pushRef/freezeRef`; falls back to `nodeForBridge` N-API push. No identity reuse across units or with the scene. Streams bypass the bridge entirely. |
| B3 | ViewSlot replace/animate | `component.ts` `ViewSlot.setView/setAnimation/stopAnimation` | `setView` **re-materializes the previous View from scratch on every update** just to obtain a base ref for patch routing, then tries scalar/path/structural/text/cold routes, else `setViewRef(fresh cold ref)`, else N-API. Animations materialize each frame view, install refs, release leases. |
| B4 | ScrollPane content | `scroll-pane.ts` `setContent/followEnd` | Same pattern as B3 (re-materialize previous → route cascade → setContentRef). |
| B5 | Component references | `View.component(handle)` inside trees | Native component ids embedded in the semantic node; resolved by native during layout. No separate lease. |
| B6 | Theme installation | `Tui.setTheme(theme.materialize())` | One-shot host mutation before first render. |

Legacy equivalents map 1:1 (kernel view swap ≈ B1; `history.push/freeze` ≈ B2;
component `set_content`/tick ≈ B3/B4), so fixing the boundaries fixes both
layers' semantics.

## 10. Compromises in the TS layer that retained-DAG FFI can remove

1. **B3/B4 base-ref acquisition is O(previous tree) per update.**
   `tryNativeMaterialize(previous)` cold-builds the entire old view (FFI graph
   walk) on *every* `setView`/`setContent` merely to have a base ref for patch
   attempts. A boundary-held root lease + BridgeNativeHint makes this O(1)
   (the handle keeps its leased current ref; hints make stable descendants
   cutoffs free). This is the single largest structural waste found in the trace.
2. **B1 route cascade is the pre-PERF-12 recipe router.** With T6–T12 landed,
   the cascade should collapse to the §20 exact-root fast path + ensureNative
   frontier materialization + §49 cold router (initial/cold renders choose the
   best cold path directly; oversized frontiers abort into the bulk path).
   The dead recipe routes (path lineage, edit transactions) become removable.
3. **No shared identity across B1 and B2.** Scene chrome and History units
   materialize independently; shared subtrees (e.g. repeated tool-card shells)
   rebuild per unit. Retained identity (NodeId + hints) gives cross-boundary
   cutoffs for free once all boundaries use the same publication funnel.
4. **Async serialization of mutations.** `historyMutation` promise chaining
   orders History mutations and scene renders; each hop yields to the microtask
   queue. With synchronous cheap FFI the app can keep ordering without paying
   a macrotask/microtask per mutation. (Behavior-preserving optimization;
   not required for correctness.)
5. **`bodyKey` string diffing duplicates what identity cutoff provides.**
   It also gates animation advancement (`advance?.(0)`). Once B1 is O(changed
   frontier), the memo can shrink to the trivial "same Scene object" check the
   runtime already performs.
6. **Freeze-time rematerialization of tool cards.** Freezing builds a brand-new
   combined view and materializes it cold (B2). With hints, the frozen view
   shares the live card's already-materialized nodes where identities match.
7. **Theme/style interplay is safe today but unguarded:** `STYLE_REF_CACHE`
   (T11) is generation-scoped, not theme-scoped; `setTheme` after materialized
   views exist would leave stale themed StyleRefs. Fine for the current
   set-theme-once flow; T13 should either forbid post-hoc theme changes or
   fold a theme epoch into the cache key.

## 11. What T13 must deliver for this surface (mapping to §78–§81, §114, §115)

```text
B1  RetainedRootBoundary inside Tui.render: adopt previous body, install new
    body via ensureNative + hostRenderRef, §49 cold router for first/cold
    renders, release-after-success already correct.
B2  History.push/freeze ride the retained path with shared identity (they
    already prefer refs; switch tryNativeMaterialize → ensureNative-backed
    materialization so repeated units hit hints/promotions instead of cold).
B3  ViewSlot holds a leased current-root ref (root lease protocol §18);
    setView = boundary install; animations keep frame-view materialization
    but reuse identity across cycles (frames are stable objects).
B4  ScrollPane.setContent likewise; followEnd untouched (native).
B5  Component references unchanged.
B6  Theme unchanged (document the single-install constraint or add epoch).
Tests: dormant-node recovery (§114) exercised via freeze→dispose→re-push of a
    user batch/tool card view; multi-host (§115) via two hosts rendering the
    same scene/history in headless mode; failure injection at each boundary
    (§118) reusing the T12 harness.
```

Nothing in the traced surface requires new framework concepts: every observed
pattern (chrome swap, unit insert/freeze, slot replace, pane content, stream
append/seal, pulse animation) is expressible as the §18 root-lease protocol
over the existing retained primitives.
