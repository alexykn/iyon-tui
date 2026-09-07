# 11 — Theme

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `iyon-tui`
- Branch: `main`
- Source revision: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Primary scope: `crates/iyon-tui/src/theme/`
- Framework boundary: `iyon-tui` is a generic terminal/UI framework. Theme and style mechanisms may express caller-supplied semantic presentation policy, but must not encode Iyon-agent/application concepts.
- Parent-added atlas documentation was treated as outside the source baseline.

I read:

- `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
- `docs/architecture/atlas-4355c02/README.md`
- `PRE-V5-ARCHITECTURE-REPORT.md`
- `AGENTS.md`

The report follows the required broad heading structure. This is a current-source architecture map, not a V5 migration/disposition decision.

### Scope boundaries

The assigned subsystem is the Rust theme implementation:

- `crates/iyon-tui/src/theme/mod.rs`
- `crates/iyon-tui/src/theme/atoms.rs`
- `crates/iyon-tui/src/theme/batch.rs`
- `crates/iyon-tui/src/theme/framework.rs`

Because the theme subsystem is intentionally split across semantic API, text IR, resolution, physical paint, content caching, and native ingress, I also inspected the principal dependent seams:

- Semantic style API:
  - `crates/iyon-tui/src/presentation/api/style.rs`
  - `crates/iyon-tui/src/content/text/style.rs`
- Semantic-to-physical resolution:
  - `crates/iyon-tui/src/presentation/paint/theme.rs`
  - `crates/iyon-tui/src/physical/style.rs`
- Paint/layout consumers:
  - `crates/iyon-tui/src/presentation/layout/mod.rs`
  - `crates/iyon-tui/src/presentation/paint/view.rs`
  - `crates/iyon-tui/src/presentation/paint/text.rs`
- Host and content theme revisions:
  - `crates/iyon-tui/src/application/kernel.rs`
  - `crates/iyon-tui/src/application/host.rs`
  - `crates/iyon-tui/src/application/content.rs`
  - `crates/iyon-tui/src/scene/host.rs`
- Native theme and style ingress:
  - `crates/iyon-tui-native/src/tui/theme_dto.rs`
  - `crates/iyon-tui-native/src/tui.rs`
  - `crates/iyon-tui-native/src/tui/view_abi.rs`
- Public binding exports:
  - `crates/iyon-tui/src/binding/mod.rs`
  - `crates/iyon-tui/src/lib.rs`

### Evidence status

This report is based on static source inspection. No build, test suite, benchmark, formatter, or runtime validation was executed. Test assertions are reported as source evidence only; they are not claims that the tests passed in this investigation.

### Fact/inference/unknown conventions

- **Observed fact** means directly represented by source.
- **Static inference** means behavior reconstructed from call paths and type ownership.
- **Unknown** means the inspected source does not establish the answer.

---

## 1. Responsibility and structure

### 1.1 Subsystem inventory

| Path | Approximate physical LOC | Production/test split | Primary responsibility | Public surface |
|---|---:|---:|---|---|
| `crates/iyon-tui/src/theme/mod.rs` | 267 | 267 production | Theme tables, setters/builders, variant ordering, semantic color/style resolution | `Theme` is exported through `binding`; the `theme` module itself is private |
| `crates/iyon-tui/src/theme/atoms.rs` | 123 | 96 production / 27 test | Bounded process-wide interning of repeated string style atoms | `intern_style_atom` is public and re-exported through `binding` under `native-host` |
| `crates/iyon-tui/src/theme/batch.rs` | 213 | 131 production / 82 test | Efficient native theme construction with one variant sort per entry | Crate-private `ThemeBatch`; used by `Theme::assemble_batched` |
| `crates/iyon-tui/src/theme/framework.rs` | 61 | 61 production | Framework-owned low-priority default semantic theme | Crate-private `framework_theme` |
| **Theme directory total** | **664** | **555 production / 109 test** | — | — |

LOC are approximate physical line counts from the source line extents, including comments and blank lines. Test sections were separated at `#[cfg(test)]`. Generated code is not present in the assigned theme directory.

Important supporting files are substantially larger than the assigned directory:

- `presentation/api/style.rs` contains the semantic style vocabulary, selectors, state bags, `StyleRef`, colors, borders, and attributes.
- `content/text/style.rs` contains typed structured-text selectors and their encoding into ordinary `StyleSelector` state predicates.
- `presentation/paint/theme.rs` contains the runtime cascade and conversion to physical styles.
- `physical/style.rs` contains the private resolved physical cell-style representation.

### 1.2 Responsibility split

The theme subsystem has five materially different responsibilities:

1. **Named semantic theme storage**
   - Named colors: `HashMap<ThemeKey, ThemeEntry<ThemeColor>>`
   - Named styles: `HashMap<ThemeKey, ThemeEntry<StyleSpec>>`
   - Base values plus selector variants.

2. **Selector ordering and matching**
   - `StyleSelector` expresses positive conjunctions of focus and state predicates.
   - Variants are sorted by predicate count and declaration order.
   - Resolution applies matching color variants or sparse style overlays.

3. **Framework/application layering**
   - `framework_theme()` provides generic framework defaults.
   - The application `Theme` supplies caller/application policy.
   - `StyleRef` local overrides are applied last.
   - Framework and application color lookup have intentionally different precedence mechanics from style lookup.

4. **Semantic text style policy**
   - `TextSelector` is a typed convenience facade over ordinary `StyleSelector`.
   - Text roles, generated parts, origins, annotations, language, format, and other dimensions are encoded into state predicates.
   - Text styles use a reserved named style key, `__iyon_tui.text`.

5. **Physical style realization support**
   - `ThemeResolver` maps semantic `ColorSpec`/`ThemeColor` values into private `PhysicalColor`.
   - `PhysicalStyle` holds inherited/resolved foreground/background and terminal text attributes.
   - Theme revision changes invalidate paint products without changing terminal geometry.

The `theme/` directory itself does not own layout, geometry, text measurement, terminal escape encoding, or physical surface storage. It supplies semantic data and resolution policy consumed by presentation and paint.

### 1.3 Framework ownership boundary

The source is consistent with the generic framework boundary in `AGENTS.md`:

- Framework defaults describe generic structured-text roles (`heading`, `strong`, `emphasis`, `link`, table header) and generic diff presentation.
- The theme code does not mention agent, assistant, provider, conversation, tool, effort, product status, or other Iyon-specific vocabulary.
- Application-specific theme values enter from callers/native payloads and are not interpreted by the Rust theme engine beyond generic selectors and style fields.

The framework’s built-in diff keys such as `diff.addition` and `diff.deletion` are generic diff vocabulary, not product policy.

---

## 2. Types, APIs and contracts

### 2.1 `ThemeEntry<T>` and `ThemeVariant<T>`

`theme/mod.rs:21-32` defines:

```rust
struct ThemeVariant<T> {
    selector: StyleSelector,
    value: T,
    declaration_order: u64,
}

struct ThemeEntry<T> {
    base: Option<T>,
    variants: Vec<ThemeVariant<T>>,
}
```

`ThemeEntry<T>` is used independently for colors and styles.

#### Base values

- A base color or style is optional.
- `Theme::set_color` and `Theme::set_style` replace the base and return the previous value.
- Base setters do not increment the variant declaration counter.
- `Theme::resolve_color` begins with `entry.base`.
- `Theme::resolve_style` begins with `entry.base.clone().unwrap_or_default()`.

The style behavior means a style entry with no base but with matching variants resolves from `StyleSpec::default()` and applies only the matching sparse patches.

#### Variants

`ThemeEntry::set_variant`:

- Finds an existing variant with exact `StyleSelector` equality.
- Replaces its value in place and updates its declaration order.
- Returns the previous value if a duplicate selector existed.
- Sorts the complete variant list after each sequential insertion.

`ThemeEntry::push_variant` is the batch counterpart:

- It replaces duplicate selectors in place.
- It does not sort immediately.
- `ThemeBatch::finish` sorts every entry once.

The duplicate-selector behavior is explicitly tested in `theme/batch.rs:149-190` and `presentation/paint/theme.rs:284-315`.

### 2.2 Declaration ordering and specificity

`sort_variants` in `theme/mod.rs:97-103` orders by:

1. `selector.predicate_count()`
2. `declaration_order`

Thus less-specific variants are applied first and more-specific variants later. For variants with equal predicate count, later declarations are applied later.

The order is not a general CSS engine:

- It only recognizes positive conjunctions.
- There are no negative predicates.
- There is no selector hierarchy beyond predicate count.
- There is no explicit priority field.
- Matching is evaluated independently for each named key.

For styles, later overlays modify only explicitly specified fields. For colors, the last matching variant replaces the color value entirely.

### 2.3 `Theme` API

`Theme` is declared in `theme/mod.rs:90-95`:

```rust
pub struct Theme {
    colors: HashMap<ThemeKey, ThemeEntry<ThemeColor>>,
    styles: HashMap<ThemeKey, ThemeEntry<StyleSpec>>,
    next_declaration_order: u64,
}
```

It is `Clone`, `Debug`, `Default`, `PartialEq`, and `Eq`.

#### Construction APIs

- `Theme::new()`
- `with_color`
- `with_color_variant`
- `with_style`
- `with_style_variant`
- `assemble_batched`

The `with_*` forms consume and return the theme, allowing builder-style construction.

#### Mutation APIs

- `set_color`
- `set_color_variant`
- `set_style`
- `set_style_variant`
- `set_text_style` from `content/text/style.rs`

Setters return replaced values where applicable. They do not return errors.

#### Lookup APIs

- `Theme::color(&self, key: &str) -> Option<ThemeColor>`
- `Theme::style(&self, key: &str) -> Option<&StyleSpec>`

The public-looking lookup helpers are limited:

- `color()` resolves only the base color using `focused = false`, `focus_within = false`, and empty state/fact bags.
- `style()` returns only the base style.
- State/focus variants are only available through crate-private `resolve_color` and `resolve_style`.

This distinction matters for callers inspecting a theme: `Theme::color()` is not a context-sensitive “what will paint here?” query.

#### Internal context-sensitive APIs

- `Theme::resolve_color`
- `Theme::resolve_style`

Both receive:

- `key`
- `focused`
- `focus_within`
- inherited `StyleStates`
- local `StyleFacts`

Color resolution:

```text
base color, if any
    ↓
each matching variant in sorted order
    ↓
last matching color wins
```

Style resolution:

```text
base StyleSpec or empty StyleSpec
    ↓
each matching variant in sorted order
    ↓
sparse overlays accumulate
```

Missing color returns `None`. Missing style returns `Some(default StyleSpec)` if the key exists as an entry, because `resolve_style` starts from `unwrap_or_default()`; a completely missing key returns `None`.

### 2.4 `StyleSelector`

`StyleSelector` is defined in `presentation/api/style.rs:440-533`:

```rust
pub struct StyleSelector {
    focused: bool,
    focus_within: bool,
    states: Vec<(StyleStateKey, StyleStateValue)>,
}
```

It is public through `binding` and is the common selector mechanism for ordinary views and structured text.

Supported predicates:

- `focused()`
- `focus_within()`
- `state(key, value)`
- `and_focused()`
- `and_focus_within()`
- `and_state(key, value)`

State entries are sorted by key when added. Reassigning an existing key replaces its value, so a selector cannot contain two values for the same state key.

`predicate_count()` counts:

```text
focused predicate
+ focus-within predicate
+ number of state assignments
```

`matches()` requires all positive predicates to be satisfied:

```text
(!focused || actual_focused)
&& (!focus_within || actual_focus_within)
&& every state assignment matches
```

State lookup gives local facts precedence over inherited states:

```rust
facts.get(key).or_else(|| states.get(key))
```

This is a deliberate semantic distinction:

- `StyleStates` propagate through the presentation tree.
- `StyleFacts` identify only the current node/span/run.
- A local fact shadows an inherited value for the current match.
- Local facts are cleared before descending to children.

### 2.5 `StyleStateKey`, `StyleStateValue`, `StyleStates`, and `StyleFacts`

`StyleStateKey` and `StyleStateValue` are public semantic wrappers over `StyleAtom`:

```rust
enum StyleAtom {
    Static(&'static str),
    Owned(String),
}
```

They compare and hash by string contents, not by representation. Static and owned forms containing the same string compare equal.

`StyleAssignments` stores normalized sorted key/value pairs and supports:

- binary-search lookup,
- ordered replacement,
- sparse overlay,
- deterministic equality/cloning.

`StyleStates` and `StyleFacts` both use `StyleAssignments`, but differ in ownership semantics:

- `StyleStates` are inheritable and crate-private in the current implementation.
- `StyleFacts` are self-only and crate-private.
- `StyleContext::enter_node` overlays states and installs facts.
- `StyleContext::for_descendant` preserves states but clears facts.

### 2.6 `StyleSpec` and sparse overlay

`StyleSpec` is defined in `presentation/api/style.rs:19-138`:

```rust
pub struct StyleSpec {
    foreground: Option<ColorSpec>,
    background: Option<ColorSpec>,
    attributes: TextAttributeSpec,
}
```

It is sparse and backend-neutral:

- `None` means unspecified/inherited.
- `Some(true)` or `Some(false)` explicitly enables or disables an attribute.
- `overlay()` copies only explicitly specified colors/attributes.
- `plain()` explicitly disables every supported text attribute while leaving colors unspecified.

Supported attributes:

- bold
- dim
- italic
- underline
- reversed
- strikethrough

The sparse contract is essential to theme variants. A focused variant containing only `.bold()` preserves a base foreground and all unrelated attributes.

### 2.7 `ThemeColor`, `ColorSpec`, and `ThemeKey`

`ThemeColor` (`presentation/api/style.rs:535-542`) is a backend-neutral color value:

- `Default`
- `Named(AnsiColor)`
- `Indexed(u8)`
- `Rgb { r, g, b }`

`ColorSpec` (`style.rs:544-571`) is a semantic color reference/value:

- `Theme(ThemeKey)`
- `Named(AnsiColor)`
- `Ansi(u8)`
- `Rgb { r, g, b }`

A `ColorSpec::Theme` is resolved through the current framework/application theme layers. `ThemeColor` itself cannot reference another theme color, so theme-color cycles are structurally excluded.

`ThemeKey` (`style.rs:574-612`) stores `Arc<str>`, compares/hash-compares by string content, and supports borrowing as `str`.

There are separate color and style hash maps. The same key string may therefore exist independently as both a named color and named style.

### 2.8 `StyleRef`

`StyleRef` (`style.rs:614-700`) combines:

- optional named style key,
- sparse local `StyleSpec`.

Constructors:

- `StyleRef::direct(style)`
- `StyleRef::theme(key)`
- `StyleRef::themed(key, overrides)`
- `.overrides(patch)`

Resolution order is:

```text
inherited physical style
    ↓
framework named style
    ↓
application named style
    ↓
StyleRef local overrides
```

A missing named style is a no-op. A direct/local style can be used without a theme key.

### 2.9 Typed structured-text selector API

`content/text/style.rs:27-305` defines the typed semantic text vocabulary:

- `TextRole`
- `TextPart`
- `TextListKind`
- `TextTaskState`
- `TextTableSection`
- `TextSelector`

Roles include paragraph, heading, block quote, list, list item, code block, table, table row/cell, strong, emphasis, links, inline code, raw text, and related generic semantic values.

Parts include generated presentation fragments:

- list marker
- task marker
- quote marker
- code label
- table rule
- thematic rule
- image fallback

`TextSelector` is explicitly not a separate selector engine. It wraps an ordinary `StyleSelector` and encodes typed predicates as style-state key/value pairs.

It exposes:

- `.any()`
- role constructors such as `.heading()`, `.strong()`, `.link()`
- `.part()`
- `.level()`
- `.origin()`
- `.list_kind()`
- `.task_state()`
- `.table_section()`
- `.language()`
- `.format()`
- `.and_annotation()`
- ordinary focus and generic state combinators.

`Theme::with_text_style` and `Theme::set_text_style` are extension methods implemented in `content/text/style.rs:307-329`.

All text styles use the reserved key:

```text
__iyon_tui.text
```

`TextSelector::any()` maps to the base style for that key. Non-empty selectors become variants.

### 2.10 Text fact encoding

`TextFact::key_value()` in `content/text/style.rs:336-394` maps typed facts to state predicates.

Examples:

```text
TextRole::Heading
    → key "__iyon_tui.text.role.heading"
    → value "present"

HeadingLevel::H1
    → key "__iyon_tui.text.heading.level"
    → value "h1"

TextPart::TaskMarker
    → key "__iyon_tui.text.part"
    → value "task-marker"
```

Annotations use a length-prefixed key:

```text
__iyon_tui.text.annotation|<namespace length>:<namespace>|<name length>:<name>
```

The length prefix prevents collisions between dotted/slashed namespace/name combinations. This is tested in `content/text/style.rs:667-687`.

Scalar dimensions replace earlier values for the same key. Repeated roles accumulate because each role has a distinct key. Tests at `content/text/style.rs:585-645` establish these normalization rules.

### 2.11 `TextFacts`

`TextFacts` (`content/text/style.rs:480-555`) is a crate-private renderer builder that emits ordinary `StyleFacts`.

It supports adding:

- roles
- parts
- heading level
- origin
- list/task/table dimensions
- language/format
- annotations

The renderer therefore produces generic semantic facts; `ThemeResolver` remains the only component that maps them to physical styles.

---

## 3. Dependency and ownership map

### 3.1 High-level dependency diagram

```text
TypeScript/native caller-supplied theme payload
                    │
                    ▼
        iyon-tui-native theme_dto.rs
        - serde DTO validation
        - color/style parsing
        - typed TextSelector construction
        - atom interning
                    │
                    ▼
       binding::Theme / Theme::assemble_batched
                    │
                    ▼
        iyon-tui::theme::Theme
        ├── color entries
        ├── style entries
        └── text style under "__iyon_tui.text"
                    │
                    ├──────────────► application::RunningApp owns Arc<Theme>
                    │                         │
                    │                         ▼
                    │                    SceneHost invalidation
                    │
                    ├──────────────► ContentHostRegistry owns Arc<Theme>
                    │                         │
                    │                         ▼
                    │              semantic projection cache + paint cache
                    │
                    ▼
         presentation::paint::ThemeResolver
         ├── framework_theme()
         ├── application Theme
         ├── StyleContext
         └── StyleRef/local patch
                    │
                    ▼
          physical::PhysicalStyle
          physical::PhysicalColor
                    │
                    ▼
             Surface / terminal cells
```

### 3.2 Forward dependencies

#### `theme/mod.rs`

Depends on:

- `std::collections::HashMap`
- `content::text::TextSelector`
- `presentation::api::{StyleFacts, StyleSelector, StyleSpec, StyleStates, ThemeColor, ThemeKey}`
- `theme::batch::ThemeBatch`

Used by:

- `presentation::paint::ThemeResolver`
- `presentation::layout` and paint tests
- `application::kernel`
- `application::host`
- `application::content`
- `scene::host`
- `history` transfer paths
- native binding and native theme DTO decoding.

#### `theme/atoms.rs`

Depends on:

- `HashMap`
- `VecDeque`
- `Arc`
- `Mutex`
- `OnceLock`

Used by:

- `binding::intern_style_atom`
- native color-string parsing
- native view ABI style parsing
- native theme key parsing.

#### `theme/batch.rs`

Depends on:

- `HashMap`
- text theme key and `TextSelector`
- presentation `StyleSelector`, `StyleSpec`, `ThemeColor`, `ThemeKey`
- private `Theme`, `ThemeEntry`, and `sort_variants`.

Used only through `Theme::assemble_batched` and its tests.

#### `theme/framework.rs`

Depends on:

- structured-text `HeadingLevel`, `TextSelector`, `TextTableSection`
- semantic color/style API
- `Theme`.

Used by `ThemeResolver::new`.

### 3.3 Reverse ownership and lifetime

| Object | Created by | Retained by | Destruction/release |
|---|---|---|---|
| `Theme` | `Theme::new`, builders, native `decode_theme` | application running state, resolver clones, content products | ordinary Rust ownership/`Arc` release |
| `Arc<Theme>` host theme | `RunningApp::host_set_theme` | `RunningApp`, content provider, prepared products | when host/content/products release |
| `ThemeBatch` | `Theme::assemble_batched` | only during assembly | consumed by `finish` |
| `ThemeResolver` | `ViewCompiler::new` / `with_interaction` | compiler for a layout/paint pass | compiler drop |
| `StyleContext` | compiler/paint traversal | current recursion path | stack/value ownership |
| `StyleRef` | view factory/text renderer/native view materialization | semantic view decorations/spans | view/span destruction |
| `PhysicalStyle` | resolver/paint | physical cells/surfaces | surface/cell destruction |
| canonical atom table | process `OnceLock` | global process state | process shutdown |
| interned `Arc<str>` | atom table and semantic values | all owning `Arc`s | last owning `Arc` release |

### 3.4 Structural ownership versus policy ownership

The theme itself does not own views, components, layout trees, or surfaces. It is policy/data:

- application owns the selected application theme;
- framework owns built-in defaults;
- views own `StyleRef` references and local semantic patches;
- paint owns the resolved physical style;
- content owns theme-dependent prepared products;
- SceneHost owns invalidation and paint-cache lifecycle.

That separation is important because a theme replacement does not require replacing the semantic view tree.

### 3.5 Hidden ownership coupling

Theme resolution is semantically independent of layout geometry, but the current runtime couples theme changes to:

- `SceneHost` paint cache invalidation;
- `ContentHostRegistry` projection/paint revision;
- content `PreparedPaintKey`;
- deferred content products retaining an immutable `Arc<Theme>`;
- view paint cache theme equality.

The theme data itself is not aware of these consumers. The coupling is at host/content/paint orchestration seams.

---

## 4. Execution paths and state transitions

### 4.1 Native theme construction path

Observed native path:

```text
TS/native JSON payload
    ↓
theme_dto::decode_theme
    ↓
serde ThemeDto / ColorEntryDto / StyleEntryDto / TextStyleDto
    ↓
ThemeDto::build
    ├── intern named color/style keys
    ├── decode ThemeColor
    ├── decode ColorSpec
    ├── decode StyleSpec
    ├── build StyleSelector
    └── build TextSelector
    ↓
Theme::assemble_batched
    ↓
ThemeBatch::build_from_parts
    ├── collect bases
    ├── collect variants with declaration orders
    ├── collect text styles
    └── sort each entry once
    ↓
Theme
```

Evidence:

- `crates/iyon-tui-native/src/tui/theme_dto.rs:305-357`
- `crates/iyon-tui/src/theme/mod.rs:188-204`
- `crates/iyon-tui/src/theme/batch.rs:79-129`

Native DTO behavior:

- malformed payloads fail with invalid-input errors;
- unknown text attributes fail;
- invalid semantic IDs/languages/formats/origins fail;
- theme colors cannot reference another theme color;
- object and string color forms normalize into the same semantic values;
- BTreeMap key iteration gives deterministic named-entry traversal;
- variant declaration order remains payload-order-dependent within each entry.

### 4.2 Application theme replacement

The application-side replacement path is:

```text
Host::set_theme(theme)
    ↓
RunningApp::host_set_theme(theme)
    ├── self.theme = Arc::new(theme)
    ├── SceneHost::invalidate_theme()
    └── invalidate_frame()
```

Evidence:

- `application/host.rs:1325-1328`
- `application/kernel.rs:409-412`

`RunningApp` owns the active `Arc<Theme>` (`application/kernel.rs:42-48`).

`SceneHost::invalidate_theme()`:

- clears the paint cache;
- invalidates content-related layout entries;
- marks `theme_invalidated`;
- retains geometry for unaffected non-content nodes;
- arranges for visible content dependencies to receive fresh paint products.

Evidence:

- `scene/host.rs:430-463`
- comments at `scene/host.rs:453-459` state that theme revisions are intentionally excluded from content layout-input keys because theme changes do not alter intrinsic terminal metrics.

### 4.3 View paint resolution

The ordinary view path is:

```text
ViewCompiler::new(theme)
    ↓
ThemeResolver::new(theme)
    ├── framework = framework_theme()
    └── application = application.clone()
    ↓
ViewPainter traverses LayoutTree
    ↓
StyleContext::enter_node
    ├── overlay inherited StyleStates
    ├── install current StyleFacts
    └── establish focused/focus-within scope
    ↓
ThemeResolver::resolve_text_style
    ├── framework named style
    ├── application named style
    └── local StyleRef patch
    ↓
PhysicalStyle
    ↓
text/border/background painting into Surface
```

Evidence:

- `presentation/layout/mod.rs:112-146`
- `presentation/paint/view.rs:384-394`
- `presentation/paint/theme.rs:97-112`

For descendants:

- inherited physical style is passed separately;
- inherited style states remain;
- local facts are cleared;
- descendant local facts may then be installed for spans/runs.

Evidence:

- `presentation/paint/theme.rs:32-47`
- `presentation/paint/view.rs:603-612`
- `presentation/paint/text.rs:152-159`

### 4.4 Text rendering path

Structured text is rendered into semantic views/spans carrying `StyleRef::theme("__iyon_tui.text")`.

```text
Text IR block/inline/run
    ↓
TextRenderer emits TextFacts
    ↓
TextFacts::finish → StyleFacts
    ↓
TextSelector rules are stored in Theme under "__iyon_tui.text"
    ↓
TextResolver resolves framework text defaults
    ↓
application text rules
    ↓
span/local StyleRef overrides
    ↓
PhysicalStyle
```

Framework text defaults are declared in `theme/framework.rs:17-61`:

- heading: bold
- H1: underline
- strong: bold
- emphasis: italic
- underline: underline
- link: underline
- strikethrough: strikethrough
- table header row: bold
- generic diff colors/styles.

Application text rules are layered through `Theme::with_text_style` and `set_text_style` (`content/text/style.rs:307-329`).

### 4.5 Focus and focus-within state

`StyleContext::for_scope` (`presentation/paint/theme.rs:49-65`) computes:

- `focused`: current scope equals focused component;
- `focus_within`: focused component is a descendant or self according to `MountGraph`.

This is evaluated at paint/compiler context creation, not stored in the theme.

A focused selector can therefore change physical output without mutating a component revision. The source test `scene/host.rs:3875-3910` explicitly verifies that focus variants paint without a component revision change.

### 4.6 Theme-dependent content projection path

Content theme flow:

```text
Host frame preparation
    ↓
ContentProvider::set_theme(&Arc<Theme>)
    ↓
ContentHostRegistry compares Arc pointer/value
    ↓
if changed:
    ├── stores Arc clone
    ├── increments theme_revision
    ├── clears connector projection caches
    └── drops candidate projections
    ↓
measure/project content
    ├── semantic projection key excludes theme
    ├── prepared paint key includes theme_revision
    ├── semantic cache may hit
    └── paint/layout product is rebuilt for new theme
    ↓
HostContentProjection stores Arc<Theme>
```

Evidence:

- `application/content.rs:6572-6586`
- `application/content.rs:1082-1103`
- `application/content.rs:1242-1255`

The source explicitly separates:

- semantic IR, which is theme-independent;
- prepared paint products, which depend on theme;
- metrics/layout input, which excludes theme revisions where theme changes cannot alter intrinsic dimensions.

### 4.7 Theme revision state transition

`ContentHostRegistry` starts with:

```text
theme = Arc::new(Theme::new())
theme_revision = 0
```

At `set_theme`:

1. pointer equality or value equality causes an early return;
2. otherwise the new `Arc<Theme>` is stored;
3. `theme_revision` increments with checked arithmetic;
4. connector projection caches and candidate projections are cleared.

The revision is then incorporated into:

- `projection_revision` (`application/content.rs:6590-6602`);
- `TextProjectionKey.theme_revision`;
- `PreparedPaintKey.theme_revision`.

It is deliberately not included in `TextProjectionKey::layout_input_revision` (`application/content.rs:243-258`), preserving geometry/layout reuse for palette-only changes.

### 4.8 Physical realization

`ThemeResolver::resolve_color` (`presentation/paint/theme.rs:178-190`) maps:

- `ColorSpec::Ansi` → `PhysicalColor::Indexed`
- `ColorSpec::Named` → `PhysicalColor::Named`
- `ColorSpec::Rgb` → `PhysicalColor::Rgb`
- `ColorSpec::Theme` → context-sensitive `ThemeColor`, or `PhysicalColor::Default` if absent.

`PhysicalStyle` (`physical/style.rs:31-40`) is terminal-neutral at this internal layer but is explicitly physical/cell-oriented:

- optional foreground/background;
- concrete boolean attributes.

`PhysicalColor` and its `AnsiColor` are private to the physical module. Public semantic `AnsiColor` is converted explicitly by `to_physical_ansi` (`presentation/paint/theme.rs:203-222`).

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation → implementation path

| Semantic operation | Primary production path | Selection/route condition | Failure or fallback behavior |
|---|---|---|---|
| Define a base named color | `Theme::set_color` | direct caller/native decoded base | replaces prior base, returns old value |
| Define a color variant | `Theme::set_color_variant` | selector variant | duplicate selector replaces; variants sorted |
| Define a base named style | `Theme::set_style` | direct caller/native decoded base | replaces prior base, returns old value |
| Define a style variant | `Theme::set_style_variant` | selector variant | duplicate selector replaces; sparse variants overlay |
| Define text style | `Theme::set_text_style` | `TextSelector::any()` becomes base; otherwise variant | same theme table/key as ordinary styles |
| Resolve a named color | `ThemeResolver::resolve_color` → `Theme::resolve_color` | selector matching uses focus, inherited states, local facts | missing semantic color becomes physical default |
| Resolve a named style | `ThemeResolver::resolve_text_style` → `Theme::resolve_style` | framework and application layers each resolve independently | missing style is a no-op |
| Resolve a direct color | `ThemeResolver::resolve_color` | no theme lookup for `Ansi`, `Named`, `Rgb` | direct values always realize |
| Resolve a `StyleRef` | framework → application → local | named key present or direct/local only | missing named key does not suppress local style |
| Resolve text semantics | `TextFacts` + reserved text style key | renderer facts match `TextSelector` variants | unmatched rules simply do not overlay |
| Replace host theme | `RunningApp::host_set_theme` → `SceneHost::invalidate_theme` | explicit host operation | paint/content caches invalidated; structure retained |
| Update content theme | `ContentProvider::set_theme` | pointer/value difference | increments revision, clears connector paint/projection candidates |
| Decode native theme | `decode_theme` → DTO build → `assemble_batched` | N-API/native ingress | malformed payload rejected; no silent shape skipping |
| Intern style atom | `binding::intern_style_atom` → global table | native style/color/key ingress | poisoned mutex falls back to a fresh `Arc` |

### 5.2 Framework versus application style route

For styles, `ThemeResolver::resolve_text_style` does:

```text
resolve framework named style
    ↓ apply onto inherited physical style
resolve application named style
    ↓ apply onto result
apply local StyleRef patch
```

Within each theme layer, variant specificity and declaration order determine the sparse overlay order.

The application layer is therefore later than the framework layer even when the framework variant has more predicates. This is demonstrated by `presentation/paint/theme.rs:419-448`: an application generic variant can disable a framework more-specific variant because the application layer is applied afterward.

### 5.3 Framework versus application color route

Colors use a different function:

```rust
self.application.resolve_color(...).or_else(|| {
    self.framework.resolve_color(...)
})
```

Consequences:

- application color wins whenever it resolves to any `ThemeColor`, including explicit `ThemeColor::Default`;
- framework color is consulted only if application has no matching color/base;
- a framework style containing `ColorSpec::Theme("accent")` still resolves that reference through the application palette first.

This behavior is tested in `presentation/paint/theme.rs:506-543`.

### 5.4 Missing style versus missing color

The failure semantics differ intentionally:

- Missing named style:
  - `ThemeResolver::resolve_text_style` skips the layer.
  - Existing inherited physical style and local overrides remain.
- Missing theme color:
  - `ThemeResolver::resolve_color` maps `None` to `PhysicalColor::Default`.
  - This is a visible compatibility fallback rather than an error.

The test `presentation/paint/theme.rs:318-346` verifies both missing style no-op behavior and local override application.

### 5.5 Native decoder failure semantics

The native DTO layer validates at the boundary:

- invalid JSON shape → invalid input;
- unknown style attribute → invalid input;
- invalid text role/part/origin/language/format → invalid input;
- nested theme color reference → invalid input;
- invalid semantic tags → invalid input;
- malformed color syntax → invalid input.

The decoder comments at `theme_dto.rs:1-12` state that malformed containers fail closed instead of being silently ignored.

### 5.6 Atom-table failure semantics

`intern_style_atom` (`theme/atoms.rs:86-95`) locks the global table:

- normal path returns canonical `Arc<str>`;
- a poisoned mutex returns `Arc::from(value)` rather than propagating the poisoning error.

This preserves ingress functionality but loses interning for that call. It is the only explicitly observed best-effort fallback in the theme subsystem.

### 5.7 No alternate legacy theme engine observed

Within the searched Rust source scope, there is one semantic theme implementation:

- `Theme`
- `ThemeBatch`
- `ThemeResolver`

Structured text does not introduce a second theme engine. `TextSelector` lowers into `StyleSelector` and uses the ordinary theme tables.

The native decoder also terminates directly in `Theme`, rather than maintaining a second persistent theme model.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Theme table lookup cost

`Theme` uses hash maps keyed by `ThemeKey`, but selector matching is linear over the variants for a named key:

```text
HashMap key lookup
    ↓
iterate all variants
    ↓
evaluate selector.matches
    ↓
overlay/replace matching values
```

There is no selector index by:

- focus state,
- focus-within state,
- state key,
- text role,
- theme revision.

For ordinary themes this is likely acceptable because variant sets are expected to be small, but the source does not impose a bound on the number of entries or variants in a `Theme`.

### 6.2 Selector match cost

State assignments are sorted and binary-searchable inside `StyleStates`/`StyleFacts`. Each selector still iterates its own predicate vector and performs one or two state lookups per predicate.

Approximate matching cost:

```text
O(selector predicate count × log(state/fact assignment count))
```

The text selector encoding keeps assignments small and deterministic but does not create a dedicated typed index.

### 6.3 Variant sorting

Sequential setters sort an entry after every insertion:

- useful for interactive mutation;
- potentially `O(n log n)` repeated for a payload with many variants.

Native ingress uses `ThemeBatch`:

- declaration orders are assigned while accumulating;
- duplicate replacement happens without sorting;
- each entry is sorted exactly once in `finish`.

This is the principal theme-specific construction optimization. The equivalence tests in `theme/batch.rs:149-211` compare batch construction with sequential construction, including duplicate selectors and text-style base/variant handling.

### 6.4 Style atom cache

`StyleAtomTable` in `theme/atoms.rs:26-77`:

- uses `HashMap<String, Arc<str>>` for lookup;
- uses `VecDeque<(String, Arc<str>)>` for FIFO eviction;
- has a configurable minimum capacity of one;
- canonical process capacity is `2048`;
- evicts oldest entries when `entries.len() > capacity`;
- checks `Arc::ptr_eq` before removing an entry, protecting against stale queue generations;
- returns independent `Arc` values, so eviction cannot invalidate semantic values already retained elsewhere.

The process-wide table is protected by a `Mutex`. It is used for repeated native ingress strings, not for every direct Rust constructor.

### 6.5 Interning coverage

Observed interning paths:

- native theme named color/style keys (`theme_dto.rs:320-330`);
- native `theme:` color strings (`tui.rs:1884-1890`);
- native structural style atoms and theme references (`view_abi.rs:3898-3912`);
- the binding re-export (`binding/mod.rs:101-104`).

Direct Rust constructors such as `ThemeKey::from(&str)` use `Arc::from(value)` directly (`presentation/api/style.rs:596-610`). Therefore, the canonical table is an ingress optimization, not an invariant that all equal `ThemeKey`s share the same allocation.

### 6.6 Paint cache

`PaintCache` in `presentation/paint/view.rs:162-200` is a two-generation cache of `Arc<Surface>` values:

- `current`
- `previous`
- stored `theme: Option<Theme>`

At `begin_epoch(theme)`:

- if the new theme is not equal to the prior theme, both generations are cleared;
- otherwise current moves to previous and a new current generation begins.

`SceneHost::invalidate_theme` also clears this cache proactively. The cache key includes view identity, geometry, style context, text layout, content revisions, and box fingerprints, but the theme is managed at cache-epoch scope.

The two-generation retention bound is tested in `presentation/paint/view.rs:1571-1592`.

### 6.7 Content projection caches

Content separates semantic and paint products:

- `SemanticProjectionKey` excludes theme;
- `PreparedPaintKey` includes `theme_revision`;
- semantic projection can be reused across recolors;
- prepared paint/layout products are rebuilt for a new theme;
- a prepared product retains `Arc<Theme>` for deferred row painting.

The source comment at `application/content.rs:1082-1084` explicitly states:

> Semantic IR is theme-independent and layout-independent: recolors, window resizes, and smooth timer delivery ticks hit the cache, while source revisions or funnel kind changes rebuild.

The theme-specific test at `application/content.rs:9093-9098` states that recolor must invalidate painted surfaces but must not rerun semantic parsing.

### 6.8 Layout invalidation

Theme changes are excluded from content layout-input revisions because the current theme fields do not affect intrinsic terminal dimensions:

- `TextProjectionKey::layout_input_revision` hashes source, width, wrap, funnel kind, and delivery revision;
- it does not hash `theme_revision`;
- `ContentHostRegistry::layout_input_revision` consequently remains geometry/content-oriented.

Theme changes still invalidate content measurement entries in `SceneHost` when necessary to prevent reuse of old paint products after the content registry drops them. This is a targeted invalidation rather than a full geometry reset.

### 6.9 Theme clone costs

`ThemeResolver::new` clones the application theme and constructs a fresh framework theme. `ViewCompiler::new` calls it. This means a compiler creation can clone the complete application theme maps, although `ThemeKey` strings are reference-counted.

The source does not expose counters for:

- theme clone count;
- selector comparisons;
- variants traversed;
- framework-theme construction count;
- atom-table hit/miss/eviction rates.

Those are observability gaps if theme scale or compiler creation becomes a performance concern.

### 6.10 Scheduling

Theme changes are host-driven rather than timer-driven:

```text
set_theme
    ↓
scene invalidation
    ↓
frame invalidation
    ↓
next render/paint
```

No theme-specific scheduling queue exists in the assigned subsystem. Smoothing/content delivery may trigger their own revisions, but theme revision participates only in content paint identity and host invalidation.

---

## 7. Tests, benchmarks and observability

### 7.1 Theme-local tests

#### `theme/atoms.rs`

- `repeated_interns_share_one_allocation`
  - verifies pointer identity for repeated values;
  - verifies one table entry.
- `unique_value_stream_stays_bounded_and_live_values_survive`
  - verifies FIFO bound;
  - verifies live `Arc` remains usable after eviction;
  - verifies re-interning equal content works.

Evidence: `theme/atoms.rs:97-123`.

#### `theme/batch.rs`

- `batch_matches_sequential_construction_including_duplicates`
  - verifies duplicate selector replacement;
  - verifies declaration ordering;
  - verifies mixed predicate counts.
- `batch_text_styles_match_set_text_style`
  - verifies text base/variant lowering matches sequential setters.

Evidence: `theme/batch.rs:132-213`.

### 7.2 Semantic style tests

`presentation/api/style.rs:1087-1180` covers:

- `StyleSpec::plain` explicit false attributes;
- sparse overlay preserving unspecified fields;
- explicit false overriding true;
- semantic primitive constructors;
- border constructors and color replacement.

`content/text/style.rs` tests cover:

- selector predicate order normalization;
- scalar last-write-wins;
- role accumulation;
- annotation collision resistance;
- conjunction matching;
- typed selector plus generic state/focus composition;
- framework defaults;
- application overrides;
- `TextSelector::any` mapping to the reserved text base style.

### 7.3 Resolver tests

`presentation/paint/theme.rs:224-567` covers:

- selector normalization and specificity;
- duplicate variant replacement;
- missing color fallback;
- missing style no-op;
- local override precedence;
- theme-only changes preserving geometry;
- sparse style variant overlay;
- application layer overriding framework;
- application generic variant beating framework-specific variant;
- application color precedence including explicit default;
- framework style color references consulting application palette;
- `plain()` resetting attributes while preserving framework colors.

These tests establish the intended distinction between:

- within-layer specificity;
- cross-layer priority;
- local override priority;
- semantic versus physical output.

### 7.4 Paint/view tests

`presentation/paint/view.rs` tests include:

- theme switch invalidates cached surfaces;
- focus variant changes physical output;
- paint cache is bounded to two generations;
- style and geometry changes affect retained surfaces as expected.

Evidence: `presentation/paint/view.rs:1465-1592`.

### 7.5 Content revision tests

`application/content.rs` includes tests that exercise:

- theme change invalidating content measurement/projection revision;
- recolor reusing semantic projection products;
- changed theme causing new prepared paint products;
- theme-dependent text styles changing content output.

Evidence includes:

- `application/content.rs:8764-8829`
- `application/content.rs:8978-9098`

### 7.6 Host integration tests

`application/host.rs` and `scene/host.rs` verify:

- setting themes on multiple hosts;
- duplicate variants;
- text heading style recoloring;
- dynamic theme changes during streamed content;
- focus/focus-within color variants;
- theme changes repainting clean siblings when content also refreshes.

Evidence:

- `application/host.rs:2652-2678`
- `application/host.rs:3987-4160`
- `scene/host.rs:3498-3566`
- `scene/host.rs:3875-3970`

### 7.7 Native decoder tests

`crates/iyon-tui-native/src/tui/theme_dto.rs:441-540` contains fixture-based decode/equality tests. They verify that native JSON decoding constructs the same public binding `Theme` as the expected builder form, including:

- base colors;
- focused/state variants;
- styles;
- text styles;
- theme color references.

### 7.8 Observability gaps

No theme-specific production counters were found for:

- theme table sizes;
- variant counts;
- selector match counts;
- variant overlay counts;
- missing style/color lookups;
- atom-table hits, misses, or evictions;
- theme clone cost;
- framework-theme construction;
- theme revision count per host;
- content paint rebuilds attributable specifically to theme revisions.

Existing generic performance counters and tests observe downstream paint/content work, but not the theme resolution internals directly.

No benchmark specific to theme construction or selector resolution was identified in the inspected source.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Semantic theme versus physical terminal realization

The code has a clear conceptual split:

```text
ThemeColor / ColorSpec / StyleSpec / StyleRef
    = semantic/backend-neutral intent
```

versus:

```text
PhysicalColor / PhysicalStyle
    = resolved terminal-cell presentation
```

The split is not absolute in vocabulary: `AnsiColor`, indexed colors, and terminal attributes are terminal-oriented concepts. Nevertheless, theme values are not converted to physical cell data until `ThemeResolver`.

The physical module’s `PhysicalStyle` is private, while semantic style types are exported through the binding. This preserves the intended public/internal boundary.

### 8.2 Framework and application policy are separate but both use `Theme`

The same `Theme` type represents:

- framework defaults;
- application-supplied semantic policy;
- test-specific themes;
- content-prepared theme snapshots.

The distinction is by ownership and layer position, not by type. `ThemeResolver` introduces the framework/application interpretation.

This allows a framework `StyleSpec` to contain a `ColorSpec::Theme` reference that resolves through the application palette. That is a useful coupling for generic defaults, but it means a framework theme is not independently closed over its colors.

### 8.3 Color and style precedence differ

The source intentionally implements:

- styles: framework then application, sparse overlays;
- colors: application first, framework fallback;
- local style: after both named style layers.

These are not equivalent cascading rules. A future maintainer changing one must not assume the other follows automatically.

### 8.4 Theme revisions live outside `Theme`

`Theme` has no revision field. Revisioning is distributed:

- `RunningApp` replaces its `Arc<Theme>`;
- `SceneHost` has a boolean `theme_invalidated`;
- `ContentHostRegistry` owns `theme_revision`;
- `PreparedPaintKey` stores `theme_revision`;
- `ContentProvider` exposes theme-dependent projection revisions;
- `PaintCache` compares entire `Theme` values at epoch boundaries.

This is currently coherent but fragmented. A `Theme` value alone cannot answer whether it is newer than another theme, and two equal themes supplied at different times are intentionally treated as equivalent by `ContentHostRegistry::set_theme`.

### 8.5 Semantic text IR remains theme-independent

The content implementation distinguishes semantic projection from prepared paint:

- text/Markdown parsing and semantic IR are not recolored in place;
- a theme swap may reuse semantic products;
- prepared physical rows/layout products are theme-keyed.

This is an important architectural seam. `TextSelector` and `TextFacts` remain semantic; they are not converted to physical styles during parsing.

### 8.6 Native ingress and direct Rust construction differ in interning

Native ingress uses `intern_style_atom` for:

- theme keys;
- `theme:` color references;
- structural style atoms.

Direct Rust constructors use `Arc::from` directly. Equal keys therefore need not share allocation across all construction routes, even though equality/hash behavior remains correct.

The atom table is an optimization, not part of semantic identity.

### 8.7 Current public exposure is mixed

`Theme` is not re-exported from the crate root as a public root API:

- `lib.rs:84` has `pub(crate) use theme::Theme`.
- `binding/mod.rs:55` publicly re-exports `Theme`.

Likewise, semantic style types are public through `binding`, while many resolver/context types remain crate-private. The actual external authoring boundary is therefore the binding module/native facade, not the Rust crate root.

### 8.8 Generic structured-text policy is framework-owned

The framework theme supplies defaults for headings, marks, links, table headers, and diff rendering. This is generic semantic presentation and is allowed under the framework boundary.

The source does not supply product-specific defaults. Application/plugin callers must provide product policy through the generic theme payload/API.

### 8.9 Theme changes do not rebuild structure

The host invalidation path retains the semantic view/layout structure and refreshes presentation:

- `SceneHost::invalidate_theme` clears paint and affected content products;
- layout geometry is retained where theme cannot affect metrics;
- content semantic products may be reused;
- physical paint is recomputed under the new theme.

This is one of the strongest current architectural seams.

---

## 9. Open questions and coverage gaps

1. **Theme scale and selector cardinality**
   - No production bounds exist on number of theme keys or variants.
   - Resolution remains linear over variants per key.
   - The intended maximum application theme size is not documented in the inspected source.

2. **Theme cloning frequency**
   - `ThemeResolver::new` clones the application theme.
   - `ViewCompiler::new` constructs a new resolver.
   - The source does not establish how frequently compilers are constructed in all host paths or whether this is material for large themes.

3. **Theme revision identity**
   - There is no intrinsic revision/fingerprint in `Theme`.
   - Host/content revisioning is external and uses replacement plus equality checks.
   - It is unknown whether all future theme consumers will use the same external revision discipline.

4. **Direct Rust API status**
   - `Theme` methods are `pub`, but the module is private and the crate root re-export is crate-private.
   - The intended long-term Rust authoring visibility is not fully evident from this scope alone.
   - Native binding exposure is clear; direct external Rust authoring is less clear.

5. **`Theme::color()` and `Theme::style()` expectations**
   - These helpers omit variants and context.
   - It is unknown whether external callers depend on them as introspection-only helpers or expect context-sensitive results.

6. **Color fallback observability**
   - Missing theme colors silently realize as physical default.
   - The source has tests for this behavior but no warning/diagnostic/counter.
   - It is unknown whether missing tokens should be observable to application/plugin authors.

7. **Theme-key namespace validation**
   - Named color and style keys are opaque strings.
   - No namespace collision or naming validation is performed in `Theme`.
   - A style and color can share a key without conflict because tables are separate, but this may be surprising to callers.

8. **Cross-layer style color references**
   - Framework styles may refer to application colors.
   - This is tested and apparently intentional.
   - The source does not document whether framework styles may safely assume application palette keys exist.

9. **Atom cache pressure**
   - Canonical capacity is 2048, but no hit/miss/eviction metrics exist.
   - Direct constructors bypass the canonical table.
   - It is unknown whether 2048 is based on measured production workloads or a conservative fixed bound.

10. **Prepared product retention**
    - Content products retain an `Arc<Theme>` to preserve deferred painting semantics.
    - The exact lifetime and maximum number of retained prepared products are managed by content caches, not the theme subsystem.
    - Any cache-retention analysis must include `application/content.rs`.

11. **Backend neutrality of `ThemeColor`**
    - The API calls itself backend-neutral, but named ANSI/indexed/RGB values are terminal-oriented.
    - The source does not define a parallel non-terminal realization contract.

12. **External TypeScript theme contract**
    - Native DTO shapes were inspected, but a complete TypeScript authoring/API census is outside this assignment.
    - The native decoder demonstrates accepted payload fields, but the complete TS-side normalization path should be reconciled by the TypeScript/public-binding assignments.

13. **Execution validation**
    - No tests or benchmarks were run in this investigation.
    - All behavioral claims beyond direct code paths are based on source assertions and static reconstruction.

---

## 10. Evidence appendix

### 10.1 Primary assigned files

- `crates/iyon-tui/src/theme/mod.rs`
  - `ThemeVariant`
  - `ThemeEntry`
  - `Theme`
  - `sort_variants`
  - builder/setter APIs
  - `assemble_batched`
  - `resolve_color`
  - `resolve_style`
  - declaration-order management

- `crates/iyon-tui/src/theme/atoms.rs`
  - `CANONICAL_ATOM_CAPACITY`
  - `StyleAtomTable`
  - `canonical_style_atoms`
  - `intern_style_atom`
  - bounded FIFO/Arc lifetime tests

- `crates/iyon-tui/src/theme/batch.rs`
  - `ThemeBatch`
  - batched color/style/text accumulation
  - `finish`
  - sequential/batched equivalence tests

- `crates/iyon-tui/src/theme/framework.rs`
  - `framework_theme`
  - generic structured-text defaults
  - generic diff colors/styles

### 10.2 Semantic style API files

- `crates/iyon-tui/src/presentation/api/style.rs`
  - `StyleSpec`
  - `StyleSelector`
  - `StyleStates`
  - `StyleFacts`
  - `StyleStateKey`
  - `StyleStateValue`
  - `ThemeColor`
  - `ColorSpec`
  - `ThemeKey`
  - `StyleRef`
  - `TextAttributeSpec`
  - border/style semantic types

- `crates/iyon-tui/src/content/text/style.rs`
  - `TextRole`
  - `TextPart`
  - `TextListKind`
  - `TextTaskState`
  - `TextTableSection`
  - `TextSelector`
  - `TextFact`
  - `TextFacts`
  - `Theme::with_text_style`
  - `Theme::set_text_style`
  - reserved text key and fact encoding
  - structured-text selector tests

### 10.3 Semantic-to-physical resolution files

- `crates/iyon-tui/src/presentation/paint/theme.rs`
  - `StyleContext`
  - `ThemeResolver`
  - `resolve_text_style`
  - `apply_style`
  - `resolve_style`
  - `resolve_theme_color`
  - `resolve_color`
  - semantic-to-physical conversion
  - framework/application/local cascade tests

- `crates/iyon-tui/src/physical/style.rs`
  - private physical `AnsiColor`
  - private `PhysicalColor`
  - private `PhysicalStyle`

- `crates/iyon-tui/src/presentation/layout/mod.rs`
  - `ViewCompiler`
  - `ThemeResolver` construction
  - style context creation

- `crates/iyon-tui/src/presentation/paint/view.rs`
  - paint traversal
  - `StyleContext` entry/descendant behavior
  - `PaintCache`
  - theme epoch invalidation
  - physical style application

- `crates/iyon-tui/src/presentation/paint/text.rs`
  - span-level style resolution
  - local span facts and inherited physical style

### 10.4 Revision/cache/lifetime files

- `crates/iyon-tui/src/application/kernel.rs`
  - `RunningApp.theme`
  - `host_set_theme`

- `crates/iyon-tui/src/application/host.rs`
  - public host theme replacement
  - host/theme integration tests

- `crates/iyon-tui/src/scene/host.rs`
  - `theme_invalidated`
  - `invalidate_theme`
  - layout/content invalidation
  - theme/focus integration tests

- `crates/iyon-tui/src/application/content.rs`
  - `TextProjectionKey.theme_revision`
  - `PreparedPaintKey.theme_revision`
  - prepared theme product retention
  - `ContentHostRegistry.theme`
  - `ContentHostRegistry.theme_revision`
  - `ContentProvider::set_theme`
  - `projection_revision`
  - `layout_input_revision`
  - semantic/paint cache tests

### 10.5 Native ingress files

- `crates/iyon-tui-native/src/tui/theme_dto.rs`
  - `ThemeColorDto`
  - `StyleColorDto`
  - `StyleDto`
  - `SelectorDto`
  - `TextSelectorDto`
  - `ThemeDto::build`
  - `decode_theme`
  - native decode/equivalence tests

- `crates/iyon-tui-native/src/tui.rs`
  - `color_spec_str`
  - canonical `theme:` atom parsing
  - ANSI/RGB/named color parsing

- `crates/iyon-tui-native/src/tui/view_abi.rs`
  - structural style atom parsing
  - `StyleRef` reconstruction
  - canonical theme/color atom use

- `crates/iyon-tui/src/binding/mod.rs`
  - public semantic style/theme re-exports
  - `Theme`
  - `intern_style_atom`

- `crates/iyon-tui/src/lib.rs`
  - crate-private root `Theme` re-export
  - crate-private internal semantic style re-exports

### 10.6 Files inspected in full or substantially

Read comprehensively for this assignment:

- all four files under `crates/iyon-tui/src/theme/`;
- `crates/iyon-tui/src/presentation/api/style.rs`;
- `crates/iyon-tui/src/content/text/style.rs`;
- `crates/iyon-tui/src/presentation/paint/theme.rs`;
- `crates/iyon-tui/src/physical/style.rs`;
- `AGENTS.md`;
- required architecture/report documents.

Read selectively around theme and revision paths:

- `application/kernel.rs`
- `application/host.rs`
- `application/content.rs`
- `scene/host.rs`
- `presentation/layout/mod.rs`
- `presentation/paint/view.rs`
- `presentation/paint/text.rs`
- native DTO and native atom parsing files
- binding and crate-root exports.

### 10.7 Files indexed or searched but not fully read

The following were searched for references and route evidence but were not read as entire files:

- broader `application/content.rs` sections unrelated to theme;
- broader `application/host.rs` sections unrelated to theme;
- broader `scene/host.rs` sections unrelated to theme;
- all non-theme content renderer files;
- all presentation/layout and paint tests unrelated to style/theme;
- TypeScript packages and generated native code outside the native theme ingress path;
- benchmark suites and CI configuration.

### 10.8 Static evidence methodology

- Exact line numbers in this report refer to the source paths and line extents returned during inspection.
- Production/test LOC for the assigned directory were estimated from physical line extents and `#[cfg(test)]` module boundaries.
- No source edits were made.
- No dependencies were installed.
- No services, agents, or external processes were launched.
- No tests or benchmarks were executed.