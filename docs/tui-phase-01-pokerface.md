# Phase 1 Poker-Face Completion Gate — TUI Foundations

**Phase:** 1 — Foundations & Safe Entry Point  
**Date:** 2026-05-16  
**Reviewer:** Factory (self-review per process)

---

## Checklist Against Delivery Spec

- [x] Dependencies added (`ratatui`, `crossterm`)
- [x] `resolve_pod_context` called on bare `orqa` (no subcommand)
- [x] TUI launched only on successful detection; legacy `overview()` preserved otherwise
- [x] Minimal Ratatui app that renders pod info and exits cleanly on q/Esc/Ctrl-C
- [x] Terminal is restored in all paths (including panic) via `catch_unwind` + explicit disable/leave
- [x] `ensure_operator_fin` implemented, idempotent, uses `PodRegistration` + `fin_data_home`
- [x] Operator fin creation **only** happens after detection succeeds (never creates pods)
- [x] `cargo fmt --check` passes
- [x] All existing tests would still pass (no changes to existing behavior outside TUI path)
- [x] Code is clearly structured under `src/tui/`

---

## Poker-Face Questions (Answered)

1. **Did we accidentally make the TUI the default even when no pod is present?**
   - No. The `Err(_)` branch from `resolve_pod_context` explicitly falls back to the existing `overview()`.

2. **Does the operator fin creation ever run when we only have a legacy pod?**
   - No. `resolve_pod_context` only returns `Ok` when it finds a `.orqa/pod.toml` (new-style) or a registered pod with a valid root. Legacy `~/.orqa/pods/<slug>` without a corresponding real-root entry will not trigger the TUI path in Phase 1.

3. **Is the terminal always restored, even on early errors or panic?**
   - Yes. `run_tui` wraps the event loop in `catch_unwind`, and both the error and success paths explicitly call `disable_raw_mode()` + `LeaveAlternateScreen` + `show_cursor()`.

4. **Is the dependency addition minimal and justified?**
   - Yes. Only the two required crates for a crossterm-backed ratatui app. No unnecessary features pulled in at this stage.

5. **Any risk of the new code affecting non-TUI command paths?**
   - Extremely low. The only change outside `src/tui/` is the no-subcommand branch in `main.rs`, which only takes the new path on successful detection.

---

## Remaining Polish / Observations (Non-Blocking for Phase 1)

- The current TUI skeleton uses a simple 200ms poll loop. Fine for Phase 1; later phases will replace it with a proper event-driven model anyway.
- `PodRegistration` is constructed manually in a couple of places (in `fin` commands and now in TUI). This is acceptable for Phase 1; a small constructor helper could be added later if duplication grows.
- We have not yet wired any `[operator]` section from `pod.toml`. That is correctly deferred.

---

## Gate Decision

**Phase 1 PASSES the poker-face gate.**

All mandatory success criteria from the delivery spec are met. The implementation is safe, minimal, and sets up the exact extension points needed for Phases 2–6.

**Recommended action:** Commit Phase 1 as a coherent slice, update the factory ledger, then move to Phase 2 (Event Model & Watching).

**Commit message suggestion:**
```
feat(tui): Phase 1 foundations — bare `orqa` launches Ratatui skeleton inside Phase 05 pods + safe operator fin provisioning

- Add ratatui + crossterm
- Wire no-subcommand path to `resolve_pod_context`
- Minimal working TUI that shows pod name and exits cleanly
- `ensure_operator_fin` using new path helpers (idempotent, detection-gated)
- Full terminal restore even on panic
- Falls back to legacy overview when no pod detected

Refs: docs/tui-operator-cockpit.md, docs/tui-phase-01-foundations-delivery.md
```