# Orqa Operational Guide

Orqa is a local coordinator for background agents. It gives a group of agents a
shared filesystem shape, names the group as a pod, names each agent as a fin,
and provides local mail and task queues that can wake fins when there is work
to handle.

Orqa does not decide what the agent should think or how the agent framework
works. It creates the homes, inboxes, task queues, config files, and runtime
environment that let an agent process understand where it is and how to talk to
the other fins in its pod.

## Core Model

A pod is a collection of fins. Use a pod when a set of agents should share a
workspace, communicate with each other, and be woken by the same scan loop.

A fin is one agent identity inside one pod. Each fin has its own home directory,
its own `.codex` directory, its own mail inbox, and its own task queue.

`ORQA_HOME` is the root for all pods. It defaults to `~/.orqa`.

```text
ORQA_HOME/
  pods/
    sample-pod/
      pod.toml
      fins/
        amy/
          fin.toml
          .codex/
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
```

Create fins inside it:

```sh
orqa fin create sample-pod amy
orqa fin create sample-pod bob-jones
```

Print homes when an agent needs to inspect paths:

```sh
orqa pod list
orqa fin list sample-pod
orqa pod home sample-pod
orqa fin home sample-pod amy
orqa mail home sample-pod amy
orqa task home sample-pod amy
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

`orqa fin run` launches one fin through the backend selected by `pod.toml` and
`fin.toml`.

```sh
orqa fin run sample-pod amy -- "handle your open mail and tasks"
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

Use `--framework` to bypass config for a one-off smoke test:

```sh
orqa fin run --framework /bin/echo sample-pod amy -- "hello"
orqa loop --framework /bin/echo sample-pod -- "wake scan"
```

When Orqa launches a fin, it sets:

```text
ORQA_HOME=<home>
ORQA_POD=<pod-slug>
ORQA_FIN=<fin-slug>
CODEX_HOME=<home>/pods/<pod-slug>/fins/<fin-slug>/.codex
```

An agent can use `ORQA_POD` and `ORQA_FIN` to call mail and task commands with
short addresses. Codex uses `CODEX_HOME` for fin-specific state.

## Status And Runs

Inspect the current runtime state:

```sh
orqa pod status sample-pod
orqa fin status sample-pod amy
```

Each fin run records a small run directory under the fin home:

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
orqa fin runs sample-pod amy
orqa fin run-status sample-pod amy
orqa fin run-log sample-pod amy
```

Tail recent output. `fin tail` defaults to the latest run for that fin; `pod
tail` reads the latest run for each fin in the pod:

```sh
orqa fin tail sample-pod amy
orqa pod tail sample-pod
orqa pod tail sample-pod --fin amy --follow
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
args = ["{prompt}"]

[backends.codex.defaults]
model = "gpt-5.3-codex"
```

`fin.toml` can override the backend or backend values for one fin:

```toml
[fin]
slug = "amy"
# backend = "codex"

[backend]
model = "gpt-5.3-codex"
```

Backend args are argv arrays, not shell strings. Template values include
`{orqa_home}`, `{pod}`, `{pod_home}`, `{fin}`, `{fin_home}`, `{codex_home}`,
`{mail_home}`, `{task_home}`, `{model}`, and `{prompt}`.

`{prompt}` is built from the arguments after `--` on `fin run` or `loop`.

## Mail

Fins communicate through pod-local Maildir inboxes.

An address is:

```text
fin@pod.orqa
```

Send mail:

```sh
orqa mail send \
  --from amy@sample-pod.orqa \
  --to bob-jones@sample-pod.orqa \
  --subject hello \
  "wake up"
```

Inside a launched fin, `ORQA_POD` and `ORQA_FIN` are already set, so short
addresses work:

```sh
orqa mail send --to bob-jones --subject hello "wake up"
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

## Tasks

Tasks use the same storage pattern as mail, but live under `tasks/`. Use tasks
for work assignments that should wake the assignee until completed.

Send a task:

```sh
orqa task send \
  --from amy@sample-pod.orqa \
  --to bob-jones@sample-pod.orqa \
  --title update-settings \
  "please update the settings"
```

Inside a launched fin:

```sh
orqa task send --to bob-jones --title update-settings "please do this"
```

Task bodies are Markdown documents with YAML front matter. Plain Markdown is
accepted; Orqa fills in canonical metadata:

```yaml
from: amy@sample-pod.orqa
to: bob-jones@sample-pod.orqa
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
orqa task list --field owner=amy
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

## Sleep And Wake

Put an entire pod to sleep:

```sh
orqa pod sleep sample-pod
```

Put one fin to sleep:

```sh
orqa fin sleep sample-pod amy
```

Sleeping pods and fins are skipped by `orqa loop`. Clear sleep state with an
explicit forced wake:

```sh
orqa pod wake sample-pod --force
orqa fin wake sample-pod amy --force
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
