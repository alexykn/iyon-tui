# 15 — Terminal backend

## 0. Baseline, scope and evidence status

### Baseline

- Repository: `iyon-tui`
- Branch: `main`
- Source baseline: `4355c02d6853549adf32a1e038b14665ce5c6bf8`
- Primary scope:
  - `crates/iyon-tui/src/backend/`
  - `crates/iyon-tui/src/terminal/` recursively
- Supporting seam files inspected:
  - `crates/iyon-tui/src/application/host.rs`
  - `crates/iyon-tui/src/application/run.rs`
  - `crates/iyon-tui/src/scene/host.rs`
  - `crates/iyon-tui/src/history/native/mod.rs`
  - `crates/iyon-tui/src/history/model.rs`
  - `crates/iyon-tui/src/history/native/frontier.rs`
  - `crates/iyon-tui/src/application/tests.rs`
  - `crates/iyon-tui/src/application/tests/driver.rs`
  - `crates/iyon-tui/Cargo.toml`
- Required context inspected:
  - `docs/architecture/atlas-4355c02/REPORT-CONTRACT.md`
  - `docs/architecture/atlas-4355c02/README.md`
  - `AGENTS.md`
  - `PRE-V5-ARCHITECTURE-REPORT.md`
  - `LAY-1-main-screen-scrollback-and-resize.md`
  - relevant PERF-13 implementation notes and completion references

The source manifest confirms eleven assigned source files: two under `backend/` and nine under `terminal/`. All assigned production files were inspected; the large test sections in `presenter.rs` and `shadow.rs` were also inspected, including the later native-scrollback and failure tests.

### Evidence status

This is a static source investigation. No tests, builds, benchmarks, terminal sessions, or runtime validation were executed. Behavioral statements below are derived from source and test contracts. Where tests encode an independent model, that is identified as test evidence rather than executed evidence.

The framework boundary from `REPORT-CONTRACT.md` and `AGENTS.md` is respected here: the terminal backend is generic terminal session, input, output, resize, and restoration machinery. No product/application meaning is present in the assigned backend code.

### Primary conclusion

The current implementation is a private, native Rust terminal session with:

1. a `TermwizBackend` façade owned by the application host;
2. a dedicated Termwiz worker thread that owns the actual `termwiz::terminal::Terminal` and `TermwizPresenter`;
3. a separate Crossterm blocking input reader thread;
4. an asynchronous frame handoff protocol using `tokio::sync::oneshot`;
5. synchronous command/reply protocols for native-history insertion, final cursor positioning, and restoration;
6. a full-screen retained Termwiz `Surface` used as the presenter’s shadow of the visible screen;
7. native scrollback insertion implemented by ordinary full-screen CRLF overflow, with model-only `ScrollRegionUp` used to update the retained shadow;
8. no alternate-screen activation;
9. resize handling that invalidates the presenter’s visible-screen model but does not reconcile or clear terminal-native scrollback.

The backend is intentionally private (`pub(crate)` throughout). The important architectural boundary is `TerminalBackend`, not Termwiz itself.

---

## 1. Responsibility and structure

### 1.1 File inventory

Approximate LOC below uses physical source line ranges and excludes test modules where they can be distinguished. It is an architectural estimate rather than an executable `wc -l` count.

| Path | Language | Approx. production LOC | Approx. test LOC | API visibility | Primary responsibility | Secondary responsibilities |
|---|---:|---:|---:|---|---|---|
| `crates/iyon-tui/src/backend/mod.rs` | Rust | 5 | 0 | `pub(crate)` module, private export | Declares the private backend-neutral module and re-exports `NativeHistorySink` | Establishes the seam used by Scene/History/application |
| `crates/iyon-tui/src/backend/native_history.rs` | Rust | 15 | 0 | `pub(crate)` trait | Defines the acknowledged-prefix native-history sink contract | Couples physical rows to backend output without exposing a concrete terminal |
| `crates/iyon-tui/src/terminal/mod.rs` | Rust | 8 | 0 | `pub(crate)` modules/re-exports | Aggregates terminal backend, Crossterm, and Termwiz modules | Re-exports `PresentReceipt`, `TerminalBackend`, `TerminalEvent`, and worker-stop classification |
| `crates/iyon-tui/src/terminal/backend.rs` | Rust | 49 | 0 | All `pub(crate)` | Defines the generic private terminal session protocol | Typed worker-stop error, semantic event enum, asynchronous presentation receipt |
| `crates/iyon-tui/src/terminal/crossterm/mod.rs` | Rust | 119 | 0 | `pub(crate)` | Raw-mode/bracketed-paste setup, restoration, event mapping, blocking reader thread | Event polling, shutdown flag, error forwarding |
| `crates/iyon-tui/src/terminal/crossterm/key.rs` | Rust | 107 | 93 | `pub(crate)` helper | Converts Crossterm key events into generic `KeyStroke` values | Modifier/media-key mapping, release filtering, canonical BackTab handling |
| `crates/iyon-tui/src/terminal/termwiz/mod.rs` | Rust | 8 | 0 | `pub(crate)` export | Private Termwiz module aggregation | Re-exports `TermwizBackend`; test-only shadow module |
| `crates/iyon-tui/src/terminal/termwiz/backend.rs` | Rust | 167 | 0 | `pub(crate)` | Host-side handle to worker commands, input events, viewport, receipts, and restoration | Owns worker/input lifetimes and cached dimensions |
| `crates/iyon-tui/src/terminal/termwiz/lower.rs` | Rust | 257 | 123 | `pub(crate)` | Lowers prepared physical frames and rows into Termwiz `Surface`/`Change` sequences | Overlay composition, color/style conversion, wide-grapheme handling |
| `crates/iyon-tui/src/terminal/termwiz/presenter.rs` | Rust | 282 | ~1,111 | `pub(crate)` | Retained visible-surface diffing and native-history transaction emission | Sync output, full repaint, scrollback model updates, cursor state |
| `crates/iyon-tui/src/terminal/termwiz/shadow.rs` | Rust | 620 | ~113 | Test-only module | Independent in-memory terminal interpreter/oracle | Scrollback capture, styled spans, failure injection, wide-cell semantics |
| `crates/iyon-tui/src/terminal/termwiz/worker.rs` | Rust | 163 | 0 | `pub(crate)` | Owns actual Termwiz terminal and serially executes terminal commands | Capability probing, initialization, command dispatch, restoration |

The assigned production implementation is approximately 1,800 physical lines, with approximately 1,400 additional test lines concentrated in `key.rs`, `lower.rs`, `presenter.rs`, and `shadow.rs`. The overwhelming production complexity is in `presenter.rs` and `shadow.rs`, not the trait boundary or worker command protocol.

### 1.2 Module structure

```text
crate::backend
└── NativeHistorySink
    └── implemented by TermwizBackend and headless/test sinks

crate::terminal
├── backend
│   ├── PresentReceipt = oneshot::Receiver<Result<()>>
│   ├── TerminalEvent
│   ├── TerminalWorkerStopped
│   └── TerminalBackend
├── crossterm
│   ├── setup / restore
│   ├── map_event
│   ├── EventReader
│   └── key::key_stroke
└── termwiz
    ├── TermwizBackend
    ├── worker
    │   └── TerminalCommand / actual termwiz::Terminal
    ├── presenter
    │   └── retained presented Surface and output protocol
    ├── lower
    │   └── Physical Surface/Rows -> termwiz Surface/Change
    └── shadow [test only]
```

### 1.3 Primary and secondary ownership

- `TerminalBackend` owns the semantic session operations expected by the native host.
- `TermwizBackend` owns the host-side transport handles and lifecycle flags, but not the actual terminal object.
- `worker::run` owns the actual terminal object and the `TermwizPresenter`.
- `TermwizPresenter` owns the last known visible terminal `Surface` and the state necessary to decide between diff, full repaint, native scrollback insertion, and synchronized-output closure.
- `lower.rs` creates per-frame Termwiz surfaces and per-history-insertion change sequences. It does not retain the source physical surface.
- Crossterm owns input decoding and terminal mode toggles. It does not own output.
- `NativeHistorySink` is a deliberately narrow output-side acknowledgment seam. History owns native frontier state; the sink only accepts physical rows and returns an accepted count or error.

### 1.4 Features and dependency posture

`crates/iyon-tui/Cargo.toml` declares non-optional dependencies on:

- `crossterm`
- `termwiz`
- `tokio`
- `anyhow`
- Unicode and presentation dependencies

The `native-host`, `test-util`, and `perf-counters` features do not select alternate terminal backends. Termwiz and Crossterm are both compiled as ordinary crate dependencies. The terminal source contains no feature-gated production fallback between Termwiz and Crossterm output implementations.

---

## 2. Types, APIs and contracts

### 2.1 `NativeHistorySink`

`crates/iyon-tui/src/backend/native_history.rs:5-15`:

```rust
pub(crate) trait NativeHistorySink {
    type Error;

    fn insert_history_rows(&mut self, rows: &[PhysicalRow])
        -> Result<usize, Self::Error>;
}
```

The documentation establishes an exact prefix contract:

- `Ok(k)` means exactly `rows[..k]` entered native history.
- No later row entered.
- An adapter that accepts a prefix and then encounters an error must return `Ok(k)`.
- `Err` means the call accepted zero rows.

This is an irreversible side-effect protocol. History advances its native frontier only after an acknowledged prefix.

A consequential tension exists between this formal contract and the broader recovery comments in `history/native/mod.rs`. History treats an ordinary sink error as potentially having performed a partial physical write and marks synchronization unknown rather than rewinding. The Termwiz presenter returns only `Err` or `Ok(rows.len())`; it has no physical partial-write count. Thus:

- logical acceptance is all-or-nothing at the presenter API;
- physical terminal mutation may not necessarily be all-or-nothing at the I/O layer;
- History deliberately preserves the logical frontier and records an unknown synchronization state on error.

This is safe against logical rollback corruption, but a real partial terminal write cannot be precisely reconciled by the current presenter because native scrollback is not addressable.

### 2.2 `PresentReceipt`

`crates/iyon-tui/src/terminal/backend.rs:6`:

```rust
pub(crate) type PresentReceipt = oneshot::Receiver<Result<()>>;
```

`begin_frame` returns before terminal I/O has completed. The receipt is fulfilled by the worker only after `presenter.present(...)` returns, which itself requires `terminal.render(...)` and `terminal.flush(...)` to succeed.

The receipt is therefore the visibility/physical-commit boundary for a prepared candidate frame. `application/host.rs` retains the candidate frame and associated state/content commit plans until this receipt succeeds.

### 2.3 `TerminalEvent`

`crates/iyon-tui/src/terminal/backend.rs:30-36`:

```rust
pub(crate) enum TerminalEvent {
    Key(crate::KeyStroke),
    Paste(String),
    Resize,
}
```

This is a generic semantic event boundary:

- physical Crossterm key events become `KeyStroke`;
- bracketed paste becomes `Paste(String)`;
- any Crossterm resize event becomes `Resize`.

There are intentionally no mouse or focus variants. Crossterm `FocusGained`, `FocusLost`, and `Mouse(_)` events are ignored by `map_event`. `ProbeHints::mouse_reporting(Some(false))` also disables mouse reporting in Termwiz capability probing.

### 2.4 `TerminalBackend`

`crates/iyon-tui/src/terminal/backend.rs:38-49`:

```rust
pub(crate) trait TerminalBackend:
    NativeHistorySink<Error = anyhow::Error>
{
    fn try_next_event(&mut self) -> Result<Option<TerminalEvent>>;
    fn viewport(&mut self) -> Result<Size>;
    fn begin_frame(&mut self, frame: &PreparedSceneFrame)
        -> Result<PresentReceipt>;
    fn position_after_final_frame(&mut self) -> Result<()>;
    fn restore(&mut self) -> Result<()>;
}
```

The contract combines five concerns:

1. nonblocking semantic input polling;
2. current viewport dimensions;
3. asynchronous frame submission;
4. final cursor positioning;
5. terminal restoration.

It is private and only consumed by the native application host and test driver. It is not a public Rust authoring API.

The trait is intentionally coupled to `PreparedSceneFrame`, which contains:

- a complete physical `Surface`;
- an optional `HistoryPhysicalOverlay`;
- a `DamageRegion`;
- retained-state bindings.

The Termwiz backend consumes the complete surface and history overlay. It does not consume `damage` or `state_bindings`; those are upstream host/runtime commit metadata.

### 2.5 Worker-stop error

`crates/iyon-tui/src/terminal/backend.rs:8-27` defines `TerminalWorkerStopped`. Sending a command through a disconnected `std::sync::mpsc::Sender` produces this typed error. The host recognizes it with `is_terminal_worker_stopped`.

This is preferable to matching the human-readable `"terminal worker stopped"` string. The distinction is used by `application/host.rs` to classify a backend that can no longer accept frames as `BACKEND_NOT_READY` and, in some shutdown paths, to treat worker death as an expected terminal shutdown condition.

### 2.6 Crossterm key contract

`crates/iyon-tui/src/terminal/crossterm/key.rs`:

- release events are dropped (`KeyEventKind::Release`);
- press and repeat are accepted;
- `BackTab` becomes `Key::Tab` with `SHIFT` added;
- all Crossterm modifier bits are preserved;
- media and modifier key variants are mapped exhaustively;
- `KeyCode::Char('\n')`, `KeyCode::Char('\r')`, and ETX remain character strokes rather than being invented as `Enter`;
- `Enter` and `Shift+Enter` remain distinct.

The tests at `key.rs:116-200` protect those normalization rules.

---

## 3. Dependency and ownership map

### 3.1 Forward dependency map

```text
Application host
  ├── owns HostBackend::Real(TermwizBackend)
  ├── calls TerminalBackend::viewport()
  ├── calls TerminalBackend::begin_frame(PreparedSceneFrame)
  ├── polls PresentReceipt
  ├── calls NativeHistorySink through Scene/History preparation
  └── calls position_after_final_frame() then restore()

TermwizBackend
  ├── std Sender<TerminalCommand> ───────────────┐
  ├── tokio UnboundedReceiver<Result<Event>>     │
  ├── Crossterm EventReader ─────────────────────┤
  ├── cached Size                                │
  └── worker JoinHandle                          │
                                                 ▼
Termwiz worker thread
  ├── Box<dyn termwiz::terminal::Terminal + Send>
  └── TermwizPresenter
       ├── presented Surface
       ├── known
       └── sync_output_active

PreparedSceneFrame
  ├── physical::Surface ──lower::desired_surface──> termwiz::Surface
  └── HistoryPhysicalOverlay ──────────────────────> effective terminal cells

PhysicalRow ──lower::row_changes──────────────────> termwiz::Change sequence
```

### 3.2 Ownership and destruction

| Resource | Created by | Owned by | Destroyed/released by |
|---|---|---|---|
| `TermwizBackend` | `application::host` | `HostBackend::Real` inside `HostInner` | Explicit host close, then `Drop` fallback |
| Termwiz worker thread | `TermwizBackend::enter` | `TermwizBackend.worker` join handle | `restore()` joins it; `Drop` calls restore if needed |
| Actual Termwiz terminal | `worker::setup_terminal` | Worker thread | Worker cleanup via `restore_terminal` |
| `TermwizPresenter` | Worker setup | Worker thread | Worker thread exits |
| Crossterm input reader | `TermwizBackend::enter` after worker startup | `TermwizBackend.input` | `restore()` takes it and calls `stop`; `Drop` fallback |
| Input worker thread | `EventReader::start` | `EventReader.worker` | `EventReader::stop` sets atomic flag and joins |
| In-flight frame | Host candidate state plus worker command | Host until receipt; worker while command executes | Host commit on receipt success, candidate discard on failure |
| Visible presenter `Surface` | `TermwizPresenter::new` and `present` | Worker/presenter | Presenter destruction |
| Native rows | History/frontier and command clone | History until sink ack; worker command during insertion | Frontier advances on ack; command vector drops after handling |

### 3.3 Threading and serialization

Output is serialized through the worker:

- `begin_frame` sends a `Present` command and returns a oneshot receiver;
- `insert_history_rows`, `position_after_final_frame`, and `restore` send synchronous commands and block on standard-library reply channels;
- the worker receives one `TerminalCommand` at a time and mutates the terminal/presenter serially.

Input is separate:

- `EventReader` polls Crossterm on its own thread;
- raw `Event` values are sent through an unbounded Tokio channel;
- `TermwizBackend::try_next_event` drains ignored events until a semantic event or channel-empty result appears.

The output worker never reads input. The input reader never writes output.

### 3.4 Reverse dependencies

- `application/host.rs` is the production consumer of `TerminalBackend` and `TermwizBackend`.
- `scene/host.rs` consumes `NativeHistorySink` while preparing a frame and native-history pressure.
- `history/native/mod.rs` is the production consumer of `NativeHistorySink`.
- `application/tests.rs` and `application/tests/driver.rs` define fake backends and headless sinks against the same private contracts.
- `physical/text_metrics.rs`, presentation wrapping, and backend lowering independently use Termwiz’s grapheme-width calculation to keep physical width and emitted terminal text aligned.

### 3.5 Architectural coupling

The backend does not reach directly into composition or application state. It does, however, receive a `PreparedSceneFrame` that carries a History-specific physical overlay. This means the current terminal lowering boundary is not purely `PhysicalSurface -> terminal bytes`; it is:

```text
PreparedSceneFrame
  = physical screen surface
  + optional HistoryPhysicalOverlay
  + host commit metadata
```

Only the first two fields are relevant to terminal output. The overlay coupling is generic History behavior, not Iyon product logic.

---

## 4. Execution paths and state transitions

### 4.1 Initialization

Production path:

```text
application::host
  -> TermwizBackend::enter()
  -> spawn "iyon-terminal" worker
  -> worker::run()
  -> setup_terminal()
      -> ProbeHints::new_from_env().mouse_reporting(Some(false))
      -> Capabilities::new_with_hints(...)
      -> termwiz::terminal::new_terminal(...)
      -> crossterm::setup()
          -> enable_raw_mode()
          -> EnableBracketedPaste on stdout
      -> terminal.get_screen_size()
      -> render hidden cursor + "\r\n".repeat(rows)
      -> presenter.present(empty Surface)
  -> startup Size sent to TermwizBackend
  -> EventReader::start(...)
```

The worker performs terminal setup before the input reader is started. It establishes an inline main-screen viewport and does not call `enter_alternate_screen`.

Initialization writes a number of CRLFs equal to the detected terminal height, hides the cursor, and paints an empty frame. The initial presenter state is `known = false`, so the empty surface goes through `full_repaint_changes`.

The startup size is converted from Termwiz’s `usize` dimensions to framework `u16` dimensions.

### 4.2 First frame

A prepared frame reaches `TermwizBackend::begin_frame`:

1. `lower::desired_surface(frame)` creates a new Termwiz `Surface`.
2. Every physical row is lowered into `Change` operations.
3. The History overlay is resolved into effective cells while lowering.
4. The command is sent to the worker.
5. The host retains a oneshot receipt and does not promote the candidate yet.
6. Worker `handle_command(Present)` calls `TermwizPresenter::present`.
7. Presenter performs a full repaint because `known` is initially false.
8. `terminal.render` and `terminal.flush` must succeed.
9. The worker sends the result through the oneshot.
10. Host receipt polling allows logical candidate commit only after success.

`PreparedSceneFrame.damage` does not influence Termwiz output. The presenter diffs complete surfaces, not damage rectangles. Therefore current backend submission allocates and transmits a complete desired Termwiz `Surface` even for a small damage region.

### 4.3 Normal differential frame

For a known, same-sized presenter:

```text
desired Surface
  -> presented.diff_screens(&desired)
  -> if empty and no active sync: no terminal write
  -> otherwise append canonical terminal state
  -> terminal.render(changes)
  -> terminal.flush()
  -> presented = desired
```

The presenter’s retained `presented` surface is updated only after both render and flush succeed.

`canonical_terminal_state(height)` resets attributes, moves the cursor to `(0, height - 1)`, and hides the cursor. Thus every successful nonempty differential frame leaves a deterministic output state.

If render or flush fails:

- `known` becomes false;
- any active synchronized-output transaction is terminated best effort;
- the desired surface is not installed as `presented`;
- the error reaches the worker reply and then the host.

### 4.4 Native-history insertion

History calls `NativeHistorySink::insert_history_rows`. The Termwiz path is:

```text
History native frontier
  -> TermwizBackend::insert_history_rows(rows)
  -> clone rows into TerminalCommand::InsertHistory
  -> worker::handle_command
  -> TermwizPresenter::insert_history
  -> native_transaction(rows, height, begin_sync)
  -> terminal.render(transaction)
  -> terminal.flush()
  -> update presenter shadow with model-only ScrollRegionUp
  -> return Ok(rows.len())
```

The native transaction is intentionally not a terminal scroll-region command. For each chunk of at most `height` rows:

1. lower each row with `row_changes(row, row_index, true)`;
2. reset attributes;
3. move the cursor to the bottom row;
4. emit `"\r\n".repeat(chunk.len())`;
5. move the cursor to the row where the new visible content begins;
6. reset attributes;
7. clear to end of screen.

On Unix, the first insertion emits `SYNC_BEGIN = "\x1b[?2026h"` and the final sequence emits `SYNC_END = "\x1b[?2026l"`. Multiple insertion calls can share one synchronized-output region. The next normal frame, final positioning, resize, or failure closes it.

After successful physical output, the presenter updates its model using:

```rust
Change::ScrollRegionUp {
    first_row: 0,
    region_size: height,
    scroll_count: count,
}
```

That `Change` is applied only to the in-memory `presented` Termwiz `Surface`. It is never sent to the real terminal. Native scrollback is created exclusively by ordinary bottom-row CRLF flow.

### 4.5 Presentation failure

`TermwizPresenter::present` and `insert_history` treat terminal render/flush errors as failed operations:

- `present` leaves the previous `presented` surface intact;
- `insert_history` does not advance the model’s native scroll count;
- `known = false`;
- synchronized output is closed best effort.

At the host level, a failed in-flight receipt marks `physical_sync_unknown = true`. A subsequent successful candidate frame clears this host-side marker and calls `host_recover_native_history_synchronization`.

History itself separately marks `History.native.synchronization_unknown` when a native sink returns an error. `scene/host.rs` detects this marker and skips another native emission during the same preparation attempt to avoid blindly duplicating rows. It paints a recovery frame instead.

The current recovery model restores the visible-screen shadow but cannot inspect or rewrite terminal-native scrollback. This is a consequential limitation discussed further in §8 and §9.

### 4.6 Resize

Resize event path:

```text
Crossterm Event::Resize(width, height)
  -> EventReader channel
  -> TermwizBackend::try_next_event
  -> self.size = Size::new(width, height)
  -> TerminalEvent::Resize
  -> application host invalidates frame
  -> next frame obtains viewport from cached self.size
  -> lower desired frame with new dimensions
  -> presenter.present sees dimensions differ
```

When dimensions differ:

1. Any active synchronized output is ended best effort.
2. The retained `presented` surface is resized.
3. `known = false`.
4. The next frame is full-repainted.

There is no explicit worker resize command and no second `get_screen_size` call. The Crossterm resize event is the dimension source after startup.

The presenter’s `resize` only resizes its visible `Surface` model and forgets knowledge of the physical screen. It does not clear native scrollback or rebuild previously transferred History rows. The source test `shadow_scrollback_survives_full_repaint` explicitly asserts that full repaint does not alter the shadow’s scrollback tape.

### 4.7 Final positioning and restoration

Host shutdown first waits for any earlier receipt, exits the running application, renders a final frame, and waits again. Then:

```text
TermwizBackend::position_after_final_frame()
  -> worker PositionAfterFinalFrame
  -> presenter.finish_sync_output_best_effort()
  -> canonical_terminal_state(height)
  -> terminal.render + flush
```

For a real backend, the host then calls `backend.restore()`:

```text
TermwizBackend::restore()
  -> idempotence guard: restored
  -> stop and join Crossterm EventReader
  -> worker Restore command
  -> finish synchronized output
  -> restore_terminal()
      -> reset attributes + show cursor
      -> flush
      -> crossterm::restore()
          -> DisableBracketedPaste
          -> disable_raw_mode
      -> final flush
  -> join terminal worker
```

`TermwizBackend::Drop` invokes `restore()` if explicit restoration did not occur.

Worker shutdown also performs cleanup after its command loop ends, so explicit `Restore` results in a second best-effort cleanup pass after the command handler returns `true`. The operations are intended to be idempotent, though they can issue repeated terminal mode/output calls.

---

## 5. Alternate routes and failure semantics

### 5.1 Semantic operation to production path

| Semantic operation | Production path | Selection/route condition | Failure behavior |
|---|---|---|---|
| Poll input | Crossterm `poll`/`read` thread -> Tokio unbounded channel -> `TermwizBackend::try_next_event` | Real backend only; headless host has no terminal events | Poll/read error is sent once, reader exits, backend returns the error; channel disconnect becomes `"terminal input closed"` |
| Key normalization | `crossterm::Event::Key` -> `key::key_stroke` -> `TerminalEvent::Key` | Release events dropped; repeat retained | Unsupported variants are not present in the exhaustive Crossterm enum mapping |
| Paste | Crossterm `Event::Paste` -> `TerminalEvent::Paste` | Bracketed paste enabled during setup | Setup failure restores raw mode before returning |
| Resize | Crossterm `Event::Resize` -> `TermwizBackend.size` update -> `TerminalEvent::Resize` | Every resize event | No explicit worker resize; next frame invalidates and full-repaints |
| Present frame | `PreparedSceneFrame` -> `lower::desired_surface` -> `TerminalCommand::Present` -> presenter diff/full repaint | Real backend | Typed worker-send failure, lost receipt, or render/flush failure; candidate remains logically uncommitted |
| Native History rows | `History` -> `NativeHistorySink` -> `InsertHistory` -> `native_transaction` | History native frontier has rows and is not synchronization-unknown | `Ok(k)` prefix contract; Termwiz currently returns all-or-error; errors mark physical synchronization unknown upstream |
| Final cursor position | `PositionAfterFinalFrame` -> canonical state | Shutdown only | Synchronous worker reply failure |
| Restoration | Stop input -> `Restore` command -> reset/show cursor -> disable paste/raw mode | Explicit shutdown or `Drop` | First error retained; later cleanup still attempted; `restored` prevents retry |

### 5.2 Compatibility and specialization classification

- **Legitimate alternate mode:** headless sinks in tests and host mode. These are not terminal fallbacks; they implement the same private `NativeHistorySink`/`TerminalBackend` contracts without physical I/O.
- **Performance specialization:** differential present versus full repaint. Both are paths within one presenter, selected by `known`, dimensions, and surface diff.
- **Required recovery mechanism:** model-only `ScrollRegionUp` versus real terminal CRLF. This is not a fallback; it is the separation between physical protocol and shadow-model maintenance.
- **Compatibility behavior:** ignored focus/mouse events and Unix-only synchronized-output sequences.
- **No silent output fallback:** there is no Crossterm output path that is selected if Termwiz output fails. Crossterm is input/mode support; Termwiz is output/capability support.

### 5.3 Not-ready versus I/O failure

The host distinguishes:

- command sender disconnected: typed `TerminalWorkerStopped`, treated as backend not-ready;
- a closed presentation receipt: reply lost, classified as backend not-ready;
- a worker presentation result containing an I/O error: backend I/O failure;
- input channel closure: terminal input closed;
- History sink failure: native transfer failure and synchronization unknown.

This is stronger than string-only error matching at the worker boundary, although some receiver-loss errors remain ordinary `anyhow` text (`"terminal worker reply lost"` and `"terminal presentation reply lost"`).

### 5.4 Important setup failure edge

`worker::setup_terminal` uses `setup_error` for most failures after terminal creation:

- Crossterm setup failure;
- screen-size query failure;
- initial viewport write failure;
- initial presenter paint failure.

However, the final conversion from Termwiz `usize` dimensions to framework `u16` occurs after initial terminal setup and painting:

```rust
u16::try_from(size.cols).context(...)?
u16::try_from(size.rows).context(...)?
```

If either conversion fails, `setup_terminal` returns an error directly. `worker::run` reports startup failure and exits, but the `run` error branch does not call `restore_terminal` because `setup_error` was not used for this final conversion. This leaves a theoretical initialization cleanup hole for terminal dimensions outside the framework’s `u16` range. It is likely rare in ordinary terminals but is source-visible and should remain an open lifecycle concern.

### 5.5 Restoration error semantics

`crossterm::restore` always attempts both:

1. disable bracketed paste;
2. disable raw mode.

It retains the first error. `worker::restore_terminal` separately attempts:

1. reset attributes and show cursor;
2. flush;
3. Crossterm restoration;
4. final flush.

The first observed error wins, but all cleanup stages are attempted. `TermwizBackend::restore` marks `restored = true` before attempting cleanup, so a failed restoration is not retried by `Drop`.

---

## 6. Caches, invalidation, scheduling and performance

### 6.1 Presenter cache state

`TermwizPresenter` retains:

- `presented: termwiz::Surface`
- `known: bool`
- `sync_output_active: bool`

The cache key is effectively:

```text
visible dimensions
+ every visible cell's grapheme and attributes
+ active synchronized-output state
```

There is no explicit frame revision or damage-key cache in the terminal layer.

Invalidation:

- dimensions differ: resize retained surface and set `known = false`;
- render/flush failure: set `known = false`;
- successful `apply`: replace entire retained `Surface`, set `known = true`;
- successful native insertion: mutate retained model via model-only scroll and keep `known = true`;
- sync output completion/failure: set `sync_output_active = false`.

### 6.2 Per-frame work

For every submitted frame:

- `lower::desired_surface` allocates a new Termwiz `Surface`;
- every row is visited;
- each painted cell is converted to text/style changes grouped by contiguous style;
- the complete desired surface is transferred through the worker command;
- the presenter either:
  - full-repaints every visible cell when unknown, or
  - computes `diff_screens` between the retained and desired complete surfaces.

The prepared frame’s upstream `DamageRegion` is not used by the backend. The terminal layer relies on Termwiz’s screen diff, not the framework’s damage rectangles.

### 6.3 Native-history work

For each native-history insertion:

- rows are cloned in `TermwizBackend::insert_history_rows`;
- rows are lowered into `Change` sequences;
- each row may emit:
  - an absolute cursor move;
  - attribute changes;
  - text grouped by style;
  - optional clear-to-end-of-line;
- each chunk performs CRLF overflow and cleanup;
- presenter mutates its retained model with model-only scrolling.

A batch larger than terminal height is chunked. Tests cover row counts both below and above viewport height, including 6 rows into a 4-row viewport and batches up to 10 rows.

### 6.4 Scheduling

The terminal backend itself does not schedule frames or ticks. Scheduling is upstream in the application/environment host. The backend provides:

- nonblocking input poll;
- asynchronous output submission;
- receipt polling;
- synchronous finalization operations.

The host enforces at most one outstanding presentation receipt. If `presentation.is_some()`, new rendering reports that it is waiting rather than submitting another frame. This bounds worker command backlog and preserves candidate/visible ordering.

### 6.5 No backend-level counters

No production counters were found in the assigned backend/terminal modules. The source does contain test-only behavioral state such as `implicit_wraps`, `sync_output_active`, shadow scrollback, and render-failure toggles, but there are no runtime output byte, change-count, flush-count, or latency counters in this layer.

---

## 7. Tests, benchmarks and observability

### 7.1 Test contracts

#### Key mapping

`crossterm/key.rs:108-200` covers:

- BackTab canonicalization and release dropping;
- all Crossterm modifiers;
- Enter versus Shift+Enter;
- newline/carriage-return/ordinary character preservation;
- ETX preservation.

#### Lowering

`termwiz/lower.rs:257-380` covers:

- physical color/style conversion;
- bold-over-dim normalization;
- italic, underline, reverse, and strikethrough;
- omission of wide-cell continuation cells;
- agreement between Iyon and Termwiz grapheme width;
- no implicit wrapping for a variety of Unicode graphemes and emoji sequences.

#### Presenter

`termwiz/presenter.rs` tests cover:

- empty and differential surfaces;
- full repaint behavior;
- native synchronized output;
- multiple native inserts sharing one sync region;
- sync termination after insert failure;
- sync termination after normal present failure;
- sync termination before resize geometry changes;
- idempotent sync completion;
- native scroll model equivalence;
- batches larger than the viewport;
- style reset before exposing newly scrolled rows;
- exact physical style preservation;
- wide-glyph and continuation-cell behavior;
- preservation of the model’s scrollback across full repaint.

Several tests use `RecordingTerminal` to capture exact `Change` sequences. Other tests use `ShadowTerminal`, which is designed not to reuse Iyon’s own surface composition or physical row placement logic.

#### Shadow terminal

`termwiz/shadow.rs` provides an independent interpreter that:

- tracks visible screen and native scrollback separately;
- simulates cursor movement and linefeed;
- understands CRLF scrollback creation;
- models wide graphemes and continuation cells;
- panics if a real render receives `ScrollRegionUp` or `ScrollRegionDown`;
- captures text and styled spans;
- supports failure injection.

This is particularly valuable because native scrollback correctness cannot be established from Iyon’s retained visible `Surface` alone.

### 7.2 Application fake backend

`application/tests.rs:182-328` defines a fake backend that can:

- delay presentation receipts;
- inject event, viewport, draw, and restoration errors;
- record submitted frame text;
- record viewport calls and sizes;
- record native rows;
- record final-position and restoration calls.

`application/tests/driver.rs` exercises the same `TerminalBackend` lifecycle with a test-only asynchronous input trait.

These tests validate host-side candidate commit and delayed-receipt behavior, but the fake backend does not model actual terminal bytes, native scrollback, partial writes, or resize-induced physical scrollback geometry.

### 7.3 Observability gaps

The following are not visible through production counters or public diagnostics in the assigned scope:

- bytes emitted by `termwiz::Terminal`;
- number of `Change` operations per frame;
- time spent lowering versus diffing versus flushing;
- terminal worker queue depth;
- input reader thread lifetime or poll latency;
- whether a real terminal partially applied a failed `render`;
- native scrollback length or geometry after resize;
- whether physical and logical cursor positions diverged.

The typed `TerminalWorkerStopped` error is a positive observability feature. Physical synchronization state is also tracked explicitly upstream through `physical_sync_unknown` and `History.native.synchronization_unknown`.

### 7.4 Validation status

No tests or commands were run for this investigation. The report only records source-defined tests and contracts.

---

## 8. Cross-boundary findings and contradictions

### 8.1 Complete prepared frame versus damage metadata

`PreparedSceneFrame` includes `DamageRegion`, but `TermwizBackend` and `TermwizPresenter` ignore it. The backend always receives and lowers a complete frame surface. This is a deliberate correctness-oriented design: the presenter’s retained complete-screen model is the source for diffing, and upstream damage is not trusted as sufficient to describe all inherited style/clear effects.

The consequence is that the physical backend boundary currently carries more data than the terminal emitter needs for a localized update, but the presenter’s own diff remains the authoritative output optimization.

### 8.2 History overlay reaches terminal lowering

`lower::desired_surface` directly accepts `HistoryPhysicalOverlay`. The backend therefore knows about a History-specific physical overlay even though the `TerminalBackend` trait itself only names `PreparedSceneFrame`.

This is generic History behavior rather than application coupling, but it means the terminal lowering contract is not entirely presentation-neutral. A future backend-independent frame representation would need to decide whether overlays are already composited into the physical surface or remain explicit.

### 8.3 `NativeHistorySink` exact-prefix contract versus partial physical writes

The trait documentation says `Err` means zero rows accepted. History’s implementation comments explicitly account for sinks that may have physically written some rows before returning an error:

- do not rewind accepted logical rows;
- mark synchronization unknown;
- do not claim the old physical screen remains intact.

Termwiz’s `insert_history` returns only `Ok(rows.len())` or `Err`, so it cannot communicate a physical partial prefix. The current source handles this by forcing a recovery frame rather than pretending the visible model is still synchronized. It does not provide a way to reconcile already-created native scrollback.

This is an important semantic distinction:

```text
logical History frontier acknowledgement
    !=
actual terminal byte-level write completion
    !=
ability to repair native scrollback
```

### 8.4 Current resize behavior versus LAY-1 research record

`LAY-1-main-screen-scrollback-and-resize.md` describes a research/reference model in which a main-screen renderer:

- retains the entire logical line buffer;
- clears native scrollback on resize;
- replays the entire document at the new width;
- rebuilds native scrollback consistently.

The current source does not implement that model. `TermwizPresenter::resize` only resizes the visible shadow `Surface`, marks it unknown, and full-repaints the desired visible frame. Existing terminal-native scrollback remains untouched. The presenter tests explicitly assert scrollback preservation across full repaint.

Thus the current source and LAY-1 research record disagree in behavior and design:

| Concern | Current source | LAY-1 research model |
|---|---|---|
| Source of truth | History units + retained physical rows + visible `Surface` shadow | Entire logical line array |
| Resize | Visible surface resize + full repaint | Clear screen and scrollback + full replay |
| Native scrollback | Preserved and not addressable | Deleted and regenerated |
| Width consistency | Previously transferred rows remain historical geometry | Entire replay regenerated at new width |
| Recovery after doubt | Visible full repaint, scrollback not repaired | Clear/replay establishes a new known state |

The LAY-1 document is a historical/research record, not current-source authority. Its proposed approach should not be treated as implemented.

### 8.5 Termwiz and Crossterm are complementary, not parallel output paths

Crossterm is used for:

- raw mode;
- bracketed paste;
- event polling/reading;
- event/key decoding;
- mode restoration.

Termwiz is used for:

- capability probing;
- opening the system terminal;
- output rendering and flushing;
- cursor visibility and movement;
- surface-level terminal changes.

There is no Crossterm output fallback. Replacing or deleting either dependency would affect a distinct part of the current session protocol.

### 8.6 No alternate-screen behavior

The worker defines `Terminal` support through Termwiz but does not call `enter_alternate_screen` or `exit_alternate_screen`. The actual current path is a main-screen inline viewport. The initial `"\r\n".repeat(size.rows)` and native-history CRLF protocol are consistent with that mode.

This is not merely an implementation detail: native scrollback is a first-class output side effect in the current backend.

### 8.7 Generic boundary remains private

All terminal/backend symbols in the assigned scope are `pub(crate)`. No public Rust terminal authoring API is exposed from these files. The TypeScript/native public boundary is elsewhere; this layer is consumed by the native host and test harness only.

---

## 9. Open questions and coverage gaps

1. **Partial render semantics:** Does the concrete Termwiz backend guarantee that `Terminal::render` either writes zero bytes or writes all changes before returning an error? The source treats partial physical writes as possible but does not prove the concrete guarantee.
2. **Native scrollback duplicate risk:** After a partial native-history write, the presenter marks itself unknown and History marks synchronization unknown. Recovery repaints the visible screen but leaves native scrollback untouched. If the row was partially or fully emitted before the error, what prevents a later retry from duplicating it?
3. **Resize source freshness:** The backend trusts queued Crossterm resize events and cached `Size`; it does not query `get_screen_size` again during `viewport()`. What happens if a resize event is lost, delayed, or coalesced by the platform?
4. **Resize and terminal-native scrollback:** Existing rows above the visible viewport are not represented by `TermwizPresenter.presented`. What is the intended product contract for already-transferred rows after width changes?
5. **Initial setup conversion cleanup:** The `usize -> u16` conversion occurs after terminal setup and initial painting but is not wrapped in `setup_error`; should this path restore terminal modes if conversion fails?
6. **Restoration retry:** `TermwizBackend::restore` sets `restored = true` before attempting operations. Is one-shot restoration failure acceptable, or should a later Drop/retry be permitted?
7. **Worker command ordering on shutdown:** Explicit `Restore` breaks the worker command loop, after which `run` performs another best-effort cleanup pass. Is the duplicate cleanup intentional and tested against real terminals?
8. **Mouse/focus scope:** Focus and mouse events are silently ignored. Is that the complete generic framework contract, or is pointer/focus support intentionally deferred to another assignment?
9. **Terminal width invariants:** `row_changes` validates wide-cell geometry but does not itself enforce that a row’s occupied width is no greater than the destination terminal width. Which upstream invariant guarantees this for every native row?
10. **Damage use:** Why is `PreparedSceneFrame.damage` carried into the backend boundary if Termwiz performs its own complete-surface diff? Is it reserved for a future lowerer or intentionally ignored for correctness?
11. **Output memory pressure:** `desired_surface` and `native_transaction` allocate owned vectors/sequences per operation. There are no backend counters or caps in this scope for very large frames or native-history batches.
12. **Test versus real terminal behavior:** The ShadowTerminal is a strong independent model, but it cannot validate terminal-specific quirks such as scrollback rewrapping policy, synchronized-output support, or partial write behavior.
13. **Cross-platform sync output:** Synchronized output is compiled only on Unix. The non-Unix path omits DEC 2026 markers and does not expose an equivalent capability check.
14. **Termwiz capability fallback:** `Capabilities::new_with_hints` and `new_terminal` are the only production capability path. There is no visible fallback if capability probing or terminal creation fails.

These gaps are architectural unknowns, not claims that the current implementation is unused or incorrect.

---

## 10. Evidence appendix

### 10.1 Exhaustive assigned-file manifest

#### Backend

- `crates/iyon-tui/src/backend/mod.rs`
  - `NativeHistorySink` re-export
- `crates/iyon-tui/src/backend/native_history.rs`
  - `NativeHistorySink`
  - acknowledged-prefix contract

#### Terminal root/protocol

- `crates/iyon-tui/src/terminal/mod.rs`
  - module declarations and private re-exports
- `crates/iyon-tui/src/terminal/backend.rs`
  - `PresentReceipt`
  - `TerminalWorkerStopped`
  - `terminal_worker_stopped`
  - `is_terminal_worker_stopped`
  - `TerminalEvent`
  - `TerminalBackend`

#### Crossterm

- `crates/iyon-tui/src/terminal/crossterm/mod.rs`
  - `map_event`
  - `setup`
  - `restore`
  - `EventReader`
  - `read_events`
- `crates/iyon-tui/src/terminal/crossterm/key.rs`
  - `key_stroke`
  - `modifiers`
  - `media_key`
  - `modifier_key`
  - key mapping tests

#### Termwiz

- `crates/iyon-tui/src/terminal/termwiz/mod.rs`
  - private module aggregation
- `crates/iyon-tui/src/terminal/termwiz/backend.rs`
  - `TermwizBackend`
  - `enter`
  - `from_startup`
  - `send`
  - `map_event`
  - `NativeHistorySink` implementation
  - `TerminalBackend` implementation
  - `Drop`
- `crates/iyon-tui/src/terminal/termwiz/lower.rs`
  - `desired_surface`
  - `surface_changes_with_overlay`
  - `direct_row_changes`
  - `effective_cell`
  - `row_changes`
  - `physical_style`
  - color and width helpers
  - lower/style/wide-cell tests
- `crates/iyon-tui/src/terminal/termwiz/presenter.rs`
  - `TermwizPresenter`
  - `resize`
  - `finish_sync_output_best_effort`
  - `present`
  - `insert_history`
  - `position_after_final_frame`
  - `abort_sync_output_best_effort`
  - `apply`
  - `native_transaction`
  - `apply_native_scroll_model`
  - `full_repaint_changes`
  - `canonical_terminal_state`
  - recording-terminal, model, sync, resize, style, and native-scroll tests
- `crates/iyon-tui/src/terminal/termwiz/shadow.rs`
  - `ShadowCell`
  - `ShadowRow`
  - `ShadowScrollbackCommit`
  - `CapturedSpan`
  - `CapturedLine`
  - `CapturedFrame`
  - `ShadowTerminal`
  - independent change interpreter and scrollback oracle
- `crates/iyon-tui/src/terminal/termwiz/worker.rs`
  - `TerminalCommand`
  - `Startup`
  - `run`
  - `setup_terminal`
  - `setup_error`
  - `handle_command`
  - `restore_terminal`

### 10.2 Supporting cross-boundary symbols

- `crates/iyon-tui/src/application/host.rs`
  - `HostBackend`
  - `HostInner`
  - `frame`, `candidate_frame`, `presentation`, `frame_pending`
  - `physical_sync_unknown`
  - `begin_frame`
  - `present_frame`
  - `commit_frame`
  - `position_after_final_frame`
  - event dispatch
- `crates/iyon-tui/src/application/run.rs`
  - `wait_for_present_blocking`
- `crates/iyon-tui/src/scene/host.rs`
  - `PreparedSceneFrame`
  - `SceneHost::prepare`
  - native-history pressure and synchronization-unknown handling
- `crates/iyon-tui/src/history/native/mod.rs`
  - `NativeTransferStatus`
  - `NativeTransferOutcome`
  - `NativeTransferError`
  - `transfer_native_prefix_with_theme_and_content`
  - `insert_prefix`
  - partial/error acknowledgment handling
- `crates/iyon-tui/src/history/model.rs`
  - `native_synchronization_unknown`
  - `recover_native_synchronization`
- `crates/iyon-tui/src/history/native/frontier.rs`
  - `synchronization_unknown`
  - `mark_synchronization_unknown`
  - `recover_synchronization`
- `crates/iyon-tui/src/application/tests.rs`
  - `FakeBackend`
  - delayed receipts and failure injection
- `crates/iyon-tui/src/application/tests/driver.rs`
  - test terminal session protocol

### 10.3 Key source locations

- Backend seam: `backend/native_history.rs:5-15`
- Terminal protocol: `terminal/backend.rs:6-49`
- Crossterm mapping/setup: `terminal/crossterm/mod.rs:21-118`
- Key mapping: `terminal/crossterm/key.rs:7-106`
- Termwiz backend lifecycle: `terminal/termwiz/backend.rs:20-166`
- Frame lowering: `terminal/termwiz/lower.rs:15-255`
- Presenter state/output: `terminal/termwiz/presenter.rs:12-282`
- Native transaction model: `terminal/termwiz/presenter.rs:184-236`
- Independent shadow model: `terminal/termwiz/shadow.rs:162-526`
- Worker setup/dispatch/restoration: `terminal/termwiz/worker.rs:35-163`
- Host candidate/receipt state: `application/host.rs:122-173`
- Host frame submission: `application/host.rs:1870-2023`
- Host commit: `application/host.rs:2026-2113`
- Scene preparation synchronization guard: `scene/host.rs:1051-1128`
- Native sink error handling: `history/native/mod.rs:80-132`
- Fake backend: `application/tests.rs:182-328`

### 10.4 Historical/research source pointers

- `LAY-1-main-screen-scrollback-and-resize.md`
  - documents a different whole-line-buffer/main-screen replay model;
  - explicitly contrasts that model with the current retained `Surface` + native scrollback approach;
  - current source behavior remains the authority for this atlas.
- `docs/history/PRE-V5/PRE-V5-RUST-LOWERING-HANDOFF.md`
  - describes receipt-driven native scrollback and preserving terminal correctness as current responsibilities;
  - its future-oriented deletion/adaptation statements are historical planning context, not current implementation evidence.
- `docs/history/PERF-13/PERF-13-F-implementation-notes.md`
  - records candidate frame, backend receipt, and synchronization boundaries that match the current host/backend source.

### 10.5 LOC methodology

Counts are approximate physical line counts from the inspected source ranges:

- production counts exclude clearly delimited `#[cfg(test)] mod tests` bodies where practical;
- test counts include test modules and test-only shadow support;
- generated code was not present in the assigned scope;
- no commands were executed, so counts should not be treated as exact compiler or `wc -l` output.