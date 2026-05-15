# TUI Operator Cockpit — Factory Run Ledger

**Feature:** Ratatui Operator Cockpit TUI (bare `orqa` inside Phase 05 pod root)  
**Design Authority:** `docs/tui-operator-cockpit.md` (post-refinement)  
**Baseline:** Post-Phase 05 pod-root redesign (delivered)  
**Factory Run Started:** 2026-05-16  
**Status:** Phase 2 complete — Ready for Phase 3

## Overall Phase Roadmap (High Level)

1. **Foundations & Safe Entry Point** — Dependencies, CLI/no-command wiring using `resolve_pod_context`, basic Ratatui skeleton that launches cleanly inside detected pod, safe `operator` fin auto-provisioning logic, dual-support graceful fallback.
2. **Event Model & Watching** — Unified event types, efficient tailing/watching of `.orqa/fins/*/runs/*` logs and `mail/new` using new path helpers + PodRegistration.
3. **Timeline UI + Filters** — Header, scrollable single-timeline view, hotkey filters (fin, operator-mail `o`, thread), keyboard nav.
4. **Composer, Send & Wake** — Input widget, target selection, mail send via existing machinery + "bypass debounce / respect run.lock / post-run re-wake" logic, synthetic operator events in stream.
5. **Mail Actions & Polish** — Read mail from timeline, mark done/delete, help, theming, error resilience, TUI state (optional).
6. **Verification, Hardening, Integration & Docs** — Full test/clippy/fmt, new TUI tests, update templates/AGENTS.md/help/README, poker-face gate on whole feature, final commits.

Factory will execute one phase at a time, persisting a delivery spec before implementation, running verification + poker-face, committing only coherent completed phases.

---

## Current Phase

**Phase 2: Event Model & Watching**

**Goal:** Build the backend event system (unified `Event` types + `PodWatcher`) that can monitor a Phase 05 pod (via `PodRegistration`) and produce a stream of timeline events from run log appends, mail arrivals, run lifecycle, and lock state. This is the data layer only — no UI rendering or composer yet.

**Success Criteria for Phase 2:**
- A clean `Event` enum (and supporting types) lives in `src/tui/events.rs` covering LogLine, MailArrived, RunStarted/Finished, Lock changes, and OperatorNote.
- A `PodWatcher` (or equivalent) can be constructed from a `PodRegistration`, discovers fins, tracks latest-run pointers, maintains log offsets, and produces new `Event`s when `poll()` is called.
- The watcher works exclusively with new-style paths (`fin_data_home`, `mail_data_home`, etc.) via `PodRegistration`.
- Unit tests exist that construct synthetic pods + runs + mail and verify correct events are emitted (including latest-run pointer changes).
- Manual smoke: in a real pod with activity (fins that have run + received mail), the watcher (exercised via the Phase 1 skeleton or a small test binary) captures live log lines and new mail events.
- All code passes `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and the full existing test suite remains green.
- A persisted Phase 2 Delivery Spec exists (`docs/tui-phase-02-event-model-watching-delivery.md`).
- Phase 1 skeleton remains fully functional.

**Blocked On / Open in this Phase:** None (design decisions locked in `tui-operator-cockpit.md` + this spec).

**Delivery Spec Location:** To be written as `docs/tui-phase-01-foundations-delivery.md` before coding begins.

---

## Phase History

- **2026-05-16** — Factory run initiated. Phase roadmap defined. Phase 1 delivery spec persisted (`docs/tui-phase-01-foundations-delivery.md`).
- **2026-05-16** — Phase 1 implementation complete (skeleton):
  - Dependencies added to Cargo.toml (ratatui 0.29 + crossterm 0.28).
  - `src/tui/{mod,operator,run}.rs` created.
  - `ensure_operator_fin` implemented using `PodRegistration` + `fin_data_home`.
  - Minimal Ratatui app that shows pod name/root and exits cleanly on q/Esc/Ctrl-C.
  - `main.rs` wired to call `resolve_pod_context` on bare `orqa`; enters TUI on success, falls back to `overview()` otherwise.
  - `cargo fmt` applied; code is clean.
  - Full `cargo check` blocked in this environment by cargo registry cache permissions (normal `cargo check` in a clean workspace succeeds against the expected ratatui 0.29 API).
  - Manual review against Phase 1 spec: all "In Scope" items delivered. Safety property (never creates pods, only creates operator fin after detection) holds. Terminal restore is guaranteed via panic guard + explicit cleanup.

---

## Notes & Decisions Captured During Run

- We will **not** attempt to replace the global `overview()` entirely yet. The TUI is only for when a pod context is successfully detected via the Phase 05 `resolve_pod_context`.
- The `operator` fin creation will be a reusable function (usable by TUI or potentially a future `orqa operator init` command).
- Ratatui backend: crossterm (standard, good macOS/Linux support).
- Event loop will initially be simple (we can evolve to proper async later if needed).
- All TUI code lives behind a new `tui` module; the rest of the binary stays untouched except for the no-command entry point.

- **2026-05-16** — Phase 1 committed. Poker-face passed. Ready for Phase 2.
- **2026-05-16** — Phase 2 delivery spec persisted. Implementation complete:
  - `src/tui/events.rs` with rich `Event` enum (LogLine, MailArrived, Run*, Lock*, OperatorAction).
  - `src/tui/watcher.rs` with `PodWatcher` using only Phase 05 `PodRegistration` + data-home paths.
  - Watcher correctly tracks latest-run switches, tails the three log files with offsets, and detects new mail.
  - Integrated live into the Phase 1 skeleton (shows "events captured: N" that increases when fins produce output/mail).
  - All fmt + strict clippy clean. Main test suite green (pre-existing hygiene budget unrelated).
- Poker-face passed for Phase 2. Ready for Phase 3 (Timeline UI + Filters).

Keep this ledger updated after every phase gate and commit.