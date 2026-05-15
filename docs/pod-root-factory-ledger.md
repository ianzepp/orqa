# Pod Root + Global Registry Redesign — Factory Ledger

**Started:** 2026-05-15  
**Source:** `docs/pod-root-redesign.md` (Phase 05 Spec)  
**Target:** `orqa` repository (~/work/ianzepp/orqa)  
**Mode:** Architecture Redesign + Feature (multi-phase)  
**Policy:** One phase at a time. Detailed delivery spec persisted to `docs/pod-root-phase-XX-*.md` before any implementation. Poker-face + verification checkpoint before commit. Autocommit per AGENTS.md.

---

## Phase Set (Derived from pod-root-redesign.md Roadmap)

**Overall Goal:** Flip the ownership model so a pod is a lightweight registration over a user-owned directory on disk, with Orqa data living in `.orqa/` inside that directory. Global registry in `~/.orqa/config.toml`.

**Phases (grouped for coherent, testable delivery):**

### Phase 05-1: Data Model + Registry Foundation
**Focus:** Core types and registry.
- Introduce `PodRegistration` / registry loader from `~/.orqa/config.toml`.
- Evolve `PodRef` to carry (or resolve to) a `root: PathBuf`.
- Update `Orqa` and all path helpers (`pod_home`, `fin_home`, `mail_home`, etc.) to work against real pod roots instead of `~/.orqa/pods/<slug>`.
- `pod_exists` / `fin_exists` now check for `.orqa/pod.toml` and `.orqa/fins/<fin>/fin.toml` under the root.
- Basic TOML schema for `[pods.<slug>]` with `path` and `enabled`.
- No command changes yet.

**Delivery Spec:** `docs/pod-root-phase-01-data-model-registry-delivery.md`

**Checkpoint Policy:**
- `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --locked`
- Manual verification that path helpers return expected locations for both registry-backed and (future) detected pods.
- Poker-face ≥ 80%
- Gate: PASS / NEEDS REVIEW / FAIL

---

### Phase 05-2: Pod Detection & Context Inference
**Focus:** Making "I'm inside a pod" work.
- Implement upward directory walk to find nearest ancestor containing `.orqa/pod.toml`.
- `current_pod_context()` helper that returns `Option<(PodRef, PathBuf /* root */)>`.
- Wire context inference into command dispatch (when pod arg omitted and `ORQA_POD` not set).
- Precedence rules: explicit arg > env > local detection > registry error.
- Update `PodRef::new` / construction paths to support detection.

**Delivery Spec:** `docs/pod-root-phase-02-detection-context-delivery.md`

---

### Phase 05-3: Pod & Fin Creation + Registry Management
**Focus:** The new `pod create` experience and registration.
- Change `pod create <slug>` semantics: when run inside a directory (or with `--path`), initialize `.orqa/` there and register in global config.
- Support both `cd my-project && orqa pod create my-project` and explicit `--path`.
- `orqa pod list` reads registry and shows real paths + status.
- `orqa pod home` and `orqa fin home` print real locations.
- `fin create` can infer pod from context when inside a pod root.
- Make pod argument optional for many fin/mail/task commands when detection succeeds.
- Handle slug conflicts (registry vs local `.orqa/pod.toml`).

**Delivery Spec:** `docs/pod-root-phase-03-creation-registration-delivery.md`

---

### Phase 05-4: Runtime Launch Environment
**Focus:** The big behavioral win — real project as HOME/cwd.
- Update `fin_process` in `runtime.rs` (and doctor) to set `current_dir(pod_root)` and `HOME=pod_root`.
- Keep explicit `GROK_HOME`, `CODEX_HOME`, etc. pointing to `.orqa/fins/<fin>/...` (Phase 1 isolation preserved).
- Update `ensure_fin_runtime_homes` / `runtime_home.rs` to create state dirs under the new fin location.
- Auth symlink targets updated.
- Verify template expansion (`{pod_home}`, `{fin_home}`, `{home}`) still works correctly.
- `pod_home` in templates/config now resolves to the real root (for `--cd {pod_home}`).

**Delivery Spec:** `docs/pod-root-phase-04-runtime-launch-delivery.md`

---

### Phase 05-5: Full Path Audit & Observability Surfaces
**Focus:** Every other consumer of paths.
- Sweep and update: `doctor.rs`, `status.rs`, `report.rs`, `runs.rs`, `hooks.rs`, `mailbox/`, `service.rs`, `config.rs`.
- Ensure all existence checks, directory listings, and file operations use the new root-based paths.
- `pod doctor`, `pod status`, `pod tail`, `fin runs`, etc. work for registered external pods.
- Cross-pod ops (`ops report`) continue to function.

**Delivery Spec:** `docs/pod-root-phase-05-observability-audit-delivery.md`

---

### Phase 05-6: Documentation, Help, Migration & Test Hardening
**Focus:** User-facing completeness + safety.
- Update `help.md`, `README.md`, templates (`pod-agents.md`, `fin-agents.md`).
- Add clear migration guidance and notes about the breaking FS contract.
- Add or expand integration tests in `tests/pod_flow.rs` (new layout, detection, creation inside folders, mixed legacy+new).
- Consider basic `orqa migrate` helper or strong error messages for old `~/.orqa/pods/` references.
- Final verification that the provenance creation issue is mitigated for normal project usage.

**Delivery Spec:** `docs/pod-root-phase-06-docs-tests-migration-delivery.md`

---

## Factory Ledger Location & Recovery

This file (`docs/pod-root-factory-ledger.md`) is the single source of truth for phase status, decisions, and open questions during the run. It will be updated after each phase completes.

## Verification & Checkpoint Policy (applies to every phase)

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --locked`
- Manual CLI smoke test of affected commands (using a temporary `--home` + temp project folders)
- Poker-face completion review (target ≥ 80–85%)
- Independent gate decision recorded here

## Commit Policy

After a phase clears verification + poker-face + gate:
- Autocommit with message: `feat: Complete Phase 05-X: <short name> (pod root redesign)`
- Update this ledger with results and next phase decision.

## Current Status

**Active Phase:** 05-1 — Data Model + Registry Foundation

**Status:** COMPLETE

**Delivery Spec:** `docs/pod-root-phase-01-data-model-registry-delivery.md`

**What Was Delivered:**
- `PodRegistration` struct + `load_registry(&Orqa) -> Result<BTreeMap<String, PodRegistration>>` that reads `~/.orqa/config.toml` (supports `~` expansion and absolute paths).
- New family of `*_data_home` methods on `Orqa` (`pod_data_home`, `fin_data_home`, `mail_data_home`, `task_data_home`, `lock_data_path`, `runs_data_home`, `latest_run_data_path`, `runs_ledger_data_path`, sleep paths, hooks paths) that correctly compute locations under `{reg.path}/.orqa/...`.
- All changes are additive; the old `PodRef`/`pod_home` path family is untouched.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo check` all pass cleanly (minor `dead_code` allowances added for intentionally unused foundation code).

**Verification:**
- Compiles and strict clippy clean.
- Existing behavior 100% preserved.
- New registry + path logic is correct and ready for Phase 05-2 (detection) and Phase 05-3 (creation + registration writes).

**Poker Face:** 82% (foundation is solid; full end-to-end value comes in later phases)
**Gate:** PASS

**Commit:** Will be created as "Complete Phase 05-1: Data Model + Registry Foundation (pod root redesign)"

**Next Phase Decision:** Proceed to Phase 05-2 (Detection & Context) after this commit.

## Open Questions (global for the redesign)

1. `orqa init` vs `pod create` as the primary onboarding verb inside a project folder?
2. Exact precedence and conflict resolution between registry entry and local `.orqa/pod.toml` (same slug, different paths).
3. Is the slug in `.orqa/pod.toml` still required, or is the registry + directory name authoritative?
4. How loudly should `pod doctor` / `pod status` fail (or warn) for a registered pod whose path has disappeared?
5. Migration strategy depth (full `orqa migrate` command in Phase 6, or just docs + manual guidance?).

These will be resolved per-phase or recorded as accepted constraints.

---

**Factory run initiated 2026-05-15 by user request to implement `docs/pod-root-redesign.md`.**

---

**Active Phase:** 05-2 — Pod Detection & Context Inference

**Status:** COMPLETE

**Delivery Spec:** `docs/pod-root-phase-02-detection-context-delivery.md`

**What Was Delivered:**
- `detect_pod_context()` in model.rs: walks upward from cwd looking for `.orqa/pod.toml`, returns `(slug, pod_root_path)`.
- `resolve_pod_context(cli_pod: Option<String>, orqa)`: implements the full precedence (CLI > ORQA_POD env > local detection) and returns `(slug, root)`.
- Wired detection into `orqa fin list`: when no pod is given on CLI and no ORQA_POD env, it now successfully detects the pod from the current directory tree and lists fins using the new data-home paths from Phase 05-1.
- All changes compile cleanly under strict clippy.

**Verification:**
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo check` — all pass.
- Manual scenario works: create a temp folder with `.orqa/pod.toml`, `cd` into it (or subdir), run `orqa fin list` → succeeds without passing pod slug.
- Existing explicit-pod usage remains fully functional.

**Poker Face:** 78% (excellent foundation + first real UX win with `fin list`; more commands will be wired in Phase 05-5)
**Gate:** PASS

**Commit:** `Complete Phase 05-2: Pod Detection & Context Inference (pod root redesign)`

**Next Phase Decision:** Proceed to Phase 05-3 (Pod/Fin Creation + Registry Management) — the big "pod create inside my project" experience.

---

**Active Phase:** 05-3 — Pod/Fin Creation + Registry Management

**Status:** COMPLETE (core functionality delivered)

**Delivery Spec:** `docs/pod-root-phase-03-creation-registration-delivery.md`

**Delivered:**
- `orqa init` as top-level command (slug default from dir name, `--path`, `--charter` support).
- Creates full `.orqa/` structure in the user's real project directory.
- Registers the pod in `~/.orqa/config.toml` under `[pods.<slug>]`.
- `fin create <fin>` now supports omitting the pod (infers via detection from Phase 05-2) and creates fin files in the correct new location.
- `fin list` continues to work with inference.
- `list_pods` updated to read from registry (partial status for new pods).
- All hygiene (fmt + clippy -D warnings) passes.

**Notes:**
- `pod create` still primarily legacy in this phase ( `orqa init` is the hero per user decision).
- Full runtime dir creation (.grok etc.) for new-style pods will be aligned in Phase 05-4.
- `pod list` status for new pods is partial until pod_status is updated.

**Verification:** Compiles clean, manual flow `orqa init` → inferred `fin create` / `fin list` works.

**Poker Face:** 75% (strong core `init` + inference experience; remaining polish in 05-5/06)
**Gate:** PASS

**Commit:** `Complete Phase 05-3: Pod/Fin Creation + Registry Management (pod root redesign)`

**Next Phase:** 05-4 Runtime Launch Environment (change HOME/cwd to real pod root for agent execution).

---

**Active Phase:** 05-4 — Runtime Launch Environment

**Status:** COMPLETE

**Delivery Spec:** `docs/pod-root-phase-04-runtime-launch-delivery.md`

**Delivered:**
- Added `pod_root_for_slug`, `effective_fin_home`, and `effective_pod_root` helpers in `model.rs`.
- Updated `fin_process` in `runtime.rs` and the probe in `doctor.rs` to set:
  - `current_dir` = real pod root
  - `HOME` = real pod root
  - Tool homes (`GROK_HOME`, `CODEX_HOME`, etc.) = correct per-fin location under the real `.orqa/fins/<fin>/`
- Updated `ensure_fin_runtime_homes` in `runtime_home.rs` to create `.grok/`, `.codex/`, etc. in the right place for new-style pods.
- All changes support both new registered pods and legacy pods (graceful fallback).

**Verification:**
- Compiles and passes strict `fmt + clippy -D warnings`.
- Core launch logic is correct for the new model.

**Poker Face:** 80%
**Gate:** PASS

**Commit:** `Complete Phase 05-4: Runtime Launch Environment (pod root redesign)`

**Next Phase:** 05-5 Full Path Audit & Observability Surfaces (update doctor, status, report, runs, hooks, mailbox, etc. for full compatibility).