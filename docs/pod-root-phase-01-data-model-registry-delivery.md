# Phase 05-1 Delivery Spec — Data Model + Registry Foundation

**Factory Run:** Pod Root + Global Registry Redesign  
**Phase:** 05-1 of 6  
**Source:** `docs/pod-root-redesign.md` (Section 11, Step 1 + Sections 4 & 5)  
**Date Prepared:** 2026-05-15  
**Target Commit Style:** "Complete Phase 05-1: Data Model + Registry Foundation (pod root redesign)"

---

## 1. Interpreted Problem

The entire redesign rests on a new ownership model:

- `Orqa` instance represents the **global** configuration root (`~/.orqa/` containing `config.toml`).
- A **pod** is now identified by a user-chosen directory on disk (the pod root).
- All pod-specific Orqa data lives under `{pod_root}/.orqa/`.
- The global `~/.orqa/config.toml` contains a registry mapping slug → real path on disk.

Currently (model.rs):
- `Orqa::pod_home(pod)` hardcodes `self.home.join("pods").join(&pod.slug)`
- `FinRef` only stores `{pod: String, fin: String}` (slugs).
- There is no concept of a pod's filesystem root outside the old `~/.orqa/pods/<slug>` convention.
- No registry exists.

Without a clean `PodRef` that can carry or resolve to a real `root: PathBuf`, every subsequent change (detection, creation, runtime launch with real `HOME`, path updates across 19 files) becomes unsafe and inconsistent.

**Goal of Phase 05-1 (Foundation Only):**
Establish the new data model and registry loader so that all later phases have a stable, correct way to answer "where is the `.orqa/` data home for this pod slug on this machine?"

No user-facing commands or runtime behavior changes in this phase.

---

## 2. Normalized Spec

### Functional Requirements

1. **Registry Type**
   - New internal type (or module) `PodRegistration` with at minimum:
     - `slug: String`
     - `path: PathBuf` (expanded, absolute or `~`-preserving)
     - `enabled: bool`
   - Loader function that reads `~/.orqa/config.toml` (or the `--home` override) and returns a map or list of registrations.
   - Graceful handling of missing `config.toml` (treat as empty registry).

2. **PodRef Evolution**
   - `PodRef` must support two construction modes:
     - Slug-only (for legacy or registry lookup)
     - With explicit root (for detected or newly created pods)
   - Add field or associated data: the resolved pod root directory.
   - `PodRef` remains `Copy`-friendly or cheap to clone; root path is the expensive part.

3. **Orqa Path Helpers (new contract)**
   - `orqa.pod_home(&pod: &PodRef) -> PathBuf` returns `{pod.root}/.orqa` (not `~/.orqa/pods/<slug>`)
   - All derived helpers (`fin_home`, `mail_home`, `task_home`, `lock_path`, `runs_home`, `latest_run_path`, `pod_sleep_path`, `pod_hooks_home`, etc.) must resolve correctly once `PodRef` carries the root.
   - `pod_exists(&PodRef) -> bool` checks for `{root}/.orqa/pod.toml`
   - `fin_exists(&FinRef) -> bool` checks for `{fin_home}/fin.toml`
   - `ensure_pod_exists` / `ensure_fin_exists` produce the same friendly messages as before (or updated to mention the real path when helpful).

4. **Registry Schema (v1)**
   ```toml
   [registry]
   version = 1

   [pods.<slug>]
   path = "~/work/my-project"   # required
   enabled = true               # default true
   ```

5. **Backward Compatibility (internal)**
   - Old `~/.orqa/pods/<slug>/...` layout is **not** supported in Phase 1. Dual support / migration is deferred to Phase 05-6.
   - All new path logic assumes the new `.orqa/`-under-root model.

### Non-Goals (explicitly out of scope for Phase 05-1)

- Any change to CLI parsing, `pod create`, or command dispatch.
- Auto-detection / upward walk.
- Runtime launch changes (`HOME`, `cwd`).
- Updates to any of the 19 consuming files beyond `model.rs` (and minimal test helpers).
- `orqa init` command.
- Writing the registry from `pod create` (that happens in Phase 3).
- Legacy path support.

---

## 3. Repo-Aware Baseline

### Current State (as of 2026-05-15)

- [src/model.rs](/Users/ianzepp/work/ianzepp/orqa/src/model.rs): 196 lines. `Orqa { home: PathBuf }`, `PodRef { slug }`, `FinRef { pod, fin }`. All paths hardcoded under `self.home.join("pods")`.
- `default_home()` uses `$HOME/.orqa` or `ORQA_HOME`.
- `PodRef::new` / `FinRef::new` only do slug validation.
- Config loading exists in [src/config.rs](/Users/ianzepp/work/ianzepp/orqa/src/config.rs) (`read_toml`, `pod_config_template`, etc.) but only for `pod.toml` / `fin.toml` inside a pod, not the global registry.
- No `config.toml` at the global `Orqa` home level yet (only `pod.toml` per pod).
- `ensure_*_exists` helpers were added in the previous high-severity factory run and live in `model.rs`.

### Good Patterns to Preserve / Extend

- `validate_slug` logic.
- The friendly error format from `ensure_pod_exists` / `ensure_fin_exists`.
- `Orqa::new(home: Option<PathBuf>)` already respects `--home` and `ORQA_HOME`.

### Files That Will Eventually Depend on This Phase

`model.rs` is the single source of truth for path construction. Once Phase 1 lands cleanly, Phases 2–5 become mechanical updates.

---

## 4. Stage Graph & Work Breakdown (for this phase only)

### Epic 1: Registry Representation & Loading

1.1 Add `PodRegistration` struct (or use a simple map).
1.2 Implement `load_registry(orqa: &Orqa) -> Result<BTreeMap<String, PodRegistration>, String>`.
1.3 Support `[registry] version = 1` and `[pods.<slug>]` table.
1.4 Handle missing file / empty registry as valid empty state.
1.5 Unit test the loader with temp TOML files.

### Epic 2: PodRef with Root

2.1 Extend `PodRef` (or introduce `PodHandle` / keep `PodRef` and add `root: Option<PathBuf>` + resolution method).
   - Preferred: Keep `PodRef` simple (slug only) and add a separate way to attach root, or make `PodRef` carry the root when known.
   - Decision needed: `PodRef { slug, root: PathBuf }` (root always present after construction) vs lazy resolution.

2.2 Update `PodRef::new` constructors (keep slug-only for registry lookup use cases).
2.3 Add `with_root(self, root: PathBuf) -> Self` or equivalent.
2.4 Update `FinRef` if needed (it can stay slug-based; root comes via its pod).

### Epic 3: Path Helper Migration in model.rs

3.1 Rewrite every `*_home` method to use `pod.root.join(".orqa")` instead of `self.home.join("pods").join(slug)`.
3.2 Update `pod_exists`, `fin_exists`, `ensure_*_exists` implementations.
3.3 Keep the public signatures identical so downstream code does not break in this phase.
3.4 Add `pod_root(&self, pod: &PodRef) -> PathBuf` helper for clarity.
3.5 Ensure all template values (`{pod_home}`, `{fin_home}`, `{home}`) will still be correct once callers pass proper `PodRef`s (no change needed in this phase).

### Epic 4: Basic Tests & Hygiene

4.1 Add or extend tests in `config_test.rs` / new `model_test.rs` (or keep in existing test files) that exercise registry loading and path construction for external roots.
4.2 Verify that `cargo test` still passes with zero behavior change for the old (now unused in Phase 1) path logic.

---

## 5. Checkpoints & Verification

**Must pass before commit:**

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --locked`
- Manual inspection of generated paths for a synthetic registry entry (e.g. pod "demo" at `/tmp/demo-project` → `/tmp/demo-project/.orqa/fins/planner/mail`).
- No regressions in existing tests that create pods/fins under a temp `--home` (they will continue to use the old layout until later phases wire the new creation path).
- Poker-face completion estimate ≥ 80% (this is foundational; later phases will validate end-to-end).

**Gate Decision (recorded in ledger):** PASS / NEEDS REVIEW / FAIL

---

## 6. Open Questions Specific to Phase 05-1

1. **PodRef shape**: Should `PodRef` always require a root after construction in the new world, or keep a "slug-only" form that requires explicit registry lookup to become usable? (Recommendation: allow slug-only for registry lookup, then attach root. Make root required for any path-producing operation.)

2. **Registry module location**: Put registry types and loader in `model.rs`, a new `registry.rs`, or inside `config.rs`? (Recommendation: small amount of code → start in `model.rs`; extract later if it grows.)

3. **Path storage in registry**: Store `~` literally and expand at load time, or always store absolute paths? (Recommendation: store user-friendly `~` form, expand on load, keep original for display in `pod list`.)

4. **Versioning**: Do we need to handle `version = 0` or future versions gracefully in Phase 1? (Keep simple: require version 1 or absent → treat as v1.)

---

## 7. Companion Skill Plan

- None required for Phase 1 (pure model + config loading).
- `bonsai` or `review` can be used at the end of implementation for polish if the diff is large.

## 8. Success Criteria for This Phase

- A `PodRef` constructed with a real root produces correct `.orqa/fins/...` paths via all existing helper methods.
- Registry can be loaded from a `config.toml` containing one or more `[pods.*]` entries.
- All existing unit tests continue to pass.
- The foundation is solid enough that Phase 05-2 (detection) and Phase 05-3 (creation) can be built on top without further changes to the path model.

---

**This spec must be persisted and the ledger updated before any code changes for Phase 05-1 begin.**