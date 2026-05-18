# Orqa Operational Guide

Orqa is a local coordinator for groups of local agent runtimes. It gives a
goal-oriented group of agents a shared filesystem shape, names the group as a
pod, names each runtime identity as a fin, and provides local mail and task
queues that can wake fins when there is work to handle.

Orqa does not decide what the agent should think or how the agent runtime
works. It creates the homes, inboxes, task queues, config files, and runtime
environment that let an agent process understand where it is and how to talk to
the other fins in its pod.

## Core Model

A pod is a collection of fins, or agents, that should share a goal, communicate
with each other, and be woken by the same scan loop.

A fin is one agent runtime identity inside one pod. In practice, a fin can be
backed by runtimes such as Claude, Codex, OpenClaw, Hermes, Pi, an Ollama-backed
agent integration, or a custom command. Each fin has its own home directory,
runtime state directories, mail inbox, and task queue.

`ORQA_HOME` stores the registry that maps pod slugs to their roots. Each pod
lives in a project root with its own `.orqa/` data directory.

```text
my-project/
  .orqa/
    AGENTS.md
    CHARTER.md
    pod.toml
    fins/
      operator/
      planner/
        AGENTS.md
        ROLE.md
        fin.toml
        .codex/       # Codex state
        .hermes/      # Hermes state
        .pi/          # Pi config and sessions
        mail/
          cur/
          new/
          tmp/
        tasks/
          cur/
          new/
          tmp/
```

Pods and fins are referenced by slug. Slugs may contain lowercase letters,
digits, and hyphens.

## Creating Pods And Fins

The recommended way to start a pod inside a project is:

```sh
cd my-project
orqa init
```

This creates a `.orqa/` directory in the current folder and registers the pod.

For explicit control (or scripting), use:

```sh
orqa pod create my-pod --path /path/to/project
orqa pod create my-pod --path . --charter "Build a focused launch plan."
```

For commands that operate on an existing pod, select a pod with `--pod`, set
`ORQA_POD`, or run the command from inside a pod directory.

Create fins inside the pod:

```sh
orqa --pod sample-pod fin create planner --role "Turn the charter into tasks."
orqa --pod sample-pod fin create builder
```

Create a pod from a reusable global template:

```sh
orqa template create executive
orqa template fin create executive ceo --role "Own company direction and executive decisions."
orqa template fin create executive cto --role "Own technical architecture and delivery quality."
orqa template fin list executive
orqa template list
orqa pod create launch-team --path /path/to/project --template executive
orqa --pod launch-team template sync executive --dry-run
orqa --pod launch-team template sync executive
```

Templates live under `ORQA_HOME/templates/<template-slug>/` and may use either
`fins/<fin>/ROLE.md` or `.orqa/fins/<fin>/ROLE.md`. Orqa creates a normal pod,
seeds `operator`, then creates each template fin with the predefined role and
standard fin directories. Template-generated fins record their template origin
in `fin.toml` so later syncs can distinguish them from manually created fins.

Template command summary:

```text
orqa template list
orqa template create <template>
orqa template sync <template> [--dry-run]
orqa template fin list <template>
orqa template fin create <template> <fin> --role <prompt|@file|->
orqa pod create <slug> --template <template> [--path <dir>] [--charter <prompt|@file|->]
```

`template create` initializes only the reusable template directory. It does not
create a real pod and does not create runtime-ready fins. Add template fins
explicitly with `template fin create`, which writes `ROLE.md` and a baseline
`fin.toml`, then materialize them through regular `pod create --template`.
When present, the template fin's `fin.toml` is used as the baseline generated
fin config. `template list` prints each template with its fin count and fin
slugs.

`template sync` applies a template to the selected pod. It always prints the
plan, adds missing template fins, updates template-owned role, AGENTS, and
fin.toml files, adopts same-named existing fins by recording template origin,
and deletes only fins whose `fin.toml` says they came from that template. Use
`--dry-run` to preview those changes without writing files.

Manage the durable pod charter and fin role files:

```sh
orqa --pod sample-pod pod charter get
orqa --pod sample-pod pod charter set @charter.md
orqa --pod sample-pod fin role get planner
orqa --pod sample-pod fin role set planner -
```

Print homes when an agent needs to inspect paths:

```sh
orqa pod list
orqa --pod sample-pod fin list
orqa --pod sample-pod pod home
orqa --pod sample-pod pod hook list
orqa --pod sample-pod fin home planner
orqa --pod sample-pod mail home planner
orqa --pod sample-pod task home planner
```

Inside a launched fin, `ORQA_POD` is already set, so a fin can list its
siblings with:

```sh
orqa fin list
```

Use `--home <DIR>` on any command to work against another Orqa root:

```sh
orqa --home /tmp/orqa-demo pod create sample-pod
```

## Running Fins

`orqa fin exec` launches one fin through the backend selected by `pod.toml` and
`fin.toml`.

```sh
orqa --pod sample-pod fin exec planner -- "handle your open mail and tasks"
```

`orqa wake` runs one wake turn for the current pod. It scans fins with wake
signals (unread mail or open tasks) and launches eligible fins through their
configured backend. The run policy in `pod.toml` and `fin.toml` can debounce
repeated runs or wake idle fins periodically with `exec_always`.

```sh
cd /path/to/sample-pod
orqa wake
orqa wake -- "handle your open Orqa mail and tasks"
```

Preview what would happen without actually launching fins:

```sh
orqa wake --dry-run
```

Start an interactive backend chat as a fin with the backend's `chat_args`:

```sh
orqa --pod sample-pod fin chat planner
```

`fin chat` attaches stdin, stdout, and stderr directly to the terminal while
using the same fin environment and lock behavior as `fin exec`.

Backend processes start in the pod root. Runtime-specific homes stay isolated
under the fin data directory.

When Orqa launches a fin, it sets:

```text
ORQA_HOME=<home>
ORQA_POD=<pod-slug>
ORQA_FIN=<fin-slug>
HOME=<pod-root>
CODEX_HOME=<pod-root>/.orqa/fins/<fin-slug>/.codex
GROK_HOME=<pod-root>/.orqa/fins/<fin-slug>/.grok
HERMES_HOME=<pod-root>/.orqa/fins/<fin-slug>/.hermes
PI_CODING_AGENT_DIR=<pod-root>/.orqa/fins/<fin-slug>/.pi/agent
```

An agent can use `ORQA_POD` and `ORQA_FIN` to call mail and task commands with
short addresses. Orqa sets the standard `HOME` variable (plus classic
tool-specific variables for compatibility) so every backend keeps its state
isolated under the fin data home. Backends can also reference `{fin_home}` from
`exec_args` or `chat_args`.

For Codex and Grok, Orqa links the user's existing `~/.codex/auth.json` or
`~/.grok/auth.json` into the fin-local copy when the source exists and the fin
does not already have an auth file.

## Status And Runs

Inspect the current runtime state:

```sh
orqa --pod sample-pod pod status
orqa --pod sample-pod fin status planner
```

Check pod readiness, including filesystem shape, config resolution, backend
execution, and upstream LLM connectivity:

```sh
orqa --pod sample-pod pod doctor
orqa --pod sample-pod pod doctor --fin planner --timeout 60
```

Each fin exec records a small run directory under the fin data home:

```text
<pod-root>/.orqa/fins/<fin>/runs/<run-id>/
  stdout.log
  stderr.log
  events.jsonl
  command.txt
  status.json
```

Read recent run history and logs:

```sh
orqa --pod sample-pod fin runs planner
orqa --pod sample-pod fin run-status planner
orqa --pod sample-pod fin run-log planner
```

Tail recent output. `fin tail` defaults to the latest run for that fin; `pod
tail` reads the latest run for each fin in the pod:

```sh
orqa --pod sample-pod fin tail planner
orqa --pod sample-pod pod tail
orqa --pod sample-pod pod tail --fin planner --follow
```

## Backend Config

`pod.toml` defines the allowed backends for the pod and names the default:

```toml
[pod]
slug = "sample-pod"
default_backend = "codex"
debounce = "5m"
exec_always = "0"

[backends.codex]
enabled = true
command = "codex"
exec_args = [
    "exec",
    "--skip-git-repo-check",
    "--sandbox", "workspace-write",
    "--cd", "{pod_root}",
    "--model", "{model}",
    "{prompt}",
]
chat_args = [
    "--sandbox", "workspace-write",
    "--cd", "{pod_root}",
    "--model", "{model}",
]

[backends.codex.defaults]
model = "gpt-5.3-codex"
```

Generated `pod.toml` files enable the built-in backend definitions up front:
Codex, OpenCode, Hermes, Pi, Grok, and Ollama-through-Codex. They do nothing
unless a fin selects them. Custom runner examples stay commented because they
need a site-specific command.

The generated Ollama example uses `ollama launch codex` rather than raw
`ollama run`, so Codex still provides the tool loop, working directory,
sandboxing, and fin-local `CODEX_HOME` while Ollama supplies the model.

`fin.toml` can override the backend or backend values for one fin:

```toml
[fin]
slug = "planner"
# backend = "codex"
# debounce = "5m"
# exec_always = "3h"

[backend]
model = "gpt-5.3-codex"
```

`debounce` and `exec_always` are run policy durations. `debounce` prevents a fin
from running more often than the configured interval when work is waiting.
`exec_always` wakes an idle fin after the configured interval even when there is
no mail or task. Pod values are defaults; fin values override them. Durations
accept plain seconds or units such as `30s`, `5m`, `3h`, or `1d`. Use
`debounce = "0"` to run any time there is work, and `exec_always = "0"` to run
only when there is work.

Backend `exec_args` and `chat_args` are argv arrays, not shell strings.
Template values include `{orqa_home}`, `{pod}`, `{pod_root}`, `{pod_home}`, `{fin}`,
`{fin_home}`, `{codex_home}`, `{mail_home}`, `{task_home}`, `{model}`, and
`{prompt}`.

`{prompt}` is built from the arguments after `--` on `fin exec` or `loop`.

## Mail

Fins communicate through pod-local Maildir inboxes.

An address is:

```text
fin@pod.orqa
```

Send mail:

```sh
orqa mail send \
  --from planner@sample-pod.orqa \
  --to builder@sample-pod.orqa \
  --subject hello \
  "wake up"
```

Inside a launched fin, `ORQA_POD` and `ORQA_FIN` are already set, so short
addresses work:

```sh
orqa mail send --to builder --subject hello "wake up"
```

List, read, finish, or delete mail:

```sh
orqa mail list
orqa mail read <message-id>
orqa mail done <message-id>
orqa mail delete <message-id>
```

Unread messages live in `mail/new` and wake the receiving fin. Marking a message
done moves it to `mail/cur`, clearing that wake signal.

`operator@<pod>.orqa` is a reserved address. Mail sent there is forwarded to
the real operator mailbox at `operator@ops.orqa`:

```sh
orqa mail send \
  --from release@sample-pod.orqa \
  --to operator@sample-pod.orqa \
  --subject "Cloudflare auth expired" \
  "Cloudflare deploy is blocked until the operator logs in again."
```

## Tasks

Tasks use the same storage pattern as mail, but live under `tasks/`. Use tasks
for work assignments that should wake the assignee until completed.

Send a task:

```sh
orqa task send \
  --from planner@sample-pod.orqa \
  --to builder@sample-pod.orqa \
  --title update-settings \
  "please update the settings"
```

Inside a launched fin:

```sh
orqa task send --to builder --title update-settings "please do this"
```

Task bodies are Markdown documents with YAML front matter. Plain Markdown is
accepted; Orqa fills in canonical metadata:

```yaml
from: planner@sample-pod.orqa
to: builder@sample-pod.orqa
title: update-settings
priority: normal
status: open
kind: need
depends_on: []
```

List and filter tasks:

```sh
orqa task list
orqa task list --status open
orqa task list --priority high
orqa task list --kind need
orqa task list --field owner=planner
orqa task list --sort priority
```

Read, finish, or delete tasks:

```sh
orqa task read <task-id>
orqa task done <task-id>
orqa task delete <task-id>
```

Open tasks live in `tasks/new` and wake the assignee. Marking a task done moves
it to `tasks/cur`, clearing that wake signal.

## Ops

Use the `ops` namespace for human/operator visibility commands:

```sh
orqa ops
orqa ops report --since 1d
```

`orqa ops` is an alias for `orqa ops report`.

Use `orqa ops report` to print a Markdown evidence bundle for the current pod,
including task records, mail records, file paths, statuses, and clipped
context. `--since` accepts Unix seconds or relative durations such as `30m`,
`2h`, or `1d`.

## Pod Hooks

Use pod hooks for cheap lifecycle work around the wake loop:

```sh
orqa --pod ops pod hook add pre-plan 10-sync-external-mail -- ./10-sync-external-mail.sh
orqa --pod ops pod hook list
orqa --pod ops pod hook run pre-plan
orqa --pod ops pod hook disable pre-plan 10-sync-external-mail
orqa --pod ops pod hook enable pre-plan 10-sync-external-mail
```

The first supported phase is `pre-plan`, which runs at the start of `orqa wake`
and each `orqa loop` turn before mail, tasks, debounce, or `exec_always` are
evaluated. Hooks live under `<pod-root>/.orqa/hooks/pre-plan/` as
`<hook-id>.toml` plus an adjacent script. Commands run from the phase directory
in lexicographic filename order. Failed hooks are reported and the wake loop
continues.

Hook commands receive `ORQA_HOME`, `ORQA_POD`, `ORQA_POD_ROOT`,
`ORQA_POD_HOME`, `ORQA_HOOK`, `ORQA_HOOK_PHASE`, `ORQA_HOOK_HOME`, and
`ORQA_HOOK_STATE`. `ORQA_POD_HOME` points at `<pod-root>/.orqa`.

## Pause And Resume

Pause an entire pod:

```sh
orqa --pod sample-pod pod pause
```

Pause one fin:

```sh
orqa --pod sample-pod fin pause planner
```

Paused pods and fins are skipped by `orqa wake` and `orqa loop`. Clear pause
state with an explicit forced resume:

```sh
orqa --pod sample-pod pod resume --force
orqa --pod sample-pod fin resume planner --force
```

Run one scan while ignoring pause markers and debounce without removing pause
state:

```sh
orqa wake --force
```

## Running the Wake Loop

To run the wake loop continuously in the foreground, use:

```sh
orqa loop --interval 60 -- "handle your open Orqa mail and tasks"
```

`orqa loop` wakes the current pod repeatedly and sleeps between turns until the
process is interrupted.

To run all registered pods in a foreground terminal, use either the dense TUI:

```sh
orqa top
```

or the plain daemon-style loop:

```sh
orqa daemon --interval 10 -- "handle your open Orqa mail and tasks"
```

`orqa top` wakes all enabled pods on its loop tick, shows global pod/fin state,
and lets the operator pause, resume, and manually wake selected pods. `orqa
daemon` runs the same global wake loop without a TUI. Both rely on per-fin
runtime locks, so running one alongside another foreground controller may add
extra scan noise but should not start the same fin twice.

The old `orqa service` commands and `--forever` flag have been removed. Run
`orqa loop` directly when you want a repeated foreground loop.

## Runtime Locks

Direct fin runs and loop-launched runs use a per-fin lock file:

```text
<pod-root>/.orqa/fins/<fin>/run.lock
```

The lock is acquired before backend execution starts, so multiple terminals can
scan the same pod concurrently without starting the same fin twice. A new lock
starts as `state=claimed` with the launching process PID, then becomes
`state=running` with the child process PID and run id after the backend process
spawns. While the recorded PID is alive, later wake scans skip the fin. If the
PID is gone, Orqa removes the stale lock and the fin can run again.

## Useful Agent Routine

When a fin starts, a useful first routine is:

```sh
orqa mail list
orqa task list
```

Then read each relevant item:

```sh
orqa mail read <message-id>
orqa task read <task-id>
```

After handling work, clear completed wake signals:

```sh
orqa mail done <message-id>
orqa task done <task-id>
```

Send follow-up mail for conversation and send tasks for assigned work. Use
short fin slugs when running inside an Orqa-launched process, and fully
qualified `fin@pod.orqa` addresses when operating outside that context.
