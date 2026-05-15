# Phase 05-3 Delivery Spec — Pod/Fin Creation + Registry Management

**Factory Run:** Pod Root + Global Registry Redesign  
**Phase:** 05-3 of 6  
**Source:** `docs/pod-root-redesign.md` (Section 7 + Section 11 Step 3) + User decision on 2026-05-15  
**Date Prepared:** 2026-05-15  
**Depends On:** Phase 05-1 (registry + data homes) + Phase 05-2 (detection + `resolve_pod_context`)  
**User Decision Recorded:** Use `orqa init` as the primary, friendly onboarding command (following `git init` / `npm init` patterns). `orqa pod create` remains available as a more explicit alternative.

---

## 1. Interpreted Problem

We now have:
- A working global registry (`~/.orqa/config.toml`)
- Reliable detection when the user is inside a folder that already contains `.orqa/pod.toml`

But there is still **no way for a user to create a new pod in the new model**.

The current `orqa pod create <slug>` always creates the pod data under the old location (`~/.orqa/pods/<slug>/`).

Without a creation path that:
- Initializes `.orqa/` inside the *user's current working directory*,
- Registers the mapping in the global config,

the new architecture is not usable end-to-end.

Additionally, `fin create` still requires an explicit pod slug in most cases.

**Goal of Phase 05-3:** Deliver the creation experience that makes the new model feel complete and natural.

---

## 2. Normalized Spec

### Primary Command: `orqa init`

Following the user's decision:

```sh
# Recommended new flow
cd ~/work/my-research-project
orqa init                    # slug defaults to "my-research-project" (if valid)
orqa init my-research        # explicit slug
orqa init --path ~/work/foo  # explicit target directory
```

Behavior of `orqa init`:
- Determines the target directory (current dir, or `--path`).
- Determines the slug (argument, or directory name sanitized to a valid slug).
- Creates the directory structure:
  ```
  <target>/
    .orqa/
      pod.toml
      pod.txt
      AGENTS.md
      CHARTER.md          (seeded with default or --charter)
      fins/
  ```
- Writes a basic `pod.toml` (with `slug`, `default_backend = "codex"`, etc.).
- Registers the pod in `~/.orqa/config.toml` under `[pods.<slug>]` with the correct `path`.
- Prints a friendly success message + next steps (`orqa fin create planner`, `orqa loop`, etc.).
- If a `.orqa/pod.toml` already exists in the target, it should error with a clear message (or offer to re-register).

### `orqa pod create` (kept for compatibility / power users)

- Continues to work with `--path` (new behavior: creates `.orqa/` at the given path + registers).
- When run without `--path` inside a detectable pod, it can warn or error.
- Still accepts `--charter`.

### `fin create` improvements

- `orqa fin create <fin>` should work when inside a detected pod (pod inferred).
- `orqa fin create <pod> <fin>` continues to work explicitly.
- Must call the new `ensure_pod_exists` using the resolved root.

### Registry Writing

- When creating a pod, we must append/update the `[pods.<slug>]` section in `~/.orqa/config.toml`.
- Use atomic-ish write (write to temp + rename) to avoid corrupting the registry.
- If the slug already exists in the registry with a *different* path, warn the user and offer options.

### Non-Goals / Deferred

- Full `orqa migrate` tool (Phase 05-6).
- Making every single `fin/mail/task` command infer the pod in this phase (we can do the high-value ones: `fin create`, `fin list`, `loop`, `plan`).
- Changing the internal structure of `pod.toml` itself.

---

## 3. Repo-Aware Baseline

### Current Creation Logic (to be evolved)

- `pod create` lives in `commands.rs:38-53`.
- It hardcodes `orqa.pod_home(&pod)` which still points to the old `~/.orqa/pods/<slug>` location.
- `fin create` always requires an explicit pod slug today.

### What Phase 05-1 + 05-2 Gave Us

- `PodRegistration`
- `load_registry`
- `pod_data_home(reg)`, `fin_data_home`, etc.
- `detect_pod_context()` + `resolve_pod_context()`

We can now construct a `PodRegistration` from a user directory + slug and use the new data-home methods to create files in the right place.

---

## 4. Stage Graph & Work Breakdown

### Epic 1: `orqa init` Command Surface

1.1 Add `Init` variant to the top-level `Command` enum in `cli.rs` (or under `PodSubcommand` as `Init`).
1.2 Define `InitArgs` (optional slug + optional `--path` + optional `--charter`).
1.3 Wire a new `pod_init(orqa, args)` handler in `commands.rs`.

### Epic 2: Core Creation Logic (reusable)

2.1 Create a helper `create_new_pod(orqa: &Orqa, slug: &str, root: PathBuf, charter: Option<String>) -> Result<(), String>`.
2.2 Inside the helper:
   - Create `<root>/.orqa/fins/`
   - Write `pod.txt`, `pod.toml` (using existing template), `CHARTER.md`, `AGENTS.md`
   - Register in global config (new function `register_pod(orqa, slug, root)`).
2.3 Make the helper usable by both `orqa init` and the updated `pod create`.

### Epic 3: Registry Writing

3.1 Implement `register_pod(orqa: &Orqa, slug: &str, root: PathBuf)` that safely reads, updates, and writes `~/.orqa/config.toml`.
3.2 Handle the case where the slug already exists (with same or different path).
3.3 Keep the file human-readable and nicely formatted.

### Epic 4: `fin create` Inference + `pod create` Updates

4.1 Update `FinSubcommand::Create` to accept optional pod (using `resolve_pod_context`).
4.2 Update `PodSubcommand::Create` to support `--path` and the new creation path + registry registration.
4.3 Keep old behavior working during transition (if someone still has pods under `~/.orqa/pods`).

### Epic 5: Polish & Messages

5.1 Good success messages after `orqa init` ("Pod 'my-research' initialized in /path/to/project. Next: orqa fin create planner").
5.2 Helpful errors when trying to init inside an existing Orqa project or when slug is invalid.

---

## 5. Checkpoints & Verification

**Must pass before commit:**

- `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --locked`
- Manual flow:
  1. `mkdir -p /tmp/demo-project && cd /tmp/demo-project`
  2. `orqa init demo-project` (or just `orqa init`)
  3. Verify `.orqa/pod.toml`, `AGENTS.md`, `fins/` exist
  4. Verify entry appears in `~/.orqa/config.toml` (or temp home)
  5. `orqa fin list` works without arguments
  6. `orqa fin create researcher` works (pod inferred)
  7. `orqa pod list` shows the new pod with correct real path
- `orqa pod create --path /tmp/another --charter "..."` still works and registers correctly.
- Poker-face ≥ 80%

---

## 6. Open Questions for Phase 05-3

1. Should `orqa init` be a top-level command (`orqa init`) or `orqa pod init`?
   - **Current leaning:** Top-level `orqa init` (more discoverable and matches user expectation).

2. Default slug when running `orqa init` with no argument: use current directory name (sanitized), or require the user to always pass a slug?

3. When the directory name is not a valid slug (e.g. contains uppercase or underscores), should `orqa init` auto-sanitize it or force the user to provide one?

4. Should we support re-running `orqa init` on an existing project just to (re)register it in the global config (useful after moving a folder)?

---

## 7. Success Criteria

- A brand new user can follow the mental model:
  ```sh
  cd ~/work/my-cool-project
  orqa init
  orqa fin create planner
  orqa fin create builder
  orqa loop
  ```
  and have everything Just Work with the agent's real files living in `~/work/my-cool-project`.
- `orqa pod list` correctly shows the real path for the newly created pod.
- Both `orqa init` and the updated `orqa pod create --path` write to the registry and create the correct `.orqa/` layout.
- No regression for anyone still using the old layout during the transition period.

---

**This spec must be persisted before implementation of Phase 05-3 begins.**