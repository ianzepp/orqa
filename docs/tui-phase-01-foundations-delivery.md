# Phase 1 Delivery Spec — TUI Operator Cockpit Foundations

**Factory Run:** TUI Operator Cockpit  
**Phase:** 1 of 6 (Foundations & Safe Entry Point)  
**Date:** 2026-05-16  
**Status:** Ready for implementation (spec persisted)

---

## 1. Objective

Make the experience of running bare `orqa` (no arguments) inside a Phase 05 pod root launch a minimal but correct Ratatui TUI application, while preserving 100% of the existing behavior outside pod roots. Lay the structural and safety foundations so that later phases can safely add the timeline, composer, wake logic, and operator fin interactions.

This phase deliberately does **not** deliver the full monitoring or chat experience — only the launch, detection integration, TUI skeleton, and the critical safety property around the `operator` fin.

---

## 2. Scope (In)

- Add dependencies: `ratatui` (latest stable, ~0.29), `crossterm` (required backend).
- Modify the no-subcommand path in `src/main.rs`:
  - Attempt `resolve_pod_context(None, &orqa)` (or equivalent that triggers detection).
  - On success (pod detected + valid root) → enter TUI mode.
  - On failure → fall through to existing `overview(&orqa)`.
- New module structure: `src/tui/mod.rs` (and supporting files as needed for Phase 1). Keep it behind a conditional or just always compiled (small cost).
- Minimal runnable Ratatui app:
  - Full-screen terminal UI using crossterm backend.
  - Header showing `orqa • <pod-slug> (<real root>)`.
  - Status line: "Press q to quit (Phase 1 skeleton)".
  - Clean exit on `q`, `Esc`, or Ctrl-C / Ctrl-D. Restores terminal properly.
  - No crash on resize or other common events.
- Safe `operator` fin provisioning logic:
  - New function (e.g. `ensure_operator_fin(orqa: &Orqa, pod_slug: &str, pod_root: &Path) -> Result<(), String>`).
  - Only called when we have a confirmed detected pod root (from `resolve_pod_context`).
  - Uses `PodRegistration { slug, path: pod_root, enabled: true }` + `orqa.fin_data_home(&reg, "operator")`.
  - Creates the minimal files only if they don't already exist (idempotent).
  - Writes a clear `ROLE.md` explaining its TUI-only purpose.
  - Does **not** register the fin in any special way yet (normal fin creation is fine).
  - Never touches pod creation paths.
- Graceful handling when the detected pod root does not yet have any fins (or only has the operator fin).
- All changes must keep the existing test suite green (`cargo test --locked`).
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings` clean on the whole crate.
- Update `Cargo.toml` with the new deps (no feature flags needed initially).
- Basic manual verification path: create a temp pod with `orqa init`, run `orqa` inside it → TUI appears; run `orqa` outside → old overview.

---

## 3. Scope (Out — Explicitly Deferred)

- Full timeline rendering, event model, file watching.
- Composer / input box / sending mail.
- Any wake logic or `fin exec` invocation from the TUI.
- Filters (`f`, `o`, thread).
- Reading mail, mark done.
- `pod.toml` `[operator]` section parsing.
- Help overlay, theming, scrollback history beyond basic.
- Tests that exercise the TUI interactively (we'll use headless or integration later).
- Changes to mail address resolution for local `operator@`.
- Any update to `AGENTS.md` templates or user docs (Phase 6).
- Global / multi-pod TUI view.

---

## 4. Technical Approach & Constraints

- **Detection integration**: Call the existing `model::resolve_pod_context` (it already handles the three precedence levels and returns `(slug, pod_root)`). We do **not** need to re-implement detection.
- **Path usage**: When we decide to enter TUI mode, construct a `PodRegistration` immediately and use only the `*_data_home` family of methods. Never hard-code `.orqa` paths.
- **Terminal handling**: Use `ratatui::Terminal::with_options` + crossterm `enable_raw_mode`, `EnterAlternateScreen`, and the standard `restore` pattern on exit (even on panic via `std::panic::catch_unwind` or `ctrlc` handler if needed for Phase 1).
- **Operator fin creation**:
  - The function should be in a new `src/tui/operator.rs` or inside the TUI module.
  - It should be safe to call multiple times.
  - Minimal files to write on creation (mirror what `fin create` does but for the special "operator" role):
    - `fin.toml` (with `[fin] slug = "operator"`)
    - `ROLE.md` (short explanation)
    - `AGENTS.md` (minimal, or reuse a template)
    - `fin.txt`
    - `mail/{new,cur,tmp}`, `tasks/{new,cur,tmp}`, `runs/`
  - Do **not** create a full `fin create` command surface for it yet.
- **Module organization**: `src/tui/` should be the home for all future TUI code. Phase 1 can have a very small `app.rs` or `run.rs` that contains the minimal event loop.
- **Error handling**: If the TUI fails to initialize (e.g. not a tty), fall back to a clear text message + the old overview behavior if possible, or at minimum a good error.
- **Dependencies**: Keep the added crate set minimal. Prefer `ratatui` with the `crossterm` feature if it exposes one, or add both explicitly.

---

## 5. Files Expected to Change

- `Cargo.toml` — add `ratatui` and `crossterm`.
- `src/main.rs` — change the `cli.command.is_none()` branch to attempt pod context + TUI launch.
- `src/tui/mod.rs` (new)
- `src/tui/run.rs` or `src/tui/app.rs` (new) — minimal Ratatui app
- `src/tui/operator.rs` (new or combined) — `ensure_operator_fin` function
- Possibly small updates in `src/model.rs` if any helper is missing (unlikely).
- `src/commands.rs` — possibly expose or move the operator-fin creation if it makes sense to share with a future command.

---

## 6. Verification & Quality Gates for This Phase

**Must pass before commit:**

1. `cargo build --release` succeeds.
2. `cargo test --locked` — all existing tests still green (no regressions).
3. `cargo fmt --check`
4. `cargo clippy --all-targets -- -D warnings`
5. Manual smoke:
   - In a fresh temp directory: `orqa init my-test-pod` → `cd` into it → `orqa` → TUI appears showing the pod, `q` exits cleanly, terminal restored.
   - Outside any pod: `orqa` still shows the familiar text overview (loop status + pods list).
   - The `operator` fin directory is created under `.orqa/fins/operator/` with the expected files on first TUI run.
   - Running `orqa` a second time in the same pod does not recreate or overwrite existing operator fin files.
6. No terminal corruption on exit (even after resize or rapid `q`).
7. The new code is clearly commented with "Phase 1 skeleton" markers where appropriate so later phases know what to replace.

**Poker-face style questions for this phase (to be answered in the completion gate):**
- Did we accidentally make the TUI the default even when no pod is present?
- Does the operator fin creation ever run when we only have a legacy pod (no `.orqa/pod.toml`)?
- Is the terminal always restored, even on early errors?
- Is the dependency addition minimal and justified?

---

## 7. Risks & Mitigations

- **Risk**: Crossterm/raw mode leaves the terminal in a bad state on panic.  
  **Mitigation**: Use `ratatui::backend::CrosstermBackend` + the standard `let mut terminal = Terminal::new(...)?;` + `terminal.show_cursor()?` + `disable_raw_mode()` + `execute!(stdout, LeaveAlternateScreen)` in a `Drop` guard or explicit `finally` block. Wrap the TUI run in a function that guarantees restore.

- **Risk**: Detection returns a root but the `.orqa` dir is incomplete (corrupt pod).  
  **Mitigation**: After detection, do a minimal existence check for `pod.toml` before entering TUI. Fall back to overview with a warning if the pod looks broken.

- **Risk**: Adding ratatui increases compile time / binary size noticeably.  
  **Mitigation**: Acceptable for this feature. We can evaluate features later (e.g. `ratatui/crossterm`).

---

## 8. Definition of Done for Phase 1

- The delivery spec above is the source of truth.
- All "In Scope" items are implemented and verified.
- All quality gates passed.
- A clean git commit (or small coherent set) with message referencing "TUI Phase 1".
- This phase ledger entry updated.
- Factory proceeds to Phase 2 only after explicit checkpoint sign-off.

---

**Persisted by:** Factory (via Grok)  
**Next action after spec:** Implement Phase 1 code, run all verification, then poker-face review before commit.