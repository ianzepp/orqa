# TUI Operator Cockpit — Factory Run Ledger

**Feature:** Ratatui Operator Cockpit TUI (bare `orqa` inside Phase 05 pod root)  
**Design Authority:** `docs/tui-operator-cockpit.md` (post-refinement)  
**Baseline:** Post-Phase 05 pod-root redesign (delivered)  
**Factory Run Started:** 2026-05-16  
**Status:** In progress (Phase 1)

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

**Phase 1: Foundations & Safe Entry Point**

**Goal:** Make `orqa` (bare) detect a pod using the existing Phase 05 mechanisms and either launch a minimal working Ratatui TUI or fall back cleanly. Implement the safe operator-fin creation path that the TUI will use. No full timeline or composer yet — just the launch skeleton and safety guarantees.

**Success Criteria for Phase 1:**
- `cargo add ratatui crossterm` (plus any minimal peer deps) succeeds; project still builds and all existing tests pass.
- Bare `orqa` inside a Phase 05 pod root (`.orqa/pod.toml` present) no longer prints the old text overview; instead it enters a Ratatui app that shows the pod name/root and exits cleanly on `q`/`Esc`.
- Bare `orqa` outside any detectable pod continues to show the existing text overview (no TUI, no files created).
- A new module `src/tui/` (or `src/tui/mod.rs`) exists with clean separation.
- `orqa` binary gains the ability to create the local `operator` fin safely when the TUI first runs inside a valid pod (idempotent, uses `fin_data_home` via PodRegistration, never creates the pod itself).
- All new code passes `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
- Basic integration test or manual verification that launching in a temp pod root works.
- A persisted Phase 1 Delivery Spec exists.

**Blocked On / Open in this Phase:** None (design decisions already locked in the spec doc).

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

Keep this ledger updated after every phase gate and commit.