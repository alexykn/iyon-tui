# 12 — controls-scroll

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Investigation mode: read-only static source inspection.
- No source, configuration, generated artifact, test, or documentation files were modified.
- No build, test suite, benchmark suite, or running service was executed.

I read:

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- `AGENTS.md`
- `docs/architecture/atlas-4355c02/evidence/assignments.json`

The assignment manifest identifies this scope as:

> `crates/iyon-tui/src/controls/` recursively, `scroll.rs`, `scroll_command.rs`  
> Goal: “Editing, control state and specialized spatial behavior.”

The report follows the required broad headings and distinguishes current-source facts, static inferences, historical/documentary context, and unknowns.

### Scope

Primary production scope:

- `crates/iyon-tui/src/controls/mod.rs`
- `crates/iyon-tui/src/controls/text_input/mod.rs`
- `crates/iyon-tui/src/controls/text_input/buffer.rs`
- `crates/iyon-tui/src/controls/text_input/command.rs`
- `crates/iyon-tui/src/controls/text_input/cursor.rs`
- `crates/iyon-tui/src/controls/text_input/edit.rs`
- `crates/iyon-tui/src/controls/text_input/output.rs`
- `crates/iyon-tui/src/controls/text_input/presentation.rs`
- `crates/iyon-tui/src/scroll.rs`
- `crates/iyon-tui/src/scroll_command.rs`

Primary tests:

- `crates/iyon-tui/src/controls/text_input/tests/mod.rs`
- `crates/iyon-tui/src/controls/text_input/tests/buffer.rs`
- `crates/iyon-tui/src/controls/text_input/tests/command.rs`
- `crates/iyon-tui/src/controls/text_input/tests/output.rs`
- `crates/iyon-tui/src/controls/text_input/tests/presentation.rs`
- Unit tests embedded in `scroll.rs`

Supporting source inspected to prove seams and consumers:

- `crates/iyon-tui/src/lib.rs`
- `crates/iyon-tui/src/application/host.rs`
- `crates/iyon-tui/src/interaction/command.rs`
- `crates/iyon-tui/src/scene/layout.rs`
- `crates/iyon-tui/src/presentation/layout/tree.rs`
- `crates/iyon-tui/src/presentation/factory.rs`
- `crates/iyon-tui/src/scene/host.rs`
- `crates/iyon-tui-native/src/tui.rs`
- `packages/iyon-tui/src/api/controls/text-input.ts`
- `packages/iyon-tui/src/api/controls/scroll-pane.ts`
- `packages/iyon-tui/src/runtime/runtime.ts`
- selected ownership/API checks and fixture references found by repository-wide search.

### Evidence status

The findings below are based on source inspection and search results. They are not claims of executed behavior. Test names and assertions are treated as behavioral evidence, but the tests were not run during this investigation.

---

## 1. Responsibility and structure

### 1.1 Production inventory

| Path | Approx. physical LOC | Primary responsibility | Secondary responsibility | Public surface |
|---|---:|---|---|---|
| `controls/mod.rs` | 3 | Crate-level control module declaration and `TextInput` re-export | Hides implementation modules | `pub(crate)` module; re-export is crate-visible |
| `controls/text_input/mod.rs` | 309 | Retained `TextInput` component state and orchestration | Component capability registration, editing dispatch, layout-aware scrolling, outputs, public facade methods | `TextInput` is `pub`, but only re-exported through the private implementation crate’s `pub(crate)` surface |
| `controls/text_input/buffer.rs` | 408 production / 32 tests | Canonical Unicode text storage and cursor/edit operations | Kill/yank state, logical row handling, vertical movement | Internal `TextBuffer` |
| `controls/text_input/command.rs` | 136 | Key-to-command mapping and command execution | Modifier policy, insertion filtering, submit dispatch | Internal `TextInputCommand` |
| `controls/text_input/cursor.rs` | 89 | Logical/wrapped row ranges and display-column calculations | Grapheme-cell-width-aware vertical cursor movement | Internal helpers |
| `controls/text_input/edit.rs` | 20 | Input canonicalization and word-separator policy | CRLF/newline/tab normalization | Internal helpers |
| `controls/text_input/output.rs` | 74 | Typed output channels for submission and projected changes | Borrowed `TextChange` snapshots, synchronous projection registration | `TextChange` is `pub` within crate-visible implementation |
| `controls/text_input/presentation.rs` | 85 | Semantic view construction for focused/unfocused and bounded input | Border decoration, cursor view, row viewport and cursor visibility | Internal `TextInput` implementation |
| `scroll.rs` | 222 production / 69 tests | Generic retained visual-row scrolling component | Follow-end/detached state, viewport construction, layout/content extent repair, key command handling | `ScrollPane` is `pub`, but crate-visible through `lib.rs` |
| `scroll_command.rs` | 26 | Generic scroll key mapping | Modifier filtering and command enum | `pub(crate)` only |

Approximate production LOC is the sum of physical line spans in the assigned files, excluding nested `#[cfg(test)]` sections where separable: approximately **1,372 production lines**. Approximate test LOC in the assigned files is approximately **769 lines**, including test-only support types and helper functions. These counts include blank lines and comments because they were derived from source line ranges rather than a compiler/token-based counter.

The line counts should be treated as approximate architectural sizing, not an exact `cloc` result.

### 1.2 Primary responsibilities

The scope contains two different kinds of generic control:

1. **`TextInput`**
   - Owns editable text and cursor state.
   - Converts native key/paste events into generic editing commands.
   - Emits typed semantic outputs.
   - Produces a `View` representation, including cursor styling and bounded scrolling.
   - Uses terminal grapheme-cell widths for vertical cursor movement.

2. **`ScrollPane`**
   - Owns only a visual-row viewport position over a caller-supplied `View`.
   - Supports follow-end and detached modes.
   - Uses `RowViewport` to translate/clamp the rendered content.
   - Receives layout and full-content extent notifications from the scene/layout system.
   - Does not own semantic content anchoring, history ordering, stream offsets, or application policy.

The distinction is explicit in `scroll.rs`:

> “The pane receives its width and height from framework layout. It keeps a visual-row position only; semantic content anchoring belongs to the content and Surface owners rather than this generic viewport.”

That comment is architecturally consequential: `ScrollPane` is a specialized spatial controller, not a content model.

### 1.3 Test inventory and contracts

The assigned tests protect these behaviors:

- Unicode grapheme-atomic editing and cursor movement.
- Cursor char-boundary invariants.
- Word movement/deletion.
- Kill-to-line-start and yank.
- Vertical movement preserving display columns.
- Soft-wrap-aware movement.
- Cell-width rather than UTF-8-byte-width movement.
- Canonicalization policy.
- Key mapping and modifier behavior.
- Single-line versus multiline Enter semantics.
- AltGr insertion.
- Submission and change output routing.
- Output projector ordering and non-clone results.
- Focused cursor painting.
- Cursor placement inside and around extended grapheme clusters.
- Bounded input scrolling and resize behavior.
- Border-aware inner dimensions.
- Empty focused input caret placement.
- `ScrollPane` follow-end/detachment and resize repair.
- Scene-level local scroll command routing and content replacement preservation.

The tests are more than unit verification: they expose the intended ownership boundaries between editing state, physical terminal geometry, semantic view generation, and local interaction routing.

---

## 2. Types, APIs and contracts

### 2.1 `TextInput`

#### Stored state

`TextInput` contains:

```text
buffer: TextBuffer
multiline: bool
focused: bool
submitted: Output<String>
change_outputs: ChangeOutputs
layout_size: Option<Size>
scroll_row: usize
border: Option<BorderSpec>
```

Source: `crates/iyon-tui/src/controls/text_input/mod.rs :: TextInput`.

The state divides into:

- **Semantic/editing state**
  - `TextBuffer`
  - `multiline`
  - `submitted`
  - `change_outputs`
- **Interaction/presentation state**
  - `focused`
  - `layout_size`
  - `scroll_row`
  - `border`

`TextInput` is not `Clone`, intentionally. Its documentation says output channels are stable for the component lifetime and are not duplicated through cloning.

#### Construction and configuration

- `TextInput::new()` creates an empty single-line input.
- `.multiline(bool)` is a builder-style configuration method.
- `.border(BorderSpec)` attaches a semantic border.
- `set_multiline` recanonicalizes existing text without emitting a user-change event.
- `set_text` replaces canonical text and puts the cursor at the end.
- `clear` clears text and kill-buffer state without emitting a change.
- `text`, `is_empty`, `cursor_bytes`, `is_multiline`, `submitted` expose state.

The public Rust declarations are not a public Rust authoring product API in this repository’s effective architecture. `lib.rs` declares the controls module as `pub(crate)` and re-exports `TextInput` as `pub(crate)`. The actual external authoring surface is the TypeScript facade and native addon.

#### Component contract

`impl Component for TextInput`:

- `view()` delegates to `semantic_view()`.
- `capabilities()` registers:
  - focusability;
  - focus-change callback;
  - typed key command mapping and handling;
  - paste handler;
  - layout-change handler.

The control therefore depends on the generic component/interaction kernel, but it does not own focus routing or terminal decoding itself.

### 2.2 `TextBuffer`

`TextBuffer` is the internal editing kernel:

```text
text: String
cursor: usize
preferred_col: Option<usize>
kill_buffer: String
```

Invariants:

- `cursor <= text.len()`
- `text.is_char_boundary(cursor)`

The cursor is stored in UTF-8 byte coordinates, but movement and deletion use Unicode grapheme boundaries. `unicode_segmentation::UnicodeSegmentation` is used for extended grapheme cluster segmentation.

Operations include:

- insertion;
- backward/forward deletion;
- word deletion;
- kill-to-line-start;
- yank;
- grapheme-aware left/right movement;
- word movement;
- logical line start/end;
- vertical movement through a supplied row-range list;
- logical row extraction.

`preferred_col` stores a terminal display column across vertical movement. It is not a UTF-8 byte offset and not a Unicode scalar count.

### 2.3 Canonicalization contract

`edit.rs :: canonicalize` applies this policy:

1. Normalize CRLF and CR to `\n`.
2. Expand tabs to four spaces.
3. Preserve newline for multiline input.
4. Convert newline to a single space for single-line input.
5. Preserve other control characters.

This means programmatic `set_text` and paste are intentionally not equivalent to filtering all terminal controls. The command path rejects control characters for direct key insertion, but programmatic input and paste are normalized rather than fully sanitized.

`WORD_SEPARATORS` is shared by word movement and deletion. It includes shell/editor punctuation such as backticks, punctuation, brackets, slash, and quote characters.

### 2.4 Command API

`TextInputCommand` is crate-private and includes:

- insertion;
- submit;
- newline;
- backspace/delete;
- word deletion;
- kill/yank;
- horizontal, word, line, and vertical movement.

`command_for_key` applies these policies:

- Plain arrows and Home/End map to direct movement.
- Control-B/F/P/N provide Emacs-like movement.
- Alt-B/F and Ctrl/Alt arrows provide word movement.
- Control-U/Y provide kill/yank.
- Enter is `Submit`.
- Shift-Enter in multiline mode inserts newline.
- Control-J/M and literal newline/CR insert newline only in multiline mode.
- Control/Alt combinations permit AltGr printable insertion.
- Super/Hyper/Meta suppress insertion.
- Control characters are otherwise rejected.
- Unsupported framework keys such as Tab, Escape, and Ctrl-C remain unhandled.

Before returning a command, `command_for_key` asks `TextInput::can_execute`. Boundary no-ops are therefore generally not mapped:

- Backspace at byte position zero;
- Delete at text end;
- movement beyond text boundaries;
- line start/end when already there;
- yank with an empty kill buffer.

Vertical movement is always considered executable at mapping time, because the actual row movement depends on layout and wrapped ranges.

`handle_command` returns:

- `Consumed` for a real text edit;
- `Consumed` for every submit, including empty submit;
- `Ignored` for a movement/edit command that produces no state change.

### 2.5 Output contracts

`TextChange<'a>` contains:

```text
text: &'a str
cursor_bytes: usize
```

It exposes:

- `text()`;
- `cursor_bytes()`;
- `is_empty()`.

`output_on_change` registers an ordered generic projector:

```rust
Fn(TextChange<'change>) -> R + 'static
```

Each registration creates a distinct stable `Output<R>`. The projection receives a borrowed snapshot and emits the resulting owned value through `EventCx`.

`submitted()` returns the stable `Output<String>` associated with the input. Submission emits the current text as an owned `String` and does not clear the input.

Documented intent is that `TextChange` represents a text mutation snapshot. The current implementation routes all commands through one `changed` branch, including successful cursor movement. This creates an important contract discrepancy discussed in section 8.

### 2.6 Cursor and geometry contracts

`cursor.rs` deliberately computes display columns from the same stored grapheme widths used by terminal rendering:

- `row_graphemes` tokenizes a row and stores `grapheme_cell_width`.
- `display_col_at` maps a byte cursor to a display column.
- A cursor inside a grapheme snaps to that cluster’s leading edge.
- `cursor_for_display_col` maps a display column to a byte cursor in another row.

This avoids re-segmenting a partial grapheme at the cursor and avoids treating UTF-8 byte length as terminal width.

### 2.7 `ScrollPane`

`ScrollPane` stores:

```text
content: View
mode: ScrollMode
layout_size: Option<Size>
content_extent: Option<Size>
```

`ScrollMode`:

```text
FollowEnd
Detached { top_row: usize }
```

Public methods:

- `new(View)`;
- `set_content(View)`;
- `scroll_up(rows)`;
- `scroll_down(rows)`;
- `page_up()`;
- `page_down()`;
- `scroll_to_start()`;
- `follow_end()`;
- `is_following_end()`.

Internal methods:

- `on_layout_changed(Size)`;
- `on_content_extent_changed(Size)`;
- `map_command`;
- `handle_command`.

The constructor and `set_content` assert that the content does not contain component identity:

```text
"ScrollPane content cannot contain Component identity"
```

This prevents the pane’s content slot from silently carrying retained component ownership under a viewport that is supposed to own only semantic `View` content.

### 2.8 Scroll positioning semantics

For a viewport with total content height `total` and visible height `viewport`:

```text
max_top = total.saturating_sub(viewport)
```

- `FollowEnd` computes `top = total - viewport`.
- `Detached { top_row }` clamps `top_row` to `max_top`.
- Scrolling down to `max_top` changes the mode to `FollowEnd`.
- Scrolling up from follow-end creates detached mode.
- Replacing content preserves detached/following mode and repairs an out-of-range detached top row.
- Resizing repairs detached state but leaves follow-end semantically at the new end.

The mode is semantic viewport state, not merely a paint cache.

### 2.9 Public TypeScript consumers

The TypeScript facade exposes:

- `TextInput` with text, cursor, set/clear, submission, multiline, and `view()`.
- `ScrollPane` with `setContent(View | (() => View))`, `followEnd()`, `view()`, and generic component capabilities.

The TypeScript pane owns retained content-builder and ABI-reference lifecycle. Rust owns the mounted component state and visual scrolling state.

Relevant facade evidence:

- `packages/iyon-tui/src/api/controls/text-input.ts :: TextInput`
- `packages/iyon-tui/src/api/controls/scroll-pane.ts :: ScrollPane`
- `packages/iyon-tui/src/api/controls/scroll-pane.ts :: NativeScrollPane`
- `packages/iyon-tui/src/runtime/runtime.ts :: createTextInput/createScrollPane`

---

## 3. Dependency and ownership map

### 3.1 TextInput dependency graph

```text
native key/paste routing
        │
        ▼
MountedTextInput in application/host.rs
        │
        ▼
TextInput::command_for_key / paste_callback
        │
        ├── TextInput::can_execute
        ├── TextInputCommand mapping
        └── TextBuffer editing
                │
                ├── edit::canonicalize / is_separator
                ├── cursor::grapheme row helpers
                └── physical::grapheme_cell_width
        │
        ├── TextInput::repair_scroll
        │       └── input_wrap_ranges
        │
        ├── ChangeOutputs / Output channels
        │       └── EventCx queue
        │
        └── TextInput::semantic_view
                ├── factory::text / text_with_cursor
                ├── factory::column
                ├── factory::row_viewport
                └── factory::border/container
```

### 3.2 ScrollPane dependency graph

```text
TS ScrollPane / native ABI
        │
        ▼
HostScrollPane
  Arc<Mutex<ScrollPane>>
        │
        ▼
MountedScrollPane component
        │
        ├── Component::view
        │       └── ScrollPane::view
        │               ├── measure_view fallback
        │               └── factory::row_viewport
        │
        ├── on_layout_changed
        │       └── ScrollPane::on_layout_changed
        │
        ├── on_content_extent_changed
        │       └── ScrollPane::on_content_extent_changed
        │
        └── local key routing
                ├── scroll_command::map_scroll_key
                └── ScrollPane::handle_command
```

### 3.3 Ownership and lifetime

#### Direct Rust component usage

A caller creates `TextInput` or `ScrollPane`, registers it in `ComponentRegistry`, and mounts a `View::component(handle)`. The registry owns the component value. The scene stores component identity and capability declarations.

The assigned control modules do not implement their own `Drop` lifecycle.

#### Host-bound usage

`application/host.rs` wraps each stateful control in an `Arc<Mutex<...>>`:

- `HostTextInput` owns `Arc<Mutex<TextInput>>`.
- `HostScrollPane` owns `Arc<Mutex<ScrollPane>>`.

The host:

1. creates the control state;
2. attaches the host;
3. registers a mounted wrapper component;
4. stores the resulting component ID;
5. exposes the handle to the native addon;
6. invalidates the host on external mutation;
7. retires the component through host/native lifecycle on disposal.

`MountedScrollPane` and `MountedTextInput` are separate wrapper components. They do not embed the control’s state by value; they lock shared host state for view and event handling.

#### TypeScript retained content ownership

For scroll panes, the TypeScript facade additionally owns:

- current content `View`;
- retained root boundary;
- builder producer;
- attachment bindings;
- structural publication/commit/abort lifecycle.

Rust owns only the installed `View` and its spatial scrolling state. The TypeScript pane’s `setContent` transaction preserves the old root until successful replacement and explicitly states that scroll state is not changed by content rebuilds.

### 3.4 Native ABI edges

The native addon exposes:

- `scrollPaneRef` to construct a host-bound native pane from a retained view reference.
- `NativeScrollPane.dispose`.
- `componentId`.
- `setContentRef`.
- `followEnd`.

Evidence:

- `crates/iyon-tui-native/src/tui.rs :: Tui::scroll_pane_ref`
- `crates/iyon-tui-native/src/tui.rs :: NativeScrollPane`
- `crates/iyon-tui-native/src/tui.rs :: NativeScrollPane::set_content_ref`
- `crates/iyon-tui-native/src/tui.rs :: NativeScrollPane::follow_end`

For text inputs, the native host exposes the corresponding state accessors and routes submission through the generic output system.

### 3.5 Geometry ownership

`ScrollPane` does not compute full layout itself. It depends on:

- the scene/layout system to provide its allocated `Size`;
- `RowViewport` layout to preserve the unscrolled child geometry;
- the layout tree to compute `ComponentGeometryMap::content_extents`;
- `LayoutSynchronizer` to deliver the extent to the mounted component.

`presentation/layout/tree.rs` explicitly distinguishes full intrinsic content extent from viewport ownership. For a `RowViewport`, `content_extent_in_subtree` reads the first child’s full layout rect rather than the clipped viewport rect.

This is a key ownership seam:

```text
layout computes:
    allocated viewport geometry
    full child/content extent

ScrollPane owns:
    current visual top row
    follow-end vs detached mode

paint/layout machinery owns:
    row translation
    clipping
    physical rendering
```

---

## 4. Execution paths and state transitions

### 4.1 TextInput creation and mount

#### Host-bound path

```text
TuiHost::create_text_input(multiline)
    → HostTextInput::new
    → TextInput::new().multiline(multiline)
    → attach_host
    → register MountedTextInput
    → assign component ID
    → expose through NativeTextInput
```

Evidence: `crates/iyon-tui/src/application/host.rs :: HostTextInput::new`, `TuiHost::create_text_input`, and `MountedTextInput`.

#### First frame

`MountedTextInput::view()` locks the shared state and returns `TextInput::view()`. The latter calls `semantic_view()`.

Before layout is known:

- focused input uses `vf::text_with_cursor` over the full text;
- unfocused input uses styled semantic text;
- border decoration is applied if configured.

After layout is known:

1. `inner_size` subtracts border dimensions.
2. Width-zero content uses a fill-sized empty column.
3. `input_wrap_ranges` computes wrapped rows.
4. The cursor’s wrapped row is found.
5. Every wrapped row becomes a `NoWrap` text child.
6. Only the cursor row gets `text_with_cursor` when focused.
7. A fill-width column is wrapped in a `RowViewport`.
8. Border decoration is applied outside the viewport.

The implementation intentionally places the border on the parent rather than the `RowViewport`: `RowViewport` copies child rows into its own surface, so a border placed on the viewport node could be overwritten by copied first/last rows.

### 4.2 TextInput key path

```text
terminal decoder
    → interaction local key routing
    → MountedTextInput capability
    → TextInput::command_for_key
    → command::command_for_key
    → TextInput::can_execute
    → TextInput::handle_command
    → command::handle_command
    → TextBuffer operation
    → repair_scroll
    → optional ChangeOutputs emission
    → InteractionResult
```

The command handler treats submit separately:

```text
Submit:
    cx.emit(submitted, input.buffer.text().to_owned())
    return Consumed
```

For all other commands:

```text
changed = operation(...)
if changed:
    emit_change(...)
    return Consumed
else:
    return Ignored
```

This means text mutations and successful cursor movements currently share the same output branch.

### 4.3 TextInput paste path

```text
native paste
    → interaction::route_paste
    → focused TextInput paste callback
    → TextInput::handle_paste
    → TextInput::insert_text
    → TextBuffer::insert_text
    → canonicalize
    → repair_scroll
    → emit_change
    → Consumed
```

A paste that canonicalizes to an empty string is ignored. Pasted CRLF is normalized, tabs become four spaces, and newlines depend on multiline mode.

The mounted tests also prove:

- a focused input consumes paste;
- a passive child lets paste bubble to an ancestor;
- a modal scope excludes background components from paste routing.

### 4.4 TextInput programmatic mutation

`HostTextInput::set_text`, `clear`, `set_multiline`, and `set_border`:

1. lock shared `TextInput`;
2. apply the mutation;
3. call `render_host`.

These operations intentionally do not emit `TextChange` or submission output. Rendering invalidation is host-level, not output-channel-level.

### 4.5 TextInput layout and scroll state

`TextInput::layout_changed` records the allocated size and calls `repair_scroll`.

`repair_scroll`:

1. returns if no layout has been received;
2. subtracts border dimensions;
3. resets `scroll_row` to zero for zero width or height;
4. recomputes wrapped ranges for the current inner width;
5. computes the maximum legal scroll row;
6. finds the cursor’s wrapped row;
7. clamps existing scroll;
8. scrolls upward if the cursor is above the viewport;
9. scrolls downward if the cursor is below the viewport;
10. clamps again.

This is cursor-following behavior, not user-driven detached scrolling. The input always repairs its window so the cursor remains visible.

The top border can carry a label such as:

```text
↑ N more
```

when `scroll_row > 0`.

### 4.6 Vertical editing state

Vertical movement uses:

- logical rows when no layout exists;
- width-dependent wrapped rows when layout exists.

The algorithm retains `preferred_col` so moving vertically across a short line and then back to a longer line returns to the original display column. Display columns are based on terminal cell widths, not byte offsets.

### 4.7 ScrollPane creation and mount

#### Direct Rust path

```text
ScrollPane::new(content)
    → component registry registration
    → View::component(handle)
    → Scene/SceneHost mount
```

The content must not contain component identity.

#### Host/native path

```text
TS retained materialization
    → NativeTuiHost.scrollPaneRef
    → TuiHost::create_scroll_pane
    → HostScrollPane::new
    → ScrollPane::new
    → MountedScrollPane registration
    → native handle receives component ID
```

`MountedScrollPane` registers:

- focusability;
- layout-change callback;
- content-extent callback;
- local scroll key mapping and handling.

### 4.8 ScrollPane initial layout

Before receiving a layout size, `ScrollPane::view` returns the content with both width and height set to `Fill`.

Once layout is known:

1. `content_height(width)` uses cached `content_extent.height`, if available.
2. Otherwise it calls `measure_view(&content, width.max(1))`.
3. `top_row` is calculated from follow/detached mode.
4. The content is wrapped in `vf::row_viewport(content, top, None)`.

If width or height is zero, the implementation returns the fill-sized content directly rather than constructing a clipped viewport. The source comment explains this prevents an empty initial view from permanently causing zero geometry and allows later retained updates to remeasure.

### 4.9 ScrollPane layout and extent callbacks

`on_layout_changed` stores the new allocated size and repairs detached mode.

`on_content_extent_changed` stores the latest full content extent and repairs detached mode.

The scene layout path:

```text
layout tree computes ComponentGeometryMap
    → content_extents for component roots
    → LayoutSynchronizer compares delivered extents
    → content_extent_changed callback
    → MountedScrollPane locks HostScrollPane
    → ScrollPane::on_content_extent_changed
```

`presentation/layout/tree.rs` computes content extents specially:

- for `RowViewport`, it reads the first child’s full layout rectangle;
- for `ContentHost`, it reads the content host rectangle;
- otherwise it recursively searches descendants.

`scene/layout.rs` retains a separate `delivered_content_extents` map and invokes the callback only when the `Size` changes.

### 4.10 ScrollPane key path

```text
local key dispatch
    → MountedScrollPane::map_command
    → HostScrollPane state lock
    → ScrollPane::map_command
    → map_scroll_key
    → ScrollPane::handle_command
    → scroll_up/down/page/start/end
    → InteractionResult
```

`map_scroll_key` accepts only no-modifier:

- Up → line up;
- Down → line down;
- PageUp/PageDown;
- Home → start;
- End → follow end.

`ScrollPane::handle_command` currently returns `InteractionResult::Consumed` for every recognized command, regardless of whether scrolling moved the viewport. This includes commands issued before layout is known, page commands whose page size resolves to zero, and boundary no-ops.

### 4.11 ScrollPane content replacement

`set_content`:

1. asserts no component identity;
2. replaces the semantic `View`;
3. clears `content_extent`;
4. repairs detached mode using fallback measurement if layout is known.

It deliberately does not change `mode`. Therefore:

- a detached pane remains detached after content replacement;
- a follow-end pane remains follow-end;
- a detached top row is clamped to the replacement content’s new maximum;
- the next layout/extent cycle can provide a new full extent.

The TypeScript retained boundary separately performs transactional content publication and only installs the new root after successful materialization.

### 4.12 Destruction and disposal

Native `NativeScrollPane::dispose` marks the native handle dead and requests deferred retirement. The actual component is reclaimed after the host’s reconciliation proves it is unmounted.

TypeScript disposal order is:

1. dispose owned builder root;
2. close retained root boundary;
3. clear current view and attachments;
4. dispose native framework handle.

The Rust control itself has no custom destruction protocol, but it is indirectly governed by component registry and host retirement.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation to production route table

| Operation | Primary route | Alternate/compatibility route | Failure/no-op semantics |
|---|---|---|---|
| Insert printable key | `TextInput` key capability → `command_for_key` → `TextBuffer::insert_text` | Control/Alt mappings for AltGr | Unsupported/control/super-modified keys return no command |
| Insert newline | Multiline Enter variant → `insert_text("\n")` | Ctrl-J/M, literal newline/CR, Shift-Enter | Single-line newline forms are unmapped; plain Enter remains submit |
| Paste | Interaction paste route → focused input callback | Ancestor paste handler/modal routing | Empty canonicalized paste is ignored; ancestor receives paste only if child does not consume |
| Submit | Enter → `Submit` → stable submission output | Host route registration or TS output router | Empty submit is valid and consumed |
| Backspace/delete | Key command → grapheme/word deletion | Ctrl-H, Ctrl/Alt modifiers | Boundary no-op returns `Ignored` |
| Vertical cursor motion | Key command → width-aware wrapped rows if laid out | Logical rows before layout | Successful movement currently emits through `change_outputs`; see section 8 |
| TextInput viewport repair | Layout callback or any successful edit → `repair_scroll` | Programmatic setters call host render after mutation | Zero inner dimensions reset scroll row |
| Scroll line/page movement | Local key map → `ScrollPane::handle_command` | Direct Rust methods | Returns false internally at no-op, but command handler still returns `Consumed` |
| Scroll start/end | Home/End → start or follow-end | Native/TS `followEnd()` only exposes end programmatically | End always consumes and changes mode |
| Scroll content replacement | TS retained root publication → native `setContentRef` → Rust `set_content` | Direct Rust `set_content` | Component-bearing content panics; failed TS publication preserves old retained root |
| Scroll content extent update | Layout tree extent map → synchronizer callback | Fallback `measure_view` if no extent is cached | Extent callback is de-duplicated by `Size` |
| Host lock failure | `HostScrollPane`/`MountedScrollPane` lock wrappers | None observed | View falls back to spacer, mapping returns `None`, command handling returns `Ignored`; this masks poisoned-lock failure at the host wrapper boundary |

### 5.2 Scroll fallback and cache-miss behavior

`ScrollPane::content_height` has two routes:

```text
if content_extent exists:
    use content_extent.height
else:
    measure_view(content, width.max(1)).height
```

This is a genuine cache-miss recovery route, not a compatibility implementation. The extent cache is invalidated on `set_content` and refreshed through layout synchronization.

The current cache key is effectively only “latest extent,” not an explicit `(content identity, offered width, revision)` tuple. The `Size` value contains width and height, but `content_height` uses only the cached height. Correctness therefore relies on the scene/layout system delivering a new extent whenever the width-dependent content geometry changes.

### 5.3 Failure masking

The low-level controls use explicit assertions for invalid structure:

- `ScrollPane::new` and `set_content` panic if content contains component identity.
- The cursor/view compiler is expected to fail loudly for an invalid cursor anchor; the test in `controls/text_input/tests/presentation.rs` expects a panic for a non-UTF-8 cursor anchor.

By contrast, host wrappers use best-effort fallbacks for poisoned locks:

- `MountedScrollPane::view` returns `vf::spacer(0)`.
- Scroll key mapping returns `None`.
- Scroll command handling returns `Ignored`.
- `MountedTextInput::view` returns a spacer on lock failure.
- Text input host methods return an error for poisoned locks, but mounted callbacks are more permissive.

This is a cross-layer inconsistency: malformed content is treated as a programming error, while host synchronization failure can be rendered as an empty control or ignored interaction.

### 5.4 Boundary command behavior

`ScrollPane::map_command` is based only on key shape, not current state or layout. Therefore recognized keys are routed to the pane even if:

- no layout size is known;
- the pane has zero dimensions;
- the requested direction is already at a boundary;
- page height is zero.

`handle_command` discards the boolean results from `scroll_up`, `scroll_down`, `page_up`, and `page_down`, then returns `Consumed`.

This is materially different from `TextInput`, where no-op command execution returns `Ignored`.

### 5.5 Absence claims

Repository-wide searches found no additional Rust control implementations under the assigned controls directory beyond `TextInput`. No separate Rust slider, checkbox, button, or generic editable control was found in the inspected source tree.

There are additional TypeScript APIs and native host wrappers for the two assigned controls, but no alternate Rust scroll controller beyond `scroll.rs`.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 TextInput retained state

`TextInput` has no explicit measurement cache or wrapped-row cache.

Retained state:

- canonical text;
- cursor byte offset;
- preferred vertical display column;
- kill buffer;
- current allocated size;
- current first visible wrapped row;
- focus state;
- border.

Derived row ranges are recomputed through `input_wrap_ranges` during:

- vertical movement when layout exists;
- scroll repair;
- semantic view generation.

This keeps the implementation simple but means wrapping work is repeated on edits, movement, layout changes, and view production. The source does not expose counters for these operations.

### 6.2 TextInput invalidation

Invalidation routes:

- host programmatic setters call `render_host`;
- mounted key/paste interactions are handled through the scene interaction path;
- layout callbacks call `repair_scroll`;
- changing multiline mode or text repairs the scroll window immediately.

There is no separate dirty bit owned by `TextInput`. The surrounding retained scene/runtime decides when the component view is re-resolved and repainted.

### 6.3 TextInput width/height dependence

Wrapping depends on:

```text
inner width = allocated width - border widths
```

Vertical cursor movement and scroll repair therefore depend on the current width and inner height.

Border geometry has two effects:

- width changes the wrapped row ranges;
- height changes the visible row count.

The tests explicitly protect this behavior, including top/bottom borders, zero dimensions, previous-height changes, and width resize.

### 6.4 ScrollPane extent cache

`ScrollPane::content_extent` is an optional latest `Size`:

- set by `on_content_extent_changed`;
- cleared by `set_content`;
- bypassed by fallback `measure_view` when absent;
- not explicitly keyed by content revision or offered width.

`repair_detached` uses the cached/fallback content height to clamp a detached `top_row`.

### 6.5 Layout synchronizer cache

Outside the primary files, `scene/layout.rs` maintains:

```text
delivered: HashMap<ComponentId, Size>
delivered_content_extents: HashMap<ComponentId, Size>
```

This prevents duplicate layout and content-extent callbacks. It also marks layout synchronization dirty when a newly delivered extent changes, allowing the component’s repaired viewport state to participate in the next pass.

### 6.6 Specialized spatial behavior

Two distinct spatial policies exist:

#### ScrollPane: follow-end/detached visual scrolling

- Position is visual row based.
- End-following is explicit state.
- Content replacement preserves the mode.
- Layout and content extent changes repair detached positions.
- Semantic content ownership remains outside the pane.

#### TextInput: cursor-following editing window

- Position is wrapped-row based.
- There is no detached mode.
- Scroll follows the cursor after edits and movement.
- Border labels expose hidden rows above.
- The viewport is constructed from manually wrapped text rows.
- Cursor visibility is prioritized over preserving a previous window.

These controls should not be collapsed into one generic scroll-state abstraction without preserving the semantic difference between “user detached from content end” and “editor must keep caret visible.”

### 6.7 Per-frame/per-event work

Observed from source structure:

- `TextInput` wrapping is recomputed on relevant command/layout/view paths.
- `ScrollPane` may call `measure_view` when no content extent has been delivered.
- Once an extent is delivered, `ScrollPane` uses the retained height rather than measuring every call.
- `RowViewport` paint geometry applies a row translation and clipping in the layout tree.
- Incremental paint explicitly searches ancestors for `RowViewport` and applies the same offset as full-tree compositing.

No counters specific to `TextInput` or `ScrollPane` were found in the assigned source. The broader repository has performance counters for component geometry and layout, but this assignment did not find control-specific operation counters.

### 6.8 Potential performance hazard

`TextInput::semantic_view` constructs one semantic text child per wrapped row on each view build. There is no retained row-view cache. This is appropriate for a compact generic control but is a potential hot path for very large multiline input.

`ScrollPane` avoids rebuilding semantic content on scroll; it clones the retained `View` and changes the `RowViewport` skip amount. That is a favorable separation: scrolling changes viewport metadata rather than the underlying content tree.

---

## 7. Tests, benchmarks and observability

### 7.1 Assigned tests

#### `TextBuffer`

`controls/text_input/tests/buffer.rs` protects:

- grapheme-atomic movement/deletion;
- ZWJ insertion and char-boundary safety;
- forward deletion of an extended grapheme;
- separator-aware word behavior;
- kill/yank;
- vertical preferred-column behavior;
- soft-wrap row motion;
- terminal-cell width behavior;
- property-based cursor invariants;
- canonicalization behavior.

The property test applies up to 64 random operations to arbitrary Unicode strings and asserts cursor invariants after each operation.

#### Command mapping

`tests/command.rs` protects:

- framework keys remaining available to outer routing;
- Alt word movement;
- Ctrl-U/Ctrl-Y;
- Emacs control movement;
- single-line/multiline Enter behavior;
- AltGr printable characters;
- no-op command handling.

#### Outputs

`tests/output.rs` protects:

- stable submission output identity;
- owned submission payload;
- empty submission;
- ordered typed change projectors;
- non-clone projected values;
- no output from programmatic mutation;
- no output from a no-op cursor movement.

The last test does not prove that a successful cursor movement emits no change; it only tests a cursor movement at a boundary where no movement occurs.

#### Mounted interaction

`tests/mod.rs` protects:

- mounted paste routing;
- deferred output after key dispatch;
- ancestor bubbling;
- modal containment.

#### Presentation

`tests/presentation.rs` protects:

- focused cursor rendering;
- empty/trailing cursor visibility;
- logical-line cursor placement;
- one physical cursor cell for wide graphemes;
- combining marks;
- cursor snapping inside an EGC;
- loud failure for invalid UTF-8 cursor anchors;
- width-dependent wrapping and vertical movement;
- multiline growth after previous height;
- bordered empty input caret location;
- bounded scrolling and resize;
- unfocused equivalence to ordinary semantic text.

#### ScrollPane

Embedded `scroll.rs` tests protect:

- initial follow-end behavior;
- detachment after upward scrolling;
- detached position preservation across content replacement;
- explicit follow-end restoration;
- resizing from one row to eight rows without stale previous-height clipping;
- end-follow restoration after scrolling down;
- resize repair while following end.

`scene/host.rs :: routes_scroll_pane_locally_and_preserves_detachment_on_content_update` additionally protects mounted local key routing and detached-state preservation across content replacement.

### 7.2 Broader tests relevant to spatial seams

`presentation/paint/view.rs` contains tests for:

- viewport scroll and geometry cache safety;
- `RowViewport` content-host windows;
- child decoration/style preservation;
- nested nonzero-origin paint equivalence;
- incremental/full paint consistency.

`presentation/layout/tree.rs` and `scene/layout.rs` tests are not assigned to this report, but they are architectural evidence for the `content_extent_changed` callback and row-translation contract.

### 7.3 Benchmark and instrumentation status

No benchmark specifically dedicated to `TextInput` or `ScrollPane` was found in the assigned files. No control-specific counter was found.

Broader geometry/layout counters exist, including component geometry traversal, but the assigned controls do not expose:

- count of text wrapping computations;
- count of scroll fallback measurements;
- count of content-extent callbacks;
- count of cursor row repairs;
- count of consumed/no-op scroll commands.

### 7.4 Missing observability

The following behaviors would be difficult to diagnose from current counters/logging:

- stale or incorrectly timed `content_extent` delivery;
- repeated `measure_view` fallback use;
- successful cursor movement emitting change outputs;
- recognized scroll commands consumed without moving;
- host lock poisoning turning into a spacer/ignored interaction;
- repeated `TextInput` row wrapping for unchanged text and width.

No claim is made that these are production defects; they are visibility gaps observable from source.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Framework boundary is generally respected

The assigned Rust code is generic:

- `TextInput` is a generic Unicode editor.
- `ScrollPane` is a generic semantic viewport.
- No Iyon-specific agent, assistant, transcript, tool, provider, or product policy names appear in the assigned controls.
- Content and styles are caller-supplied.
- The Rust crate documentation explicitly says Rust applications do not author Views through the crate and that TypeScript is the intended public facade.

This matches the framework boundary in `AGENTS.md`.

### 8.2 Rust declarations are public in syntax but crate-private in architecture

`TextInput` and `ScrollPane` use `pub` methods/types internally, but:

- `controls` is `pub(crate)`;
- `scroll` is `pub(crate)`;
- `lib.rs` re-exports `TextInput` and `ScrollPane` with `pub(crate)`;
- the crate itself is described as an unpublished implementation crate.

Therefore the Rust `pub` declarations are primarily intra-crate/native-binding surfaces, not intentional public Rust authoring APIs. The actual external surface is:

```text
TypeScript facade → N-API/native addon → HostTextInput/HostScrollPane → Rust control state
```

This is an important distinction for any architecture census: making these Rust symbols “more private” would not by itself remove their runtime responsibilities.

### 8.3 Specialized spatial logic crosses presentation and interaction planes

`ScrollPane` requires all of the following:

- local key-command registration;
- focusability;
- layout allocation;
- full content extent extraction;
- `RowViewport` construction;
- incremental paint translation;
- host invalidation and retained content replacement.

The control is not merely an input handler, and `RowViewport` is not merely a layout primitive. The behavior is split across:

```text
scroll.rs
application/host.rs
interaction/command.rs
scene/layout.rs
presentation/layout/tree.rs
presentation/factory.rs
paint/view.rs
```

Deleting or relocating the control without preserving this seam would break either local key routing or row translation during incremental painting.

### 8.4 `ContentHost` and `RowViewport` share the extent callback mechanism

`ScrollPane` comments that its content-extent hint lets `ContentHost` content use the same controller without making `ContentPort` own scroll state.

The layout tree confirms that `content_extent_in_subtree` treats both `RowViewport` and `ContentHost` as special extent sources. This means the generic scroll controller is deliberately designed to operate above either ordinary semantic `View` content or retained content-host projections.

That is a useful surviving runtime seam independent of any particular content model.

### 8.5 TextInput output contract discrepancy

The source documents `TextChange` as:

> “A borrowed snapshot of a `TextInput` after a user text mutation.”

`output_on_change` is documented as a projection of user text changes.

However, `command::handle_command` emits `input.emit_change(cx)` whenever any command reports `changed`, including:

- `MoveLeft`;
- `MoveRight`;
- `MoveWordLeft`;
- `MoveWordRight`;
- `MoveLineStart`;
- `MoveLineEnd`;
- `MoveUp`;
- `MoveDown`.

Those operations mutate cursor state but not text. Therefore a successful cursor movement statically appears to emit a `TextChange`.

The existing test named `programmatic_mutation_and_cursor_movement_emit_no_change` only calls `MoveRight` while the cursor is already at the end, so it exercises a no-op and cannot disprove this route.

This is a concrete source/test/documentation contradiction that should be retained as an open architectural finding. It is not resolved here because this assignment is read-only.

### 8.6 Scroll no-op contract differs from TextInput

`TextInput` maps many boundary no-ops to `None` and returns `Ignored` for direct no-op execution.

`ScrollPane` maps keys without checking state and always returns `Consumed` from `handle_command`, even if:

- the layout is absent;
- dimensions are zero;
- movement is already at the boundary;
- page size is zero.

This may be intentional to keep recognized scroll keys from bubbling, but the behavior is materially different from the editing control and is not visible through a result distinction at the mounted route.

### 8.7 Host error handling is inconsistent

Low-level invalid content is asserted loudly. Host lock poisoning is silently converted to:

- spacer view;
- absent key mapping;
- ignored command.

This means the same control can fail loudly when directly manipulated and fail invisibly when accessed through the host wrapper. The behavior is outside the primary control state but directly affects the control’s runtime observability and failure semantics.

### 8.8 TypeScript and Rust ownership are intentionally split for ScrollPane

The TypeScript `NativeScrollPane` owns:

- builder execution;
- root leases;
- structural replacement;
- attachment binding;
- direct versus builder content ownership.

The Rust `ScrollPane` owns:

- visual scroll mode;
- detached top row;
- layout size;
- content extent hint;
- viewport view construction;
- keyboard scrolling.

The TypeScript implementation explicitly says content rebuilds must not change viewport/follow-end state. Rust’s `set_content` preserves `mode`, confirming the split.

### 8.9 Content identity prohibition is narrower than retained content ownership

`ScrollPane` rejects `View` content containing component identity. The TypeScript facade nevertheless supports retained builder content and structural root leases. Therefore retained content ownership exists, but component identity is kept outside the pane’s content root.

This is a deliberate seam rather than an absence of retained content:

```text
TS retained root/content ownership
    ≠
Rust component identity nested in ScrollPane content
```

### 8.10 Current source versus historical V5 instructions

`PRE-V5-ARCHITECTURE-REPORT.md` asks for responsibility decomposition before deciding whether current abstractions survive. This report records the current control ownership and seams only. It does not make V5 deletion, preservation, or migration decisions.

The strongest current-source evidence relevant to later architecture work is:

- `ScrollPane` contains genuine generic spatial state and cannot be equated with a content model.
- `TextInput` contains genuine Unicode editing behavior and terminal-cell-aware cursor geometry.
- Both controls are entangled with current `Component` capability registration and `View` production.
- Their surviving responsibilities can be separated conceptually from the current wrapper/host/layout machinery, but no future owner is selected here.

---

## 9. Open questions and coverage gaps

1. **Successful cursor movement output**
   - Should `output_on_change` emit for cursor-only movement?
   - If not, should command handling distinguish text mutation from cursor movement?
   - Existing tests do not cover successful cursor movement with a registered change projector.

2. **Recognized scroll no-op consumption**
   - Is it intentional for PageUp/PageDown/Home/End and arrow keys to be consumed before layout is known or at boundaries?
   - Should `ScrollPane::handle_command` return `Ignored` when no position changes, as `TextInput` does?

3. **Content extent cache key**
   - Is the latest `Size` sufficient as an implicit cache key?
   - Does every width-dependent content change reliably trigger `on_content_extent_changed` before the next `ScrollPane::view`?
   - The code relies on layout synchronization but does not retain an explicit width/content revision key in `ScrollPane`.

4. **Extent callback ordering**
   - The layout synchronizer marks itself dirty after delivering a new extent. The exact number and ordering of resolve/render passes needed before the repaired viewport appears should be validated with an executed host test.

5. **Content extent for nested structures**
   - `content_extent_in_subtree` takes the first child of a `RowViewport` and recursively finds the first special content node for ordinary containers. It is not obvious from the assigned scope whether multiple independently sized content roots inside one pane are intentionally unsupported or simply resolved by first-match semantics.

6. **Large content limits**
   - `ScrollPane::view` clamps `top` to `u16::MAX` before passing it to `RowViewport`.
   - Layout sizes and content extents are also `u16`-based.
   - The behavior for content taller than 65,535 rows is not covered in the assigned tests.

7. **Very large multiline TextInput**
   - Wrapping and semantic row-view construction are repeated without a retained row cache.
   - No benchmark establishes acceptable behavior for large buffers.

8. **Control character policy**
   - Direct key insertion rejects control characters, while `set_text` and paste preserve non-newline controls.
   - Whether this distinction is intentional API policy or an undocumented inconsistency is not resolved by the assigned tests.

9. **Cursor preferred-column lifecycle**
   - `preferred_col` is cleared by horizontal movement and edits, but behavior when hitting the top/bottom row and then reversing direction is not exhaustively specified.

10. **Host poison handling**
    - It is unclear whether lock poisoning should be treated as an unrecoverable framework failure rather than converted to an empty view/ignored event.

11. **Rust direct API reachability**
    - The crate-level documentation says Rust callers do not author through this crate, but in-crate tests and native binding code use the same `pub` declarations.
    - The desired distinction between implementation visibility and native-binding reuse is architectural, not enforced by a separate Rust public/private type split.

12. **Test execution**
    - No tests were run during this investigation. The source contracts above should be confirmed by the repository’s normal focused Rust test command, especially:
      - controls/text-input tests;
      - `scroll.rs` tests;
      - scene host scroll routing;
      - layout/content extent synchronization;
      - paint `RowViewport` incremental paths.

13. **Uninspected files**
    - The repository-wide source manifest, all unrelated native/TypeScript tests, full benchmark suites, and all generated ABI bodies were not read line-by-line.
    - They were only searched where needed to prove control consumers and ownership edges.

---

## 10. Evidence appendix

### 10.1 Primary source paths and symbols

#### Control module

- `crates/iyon-tui/src/controls/mod.rs`
  - `text_input` module declaration
  - `TextInput` re-export

#### TextInput orchestration

- `crates/iyon-tui/src/controls/text_input/mod.rs`
  - `TextInput`
  - `TextInput::new`
  - `TextInput::set_text`
  - `TextInput::set_multiline`
  - `TextInput::clear`
  - `TextInput::submitted`
  - `TextInput::output_on_change`
  - `TextInput::move_up`
  - `TextInput::move_down`
  - `TextInput::repair_scroll`
  - `TextInput::layout_changed`
  - `TextInput::semantic_view` via `presentation.rs`
  - `impl Component for TextInput`

#### Text editing

- `crates/iyon-tui/src/controls/text_input/buffer.rs`
  - `TextBuffer`
  - `set_text`
  - `recanonicalize`
  - `insert_text`
  - `backspace`
  - `delete`
  - `delete_word_backward`
  - `delete_word_forward`
  - `kill_to_line_start`
  - `yank`
  - horizontal/word/line movement
  - `move_up_in_rows`
  - `move_down_in_rows`
  - `assert_invariant`

- `crates/iyon-tui/src/controls/text_input/edit.rs`
  - `canonicalize`
  - `is_separator`
  - `WORD_SEPARATORS`

- `crates/iyon-tui/src/controls/text_input/cursor.rs`
  - `logical_line_ranges`
  - `wrapped_line_index_by_start`
  - `display_col_at`
  - `cursor_for_display_col`
  - `row_graphemes`

#### Input commands

- `crates/iyon-tui/src/controls/text_input/command.rs`
  - `TextInputCommand`
  - `command_for_key`
  - `handle_command`
  - `is_word_modifier`
  - `can_insert_character`

#### Outputs

- `crates/iyon-tui/src/controls/text_input/output.rs`
  - `TextChange`
  - `ChangeProjector`
  - `TypedProjector`
  - `ChangeOutputs`

#### Input presentation

- `crates/iyon-tui/src/controls/text_input/presentation.rs`
  - `TextInput::decorated`
  - `TextInput::semantic_view`
  - row construction
  - border/viewport placement

#### Scroll

- `crates/iyon-tui/src/scroll.rs`
  - `ScrollMode`
  - `ScrollPane`
  - `ScrollPane::new`
  - `set_content`
  - `scroll_up`
  - `scroll_down`
  - `page_up`
  - `page_down`
  - `scroll_to_start`
  - `follow_end`
  - `top_row`
  - `content_height`
  - `repair_detached`
  - `on_layout_changed`
  - `on_content_extent_changed`
  - `map_command`
  - `handle_command`
  - `impl Component for ScrollPane`

- `crates/iyon-tui/src/scroll_command.rs`
  - `ScrollCommand`
  - `map_scroll_key`

### 10.2 Supporting runtime and geometry paths

- `crates/iyon-tui/src/lib.rs`
  - module visibility
  - crate-visible `TextInput` and `ScrollPane` re-exports

- `crates/iyon-tui/src/application/host.rs`
  - `HostTextInput`
  - `HostScrollPane`
  - `MountedTextInput`
  - `MountedScrollPane`
  - host creation, invalidation, setters, scroll command forwarding, and retirement

- `crates/iyon-tui/src/interaction/command.rs`
  - `ComponentCapabilities`
  - `ComponentCx::on_layout_changed`
  - `ComponentCx::on_content_extent_changed`
  - key command capability registration

- `crates/iyon-tui/src/scene/layout.rs`
  - `LayoutSynchronizer`
  - `delivered`
  - `delivered_content_extents`
  - callback delivery and dirty synchronization

- `crates/iyon-tui/src/presentation/layout/tree.rs`
  - `LayoutContent::RowViewport`
  - `LayoutContent::ContentHost`
  - `ComponentGeometryMap::content_extents`
  - `content_extent_in_subtree`
  - component geometry patching
  - incremental paint geometry and row translation

- `crates/iyon-tui/src/presentation/factory.rs`
  - `row_viewport`
  - `row_viewport_default`
  - `bounded_row_viewport`

- `crates/iyon-tui/src/scene/host.rs`
  - `routes_scroll_pane_locally_and_preserves_detachment_on_content_update`
  - host resolve/render and layout synchronization behavior

- `crates/iyon-tui-native/src/tui.rs`
  - `Tui::scroll_pane_ref`
  - `NativeScrollPane`
  - `dispose`
  - `component_id`
  - `set_content_ref`
  - `follow_end`

### 10.3 TypeScript facade paths

- `packages/iyon-tui/src/api/controls/text-input.ts`
  - `TextInputOptions`
  - `TextInput`
  - `TextInput::submitted`
  - `createTextInput`

- `packages/iyon-tui/src/api/controls/scroll-pane.ts`
  - `ScrollPane`
  - `NativeScrollPane`
  - `buildPaneHandle`
  - retained boundary ownership
  - `setContent`
  - `setContentDirect`
  - `followEnd`
  - `dispose`
  - `createScrollPane`

- `packages/iyon-tui/src/runtime/runtime.ts`
  - `createTextInput`
  - `createScrollPane`

### 10.4 Tests read

- `crates/iyon-tui/src/controls/text_input/tests/mod.rs`
- `crates/iyon-tui/src/controls/text_input/tests/buffer.rs`
- `crates/iyon-tui/src/controls/text_input/tests/command.rs`
- `crates/iyon-tui/src/controls/text_input/tests/output.rs`
- `crates/iyon-tui/src/controls/text_input/tests/presentation.rs`
- embedded `#[cfg(test)]` module in `crates/iyon-tui/src/scroll.rs`
- relevant scene host scroll test in `crates/iyon-tui/src/scene/host.rs`

### 10.5 Commands/search methods used

Read-only repository searches were performed with path-scoped file listing and content search to:

- inventory assigned files;
- locate all `ScrollPane` references;
- locate content extent callback registration and delivery;
- locate native addon exports;
- locate TypeScript facade consumers;
- locate relevant tests and host paths;
- inspect exact source lines and symbols.

No write, install, build, benchmark, or test command was run.

### 10.6 Files indexed but not read comprehensively

The following were identified through repository search but were not read in full because they are outside the assignment:

- complete TypeScript runtime and transport implementation;
- all native addon files beyond the `NativeScrollPane` portions needed for the boundary;
- all generated ABI/schema files;
- all repository benchmarks;
- all unrelated Rust controls, presentation, content, history, and backend modules;
- all consumer fixture tests except references relevant to `ScrollPane`.

### 10.7 LOC methodology

Counts were estimated from physical source line ranges returned during inspection:

- production lines exclude separable nested test modules;
- test lines include test helpers and test-only support types;
- comments and blank lines remain included;
- no formatter/compiler/token-based count was executed;
- figures are architectural sizing estimates rather than exact code metrics.