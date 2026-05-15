# Orqa Correctness & Robustness Findings

**Date:** 2026-05-15  
**Review Method:** Two independent deep read-only subagent reviews (one focused on CLI/user-facing command surface, one on runtime/daemon/scheduling/storage/lifecycle) + project hygiene checks (`cargo fmt --check` + `cargo clippy --all-targets -- -D warnings`).  
**Rubric:** Informed by `housekeeping/references/correctness-rs.md` (error handling integrity, invariants, TOCTOU races, missing guard clauses, silent drops, data flow, resource management, and architectural violations).

---

## Executive Summary

The orqa codebase is generally sound:
- No `.unwrap()` or `.expect()` on user-controlled paths or IO in non-test production code.
- Consistent slug validation via `PodRef`/`FinRef`.
- Good tolerance in some paths (`list_dirs` treats missing directories as empty; `unread_count` and several maildir helpers are resilient).
- Daemon lifecycle has explicit stale-pidfile handling and liveness self-checks.
- Error messages usually include paths.

**Primary systemic weakness:** Defensive existence checks and high-quality, actionable error messages are applied **inconsistently**. Many code paths assume pods, fins, `fins/`, `pod.toml`, `fin.toml`, `latest-run`, `run.lock`, etc. already exist, then fall through to raw `fs::read_dir`, `read_toml`, or `print_file` failures. This produces exactly the class of confusing low-level error the user encountered (`failed to read fins directory ... No such file or directory`).

The `loop` command tree restructure (v0.6) amplified several of these gaps because old invocation patterns now produce poor errors, and new paths (daemon args, `loop run` without pod, etc.) were not fully hardened.

---

## High-Severity Issues

### 1. Raw filesystem errors on non-existent or partial pods/fins ("failed to read fins directory")

**Severity:** High  
**Locations:**
- `src/runtime.rs:153-160` (`plan_pod` — used by `loop run <pod>`, `loop plan`, and daemon `loop_single_pod`)
- `src/runs.rs:309-316` (`tail_pod` — `orqa pod tail`)
- `src/commands.rs:58,155,235` and many others (charter/role get/set, fin tail/runs/run-status, etc.)
- `src/config.rs:293-299` (`read_toml` used by backend resolution)

**What's wrong:** `plan_pod`, `tail_pod`, and numerous command handlers perform unconditional `fs::read_dir` or `read_to_string` on `pods/<slug>/fins`, `fin.toml`, `CHARTER.md`, etc., without first verifying the pod or fin home exists. Only `mailbox::ensure_target_fin` (which checks for `fin.toml`) produces a friendly message.

**Why it matters:**  
- `orqa loop run nonexistent`, `orqa loop plan bad-pod`, `orqa pod tail bad-pod`, `orqa fin exec ghost/ghost`, `orqa pod charter get missing` all emit raw OS errors.
- Users doing the most common "slightly wrong" things (typo, running before `pod create`/`fin create`, manual cleanup, old post-restructure invocation) get unhelpful output.
- In daemon mode these errors are logged but the loop continues with partial data.

**Fix suggestion:**  
Introduce `ensure_pod_exists(orqa, &PodRef)` and `ensure_fin_exists(orqa, &FinRef)` helpers that check for the canonical file (`pod.toml` / `fin.toml`) or home directory and return a clear message including the suggested `orqa pod/fin create` command.  
Use `list_dirs` (or a wrapper that returns empty on missing `fins/`) in `plan_pod` and `tail_pod` so a pod without a `fins/` directory is treated gracefully (empty plan / no output) instead of hard error. Apply the helpers before `read_toml` / `print_file` paths.

### 2. `orqa loop start -- "prompt..."` arguments are silently dropped or cause child parse failure

**Severity:** High  
**Locations:**
- `src/main.rs:50-56` (daemon branch hardcodes `LoopRunArgs { args: vec![] }`)
- `src/cli.rs:284-286` (`LoopStartArgs.args` with `last = true`)
- `src/commands.rs:411-427` (args appended to child argv)
- `src/help.md:448,458` (still documents old `--forever` and prompt patterns)

**What's wrong:** The daemon child process performs full `Cli::get_matches()` + `from_arg_matches` *before* the `ORQA_DAEMON` environment check. Extra arguments after `--` therefore either cause an immediate clap parse error (child exits) or are dropped entirely. The daemon never receives the prompt args that were intended for every wake scan.

**Why it matters:** `orqa loop start -- "handle your open Orqa mail and tasks"` — one of the documented and most useful daemon patterns — does not work after the restructure. This is the exact friction reported in `docs/feedback-01.md`.

**Fix suggestion:**  
Pass extra prompt arguments via an environment variable (e.g. `ORQA_LOOP_ARGS`) that the daemon branch reconstructs into `LoopRunArgs`. Alternatively, do a minimal pre-parse for daemon mode before full clap dispatch. Update `help.md` to remove references to the removed `--forever` flag.

### 3. Non-atomic run lock acquisition (TOCTOU) + poor handling of corrupted `run.lock` / `latest-run` / `status.json`

**Severity:** High  
**Locations:**
- `src/runtime.rs:486-495, 650-659` (FinLock `try_existing` + write-after-spawn)
- `src/runs.rs:246-262, 348-354` (`resolve_run_id("latest")`, `list_runs`, `read_run_record_for`)
- `src/runtime.rs:211` (`latest_run_age` strict `?` on non-NotFound errors)

**What's wrong:** Lock acquisition checks for existence then writes; there is a window allowing two backends for the same fin. Corrupted `run.lock`, `latest-run`, or `status.json` files cause hard failures in planning, `fin runs`, `fin run-status latest`, `fin tail`, etc., instead of graceful "stale/corrupt state — consider removing" behavior.

**Why it matters:**  
- Concurrent `orqa loop` (manual + daemon) + `fin exec` can produce duplicate AI runs, overwritten locks, and corrupted run records.
- A single bad file (from crash, manual edit, or disk issue) bricks wakes or observability for an entire fin.
- Pid reuse edge cases on `kill -0` can cause skipped wakes or double execution.

**Fix suggestion:** Use `OpenOptions::create_new(true)` + post-write owner verification for locks. Make `resolve_run_id` / `list_runs` treat corruption as "no last run" (with a warning) rather than a hard error. Consider storing a nonce or start timestamp in the lock file for stronger liveness checks.

### 4. Inconsistent pod/fin existence validation across the call graph

**Severity:** High  
**Locations:** ~60 call sites of `PodRef::new` / `FinRef::new` (commands.rs, status.rs, doctor.rs, hooks.rs, report.rs, runtime.rs, mailbox/, config.rs, etc.). Only `ensure_target_fin` (mailbox/mod.rs:154) and `pod create`/`fin create` paths perform meaningful existence work.

**What's wrong:** Most paths (status, doctor for specific fins, exec, chat, tail, runs, charter/role get/set, hook operations, overview after manual tampering) assume the pod/fin was previously created via the CLI. `fin create` can succeed without a prior pod. `list_dirs` discovery in overview/`list_pods`/`loop run` (all pods) can feed invalid directory names into `PodRef::new`, causing hard failures.

**Why it matters:** Users (and the daemon) encounter a mix of friendly errors, raw IO errors, and silent zero results depending on which command they use. `orqa` (bare, the new dashboard) and `orqa pod list` are tolerant; `orqa fin status ghost/ghost` or `orqa loop run` on a deleted pod are not.

**Fix suggestion:** Centralize `pod_exists` / `fin_exists` (or `ensure_*`) helpers that check for the canonical TOML file. Use them early in command handlers, status, doctor, exec, and plan paths. In discovery loops (overview, list_pods, all-pods loop), skip or warn on `PodRef::new` failure instead of propagating the error.

---

## Medium-Severity Issues

### 5. Daemon lifecycle races and incomplete state transfer

- Pidfile written *after* spawn with no atomic rename/fsync and no kill of the child if the write fails (`commands.rs:424`).
- `is_process_running` logic duplicated between `commands.rs:507` and `runtime.rs`.
- Daemon branch in `main.rs` has no graceful SIGTERM handler and always uses `args: vec![]`.
- Windows liveness is weak (`always true` if pidfile exists).

**Impact:** Orphan daemons, undetectable "running" processes after external kill, takeover races, and confusing `loop status`/`loop stop` behavior.

### 6. Error messages and partial failure handling are inconsistent in quality

- Many `read_toml`, `print_file`, and `resolve_run_id` paths produce raw "failed to read X: ..." strings.
- `doctor.rs` is relatively good at explicit checks; `plan_pod`, direct `fin exec`, and several status paths are not.
- Hooks and auth symlink creation use silent `let _ =` or best-effort behavior (`runtime_home.rs`).

**Impact:** Users see a spectrum from excellent ("target fin does not exist") to terrible (raw OS error + full path) depending on the operation.

### 7. `LoopPlanArgs.pod` is required while `LoopRunArgs.pod` is optional; help text and error guidance lag the restructure

- Old `orqa loop <pod>` invocations now produce "unrecognized subcommand" or treat the subcommand name as a pod slug.
- `orqa loop` (no subcommand) shows good help, but the transition from the pre-0.6 grammar is not explained anywhere in error messages or docs.

---

## Lower-Severity / Maintenance Items

- Duplicate `list_dirs` implementations (`commands.rs:268` vs `report.rs:208`).
- `ensure_maildir` error messages are imprecise.
- `latest-run` pointer can dangle after run directory deletion; `resolve_run_id("latest")` fails hard.
- Best-effort auth symlinks in `runtime_home.rs` can leave broken symlinks or fail silently on permission changes.
- No central place documents the assumption "at most one concurrent backend per fin."

---

## Positive Observations (for context)

- Zero production `unwrap`/`expect` on IO or user input.
- `list_dirs` and several maildir helpers are defensively written.
- Daemon has good (if not perfect) stale-pidfile and liveness logic.
- The wake plan / decision model (`WakeDecision`, `WakeReason`) is clean and observable.
- Hooks are intentionally non-fatal to the main loop (correct design).

---

## Recommended Next Steps

1. Implement `ensure_pod_exists` / `ensure_fin_exists` + friendly error wrappers (highest leverage).
2. Fix daemon prompt argument forwarding via environment variable.
3. Harden `FinLock` acquisition (atomic create_new + verify) and make run record / latest-run resolution tolerant of corruption.
4. Audit and normalize existence checks across the ~60 call sites.
5. Add a small integration test that exercises "non-existent pod/fin" error paths for the most common commands.

This document was generated from the May 2026 correctness review and can be updated as issues are fixed.

---

*Review performed against the state of the repository after the bare-`orqa` dashboard feature (commit d43bb9b) and the associated hygiene/formatting pass.*