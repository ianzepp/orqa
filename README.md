# orqa

`orqa` is a small local coordinator for background fins.

It does not try to be a full orchestration platform. Its job is to keep a pod
and fin filesystem layout, give fins simple local mail and task channels,
scan for wake signals, and shell out to the configured framework when a fin
should run.

## Concepts

A **pod** is a collection of fins that can communicate with each other.

A **fin** belongs to exactly one pod. Each fin has its own home directory
inside that pod, including an isolated `.codex` directory for Codex state, a
Maildir inbox for pod-local messages, and a Maildir-style task queue.

`ORQA_HOME` is the root directory for all pods. It defaults to `~/.orqa`.

```text
ORQA_HOME/
  pods/
    sample-pod/
      pod.txt
      pod.toml
      fins/
        amy/
          fin.txt
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
        bob-jones/
          fin.txt
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

## Configuration

Pods and fins have TOML config files:

```text
ORQA_HOME/pods/<pod>/pod.toml
ORQA_HOME/pods/<pod>/fins/<fin>/fin.toml
```

`pod.toml` owns backend definitions. This keeps command formats and framework
policy in one place for the whole pod:

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

[backends.opencode]
enabled = false
command = "opencode"
args = ["run", "--model", "{model}", "{prompt}"]

[backends.pi]
enabled = false
command = "pi"
args = ["exec", "--home", "{fin_home}", "--pod", "{pod}", "--fin", "{fin}", "{prompt}"]
```

`fin.toml` records the fin's backend choice and per-fin backend values:

```toml
[fin]
slug = "amy"
backend = "codex"

[backend]
model = "gpt-5.3-codex"
```

Backend argument lists are stored as argv arrays instead of shell strings. That
keeps quoting behavior predictable when prompts or paths contain spaces.

The config files are seeded by `pod create` and `fin create`. They are not wired
into execution yet; current execution still uses `orqa fin run --framework ...`
and `orqa loop --framework ...` while the backend command surface is being
designed.

## Quick Start

```sh
orqa pod create sample-pod
orqa fin create sample-pod amy
orqa fin create sample-pod bob-jones
```

Send a fully qualified pod-local message:

```sh
orqa mail send \
  --from amy@sample-pod.orqa \
  --to bob-jones@sample-pod.orqa \
  --subject hello \
  "wake up"
ORQA_POD=sample-pod ORQA_FIN=bob-jones orqa mail list
ORQA_POD=sample-pod ORQA_FIN=bob-jones orqa mail read <message-id>
ORQA_POD=sample-pod ORQA_FIN=bob-jones orqa mail done <message-id>
ORQA_POD=sample-pod ORQA_FIN=amy orqa task send --to bob-jones --title update-settings "please do this"
```

Run one wake scan for the pod:

```sh
orqa loop sample-pod
```

Run a fin directly through the default framework, currently `codex`:

```sh
orqa fin run sample-pod amy -- --help
```

Everything can run against a temporary or alternate root with `--home`:

```sh
orqa --home /tmp/orqa-demo pod create sample-pod
```

## Fin Execution

`orqa fin run` shells out to a framework. By default, that executable
is `codex`.

```sh
orqa fin run sample-pod amy -- "work on the next task"
```

Use `--framework` to run another executable:

```sh
orqa fin run sample-pod amy --framework /bin/echo -- "hello"
```

When a fin runs, `orqa` sets these environment variables:

```text
ORQA_HOME=<home>
ORQA_POD=<pod-slug>
ORQA_FIN=<fin-slug>
CODEX_HOME=<home>/pods/<pod-slug>/fins/<fin-slug>/.codex
```

That lets Codex use the fin-specific `.codex` directory as its config home.
It also gives commands executed by the fin enough context to use short mail
addresses.

Direct fin runs and loop-launched runs use a per-fin lock file:

```text
ORQA_HOME/pods/<pod>/fins/<fin>/run.lock
```

The lock records the child process PID. If the lock exists and the PID is still
alive, another wake scan skips that fin. If the PID is gone, `orqa` treats the
lock as stale, removes it, and the fin can run again.

## Mail

Fins communicate through pod-local Maildir inboxes.

An address has the form:

```text
fin@pod.orqa
```

For example, this message:

```sh
orqa mail send \
  --from amy@sample-pod.orqa \
  --to bob-jones@sample-pod.orqa \
  --subject update-settings \
  "please update the settings"
```

is delivered to:

```text
ORQA_HOME/pods/sample-pod/fins/bob-jones/mail/new/
```

Unread messages in `mail/new` are wake signals. `orqa loop sample-pod` scans
fin inboxes and prints fins that should run:

```text
wake sample-pod/bob-jones unread=1
```

When a fin finishes handling a message, it can mark that message done. This
moves the file from `mail/new` to `mail/cur`, which clears the wake signal:

```sh
orqa mail done <message-id>
```

Messages can also be deleted from either `mail/new` or `mail/cur`:

```sh
orqa mail delete <message-id>
```

### Short Addresses

Inside an Orqa-launched fin process, `ORQA_POD` and `ORQA_FIN` are already
set. In that context, a fin can send mail with just the recipient slug:

```sh
orqa mail send --to bob-jones --subject hello "wake up"
```

That resolves to:

```text
from: amy@sample-pod.orqa
to:   bob-jones@sample-pod.orqa
```

Outside a fin context, either use fully qualified addresses or provide a
fully qualified sender so the pod can be inferred:

```sh
orqa mail send \
  --from amy@sample-pod.orqa \
  --to bob-jones \
  --subject hello \
  "wake up"
```

If `orqa` cannot infer the pod, it returns an error asking for `fin@pod.orqa`
or the relevant environment variables.

## Tasks

Tasks use the same storage pattern as mail, but live under `tasks/`.

Sending mail to another fin is a communication request. Sending a task is a
work assignment:

```sh
orqa task send \
  --from amy@sample-pod.orqa \
  --to bob-jones@sample-pod.orqa \
  --title update-settings \
  "please update the settings"
```

That task is delivered to:

```text
ORQA_HOME/pods/sample-pod/fins/bob-jones/tasks/new/
```

Task bodies are Markdown documents with YAML front matter. Fins may provide a
complete front matter block, or they may provide plain Markdown and let `orqa`
fill in the canonical metadata.

Required task properties are:

```yaml
from: amy@sample-pod.orqa
to: bob-jones@sample-pod.orqa
title: update-settings
priority: normal
status: open
kind: need
depends_on: []
```

`kind` is either `need` or `want`. `depends_on` is a lightweight dependency list
for related task ids or names. Extra front matter properties are preserved.

For example, plain Markdown:

```sh
orqa task send --to bob-jones --title update-settings "please update the settings"
```

is stored as:

```markdown
---
from: amy@sample-pod.orqa
to: bob-jones@sample-pod.orqa
title: update-settings
priority: normal
status: open
kind: need
depends_on: []
---

please update the settings
```

A fin can also send a fuller task document:

```sh
cat <<'TASK' | orqa task send --to bob-jones
---
title: update-settings
priority: high
status: blocked
kind: need
depends_on: [choose-config-path]
---

Update the settings after the config path is decided.
TASK
```

Open tasks in `tasks/new` are wake signals, just like unread mail. When the
assignee finishes a task, it can mark the task done:

```sh
orqa task done <task-id>
```

This moves the task from `tasks/new` to `tasks/cur` and clears that wake signal.

## Commands

### `orqa doctor`

Prints basic runtime information, including the active `ORQA_HOME`.

```sh
orqa doctor
```

### `orqa pod create <pod>`

Creates a pod home directory and its `fins` directory.

```sh
orqa pod create sample-pod
```

### `orqa pod home <pod>`

Prints the filesystem path for a pod.

```sh
orqa pod home sample-pod
```

### `orqa fin create <pod> <fin>`

Creates a fin home directory, its `.codex` directory, its Maildir inbox, and
its task queue.

```sh
orqa fin create sample-pod amy
```

### `orqa fin home <pod> <fin>`

Prints the filesystem path for a fin.

```sh
orqa fin home sample-pod amy
```

### `orqa fin run <pod> <fin>`

Runs a fin through the configured framework.

```sh
orqa fin run sample-pod amy -- "work on the next task"
orqa fin run sample-pod amy --framework codex -- "work on the next task"
```

Arguments after `--` are passed through to the framework.

### `orqa mail home <pod> <fin>`

Prints the Maildir path for a fin.

```sh
orqa mail home sample-pod amy
```

### `orqa mail send`

Sends a pod-local message.

```sh
orqa mail send --from amy@sample-pod.orqa --to bob-jones@sample-pod.orqa "hello"
orqa mail send --to bob-jones --subject hello "hello from fin context"
```

If no message body is provided as an argument, `orqa` reads the body from stdin:

```sh
cat message.txt | orqa mail send --to bob-jones --subject hello
```

### `orqa mail list`

Lists unread messages for the current fin context. Use `--all` to include
done messages from `mail/cur`.

```sh
orqa mail list
orqa mail list --all
orqa mail list --pod sample-pod --fin bob-jones
```

The output includes the mail state, message id, and subject:

```text
new 1778757271046041.31124.0.orqa update-settings
```

### `orqa mail read <message>`

Prints a message. `<message>` may be the id from `mail list` or a full path.

```sh
orqa mail read 1778757271046041.31124.0.orqa
orqa mail read --pod sample-pod --fin bob-jones 1778757271046041.31124.0.orqa
```

### `orqa mail done <message>`

Marks an unread message done by moving it from `mail/new` to `mail/cur`.

```sh
orqa mail done 1778757271046041.31124.0.orqa
```

### `orqa mail delete <message>`

Deletes a message from `mail/new` or `mail/cur`.

```sh
orqa mail delete 1778757271046041.31124.0.orqa
```

### `orqa mail unread <pod> <fin>`

Lists unread message file paths in a fin's `mail/new` inbox. This is a
lower-level helper; fins usually want `orqa mail list`.

```sh
orqa mail unread sample-pod bob-jones
```

### `orqa task home <pod> <fin>`

Prints the task queue path for a fin.

```sh
orqa task home sample-pod amy
```

### `orqa task send`

Assigns a pod-local task.

```sh
orqa task send --from amy@sample-pod.orqa --to bob-jones@sample-pod.orqa --title update-settings "please do this"
orqa task send --to bob-jones --title update-settings "please do this"
cat task.md | orqa task send --to bob-jones
```

If no task body is provided as an argument, `orqa` reads the body from stdin:

```sh
cat task.md | orqa task send --to bob-jones --title update-settings
```

Task bodies are normalized into Markdown with YAML front matter. If `--title` is
omitted, `orqa` uses `title:` from the provided front matter or falls back to
`(untitled task)`.

### `orqa task list`

Lists open tasks for the current fin context. Use `--all` to include done
tasks from `tasks/cur`. Output is shell-friendly: state, id, and front matter
properties as `key=value` fields.

```sh
orqa task list
orqa task list --all
orqa task list --pod sample-pod --fin bob-jones
```

Example output:

```text
new 1778757936473943.33536.0.orqa priority=high status=blocked kind=want title="urgent task"
```

Filter by common task properties:

```sh
orqa task list --status open
orqa task list --priority high
orqa task list --kind need
```

Filter by custom front matter with `--field key=value`:

```sh
orqa task list --field owner=amy
orqa task list --status blocked --field owner=amy
```

Sort by a front matter key, or by `state` or `id`:

```sh
orqa task list --sort priority
orqa task list --sort title
orqa task list --sort priority --reverse
```

Known priorities sort by severity: `critical`/`urgent`, `high`,
`normal`/`medium`, then `low`.

### `orqa task read <task>`

Prints a task. `<task>` may be the id from `task list` or a full path.

```sh
orqa task read 1778757485781904.31898.0.orqa
```

### `orqa task done <task>`

Marks an open task done by moving it from `tasks/new` to `tasks/cur`.

```sh
orqa task done 1778757485781904.31898.0.orqa
```

### `orqa task delete <task>`

Deletes a task from `tasks/new` or `tasks/cur`.

```sh
orqa task delete 1778757485781904.31898.0.orqa
```

### `orqa loop <pod>`

Scans a pod for fins with unread mail or open tasks. Wakeable fins are
launched through the configured framework.

```sh
orqa loop sample-pod
orqa loop sample-pod --framework codex -- "handle your open Orqa mail and tasks"
```

For each wakeable fin, `orqa loop` creates `run.lock` with the spawned process
PID. Later scans skip that fin while the PID is alive. Stale locks are removed
when the PID no longer exists.

## Status

This is intentionally early and small. The current implementation defines the
filesystem contract, creates pods and fins, delivers local Maildir messages
and tasks, detects wake signals, and shells out to a framework.
