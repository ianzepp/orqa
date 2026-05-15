# Phase 05-6 Delivery Spec — Documentation, Help, Migration & Test Hardening

**Factory Run:** Pod Root + Global Registry Redesign  
**Phase:** 05-6 of 6 (Final)  
**Source:** `docs/pod-root-redesign.md` (Section 11 Step 6 + Section 9)  
**Date Prepared:** 2026-05-15

---

## 1. Interpreted Problem

The technical implementation across Phases 05-1 to 05-5 is largely complete. However, the user-facing surface is still heavily documented around the old model:

- README, `help.md`, and templates still describe `~/.orqa/pods/<slug>/fins/<fin>/...` as the primary layout.
- There is no migration guidance for existing users.
- Integration tests (`pod_flow.rs`) were written against the old layout and some are already fragile.
- New users have no clear "getting started with the new model" story.

This final phase makes the redesign **documented, discoverable, and safe to adopt**.

---

## 2. Normalized Spec

### Functional Requirements

1. **Documentation Updates**
   - Major updates to `README.md` explaining the new pod = real directory model.
   - Update `help.md` (the embedded operational guide) to reflect new paths and `orqa init` flow.
   - Update `templates/pod-agents.md` and `templates/fin-agents.md` if they contain hardcoded path assumptions.

2. **Migration Guidance**
   - Add a clear "Migration from previous versions" section (or separate `MIGRATION.md`).
   - Recommend `orqa init` in an existing project folder + manual move of important state if desired.
   - Document that old `~/.orqa/pods/` pods continue to work during transition.

3. **Test Hardening**
   - Improve / stabilize integration tests in `tests/pod_flow.rs` to work reliably with the new model (using temp directories + `--home`).
   - Add at least one solid end-to-end test for the `orqa init` + inferred `fin create` + launch flow.

4. **Help Text Polish**
   - Ensure `orqa init --help` and `orqa pod create --help` give good guidance.

### Non-Goals

- Full automated `orqa migrate` command (can be added later).
- Complete removal of legacy path support (kept for transition period).

---

## 3. Repo-Aware Baseline

- `README.md` has a large "Filesystem Contract" section describing the old layout.
- `help.md` is the single source of truth embedded in the binary for agents.
- `pod_flow.rs` has many tests that create pods under a temp home using the old `pod create` path.

---

## 4. Stage Graph & Work Breakdown

### Epic 1: Documentation Refresh

1.1 Major rewrite of the "Concepts" + "Filesystem" sections in `README.md`.
1.2 Update `help.md` operational guide (especially mail/task paths, fin home, `orqa init`).
1.3 Minor updates to agent templates if needed.

### Epic 2: Migration Notes

2.1 Add migration section to README (or new file).
2.2 Document recommended path: `orqa init` inside existing project + (optional) copy of important state.

### Epic 3: Test Stabilization + New Coverage

3.1 Make `pod_flow.rs` tests more robust (better temp home isolation, focus on new flow).
3.2 Add at least one test that exercises `orqa init` + detection + launch.

---

## 5. Success Criteria

- A new user reading the README or running `orqa help` understands that pods now live in real directories.
- Existing users have clear guidance on how to adopt the new model without losing data.
- The test suite has at least one reliable happy-path test for the new `orqa init` flow.
- All documentation is consistent with the behavior delivered in Phases 05-1 to 05-5.

---

**This is the final phase.** After it is complete and committed, the pod-root redesign scope of work is considered delivered.