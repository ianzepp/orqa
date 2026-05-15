# Phase 05-4 Delivery Spec — Runtime Launch Environment

**Factory Run:** Pod Root + Global Registry Redesign  
**Phase:** 05-4 of 6  
**Source:** `docs/pod-root-redesign.md` (Section 8 + Section 11 Step 4)  
**Date Prepared:** 2026-05-15  
**Depends On:** Phase 05-3 (creation + registry + fin inference)

---

## 1. Interpreted Problem

After Phase 05-3, a user can do:

```sh
cd ~/work/my-project
orqa init
orqa fin create planner
```

But when the fin actually runs (via `orqa fin exec`, `orqa loop`, etc.), the following still happens:

- `current_dir` is set to the old synthetic fin home under `~/.orqa/pods/...`
- `HOME` is set to the old synthetic fin home
- The agent does **not** see the user's real project files as its natural working environment.

The entire point of the redesign (the agent operating inside the real git repo / research folder) is not yet realized at runtime.

Phase 05-4 must flip the execution environment so that:

- `cwd` = real pod root (e.g. `~/work/my-project`)
- `HOME` = real pod root
- The explicit `GROK_HOME`, `CODEX_HOME`, `HERMES_HOME`, `PI_CODING_AGENT_DIR` still point to the per-fin locations under `.orqa/fins/<fin>/...` (preserving isolation from Phase 1 decision)

This is the moment the new architecture "pays off" for the agent.

---

## 2. Normalized Spec

### Functional Requirements

1. **Launch Environment Change**
   - In `runtime.rs` (`fin_process`) and `doctor.rs` (the probe path):
     - `current_dir` → real pod root
     - `HOME` → real pod root
   - Keep setting the tool-specific overrides to the per-fin state dirs under the new `.orqa/fins/<fin>/` location.

2. **Template Values**
   - `{pod_home}` should resolve to the real pod root (for `--cd {pod_home}` in backend configs).
   - `{fin_home}` and `{home}` should still resolve to the fin's data directory (for mail/task paths etc.).
   - Existing backend templates must continue to work without changes.

3. **Runtime State Creation**
   - `ensure_fin_runtime_homes` (in `runtime_home.rs`) must create `.grok/`, `.codex/`, `.hermes/`, `.pi/...` under the **new** fin location when the pod is a new-style registered pod.
   - Auth symlinks must target the correct new per-fin auth paths.

4. **Backward Compatibility**
   - For any pods still using the legacy `~/.orqa/pods/<slug>` layout, behavior must remain unchanged.

### Non-Goals

- Changing the per-fin isolation model (still per-fin `.grok/` etc. — pod-level sharing is future).
- Updating all backend example configs in `pod.toml` templates (minor doc follow-up ok).

---

## 3. Repo-Aware Baseline

### Key Launch Sites

- `runtime.rs:606` (`fin_process`) — sets `current_dir` + `HOME` + tool homes.
- `doctor.rs:194` — similar setup for the connectivity probe.
- `runtime_home.rs:11` (`ensure_fin_runtime_homes`) — creates the dot-directories and auth symlinks.
- `model.rs` path helpers (now have both old and new data-home variants from previous phases).

### Current Hardcoding

```rust
let fin_home = orqa.fin_home(fin);   // still old path
process.current_dir(&fin_home)
       .env("HOME", &fin_home)
       ...
```

We need to resolve the "effective fin home for execution" based on whether the pod is new-style or legacy.

---

## 4. Stage Graph & Work Breakdown

### Epic 1: Effective Home Resolution

1.1 Add helper(s) in `model.rs`:
   - `effective_fin_home(orqa: &Orqa, fin: &FinRef) -> PathBuf`
   - Or better: given a `PodRegistration` + fin slug, return the correct fin data dir.
   - For legacy pods, fall back to old `fin_home`.

1.2 Make `FinRef` or a new wrapper able to carry root context, or resolve via registry + detection.

### Epic 2: Runtime Launch Updates

2.1 Modify `fin_process` in `runtime.rs` to:
   - Resolve the real pod root (via registry or detection).
   - Set `current_dir(real_pod_root)`
   - Set `HOME = real_pod_root`
   - Set tool homes to the correct per-fin location under `.orqa/fins/<fin>/`

2.2 Same changes in the doctor probe path.

### Epic 3: Runtime Home Creation Updates

3.1 Update `ensure_fin_runtime_homes` + `runtime_home.rs` (link_codex_auth, link_grok_auth, etc.) to accept or resolve the correct target directory for the new-style fin.

3.2 Symlinks and directory creation must land in the right place for both legacy and new pods.

### Epic 4: Verification

4.1 After `orqa init` + `fin create`, running `orqa fin exec` or `orqa loop --dry-run` should show the agent process with `pwd` = real project and `HOME` = real project.
4.2 Tool-specific state (e.g. `GROK_HOME`) still correctly isolated per fin.
4.3 Existing legacy pods continue to work exactly as before.

---

## 5. Checkpoints & Verification

- Clean `fmt + clippy -D warnings + test`
- Manual test:
  1. `orqa init` in a temp project
  2. `orqa fin create planner`
  3. `orqa fin exec planner -- "echo PWD=\$PWD && echo HOME=\$HOME && ls ~"`
  4. Verify PWD and HOME are the real project root, while `.grok/` etc. are created under `.orqa/fins/planner/`
- Legacy pods (under old `~/.orqa/pods`) still launch with old behavior.
- Poker-face ≥ 80%

---

## 6. Success Criteria

- The agent now "lives" inside the user's real project folder when launched by Orqa.
- All previous per-fin isolation guarantees are preserved.
- No breakage for users who have not yet migrated to the new pod model.

---

This phase delivers the core promise of the entire redesign. Once complete, the architecture is functionally usable end-to-end for new projects.