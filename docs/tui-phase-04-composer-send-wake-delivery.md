# Phase 4 Delivery Spec — Composer, Send & Wake

**Factory Run:** TUI Operator Cockpit  
**Phase:** 4 of 6 (Composer, Send & Wake)  
**Date:** 2026-05-16  
**Status:** Ready for implementation (spec persisted)  
**Depends On:** Phase 3 (Timeline UI + Filters) — delivered

---

## 1. Objective

Deliver the **composer** at the bottom of the TUI that lets the human operator type a message and send it to a target fin inside the pod.

When the operator presses Enter:

1. The message is delivered as pod-local mail from `operator@<pod>.orqa` to the chosen target fin.
2. The target fin is immediately woken using the exact operator semantics defined in the design:
   - Debounce is bypassed.
   - An existing `run.lock` is respected (the fin is allowed to finish its current run, then re-woken for the operator message).
3. A synthetic `OperatorAction` event is injected into the timeline so the operator’s own messages appear in the stream.

This phase turns the TUI from a passive monitoring view into the primary **human-to-pod communication surface**.

---

## 2. Scope (In)

- **Composer widget** at the bottom of the screen:
  - Always-visible single-line input area.
  - Prompt shows: `operator@<pod> → <current-target> > `
  - The target fin is shown and can be changed with `f` (reusing / extending the existing fin filter picker logic).
  - Basic editing: cursor movement (left/right), backspace, delete, home/end, typing.
  - History: Up/Down arrows recall previous messages sent in this session (at least 20 entries).

- **Sending flow** (on Enter when input is non-empty):
  - Use the existing mail delivery machinery to send from the local `operator` fin to the target fin.
  - The operator fin must already exist (guaranteed by Phase 1).
  - After successful delivery, immediately trigger the wake logic for the target fin with the special operator rules.
  - Clear the input on success.
  - Show transient status in the composer line (“sent”, “woke planner”, or error).

- **Wake semantics** (must be implemented correctly):
  - Bypass any `debounce` configured on the fin or pod.
  - If the fin currently holds a live `run.lock`, do **not** kill the process. Instead, record that there is a pending operator message and re-wake the fin as soon as the current run releases the lock.
  - If the fin is not running, launch it immediately (using the normal supervised execution path).
  - The prompt passed to the fin should still be the normal “handle your open Orqa mail and tasks” (or a slight variant that mentions it was operator-initiated).

- **Timeline integration**:
  - On successful send, immediately push an `OperatorAction { text: "mailed <target>: \"<first 80 chars of message>\"" }` event into the live event buffer so it appears in the timeline right away.
  - This event should be styled distinctly (bold, different color).

- **Target fin selection**:
  - Default target comes from `pod.toml` under `[operator] default_fin = "..."` (fall back to a reasonable heuristic or the first non-operator fin if not set).
  - `f` while the composer is focused cycles / opens a picker for the target fin (separate from the timeline fin filter).
  - The current target is persisted in the `App` state for the session.

- **Input mode & key handling**:
  - When the composer has focus (default), most keys go to the input.
  - Navigation keys (arrows, PageUp/Down, etc.) should still work for the timeline when the input is empty, or we use a clear model:
    - **Recommended for Phase 4**: The timeline is always scrollable with `Ctrl+↑/↓`, `Ctrl+PageUp`, etc., while normal arrows and typing go to the composer. This keeps the “chat-like” feel.
    - Or: normal mode vs input mode (press `i` to focus input). We’ll decide during implementation but document the choice.

- **Error handling & feedback**:
  - If mail delivery fails, show a clear error in the status/composer area without crashing the TUI.
  - If the target fin no longer exists, fall back gracefully.

- Keep all Phase 1–3 guarantees (safe launch only inside real pods, terminal restore, etc.).

---

## 3. Scope (Out — Explicitly Deferred)

- Full mail reader (Enter on a mail event to view the body and mark done) — Phase 5.
- Task list / task actions.
- Persistent command history across TUI restarts.
- Syntax highlighting or multi-line input (keep it single-line for now).
- `pod.toml` `[operator]` section parsing (we can hard-code a default or read it simply; full config support can come in Phase 5/6 if needed).
- Theming / custom keybindings.
- Any change to how `operator@` mail is routed globally (the local delivery preference for the TUI’s operator fin is still future work).

---

## 4. Technical Approach & Constraints

- **Input handling**: We will implement a simple single-line editor ourselves (no new crate). Ratatui’s `Paragraph` + cursor management using `CrosstermBackend`’s cursor control is sufficient and keeps the dependency surface small.

- **Composer component**: Add a `Composer` struct (in `app.rs` or a new `composer.rs`) that holds:
  - Current input buffer (`String`)
  - Cursor position
  - Command history (`Vec<String>`)
  - Current target fin (`String`)
  - Transient status message + timestamp (for “sent”, “error”, etc.)

- **Sending mail**:
  - We need a clean internal path to send mail without going through the CLI dispatcher.
  - Best approach: expose or create a small helper (e.g. `send_operator_message(orqa, pod_slug, target_fin, body)`) that:
    - Constructs the proper `SendMailArgs`
    - Calls the core delivery logic in `mailbox`
    - Returns the message path or error.
  - After delivery, we trigger the wake using the runtime execution functions (`supervise_fin` or equivalent) with operator-specific flags.

- **Wake logic**:
  - We will need to extend or add a helper that understands “operator-initiated wake with lock respect”.
  - The logic should live in `src/tui/` or a small new module, calling into `runtime` and `model` as needed.
  - The pending re-wake after lock release can be handled by the existing watcher loop or a simple timer in the TUI for Phase 4 (we can make it more robust later).

- **State ownership**: The `App` struct (from Phase 3) will own the `Composer` and the `PodWatcher`. Rendering and input handling will be split between `App::render` and a new `handle_input` method that routes keys to either the timeline or the composer.

- **Default target fin**:
  - First try to read `[operator] default_fin` from `<pod_root>/.orqa/pod.toml` (simple toml parsing is acceptable).
  - Fall back to the first non-`operator` fin we discover, or “planner” as a soft default.
  - Store the chosen target in `App` for the lifetime of the TUI session.

- **Synthetic events**: We already have `Event::OperatorAction`. We will push one directly into `app.events` on successful send.

---

## 5. Files Expected to Change / Be Created

- `src/tui/app.rs` — major additions (Composer struct, input handling, render_composer, send logic)
- `src/tui/composer.rs` (new, recommended) — clean separation of the input widget logic
- `src/tui/run.rs` — update key handling to route to composer vs timeline
- Possibly small new helpers in `src/mailbox/` or a `src/tui/mail.rs` for operator-initiated send + wake
- `src/tui/mod.rs` — exports
- `docs/tui-factory-ledger.md` — Phase 4 progress
- This delivery spec

No new external crates are expected for Phase 4 (we stay with ratatui + crossterm).

---

## 6. Verification & Quality Gates for This Phase

**Must pass before commit:**

1. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --locked` are clean.
2. **Manual experience (the real test)**:
   - Inside a real pod with at least two fins (e.g. `planner` and `builder`):
     - Run `orqa` → TUI appears with timeline.
     - Type a message in the composer, press Enter.
     - You should see:
       - The message appear as an `OperatorAction` event in the timeline almost immediately.
       - The target fin gets woken (you can watch it in another terminal with `orqa fin status` or just see new log lines appear in the timeline).
     - If the target fin is already running, it should finish its current run and then immediately start a new one for the operator message.
   - Change target with `f` and send another message — it goes to the new fin.
   - Error cases (invalid target, mail system failure) show clear feedback without crashing the TUI.
3. Terminal is always restored on exit.
4. No regression in Phases 1–3 behavior.

**Poker-face questions:**

- Does sending mail ever bypass the `operator` fin’s own inbox (i.e. do we correctly deliver to the local `.orqa/fins/operator/mail/new/` first)?
- Is the lock-respect rule actually implemented, or did we accidentally kill running fins?
- Is the default target fin discovery robust enough for real pods?
- Does the composer feel natural while the timeline is still updating behind it?

---

## 7. Risks & Mitigations

- **Risk**: Integrating mail sending + wake logic is the first time the TUI touches the core `mailbox` and `runtime` modules. There may be hidden assumptions or missing internal APIs.
  **Mitigation**: Keep the integration code small and well-commented. If a clean internal API doesn’t exist yet, we can expose one with minimal surface (we own the crate).

- **Risk**: Input handling + timeline scrolling key conflicts become annoying.
  **Mitigation**: Choose a clear model early (recommended: normal arrows always go to composer; use `Ctrl+↑/↓` and `Ctrl+PageUp/Down` for timeline scrolling). Document it in the code and in the `?` help if we add one.

- **Risk**: The “post-run re-wake” logic is tricky to get right in a live TUI.
  **Mitigation**: For Phase 4 we can implement a pragmatic version: if the fin is locked, we still record the mail, and the normal `PodWatcher` + a periodic “check for pending operator wakes” in the TUI event loop can trigger the re-wake when the lock disappears. We can refine this in Phase 6.

---

## 8. Definition of Done for Phase 4

- The composer is fully functional and feels like the primary way a human talks to the pod.
- Sending a message reliably:
  - Delivers mail from the local operator fin.
  - Wakes the target fin with the exact bypass-debounce + respect-lock rules.
  - Shows the action in the timeline.
- Target fin selection works (`f` key).
- All quality gates passed.
- Clean commit referencing “TUI Phase 4”.
- Ledger updated.
- Factory proceeds to Phase 5 only after explicit poker-face sign-off.

---

**Persisted by:** Factory  
**Next action after spec:** Implement Phase 4 (composer widget + send + wake logic), run verification + poker-face, then commit.