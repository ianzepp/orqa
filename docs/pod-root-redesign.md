# Phase 05 Spec — Pod Root + Global Registry Redesign

**Status:** Planning / Design Spec  
**Phase:** 5 (Next Major Architectural Change)  
**Date Prepared:** 2026-05-15  
**Related:** `feedback-01.md` (provenance friction), Phase 04 existence audit, runtime home model

---

## 1. Motivation

The current architecture treats `ORQA_HOME` (`~/.orqa`) as the owner of all pod and fin state:

```
~/.orqa/pods/<pod>/fins/<fin>/...
```

This creates several structural problems:

- The agent's actual working environment is an artificial mirror inside `~/.orqa` rather than the user's real project folder (git repo, research directory, etc.).
- Creating pods/fins under `~/.orqa/pods/` can be blocked by macOS provenance extended attributes (`com.apple.provenance`) when a background process (previously LaunchAgent, now `orqa loop start`) has touched the tree.
- The mental model is inverted: users must think in terms of "orqa pods" instead of "my project that orqa is helping coordinate."
- Multi-checkout, multi-project, and research workflows feel awkward because the real files live outside the orqa-managed tree.
- `cwd` and `HOME` during fin execution point at synthetic directories, requiring heavy template substitution (`{pod_home}`, `{fin_home}`) and explicit env var overrides.

The goal of this redesign is to flip the relationship:

> A pod is a registration over an existing user-owned directory on disk. Orqa augments that directory with a `.orqa/` data home rather than owning the workspace itself.

---

## 2. Goals (Phase 1)

- A pod root can be any directory on disk (typically a git repository root or project container).
- All Orqa coordination data for a pod lives inside a gitignored `.orqa/` directory at the pod root.
- A global registry at `~/.orqa/config.toml` tracks pod slugs → filesystem locations + basic metadata.
- When working inside (or below) a pod root, Orqa can auto-detect the current pod.
- When a fin executes, `cwd` and `HOME` are set to the **user's real pod root** (the project folder).
- Per-fin isolation for mail, tasks, runs, locks, and runtime state (`.grok/`, `.codex/`, etc.) is preserved in Phase 1.
- The existing `pod.toml` + `fin.toml` model and backend configuration remain largely unchanged.
- `orqa` continues to work from outside any pod (global/operator context).

**Explicit Non-Goal for Phase 1:** Moving runtime state (`.codex/`, `.grok/`, etc.) from per-fin to pod-level. This is deferred to a future phase.

---

## 3. Proposed Filesystem Contract (Phase 1)

### Global Orqa Home

```text
~/.orqa/
  config.toml                 # Registry of known pods + global settings
  operator/                   # Data for operator@ops.orqa (mail, etc.)
  # loop.pid, services/, etc.
```

### Pod Root (User's Real Directory)

```text
~/work/minted-geek-swarm/swarm-api/     # Pod root (user-chosen, arbitrary location)
  .orqa/                                # Gitignored pod data home
    pod.toml
    AGENTS.md
    CHARTER.md
    pod.txt
    fins/
      planner/
        fin.toml
        ROLE.md
        AGENTS.md
        fin.txt
        mail/{new,cur,tmp}/
        tasks/{new,cur,tmp}/
        runs/<run-id>/
        runs.jsonl
        latest-run
        run.lock
        sleep.lock
        .grok/
        .codex/
        .hermes/
        .pi/agent
        .pi/sessions/
      builder/
        ...
  src/
  data/
  notebooks/
  .git/
  ...
```

**Key invariants:**
- `.orqa/` is always a direct child of the pod root.
- `fins/` lives under `.orqa/fins/`.
- All per-fin Orqa artifacts and runtime state directories remain under their respective `fins/<fin>/` (Phase 1).
- The pod root itself becomes the natural `cwd` and `HOME` for executed fins.

---

## 4. Global Registry Schema (`~/.orqa/config.toml`)

The registry is the source of truth for "what pods exist and where they live."

```toml
# ~/.orqa/config.toml

[registry]
version = 1

[pods.swarm-api]
path = "~/work/minted-geek-swarm/swarm-api"
enabled = true
# Optional per-pod overrides (debounce, exec_always, etc. can also stay in pod.toml)

[pods.research-2026]
path = "~/research/agent-evals"
enabled = true

# Special / reserved
[pods.ops]
path = "~/.orqa/operator"   # or special handling
enabled = true
```

- `path` is stored with `~` and expanded at runtime.
- The registry enables `orqa pod list`, cross-pod `ops report`, and the loop daemon to discover pods without scanning the filesystem.
- Pod-level configuration (charter, backends, etc.) remains in the pod's `pod.toml` inside `.orqa/`.

---

## 5. Data Model Changes

### `Orqa` struct and path helpers (`model.rs`)

- `Orqa` continues to represent the *global* orqa home (location of `config.toml`).
- `PodRef` gains (or resolves to) a `root: PathBuf` — the user's chosen pod directory.
- `pod_home(pod)` now returns `{pod.root}/.orqa`
- `fin_home(fin)` returns `{pod.root}/.orqa/fins/{fin}`
- All existing helpers (`mail_home`, `task_home`, `lock_path`, `runs_home`, etc.) are updated relative to the new structure.
- `pod_exists` checks for `{pod.root}/.orqa/pod.toml`
- `fin_exists` checks for `{fin_home}/fin.toml`

A `PodRef` can be constructed in two ways:
1. From a slug + registry lookup (most commands when invoked globally).
2. From auto-detection (walking upward from cwd to find a `.orqa/pod.toml`).

### New / changed concepts

- `current_pod_context()` — attempts to resolve the pod from cwd (nearest `.orqa/pod.toml`).
- Registry loader that returns `HashMap<String, PodRegistration>`.

---

## 6. Pod Detection & Context Rules

When a command is invoked without an explicit pod argument (or when `ORQA_POD` is not set):

1. Walk upward from the current working directory looking for a directory containing `.orqa/pod.toml`.
2. If found, that directory is the pod root. Load `pod.toml` to get the slug (or trust the directory name + registry).
3. If no marker is found, fall back to the global registry + explicit slug (current behavior).
4. Commands that require a pod (most fin/mail/task/loop operations) must either receive a slug or be inside a detectable pod root.

`orqa` (bare) and `orqa pod list` should still work from anywhere and show status across the registry.

---

## 7. Command Surface Changes

### `orqa pod create <slug>`

New semantics (two supported styles):

```sh
# Preferred new style — run inside the target folder
cd ~/work/my-research
orqa pod create my-research

# Explicit path style (still supported)
orqa pod create my-research --path ~/work/my-research
```

Behavior:
- Creates `.orqa/` (with `pod.toml`, `AGENTS.md`, `CHARTER.md`, `fins/`) inside the target directory.
- Registers the pod in `~/.orqa/config.toml` with the absolute (or `~`-prefixed) path.
- If the slug already exists in the registry with a different path, error or prompt.

### New / proposed: `orqa init`

A gentler onboarding command that does the same as `pod create` but with better messaging for first-time users inside a project folder.

### Other commands

- `orqa pod list` — reads the registry and prints status for each registered pod (probes the `.orqa/` location).
- `orqa fin create <pod> <fin>` — when `<pod>` is omitted and inside a pod root, uses the detected pod.
- `orqa loop <pod>` — same detection rules.
- `orqa pod home <slug>` — prints the *real* pod root (not the old `~/.orqa/pods/...` path).
- `orqa fin home <pod> <fin>` — prints the fin directory under `.orqa/fins/<fin>`.

All existing commands (`status`, `doctor`, `tail`, `mail`, `task`, hooks, etc.) must continue to work once the pod root is resolved.

---

## 8. Fin Execution Environment (Phase 1)

When launching a fin (direct `fin exec/chat` or via loop):

```rust
process
    .current_dir(&pod_root)
    .env("HOME", &pod_root)
    .env("ORQA_HOME", global_orqa_home)
    .env("ORQA_POD", &pod.slug)
    .env("ORQA_FIN", &fin.slug)
    .env("GROK_HOME",   fin_home.join(".grok"))
    .env("CODEX_HOME",  fin_home.join(".codex"))
    .env("HERMES_HOME", fin_home.join(".hermes"))
    .env("PI_CODING_AGENT_DIR", fin_home.join(".pi/agent"))
    // ... existing logic
```

This gives the agent natural access to the entire real project tree while still isolating per-fin runtime state and Orqa coordination files.

Auth symlink logic (`runtime_home.rs`) moves from fin creation time to the same `ensure_*_homes` call, but now targets `.orqa/fins/<fin>/.grok/auth.json` etc.

---

## 9. Backward Compatibility & Migration

This is a **breaking change** to the on-disk contract.

Options to evaluate:

- **Dual support (recommended for transition)**: Keep the ability to read old `~/.orqa/pods/<slug>/` layouts for a period. `PodRef` can be "legacy" or "rooted".
- **Migration command**: `orqa migrate` that takes an existing pod slug, asks for (or infers) a target directory, moves the data into `.orqa/` inside that directory, and registers it.
- **Clear error + guidance** when an old-style pod is referenced after the change.

Existing users with data in `~/.orqa/pods/` must not lose their pods, mail history, or run logs.

---

## 10. Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Users lose track of where their pods live | Strong `orqa pod list` + `orqa pod home` UX; registry is human-readable TOML |
| Auto-detection picks the wrong ancestor `.orqa/` | Clear precedence rules + ability to override with `ORQA_POD` env or explicit args |
| Registry and on-disk `.orqa/pod.toml` get out of sync | `pod list` and doctor should detect and report drift |
| `~` expansion and absolute vs relative paths | Always store `~`-prefixed or absolute paths; normalize on load |
| Concurrent access to registry file | Use the same atomic write + read patterns already used for other state files |

---

## 11. Suggested Implementation Roadmap

1. **Registry & data model foundation**
   - Add `config.toml` parsing for the registry.
   - Update `PodRef` / `Orqa` with root path support.
   - Implement `pod_home` / `fin_home` resolution against real roots.

2. **Detection + context**
   - Implement upward walk for nearest pod root.
   - Wire context inference into command handlers.

3. **Command changes**
   - Update `pod create` + registration.
   - Update `pod list`, `pod home`, `fin home`, etc.
   - Make pod/fin arguments optional when inside a detectable pod.

4. **Runtime launch updates**
   - Change `cwd` and `HOME` to pod root while keeping per-fin `*_HOME` overrides.
   - Move auth symlinks and runtime dir creation under the new `.orqa/fins/<fin>/` paths.

5. **Doctor, status, report, hooks, mailbox**
   - Update all path construction and existence checks.

6. **Documentation & help**
   - Update `help.md`, README, templates.
   - Add migration guidance.

7. **Migration tooling** (stretch for initial cut)
   - Basic `orqa migrate` or at least clear documentation + data copy script.

8. **Tests**
   - Update `pod_flow.rs` and integration tests for the new layout.
   - Add tests for detection, registry, and mixed legacy + new pods.

---

## 12. Open Questions (to resolve before implementation)

1. Should we introduce `orqa init` as the primary first-time command inside a project folder, with `pod create` becoming more of a power-user / explicit path tool?
2. Exact precedence when both a registry entry *and* a local `.orqa/` marker exist for the same slug (different paths)?
3. Should `pod.toml` inside `.orqa/` still contain the `slug` field, or is the registry authoritative for the slug?
4. How should `orqa pod doctor` and `orqa pod status` behave for pods whose registered path no longer exists or no longer contains a `.orqa/`?
5. Do we want a `orqa pod register` / `orqa pod unregister` command separate from `create`?
6. Storage of the registry: single `config.toml` vs. a `pods/` directory of small files? (TOML map is simpler for now.)

---

## 13. Success Criteria

- A user can `cd` into any project folder, run `orqa pod create my-project`, and immediately use `orqa fin create planner`, `orqa loop`, mail, tasks, etc., with the agent seeing the real project files as its working tree.
- `orqa pod list` from anywhere shows all registered pods with correct status.
- Existing per-fin isolation, run history, mail/task delivery, and locking continue to work without behavioral regression.
- The provenance creation problem is eliminated for normal project-based usage.
- Documentation clearly explains the new model and how to migrate.

---

This document captures the target shape for the Phase 05 redesign. Once reviewed and approved, it can be turned into detailed task breakdowns and implementation work.