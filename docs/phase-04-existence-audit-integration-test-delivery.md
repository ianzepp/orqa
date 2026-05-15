# Phase 04 Delivery Spec — Existence Audit + Integration Test

**Factory Run:** High-Severity Correctness Fixes  
**Phase:** 4 of 4 (Final)  
**Source:** Recommended Next Steps in `docs/bugs.md` + remaining call sites after Phases 1–3  
**Date Prepared:** 2026-05-15

---

## 1. Interpreted Problem

After Phases 1–3, the core helpers (`ensure_pod_exists`, `ensure_fin_exists`) exist and protect the highest-traffic paths (`loop run/plan`, `pod tail`, all major `fin exec/chat/tail/runs/run-status/status/home/role`, charter get/set, etc.).

However, a systematic audit of the ~60 `PodRef::new` / `FinRef::new` call sites shows several remaining areas that still assume the pod or fin was previously created:

- `hooks.rs` (all pod-level hook operations)
- `doctor.rs` (pod doctor and per-fin doctor)
- `report.rs` (overview / `orqa` bare dashboard for specific pods/fins)
- `status.rs` (pod-level status)
- `mailbox/` (some resolution paths)
- `config.rs` (internal `read_toml` for pod/fin configs, reached from various places)
- `service.rs` (legacy loop code)
- `fin create` can still succeed for a non-existent parent pod (creates orphan fins)

These paths produce either raw FS errors or confusing behavior when given ghost pods/fins.

Additionally, the original review recommended:

> "Add a small integration test that exercises 'non-existent pod/fin' error paths for the most common commands."

Without this test, future refactors could silently regress the friendly-error behavior.

---

## 2. Normalized Spec

### Functional Requirements

1. **Complete pragmatic audit sweep**
   - Add early `orqa.ensure_pod_exists(&pod)?` (right after `PodRef::new`) in:
     - All hook command handlers (`list_hooks`, `add_hook`, `remove_hook`, etc.)
     - `pod_doctor` (top level)
     - `print_pod` / overview paths in `report.rs`
     - pod-level status paths
   - Add early `orqa.ensure_fin_exists(&fin)?` in:
     - `doctor_fins` / `doctor_fin` paths (when a specific fin is named)
     - Any remaining direct fin command paths not yet covered
   - For internal/config paths (`config.rs`): ensure callers have already validated, or add defensive checks where `read_toml` on pod/fin toml is the first real access.

2. **Enforce pod existence for `fin create` (correctness improvement)**
   - `fin create` must now require the parent pod to exist (call `ensure_pod_exists` early).
   - This prevents orphan fins and matches user expectation ("you can't create a fin in a pod that doesn't exist").
   - Error message must be the standard friendly one from the helper.

3. **Integration test**
   - Add a test (in `tests/pod_flow.rs` or a new `tests/error_paths.rs`) that:
     - Creates a valid pod
     - Attempts several ghost operations on non-existent pods and fins:
       - `loop run ghost-pod`
       - `pod tail ghost-pod`
       - `fin exec ghost-pod/ghost-fin ...`
       - `pod charter get ghost-pod`
       - `fin run-status ghost-pod/ghost-fin latest`
       - `fin create ghost-pod ghost-fin` (after the pod-exists requirement)
     - Asserts that each produces the exact friendly error containing the suggested `orqa pod/fin create ...` command.
   - The test must continue to pass after the change to `fin create`.

4. **No over-auditing**
   - Test-only code (`*_test.rs`) and pure internal helpers that are only called after validation are out of scope.
   - Paths that already go through `ensure_target_fin` (mailbox) or the high-traffic guarded paths do not need duplicate guards.

### Non-Goals

- Exhaustive coverage of every single one of the 64 call sites (many are now safe because of Phase 1 helpers or higher-level validation).
- Changing `pod create` semantics.
- Adding a full "dry-run existence checker" or new CLI flag.

### Success Criteria

- Running any of the documented ghost commands now reliably produces the friendly, actionable message from the helpers.
- `fin create` on a non-existent pod fails early with the correct pod-not-found message.
- A new integration test (or expanded `pod_flow` test) exercises at least 5–6 of the key error paths and would have caught regressions.
- All hygiene passes.

---

## 3. Repo-Aware Baseline

### Files Likely to Need Changes

- `src/hooks.rs` — 5+ pod Ref sites
- `src/doctor.rs` — pod + fin doctor paths
- `src/report.rs` — overview functions
- `src/status.rs` — pod_status entry
- `src/commands.rs` — `fin create` handler + any remaining direct handlers
- `tests/pod_flow.rs` (or new test file) — integration test

### Already Good Patterns (to emulate)

- Phase 1 pattern: `let x = XRef::new(...)?; orqa.ensure_*_exists(&x)?;` immediately after.
- `ensure_target_fin` delegation (now uses the helper).
- `list_dirs` tolerance for missing directories.

### Related Open Item from Earlier Phases

The bugs.md noted: "`fin create` can succeed without a prior pod." Phase 4 is the natural place to close this as part of "normalize existence validation."

---

## 4. Stage Graph (for Phase 4)

1. **Audit & Prioritize** — Produce a short table of remaining high-value vs low-value call sites. Decide exact list to touch in this phase.
2. **Guard Addition** — Add `ensure_*` calls in hooks, doctor, report, status, and `fin create`.
3. **Behavior Change for `fin create`** — Enforce pod existence; update any tests/docs if needed.
4. **Integration Test** — Write a test that exercises the friendly error surface for the commands listed in the review.
5. **Verification** — Full hygiene + manual run of the ghost commands + new test passes.

---

## 5. Work Items / Scoped Issues for Phase 4

- [ ] Add `ensure_pod_exists` guards in `hooks.rs` (all public handlers).
- [ ] Add guards in `doctor.rs` (pod_doctor + doctor_fins when specific fin given).
- [ ] Add guards in `report.rs` (print_pod / print_fin).
- [ ] Add guard in `status.rs` for pod-level status entry point.
- [ ] Modify `fin create` handler in `commands.rs` to call `ensure_pod_exists` first (or require pod home + pod.toml).
- [ ] Add a new or expanded integration test in `tests/` that asserts friendly error messages for at least:
  - non-existent pod on `loop run`, `pod tail`, `pod charter get`
  - non-existent fin on `fin exec`, `fin run-status latest`
  - `fin create` on non-existent pod
- [ ] Run full `cargo test`, clippy, fmt.
- [ ] Manually verify the ghost command matrix produces the expected messages.

---

## 6. Checkpoints & Gates

**Primary Checkpoint (end of Phase 4 / end of factory run):**
- The remaining high-visibility surfaces (hooks, doctor, overview, status, fin create) now give the same friendly "does not exist (run `orqa ... create ...`)" errors as the paths fixed in Phase 1.
- `fin create` without its parent pod fails with the pod-not-found message.
- A committed integration test would have caught any regression of the friendly error behavior.
- No new raw FS errors appear on the documented ghost cases.
- All existing valid-pod/valid-fin workflows continue to work.

**Gate Criteria:**
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` all pass.
- New test exercises the error paths and passes.
- Manual matrix of ghost commands matches the expected friendly messages.

**Success Evidence:**
- `orqa fin create ghost-pod newfin` now says the pod does not exist (instead of silently creating an orphan fin).
- The new integration test output shows 5+ ghost cases all producing the correct create suggestions.

---

## 7. Companion Skill Plan

- **Correctness Mode** — final normalization + test coverage for the existence validation work.
- **Poker-face** — required to confirm the audit + test actually close the original recommendation.
- Light `housekeeping` awareness only if the audit reveals obvious duplication or dead code (do not expand scope).

---

## 8. Gate Plan & Exit Criteria

After implementation:
1. Hygiene + full test suite.
2. New integration test passes.
3. Manual verification of the ghost command matrix.
4. Poker-face (self + independent) ≥ 85%.
5. Checkpoint evaluation.
6. Commit with message "Complete Phase 04: Existence Audit + Integration Test".
7. Final factory ledger update + summary that all high-severity issues from the May 2026 review have been addressed.

If gate fails: return to implementation for the missing piece.

---

## 9. Open Questions (Phase 4 Specific)

- How many additional call sites are "worth" guarding vs. "internal and already protected by higher layers"? (Recommendation: focus on user-facing command handlers + doctor + report + hooks.)
- Should the integration test live in `tests/pod_flow.rs` (existing pattern) or a new `tests/error_paths.rs`? (Existing file is fine to keep changes small.)
- Do we want to also guard `mailbox` resolution paths that bypass `ensure_target_fin`? (Most already go through it.)

**Resolved during intake:** Keep the test in `tests/pod_flow.rs` for simplicity. Guard the clearly user-facing surfaces listed above. Enforce pod existence on `fin create`.

---

## 10. Delivery Sign-off

This is the final phase. It completes the work started in the May 2026 correctness review by:
- Finishing the existence validation normalization in the remaining practical surfaces.
- Preventing orphan fins.
- Adding the recommended regression test for the friendly error behavior.

Once this phase passes its gate, the high-severity factory run can be declared complete.

**Next action:** Mark delivery complete, perform the audit + `fin create` change + test, verify, poker-face, commit, and produce the final factory summary.

---
*Artifact persisted before code changes for Phase 4 (final phase).*