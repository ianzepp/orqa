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
backed by runtimes such as Claude, Codex, OpenClaw, Hermes, Pi, or a custom
command. Each fin has its own home directory, runtime state directories, mail
inbox, and task queue.

`ORQA_HOME` is the root for all pods. It defaults to `~/.orqa`.

```text
ORQA_HOME/
  pods/
    sample-pod/
      AGENTS.md
      CHARTER.md
      pod.toml
      fins/
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

Create a pod:

```sh
orqa pod create sample-pod
orqa pod create sample-pod --charter "Build a focused launch plan."
```

Create fins inside it:

```sh
orqa fin create sample-pod planner --role "Turn the charter into tasks."
orqa fin create sample-pod builder
```

Manage the durable pod charter and fin role files:

```sh
orqa pod charter get sample-pod
orqa pod charter set sample-pod @charter.md
orqa fin role get sample-pod planner
orqa fin role set sample-pod planner -
```

Print homes when an agent needs to inspect paths:

```sh
orqa pod list
orqa fin list sample-pod
orqa pod home sample-pod
orqa fin home sample-pod planner
orqa mail home sample-pod planner
orqa task home sample-pod planner
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
orqa fin exec sample-pod planner -- "handle your open mail and tasks"
```

`orqa loop` scans a pod for fins with wake signals. Unread mail and open tasks
are wake signals. Each wakeable fin is launched through its configured backend.

```sh
orqa loop sample-pod
orqa loop sample-pod -- "handle your open Orqa mail and tasks"
```

Preview the same wake decisions without launching anything:

```sh
orqa plan sample-pod
orqa loop --dry-run sample-pod
```

Start an interactive backend chat as a fin with the backend's `chat_args`:

```sh
orqa fin chat sample-pod planner
```

`fin chat` attaches stdin, stdout, and stderr directly to the terminal while
using the same fin environment and lock behavior as `fin exec`.

Backend processes start in the fin home, so runtimes can discover the
fin-level `AGENTS.md` and the pod-level `AGENTS.md` in parent directories.

When Orqa launches a fin, it sets:

```text
ORQA_HOME=<home>
ORQA_POD=<pod-slug>
ORQA_FIN=<fin-slug>
CODEX_HOME=<home>/pods/<pod-slug>/fins/<fin-slug>/.codex
HERMES_HOME=<home>/pods/<pod-slug>/fins/<fin-slug>/.hermes
PI_CODING_AGENT_DIR=<home>/pods/<pod-slug>/fins/<fin-slug>/.pi/agent
```

An agent can use `ORQA_POD` and `ORQA_FIN` to call mail and task commands with
short addresses. Runtime-specific home variables keep supported backend state
under the fin home. Backends can also reference `{fin_home}` from `exec_args`
or `chat_args`.

For Codex, Orqa links the user's existing `~/.codex/auth.json` into the
fin-local `.codex/auth.json` when the source exists and the fin does not already
have an auth file.

## Status And Runs

Inspect the current runtime state:

```sh
orqa pod status sample-pod
orqa fin status sample-pod planner
```

Each fin exec records a small run directory under the fin home:

```text
ORQA_HOME/pods/<pod>/fins/<fin>/runs/<run-id>/
  stdout.log
  stderr.log
  events.jsonl
  command.txt
  status.json
```

Read recent run history and logs:

```sh
orqa fin runs sample-pod planner
orqa fin run-status sample-pod planner
orqa fin run-log sample-pod planner
```

Tail recent output. `fin tail` defaults to the latest run for that fin; `pod
tail` reads the latest run for each fin in the pod:

```sh
orqa fin tail sample-pod planner
orqa pod tail sample-pod
orqa pod tail sample-pod --fin planner --follow
```

## Backend Config

`pod.toml` defines the allowed backends for the pod and names the default:

```toml
[pod]
slug = "sample-pod"
default_backend = "codex"

[backends.codex]
enabled = true
command = "codex"
exec_args = [
    "exec",
    "--skip-git-repo-check",
    "--sandbox", "workspace-write",
    "--cd", "{pod_home}",
    "--model", "{model}",
    "{prompt}",
]
chat_args = [
    "--sandbox", "workspace-write",
    "--cd", "{pod_home}",
    "--model", "{model}",
]

[backends.codex.defaults]
model = "gpt-5.3-codex"
```

Generated `pod.toml` files also include commented examples for OpenCode,
Hermes, Pi, and custom runners.

`fin.toml` can override the backend or backend values for one fin:

```toml
[fin]
slug = "planner"
# backend = "codex"

[backend]
model = "gpt-5.3-codex"
```

Backend `exec_args` and `chat_args` are argv arrays, not shell strings.
Template values include `{orqa_home}`, `{pod}`, `{pod_home}`, `{fin}`,
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

`operator@<pod>.orqa` is a reserved address. Mail sent there becomes an
operator issue instead of normal fin mail:

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

## Operator Issues

Operator issues are task-like records for work that needs human or privileged
operator action. Fins create them by mailing `operator@<pod>.orqa`.

List, read, acknowledge, resolve, or dismiss issues:

```sh
orqa ops
orqa ops issues
orqa ops issue read <issue-id>
orqa ops issue ack <issue-id>
orqa ops issue resolve <issue-id> --note "Re-authenticated Cloudflare."
orqa ops issue dismiss <issue-id> --note "No longer relevant."
```

Resolving or dismissing an issue moves it to the closed issue store and sends a
normal mail message back to the originating fin. That returned mail is a wake
signal, so the fin can resume through the usual loop.

## Sleep And Wake

Put an entire pod to sleep:

```sh
orqa pod sleep sample-pod
```

Put one fin to sleep:

```sh
orqa fin sleep sample-pod planner
```

Sleeping pods and fins are skipped by `orqa loop`. Clear sleep state with an
explicit forced wake:

```sh
orqa pod wake sample-pod --force
orqa fin wake sample-pod planner --force
```

Run one scan while ignoring sleep markers without removing them:

```sh
orqa loop --force sample-pod
```

## Background Service

Use `orqa service` to install and control one background wake-loop service for
the active `ORQA_HOME`:

```sh
orqa service install --interval 60 -- "handle your open Orqa mail and tasks"
orqa service start
orqa service status
orqa service stop
orqa service uninstall
orqa service run --interval 60 -- "handle your open Orqa mail and tasks"
```

On macOS, Orqa writes a user LaunchAgent and controls it with `launchctl`. On
Linux, Orqa writes a user systemd unit and controls it with `systemctl --user`.
The service repeatedly discovers all pods under `ORQA_HOME/pods/` and runs the
equivalent of `orqa loop <pod>` for each pod at the interval chosen during
install. New pods are picked up on the next scan. Use `orqa service run` to run
that same foreground loop directly while debugging.

## Runtime Locks

Direct fin runs and loop-launched runs use a per-fin lock file:

```text
ORQA_HOME/pods/<pod>/fins/<fin>/run.lock
```

The lock records the child process PID. While that PID is alive, later wake
scans skip the fin. If the PID is gone, Orqa removes the stale lock and the fin
can run again.

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
