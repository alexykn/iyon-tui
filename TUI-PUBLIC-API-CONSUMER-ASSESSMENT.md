# External Consumer Public API Assessment

**Status:** feedback record; no API change is implied
**Source:** external TypeScript consumer review
**Scope:** the public `@iyon/tui` facade and the application-owned `iyon:tui`
alias

This record captures consumer feedback about the framework surface as it is
currently used. It is deliberately limited to generic framework concerns; it
does not add application policy or product concepts to `iyon-tui`.

## Observed public surface

### Core view and presentation primitives

- `View` as the composable view value and fluent builder
- `View.text()`, `View.vertical()`, `View.horizontal()`, `View.hanging()`,
  `View.spacer()`, and `View.component()`
- view modifiers such as `fillWidth()`, `fillHeight()`, `noWrap()`, `style()`,
  `padding()`, `border()`, and `clampRows()`
- `Scene`, `Style`, `Theme`, `TextSelector`, and `Insets`

### Generic controls and streams

- `History`, including ordered units and streaming push/seal operations
- `TextInput`
- `TextStream`
- `ScrollPane`
- `ViewSlot`, including view replacement and animation control

### Reactive and runtime facilities

- `defineView` and `state`
- `Tui`, `TuiRuntime`, and `TuiEvent`
- rendering, lifecycle, event routing, key binding, paste interception, theme
  installation, and generic control factories
- `createAppHarness` for deterministic consumer tests

## Strengths reported by the consumer

- The fluent `View` builder and callback-based vertical/horizontal composition
  are readable and ergonomic.
- Scoped state invalidation is a substantial capability rather than a nominal
  abstraction. Execution counters make the invalidation behavior measurable.
- The headless application harness provides a useful way to exercise the
  runtime without a real terminal.
- `TextStream` together with history streaming and sealing provides a clear
  progressive-rendering approach.
- The core concepts are coherent once the retained view, scope, slot, and
  native-boundary relationships are understood.

## Rough edges and design concerns

### 1. Two import identities are visible to consumers

Consumers currently see both `@iyon/tui` and the application-owned `iyon:tui`
alias. A mixed codebase can make it unclear which path is canonical and can
make type identity or bundler behavior look inconsistent.

**Question for follow-up:** should public examples and framework-facing
consumer code standardize on `@iyon/tui`, with `iyon:tui` documented only as an
optional host alias, or should the alias be the primary documented path?

### 2. Repeated `as unknown as View` casts indicate a typing seam

Consumer render helpers frequently end with casts from a structurally
compatible result to `View`. This is a usability and type-safety signal. It
may mean that helper return types are too broad, that `View` is nominal where
it should be structural, or that a public `IntoView`-style contract is not
being propagated through the helper APIs.

**Suggested investigation:** identify the smallest common return contract,
then make helper functions return it directly. Do not make `View` permissive
without preserving invalid-view rejection at the public boundary.

### 3. `View.component()` and `ViewSlot` have a steep composition cost

Dynamic content can require coordinating a component reference, a retained
slot, animation state, and a scroll pane. The primitives are individually
useful, but the combined construction pattern is difficult to discover and
verbose for common dynamic-view cases.

**Suggested investigation:** document the lifecycle relationship first. Only
then consider a generic convenience composition that does not hide ownership,
focus, animation, or disposal semantics.

### 4. Theme color references are inconsistent and stringly typed

The consumer sees both style operations such as
`foreground("theme:...")` and `theme("...")`. String references are flexible
but fragile and provide no key completion or compile-time validation.

**Suggested investigation:** define one canonical theme-reference operation
and consider a typed `ThemeKey`/`ColorSpec` path while retaining an explicit
escape hatch for caller-defined namespaces.

### 5. `TuiRuntime` combines several responsibilities

The current runtime interface covers input routing, paste handling, themes,
control factories, rendering, lifecycle, event polling, and advancement. The
surface is coherent at the runtime boundary, but it is broad and makes
mocking or capability-specific use harder.

**Suggested investigation:** shape narrower capability views or injected
facets without breaking the existing runtime object or forcing consumers to
assemble a framework-specific service container.

### 6. `View` is both a type and a factory namespace with many overloads

Using one symbol for the view value type and construction namespace is
convenient, but the overload volume can make inference failures difficult to
diagnose. The repeated casts reinforce the perception that the type and
factory layers are not fully aligned.

**Suggested investigation:** audit overloads by composition family and expose
small, composable typed contracts where inference currently falls back to
casts. Preserve the fluent API unless measurements show a real maintenance or
bundle-size cost.

### 7. Widget construction has two visible paths

Some controls can be constructed directly in TypeScript, while runtime
factories create native-backed handles. Consumers may need casts when moving
between those paths, which makes ownership and disposal less obvious.

**Suggested investigation:** document which constructors are value-only,
which are runtime-bound, and which path is canonical for each control. Consider
shared public interfaces only where lifecycle semantics are genuinely equal.

## Suggested follow-up order

1. Establish and document the canonical import path and alias policy.
2. Audit `View`/`IntoView` return typing and remove unnecessary consumer casts.
3. Clarify control construction, ownership, focus, animation, and disposal.
4. Normalize theme-reference APIs and evaluate typed keys.
5. Reassess runtime capability facets only after the simpler type and lifecycle
   issues are addressed.

## Non-goals

This feedback does not authorize a public API change, a framework split, or
an application-specific convenience layer. Any change must preserve the
framework boundary, native handle identity, lifecycle semantics, and public
Rust/TypeScript parity. The existing API remains the compatibility contract
until a separately reviewed design and migration plan are approved.
