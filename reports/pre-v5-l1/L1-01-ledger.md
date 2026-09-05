# L1-01 migration ledger: remaining internal fluent-constructor uses (checked)

Date: 2026-09-05. This ledger is not a compatibility allowance: every row must
be emptied by its target tranche, and the final DSL deletion occurs in L1-13.
Owners are production files/functions; core unit tests (module tree) and the
`tests/ui` compile-fail contracts are out of scope — the latter already encode
the non-authoring posture and stay green.

## Native structural ingress (`crates/iyon-tui-native/src/tui/view_abi.rs`)

| Site | Fluent use | Target |
|---|---|---|
| `finish_axis_builder:614`, `publish_structural_path:2643`, `create_small_axis:2690` | `View::native_axis_from_children` final-root assembly | L1-02 final node construction |
| `validate_path_publication:2309,2344` | `View::native_content_host`, `View::spacer` finals | L1-02 / L1-04 remaining kinds |
| `parse_and_build_grid:2964-3041` | `GridTrack::*`, `View::grid` + `row_with`/`cell_with` builder | L1-04 persistent axis/grid |
| `parse_and_build_decorated:3591,3706,3402-3621` | `View::hanging`, `View::native_component`, `.foreground/.background/.container` chains | L1-02 decoration precedence |
| `parse_and_build_diff:3199-3201` | `line.with_termination`, `DiffHunk::new` | L1-04 remaining kinds |
| `text_view_from_spans:3917`, `text_view_from_owned:3924`, `cstring_text_spans:3946`, `utf8_text_spans:4115`, `parse_and_build_text_buffer:4429` | `View::styled_text().wrap().text_align().into_view()`, `TextSpan::styled` | L1-03 direct text constructors |
| `style_from_bits:3843-3860`, `parse_color_atom:3875-3911` | `StyleSpec::new` + `.foreground/.background` chains, `ColorSpec::*` atoms | L1-07 style normalization |

## Native host integration (`crates/iyon-tui-native/src/tui.rs`)

| Site | Fluent use | Target |
|---|---|---|
| `lower_style_spec:1832` | `StyleSpec::new` + `.foreground/.background/.attribute` chains from JSON | L1-07 style normalization |
| `lower_theme_color:2112`, `color_spec:2201-2257` | `ColorSpec::*/ThemeColor::*` passive values from JSON | L1-07 (types survive per §5.4; builder-method call sites migrate) |
| `tui_smoke:111` | `View::text().into_view()` smoke probe | Transport/smoke fixture (not a product path) |

## External integration tests (`crates/iyon-tui/tests/`, plan only in L1-01)

These keep compiling against the crate root in L1-01. Migration plan, not
execution: `markdown_*`, `projection_public`, `document_public`,
`pulldown_characterization` → transport/ABI fixture or binding-level projector
coverage as projectors move; `scene_public`, `history_public`, `diff_public`,
`text_origin` → binding/transport coverage as scenes/history lower; `p3c_*`,
`public_semantic_api`, `root_compile_contract` → shrink with the DSL toward
L1-13. None reference `iyon_tui::prelude` (verified), so the prelude deletion
is already transparent to them.

## Standing rule

No new production use of fluent constructors may be added behind any name.
`bun run check:tui-binding` pins the seam; this ledger pins the exit.
