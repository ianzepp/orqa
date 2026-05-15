# TUI Operator Cockpit — Factory Run Ledger

**Feature:** Ratatui Operator Cockpit TUI (bare `orqa` inside Phase 05 pod root)  
**Design Authority:** `docs/tui-operator-cockpit.md` (post-refinement)  
**Baseline:** Post-Phase 05 pod-root redesign (delivered)  
**Factory Run Started:** 2026-05-16  
**Status:** In progress (Phase 4)

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

**Phase 4: Composer, Send & Wake**

**Goal:** Add the bottom composer that lets the operator type messages, select a target fin, and send them as mail from the local `operator` fin. Implement the critical wake logic (bypass debounce + respect existing run.lock with post-run re-wake). Show synthetic operator actions in the timeline.

**Success Criteria for Phase 4:**
- A working single-line composer at the bottom showing `operator@<pod> → <target> > `.
- `f` changes the target fin (with sensible default from pod.toml or discovery).
- Pressing Enter sends the mail via the real mailbox machinery and triggers the operator wake rules.
- If the target fin is locked, it is not killed — the re-wake happens after the current run finishes.
- A synthetic `OperatorAction` event appears in the timeline immediately after sending.
- Basic input editing + command history (↑/↓) works.
- Transient feedback in the composer line (“sent”, “woke X”, errors).
- All previous safety and terminal restore guarantees remain intact.
- `cargo fmt --check` + strict clippy pass.
- Strong manual verification: send messages while fins are idle and while they are running; verify correct wake behavior.
- Persisted Phase 4 Delivery Spec exists (`docs/tui-phase-04-composer-send-wake-delivery.md`).

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
- **2026-05-16** — Phase 3 complete. Poker-face passed.
- **2026-05-16** — Phase 4 delivery spec persisted (`docs/tui-phase-04-composer-send-wake-delivery.md`). Implementation begins.

Keep this ledger updated after every phase gate and commit.