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

Fins respond by mailing back to `operator@<pod>.orqa` (the local inbox). Those replies appear as distinct events in the single flowing timeline (and are easy to surface with the `o` filter).

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
- If a pod root is found (`.orqa/pod.toml` exists and/or the directory is registered), the TUI launches in **cockpit mode** for that pod.
- If no valid pod context is detectable, `orqa` falls back to the existing text-based overview (current `overview()` behavior). There is no global multi-pod TUI view at this time.

The design prioritizes the "I'm inside my project → I get the cockpit for its pod" path. Pod data is created explicitly with `orqa init` (or `orqa pod create --path ...`). The TUI never creates pods.

### Operator Fin Provisioning (Safety Rules)

**Pods are never created by the TUI.** Use `orqa init` (preferred) or `orqa pod create` while inside the target project directory to create the pod data (the `.orqa/` directory + registry entry).

The TUI **only** creates the special `operator` fin, and only under these strict conditions:

- A valid pod root has already been detected (`.orqa/pod.toml` exists at the project root, and the pod is registered or the directory is a recognized pod root).
- The `operator` fin does not yet exist under `.orqa/fins/operator/`.

On first TUI startup inside a pod that lacks the operator fin, it safely provisions:

- `.orqa/fins/operator/`
- Minimal `fin.toml`, `ROLE.md` ("This fin is the dedicated identity for the human operator using the TUI cockpit"), `AGENTS.md`, `fin.txt`, and the standard `mail/`, `tasks/`, `runs/` layout.

The operator fin is intentionally excluded from normal background wake-loop scheduling. It is only woken when the human uses the TUI composer to send it mail, or when other fins escalate to `operator@<pod>.orqa`.

**Accidental launch protection**: If you run `orqa` in a random directory that does not contain (or is not registered as) a pod, you will not get a TUI and no files will be created. You get the normal overview text instead. This is deliberate.

### The Main Cockpit View — One Flowing Timeline

The preferred model is **everything in one flowing timeline** (reverse-chronological, follow mode by default). There is no permanent split inbox pane.

Example layout:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ orqa • swarm-api (~/work/minted-geek-swarm/swarm-api)   loop: running  3w  2o │
│ target: planner [f]   filter: all [F]   2 operator mail [o]   [? help] [q]   │
├─────────────────────────────────────────────────────────────────────────────┤
│ ACTIVITY TIMELINE — follow (newest at bottom)                                │
│                                                                              │
│ 14:31:55 [builder] run started (latest)                                      │
│ 14:32:11 [planner stdout] Starting task "update-cloudflare-token"...         │
│ 14:32:15 [planner] mail → builder: "please handle the KV secret"             │
│ 14:33:02 [builder stderr] npm ERR! code 1                                    │
│ 14:33:40 [operator → planner] why did the last deploy use the old token?     │
│ 14:34:10 [planner stdout] I checked the env var — it was still the old one   │
│ 14:34:22 [planner] mail → operator: "Root cause: wrong KV namespace in the   │
│                           Cloudflare Pages project settings. Fixed."         │
│ 14:34:55 [planner] run finished (exit 0, 3m 44s)                             │
│ 14:35:10 [builder] run finished (exit 1) — lock released                     │
│ 14:35:12 [system] pending operator mail for builder will now wake it         │
│                                                                              │
│ (scroll with ↑↓ PgUp/PgDn; filters active in header)                         │
├─────────────────────────────────────────────────────────────────────────────┤
│ operator@swarm-api → planner   > why did the last deploy use the old token?  │
│ [Send: ↵]  [Tab / f: change target]  [F: fin filter] [O: operator mail] [q]  │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key elements:**

- **Header**: Pod + real path, loop status, wakeable count (`3w`), operator mail count (`2o`), current target fin, active filters.
- **Main timeline (everything)**: A single unified, flowing event stream containing run output, mail events (especially anything involving `operator@`), task changes, wake/lock events, and operator actions. New events append at the bottom in follow mode.
- **Filters** (hotkeys, shown in header when active):
  - `f` / `F`: Cycle or picker for "only this fin" (or "all fins").
  - `o` / `O`: Toggle "only events involving the operator fin" (mail to/from `operator@`, your sent messages, escalations).
  - Thread / subject filter (e.g. `/` or `t` opens a thread filter that groups mail conversations).
  - Additional filters: errors only, one specific run, etc.
- **Bottom composer**: Always visible. Clearly shows `operator@<pod> → <target-fin>`. Enter sends the mail and triggers the wake logic.

Operator mail and escalations are highly visible in the single timeline (distinct color or prefix such as `[operator → fin]` or `[fin → operator]`). The header badge (`2o`) gives at-a-glance count of pending human items. Filtering with `o` gives the "list of mail items I need to deal with" experience without a separate pane.

### Sending a Message ("Chatting with the Fins")

1. The composer has a current target fin (default configured in `pod.toml` under `[operator] default_fin = "planner"`, or last-used, or a sensible heuristic).
2. You can change target with `f` (pops a fin picker) or by typing an address prefix (e.g. `builder: please update the deploy script`).
3. On Enter:
   - The message is delivered via the existing `mail send` machinery (`operator@<pod>.orqa` → `target@<pod>.orqa`).
   - **Immediate wake rules** (operator-initiated mail):
     - Debounce is **bypassed** — the fin is eligible to run right now.
     - An existing `run.lock` is **respected** — if the fin is currently running (live PID in the lock), the TUI does not kill it. Instead it records that there is pending operator mail for this fin. When the current run releases the lock, the TUI (or the next loop scan) immediately re-wakes the fin to process the new operator message.
     - If the fin is not running, the TUI launches it right away (supervised exec).
   - A synthetic event appears in the stream: `[operator → planner] "why did the last deploy..." (wake requested)`.
4. The fin eventually processes the mail (either immediately or right after its current run finishes), and can reply by mailing `operator@<pod>.orqa`. The reply appears in the timeline.

This gives the "ask a question and immediately see the fin start working on it" feel the user wants, while keeping all coordination as durable pod-local mail.

### Receiving Replies and Escalations

- Any mail delivered to the local `operator` fin appears as a distinct event in the single timeline (e.g. `[planner → operator] "Auth fixed..."` or `[escalation] Cloudflare token expired`).
- These events are easy to spot and can be filtered with the `o` / `O` hotkey ("only operator mail").
- Selecting an operator-directed mail event (Enter or `r`) opens a read view with full body and the standard actions: mark done (moves mail from `new/` to `cur/`), delete, or reply (which focuses the composer pre-addressed back to the originating fin).
- Marking done in the TUI is the human equivalent of a fin calling `orqa mail done`. The timeline reflects the state change.

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
- `f`: Open fin picker / change target fin for the composer
- `F`: Open fin filter for the timeline (show only events from one fin, or all)
- `o` / `O`: Toggle "operator mail only" filter (shows only events involving `operator@<pod>`)
- `t` or `/`: Open thread / subject filter (group or isolate a mail conversation)
- `r`: Force-refresh / re-scan all watched paths
- `w`: Request wake for the current target fin (or all wakeable if none targeted)
- `?`: Show help overlay
- In the timeline:
  - `Enter` or `r` on a mail event: Open full message read view + actions (mark done, delete, reply)
  - `d` on a mail event: Mark the mail done
  - `↑` / `↓`, `PgUp` / `PgDn`, `Home` / `End`: Scroll the timeline (pauses follow mode while scrolling)
- In composer:
  - `Enter`: Send current message to target fin + trigger wake logic
  - `Ctrl-C` or `Esc`: Clear composer
  - `Tab`: Cycle target fin

The TUI should feel responsive and "always on" — you can leave it running in a tmux pane while you edit code, and glance at it when you want to steer the pod. Filters and the operator-mail count in the header give you the "list of things I need to deal with" view on demand.

---

## 7. Integration with Phase 05 Redesign

- **Pod detection**: The TUI relies entirely on the upward-walk + registry mechanism described in the redesign. This is the primary reason the redesign makes the TUI "easier" and more natural.
- **Path resolution**: All TUI code uses the new `PodRegistration` + `Orqa::pod_data_home(reg)`, `fin_data_home`, `mail_data_home`, etc. There is no legacy `~/.orqa/pods/` path in the TUI implementation.
- **Fin execution environment**: When the TUI wakes a fin in response to an operator message, it uses the post-redesign launch path (`cwd` = real pod root, `HOME` = real pod root, per-fin `*_HOME` still under `.orqa/fins/<fin>/`).
- **operator@ addressing**: In the local pod context, `operator@<pod>.orqa` resolves to the local `.orqa/fins/operator/` fin. The global bridge to `ops` remains available for true cross-pod escalations.

---

## 8. Open Questions & Trade-offs (Resolved or Remaining)

**Resolved in this design:**

- **Auto-creation of the `operator` fin**: Lazily created by the TUI on first startup inside a pod that already exists (created via `orqa init` or `pod create`). The TUI must never create a pod. The operator fin gets a minimal `ROLE.md` explaining its purpose as the human TUI identity.
- **Inbox vs Stream**: Everything lives in one flowing timeline. Operator mail and escalations are first-class events in that timeline. The `o` filter + header count (`2o`) gives the "things I need to deal with" view without a permanent separate pane.
- **Immediate wake policy**: Operator mail bypasses debounce. Existing `run.lock` is respected — the TUI queues a post-run re-wake if the fin is currently executing.
- **Global / multi-pod TUI view**: Not supported in this phase. Bare `orqa` outside a detectable pod root falls back to the existing text overview.

**Remaining:**

1. **Persistence of TUI state**
   - Should the TUI remember (per pod) the last target fin, scroll position, active filters, window layout? Probably yes, in a small `<pod>/.orqa/tui-state.toml` or similar.

2. **Fin instructions for the operator surface**
   - The pod-level `AGENTS.md` (and fin templates) should be updated to document the new expectation: "Mail `operator@$ORQA_POD.orqa` when you need the human. The human primarily interacts with you via the `orqa` TUI cockpit inside the project."

3. **Read / action UI for mail in the timeline**
   - Exact interaction when you press Enter on an operator mail event (full-screen reader? inline expansion? dedicated modal?) can be refined during prototyping.

---

## 9. Success Criteria

- A developer can `cd` into a project with a Phase 05 pod, run `orqa`, and immediately see a live, useful view of the agents working on their code without any extra flags or environment variables.
- The operator has a single, obvious place (`operator@<pod>.orqa` events in the timeline, filterable with `o`) where all human-escalation and human-directed replies land. The header shows a live count of pending operator mail.
- Sending a message through the composer feels as lightweight as typing a Slack message, but produces durable, auditable mail + an immediate wake + full run logs.
- The `operator` fin is a clean, first-class concept that does not pollute the autonomous fin list.
- The design works entirely within the new pod-root + registry model and does not require the old central `~/.orqa/pods/` layout.

---

## 10. Next Steps (After Design Approval)

1. Finalize this spec with feedback.
2. Implement pod auto-detection + `current_pod_context()` helper (shared with the rest of Phase 05).
3. Add support for the local `operator` fin creation and the `[operator]` section in `pod.toml`.
4. Update `pod create` / `init` and the `AGENTS.md` templates to mention the operator surface.
5. Prototype the Ratatui app (start with a single unified timeline + composer + filter hotkeys).
6. Wire the composer to `send_mail` + the immediate supervised execution path.
7. Add file watching / event normalization for the activity stream.
8. Iterate on layout and keybindings based on real use.

---

This design treats the post-redesign world as the foundation and gives the human operator a natural, project-local "I am here with my agents" experience. The auto-generated `operator` fin + TUI combination turns the abstract `operator@` address into a concrete, always-available cockpit.