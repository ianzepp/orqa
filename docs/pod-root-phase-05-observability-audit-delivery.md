# Phase 05-5 Delivery Spec — Full Path Audit & Observability Surfaces

**Factory Run:** Pod Root + Global Registry Redesign  
**Phase:** 05-5 of 6  
**Source:** `docs/pod-root-redesign.md` (Section 11 Step 5)  
**Date Prepared:** 2026-05-15  
**Depends On:** Phases 05-1 through 05-4 (registry, detection, creation, runtime launch)

---

## 1. Interpreted Problem

Phases 05-1 to 05-4 established the new data model, detection, creation flow (`orqa init`), and runtime launch behavior (real pod root as `cwd` + `HOME`).

However, many observability and management surfaces still assume the old `~/.orqa/pods/<slug>` layout:

- `pod doctor`
- `pod status` / `pod list`
- `pod tail`
- `fin status`, `fin runs`, `fin run-log`, `fin tail`
- `report.rs` (ops overview)
- `runs.rs` (run records, latest-run)
- `hooks.rs`
- `mailbox/` (mail + task storage)
- `service.rs` (legacy daemon paths)
- `config.rs` (some internal reads)

If these paths are not updated, users will see broken status, missing runs, failed doctor checks, and incorrect mail/task behavior on new-style pods.

This phase performs a systematic audit and update of **all remaining call sites** that construct or traverse pod/fin paths.

---

## 2. Normalized Spec

### Functional Requirements

1. **Path Resolution Audit**
   - Every place that calls `orqa.pod_home()`, `orqa.fin_home()`, `orqa.mail_home()`, `orqa.task_home()`, `orqa.runs_home()`, `orqa.lock_path()`, etc. must either:
     - Continue to work for legacy pods, **or**
     - Use `orqa.effective_*` helpers (or equivalent) for new-style registered pods.

2. **Key Surfaces to Harden**
   - `doctor.rs` — `pod_doctor`, `doctor_fins`, readiness checks for `.orqa/` structure
   - `status.rs` — `pod_status`, `fin_status`, `print_*_status`
   - `report.rs` — `ops_report`, overview dashboard
   - `runs.rs` — `list_runs`, `read_run_record_for`, `tail_*`, `latest_run` pointer handling
   - `hooks.rs` — hook discovery and execution under `hooks/`
   - `mailbox/` — `ensure_maildir`, list/read/done/delete for mail and tasks
   - `service.rs` — any remaining pod scanning logic (mostly legacy now)
   - `config.rs` — any direct `pod_home` / `fin_home` usage for reading `pod.toml` / `fin.toml`

3. **Existence & Error Quality**
   - All new-style pods must produce friendly errors via `ensure_pod_exists` / `ensure_fin_exists` when the `.orqa/pod.toml` or `.orqa/fins/<fin>/fin.toml` is missing.
   - `pod doctor` must correctly validate the new `.orqa/` layout.

4. **Backward Compatibility**
   - Every legacy pod under `~/.orqa/pods/<slug>` must continue to work exactly as before.

### Non-Goals (out of scope for this phase)

- Full migration tooling (`orqa migrate`)
- Documentation updates (Phase 05-6)
- Deep test coverage expansion (Phase 05-6)

---

## 3. Repo-Aware Baseline

From previous phases we already have:
- `effective_pod_root(slug)` and `effective_fin_home(fin)` in `model.rs`
- `resolve_pod_context()` for inference
- `load_registry()`

Most remaining work is mechanical replacement of direct `pod_home` / `fin_home` calls with the effective versions, or adding registry-aware logic in listing/reporting functions.

High-risk areas:
- `runs.rs` (many direct filesystem operations on `runs/`, `latest-run`, `runs.jsonl`)
- `mailbox/storage.rs` (Maildir operations)
- `doctor.rs` (multiple hardcoded checks for `.codex`, `.grok`, etc.)

---

## 4. Stage Graph & Work Breakdown

### Epic 1: Core Path Helpers Usage Sweep

1.1 Audit + update `doctor.rs` (pod/ fin readiness, backend probe paths)
1.2 Audit + update `status.rs`
1.3 Audit + update `report.rs` (ops report)
1.4 Audit + update `runs.rs` (run discovery, logging, tailing)

### Epic 2: Mailbox & Task Surfaces

2.1 Update `mailbox/mod.rs` and `mailbox/storage.rs` to resolve correct mail/task homes for new-style pods
2.2 Ensure `ensure_maildir`, read/write/done/delete all work under `.orqa/fins/<fin>/mail` and `/tasks`

### Epic 3: Hooks & Service

3.1 Update `hooks.rs` to use `pod_hooks_data_home` equivalent when pod is new-style
3.2 Review `service.rs` for any remaining old-path assumptions (mostly cleanup)

### Epic 4: Verification

4.1 After changes, both legacy and new pods must pass:
   - `orqa pod doctor`
   - `orqa pod status`
   - `orqa fin status`
   - `orqa fin runs` / `fin tail`
   - `orqa ops`
   - Mail and task send/list/read/done

---

## 5. Checkpoints & Verification

**Must pass before commit:**

- `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --locked`
- Manual verification matrix:
  - New pod (via `orqa init`): `doctor`, `status`, `tail`, `runs`, `mail`, `task`, `ops report` all functional
  - Legacy pod: no regression
- Poker-face ≥ 80%

---

## 6. Success Criteria

- Every observability and management command works correctly on both new-style registered pods and legacy pods.
- No raw filesystem errors leak to users on new pods.
- The system is observably complete for the new architecture.

---

**This spec must be persisted before major edits for Phase 05-5 begin.**