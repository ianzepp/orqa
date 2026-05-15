# Phase 03 Delivery Spec — FinLock Atomicity + Run State Tolerance

**Factory Run:** High-Severity Correctness Fixes  
**Phase:** 3 of 4  
**Source Issue:** HS #3 from `docs/bugs.md`  
**Date Prepared:** 2026-05-15

---

## 1. Interpreted Problem

The current `FinLock` implementation and run record handling have two classes of serious reliability problems:

### A. TOCTOU Race in Lock Acquisition
- `FinLock::try_existing` (runtime.rs:659-671) does a non-atomic `path.exists()` check followed by `read_to_string`.
- `FinLock::write` (runtime.rs:673-693) does `create_dir_all` + unconditional `fs::write`.
- Between the check and the write, two different backends (manual `fin exec` + daemon wake, or two concurrent `fin exec` attempts) can both decide "no lock exists" and both proceed to spawn and write a lock.
- Result: duplicate AI runs for the same fin, overwritten locks, corrupted run records, and violated "at most one concurrent backend per fin" invariant.

### B. Brittle / Hard-Failing Handling of Corrupted State Files
- `resolve_run_id("latest")` (runs.rs:342-349) and `latest_run_started_at` (runs.rs:246-264) treat *any* error reading/parsing `latest-run` or `status.json` as a hard failure (except plain NotFound).
- `read_run_record` (runs.rs:335-340) and `read_run_record_for` do strict `fs::read_to_string` + `serde_json::from_str` with no tolerance.
- `list_runs` (runs.rs:217-232) calls `read_run_record` inside the loop; one bad `status.json` aborts the entire listing.
- Corrupted files (from crash during write, manual edit, disk glitch, or partial `run.lock`) brick:
  - `fin runs`
  - `fin run-status latest`
  - `fin tail`
  - `orqa plan` / daemon wake decisions (via `latest_run_age`)
  - Overview / status reports

Users are left with unhelpful raw errors and no guidance ("consider removing the stale file").

The review explicitly calls out the assumption "at most one concurrent backend per fin" with "no central place documents" it, plus pid reuse risks on `kill -0`.

---

## 2. Normalized Spec

### Functional Requirements

1. **Atomic lock acquisition**
   - `FinLock::write` must use `std::fs::OpenOptions` with `create_new(true)` + `write(true)` so that only one process can succeed in creating the lock file.
   - If `create_new` fails because the file already exists, the write path must return a clear "already locked" style error (or fall through to the existing live-process check).
   - After a successful atomic create + write, perform a post-write owner verification (re-read the file and confirm the pid we just wrote is present). This detects certain TOCTOU / rename races on some filesystems.

2. **Improved lock file content (recommended but not mandatory for minimal fix)**
   - Optionally add a `started_at=` timestamp (or a random nonce) to the lock file contents so that `is_live` + pid-reuse detection can be strengthened later. At minimum, keep the existing `pid=...` format parseable.

3. **Corruption tolerance for run state (highest user-visible impact)**
   - `resolve_run_id("latest")`: if `latest-run` is missing, unreadable, or contains invalid data → return `Ok("no valid latest")` behavior (i.e. treat as "no last run") + emit a warning via `eprintln!` or a structured log. Never hard-fail the whole operation.
   - `latest_run_started_at`: on any error other than clean NotFound (including parse failures on the id format), return `Ok(None)` + warning.
   - `read_run_record` / `read_run_record_for`: when the target `status.json` is unreadable or fails JSON parse, return a graceful error or (for "latest") fall back to "no valid run" instead of propagating the low-level parse error.
   - `list_runs`: skip individual corrupt `status.json` entries (log a warning) instead of aborting the entire listing. Still return the runs that could be read successfully.

4. **User-visible error messages for corrupt state**
   - When we detect corruption on "latest" or a specific run, the error (or warning) must be actionable: e.g. "latest run pointer for fin X/Y is corrupt or unreadable — consider `rm .../latest-run` and/or inspecting the runs/ directory".
   - Do not change the happy-path success messages.

5. **No behavior change for valid, uncorrupted state**
   - Existing locks, valid `latest-run` pointers, and good `status.json` files must continue to work exactly as before (including the "fin already running" error when a live lock is present).

### Non-Goals (out of scope for Phase 3)

- Full pidfile atomicity + fsync + SIGTERM handling for the *daemon pidfile* itself (part of medium-severity daemon lifecycle issues).
- Adding a full nonce + timestamp liveness protocol with cryptographic strength.
- Changing the on-disk format of `status.json` or the runs ledger.
- Central documentation of the "at most one backend" invariant (can be a small comment or left for Phase 4).
- Windows `process_is_alive` improvements (currently a no-op).

### Error Handling Philosophy (Correctness Mode)

- For *lock races*: hard error is acceptable ("fin is already running"), but we must make the race window impossible via `create_new`.
- For *corrupted user data* (`latest-run`, `status.json`, `run.lock` contents): treat as "absent or stale" with a clear warning. Never let one bad file on disk brick the entire observability or planning surface for a fin.

---

## 3. Repo-Aware Baseline

### Existing Code to Modify

- **Lock logic**:
  - `FinLock::try_existing` and `FinLock::write` in [src/runtime.rs:653-716](/Users/ianzepp/work/ianzepp/orqa/src/runtime.rs)
  - `write_child_lock` helper (around 635)
  - `lock_pid` parser (718)
  - Call sites: `exec_fin_logged` (471), `fin_chat_interactive`, supervised wake path, etc.

- **Run state handling**:
  - `resolve_run_id` (runs.rs:342)
  - `latest_run_started_at` (246)
  - `read_run_record` + `read_run_record_for` (335, 234)
  - `list_runs` (217)
  - `latest_run_age` wrapper in runtime.rs (311) which calls the above.

- **Related types**:
  - `RunFiles` creation and `write_latest` (351)
  - `RunRecord` serde type

### Good Patterns Already Present

- `list_dirs` (commands.rs) already demonstrates "treat missing as empty".
- `latest_run_started_at` already has a clean `NotFound → Ok(None)` branch — we just need to extend the same tolerance to parse/IO/corruption errors.
- The project already uses `std::fs::OpenOptions` in a few places (runs.rs has `OpenOptions` import).

### Design Notes for the Atomic Lock

The standard robust pattern for a pid lock file is:

```rust
let file = OpenOptions::new()
    .create_new(true)
    .write(true)
    .open(&path)?;
writeln!(file, "pid={}\n...", pid)?;
file.sync_all()?; // optional but good
```

If `create_new` fails with `AlreadyExists`, we treat it as "someone else just took the lock" and fall back to the existing `try_existing + is_live` path (which will then produce the "already running" message).

Post-write verification: after the successful `create_new` write, we can re-open the file (or seek+read) and confirm our pid string is at the beginning. This catches certain exotic rename/replace races.

For the minimal high-severity fix, a correct `create_new` + write that fails fast on conflict is the primary goal.

---

## 4. Stage Graph (for Phase 3)

1. **Design & API** — Decide on exact lock acquisition change (`create_new` + error handling) and the tolerance strategy for each of the four run-state functions. Decide whether to enhance lock file format in this phase (timestamp/nonce) or keep format compatible.

2. **Lock hardening** — Rewrite `FinLock::write` to use atomic `create_new`. Update `try_existing` / call sites if needed for clearer errors. Add post-write verification step.

3. **Run state tolerance** — Make `resolve_run_id`, `latest_run_started_at`, `read_run_record`, and `list_runs` corruption-tolerant with clear warnings. Keep NotFound as the clean "absent" path.

4. **Call-site review** — Ensure `plan` / daemon wake paths, `fin runs`, `fin run-status latest`, `fin tail`, and status/overview paths now degrade gracefully instead of hard-failing on a single bad file.

5. **Verification** — Hygiene, manual corruption injection tests (create bad `latest-run`, truncated `status.json`, etc.), confirm happy path unchanged.

---

## 5. Work Items / Scoped Issues for Phase 3

- [ ] Change `FinLock::write` to atomic creation using `OpenOptions::create_new(true)`.
- [ ] Add post-write owner verification after successful lock creation.
- [ ] Update error messages on lock conflict to be clear.
- [ ] Make `resolve_run_id("latest")` and `latest_run_started_at` treat parse/IO errors on the pointer file as "no valid latest run" + warning.
- [ ] Make `read_run_record` (and callers `read_run_record_for`, `list_runs`) skip or gracefully degrade on unparseable `status.json`.
- [ ] Ensure `list_runs` never aborts the whole listing because of one corrupt run directory.
- [ ] Add or improve warnings that tell the user which file is bad and suggest removal.
- [ ] Run full hygiene + manual verification that:
   - Normal lock/unlock still works
   - Concurrent exec attempts produce the expected "already running" error (no double-spawn)
   - A fin with a corrupted `latest-run` or one bad `status.json` still allows `fin runs`, `plan`, `tail`, etc.

---

## 6. Checkpoints & Gates

**Primary Checkpoint (end of Phase 3):**
- `FinLock` acquisition is now race-free via `create_new(true)`.
- A fin whose `latest-run` pointer or `status.json` is corrupted no longer produces raw "failed to read/parse" errors that block `fin runs`, `fin run-status latest`, `fin tail`, or wake planning.
- Instead, the operations either skip the bad data with a clear warning or treat the fin as having "no valid last run".
- All existing valid-run and valid-lock behavior is unchanged.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` pass.

**Gate Criteria:**
- No TOCTOU window remains in the normal `exec_fin_logged` + supervised wake paths.
- Corruption in any single state file is isolated and does not brick the fin's observability surface.
- Warnings are actionable.

**Success Evidence:**
- Manual test: two `fin exec` attempts in quick succession → second gets clean "already running".
- Manual corruption test: `echo "garbage" > .../latest-run` then `orqa fin run-status latest` and `orqa plan` succeed with warning.
- Same for a truncated `status.json` inside a runs/ directory.

---

## 7. Companion Skill Plan

- **Correctness Mode** (primary) — this phase is exactly about eliminating TOCTOU races and turning silent/hard failures on bad state into resilient behavior.
- **Poker-face** — mandatory.
- Possibly light use of `bonsai` if the lock code becomes more complex than necessary.

---

## 8. Gate Plan & Exit Criteria

After implementation + verification:
1. Hygiene passes.
2. Manual race + corruption scenarios pass.
3. Poker-face (self then independent) ≥ 85%.
4. Checkpoint evaluation → commit if PASS.
5. Update ledger, select Phase 4.

---

## 9. Open Questions (Phase 3 Specific)

- Should the lock file format change in this phase (add `started_at=...` or a nonce) or stay byte-for-byte compatible with existing locks? (Recommendation: stay compatible for now; the pid line remains the first line. We can parse extra lines if present.)
- Where should the corruption warnings go? `eprintln!` is acceptable for CLI surface; a future logging framework could be added later.
- Do we want a small helper `fn warn_corrupt_fin_state(fin: &FinRef, file: &Path, detail: &str)` to keep messages consistent?
- Scope of "tolerance": should `fin tail` on a corrupt latest run still succeed by listing available runs, or just say "no valid latest run"?

**Decisions for implementation:**
- Keep lock file format compatible (pid= first).
- Use a small internal `fn tolerant_read_latest_run(...)` helper if it reduces duplication.
- Warnings via `eprintln!` with clear "consider removing" guidance.

---

## 10. Delivery Sign-off

This phase directly attacks the two worst reliability problems in the runtime lock + observability layer. Fixing the TOCTOU prevents data corruption and duplicate execution; making state files tolerant prevents one bad file on disk from making the entire tool unusable for a fin.

The changes are localized to `runtime.rs` (lock) and `runs.rs` (state reading), with clear acceptance criteria.

**Next action:** Mark delivery complete, implement atomic lock + tolerant readers, verify with both happy-path and corruption-injection tests, run poker-face, and commit.

---
*Artifact persisted before any code changes for Phase 3.*