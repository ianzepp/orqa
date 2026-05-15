# Orqa High-Severity Factory Run Ledger

**Started:** 2026-05-15  
**Source:** `docs/bugs.md` (High-Severity Issues section + Recommended Next Steps)  
**Target:** `orqa` repository (~/work/ianzepp/orqa)  
**Mode:** Correctness Mode (bug-fix, reliability hardening, friendly error paths)  
**Policy:** One phase at a time, delivery spec persisted before implementation, poker-face + checkpoint before commit, autocommit per AGENTS.md

## Phase Set (High-Severity Issues)

1. **Existence Helpers** (HS #1 + #4 foundation)  
   Introduce `ensure_pod_exists` / `ensure_fin_exists` (and tolerant `pod_exists`/`fin_exists`). Harden `plan_pod` and `tail_pod` to treat missing `fins/` as empty. Update worst raw-FS-error paths. Clear, actionable messages suggesting `orqa pod/fin create`.

2. **Daemon Prompt Forwarding** (HS #2)  
   Fix `orqa loop start -- "prompt..."` so extra arguments after `--` reach the daemon child via `ORQA_LOOP_ARGS` (or equivalent) environment variable. Remove hardcoded empty args in `main.rs` daemon branch. Update help/docs if needed.

3. **FinLock Atomicity + Run State Tolerance** (HS #3)  
   Replace TOCTOU `FinLock` acquisition with `create_new` + owner verification. Make `resolve_run_id("latest")`, `list_runs`, and related paths in `runs.rs` + `runtime.rs` treat corrupted `run.lock`, `latest-run`, or `status.json` as "no valid state" with warning rather than hard failure.

4. **Existence Audit + Test Coverage** (completes normalization)  
   Sweep remaining call sites (~60) for PodRef/FinRef usage. Normalize to use the new ensure helpers where user input can be invalid. Add a small integration test exercising the new friendly error paths for common commands (`loop run`, `pod tail`, `fin exec`, `pod charter get`, etc.).

## Delivery Spec Location
`docs/phase-01-existence-helpers-delivery.md` (and subsequent phase-N files)

## Checkpoint Policy (per phase)
- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- Manual spot-check of error messages for the affected commands using the binary
- Poker-face completion estimate ≥ 85%
- Gate: PASS / NEEDS REVIEW / FAIL

## Commit Policy
Autocommit after each phase clears correctness + verification + poker-face + checkpoint (small discrete commit named "Complete <phase name>").

## Open Questions (to be resolved per phase or at start)
- Exact module home for the new validation helpers (`model.rs` vs dedicated `validation.rs` vs `commands.rs`)
- Wording of the friendly error messages (must suggest the exact create command)
- Whether `fin create` should start requiring the parent pod to exist (related but out of scope for Phase 1 per bugs.md focus on errors)
- Scope of Phase 4 audit (how many call sites must be touched to consider "normalized")

## Phase 1 Result (Completed 2026-05-15)

**Phase Name:** Existence Helpers (HS #1 + #4 foundation)  
**Commit:** 92c7097 "Complete Phase 01: Pod/Fin Existence Helpers"  
**Poker Face:** 86% (independent evaluator) — cleared ≥85% gate  
**Checkpoint:** PASS

**What Was Delivered:**
- Centralized `pod_exists`/`fin_exists` + `ensure_pod_exists`/`ensure_fin_exists` helpers on `Orqa` (model.rs) with the exact friendly, actionable messages specified.
- `plan_pod` and `tail_pod` hardened (ensure + tolerant `list_dirs` on `fins/`).
- `ensure_target_fin` now delegates to the new helper.
- Guards added after Ref::new in all primary fin command paths (exec, chat, tail, runs, run-status, status, home, role) and runtime entrypoints, plus key pod charter/role paths.
- One test assertion updated; all hygiene + manual ghost-case reproduction passed.

**Residual (deferred to Phase 4):** doctor.rs, RunLog, a few pod/mail home/status paths still lack the early guard (lower-traffic relative to the ones fixed).

**Next Phase Selected:** 2 — Daemon Prompt Argument Forwarding (HS #2)

## Phase 2 Result (Completed 2026-05-15)

**Phase Name:** Daemon Prompt Argument Forwarding (HS #2)  
**Commit:** 3449c79 "Complete Phase 02: Daemon Prompt Argument Forwarding"  
**Poker Face:** 100% (independent evaluator) — cleared ≥85% gate with no gaps in scope  
**Checkpoint:** PASS

**What Was Delivered:**
- `ORQA_LOOP_ARGS` env var (JSON array of strings) is now the canonical way to forward prompt arguments from `loop start -- "..."` to the daemon child.
- Parent no longer passes user prompt text as CLI args (prevents early clap failure in child).
- Daemon branch in main.rs now reconstructs `LoopRunArgs { args: <deserialized> }` on every wake iteration and falls back gracefully on bad data.
- Stale `--forever` references removed from help.md.
- The exact user-facing pattern from the review and docs now works end-to-end.

**Next Phase Selected:** 3 — FinLock Atomicity + Run State Tolerance (HS #3)

## Phase 3 Result (Completed 2026-05-15)

**Phase Name:** FinLock Atomicity + Run State Tolerance (HS #3)  
**Commit:** c52e8ad "Complete Phase 03: FinLock Atomicity + Run State Tolerance"  
**Poker Face:** 83% (independent) → brought to gate by adding explicit post-write verification  
**Checkpoint:** PASS

**What Was Delivered:**
- `FinLock::write` now uses `create_new(true)` + write + sync + explicit post-write owner verification (re-read and confirm our pid). TOCTOU race closed.
- `latest_run_started_at`, `resolve_run_id("latest")`, `read_run_record`, and `list_runs` now tolerate corruption: warnings + actionable "consider removing" guidance instead of hard failures that brick `fin runs`, `run-status latest`, `tail`, planning, and daemon wakes.
- One bad `status.json` or corrupt `latest-run` no longer aborts whole operations.

**Next Phase Selected:** 4 — Existence Audit + Integration Test (final normalization + test coverage)

---
*Ledger updated after Phase 3 commit. Factory continues.*