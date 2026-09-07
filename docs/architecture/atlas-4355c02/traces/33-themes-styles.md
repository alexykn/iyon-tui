# 33 — Themes and styles: TypeScript authoring through rendered cells

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `/Users/alxknt/github/iyon-n/iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Parent-added atlas documentation is outside the source baseline.
- Investigation was read-only. No source files, configuration, generated files, or running services were modified.
- No tests or builds were executed by this scout. Behavioral claims based on tests are static evidence from the test source, not execution observations.

The investigation followed:

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- `AGENTS.md`

The framework boundary in `AGENTS.md` is consequential here: the repository is a generic TUI framework. Theme/style values, selectors, content roles, focus predicates, and state dimensions are framework primitives; application-specific meaning must be supplied by callers through generic names and values. No application/product theme policy was assumed.

### Scope

Primary scope:

- TypeScript style and theme APIs.
- Initial style construction and normalization.
- Dynamic theme replacement.
- Dynamic selector/state changes.
- Ordinary `View` content and styled spans.
- Generic controls, especially `TextInput`.
- `History` static/live/frozen units.
- The path from semantic styles through native retained materialization, Rust theme resolution, physical styles, and rendered cells.

Included seams:

```text
TS public API
  → semantic View/style records
  → retained structural materialization
  → native StyleRef/style atom table
  → Rust retained View / layout / paint
  → ThemeResolver and selector cascade
  → PhysicalStyle
  → PhysicalCell / screen buffer / native scrollback
```

Also included:

- `ViewState` presentation overrides and style-state overrides.
- Text semantic selectors (`TextSelector`) and content rendering.
- Theme/style invalidation and cache keys.
- History native transfer and frozen physical rows.
- Initial control styling and focus-dependent presentation.

Out of scope:

- Product/plugin behavior absent from this repository.
- General layout census except where style invalidation or cell production depends on layout.
- Full stream/projection architecture except at the content/theme cache seam.
- V5 redesign or disposition decisions.
- Rust public-authoring migration analysis.

### Facts, inferences, and unknowns

- **Current-source fact:** TypeScript `Theme`, `StyleSpec`, `StyleRef`, `StyleSelector`, `TextSelector`, `View`, `ViewState`, and `Tui.setTheme` are implemented as described below.
- **Current-source fact:** The production structural route is the semantic retained route. `retained-dag.ts` directly materializes semantic nodes and style references into the native ABI.
- **Current-source fact:** Theme replacement invalidates Rust paint state and causes a full repaint obligation while retaining layout where possible.
- **Current-source fact:** State selector updates can alter rendered styles without republishing the structural View.
- **Current-source fact:** Native-transferred History rows are physical snapshots. Once a row has entered native scrollback or a frozen remainder, its already-resolved `PhysicalStyle` is retained.
- **Inference:** A theme change can recolor resident semantic History units but cannot retroactively recolor physical rows already transferred to terminal scrollback. This follows from the separate semantic and physical frontier structures.
- **Unknown:** No executed test run was performed here, so no claim is made about the current native artifact actually being loadable in this environment.
- **Unknown:** The source does not expose a TypeScript-level API for changing a `TextInput` border after construction. The native host has an internal border setter, but the TypeScript `TextInput` facade exposes only construction-time `border`.

---

## 1. Responsibility and structure

### 1.1 Module inventory

| Path | Approximate production LOC | Test LOC | Primary responsibility | Public/API status | Relevant plane |
|---|---:|---:|---|---|---|
| `packages/iyon-tui/src/api/presentation/style.ts` | 220 | inline validation only | `StyleSpec`, `StyleRef`, selectors, attributes, border types | Public TS API | semantic presentation |
| `packages/iyon-tui/src/api/presentation/theme.ts` | 242 | consumed by TS tests | Immutable `Theme`, theme colors, named styles, text-style entries | Public TS API | semantic presentation |
| `packages/iyon-tui/src/api/presentation/theme-key.ts` | single-digit | none observed | Opaque validated theme key | Public TS API | semantic presentation |
| `packages/iyon-tui/src/api/presentation/semantic-style.ts` | 346 | exercised by H3-A tests | Backend-neutral copying, validation, freezing, style merge | Internal normalization | semantic presentation |
| `packages/iyon-tui/src/api/content/text.ts` | ~180 | exercised broadly | `TextSelector`, `TextSpan`, text role/part selectors | Public TS API | semantic content/presentation |
| `packages/iyon-tui/src/api/view/view.ts` | ~950 | exercised broadly | Immutable semantic View tree, style/decorations, state attachments | Public TS API | structural/semantic presentation |
| `packages/iyon-tui/src/api/view/semantic-node.ts` | not fully line-counted here | exercised by H3-A | Semantic node representation and attachment metadata | Internal | semantic structural/state |
| `packages/iyon-tui/src/api/view/retained-state.ts` | 236 | PERF-13-B/state-envelope tests | Dynamic geometry/presentation/style-state mutation | Public TS API | retained state |
| `packages/iyon-tui/src/api/controls/history.ts` | 132 | History tests | History handle, push/freeze/discard/layout | Public TS API | History/content |
| `packages/iyon-tui/src/api/controls/text-input.ts` | 95 | runtime/control tests | Host-bound TextInput facade and initial border options | Public TS API | controls |
| `packages/iyon-tui/src/transport/structural/retained-dag.ts` | ~2,140 | retained transport tests | Semantic-to-native View materialization, style ref sidecar, identity/caching | Internal transport | structural bridge |
| `packages/iyon-tui/src/transport/structural/encoding.ts` | not fully line-counted here | transport tests | Semantic color/style atom encoding | Internal transport | structural bridge |
| `packages/iyon-tui/src/transport/structural/style-lowering.ts` | 222 | validation/transport tests | Legacy/private native-boundary lowering helpers | Internal; canonical boundary helper | structural bridge |
| `packages/iyon-tui/src/runtime/runtime.ts` | 928 | runtime tests | `Tui`, lifecycle, render, theme replacement, host scheduling | Public runtime API | mixed runtime |
| `packages/iyon-tui/src/index.ts` | 132 | package compile/use | Package-root exports | Public package surface | API surface |
| `crates/iyon-tui/src/presentation/api/style.rs` | ~1,180 including tests | inline Rust tests | Native semantic style, selectors, colors, borders, sparse attributes | Crate-visible runtime API | semantic presentation |
| `crates/iyon-tui/src/theme/mod.rs` | 267 | inline tests | Theme tables, variant ordering, selector resolution | Runtime theme | presentation |
| `crates/iyon-tui/src/theme/framework.rs` | 61 | inline tests | Low-priority generic framework theme defaults | Internal framework policy | presentation |
| `crates/iyon-tui/src/presentation/paint/theme.rs` | ~566 including tests | inline tests | Theme layering, selector matching, semantic-to-physical style resolution | Internal paint | paint |
| `crates/iyon-tui/src/presentation/paint/view.rs` | not fully line-counted here | inline tests | Paint traversal and style context propagation | Internal paint | paint |
| `crates/iyon-tui/src/presentation/paint/text.rs` | not fully line-counted here | inline tests | Styled text span/run paint and cell generation | Internal paint | paint |
| `crates/iyon-tui/src/retained_state/presentation.rs` | ~180 | inline tests | State presentation patches and style-state merge | Internal retained state | state/presentation |
| `crates/iyon-tui/src/retained_state/occurrence.rs` | <100 | inline tests | Effective state/decoration per attached occurrence | Internal retained state | state/presentation |
| `crates/iyon-tui/src/history/model.rs` | 407 | inline tests | Semantic History unit lifecycle and layout cache | Runtime History | History |
| `crates/iyon-tui/src/history/projection/mod.rs` | ~1,100 | inline tests | History semantic projection, viewport selection, frozen overlays | Internal History | History/layout |
| `crates/iyon-tui/src/history/native/mod.rs` | ~520 | inline tests | Native scrollback transfer and frozen physical remainders | Internal History/backend seam | History/backend |
| `crates/iyon-tui/src/history/native/frontier.rs` | 104 | inline tests nearby | Native frontier and physical frozen-row state | Internal | History/backend |
| `crates/iyon-tui/src/scene/host.rs` | >4,000 | extensive inline tests | Theme invalidation, resolve/paint scheduling, History and content integration | Runtime host | mixed |
| `crates/iyon-tui-native/src/tui.rs` | >1,900 | inline tests | N-API host, History, theme, View ABI entrypoints | Native addon | native boundary |
| `crates/iyon-tui-native/src/tui/theme_dto.rs` | 597 | inline tests | Theme DTO decoding and validation | Native boundary | native bridge |

**Counting method:** approximate production LOC is based on source line spans returned by static inspection, excluding generated bodies where practical and not attempting to subtract comments precisely. Large files were reported by responsibility rather than pretending that every line belongs to this assignment. Inline test LOC is noted where tests are colocated; TypeScript test-file counts are listed separately below.

### 1.2 TypeScript responsibility split

The TypeScript implementation separates three related but distinct responsibilities:

1. **Authoring facade**
   - `StyleSpec`, `StyleRef`, `StyleSelector`, `Theme`, `TextSelector`, `TextSpan`.
   - These are immutable semantic values.
   - They do not know terminal cells or physical color formats.

2. **Semantic normalization**
   - `semantic-style.ts`.
   - Copies caller-owned records into frozen backend-neutral values.
   - Validates color channels, attributes, border glyphs, insets, dimensions, and style states.
   - This module is intentionally independent of the wire/native representation.

3. **Retained structural transport**
   - `retained-dag.ts`.
   - Converts stable semantic styles to native style references.
   - Owns generation-scoped acceleration metadata, not the authoritative native style cache.
   - Materializes text spans, decorated Views, overflow styles, and diff styles.

The `View` class composes all of these through immutable derivation. It never directly calls native ABI functions. It builds semantic nodes; retained transport later consumes them.

### 1.3 Rust responsibility split

Rust contains the runtime implementation of the same semantic concepts:

- `presentation/api/style.rs`: style and selector data model.
- `theme/mod.rs`: application theme tables and variant precedence.
- `theme/framework.rs`: generic framework defaults, such as heading/strong/emphasis defaults and diff colors.
- `presentation/paint/theme.rs`: resolves a semantic `StyleRef` against framework and application layers, focus, inherited states, and local facts.
- `presentation/paint/view.rs` and `paint/text.rs`: applies the resolved `PhysicalStyle` to rendered rows/cells.
- `scene/host.rs`: schedules theme invalidation and repaint.
- `history/native/*`: turns resolved rows into native scrollback and retains physical snapshots when transfer is partial.
- `retained_state/*`: applies dynamic state overrides to retained occurrences without changing structural View identity.

---

## 2. Types, APIs and contracts

### 2.1 Public TypeScript style API

#### `StyleSpec`

`StyleSpec` is a sparse immutable style patch:

```ts
interface StyleSpecValue {
  readonly foreground?: ColorSpec;
  readonly background?: ColorSpec;
  readonly attributes: Readonly<Partial<Record<TextAttribute, boolean>>>;
}
```

Defined in `packages/iyon-tui/src/api/presentation/style.ts:34-80`.

Important semantics:

- `new StyleSpec()` means no explicit colors and no explicit attributes.
- `.foreground()` and `.background()` return new `StyleSpec` values.
- `.attribute(name, enabled)` validates the closed vocabulary:
  - `bold`
  - `dim`
  - `italic`
  - `underline`
  - `reversed`
  - `strikethrough`
- `.plain()` explicitly sets every supported attribute to `false`; it does **not** clear foreground/background. This is intentional sparse-style semantics.
- `Style.plain()` and `Style.new()` are convenience constructors at lines 217-220.

`StyleSpec` is not itself a named theme lookup. It is a direct patch.

#### `StyleRef`

`StyleRef` combines optional named-style identity with a sparse local override:

```ts
class StyleRef {
  private constructor(
    readonly themeKey: ThemeKey | undefined,
    readonly local: StyleSpec,
  ) {}
}
```

Defined at `style.ts:83-107`.

Factories:

- `StyleRef.direct(style)` — direct style, no theme key.
- `StyleRef.theme(key, overrides)` — named theme style plus sparse local override.
- `StyleRef.from(style)` — converts a `StyleSpec` to a direct `StyleRef`, preserving a `StyleRef` unchanged.
- `.overrides(patch)` merges sparse local fields.

The named identity is important: a theme change can alter a named style without rebuilding the semantic View. A direct `StyleSpec` carries concrete semantic style intent instead.

#### `StyleSelector`

`StyleSelector` is a positive conjunction of:

- `focused`
- `focusWithin`
- arbitrary caller-owned string state assignments

Defined at `style.ts:110-163`.

Selectors are immutable:

- `StyleSelector.any()`
- `.focused()`
- `.focusWithin()`
- `.state(key, value)`
- `.andFocused()`
- `.andFocusWithin()`
- `.andState(key, value)`

The selector has no negative predicates and no product-specific vocabulary. State keys and values are caller supplied.

#### Colors and theme references

`ColorSpec` is:

```ts
type ColorSpec =
  | ThemeColorReference
  | ThemeColorNamed
  | ThemeColorIndexed
  | RgbColor;
```

Defined at `theme.ts:29-60`.

The notable distinction is:

- `ThemeColor` values define a theme entry (`default`, named ANSI, indexed ANSI, RGB).
- `ColorSpec` values used in a View may reference a theme color (`themeColor("accent")`) or contain an explicit terminal-independent color.
- `StyleRef.theme("field")` references an entire named style, not merely a color.

Theme keys are validated non-empty strings by `ThemeKey` and helper functions.

### 2.2 Public TypeScript `Theme`

`Theme` is immutable. `Theme.new()` creates empty maps; each `with*` method returns a new `Theme`.

Relevant methods at `packages/iyon-tui/src/api/presentation/theme.ts:103-185`:

- `withStyle(key, style)`
- `withStyleVariant(key, selector, style)`
- `withColor(key, color)`
- `withColorVariant(key, selector, color)`
- `withTextStyle(selector, style)`
- `style(key)` — retrieves only the declared base style.
- `color(key)` — resolves only an unconditional color variant for the convenience lookup.

The internal definition projected to the native boundary includes:

```text
styles:
  key → { base?: StyleSpecValue, variants: [{ selector, value }] }

colors:
  key → { base?: ThemeColor, variants: [{ selector, value }] }

textStyles:
  [{ selector: TextSelectorValue, value: StyleSpecValue }]
```

`themeDefinitionFor(theme)` at `theme.ts:188-214` copies maps into plain records. The data is not sent to native as a live JS object; it is subsequently lowered by `materializeTheme()` and decoded by the native DTO.

A notable API property is that all theme construction is immutable on the TS side, while the Rust `Theme` runtime has both immutable-style builders and mutable setters for native assembly. This is not an exposed contradiction: native assembly is a transport/runtime operation.

### 2.3 Text semantic styles and selectors

`TextSelector` in `api/content/text.ts` is a typed facade for generic selector facts:

- roles: paragraph, heading, block quote, list, list item, code block, table, strong, emphasis, link, etc.
- parts: list marker, task marker, quote marker, code label, table rule, thematic rule, image fallback.
- annotations
- language
- origin
- format
- focused/focusWithin
- arbitrary style states

`TextSpan.styled(text, style)` stores a `StyleRef` and later becomes a semantic span style.

The Rust text implementation explicitly documents that `TextSelector` is not a second theme engine. `crates/iyon-tui/src/content/text/style.rs:1-6, 96-107` says it is a convenience facade over ordinary `StyleSelector` matching. Text facts are encoded into `StyleFacts`; the same `ThemeResolver` handles them.

### 2.4 `View` style APIs

`View` is immutable and style changes create derived semantic nodes.

Relevant methods at `packages/iyon-tui/src/api/view/view.ts:365-440`:

- `.bold()`, `.dim()`, `.italic()`, `.underline()`, `.reversed()`, `.strikethrough()`
- `.textAttribute(name, enabled)`
- `.background(color)`
- `.foreground(color)`
- `.border(border)`
- `.style(style)`
- `.state(state)`
- `.styleState(key, value)`

Key distinctions:

#### Direct decoration

`.background`, `.foreground`, `.border`, and direct style patches produce normalized semantic decoration data.

Foreground is stored in the semantic style record rather than as a separate decoration field:

```text
View.foreground()
  → semantic style patch
  → inherited text-style cascade
```

This preserves style ordering and named-style behavior (`view.ts:382-387`).

#### Named style replacement versus sparse overlay

`View.style()` distinguishes:

- a `StyleRef` with a theme key: replace the current named-style identity;
- a direct `StyleSpec`: apply as a sparse overlay.

`view.ts:393-405` explicitly avoids carrying stale local style fields across a named-style replacement.

#### Semantic style state

`.styleState(key, value)` adds a style state to the decoration (`view.ts:417-440`). This is static semantic state, part of the View value. It differs from `ViewState.setStyleState()`, which changes dynamic retained state without reconstructing the View.

### 2.5 Dynamic `ViewState` style API

`packages/iyon-tui/src/api/view/retained-state.ts` provides dynamic retained presentation:

- `setPresentation()`
- `clearPresentation()`
- `setStyleState(key, value)`
- `clearStyleState(key)`

The native resource contract at `retained-state.ts:28-37` includes:

```text
setPresentation(...)
clearPresentation(...)
setStyleState(key, value)
clearStyleState(key)
```

Dynamic style-state updates are validated at the TS boundary (`retained-state.ts:193-204, 232-235`) and sent through the state plane. They do not alter the semantic View tree.

The attached state is added to one View occurrence through `.state(state)`. It is not a general mutable style object and cannot be attached to multiple occurrences or across hosts. The PERF-13-B tests explicitly cover wrong-host and duplicate attachment rejection.

### 2.6 Public controls

`TextInputOptions` exposes only:

```ts
interface TextInputOptions {
  readonly multiline?: boolean;
  readonly border?: BorderSpec;
}
```

`packages/iyon-tui/src/api/controls/text-input.ts:10-24`.

A `TextInput` is host-bound and rendered as a native component View. The border is passed at construction through `Tui.createTextInput()` and lowered by `borderNodeFor()` in `runtime.ts:632-640`.

The TS `TextInput` facade exposes no `setBorder()` method. The native host implementation has an internal `set_border()` path, but that path is not surfaced through the TS class. Therefore:

- initial border style is supported;
- text/input content and focus change dynamically;
- border replacement after construction is not part of the current TS API.

### 2.7 Public History API

`packages/iyon-tui/src/api/controls/history.ts:9-16` exposes:

- `layout()`
- `push(view)`
- `freeze(unit, view)`
- `discardLive(unit)`
- `setLayout(layout)`

`History.push()` and `History.freeze()` both use the retained materialization route:

```text
View
  → tryRetainedMaterializeRef()
  → native History.pushRef()/freezeRef()
  → release temporary View ref
```

`history.ts:62-92`.

The native History handle is attach-once and host-owned after transfer. `freeze` and `discardLive` require an attached History (`history.ts:28-43`).

---

## 3. Dependency and ownership map

### 3.1 Forward dependency diagram

```text
TS caller
  │
  ├─ Theme.new().withColor()/withStyle()/with*Variant()
  ├─ StyleRef.theme()/direct()
  ├─ View.text()/styledText().style()/foreground()/styleState()
  ├─ ViewState.setPresentation()/setStyleState()
  └─ Tui.setTheme()
       │
       ▼
TS semantic values
  │
  ├─ semanticColorFor()
  ├─ semanticStyleFor()
  ├─ semanticDecorationFor()
  └─ semanticTextSpanFor()
       │
       ▼
Immutable SemanticViewNode / SemanticTextSpan / attachment IDs
       │
       ▼
RetainedRootBoundary → retained-dag.ensureSemanticNative()
       │
       ├─ generation-scoped StyleRef WeakMap
       ├─ style atom sidecar
       └─ styleCreateBits(native style table)
       │
       ▼
Native retained View
       │
       ├─ layout/measure cache
       ├─ state occurrence overlay
       ├─ component focus scope
       └─ content host / History projection
       │
       ▼
Rust ThemeResolver
       │
       ├─ framework Theme
       ├─ application Theme
       ├─ inherited StyleStates
       ├─ local StyleFacts
       ├─ focused/focusWithin
       └─ local StyleRef override
       │
       ▼
PhysicalStyle
       │
       ▼
PhysicalRow / PhysicalCell
       │
       ├─ screen surface
       └─ native History scrollback transfer
```

### 3.2 Ownership and lifetime

| Object | Created by | Strong owner | Destroyed/released by | Identity/lifetime notes |
|---|---|---|---|---|
| `StyleSpec` / `StyleRef` | TS caller | semantic View/span or caller | JS GC | Immutable; no native lease |
| `Theme` | TS caller | runtime during `setTheme` lowering; caller may retain immutable value | JS GC | Replacement creates a new native Rust theme |
| Semantic View node | TS `View` construction/derivation | View object / semantic DAG / retained root | JS GC after root and references disappear | Fresh semantic identity for immutable derivations |
| Native View ref | retained-dag ABI materializer | native retained runtime/root/History | native release protocol | Temporary leases drain after publication or History push |
| Style atom | retained-dag → native | native style table; TS Map is acceleration metadata | native runtime generation teardown | String atom identity is cached per runtime/generation |
| Native style ref | `styleCreateBits` | native style table and View payloads | native style table/runtime teardown | TS WeakMap is not authoritative |
| `ViewState` | `Tui.viewState()` | TS caller plus native resource/attachment lease | explicit disposal/TUI cleanup | One host, attachment restrictions |
| `History` | `new History()` or `Tui.createHistory()` | caller or TUI | explicit/TUI cleanup | Detached-to-attached transition is one-way |
| `HistoryUnit` | Rust History push | History semantic unit deque | discard/freeze/retirement | Unit ID stable during semantic lifetime |
| Frozen physical rows | Rust native History transfer | `NativeFrontier` | after accepted transfer/retirement | Physical style snapshot; no theme identity retained |

### 3.3 Theme/style ownership boundaries

- TS owns caller-facing immutable declarations.
- Native DTO owns validation and conversion from JSON-like materialized theme values to canonical Rust theme records.
- Rust `Theme` owns variant tables and selector resolution.
- `ThemeResolver` owns framework/application layer precedence and conversion to `PhysicalColor`/`PhysicalStyle`.
- Paint owns applying resolved styles to cells.
- The terminal/backend owns actual writes and buffer comparison.
- History owns semantic ordering and native transfer state; it does not own theme policy.

### 3.4 Important non-edge

The TS side does **not** resolve theme keys to concrete colors. A TS `ColorSpec` with `{ type: "theme", key }` becomes a semantic theme atom (`theme:key`) or semantic `{ kind: "theme", key }`. Resolution is deferred to Rust paint time.

This deferred resolution is what allows resident Views and ordinary content to respond to `Tui.setTheme()` without reconstructing the semantic tree.

---

## 4. Execution paths and state transitions

### 4.1 Initial ordinary styled View

Example authoring:

```ts
const view = View.text("hello")
  .foreground(themeColor("accent"))
  .style(StyleRef.theme("label"))
  .bold();
```

Actual path:

1. `View.text("hello")` creates a semantic text node (`view.ts:299-307`).
2. `.foreground(themeColor("accent"))` creates a decorated semantic node whose inherited style contains a theme color reference (`view.ts:382-387`).
3. `.style(StyleRef.theme("label"))` installs named style identity and replaces the earlier named-style identity where applicable (`view.ts:393-405`).
4. `.bold()` creates another sparse style patch.
5. Semantic normalization copies/freeze records:
   - `semanticColorFor()` validates the theme key (`semantic-style.ts:39-55`).
   - `semanticStyleFor()` stores theme identity, colors, and attributes (`semantic-style.ts:58-67`).
6. The retained runtime obtains the semantic root.
7. `retained-dag.ts` reaches `styleRefFor()`:
   - style object lookup in `STYLE_REF_CACHE.refs`;
   - color/theme atoms lookup in `STYLE_REF_CACHE.atoms`;
   - `styleCreateBits()` creates a native style ref (`retained-dag.ts:638-682`).
8. Text spans use fixed-arity cstring or UTF-8 ABI calls for one to four spans (`retained-dag.ts:685-755`), or the variadic buffer lane for more than four spans (`retained-dag.ts:758-796`).
9. Native retained View is installed.
10. Rust layout computes geometry independently of the eventual theme color.
11. Rust paint traverses the retained tree:
    - enters style states/facts and focus context;
    - resolves the named style and color references through `ThemeResolver`;
    - writes `PhysicalStyle` into physical cells.
12. The host commits the visible frame.

The style is therefore semantically immutable after construction, but theme-dependent portions remain dynamically resolvable in Rust.

### 4.2 Initial theme installation

`Tui.open(options)` accepts an optional theme:

```text
Tui.open({ theme })
  → new native host
  → new Tui
  → tui.setTheme(theme)
```

`runtime.ts:319-341`.

`Tui.setTheme()`:

1. Validates runtime state and mutation phase (`runtime.ts:880-882`).
2. Projects `Theme` to plain `ThemeDefinition` (`themeDefinitionFor`).
3. Materializes colors/styles/text styles for the native boundary (`materializeTheme`).
4. Calls native `host.setTheme(lowered)`.
5. Always resets the TS retained style sidecar in `finally` (`runtime.ts:880-894`).

On the native/Rust side:

```text
NativeTuiHost.set_theme()
  → decode_theme()
  → Theme::assemble_batched()
  → host_set_theme()
  → Arc<Theme> replacement
  → SceneHost::invalidate_theme()
  → invalidate frame
  → resolve/paint
```

Evidence:

- N-API entrypoint: `crates/iyon-tui-native/src/tui.rs:776-781`.
- DTO decode/assembly: `crates/iyon-tui-native/src/tui/theme_dto.rs:288-357`.
- Host set theme: `crates/iyon-tui/src/application/host.rs:1325-1329`.
- Kernel theme replacement: `crates/iyon-tui/src/application/kernel.rs:409-413`.

### 4.3 Dynamic theme replacement

The replacement path is intentionally not a structural View replacement:

```text
Tui.setTheme(newTheme)
  → host theme Arc replaced
  → SceneHost.invalidate_theme()
  → paint cache cleared
  → content theme revision advanced
  → full paint pending
  → existing semantic/layout structures reused where valid
  → ThemeResolver resolves all visible semantic styles against new theme
  → PhysicalCell styles change
```

`SceneHost::invalidate_theme()` at `crates/iyon-tui/src/scene/host.rs:430-477`:

- clears `paint_cache`;
- invalidates content layout entries and theme-sensitive content view IDs;
- sets `theme_invalidated`;
- increments content dirty epoch for content roots;
- marks content paint dirty;
- sets `full_paint_pending`;
- increments paint propagation observability.

The source explicitly states that theme changes do not alter terminal metrics in the current engine. Layout can therefore remain retained while paint is regenerated.

The TS style-ref sidecar is reset because its native refs belong to the previous runtime/generation/theme materialization context:

`retained-dag.ts:2128-2138`.

The reset is done in `finally`, including a host failure path. This is conservative and intentional: native theme application may have occurred before a later presentation failure, so stale cached refs must not survive the failed call.

### 4.4 Dynamic selector/state change without structural rebuild

Example from the PERF-13-B test source:

```ts
const theme = Theme.new()
  .withStyle("status", Style.new())
  .withStyleVariant(
    "status",
    StyleSelector.state("status", "error"),
    Style.new().bold(),
  );

tui.setTheme(theme);
tui.render(() => ({
  body: View.text("state")
    .style(StyleRef.theme("status"))
    .state(state),
}));

state.setStyleState("status", "error");
tui.flush();
```

Evidence: `packages/iyon-tui/tests/tui_perf13_b.test.ts:165-182`.

The path is:

```text
View.state(state)
  → semantic state attachment ID
  → native retained occurrence binding
  → ViewState.setStyleState()
  → N-API state resource
  → retained-state record.style_states
  → state wake/invalidation
  → occurrence effective_style_states = base states + state overrides
  → Rust paint context
  → ThemeResolver selector match
  → physical cell style update
```

Rust state details:

- `retained_state/record.rs:143-165` updates/removes style-state key/value, increments state/presentation revisions, and emits presentation effects.
- `retained_state/presentation.rs:172-177` overlays dynamic state assignments on base `StyleStates`.
- `retained_state/occurrence.rs:70-76` refreshes effective occurrence state.
- `presentation/paint/theme.rs:23-40` carries inherited states and clears local facts when descending.
- `theme/mod.rs:242-259` resolves named style base plus matching variants.
- `presentation/paint/theme.rs:97-112` applies framework named style, application named style, and local sparse override.

The dynamic update does not reconstruct the TypeScript `View`, republish the structural DAG, or recreate the `StyleRef`. It changes the selector inputs used by paint.

### 4.5 Static `.styleState()` versus dynamic `ViewState.setStyleState()`

These are semantically similar but architecturally different:

| API | Storage | Structural publication | Typical use |
|---|---|---:|---|
| `View.styleState(key, value)` | Immutable semantic decoration on View | Required when View is first materialized or replaced | Static state declared as part of a semantic View |
| `ViewState.setStyleState(key, value)` | Native retained state record attached to an occurrence | No structural republish | Dynamic state mutation, selector changes while retaining View identity |

The selector cascade can see both because Rust occurrence preparation merges the base semantic state with the retained state override.

### 4.6 Dynamic presentation override

`ViewState.setPresentation()` differs from selector state:

```text
base View style
  + retained state presentation patch
  → effective decoration / style at occurrence
```

Evidence in `tui_perf13_b.test.ts:23-49` shows:

- state changes foreground and `bold`;
- desired structural revision remains unchanged;
- visible frame revision changes after flush;
- clearing the override reveals the View’s original style.

This is a state-plane paint mutation, not a TS structural mutation.

The same source tests multi-span styled text (`tui_perf13_b.test.ts:54-73`), later immutable modifiers (`:75-88`), border color/style changes (`:90-107`), null-versus-clear semantics (`:109-127`), and style-state changes (`:165-186`).

### 4.7 Ordinary content and styled text spans

There are two ordinary content routes:

#### Static `View.text` / `View.styledText`

```text
TextSpan / View.styledText
  → semantic TextSpan
  → retained-dag styleRefFor()
  → native text constructor
  → Rust TextView / TextSpan
  → paint text spans with inherited and local PhysicalStyle
  → PhysicalCell
```

`View.styledText()` snapshots each span through `semanticTextSpanFor()` (`view.ts:310-317`). `TextSpan.styled()` wraps a `StyleSpec` into `StyleRef.from()` (`text.ts:168-180`).

Span style is resolved in Rust `presentation/paint/text.rs`:

- each span style is passed through `ThemeResolver.resolve_text_style`;
- local span facts are supplied using `context.with_local_facts(&span.style_facts)`;
- inherited physical style is preserved for unspecified fields.

Evidence: `presentation/paint/text.rs:156-160, 343-347`.

#### `ContentPort` / retained text content

```text
TextContent / source
  → ContentPort / Connector
  → ContentHost View
  → Rust ContentHostRegistry
  → semantic text projection and TextRenderer
  → text facts / semantic style refs
  → width-dependent layout and theme-dependent paint
  → PhysicalRows / cells
```

The content provider receives the current theme before layout/paint:

`crates/iyon-tui/src/presentation/content.rs:152-165`.

The content cache intentionally separates:

- semantic projection cache — theme-independent;
- prepared paint cache — keyed by theme revision and width;
- text geometry/layout — retained through theme-only changes where metrics remain unchanged.

Evidence from `crates/iyon-tui/src/application/content.rs`:

- semantic projection key excludes theme (`:269-280`);
- prepared paint key includes `theme_revision` and width (`:900-906`);
- comments state semantic IR is theme-independent and theme changes rebuild the paint product, not the semantic IR (`:1082-1089`);
- prepared content retains width-dependent layout while rebuilding theme-resolved physical rows (`:911-914`).

`ContentHostRegistry::set_theme()` at `application/content.rs:6572-6585`:

- ignores pointer/value-identical themes;
- replaces the theme Arc;
- increments `theme_revision`;
- clears connector projection candidates/cache entries that depend on the old theme.

The content route therefore supports dynamic theme replacement without reparsing the source semantic content. Theme changes can update ordinary Markdown/plain-text/diff-like semantic spans and their rendered cells through paint-only or paint-dominant invalidation.

### 4.8 Text semantic styles

The content renderer emits facts such as:

- role (`heading`, `strong`, `link`, etc.);
- part (`listMarker`, `codeLabel`, etc.);
- annotations;
- origin;
- language;
- format;
- task/table/list state.

These facts are local to the current text node/span. `StyleContext` carries inherited states separately from local facts:

`presentation/paint/theme.rs:12-18, 20-45`.

The Rust ThemeResolver applies:

1. framework theme defaults;
2. application named style;
3. application text selector variants;
4. local style patch.

Framework text defaults live in `theme/framework.rs:17-61`, including generic heading, strong, emphasis, underline, link, strikethrough, and diff styles. These are generic framework behavior, not product policy.

### 4.9 Generic controls: `TextInput`

Initial TS path:

```text
Tui.createTextInput({ multiline, border })
  → runtime.borderNodeFor(options.border)
  → native host.textInput(multiline, border)
  → NativeTextInput / HostTextInput
  → MountedTextInput component
  → TextInput.semantic_view()
  → retained component View
  → layout/paint/cells
```

Evidence:

- TS option and facade: `api/controls/text-input.ts:10-24, 44-80`.
- Border lowering and creation: `runtime/runtime.ts:632-640`.
- Native border DTO validation: `tui/theme_dto.rs:360-438`.
- Mounted component: `crates/iyon-tui/src/scene/host.rs:751-767`.

Native `TextInput` presentation:

- border is stored on the native input object;
- `semantic_view()` wraps the input text/viewport in a border;
- focus causes cursor cells to be rendered reversed;
- focus changes are routed through component capabilities and trigger a new rendered View.

Evidence:

- `crates/iyon-tui/src/controls/text_input/presentation.rs:1-84`.
- `crates/iyon-tui/src/controls/text_input/mod.rs:38-80, 207-208, 274-307`.

The cursor style is not a theme-token selector by default; it is a generic control presentation behavior that applies reversed style to the cursor cell. The control's border is semantic and affects geometry, while cursor focus presentation is stateful and can affect paint.

There is no TS `setBorder()` method. The native host has `HostTextInput::set_border()` (`scene/host.rs:657-660`), but no matching TS method was found under `packages/iyon-tui/src`. This is a real API asymmetry and an important limitation for dynamic control border themes.

### 4.10 History push, freeze, and rendering

`History.push(view)`:

```text
TS History.push(view)
  → retained materialization
  → native History.pushRef(ref)
  → Rust History.push(View)
  → Static(View) if no component identity
  → Live(View) if component identity exists
```

Evidence:

- TS route: `history.ts:62-77`.
- Rust semantic classification: `history/model.rs:72-96`.

`History.freeze(unit, finalView)`:

```text
TS History.freeze(unit, finalView)
  → retained materialization
  → native freezeRef
  → History::freeze()
  → require current unit Live
  → reject final View containing component identity
  → replace Live(View) with Static(finalView)
  → invalidate unit layout
  → increment History revision
```

Evidence: `history.ts:80-92`, `history/model.rs:116-127`.

The semantic freeze operation does **not** immediately mean that the unit is already a physical snapshot. It changes the semantic unit from `Live(View)` to `Static(View)` and removes component-driven liveness. While still resident in semantic History, the static View still contains theme references and can be resolved against a replacement theme.

History projection distinguishes:

```text
PlannedContent::Static
PlannedContent::Frozen(FrozenPhysicalRows)
PlannedContent::Live(View)
```

Evidence: `history/projection/mod.rs:48-62`.

A static semantic unit is laid out and painted as a View until native scrollback transfer accepts its rows. A `FrozenPhysicalRows` plan is used when rows have already been partially accepted by native scrollback.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation → production path

| Semantic operation | Production path | Selection/fallback | Failure semantics |
|---|---|---|---|
| `View.text()` | semantic text node → retained materializer → native text constructor | No alternate complete-object route in production | Invalid input throws; native failure becomes retained refusal/preparation failure |
| `View.styledText()` | semantic spans → style refs → fixed-arity or variadic text ABI | Cstring for NUL-free text; UTF-8 buffer for embedded NUL; >4 spans use buffer lane | Empty span list refuses; payload over native cap refuses |
| `.foreground/.background/.border/.style` | semantic decoration → retained decorated View | Direct semantic path; no secondary transport | Validation errors at normalization or native publication |
| `StyleRef.theme()` | semantic named style identity → native style ref with theme atom → Rust theme resolution | Missing named style is a no-op; missing color resolves to default physical color | Native style publication failure is explicit retained refusal |
| `View.styleState()` | semantic decoration state → retained View | No fallback | Invalid empty key/value throws |
| `ViewState.setPresentation()` | state-plane envelope → native record → occurrence patch → incremental paint | Does not republish structure | Invalid patch is rejected atomically |
| `ViewState.setStyleState()` | state-plane mutation → effective state overlay → selector cascade | No View reconstruction | Empty key/value rejected; disposed/wrong-host state rejected |
| `Tui.setTheme()` | TS Theme projection → materialized JSON shape → N-API DTO → Rust Theme | No alternate theme route | Decode/validation error; style sidecar reset occurs in `finally` |
| `History.push()` | retained View ref → native History | No legacy complete-object push path | Could not materialize → explicit `HISTORY_PUSH_FAILED` |
| `History.freeze()` | retained View ref → native History freeze | No alternate path | Could not materialize → `HISTORY_FREEZE_FAILED`; unit/content validity errors |
| TextInput initial border | `TextInputOptions.border` → `borderNodeFor` → native DTO → control semantic View | Construction-time only in TS | Malformed border rejected before host component is left reachable |
| ContentPort theme refresh | `ContentProvider::set_theme` → theme revision/cache invalidation → content paint | Semantic IR retained, paint rebuilt | Provider/cache failures surface through host resolve/paint |

### 5.2 Retained refusal is not route selection

`retained-dag.ts` explicitly documents that retained refusal is not permission to select a previous transport. Relevant comments are at `retained-dag.ts:185-197` and `:416-468`.

The current production route is:

```text
retained semantic materialization
  → success
or
  → explicit preparation failure
```

This matters to style paths because a malformed or unsupported style payload does not silently drop to an older generic object-decoding route.

### 5.3 Validation and masking

Validation is layered:

- TS semantic normalization validates public values before creating semantic records.
- Native DTO validation validates values crossing the N-API boundary.
- Rust internal code assumes established semantic invariants.
- Expected native constructor status codes become retained refusal errors.
- Theme replacement resets style sidecars even after host errors to avoid stale refs.

Examples:

- unknown attributes rejected in `semantic-style.ts:233-245`, `style-lowering.ts:128-136`, and native `theme_dto.rs:99-113`;
- malformed theme colors rejected in native `theme_dto.rs:54-67`;
- malformed border glyphs rejected in TS and native DTO paths;
- missing theme colors do not fail: Rust `ThemeResolver::resolve_color` falls back to `PhysicalColor::Default` (`paint/theme.rs:178-190`).
- missing named styles do not fail: no named style layer is applied, leaving inherited/local style unchanged (`paint/theme.rs:97-112`).

The distinction is consequential:

- malformed input is rejected;
- valid but absent theme keys are compatibility/default cases;
- native publication failures are explicit retained failures;
- missing named style is a semantic no-op, not a transport error.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 TypeScript style-ref cache

`retained-dag.ts:92-103` defines:

```text
STYLE_REF_CACHE:
  runtime
  generation
  refs: WeakMap<object, number>
  atoms: Map<string, number>
```

Properties:

- one native style ref per stable semantic style object per runtime generation;
- one native style atom per string atom per runtime/generation;
- `WeakMap` means style object metadata does not keep semantic objects alive;
- the native runtime's style table remains authoritative;
- cache is reset on runtime/generation change;
- explicit theme change resets all sidecar refs and atoms.

This sidecar is acceleration metadata, not a second semantic theme engine.

### 6.2 Retained structural identity

The semantic View node sidecar (`SEMANTIC_NATIVE`) caches generation-scoped NativeRef hints (`retained-dag.ts:52-60`).

Style interaction:

- stable View identity can reuse a native View ref;
- stable semantic style objects can reuse style refs;
- dynamic `ViewState` changes do not require style ref recreation because the state overlay is applied at native occurrence/paint time;
- dynamic `Tui.setTheme()` invalidates paint even if structural identity remains stable.

### 6.3 Theme invalidation bounds

Rust `SceneHost::invalidate_theme()`:

- clears paint cache globally;
- preserves layout where metrics are unaffected;
- invalidates content-derived paint/layout entries associated with content roots;
- sets a full-paint obligation.

The source comments explicitly reject using theme revisions as layout-input keys for ordinary geometry. This avoids registry-wide relayout on palette-only changes.

The invalidation is broader than only themed cells because the host does not maintain a per-cell “contains theme token” index. It repaints visible semantic rows, while retaining layout products where safe.

### 6.4 Content cache keys

Content caches distinguish:

- source/projection revisions;
- width;
- delivery/frontier;
- theme revision;
- whether finalized/physical rows are needed.

Evidence in `application/content.rs`:

- semantic IR key excludes theme (`:269-280`);
- prepared paint key includes theme revision (`:900-906`);
- theme-only changes reuse semantic projection and generally retain width geometry, then regenerate theme-resolved paint (`:911-914, :1082-1089`).

This is the desired dependency separation for ordinary content:

```text
source change       → semantic + paint work
width change        → layout + paint work
theme change        → paint work, possibly content paint products
delivery tick       → delivery/paint work as needed
```

### 6.5 History layout and physical frontier caches

Semantic History unit height cache keys include:

- static View identity;
- ContentPort projection revision;
- Live View identity plus component dependencies;
- width.

`history/unit.rs:12-35`.

Theme revision is not part of the semantic height key because current terminal metrics are theme-independent. A theme change invalidates paint, not intrinsic dimensions.

Native transfer retains:

- `FrozenStaticRemainder`;
- `FrozenContentRemainder`;
- `SpacingTransferState::Frozen`;
- physical rows already produced by the theme-specific paint pass.

`history/native/frontier.rs:5-69`.

### 6.6 Per-operation/per-frame work

Static observations:

- `Tui.setTheme` performs one theme DTO materialization, one native theme install, sidecar reset, and host invalidation.
- Theme paint invalidation causes a full visible paint obligation, but not necessarily full layout.
- `ViewState` presentation/style-state updates use native state records and incremental invalidation rather than structural publication.
- Ordinary text style refs are created once per stable semantic style object per generation, not per cell.
- Text spans are transported in fixed arity or borrowed buffer lanes; style refs are passed as compact native references.
- History native transfer can avoid recompiling rows already stored as `FrozenPhysicalRows`, but this is also why those rows are no longer theme-reactive.

### 6.7 Important cache/invalidation edge

`Tui.setTheme()` resets TS style-ref caches in `finally`, but the structural semantic tree remains. Therefore a later structural rematerialization creates fresh native style refs that still carry semantic theme atoms, while the current Rust host theme resolves those atoms during paint.

The sidecar reset is necessary for stale native ref safety, but it does not itself cause semantic View recreation.

---

## 7. Tests, benchmarks and observability

### 7.1 TypeScript behavioral evidence

No tests were executed by this scout. The following tests were inspected as source evidence.

#### `packages/iyon-tui/tests/tui_perf13_b.test.ts`

Relevant contracts:

- presentation override without structural republish: lines 23-52;
- styled text state path: lines 54-73;
- attachment identity through later immutable modifiers: lines 75-88;
- border presentation update without geometry change: lines 90-107;
- null versus clear semantics: lines 109-127;
- dynamic style-state selector cascade: lines 165-186;
- invalid/disposed state handling: lines 188-230.

The test at lines 165-182 is the strongest direct evidence for this assignment: a theme variant is declared once, a View retains `StyleRef.theme("status")`, and `ViewState.setStyleState()` changes the rendered `bold` cell state without rebuilding the semantic View.

#### `packages/iyon-tui/tests/tui_retained_scene_regressions.test.ts`

Lines 35-65 cover a theme replacement during a content refresh:

- initial theme color index 1;
- source update;
- replacement theme color index 2;
- rendered `label` cell is expected to use `ansi:2`.

This protects the full-paint theme obligation when content and theme changes overlap.

#### `packages/iyon-tui/tests/tui_h3_a_semantic.test.ts`

Relevant contracts:

- style/border/theme semantic normalization;
- named style and local override representation;
- style-state snapshot and freezing;
- caller-owned records are copied/frozen, not retained mutably.

Key lines include:

- semantic style fixture: 249-269;
- decorated theme/border fixture: 303-309;
- normalized style-state field: 505-506;
- caller snapshot/freeze behavior: 721-749.

#### `packages/iyon-tui/tests/tui_history_prefix.test.ts`

Lines 61-65 cover `History.freeze(live, View.text("final tail"))` followed by rendering. This is a direct TS semantic freeze path, though it does not by itself prove theme behavior for already-transferred physical rows.

### 7.2 Native DTO validation evidence

`crates/iyon-tui-native/src/tui/theme_dto.rs:441-597` includes tests for:

- theme fixture decoding;
- indexed/default/named/RGB forms;
- duplicate selector last-write behavior;
- malformed colors;
- malformed attributes;
- malformed text roles/parts;
- empty text selector accepted as universal selector;
- border decoding and invalid border style/edges.

This confirms that the native boundary rejects malformed inputs rather than silently ignoring malformed leaves.

### 7.3 Rust theme/paint behavioral evidence

`crates/iyon-tui/src/presentation/paint/theme.rs:224-566` tests:

- selector normalization and specificity;
- focused selector precedence;
- missing theme color fallback to physical default;
- missing named-style no-op;
- local sparse style override precedence;
- theme changes affecting paint without changing geometry;
- sparse style variant overlays;
- framework/application layering;
- application explicit default overriding framework color;
- framework style color references resolving through the application palette.

The test named `theme_changes_paint_without_changing_geometry` is especially relevant at `paint/theme.rs:350-367`.

### 7.4 Rust History evidence

`history/model.rs` and the History projection/native modules include inline tests and source invariants around:

- Live-tail restriction;
- final freeze component rejection;
- layout invalidation;
- native transfer;
- frozen physical remainders;
- semantic versus native revisions.

The native frontier types at `history/native/frontier.rs:36-69` provide the most direct evidence for the physical snapshot contract.

### 7.5 Observability

Relevant counters and hooks include:

- retained structural identity counters in `retained-dag.ts:111-151`;
- Rust performance counters for view-state mutation, style-state invalidation, paint propagation, and paint nodes;
- host epochs exposed through testing harnesses;
- `Tui.styleAt(row, column)` testing/runtime access;
- screen row inspection and native History row inspection.

The PERF-13-B source inspects host epochs to prove that dynamic presentation changes do not change the desired structural revision (`:30-43`).

### 7.6 Missing visibility

No direct TS test was found that asserts:

- a theme replacement recolors a History unit after that unit has already been transferred to native scrollback;
- a frozen physical row remains in its old theme after `Tui.setTheme`;
- a dynamic TextInput border replacement through TS, because no such TS setter exists;
- cache entry counts specifically for theme/style refs;
- exact per-cell repaint counts for theme replacement.

These remain coverage gaps rather than claims of incorrectness.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Theme replacement is semantically deferred but physically eager for visible cells

The TS layer stores theme references, not resolved colors. Rust resolves them during paint. On replacement:

- semantic View identity can stay unchanged;
- layout can remain cached;
- visible physical cells are repainted.

This is an effective separation of semantic theme policy from terminal realization.

### 8.2 The native style table is authoritative; TS cache is only acceleration

`retained-dag.ts` comments explicitly prohibit a second authoritative native style cache. The TS `WeakMap` sidecar maps stable semantic style objects to refs for reuse, but native owns actual style records.

This avoids an ownership contradiction between TS and native style lifetimes.

### 8.3 Framework and application theme layers are distinct

Rust `ThemeResolver` first applies framework named styles and then application named styles (`paint/theme.rs:97-112`). Generic defaults are in `theme/framework.rs`, while application values arrive from TS through the theme DTO.

Application style layers can override framework style attributes sparsely, including explicit `false` values. Missing application fields preserve framework fields unless `.plain()` explicitly clears attributes.

This is a meaningful contract for generic content:

```text
framework generic defaults
  → application generic overrides
  → local View/Span StyleRef override
```

### 8.4 Text selectors are not a parallel style system

The TypeScript `TextSelector` and Rust `TextSelector` look like a separate semantic API, but source comments and implementation show they compile into the same style selector/fact machinery.

That avoids a conceptual duplication:

```text
TextSelector facts
  → StyleFacts
  → ordinary StyleSelector matching
  → ThemeResolver
```

### 8.5 History freeze has two meanings at two layers

There are two distinct transitions:

1. **Semantic freeze**
   - `Live(View)` → `Static(View)`.
   - Components are removed from the unit's live lifecycle.
   - Theme references and semantic styles remain present while the unit is resident.
   - Layout is invalidated and recomputed as necessary.

2. **Physical/native freeze**
   - A rendered prefix is accepted into native scrollback.
   - Remaining rows are stored as `FrozenPhysicalRows`.
   - Those rows contain already-resolved physical styles and are no longer semantically re-resolved.

The source does not conflate these; reports must not describe `History.freeze()` alone as immediately freezing cell colors.

### 8.6 Consequential theme/frozen-unit behavior

The native frontier stores `FrozenPhysicalRows` without a theme revision or semantic View reference (`history/native/frontier.rs:5-69`). Transfer of a frozen static remainder (`history/native/mod.rs:479-505`) directly inserts stored rows. It does not call `static_rows()` or `compile_view_with_theme()` again.

Therefore:

- resident static History content can repaint after a theme change;
- rows already inserted into native scrollback retain their old `PhysicalStyle`;
- partially transferred remainder rows also retain the theme from the paint pass that produced them;
- a theme change does not retroactively recolor terminal scrollback.

This appears intentional and follows the generic terminal durability model, but it is the most consequential style/history seam in this assignment. It should be documented as “physical scrollback is a presentation snapshot,” not as a theme cache bug unless product requirements demand retroactive recoloring.

### 8.7 Controls have a split presentation model

`TextInput` demonstrates two style paths:

- caller-supplied initial border is a semantic `BorderSpec` and participates in layout/paint;
- native control-owned focus/cursor styling is produced by the TextInput component itself.

The TS facade can configure the first at construction but cannot dynamically replace it. Focus changes are native/component state and can repaint reversed cursor cells without TS reauthoring the View.

This is consistent with the framework boundary: native controls own interaction and cursor presentation.

### 8.8 Documentation/source parity

The architecture documents describe theme/style values as generic semantic presentation and explicitly prohibit product theme keys. Current source follows that boundary:

- TS style state accepts arbitrary caller keys;
- text selector roles/parts are generic;
- framework defaults are generic Markdown/diff/text semantics;
- no application-specific terms were found in the inspected theme/style implementation.

---

## 9. Open questions and coverage gaps

1. **Frozen scrollback recoloring requirement**
   - Is retaining the original `PhysicalStyle` after native History transfer the intended terminal contract?
   - If themes are expected to recolor all visible history, the current native frontier representation is insufficient because it stores rows rather than semantic content/theme keys.

2. **History theme-change test coverage**
   - There is no inspected TS regression explicitly distinguishing:
     - resident semantic static History after `setTheme`;
     - already-transferred native rows after `setTheme`.
   - A focused test would clarify and protect the intended behavior.

3. **TextInput dynamic border API**
   - Native `HostTextInput` has an internal `set_border()` method, but TypeScript `TextInput` does not expose one.
   - Is construction-time-only border configuration intentional, or is the TS facade incomplete relative to the native control?

4. **Theme change and native History frontier coordination**
   - `SceneHost.invalidate_theme()` clears paint state and schedules full paint, but does not reset the native History frontier.
   - This is consistent with physical snapshot semantics, but the source does not present a single high-level comment explicitly stating that theme invalidation must not replay/rewrite native scrollback.

5. **StyleRef cache scope and theme identity**
   - The TS cache is reset on `Tui.setTheme`, while Rust style refs carry semantic theme atoms and are resolved by the current theme.
   - The source comments explain this, but there is no direct test here proving that all stale style refs are harmless under same-object or value-identical theme replacement.

6. **Runtime execution**
   - No tests/builds were run during this investigation.
   - Native artifact availability and exact current runtime behavior remain unobserved.

7. **Content prepared-product invalidation**
   - Source shows semantic projection is theme-independent and prepared paint is theme-revision keyed.
   - The exact behavior when a theme change overlaps a source append, smoothing delivery tick, and History native transfer is covered partially by regression tests but not exhaustively in this report.

8. **Theme variant precedence at the TS facade**
   - TS `Theme.withStyleVariant` replaces an exact selector in its immutable variant list.
   - Native DTO/Rust assembly preserves declaration order and specificity. This appears compatible, but no direct cross-language property test was inspected for every combination of duplicate selectors and specificity ties.

9. **Unknown fields in TS Theme values**
   - Native DTO decoding is strict enough to reject malformed leaves, but this investigation did not exhaustively classify whether unknown object keys are rejected or ignored for every nested theme/style shape.

10. **Physical style cache granularity**
    - Paint caching and style resolution caches were inspected at module level, but no complete cell-level cache-key census was performed outside the assignment’s style/theme path.

---

## 10. Evidence appendix

### 10.1 Primary TS files inspected

- `packages/iyon-tui/src/api/presentation/style.ts`
  - `StyleSpec`
  - `StyleRef`
  - `StyleSelector`
  - `validateTextAttribute`
- `packages/iyon-tui/src/api/presentation/theme.ts`
  - `Theme`
  - `ThemeDefinition`
  - `themeDefinitionFor`
  - `themeColor`
- `packages/iyon-tui/src/api/presentation/theme-key.ts`
  - `ThemeKey`
- `packages/iyon-tui/src/api/presentation/semantic-style.ts`
  - `semanticColorFor`
  - `semanticStyleFor`
  - `semanticDecorationFor`
  - `semanticTextSpanFor`
  - style/color/border validation
- `packages/iyon-tui/src/api/content/text.ts`
  - `TextSelector`
  - `TextSpan`
- `packages/iyon-tui/src/api/view/view.ts`
  - `View.text`
  - `View.styledText`
  - `View.foreground`
  - `View.background`
  - `View.border`
  - `View.style`
  - `View.styleState`
  - `View.state`
  - immutable decoration/derivation helpers
- `packages/iyon-tui/src/api/view/retained-state.ts`
  - `ViewState`
  - `setPresentation`
  - `clearPresentation`
  - `setStyleState`
  - `clearStyleState`
- `packages/iyon-tui/src/api/controls/history.ts`
  - `History`
  - `push`
  - `freeze`
  - `discardLive`
- `packages/iyon-tui/src/api/controls/text-input.ts`
  - `TextInputOptions`
  - `TextInput`
- `packages/iyon-tui/src/transport/structural/retained-dag.ts`
  - `STYLE_REF_CACHE`
  - `styleRefFor`
  - `styleAtomRef`
  - `materializeTextNode`
  - `materializeWideTextNode`
  - `resetStyleRefCacheForThemeChange`
- `packages/iyon-tui/src/transport/structural/style-lowering.ts`
  - `colorNodeFor`
  - `styleNodeFor`
  - `materializeTheme`
  - `borderNodeFor`
- `packages/iyon-tui/src/transport/structural/ir.ts`
  - `ColorNode`
  - `StyleNode`
  - `TextSpanNode`
  - `BorderNode`
- `packages/iyon-tui/src/runtime/runtime.ts`
  - `TuiRuntime`
  - `Tui.open`
  - `Tui.setTheme`
  - root publication and History binding
- `packages/iyon-tui/src/index.ts`
  - package-root public exports

### 10.2 Primary Rust/native files inspected

- `crates/iyon-tui/src/presentation/api/style.rs`
  - `StyleSpec`
  - `StyleRef`
  - `StyleSelector`
  - `StyleStates`
  - `StyleFacts`
  - `ColorSpec`
  - `ThemeColor`
  - `BorderSpec`
- `crates/iyon-tui/src/theme/mod.rs`
  - `Theme`
  - `ThemeEntry`
  - `ThemeVariant`
  - `resolve_color`
  - `resolve_style`
  - batched assembly
- `crates/iyon-tui/src/theme/framework.rs`
  - `framework_theme`
- `crates/iyon-tui/src/presentation/paint/theme.rs`
  - `StyleContext`
  - `ThemeResolver`
  - `resolve_text_style`
  - `resolve_color`
- `crates/iyon-tui/src/presentation/paint/view.rs`
  - style context traversal and View paint
- `crates/iyon-tui/src/presentation/paint/text.rs`
  - span style resolution and cell painting
- `crates/iyon-tui/src/content/text/style.rs`
  - `TextSelector`
  - text facts
  - text theme style mapping
- `crates/iyon-tui/src/presentation/content.rs`
  - `ContentProvider`
  - `set_theme`
  - content revision interface
- `crates/iyon-tui/src/application/content.rs`
  - `ContentHostRegistry`
  - semantic projection cache
  - prepared paint cache
  - `theme_revision`
  - `set_theme`
- `crates/iyon-tui/src/retained_state/presentation.rs`
  - presentation patch
  - effective style state overlay
- `crates/iyon-tui/src/retained_state/record.rs`
  - dynamic state mutation and revisions
- `crates/iyon-tui/src/retained_state/occurrence.rs`
  - effective occurrence state
- `crates/iyon-tui/src/history/model.rs`
  - History unit lifecycle
  - `push`
  - `freeze`
  - `discard_live`
  - layout invalidation
- `crates/iyon-tui/src/history/projection/mod.rs`
  - static/live/frozen planned content
  - semantic History projection
- `crates/iyon-tui/src/history/native/mod.rs`
  - themed static row compilation
  - native transfer
  - frozen static/content transfer
- `crates/iyon-tui/src/history/native/frontier.rs`
  - `FrozenPhysicalRows`
  - `FrozenStaticRemainder`
  - `FrozenContentRemainder`
  - `NativeFrontier`
- `crates/iyon-tui/src/scene/host.rs`
  - `invalidate_theme`
  - paint scheduling
  - content/History integration
- `crates/iyon-tui/src/application/kernel.rs`
  - `host_set_theme`
- `crates/iyon-tui/src/application/host.rs`
  - host-level `set_theme`
  - native control creation
- `crates/iyon-tui/src/controls/text_input/mod.rs`
  - TextInput state, focus, border, component capability
- `crates/iyon-tui/src/controls/text_input/presentation.rs`
  - TextInput semantic View and cursor/border rendering
- `crates/iyon-tui-native/src/tui.rs`
  - `NativeHistory`
  - `NativeTuiHost.set_theme`
  - `push_ref`
  - `freeze_ref`
- `crates/iyon-tui-native/src/tui/theme_dto.rs`
  - theme/style selector DTOs
  - decoding and validation
  - border DTO

### 10.3 Tests inspected as behavioral evidence

- `packages/iyon-tui/tests/tui_perf13_b.test.ts`
- `packages/iyon-tui/tests/tui_retained_scene_regressions.test.ts`
- `packages/iyon-tui/tests/tui_h3_a_semantic.test.ts`
- `packages/iyon-tui/tests/tui_history_prefix.test.ts`
- `packages/iyon-tui/tests/tui_native_input_validation.test.ts`
- `crates/iyon-tui-native/src/tui/theme_dto.rs` inline tests
- `crates/iyon-tui/src/presentation/paint/theme.rs` inline tests
- `crates/iyon-tui/src/content/text/style.rs` inline tests
- `crates/iyon-tui/src/controls/text_input/tests/presentation.rs` inline tests
- `crates/iyon-tui/src/scene/host.rs` inline theme/focus/content tests

### 10.4 Files indexed but not comprehensively read

The following broader files were searched for cross-references but were not treated as fully read ownership for this assignment:

- other TS composition/runtime modules outside style/state/theme call chains;
- generated ABI bodies under `packages/iyon-tui/src/transport/abi/**`;
- generated native schema files;
- unrelated Rust layout, backend, interaction, projection, and stream modules;
- the complete TypeScript test suite outside files listed above;
- large content/application files beyond the theme/cache sections cited above.

### 10.5 Static commands/search scope

Read-only repository inspection used:

- file listing under `docs/architecture/atlas-4355c02`;
- content searches in `REPORT-CONTRACT.md`, `README.md`, `PRE-V5-ARCHITECTURE-REPORT.md`, and `AGENTS.md`;
- source file enumeration under `packages/iyon-tui/src`, `packages/iyon-tui/tests`, `crates/iyon-tui/src`, and `crates/iyon-tui-native/src`;
- symbol/reference searches for:
  - `Theme`
  - `StyleSpec`
  - `StyleRef`
  - `StyleSelector`
  - `TextSelector`
  - `styleState`
  - `setStyleState`
  - `setTheme`
  - `invalidate_theme`
  - `FrozenPhysicalRows`
  - `freeze`
  - `styleRefFor`
  - `STYLE_REF_CACHE`
  - `theme_revision`
  - `PhysicalStyle`
  - `TextInput`
  - `History`

### 10.6 Bottom-line current-state statement

At the inspected baseline, style/theme behavior is split cleanly across semantic authoring, retained transport, native theme tables, and paint-time physical resolution:

```text
TS values remain semantic and immutable.
Dynamic retained state changes selector/presentation inputs.
Theme replacement invalidates paint, not necessarily layout or structure.
Resident ordinary content and semantic History units can repaint.
Native-transferred/frozen physical rows retain their already-resolved styles.
```

The most consequential boundary is therefore not the initial TS style API; it is the transition from semantic resident content to physical native History rows. Before that transition, theme and selector changes remain reactive. After that transition, cell styles are physical snapshots owned by the native scrollback frontier.