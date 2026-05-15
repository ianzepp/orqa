# Phase 05-2 Delivery Spec — Pod Detection & Context Inference

**Factory Run:** Pod Root + Global Registry Redesign  
**Phase:** 05-2 of 6  
**Source:** `docs/pod-root-redesign.md` (Section 6 + Section 11 Step 2)  
**Date Prepared:** 2026-05-15  
**Depends On:** Phase 05-1 (PodRegistration + data-home helpers)  
**Target Commit Style:** "Complete Phase 05-2: Pod Detection & Context Inference (pod root redesign)"

---

## 1. Interpreted Problem

One of the biggest UX wins of the new architecture is:

> When the user is inside their real project folder (which contains `.orqa/pod.toml`), most Orqa commands should "just work" without requiring them to type the pod slug every time.

Currently, almost every command that operates on a pod or fin **requires** an explicit pod slug argument (via `SlugArgs`, `FinRefArgs`, `PodStatusArgs`, etc.). There is no concept of "current pod context" derived from the filesystem.

Without reliable detection:
- `cd my-research-project && orqa fin list` will fail or require `orqa fin list my-research-project`.
- `orqa loop` (without pod) has no way to know which pod to run.
- The experience feels like the old model, not the new "orqa lives inside my project" model.

Phase 05-2 must deliver a solid, testable detection mechanism and wire it into the command layer so that later phases (especially creation and runtime) can rely on "if I'm inside a pod, I can infer it."

---

## 2. Normalized Spec

### Functional Requirements

1. **Detection Primitive**
   - Implement a function (e.g. `detect_pod_context() -> Option<(String, PathBuf)>`) that:
     - Starts at `std::env::current_dir()`.
     - Walks upward (parent directories) until it finds a directory that contains a `.orqa/pod.toml` file.
     - Returns the pod slug (read from `pod.toml` or derived from directory name + registry) and the pod root path.
   - The walk must stop at filesystem root and must not follow symlinks in a way that causes infinite loops or security issues.
   - Must be efficient (small number of `exists()` checks).

2. **Precedence Rules (authoritative order)**
   1. Explicit pod slug passed on the command line (highest priority).
   2. `ORQA_POD` environment variable.
   3. Local filesystem detection (nearest ancestor with `.orqa/pod.toml`).
   4. If still unknown and the command requires a pod → produce a clear error suggesting the user either `cd` into the pod or pass the slug explicitly.

3. **Integration Points (commands that should benefit in this phase or immediately after)**
   - `orqa fin list` (when no pod given)
   - `orqa loop <pod>` (pod optional)
   - `orqa plan <pod>`
   - `orqa pod status`, `orqa pod doctor`, `orqa pod tail` (pod optional in many cases)
   - Mail and task commands when operating inside one pod (`orqa mail list`, `orqa task list`, etc.)
   - `orqa fin create <pod> <fin>` — pod can be inferred

4. **Error / UX Quality**
   - When detection fails for a pod-requiring command, the error must be helpful (e.g. "No pod detected in current directory or parents. Run 'orqa pod create <slug>' inside your project, or pass the pod slug explicitly.").

5. **No Behavior Change**
   - When the user is *outside* any pod, behavior must remain exactly as today (explicit slug required).

### Non-Goals for Phase 05-2

- Writing the registry entry (`pod create` still uses the old layout in this phase).
- Changing `pod create` semantics (Phase 05-3).
- Making `ORQA_FIN` auto-detection (future nice-to-have, not required now).
- Full migration of every single command (we can wire the most common ones; the rest can follow in Phase 05-5 audit).

---

## 3. Repo-Aware Baseline

### Current State

- `cli.rs` defines many `*Args` structs that require `pod: String` or `fin: FinRefArgs` (which contains pod + fin).
- `commands.rs` does `PodRef::new(&args.slug)?` very early in almost every handler.
- `main.rs` dispatches based on the parsed `Command` enum — there is no "current context" layer yet.
- `PodRef::new` and `FinRef::new` only validate slug syntax.
- No code anywhere walks the filesystem looking for `.orqa/`.

### Existing Good Patterns

- The previous high-severity factory work already introduced early `ensure_pod_exists` / `ensure_fin_exists` calls right after `PodRef::new`. We can keep that pattern.
- Environment variables `ORQA_POD` and `ORQA_FIN` are already respected in some paths (especially inside launched fins).

### Risk Areas

- Some commands (especially `loop`, `plan`, `ops`) have special parsing (`LoopCommand` has subcommands).
- Detection must not accidentally trigger on a `.orqa/` that belongs to a different tool or an old nested layout.

---

## 4. Stage Graph & Work Breakdown

### Epic 1: Detection Core Logic (model.rs or new small module)

1.1 Add a clean function `pub(crate) fn detect_current_pod() -> Option<(String, PathBuf)>` (or return a small `PodContext` struct).
1.2 Implement safe upward walk (stop at root, handle permission errors gracefully by stopping).
1.3 Read the `pod.toml` (or at minimum check for its existence) to confirm it's a real Orqa pod.
1.4 Decide on slug source for detection: prefer `slug = "..."` inside `pod.toml`, fall back to directory name of the pod root.
1.5 Unit-testable (pure or with injectable `current_dir` for tests).

### Epic 2: Context Resolution Helper

2.1 New helper (probably on `Orqa` or free function) that combines:
   - CLI-provided slug (if any)
   - `ORQA_POD` env
   - Detection result
   → returns a fully resolved `(PodRef with root, or PodRegistration)` or a good error.

2.2 Because Phase 1 kept `PodRef` unchanged, we may return a `(slug, root_path)` pair or a temporary struct that later phases can turn into a proper `PodRef`.

### Epic 3: Wiring into Command Dispatch

3.1 Modify the top-level command handlers in `commands.rs` (or a new `context.rs` module) so that functions like `fin_list`, `loop_pod`, `plan`, etc. can receive an `Option<String>` for pod and resolve it.
3.2 Update the relevant `Args` structs in `cli.rs` to make the pod field `Option<String>` where it makes sense (or keep required and resolve before calling `PodRef::new`).
3.3 Keep backward compatibility: if a pod is explicitly given, it must still work exactly as before.

### Epic 4: Verification

4.1 Add or extend tests (in `tests/pod_flow.rs` or new unit tests) that exercise detection from different cwd locations.
4.2 Manual smoke test: create a temporary project folder with `.orqa/pod.toml`, `cd` into it (or a subdir), and run `orqa fin list`, `orqa loop --dry-run`, etc. without passing the pod name.

---

## 5. Checkpoints & Verification

**Must pass before commit of Phase 05-2:**

- `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --locked`
- Detection works when `cd`'d into the pod root and also when `cd`'d into a subdirectory (e.g. `src/`).
- Detection stops correctly and does not pick up a parent project's `.orqa/` when the user is in a nested unrelated folder.
- When outside any pod, commands that previously required a slug still require it and give the same (or better) error.
- Manual test with a real temp project folder + `.orqa/pod.toml` succeeds for at least `fin list`, `pod status`, and `loop --dry-run` without explicit pod argument.
- Poker-face ≥ 80%

**Gate:** PASS / NEEDS REVIEW / FAIL

---

## 6. Open Questions for Phase 05-2

1. Should detection read the `slug` value from inside `.orqa/pod.toml`, or always trust the directory name of the pod root + cross-check against the registry?
2. Do we want a `ORQA_POD_ROOT` env var override for advanced / testing use cases?
3. How deep should we document the detection rules in `help.md` in this phase (or defer to Phase 6)?
4. Should `detect_current_pod` be cached for the duration of one `orqa` invocation (it usually is, since the process is short-lived)?

---

## 7. Success Criteria for This Phase

- A user can do:
  ```sh
  mkdir -p /tmp/my-new-pod/.orqa
  # (manually create minimal pod.toml for testing)
  cd /tmp/my-new-pod
  orqa fin list          # works, pod inferred
  orqa loop --dry-run    # works, pod inferred
  ```
- All previous explicit-pod usage continues to work unchanged.
- The detection logic is clean, tested, and ready for Phase 05-3 to use when implementing the new `pod create` flow.

---

This spec must be persisted before any code changes for Phase 05-2 are made.