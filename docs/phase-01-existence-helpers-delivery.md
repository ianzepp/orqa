# Phase 01 Delivery Spec — Pod/Fin Existence Helpers

**Factory Run:** High-Severity Correctness Fixes  
**Phase:** 1 of 4  
**Source Issues:** HS #1 (Raw filesystem errors) + HS #4 (Inconsistent existence validation) from `docs/bugs.md`  
**Date Prepared:** 2026-05-15

---

## 1. Interpreted Problem

The most common user error paths (`orqa loop run nonexistent`, `orqa pod tail bad-pod`, `orqa fin exec ghost/ghost`, `orqa pod charter get missing`, `orqa loop plan bad`) produce raw, unhelpful OS errors such as:

```
failed to read fins directory /.../pods/bad-pod/fins: No such file or directory
failed to read config /.../pods/bad-pod/pod.toml: No such file or directory
```

This happens because:

- `plan_pod` (runtime.rs:153) and `tail_pod` (runs.rs:309) perform unconditional `fs::read_dir` on the `fins/` subdirectory without first verifying the pod exists.
- Dozens of command handlers construct a `PodRef`/`FinRef` (which only validates the *slug syntax*), then immediately read `pod.toml`, `fin.toml`, `CHARTER.md`, `ROLE.md`, etc. via `read_toml` or `print_file` with no existence guard.
- The only high-quality existence check today is the private `ensure_target_fin` in `mailbox/mod.rs:154`, which looks for `fin.toml` and emits `"target fin {} does not exist"`.
- `list_dirs` (duplicated in commands.rs and report.rs) already has the correct defensive pattern: return empty Vec when the directory does not exist.

The root cause is **missing centralized, early existence validation** that produces actionable errors and the inconsistent application of defensive I/O patterns.

**Goal of this phase:** Eliminate the class of confusing low-level errors for the highest-traffic failure modes by introducing reusable helpers and hardening the two worst direct `read_dir` sites.

---

## 2. Normalized Spec

### Functional Requirements

1. **New helpers** (public within the crate):
   - `orqa.pod_exists(&PodRef) -> bool`
   - `orqa.fin_exists(&FinRef) -> bool`
   - `ensure_pod_exists(orqa: &Orqa, pod: &PodRef) -> Result<(), String>`
   - `ensure_fin_exists(orqa: &Orqa, fin: &FinRef) -> Result<(), String>`

2. **Error message contract** (must be user-actionable):
   - Pod does not exist: `"pod 'foo' does not exist (run 'orqa pod create foo' to create it)"`
   - Fin does not exist: `"fin 'bar/baz' does not exist (run 'orqa fin create bar/baz' to create it)"`
   - Messages must include the exact suggested create command.

3. **Hardening of plan_pod / tail_pod**:
   - `plan_pod` must treat a missing `fins/` directory (or a pod that has no `fins/` yet) as an empty plan instead of a hard error. It may still require the pod itself to exist (i.e. `pod.toml` present) or produce a friendly pod-not-found error.
   - `tail_pod` must behave the same for the discovery loop (skip or friendly error).

4. **Usage pattern**:
   - High-traffic write paths and status/observability paths that receive user-supplied pod/fin names should call the `ensure_*` helper **early**, before any filesystem reads of pod/fin contents.
   - Discovery paths (overview, list, all-pods loop) should use the tolerant `list_dirs` behavior and skip or warn on entries that fail `PodRef::new` or do not contain a `pod.toml`.

5. **No behavior change** for valid, existing pods and fins.

### Non-Goals (out of scope for Phase 1)

- Full sweep of all ~60 call sites (Phase 4).
- Changing `fin create` to require the parent pod (related but separate correctness item).
- Deduplicating the two `list_dirs` implementations (lower-severity).
- Any changes to daemon lifecycle or lock acquisition (Phases 2 & 3).

---

## 3. Repo-Aware Baseline

### Existing Good Patterns (to emulate)

- `ensure_target_fin` (src/mailbox/mod.rs:154-161):
  ```rust
  fn ensure_target_fin(orqa: &Orqa, fin: &FinRef) -> Result<(), String> {
      let config = orqa.fin_home(fin).join("fin.toml");
      if config.exists() {
          Ok(())
      } else {
          Err(format!("target fin {} does not exist", fin.label()))
      }
  }
  ```
  This is the canonical example. We will generalize it and improve the message.

- `list_dirs` (src/commands.rs:268-283) — already returns `Ok(Vec::new())` when `!dir.exists()`. The duplicate in report.rs is private.

- `PodRef::new` / `FinRef::new` (src/model.rs:77-99) — only slug syntax validation via `validate_slug`. Existence is deliberately separate.

- `Orqa` path helpers (src/model.rs:12-69): `pod_home`, `fin_home`, `pod.toml` is always at `pod_home/pod.toml`, `fin.toml` at `fin_home/fin.toml`.

### Key Call Sites That Must Be Addressed in Phase 1 (minimum)

**Direct raw read_dir on fins (must be fixed):**
- `src/runtime.rs:153-160` — `plan_pod` (used by `loop run`, `loop plan`, daemon `loop_single_pod`)
- `src/runs.rs:309-316` — `tail_pod` (`orqa pod tail`)

**High-traffic read_toml / content paths (should call ensure early):**
- `src/config.rs:249,266` — `run_policy`, `backend_chat_command` (read pod.toml + fin.toml)
- `src/commands.rs:57-58` (charter get), 155 (role get), 235 (fin tail), many others
- `src/status.rs`, `src/doctor.rs`, `src/report.rs` (overview paths)

**Places that already do some work:**
- `mailbox/mod.rs` already calls `ensure_target_fin` before mail/task operations.
- `pod create` and `fin create` are the success paths that create the TOML files.

### Module Placement Decision

Place the four new methods on the `Orqa` struct in `src/model.rs` (next to the existing path helpers and `PodRef`/`FinRef` definitions). This keeps all "does this pod/fin identity make sense on disk" logic co-located with the identity types and path computation.

`ensure_target_fin` in mailbox can later be updated to call the new general `ensure_fin_exists` (or kept as a thin wrapper for now).

---

## 4. Stage Graph (for Phase 1)

Because this is a focused correctness phase, the internal stage graph is small:

1. **Design & API** — Define the four helper signatures + error message format exactly. Decide on `pod_exists` (bool) vs only the ensure versions.
2. **Implementation** — Add methods to `Orqa` in model.rs. Update `plan_pod` to use tolerant discovery + early ensure. Update `tail_pod` similarly.
3. **Propagation** — Update the most critical direct callers (`loop` commands, `pod tail`, `fin exec`, `status` for specific fin, `doctor` for specific fin) to invoke the ensure helpers so the friendly error is seen first.
4. **Verification** — Run full test + clippy suite. Manually exercise the new error paths with the built binary for the commands listed in bugs.md.
5. **Poker-face + Checkpoint** — Independent completion check against this spec.

No parallel workstreams needed; the changes are tightly coupled within the existence/validation surface.

---

## 5. Work Items / Scoped Issues for Phase 1

- [ ] Add `pod_exists`, `fin_exists`, `ensure_pod_exists`, `ensure_fin_exists` to `Orqa` impl in model.rs with clear error messages.
- [ ] Refactor `plan_pod` (runtime.rs) to:
  - Call `ensure_pod_exists` early (or produce the friendly pod error).
  - Use the tolerant `list_dirs` (or duplicate its logic locally) for the `fins/` directory so a pod with no `fins/` subdirectory yields an empty plan.
- [ ] Apply the same treatment to `tail_pod` in runs.rs.
- [ ] Update at least the following high-visibility command paths to call the ensure helpers before further work:
  - `loop run <pod>` and `loop plan <pod>` (via plan_pod)
  - `pod tail`
  - `fin exec`, `fin chat`, `fin tail`, `fin run-status`, `fin runs`
  - `pod charter get/set`, `pod role get/set` (the per-pod ones)
  - `doctor` when a specific pod/fin is given
- [ ] Ensure the new helpers are used by `ensure_target_fin` or that mailbox continues to work.
- [ ] Add or update any unit tests in `config_test.rs` / `main_test.rs` if the new helpers have pure logic worth testing.
- [ ] Verify no regression on the happy path for existing pods/fins (existing integration tests in `tests/pod_flow.rs` should still pass).

---

## 6. Checkpoints & Gates

**Primary Checkpoint (end of Phase 1):**
- Running `orqa loop run nonexistent-pod` produces a friendly error containing the suggested `orqa pod create` command (no raw "failed to read fins directory").
- Same for `orqa pod tail bad-pod`, `orqa fin exec ghost/ghost`, `orqa pod charter get missing`.
- A pod that exists but has no `fins/` directory (freshly created pod) produces a clean empty plan / no crash on `loop run` or `pod tail`.
- `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` all pass.
- Poker-face completion estimate ≥ 85% against this delivery spec.

**Gate Criteria:**
- All functional requirements in section 2 satisfied.
- No new `.unwrap()` or panics on user-controlled paths introduced.
- Error messages are consistent in style with the existing `ensure_target_fin` message.

**Success Evidence:**
- Before/after manual reproduction of the 5-6 example failing commands from bugs.md.
- Diff limited to model.rs + runtime.rs + runs.rs + a few command entry points.

---

## 7. Companion Skill Plan

- **Correctness Mode** (this reference) — primary mode for the whole phase.
- **Poker-face** — mandatory completion gate before checkpoint.
- **Check** (if available) — optional deeper verification subagent after the main implementation.
- **Bonsai** — only if the changes introduce obvious style debt that would fail a later housekeeping pass; prefer to keep the diff minimal.
- No `carmack-linus` or heavy design review needed — the fix is localized and follows an existing good pattern.

---

## 8. Gate Plan & Exit Criteria

**After implementation + verification:**
1. Run the repo validation commands.
2. Execute manual error-path tests for the documented failing cases.
3. Run poker-face (self-estimate first, then independent evaluation).
4. Evaluate checkpoint.
5. If gate = PASS, commit with message "Complete Phase 01: Pod/Fin Existence Helpers".
6. Update the factory ledger with results and select Phase 2.

**If gate fails or poker-face < 85%:** return to implementation, fix the gap, re-verify, re-run poker-face.

---

## 9. Open Questions (Phase 1 Specific)

- Should `ensure_pod_exists` be called inside `plan_pod`, or should the callers (`loop` commands) call it before invoking `plan_pod`? (Recommendation: inside `plan_pod` at the top, so every consumer benefits.)
- Do we want a separate `pod_has_fins` or just let the tolerant read_dir handle "pod exists but fins/ does not"?
- Message wording: use backticks around the suggested command? Use single quotes for the slug? Match the style of existing messages exactly.
- Should the helpers live in `model.rs` or should we create `src/validation.rs` for future expansion? (Current decision: `model.rs` to keep blast radius tiny for Phase 1.)

**Resolved during intake:** Helpers go in `model.rs`. `plan_pod` itself will call the ensure for the pod and use tolerant discovery for fins.

---

## 10. Delivery Sign-off

This phase is ready for implementation. The spec is bounded, follows existing patterns, targets the exact failure class described in the review, and produces a clear, testable checkpoint.

**Next action (factory supervisor):** Mark delivery complete, begin implementation of the helpers and the two critical call sites (`plan_pod`, `tail_pod`), then the minimal propagation to commands.

---
*Artifact persisted before any code changes for Phase 1.*