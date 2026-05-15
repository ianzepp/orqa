# Phase 02 Delivery Spec — Daemon Prompt Argument Forwarding

**Factory Run:** High-Severity Correctness Fixes  
**Phase:** 2 of 4  
**Source Issue:** HS #2 from `docs/bugs.md`  
**Date Prepared:** 2026-05-15

---

## 1. Interpreted Problem

`orqa loop start -- "handle your open Orqa mail and tasks"` (and similar documented patterns) does not work after the v0.6 `loop` command restructure.

**Root cause chain:**
1. `LoopStartArgs` in [src/cli.rs](/Users/ianzepp/work/ianzepp/orqa/src/cli.rs) correctly captures everything after `--` into `args: Vec<OsString>` via `#[arg(last = true)]`.
2. `loop_start()` in [src/commands.rs:427-429](/Users/ianzepp/work/ianzepp/orqa/src/commands.rs) passes those args as additional command-line arguments to the spawned child: `cmd.arg(arg)`.
3. The child process (the daemon) executes the normal `main()` entrypoint: it does a full `Cli::command().get_matches()` + `from_arg_matches` **before** checking `env::var("ORQA_DAEMON")`.
4. In the daemon branch ([src/main.rs:50-56](/Users/ianzepp/work/ianzepp/orqa/src/main.rs)), `LoopRunArgs` is **hardcoded** with `args: vec![]`, completely ignoring any prompt text the user supplied.
5. Any extra positional arguments the parent tried to forward either cause an immediate clap parse error in the child (child exits, daemon fails silently) or are dropped.

**Impact:** The single most useful daemon invocation pattern (`loop start -- "custom system prompt for every wake"`) is broken. The daemon runs, but every scan uses an empty prompt vector, so the AI never receives the user's custom instructions.

The review also notes that `help.md` still contains stale references to the removed `--forever` flag and old invocation patterns.

---

## 2. Normalized Spec

### Functional Requirements

1. `orqa loop start -- "prompt text here"` (and multiple arguments after `--`) must result in the exact same `Vec<OsString>` being available on **every** wake scan inside the daemon as if the user had run `orqa loop run -- "prompt text here"`.

2. The prompt arguments must be forwarded via an environment variable (recommended name: `ORQA_LOOP_ARGS`) rather than as top-level CLI arguments to the child process. This prevents the child's early clap parse from ever seeing user prompt text.

3. In the daemon branch of `main.rs`, reconstruct a `LoopRunArgs` that contains:
   - `pod: None` (scan all pods, as today)
   - `force` from `ORQA_FORCE`
   - `dry_run: false`, `json: false`
   - `args: Vec<OsString>` deserialized from the `ORQA_LOOP_ARGS` env var

4. The parent `loop_start` must **stop** appending the user `args` as `.arg()` values on the child `Command`. Only `--home` (and the daemon control env vars) should be on the argv of the child.

5. Backward compatibility: `orqa loop start` (with no `--` arguments) must continue to work exactly as today (empty prompt vector).

6. Update stale documentation in `src/help.md` that references the removed `--forever` flag or old `loop run --forever` patterns.

### Non-Goals (out of scope for Phase 2)

- Full daemon lifecycle hardening (pidfile atomicity, SIGTERM handling, Windows liveness) — these are part of HS #5 (medium severity) and Phase 3/4 scope.
- Changing how the prompt args are *used* inside `resolve_exec_command` / `plan_pod` — only the forwarding mechanism.
- Adding a `--pod` flag to `loop start` (current design is "all pods"; per-pod daemons are out of scope).
- Any changes to `LoopPlanArgs`.

### Error / Edge Cases to Handle Gracefully

- Malformed `ORQA_LOOP_ARGS` in the env var → fall back to empty vec with a warning log (do not crash the daemon loop).
- Non-UTF-8 in prompt args (rare for this use case) → use `to_string_lossy()` for JSON roundtrip.

---

## 3. Repo-Aware Baseline

### Relevant Code Locations

- **CLI definitions**: [src/cli.rs:275-287](/Users/ianzepp/work/ianzepp/orqa/src/cli.rs) (`LoopStartArgs`), [src/cli.rs:245-260](/Users/ianzepp/work/ianzepp/orqa/src/cli.rs) (`LoopRunArgs` — note both use `last = true` for their `args` field).
- **Parent spawning logic**: [src/commands.rs:402-440](/Users/ianzepp/work/ianzepp/orqa/src/commands.rs) (`loop_start`).
- **Daemon entrypoint**: [src/main.rs:38-76](/Users/ianzepp/work/ianzepp/orqa/src/main.rs) (the `if env::var("ORQA_DAEMON")` block).
- **Consumption of prompt args**: [src/runtime.rs:115](/Users/ianzepp/work/ianzepp/orqa/src/runtime.rs) (`plan_pod(..., &args.args)`), then passed to `resolve_exec_command` and `backend_command_for` in [src/config.rs](/Users/ianzepp/work/ianzepp/orqa/src/config.rs) (the values are expanded into the final backend command line as the user's custom prompt).
- **Documentation**: [src/help.md:448,458](/Users/ianzepp/work/ianzepp/orqa/src/help.md).

### Existing Patterns to Follow

- The project already uses environment variables for daemon control (`ORQA_DAEMON`, `ORQA_INTERVAL`, `ORQA_FORCE`).
- `serde_json` is already a dependency (used in runs.rs for `RunRecord` serialization).
- `OsString` handling for user-supplied prompt text is already present in `Loop*Args`.

### Design Choice: Serialization Format for `ORQA_LOOP_ARGS`

Use a simple JSON array of strings:

- Parent: `serde_json::to_string(&args.args.iter().map(|s| s.to_string_lossy().into_owned()).collect::<Vec<String>>())`
- Child: `serde_json::from_str::<Vec<String>>(&value).map(|v| v.into_iter().map(OsString::from).collect()).unwrap_or_default()`

This is robust, human-readable in `ps`/`env`, and easy to debug. No new dependencies.

Alternative (shell-escaped single string) is more fragile across platforms; JSON wins.

---

## 4. Stage Graph (for Phase 2)

Because this is a narrow, well-isolated fix:

1. **Design** — Decide on env var name (`ORQA_LOOP_ARGS`), serialization (JSON array of strings), and exact reconstruction of `LoopRunArgs` in the daemon branch.
2. **Parent change** (`loop_start` in commands.rs) — Remove the `for arg in &args.args { cmd.arg(arg) }` loop. Add the JSON env var instead. Keep all existing daemon control env vars.
3. **Child change** (`main.rs` daemon branch) — Read `ORQA_LOOP_ARGS`, deserialize, build a proper `LoopRunArgs { args: ..., ... }` instead of the hardcoded empty one. Add graceful fallback on parse error.
4. **Docs** — Remove the stale `--forever` example(s) from `help.md`.
5. **Verification** — Build, run clippy/fmt/test, manually test `loop start -- "test prompt"` (observe that the prompt reaches `plan_pod` / backend command construction on the first wake), test plain `loop start` (no args).

No parallel workstreams.

---

## 5. Work Items / Scoped Issues for Phase 2

- [ ] Add `ORQA_LOOP_ARGS` serialization in `loop_start` (commands.rs) and stop passing user args via `.arg()`.
- [ ] In the daemon loop in `main.rs`, read and deserialize `ORQA_LOOP_ARGS` (with fallback to `vec![]` + warning on corruption).
- [ ] Construct `LoopRunArgs` using the deserialized args (plus the existing `force` / interval / etc. logic).
- [ ] Remove or update the two stale `--forever` / old pattern examples in `src/help.md`.
- [ ] Verify that a daemon started with a custom prompt actually supplies that prompt vector to `resolve_exec_command` on each iteration (can be observed via dry-run or by adding a temporary log; manual test is sufficient).
- [ ] Ensure `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` still pass.

---

## 6. Checkpoints & Gates

**Primary Checkpoint (end of Phase 2):**
- `orqa loop start -- "You are a meticulous email and task triage agent. ..."` launches a daemon.
- On the first wake scan (and subsequent scans), the custom prompt text is visible in the arguments passed to the backend (can be verified by `ORQA_HOME=... strace -e execve -p <daemon-pid>` or by temporarily instrumenting `resolve_exec_command`, or by using a test backend that echoes the prompt).
- `orqa loop start` (no extra args) continues to work.
- No clap parse errors appear in the child related to prompt text.
- Stale `--forever` references removed from help output.
- All repo hygiene checks pass.

**Gate Criteria:**
- The prompt text supplied at `loop start` time is demonstrably present in the `args` field of the `LoopRunArgs` used by `loop_pod` / `plan_pod` inside the daemon.
- No behavior change for daemons started without a custom prompt.

**Success Evidence:**
- Before/after manual test of the documented `loop start -- "..."` pattern.
- `git diff` limited to main.rs, commands.rs, cli.rs (if any), and help.md.

---

## 7. Companion Skill Plan

- **Correctness Mode** — primary (reliability of daemon prompt forwarding is a correctness issue).
- **Poker-face** — required before checkpoint.
- Minimal scope, so no heavy use of `bonsai` or `carmack-linus` unless the serialization code looks ugly.

---

## 8. Gate Plan & Exit Criteria

After implementation + verification:
1. Run full hygiene (`fmt --check`, `clippy -D warnings`, `test`).
2. Manual reproduction of the broken pattern + verification that the prompt now reaches the AI path.
3. Poker-face evaluation (self-estimate first, then independent).
4. If ≥85% and gate passes → commit with message "Complete Phase 02: Daemon Prompt Argument Forwarding".
5. Update factory ledger and proceed to Phase 3 (or pause).

If poker-face < 85% or gate fails: return to implementation.

---

## 9. Open Questions (Phase 2 Specific)

- Should we also support a `pod` selector for the daemon (e.g. `loop start --pod mypod -- "prompt"`)? (Current design and `LoopRunArgs` in daemon hardcode `pod: None`; out of scope for this phase per the original review.)
- Do we want to log (at debug level) the prompt args the daemon is using on first wake? (Nice for UX, but not required for the fix.)
- Exact env var name: `ORQA_LOOP_ARGS` (matches the suggestion in bugs.md) or `ORQA_PROMPT_ARGS`? `ORQA_LOOP_ARGS` is preferred for consistency with the other `ORQA_*` daemon vars.

**Resolved:** Use `ORQA_LOOP_ARGS`.

---

## 10. Delivery Sign-off

This phase is small, high-value, and directly repairs the documented broken user workflow for the daemon. The change is localized to the daemon launch + consumption path and follows the exact recommendation in the correctness review.

**Next action:** Mark delivery complete, implement the env-var forwarding + docs update, verify, poker-face, and commit.

---
*Artifact persisted before code changes for Phase 2.*