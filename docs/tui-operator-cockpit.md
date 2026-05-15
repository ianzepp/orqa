# TUI Operator Cockpit Design

**Status:** Design Draft  
**Phase:** Post Phase 05 (Pod Root + Global Registry)  
**Date:** 2026-05-15  
**Related:** [pod-root-redesign.md](./pod-root-redesign.md), `ops` / `operator@` mail model, bare `orqa` dashboard

---

## 1. Vision

In the post-redesign world, a pod is no longer a synthetic thing living under `~/.orqa/pods/`. A pod *is* a registration over a real user project directory. The coordination data lives in `<project-root>/.orqa/`.

In this world, running `orqa` with no arguments while inside (or below) a pod root should no longer print a static text overview. It should open a **live Ratatui operator cockpit** for that specific pod.

The TUI is the human operator's dedicated, always-available surface for one pod:

- You `cd` into your project.
- You run `orqa`.
- You are dropped into a monitoring + action surface that shows:
  - The **operator inbox** — mail items addressed to you (`operator@<pod>.orqa`) that require human attention.
  - A **live activity stream** of what all the fins in the pod are currently doing (run output, mail they send/receive, wake decisions, task state).
  - A **composer** at the bottom that lets you type messages or directives.

When you type a message and send it, the TUI:
1. Delivers it as mail from the local `operator` fin to the target fin.
2. Immediately wakes the target fin so it processes the message promptly.

Fins respond by mailing back to `operator@<pod>.orqa` (the local inbox). Those replies appear in both the operator inbox pane and the activity stream.

The TUI is **not** primarily a chat-with-one-agent interface. It is a **pod activity monitor + human injection point**.

---

## 2. Core Concept: The Local `operator` Fin

Each pod gets a special, auto-provisioned fin named `operator` under `.orqa/fins/operator/`.

**Characteristics:**

- It is created automatically the first time the TUI is invoked inside a pod (or during `orqa pod create` / `orqa init` for convenience).
- It exists **only for the human operator's TUI**. The normal wake loop should treat it specially (it is not a background autonomous fin).
- Its `mail/new/` directory is the authoritative "things the human needs to look at for this pod".
- Fins in the pod are instructed (via the pod-level `AGENTS.md`) to mail `operator@$ORQA_POD.orqa` when they need human input, when they have results for the operator, or when they want to escalate.
- Because of the Phase 05 layout, this mail lands in the pod-local `<project>/.orqa/fins/operator/mail/new/`, which the TUI watches directly. No mandatory hop through the global `ops` pod is required for pod-local operator dialogue (though the cross-pod escalation bridge to `operator@ops.orqa` can still exist for true multi-pod / global operator needs).

The TUI **is** the runtime for the `operator` fin from the human's perspective. The fin itself may have a minimal `ROLE.md` and `AGENTS.md` that simply say "this identity is used by the human via the TUI cockpit."

This gives a clean, symmetric address for everything:
- `planner@my-pod.orqa` (autonomous coding fin)
- `builder@my-pod.orqa`
- `operator@my-pod.orqa` (the human sitting in the TUI)

---

## 3. User Experience (Post-Redesign Baseline)

### Invocation

```sh
cd ~/work/minted-geek-swarm/swarm-api
orqa
```

- The TUI performs pod auto-detection (upward directory walk for `.orqa/pod.toml` + lookup in the global registry at `~/.orqa/config.toml`).
- If a pod root is found, the TUI launches in **cockpit mode** for that pod.
- If no pod context is detectable, fall back to an enhanced global overview / registry browser (the current `overview()` behavior, possibly also in TUI form later).

The design prioritizes the "I'm inside my project → I get the cockpit for its pod" path.

### First Launch in a Pod

On first use in a new pod, the TUI (or a supporting `orqa` command it calls) ensures the `operator` fin exists:

- Creates `.orqa/fins/operator/`
- Writes a minimal `fin.toml`, `ROLE.md`, `AGENTS.md`, `fin.txt`, and the standard mail/tasks/run directories.
- The operator fin is registered in the pod but marked in a way that the normal `loop` daemon largely ignores it (or only wakes it on explicit human mail).

### The Main Cockpit View

A sensible initial layout (Ratatui):

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ orqa • swarm-api (~/work/minted-geek-swarm/swarm-api)   loop: running  3 wakeable │
│ operator inbox: 2 unread   |   target: planner   [f] change target            │
├────────────────────────────┬────────────────────────────────────────────────┤
│ OPERATOR INBOX (2)         │ ACTIVITY STREAM — follow mode                  │
│                            │                                                │
│ ⬤ planner → operator       │ 14:32:11 [planner stdout] Starting task...     │
│   "Cloudflare token exp…"  │ 14:32:15 [planner event]  mail sent to builder │
│   2m ago                   │ 14:33:02 [builder stderr] npm ERR! ...         │
│                            │ 14:33:40 [operator] mailed planner: "why did…  │
│ ⬤ builder → operator       │ 14:34:10 [planner stdout] I checked the env…   │
│   "Auth fixed, ready for… │ 14:34:22 [planner] mailed operator: "The root  │
│   47s ago                  │                 cause was the wrong KV…"       │
│                            │                                                │
│ [Enter] read  [d] done     │                                                │
├────────────────────────────┴────────────────────────────────────────────────┤
│ operator@swarm-api → planner   > why did the last deploy use the old token?  │
│ [Send: ↵]  [Tab: cycle target]  [Ctrl-C: clear]  [? help] [q quit]            │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key elements:**

- **Header**: Pod name + real root path, loop daemon status, aggregate wakeable count, operator inbox unread count, current target fin.
- **Left pane (or top section)**: Operator Inbox — focused list of unread mail addressed to the local `operator` fin. Shows from, subject preview, age. Supports read / mark done / delete.
- **Main pane**: Unified activity stream. Reverse-chronological (newest at bottom in follow mode), color-coded by source:
  - Fin run output (stdout green, stderr red, events blue)
  - Mail events (especially anything involving `operator@`)
  - Task state changes
  - Wake / sleep / run start events
  - Operator actions (messages you sent)
- **Bottom composer**: Always visible. Shows current "from → to" (you are always `operator@<pod>`). Typing focuses here. Enter sends as mail + triggers immediate wake of the target fin.

### Sending a Message ("Chatting with the Fins")

1. The composer has a current target fin (default configured in `pod.toml` under `[operator] default_fin = "planner"`, or last-used, or a sensible heuristic).
2. You can change target with `f` (pops a fin picker) or by typing an address prefix (e.g. `builder: please update the deploy script`).
3. On Enter:
   - The message is delivered via the existing `mail send` machinery (`operator@<pod>.orqa` → `target@<pod>.orqa`).
   - The TUI immediately invokes the execution logic for that fin (equivalent to a forced wake for this mail item).
   - A synthetic event appears in the stream: `[operator] mailed planner: "..." (woke pid 47291)`.
4. The fin wakes, sees the mail in its inbox, processes it according to its `AGENTS.md` + the message content, and can reply by mailing `operator@<pod>.orqa`.

This gives the "ask a question and immediately see the fin start working on it" feel the user wants, while keeping all coordination as durable pod-local mail.

### Receiving Replies and Escalations

- Any mail delivered to the local `operator` fin appears in the Inbox pane and is also injected into the activity stream (with a distinct style, e.g. yellow or bold).
- The operator can navigate the inbox, read full messages, mark them done (moves from `new/` to `cur/`), or delete.
- Marking done in the TUI is the human equivalent of a fin calling `orqa mail done`.

---

## 4. Activity Stream Details

The stream is the heart of the "monitoring view of the pod itself."

It should surface:

- Appends to any fin's latest `stdout.log`, `stderr.log`, `events.jsonl` (with fin label and stream type).
- New mail arrivals to any fin (at minimum: anything to/from `operator@`, or all mail with a compact `from → to: subject` line).
- Task creations, state changes, completions.
- Fin run lifecycle (start, exit code, duration).
- Explicit sleep/wake actions.
- Operator-sent messages (for self-awareness).

**Implementation notes (for later):**
- The TUI maintains a ring buffer of recent events.
- Background threads or a `notify`-based watcher (or efficient polling) watch:
  - All `<fin>/runs/<latest>/` log files
  - All `<fin>/mail/new/` directories
  - All `<fin>/tasks/new/`
  - Run lock files and latest-run pointers
- Events are normalized and pushed into the UI thread for rendering.

Follow mode (default) keeps the view pinned to the bottom. The user can scroll up for history.

---

## 5. Configuration Surface

Additions to `pod.toml` (inside the project's `.orqa/pod.toml`):

```toml
[operator]
# The fin that operator messages are sent to by default when the TUI composer
# does not specify a recipient.
default_fin = "planner"

# Optional: name of the local operator fin (defaults to "operator")
# fin = "operator"

# Optional: whether the normal loop daemon should ever wake the operator fin
# (usually false; the human drives it via TUI)
wake_via_loop = false
```

The TUI can also expose runtime controls (pause follow, filter by fin, force full pod wake, etc.).

---

## 6. Keyboard & Interaction Model (Initial Proposal)

- `q`, `Esc`, `Ctrl-D`: Quit the TUI
- `f`: Open fin picker / change target fin
- `i` or `Tab`: Toggle focus between Inbox pane and Activity Stream
- `r`: Force-refresh / re-scan all watched paths
- `w`: Wake all wakeable fins (or the current target)
- `?`: Show help overlay
- In composer:
  - `Enter`: Send current message to target fin + wake it
  - `Ctrl-C` or `Esc`: Clear composer
  - `Up` / `Down` (when not in input mode): scroll the activity stream
  - `PgUp` / `PgDn`, `Home` / `End`: stream navigation
- In Inbox:
  - `Enter`: Read selected message
  - `d`: Mark done
  - `x` or `Del`: Delete

The TUI should feel responsive and "always on" — you can leave it running in a tmux pane while you edit code, and glance at it when you want to steer the pod.

---

## 7. Integration with Phase 05 Redesign

- **Pod detection**: The TUI relies entirely on the upward-walk + registry mechanism described in the redesign. This is the primary reason the redesign makes the TUI "easier" and more natural.
- **Path resolution**: All TUI code uses the new `PodRegistration` + `Orqa::pod_data_home(reg)`, `fin_data_home`, `mail_data_home`, etc. There is no legacy `~/.orqa/pods/` path in the TUI implementation.
- **Fin execution environment**: When the TUI wakes a fin in response to an operator message, it uses the post-redesign launch path (`cwd` = real pod root, `HOME` = real pod root, per-fin `*_HOME` still under `.orqa/fins/<fin>/`).
- **operator@ addressing**: In the local pod context, `operator@<pod>.orqa` resolves to the local `.orqa/fins/operator/` fin. The global bridge to `ops` remains available for true cross-pod escalations.

---

## 8. Open Questions & Trade-offs

1. **Auto-creation of the `operator` fin**
   - Should `orqa pod create` / `orqa init` automatically create the `operator` fin, or should it be lazily created on first TUI launch?
   - Should the operator fin have a visible `ROLE.md` that humans can edit, or should it be intentionally minimal / hidden?

2. **Inbox vs Stream emphasis**
   - Is the operator inbox a first-class left pane (as sketched), or is it primarily surfaced *through* the unified activity stream (with a badge for unread count)?
   - Many operators may prefer "everything in one flowing timeline" + the ability to filter for `to: operator`.

3. **Immediate wake policy**
   - When the operator sends a message, should we always force-wake the target fin (bypassing `debounce`), or respect the fin's normal policy?
   - Proposal: operator-initiated mail is always treated as high priority and wakes the fin immediately (with a note in the run record that it was operator-driven).

4. **Multi-pod / global operator view**
   - The TUI as described is per-pod. Should there also be a global `orqa` (when run outside any pod) that gives a registry overview + ability to jump into a specific pod's cockpit? Or is that a separate `orqa ops` TUI later?

5. **Persistence of TUI state**
   - Should the TUI remember (per pod) the last target fin, scroll position, active filters, window layout? Probably yes, in a small `<pod>/.orqa/tui-state.toml` or similar.

6. **Fin instructions for the operator surface**
   - The pod-level `AGENTS.md` (and fin templates) should be updated to document the new expectation: "Mail `operator@$ORQA_POD.orqa` when you need the human. The human primarily interacts with you via the `orqa` TUI cockpit inside the project."

---

## 9. Success Criteria

- A developer can `cd` into a project with a Phase 05 pod, run `orqa`, and immediately see a live, useful view of the agents working on their code without any extra flags or environment variables.
- The operator has a single, obvious place (`operator@<pod>.orqa` inbox) where all human-escalation and human-directed replies land.
- Sending a message through the composer feels as lightweight as typing a Slack message, but produces durable, auditable mail + an immediate wake + full run logs.
- The `operator` fin is a clean, first-class concept that does not pollute the autonomous fin list.
- The design works entirely within the new pod-root + registry model and does not require the old central `~/.orqa/pods/` layout.

---

## 10. Next Steps (After Design Approval)

1. Finalize this spec with feedback.
2. Implement pod auto-detection + `current_pod_context()` helper (shared with the rest of Phase 05).
3. Add support for the local `operator` fin creation and the `[operator]` section in `pod.toml`.
4. Update `pod create` / `init` and the `AGENTS.md` templates to mention the operator surface.
5. Prototype the Ratatui app (start with a single unified stream + composer, add the inbox pane in a second iteration).
6. Wire the composer to `send_mail` + the immediate supervised execution path.
7. Add file watching / event normalization for the activity stream.
8. Iterate on layout and keybindings based on real use.

---

This design treats the post-redesign world as the foundation and gives the human operator a natural, project-local "I am here with my agents" experience. The auto-generated `operator` fin + TUI combination turns the abstract `operator@` address into a concrete, always-available cockpit.