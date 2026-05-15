# Phase 2 Delivery Spec — TUI Event Model & Watching

**Factory Run:** TUI Operator Cockpit  
**Phase:** 2 of 6 (Event Model & Watching)  
**Date:** 2026-05-16  
**Status:** Ready for implementation (spec persisted)  
**Depends On:** Phase 1 (Foundations & Safe Entry Point) — delivered and committed

---

## 1. Objective

Build the **backend event system** that will power the live unified timeline in the Operator Cockpit TUI.

In this phase we create:

- A clean, unified `Event` type (or small family of types) that represents everything that can appear in the activity timeline.
- A reliable watcher / producer that, given a `PodRegistration`, continuously monitors the pod's fins and produces a stream of events for:
  - Appends to the latest run logs (`stdout.log`, `stderr.log`, `events.jsonl`)
  - New mail arriving in any fin's `mail/new/`
  - Task arrivals / state changes (stretch)
  - Run start / finish / lock state changes
- The ability for the TUI (in later phases) to consume this event stream efficiently while the app is running.

**This phase does NOT** render the timeline in the UI, add the composer, or perform any wake/send actions. It delivers the data layer only. The Phase 1 skeleton TUI continues to be the visible surface (we can enhance the skeleton slightly to demonstrate the new watcher if it helps verification).

The goal is a solid, testable event pipeline using only the Phase 05 new-style paths (`PodRegistration` + `*_data_home`).

---

## 2. Scope (In)

- Define a clear `Event` enum in `src/tui/events.rs` (or equivalent) covering at minimum:
  - `LogLine { fin: String, stream: LogStream (Stdout | Stderr | Event), line: String, timestamp: Option<SystemTime> }`
  - `MailArrived { fin: String, message_path: PathBuf, from: Option<String>, to: Option<String>, subject: Option<String> }`
  - `RunStarted { fin: String, run_id: String }`
  - `RunFinished { fin: String, run_id: String, exit_code: Option<i32> }`
  - `LockAcquired { fin: String }` / `LockReleased { fin: String }`
  - `OperatorNote { text: String }` (for synthetic events the TUI itself will emit in Phase 4)
- Implement a `PodWatcher` (or `EventSource`) struct that:
  - Takes a `PodRegistration` (the single source of truth for the pod root).
  - On construction or `start()`, discovers all fins under `.orqa/fins/`.
  - For each fin, tracks the "current latest run" by reading the `latest-run` pointer file.
  - Maintains file offset cursors for the three log files of the current latest run.
  - Polls (or watches) `mail/new/` directories for new files (using `sorted_files` + mtime or simple readdir delta is acceptable for Phase 2).
  - Produces `Event` items in roughly chronological order (best effort; perfect global ordering is not required yet).
- The watcher must be usable from the TUI event loop (Phase 1 style sync loop is fine; we can use a background thread + `mpsc::channel` or a simple `poll_events(&mut self) -> Vec<Event>` API).
- All path construction must go through `PodRegistration` + the `*_data_home` family on `Orqa` (or equivalent methods). No hard-coded `.orqa` strings in the watcher.
- Handle the case where a fin has no runs yet (graceful "no latest run" state).
- Handle "latest-run" pointer changing to a new run id (switch cursors cleanly, emit a `RunStarted` if appropriate).
- Basic deduplication / "only new data since last poll".
- The watcher should be restartable / reusable across TUI sessions.
- Unit tests for the event types and the watcher logic (using temp directories with synthetic run logs and mail files).
- Keep compile / clippy / fmt clean. Existing tests must continue to pass.
- Small enhancement to the Phase 1 skeleton (optional but recommended for verification): if the watcher is easy to integrate, show a live count of "events captured so far" or dump recent events when the user presses a debug key. This is **not** the full timeline UI.

---

## 3. Scope (Out — Explicitly Deferred)

- Any Ratatui widget or rendering of the timeline (that's Phase 3).
- The composer, target fin selection, or any `send_mail` + wake logic (Phase 4).
- Reading / acting on individual mail items from the TUI (Phase 5).
- Efficient filesystem notification (`notify` crate) — polling with a reasonable interval (200-500ms) is acceptable and simpler for Phase 2.
- Perfect time-ordering across all fins (we can sort or merge later).
- Support for legacy (non-`.orqa`) pods in the watcher (we only support detected Phase 05 pods).
- `[operator]` section parsing from `pod.toml`.
- Changes to mail address resolution for local `operator@`.
- Documentation / template updates (Phase 6).
- Interactive TUI tests that drive the full event loop.

---

## 4. Technical Approach & Constraints

- **Event type location**: New file `src/tui/events.rs`. Keep it free of Ratatui and heavy dependencies. It can depend on `std::path` and `serde` if we want cheap serialization later, but start plain.
- **Watcher API sketch** (to be refined in code):
  ```rust
  pub struct PodWatcher {
      reg: PodRegistration,
      fins: Vec<String>,
      // per-fin state: current_run_id, cursors for the three log files, seen mail ids, etc.
  }

  impl PodWatcher {
      pub fn new(orqa: &Orqa, reg: PodRegistration) -> Result<Self, String>;
      pub fn poll(&mut self) -> Result<Vec<Event>, String>;   // returns new events since last call
      pub fn refresh_fins(&mut self) -> Result<(), String>;   // for when fins are added dynamically
  }
  ```
- **Log tailing**: Reuse / adapt patterns from the existing `runs.rs` (`tail_paths`, `last_lines`, offset tracking). The watcher will be more sophisticated because it must track "current latest run per fin" and switch when the pointer file changes.
- **Mail watching**: Use `mailbox::sorted_files` + `unread_count` or direct readdir of `mail/new`. For Phase 2 we can treat each new filename as a `MailArrived` event (we can enrich subject/from later by reading the file header).
- **Latest-run handling**: Read `<fin>/latest-run` on every poll (or on change detection). When it changes, reset the three log cursors for that fin and emit appropriate events.
- **Threading model for Phase 2**: The simplest correct approach is:
  - TUI main thread owns the Ratatui terminal.
  - A background thread owns the `PodWatcher` and sends `Event` batches over an `mpsc::channel`.
  - The main loop does `terminal.draw(...)` then non-blocking `recv_timeout` or `try_recv` for new events.
  - This keeps the event model decoupled from the UI thread.
- **Performance**: Polling every 300-400ms across a modest number of fins (5-20) and their log files is acceptable. We are not building a high-frequency log shipper.
- **Error resilience**: Individual fin log read failures or missing `latest-run` must not crash the watcher. Log warnings (to stderr or a future TUI status line) and continue.
- **Testing**: Use `tempfile` + manually constructed `.orqa/fins/<fin>/runs/<id>/...` trees and `mail/new/` files. The existing test infrastructure (`pod_flow.rs` style) can be extended.

---

## 5. Files Expected to Change / Be Created

- `src/tui/events.rs` (new) — `Event` enum + `LogStream` helper + `Display` impls.
- `src/tui/watcher.rs` (new) or add `PodWatcher` inside `events.rs` / a new `watch.rs`. Keep it in `src/tui/`.
- `src/tui/mod.rs` — export the new types.
- `src/tui/run.rs` — (minor) possibly integrate a `PodWatcher` instance for verification / live event counting in the skeleton.
- `Cargo.toml` — no new dependencies expected for Phase 2 (we can use `std::fs`, `std::time`, and existing `mailbox` helpers).
- Possibly small helpers in `src/runs.rs` or `src/mailbox/storage.rs` if we want to expose reusable "read latest run id for fin" logic (prefer keeping watcher self-contained unless duplication is painful).
- `docs/tui-factory-ledger.md` — update Phase history.
- This delivery spec + Phase 2 poker-face note.

---

## 6. Verification & Quality Gates for This Phase

**Must pass before commit:**

1. `cargo build`, `cargo test --locked`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` all clean.
2. New unit tests in the `tui` module (or `#[cfg(test)]` inside the new files) that:
   - Construct a synthetic pod with 2-3 fins, multiple runs, mail files.
   - Create a `PodWatcher`, call `poll()` multiple times, assert the expected `Event` variants and content are produced.
   - Verify that changing the `latest-run` pointer causes the watcher to pick up the new run's logs.
3. Manual integration smoke (using the enhanced skeleton if present):
   - `orqa init` a temp pod.
   - Create one or two fins + run them a couple of times via `orqa fin exec` or the loop.
   - Send some mail to a fin.
   - Launch the TUI (bare `orqa`).
   - Observer (via debug key or log) that the watcher is capturing log appends and mail arrivals in real time.
4. The watcher uses **only** `PodRegistration` + data-home methods. No legacy `~/.orqa/pods` paths appear in the new watcher code.
5. The event types are well-documented with doc comments explaining each variant's meaning and when it is emitted.
6. No regressions in existing functionality (especially `orqa init`, `fin create`, detection, and the Phase 1 TUI skeleton launch).

**Poker-face style questions for this phase:**

- Does the watcher ever assume a fin has a "latest" run when it doesn't?
- Are mail events only emitted for genuinely new files (not re-emitting the same mail on every poll)?
- Does switching to a new run id for a fin correctly reset offsets and not duplicate old log lines?
- Is the API pleasant for the future timeline renderer (Phase 3)?
- Are all paths derived from the single `PodRegistration` passed in at construction?

---

## 7. Risks & Mitigations

- **Risk**: Polling many log files + mail dirs on every tick becomes slow or spammy when there are many fins or very large logs.
  **Mitigation**: Phase 2 accepts simple polling. We will measure and can add smarter "only check fins that have pending work" or `notify` in a later phase if needed. Keep poll interval tunable.

- **Risk**: "latest-run" pointer is updated by a running fin while we are reading its logs, causing torn reads or missed lines.
  **Mitigation**: Read the pointer first, then open the log files for that specific run id. Accept that a line written exactly during the switch may appear in the next poll. Document the best-effort nature.

- **Risk**: The event model we choose in Phase 2 turns out to be awkward for the timeline renderer in Phase 3.
  **Mitigation**: Keep the `Event` enum small and extensible. Use a simple `enum` with clear variants rather than a giant polymorphic design. We can always add new variants later.

- **Risk**: Background thread + channel introduces lifetime / shutdown complexity.
  **Mitigation**: For Phase 2 we can keep the watcher fully synchronous (`poll()` called from the main TUI loop at the top of each draw cycle). Move to background thread only if the polling cost is noticeable. Document the chosen model clearly.

---

## 8. Definition of Done for Phase 2

- The delivery spec is the source of truth.
- A working `Event` type + `PodWatcher` (or equivalent) exists that can be instantiated with a `PodRegistration` from a real Phase 05 pod and produces correct events for log appends and new mail.
- All "In Scope" items are implemented, tested, and verified.
- Quality gates passed (including the manual smoke with a live pod).
- Clean git commit referencing "TUI Phase 2".
- This ledger entry updated.
- Factory proceeds to Phase 3 (Timeline UI + Filters) only after explicit checkpoint / poker-face sign-off.

---

**Persisted by:** Factory (via Grok)  
**Next action after spec:** Implement Phase 2 (event types + watcher), add tests, run verification + poker-face, then commit.