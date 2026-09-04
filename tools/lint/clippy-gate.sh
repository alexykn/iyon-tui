#!/bin/sh
# Warn-mode strictness gate for the iyon-tui workspace.
#
# Denies every default lint group and keeps the audit backlog at warn:
#   - clippy pedantic + cognitive complexity (workspace policy, warn until
#     the deny phase; see [workspace.lints] in Cargo.toml and clippy.toml)
#   - the 19 lints below, previously hidden by per-crate blanket `allow`s.
#
# Do NOT extend the -W list. New violations must be fixed; backlog items are
# removed from this list as they are fixed, until only the two audit groups
# remain and flip to deny.
set -eu
# "$@" forwards cargo flags such as --message-format=json for audits.
exec cargo clippy --workspace --all-targets --all-features "$@" -- \
  -D clippy::all \
  -D unused \
  -W clippy::pedantic \
  -W clippy::cognitive-complexity \
  -W clippy::arc_with_non_send_sync \
  -W clippy::bind_instead_of_map \
  -W clippy::bool_assert_comparison \
  -W clippy::collapsible_if \
  -W clippy::derivable_impls \
  -W clippy::field_reassign_with_default \
  -W clippy::if_same_then_else \
  -W clippy::iter_cloned_collect \
  -W clippy::let_unit_value \
  -W clippy::manual_clamp \
  -W clippy::manual_c_str_literals \
  -W clippy::manual_inspect \
  -W clippy::manual_is_multiple_of \
  -W clippy::missing_safety_doc \
  -W clippy::needless_borrow \
  -W clippy::needless_lifetimes \
  -W clippy::needless_range_loop \
  -W clippy::new_without_default \
  -W clippy::obfuscated_if_else \
  -W clippy::question_mark \
  -W clippy::redundant_slicing \
  -W clippy::too_many_arguments \
  -W clippy::type_complexity \
  -W clippy::unnecessary_cast \
  -W clippy::unnecessary_unwrap \
  -W clippy::useless_conversion \
  -W dead_code
