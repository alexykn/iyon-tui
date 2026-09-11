# Agent validation workflow

This repository exposes two validation layers for implementation agents working through GitHub.

## Fast iteration

Use an `agent/<task>` branch. Every push to that branch runs **Agent fast validation** with three independent jobs:

- **Fast Rust correctness**: `cargo fmt`, workspace `cargo check --all-features`, and the `iyon-tui` core library tests.
- **Fast TypeScript and ABI surface**: generated ABI parity, TypeScript typechecking, declaration closure, native binding surface, ownership rules, and patch cleanliness.
- **Fast native content smoke**: builds/stages the default N-API addon and exercises the packaged content-funnel path through `native:smoke`.

The workflow uses concurrency cancellation, so a newer push supersedes stale fast runs on the same branch.

This path intentionally does **not** run Clippy or the complete test/performance matrix. It is for rapid correctness feedback while an implementation is still moving.

## Full validation

Open a pull request when the implementation is ready for comprehensive validation. The existing PR workflows provide the full gate:

- **Iyon TUI checks**
  - Rust formatting and the strict Clippy gate.
  - Full `cargo test --workspace --all-features`.
  - ABI generation cleanliness.
  - TypeScript typecheck and lint.
  - declaration/binding/ownership checks.
  - full Bun test suites.
  - canonical React/native packaged smoke and Source content FFI tests.
  - instrumented React Source-content benchmark with default-addon restore.
- **Standalone TUI native viability**
  - Linux x64 and macOS arm64 matrix.
  - workspace compile, generated ABI, canonical N-API/Source FFI tests, React
    content benchmark, and generated-file cleanliness.
- **TUI API and ABI parity**
  - stable API/ABI checks and workspace compilation.
- **TUI TypeScript surface**
  - TypeScript, ABI, binding, ownership, and package tests.

The Rust tests in `ci.yml` are explicitly allowed to execute after a Clippy failure. Clippy remains a failing gate, but lint debt can no longer hide correctness-test results from an agent.

## Connector-driven development contract

For a large implementation task, the expected flow is:

1. Create `agent/<task>` from the current target branch.
2. Commit implementation slices to that branch.
3. After each push, inspect the three fast-validation jobs and their logs; fix correctness failures before continuing.
4. When the implementation is coherent, open a pull request to trigger the full validation matrix.
5. Inspect failed jobs and raw GitHub Actions logs, patch the branch, and rerun/follow subsequent checks until only explicitly known debt remains.
6. Review the final PR diff before merge.

Do not weaken a full gate merely to obtain a green build. Known lint debt should remain visible and be fixed separately; correctness and compatibility checks should keep running independently where possible.
