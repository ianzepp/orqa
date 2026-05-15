# Phase 3 Delivery Spec — TUI Timeline UI + Filters

**Factory Run:** TUI Operator Cockpit  
**Phase:** 3 of 6 (Timeline UI + Filters)  
**Date:** 2026-05-16  
**Status:** Ready for implementation (spec persisted)  
**Depends On:** Phase 2 (Event Model & Watching) — delivered

---

## 1. Objective

Replace the Phase 1/2 text skeleton with a real, usable **scrollable unified timeline** in the Ratatui Operator Cockpit.

The TUI should now feel like a proper monitoring view:

- A live, growing list of events coming from the `PodWatcher` (Phase 2).
- Support for the key filters described in the design doc (`f` for fin, `o` for operator-mail, thread/subject filter).
- Smooth keyboard navigation (scrolling, follow mode).
- Clean header showing current pod, active filters, and status.
- The bottom area remains a placeholder for the composer (Phase 4), but the overall layout should feel complete for monitoring.

This phase delivers the **visual and interaction layer** for the "one flowing timeline" experience. The heavy lifting of event production is already done.

---

## 2. Scope (In)

- **Layout overhaul in `src/tui/run.rs`** (or a new `app.rs` / `timeline.rs`):
  - Header bar: `orqa • <pod-slug> (<short root>)` + loop status (if easy) + active filter badges + event count.
  - Main area: scrollable list of events.
  - Bottom status line: current target fin (placeholder), help hint, "follow" indicator.
- **Event list rendering**:
  - Use `ratatui::widgets::List` + `ListState` (or a custom `Paragraph` + manual wrapping/scroll for more control).
  - Each `Event` rendered as one or more lines with color coding:
    - Log lines: dim for stdout, red for stderr, blue for events.jsonl.
    - Mail: yellow/magenta highlight, show `fin: from → subject` or path.
    - Run start/finish and lock events: distinct style (green for success, etc.).
    - Operator actions: bold or reverse video.
  - Truncate very long lines reasonably.
- **Filter system** (core of this phase):
  - Maintain `FilterState` struct:
    - `fin_filter: Option<String>` (show only this fin)
    - `only_operator_mail: bool`
    - `thread_filter: Option<String>` (substring match on subject or recent context)
  - Hotkeys (matching the design):
    - `f` / `F`: Open simple fin picker (cycle through available fins or show a small popup/list).
    - `o` / `O`: Toggle "only operator mail" (events involving `operator@` or `OperatorAction`).
    - `t` or `/`: Prompt for thread/subject filter (simple text input for now, or just a toggle + last subject).
  - Filtering must be efficient (pre-filter the event buffer on every render or on filter change).
- **Scrolling & Follow mode**:
  - Arrow keys / PageUp / PageDown / Home / End for manual scroll.
  - `f` (lowercase in some contexts) or auto-follow: when at bottom, new events keep the view pinned.
  - "Follow" indicator in the UI.
  - Pause follow on manual scroll up; resume when user scrolls to bottom or presses a "resume follow" key.
- **Event buffer management**:
  - Keep a bounded ring buffer of recent `Event`s in the TUI app state (e.g. last 2000 events).
  - On every draw tick, call `watcher.poll()` and append new events.
  - Apply current filters when rendering.
- **Keyboard handling**:
  - All previous Phase 1 keys (`q`, `Esc`, Ctrl-C) continue to work.
  - New filter and scroll keys as described.
  - `?` can open a minimal help overlay (stretch, nice-to-have).
- **Integration with existing code**:
  - `run_tui` now takes ownership of a `PodWatcher` (created in Phase 2 style).
  - The `Event` type from `events.rs` is the single source of data.
  - Use `PodRegistration` only for header display (slug + root).
- Keep terminal restore guarantees on all exit paths (including filter input mode).
- All changes must pass `cargo fmt`, strict clippy, and not break existing tests.

---

## 3. Scope (Out — Explicitly Deferred)

- The composer / input box at the bottom (Phase 4).
- Sending mail or waking fins from the TUI.
- Reading individual mail (click/Enter on a mail event to open full view) — Phase 5.
- Advanced thread grouping or conversation view.
- Theming / colors beyond basic `Style` usage (we'll use ratatui's default + a few `Color` choices).
- Saving filter state between runs.
- `pod.toml` `[operator]` config.
- Any changes to mail routing for local `operator@`.
- Full help screen or command palette.

---

## 4. Technical Approach & Constraints

- **State management**: Introduce a simple `AppState` struct inside the TUI module that owns:
  - The `PodWatcher`
  - `Vec<Event>` (the live buffer)
  - Current `FilterState`
  - Scroll offset / `ListState`
  - Follow mode flag
  - List of known fins (for the `f` picker)

- **Rendering strategy**:
  - Preferred: `ratatui::widgets::List` with a custom `ListItem` generator that turns `Event` into styled text.
  - Alternative (if wrapping is painful): A `Paragraph` that builds a big `Text` from the filtered events and manages vertical scroll manually. `List` is usually better for this.

- **Filter application**:
  - On every render (or on filter change), build a filtered view: `Vec<&Event>` or indices.
  - Keep it cheap — events are small.

- **Hotkey implementation**:
  - `f` can be a simple cycle: "All" → first fin → next fin → ...
  - Or open a small centered list of fins (using a `Clear` + `List` popup pattern — common in ratatui examples).
  - For Phase 3, a cycling approach or a very simple inline status is acceptable. A nice popup is better UX.

- **Event loop tick**:
  - Keep the existing 200-300ms poll for keyboard.
  - Also poll the watcher on every tick (or every other tick) to keep the timeline fresh.
  - Redraw when new events arrive or user interacts.

- **Performance**:
  - The ring buffer should be bounded (e.g. 5000 events max) to avoid memory growth.
  - Filtering and rendering should stay responsive even with hundreds of events.

- **Module organization** (recommended):
  - Keep `run.rs` as the high-level entry + event loop.
  - Extract `Timeline` or `App` logic into `src/tui/timeline.rs` or `src/tui/app.rs` for cleanliness.
  - This will make Phase 4 (adding the composer) much easier.

---

## 5. Files Expected to Change / Be Created

- `src/tui/run.rs` — major rewrite of the event loop and drawing (or split out).
- `src/tui/timeline.rs` (new, recommended) — `TimelineView` struct, filter logic, rendering helpers.
- `src/tui/app.rs` (new, optional) — top-level `App` that owns watcher + timeline state.
- `src/tui/mod.rs` — export new types.
- Possibly small additions to `events.rs` (e.g. helper methods like `is_operator_related()`, `matches_thread()`).
- `docs/tui-factory-ledger.md` — update Phase 3 progress.
- This delivery spec.

No new Cargo dependencies expected.

---

## 6. Verification & Quality Gates for This Phase

**Must pass before commit:**

1. `cargo build`, `cargo test --locked`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` all clean.
2. Manual experience (the important one):
   - `orqa init` a fresh pod.
   - Create 2–3 fins.
   - Run some `fin exec` and send some mail (including to `operator@`).
   - Run bare `orqa` inside the pod.
   - You should see a live, scrollable timeline of log lines, mail arrivals, run events.
   - Press `o` — only operator-related events remain.
   - Press `f` multiple times — timeline filters to one fin then back to all.
   - Scroll up with arrows → follow pauses.
   - Scroll back to bottom or wait → new events appear and view follows again.
   - `q` exits cleanly, terminal restored.
3. The UI must remain usable with 100+ events in the buffer.
4. No visual glitches on terminal resize.
5. All filter hotkeys documented in the code (comments or a small `?` overlay if implemented).

**Poker-face questions:**

- Does the timeline ever show events from a fin that shouldn't be visible under the current filter?
- Is follow mode intuitive and not frustrating?
- Does the UI feel "alive" when fins are producing output?
- Are we still respecting the Phase 1 safety guarantees (only runs inside detected pods, never creates pods)?

---

## 7. Risks & Mitigations

- **Risk**: Building a good scrollable + filterable list in Ratatui takes more time than expected (wrapping, performance, state management).
  **Mitigation**: Start simple (fixed-width lines + manual scroll with `Paragraph`), then upgrade to `List` + `ListState` if time allows. Prioritize correctness and filter behavior over perfect text wrapping in Phase 3.

- **Risk**: Frequent redraws + watcher polling cause high CPU or flicker.
  **Mitigation**: Only redraw when there are new events or user input. Use a reasonable tick rate (250ms). Ratatui is efficient.

- **Risk**: Filter state gets out of sync with the event buffer.
  **Mitigation**: Always derive the visible list from the master buffer + current filters on every draw. No caching of filtered results across frames unless proven necessary.

---

## 8. Definition of Done for Phase 3

- A beautiful, functional scrollable timeline is the main view when you run `orqa` inside a pod.
- The three primary filters (`f`, `o`, thread) work as described in the operator cockpit design doc.
- Keyboard navigation (scroll + follow) feels natural.
- The code is well-organized for the remaining phases (composer in Phase 4, mail actions in Phase 5).
- All quality gates passed.
- Clean commit referencing "TUI Phase 3".
- Ledger updated.
- Factory ready for Phase 4 only after poker-face sign-off.

---

**Persisted by:** Factory  
**Next action:** Implement Phase 3 (Timeline UI + Filters), verify, poker-face, commit.