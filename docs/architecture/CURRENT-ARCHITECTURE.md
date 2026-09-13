# Current architecture of iyon-tui

**Status:** current-source reference for the completed T5/M1 deletion, direct
Taffy integration, and T7/M2 content-lowering implementation. The current
route and post-tranche cleanup passed parent review and local integration
checks; historical performance
comparison remains unverified and is not a current-source acceptance claim.
**Companion:** [DOM runtime implementation ledger](DOM-RUNTIME-IMPLEMENTATION.md).

## 1. Ownership in one view

TypeScript React is the only production UI authoring route. Native Rust owns
the occurrence document, resource lifetimes, controls, content projection,
frame receipts, terminal scheduling and physical output. The old immutable
View publication ABI and ordinary ViewState plane are absent.

```text
React HostConfig / CommitCoordinator
        |
        | one qualified commitUiV1 batch
        v
NativeTuiHost -> UiResourceOwner -> OccurrenceDocument
        |                  |
        |                  +-- declared/override/effective properties
        |                  +-- Port/Connector/Control ownership
        |                  +-- topology, roots, generations and revisions
        v
SceneHost -> Taffy layout -> direct content/control/History paint
        |                                      |
        +-- bounded worker products ----------------+
                                                   v
captured frame -> terminal worker -> exact receipt -> confirmed frame
```

`SceneHost` is the current occurrence/Taffy layout and direct-paint coordinator.
Taffy owns derived geometry; content workers produce immutable semantic
products; native controls and History retain their own execution and physical
transfer contracts. There is no occurrence-to-View adapter, View allocator,
or ordinary ViewState publication path in the current route.

## 2. Repository boundaries

| Location | Current responsibility |
|---|---|
| `packages/iyon-tui/src/react/` | React host instances, commit journal, root barriers, controls and content hooks |
| `packages/iyon-tui/src/api/content/` | Source, Funnel, ContentPort and Connector caller semantics |
| `packages/iyon-tui/src/api/presentation/` | Finite public style, theme and geometry values |
| `packages/iyon-tui/src/transport/ui/generated/` | Generated direct-occurrence IDs, value descriptors and packers |
| `packages/iyon-tui/src/transport/content/` | Existing high-volume Source payload FFI and content control seam |
| `packages/iyon-tui/src/transport/native/` | Addon loading and caller-owned native resource qualification |
| `packages/iyon-tui/src/runtime/` | Host lifecycle, barriers, events, diagnostics and output waiting; no frame clock |
| `crates/iyon-tui/src/occurrence/` | Host-local generated schema types, topology, resources, controls and effective properties |
| `crates/iyon-tui/src/application/` | Native environment, direct occurrence host, content integration, controls, frame state and receipts |
| `crates/iyon-tui/src/presentation/` | Direct occurrence geometry, Taffy layout, content capture and physical paint products |
| `crates/iyon-tui/src/history/` | History semantics, physical transfer and confirmed-prefix handling |
| `crates/iyon-tui/src/content/`, `projection/` | Source-rooted semantic content and delivery behavior |
| `crates/iyon-tui-native/src/tui/ui_commit.rs` | Qualified UI batch decoder and native binding glue |
| `crates/iyon-tui-native/src/content_ffi.rs` | Existing Source payload ABI; independent from UI batch encoding |
| `tools/tui-abi/ui_abi.toml` | Sole current UI schema input |
| `tools/tui-abi-gen/` | UI schema generator and generated-output check |

The implementation crate remains unpublished as an application SDK. The
curated `iyon_tui::binding` module exports only the native seam required by
the addon: occurrence contracts, content/source types, native host/control
mechanics that remain in use, passive styling types and perf counters.

## 3. Direct occurrence and effective properties

`OccurrenceDocument` is the sole desired UI identity and topology owner. Node
and resource handles are host-qualified `(namespace, slot, generation, kind)`
values. Stale generations and wrong kinds fail at the native boundary. The
document reserves its body root and rejects orphaned surviving nodes, cycles,
wrong anchors, duplicate resource attachments and invalid owner transfers.

Each occurrence stores declared values and explicit overrides. Native control
facts, inherited theme context and style-state selection are combined into an
effective snapshot before SceneHost performs direct Taffy layout and paint.
A declared update
does not erase an override; clearing an override reveals the latest declared
value. Last-write coalescing and true no-op behavior are implemented by the
occurrence commit owner, not by a second state registry.

`UiResourceOwner` keeps occurrence Port/Connector/Control resource records and
qualified Source membership. It is not a mirror of the document: occurrence
records remain authoritative, while ContentHostRegistry and concrete native
controls are execution owners for their respective products. External Sources
survive React unmount when their caller-owned lifetime remains live.

## 4. React and native execution

`Tui.open` creates one native host. `createReactRoot(tui)` reserves one root
authority and sends accepted structural/content/control changes through the
same `CommitCoordinator` revision stream. `whenVisible` waits on native
confirmed revisions and, where requested, confirmed content visibility. JS is
an observer of the native event/failure lanes; it is not the frame clock.

The native environment drains accepted work without a mandatory TypeScript
pump. Native TextInput, paste routing, global-before-local key handling,
Scroll state, Animation deadlines and output FIFO remain native mechanics.
Animation frame selection is synchronized back into the occurrence control
facts before a scheduler-only content demand pass. This lets a deadline update
the current frame without resetting native editor state or requiring a React
commit.

The native host owns one presentation candidate/receipt at a time. Candidate
products retain exact Source/resource/frame data until the corresponding
receipt completes. An older receipt can promote only its captured product;
newer desired work stays pending. Failed output marks physical synchronization
unknown and preserves the confirmed prefix. Close joins pending receipts and
releases resources through their actual owners.

## 5. Content and History

Source bytes and annotations enter through the existing qualified content FFI.
N-API creates Source/Port/Connector objects and carries control operations;
there is no second payload implementation. Content projection is paced per
Connector and width, with Source lifetime independent of a mounted React
occurrence.

History behavior remains under the native History model and transfer owner.
React `History`/`HistoryUnit` roots are typed occurrence roots with explicit
actions and root configuration. Physical export requires a complete supported
content adapter shape and a confirmed receipt; composite or physically
incomplete units are blocked rather than exported as content-only rows.

## 6. Deleted M1 owners

The following paths and APIs no longer exist and must not be recreated:

- `crates/iyon-tui-native/src/tui/view_abi.rs` and `src/tui/view_state.rs`;
- generated native View ABI Rust files, C View header, state schema, generated
  TypeScript View calls/manifest and state envelope;
- `tools/tui-abi/view_abi.toml` and its old generator render/validation
  modules, templates, snapshot, wrapper test and benchmark registry;
- `crates/iyon-tui/src/application/view_state.rs` and the entire ordinary
  `retained_state/` registry/record/effect/capture plane;
- standalone NativeHistory/ViewSlot/ScrollPane ViewRef N-API classes and
  `setDesiredViewRef`, ViewState and structural attachment methods;
- binding exports for ViewRefs, NativeRefs, path/build/edit publication,
  NativeCommonPatch, WeakView, HostViewState and ordinary ViewState patches.

The current UI schema generator emits only:

- `crates/iyon-tui/src/occurrence/generated.rs`;
- `packages/iyon-tui/src/transport/ui/generated/ui_schema.ts`;
- `packages/iyon-tui/src/transport/ui/generated/ui_abi_manifest.json`;
- `docs/architecture/generated/UI-ABI-REFERENCE.md`.

The checked-in binding and Rust API snapshots were reduced to the current
seam. Generated-output checks and staging inspect the UI schema and existing
content ABI; no old/new selector or forwarding stub remains.

## 7. Current execution owners

The current route has no legacy occurrence-to-View adapter or deferred M2
deletion gate. The following private modules remain because they own current
behavior:

1. `crates/iyon-tui/src/scene/host.rs` coordinates occurrence snapshots,
   Taffy-driver requests, content products, direct paint and candidate frames.
2. `crates/iyon-tui/src/presentation/taffy.rs`,
   `presentation/direct.rs` and `presentation/direct_tree.rs` own derived
   geometry and direct physical products. The presentation API and paint
   modules provide finite values, content realization and styling.
3. Content projectors, Source FFI, and History transfer helpers retain their
   independent semantic, lifetime and physical-prefix contracts.
4. Host-owned editor, scroll and animation components retain concrete input,
   focus and deadline behavior. They are not View publication targets or a
   second semantic UI tree.

These owners are part of the direct occurrence/Taffy/content route. Future
changes must preserve their ownership boundaries and must not recreate the
deleted View ABI, ordinary ViewState registry, or compatibility selector.

## 8. Verification

Relevant focused checks for this slice are:

```text
cargo fmt --all -- --check
cargo test -p tui-abi-gen
cargo test -p iyon-tui --features native-host --lib
cargo test -p iyon-tui-native --lib --tests
bun run check:tui-abi
bun run check:tui-binding
bun run check:tui-declarations
bun run check:ownership
bun run typecheck
bun run native:stage
bun test packages/iyon-tui/tests packages/tui-consumer-fixture/tests
```

The default staged addon is the darwin-arm64 normal N-API artifact. Instrumented
performance staging is opt-in (`ION_NATIVE_FEATURES=perf-counters`) and must be
followed by default staging before packaging or handoff. Linux x64 viability
requires an available Linux target/toolchain; a macOS build is not Linux
evidence. This slice's full workspace/all-features tests, strict Clippy,
generator, TypeScript/Biome, declaration/binding/ownership, packaged smoke,
canonical Source content-FFI tests, consumer suite and instrumented benchmark
passed; the resulting M1 source is ready for parent review. Source FFI is part
of the canonical addon and is not a second UI transport. The parent retains
final source/design acceptance, and the unavailable Linux gate remains
explicit.
