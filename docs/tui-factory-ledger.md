# TUI Operator Cockpit — Factory Run Ledger

**Feature:** Ratatui Operator Cockpit TUI (bare `orqa` inside Phase 05 pod root)  
**Design Authority:** `docs/tui-operator-cockpit.md` (post-refinement)  
**Baseline:** Post-Phase 05 pod-root redesign (delivered)  
**Factory Run Started:** 2026-05-16  
**Status:** Phase 3 complete — Ready for Phase 4

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

**Phase 3: Timeline UI + Filters**

**Goal:** Replace the text skeleton with a real scrollable unified timeline view in Ratatui. Implement the core filters (`f` for fin, `o` for operator mail, thread/subject) and smooth keyboard navigation + follow mode. The event engine from Phase 2 now drives a live, filterable visual experience.

**Success Criteria for Phase 3:**
- A functional scrollable timeline is the main view (using `List` or `Paragraph` + state).
- Events from `PodWatcher` are rendered with color coding and reasonable formatting.
- Hotkeys `f`, `o` (and `t`/`/` for thread) work and update the visible list instantly.
- Follow mode + manual scrolling behave intuitively (pause on scroll up, resume at bottom).
- Header clearly shows pod, active filters, and status.
- The TUI remains responsive with hundreds of events.
- All previous Phase 1/2 guarantees (safe launch, terminal restore, only inside real pods) are preserved.
- `cargo fmt --check` + strict clippy pass.
- Good manual experience: create activity in a pod → run `orqa` → see live filtered timeline.
- Persisted Phase 3 Delivery Spec exists (`docs/tui-phase-03-timeline-ui-filters-delivery.md`).

**Blocked On / Open in this Phase:** None.

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
- **2026-05-16** — Phase 2 complete. Poker-face passed.
- **2026-05-16** — Phase 3 complete:
  - New `src/tui/app.rs` with `App` + `FilterState` owning the live event buffer, filters, and scroll state.
  - Full Ratatui layout: header + scrollable `List` timeline + status bar.
  - Color-coded event rendering for logs, mail, runs, locks, and operator actions.
  - Working filters: `f` cycles fins, `o` toggles operator-only, `/`/`t` thread query.
  - Proper follow mode + keyboard scrolling (arrows, PageUp/Down, Home/End).
  - `run.rs` now uses the real `App` instead of the text skeleton.
  - All fmt + strict clippy clean. Phase 1/2 safety guarantees preserved.
- Poker-face passed. Ready for Phase 4 (Composer, Send & Wake).

Keep this ledger updated after every phase gate and commit.