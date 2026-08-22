# LAY-1 — Main-screen scrollback rendering and resize handling

**Status:** research record; reference implementation for a future iyon-tui main-screen mode  
**Reference:** pi coding agent (`@earendil-works/pi-coding-agent`), package `packages/tui`, primary source `tui-main-screen.ts` (class `TuiMainScreen`), support in `tui.ts` (`TuiBase`), `terminal.ts` (`ProcessTerminal`). Local clone used: `/tmp/pi`.  
**Purpose:** document exactly how pi achieves correct rendering of an owned active area plus native terminal scrollback — including resize — without the alternate screen buffer, so the mechanism can be evaluated and adopted for iyon-tui.

---

## 0. The problem

Iyon's current termwiz presenter keeps a shadow `Surface` of the visible screen and pushes frozen `PhysicalRow`s into native scrollback by emitting ordinary full-screen CRLF from the bottom row. This assumes the application knows what the terminal displays. That assumption breaks on terminal resize:

```text
1. Native scrollback is not addressable. No escape sequence can repaint rows
   that have scrolled above the viewport. After a width change those rows are
   rewrapped by the terminal at the new width (or not, depending on terminal),
   while rows Iyon transfers afterwards are wrapped at the new width. The
   scrollback becomes a mixture of two geometries.
2. The shadow model never covered scrollback, only the visible screen. After a
   resize nothing describes what is actually above the viewport anymore.
3. The History projection's cached geometry, anchors, and frozen-vs-live split
   straddle the resize boundary, so even the repaired visible frame is
   inconsistent with what surrounds it.
```

Result: resizing breaks both the active area and the scrollback, in different ways. Pi demonstrates this is avoidable.

## 1. Pi's core model: own the document, rent the screen

Pi does **not** maintain a screen-sized cell grid as its source of truth. It maintains:

```ts
previousLines: string[]   // the ENTIRE logical transcript as styled terminal lines
```

Every frame, components render to a fresh `newLines: string[]` covering all content ever produced (the chat transcript plus the live editor/footer), not just one screenful. The relationship between that array and the physical terminal is:

```text
terminal screen  :=  the last `height` entries of the line array
native scrollback := everything above, produced implicitly by ordinary
                     line-feed scrolling when the array tail overflows
                     the screen bottom
```

Two scalar trackers relate the array to hardware reality:

```text
cursorRow          logical index of the last content line (end of document)
hardwareCursorRow  the actual terminal cursor row within the whole buffer,
                   tracked across every write
viewportTop        how many lines of the array have already scrolled off the
                   top into native scrollback:
                       previousViewportTop = max(0, bufferLength - height)
```

Nothing claims to know what is *inside* native scrollback. The renderer only knows its own array and where the viewport boundary sits. All cursor movement is relative (`ESC[{n}A/B`) computed from `computeLineDiff(targetRow) = targetScreenRow - currentScreenRow`; there is no absolute addressing into unknown territory.

## 2. The render decision tree (`doRender`)

Each frame:

```text
width  = terminal.columns        // from process.stdout, refreshed on SIGWINCH
height = terminal.rows
newLines = renderComponents(width)            // full transcript, new width
newLines = compositeOverlays(newLines)         // modals composited into lines
cursorPos = extractCursorPosition(newLines)    // find CURSOR_MARKER, strip it
newLines = applyLineResets(newLines)           // append SGR reset per line
```

Then, in order:

### 2.1 First render

If `previousLines.length === 0 && !widthChanged && !heightChanged`: emit all lines with plain `\r\n` flow, **without clearing** (`fullRender(false)`). Assumes a clean screen at startup.

### 2.2 Dimension changes → total invalidation

```ts
if (widthChanged)  { fullRender(true); return; }
if (heightChanged && !isTermuxSession()) { fullRender(true); return; }
```

Width change forces replay because **line wrapping changed for every line**. Height change forces replay to keep the visible viewport aligned with the bottom of the buffer. Exception: Termux fires fake height changes when the software keyboard toggles; there, height-only changes skip the replay to avoid replaying the entire history on every keyboard toggle.

### 2.3 Optional clear-on-shrink

If enabled and `newLines.length < maxLinesRendered` (content shrank below its historical high-water mark) and no overlays are active: `fullRender(true)`. Otherwise stale blank rows would linger. `maxLinesRendered` tracks growth but resets whenever a clearing replay happens. (A past bug note in the code: using the high-water mark when padding for overlays caused self-reinforcing inflation that pushed content into scrollback on widen.)

### 2.4 Differential update (common path)

Compute `firstChanged`/`lastChanged` by comparing old vs new arrays element-wise (plus appended-lines handling). Then a series of provability guards, each escalating to `fullRender(true)` when the differential assumption cannot be guaranteed:

```text
firstChanged < prevViewportTop      → change touches area already in native
                                      scrollback which cannot be repainted →
                                      full replay
deletion moves viewport up          → full replay
extraLines > height                 → clear range too large to trust → full
                                      replay
```

Otherwise emit one synchronized buffer: move cursor relatively to `firstChanged`, then for each changed line `\r ESC[2K <line>` with `\r\n` between lines. Only the changed range is rewritten — not everything to screen bottom — which keeps spinner-tick frames minimal. Appended lines use `appendStart` to open exactly one newline gap before writing.

Deletion-only updates move the cursor to the end of surviving content, clear the surplus rows with `\x1b[2K` walking downward, and step back up — no scrolling triggered.

### 2.5 Scroll handling inside the common path

When the write target lies below the current viewport bottom, pi first walks the cursor to the screen bottom with relative moves and emits the required number of bare `\r\n` to make the terminal scroll naturally until the target row is on-screen, updating `prevViewportTop`/`viewportTop` bookkeeping to match. Content thus crosses into native scrollback exclusively through ordinary CRLF overflow — never via scroll-region commands.

## 3. The resize algorithm itself (`fullRender(true)`)

This is the answer to "how does pi survive resizing both the active area and scrollback":

```text
buffer = "\x1b[?2026h"          // DECSET 2026: begin synchronized output
buffer += "\x1b[2J"             // ED 2:     clear entire screen
buffer += "\x1b[H"              // CUP home: cursor to 1,1
buffer += "\x1b[3J"             // ED 3:     CLEAR SCROLLBACK  <-- the key
for each line in newLines:      // replay ENTIRE document at NEW width
    buffer += "\r\n"            //   ordinary newline flow
    buffer += line              //   pre-wrapped by pi's renderer at new width
buffer += "\x1b[?2026l"         // end synchronized output
```

Mechanics, precisely:

1. **`\x1b[3J` erases native scrollback.** Whatever geometry mess existed above the viewport is deleted rather than reconciled. There is no repair path because none is needed afterward.
2. **The replay re-wraps everything.** Pi's component renderer wraps every line at the *current* width (it throws a hard error if any rendered line exceeds `visibleWidth > width`, dumping a crash log first). Because wrapping happened inside pi at the new geometry, the replayed stream is internally consistent end to end.
3. **Ordinary CRLF repopulates scrollback correctly.** The document is longer than the screen, the cursor starts at home after the clear, and each `\r\n` at the screen bottom scrolls one row up into fresh, correctly-wrapped scrollback. By the end of the replay the screen shows the last `height` lines and the scrollback contains the rest, all wrapped identically.
4. **State is rebuilt from scratch**, not patched:
   ```text
   cursorRow = hardwareCursorRow = newLines.length - 1
   maxLinesRendered = newLines.length            // reset, since we cleared
   previousViewportTop = max(0, max(height, len) - height)
   previousLines = newLines; previousWidth = width; previousHeight = height
   ```
5. **Synchronized output** (`DEC 2026`, guarded by unix cfg) makes terminals present the whole clear+replay atomically, so users see one clean frame instead of a flicker storm.
6. Kitty image protocol graphics are explicitly deleted (`deleteKittyImages`) before the clear, since pixel data would otherwise survive `\x1b[3J`.

Costs accepted by this design:

```text
- the user's scroll position in native scrollback is destroyed on every
  resize (matches Claude Code / Codex CLI behavior)
- replay cost is O(document length) per resize; fine for agent transcripts,
  worth a cap if transcripts grow very large
- one large write burst per resize, mitigated by synchronized output
```

## 4. Supporting mechanisms that keep the invariant trustworthy

These are what let pi restrict full replays to genuine invalidation events instead of constantly second-guessing the shadow state:

1. **Line-array diffability.** Lines are plain strings with embedded SGR; `applyLineResets` appends a reset segment to every line so styling can never bleed across line boundaries or into the clear operations. Diffing is exact string comparison — no style-state carryover ambiguity.
2. **Cursor marker extraction.** Components embed a `CURSOR_MARKER`; before diffing, pi locates it in the viewport region only, converts marker offset to a visual column via `visibleWidth`, strips it, and later positions the real hardware cursor there with relative row movement + absolute column (`ESC[{n}G`). The hardware cursor is therefore always a *derived*, re-established quantity — never trusted across frames.
3. **Relative-movement discipline.** Every cursor motion derives from `hardwareCursorRow`, which is updated synchronously with every emitted buffer. Combined with guard 2.4's escalation rules, the renderer can assert "my model of the cursor is exact" at all times; if it cannot, it replays.
4. **Resize event plumbing.** `ProcessTerminal.start` subscribes `process.stdout.on("resize", ...)` → `requestRender()`; dimensions are read live from `process.stdout.columns/rows`. A synthetic `SIGWINCH` is self-delivered at startup to refresh possibly-stale sizes after suspend/resume. Resize handling is therefore just "next frame notices different dimensions" — no special resize code path outside the decision tree.
5. **Frame scheduling.** `requestRender` coalesces via `process.nextTick` + a min-interval timer (~16 ms); forced renders bypass throttling. Resize storms thus collapse into one replay.
6. **Mode duality.** `TuiAltScreen` exists for fullscreen mode with its own search/UI machinery; the coding agent defaults to `TuiMainScreen` and can hot-switch between them (`switchTuiMode`), transferring components, focus, and (main→main) the captured `TuiMainScreenRenderState`. The main-screen renderer is a peer mode, not a fallback hack.

## 5. Why iyon currently diverges, mapped to code

```text
pi                                     iyon-tui today
-------------------------------------  -----------------------------------------
source of truth: line array covering   source of truth: History units +
whole document                         retained PhysicalRows + presented Surface
                                       shadow of the VISIBLE screen only
scrollback written by ordinary CRLF    same primitive (native_transaction:
overflow, ownership never claimed       CRLF repeat from bottom row), BUT rows
                                        are frozen artifacts wrapped at their
                                        original width
resize = invalidate everything +       resize = Surface.resize + known=false +
clear scrollback + replay               full_repaint_changes of the desired
                                       FRAME; scrollback untouched and now
                                       inconsistent; History cached geometry
                                       straddles the boundary
diff allowed only when provably safe   diff/present runs against a shadow whose
                                       assumptions resize silently broke
```

The deepest difference is philosophical: **pi treats the terminal as a dumb line printer it fully owns and re-establishes on doubt; iyon treats the terminal as stateful memory it incrementally maintains.** The second approach is faster in steady state but cannot survive geometry invalidation of memory it does not control (scrollback). Pi's design survives because it never pretends to control scrollback — it only ever appends into it, and on doubt deletes and refills it.

## 6. Adoption sketch for iyon-tui (not a handoff; evaluation only)

Pi's mechanism is compatible with Iyon's architecture because Iyon also owns semantic content sufficient to replay:

1. Add a main-screen presentation mode alongside the alt-screen path (the framework-boundary rule in AGENTS.md is satisfied: a main-screen transcript mode is generic TUI capability).
2. On `TerminalEvent::Resize`: mark the presenter unknown, clear screen+scrollback (`\x1b[2J\x1b[H\x1b[3J`, synchronized), re-project History from retained units at the new width (units retain semantic content, so re-wrapping is possible), and replay as one transaction. Frozen `PhysicalRow`s must be regenerated from source at the new width, not reused.
3. Keep the differential fast path for non-resize frames, including pi's escalation guards (any change above the known viewport top ⇒ replay).
4. Accept the trade-offs deliberately: user scroll position resets on resize; O(transcript) replay cost; consider a replay cap that falls back to truncating oldest content beyond a budget.

## 7. Source pointers

```text
packages/tui/src/tui-main-screen.ts   TuiMainScreen.doRender, fullRender,
                                      diff guards, scroll handling,
                                      positionHardwareCursor
packages/tui/src/tui.ts               TuiBase: requestRender/renderNow throttle,
                                      applyLineResets, extractCursorPosition,
                                      overlay compositing, working-area padding
packages/tui/src/terminal.ts          ProcessTerminal: resize subscription,
                                      SIGWINCH refresh, columns/rows sources
packages/coding-agent/src/modes/
  interactive/interactive-mode.ts     createInteractiveTui mode selection,
                                      switchTuiMode + render-state transfer
```

Key escape sequences relied upon: `DECSET 2026` / `DECRST 2026` (synchronized output), `ED 2` (clear screen), `ED 3` (clear scrollback), `CUP` (home), `CUU/CUD` (relative row movement), `CHA` (absolute column), `EL 2` (clear line), plain `CR LF` (scrollback-producing overflow).
