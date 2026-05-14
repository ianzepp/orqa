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

## Installation

Install the latest crates.io release with Cargo:

```sh
cargo install orqa
```

Install the latest GitHub release with the shell installer:

```sh
curl -fsSL https://raw.githubusercontent.com/ianzepp/orqa/main/install.sh | sh
```

The installer downloads a prebuilt archive for macOS Apple Silicon, macOS
Intel, or Linux Intel, verifies the published SHA-256 checksum when checksum
tools are available, and installs `orqa` to `~/.local/bin`. Set
`ORQA_INSTALL_DIR` to choose another directory:

```sh
curl -fsSL https://raw.githubusercontent.com/ianzepp/orqa/main/install.sh | ORQA_INSTALL_DIR=/usr/local/bin sh
```

Or install the prebuilt CLI with Homebrew:

```sh
brew install ianzepp/tap/orqa
```

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
policy in one place for the whole pod. `pod create` enables Codex by default
and writes commented examples for OpenCode, Pi, and a custom runner:

```toml
# Orqa pod configuration.

[pod]
slug = "sample-pod"
default_backend = "codex"

# Codex is enabled by default.
[backends.codex]
enabled = true
command = "codex"
args = ["{prompt}"]

[backends.codex.defaults]
model = "gpt-5.3-codex"

# Enable and edit these examples if this pod should allow additional backends.

# [backends.opencode]
# enabled = true
# command = "opencode"
# args = ["run", "--model", "{model}", "{prompt}"]

# [backends.pi]
# enabled = true
# command = "pi"
# args = [
#     "exec",
#     "--home", "{fin_home}",
#     "--pod", "{pod}",
#     "--fin", "{fin}",
#     "{prompt}",
# ]
```

`fin.toml` records per-fin backend values. A fin inherits the pod default
backend unless `fin.backend` is uncommented:

```toml
[fin]
slug = "amy"
# backend = "codex"

[backend]
model = "gpt-5.3-codex"
```

Backend argument lists are stored as argv arrays instead of shell strings. That
keeps quoting behavior predictable when prompts or paths contain spaces.

The config files are seeded by `pod create` and `fin create`. `orqa fin run`
and `orqa loop` use them when `--framework` is omitted. `--framework` remains
an explicit command override for quick smoke tests and one-off runs.

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

Run a fin directly through the configured backend:

```sh
orqa fin run sample-pod amy -- --help
```

Everything can run against a temporary or alternate root with `--home`:

```sh
orqa --home /tmp/orqa-demo pod create sample-pod
```

## Development

`orqa` is a single Rust binary crate. The command dispatcher stays in
`src/main.rs`, while the implementation is split by responsibility:

```text
src/
  cli.rs              clap command and argument definitions
  commands.rs         top-level command handlers
  config.rs           pod and fin config templates
  mailbox/
    mod.rs            mail and task command behavior
    storage.rs        Maildir storage, addresses, ids, and sleep markers
    tasks.rs          task front matter, filtering, sorting, and formatting
  model.rs            Orqa paths plus pod, fin, and address types
  runtime.rs          wake loop, process spawning, and run locks
  main_test.rs        unit tests loaded from src/main.rs
tests/
  hygiene.rs          source hygiene ratchet
```

Run the normal checks with:

```sh
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

## Fin Execution

`orqa fin run` shells out to the backend selected by `pod.toml` and
`fin.toml`. A fin inherits `[pod].default_backend` unless `fin.toml` sets
`fin.backend`.

```sh
orqa fin run sample-pod amy -- "work on the next task"
```

Use `--framework` to bypass config and run another executable:

```sh
orqa fin run --framework /bin/echo sample-pod amy -- "hello"
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

Pods and fins can also be put to sleep manually:

```sh
orqa pod sleep sample-pod
orqa fin sleep sample-pod amy
```

Sleeping pods and fins are skipped by the wake loop. Clear sleep state with an
explicit forced wake:

```sh
orqa pod wake sample-pod --force
orqa fin wake sample-pod amy --force
```

Use `loop --force` to run one wake scan while ignoring sleep markers without
removing them:

```sh
orqa loop --force sample-pod
```

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
wake sample-pod/bob-jones pid=12345 unread_mail=1 open_tasks=0
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

All commands accept the global `--home <DIR>` option to override `ORQA_HOME`.
With no command, `orqa` runs `doctor`.

### Top Level

```text
Usage: orqa [OPTIONS] [COMMAND]

Commands:
  doctor  Show basic runtime information
  help    Print the operational guide for agents using Orqa
  pod     Create or inspect pods
  fin     Create or run fins inside a pod
  mail    Mail helpers for pod-local fin messages
  task    Task helpers for pod-local work items
  loop    Run the wake loop for a pod
  service Manage the background wake-loop service
```

`orqa doctor` prints basic runtime information, including the active
`ORQA_HOME`.

`orqa help` prints an embedded Markdown operational guide for agents and humans
who need the runtime overview without install or development notes.

### Pod Commands

```text
orqa pod create <slug>
orqa pod list
orqa pod home <slug>
orqa pod sleep <slug>
orqa pod wake <slug> --force
```

`pod create` creates `ORQA_HOME/pods/<slug>/`, its `fins/` directory,
`pod.txt`, and `pod.toml`. `pod list` prints known pod slugs, one per line.
`pod sleep` writes a pod-level sleep marker, and `pod wake` requires `--force`
before it removes that marker.

### Fin Commands

```text
orqa fin create <pod> <fin>
orqa fin list [pod]
orqa fin home <pod> <fin>
orqa fin sleep <pod> <fin>
orqa fin wake <pod> <fin> --force
orqa fin run [--framework <framework>] <pod> <fin> [-- <args>...]
```

`fin create` creates the fin home, `.codex/`, `mail/`, `tasks/`, `fin.txt`,
and `fin.toml`. `fin list` prints fin slugs for the provided pod, or for
`ORQA_POD` when the pod argument is omitted. `fin run` launches the configured
backend unless `--framework` is provided, and passes any arguments after `--` as
the `{prompt}` template value:

```sh
orqa fin run sample-pod amy -- "work on the next task"
orqa fin run --framework /bin/echo sample-pod amy -- "hello"
```

`fin wake` requires `--force` before it removes a fin-level sleep marker.

### Mail Commands

```text
orqa mail home <pod> <fin>
orqa mail send [--from <from>] --to <to> [--subject <subject>] [body]
orqa mail list [--pod <pod>] [--fin <fin>] [--all]
orqa mail read [--pod <pod>] [--fin <fin>] <message>
orqa mail done [--pod <pod>] [--fin <fin>] <message>
orqa mail delete [--pod <pod>] [--fin <fin>] <message>
orqa mail unread <pod> <fin>
```

`mail send` requires `--to`. `--from` defaults to
`ORQA_FIN@ORQA_POD.orqa`; `--subject` defaults to `(no subject)`. If `body` is
omitted, `orqa` reads the message body from stdin:

```sh
orqa mail send --from amy@sample-pod.orqa --to bob-jones@sample-pod.orqa "hello"
cat message.txt | orqa mail send --to bob-jones --subject hello
```

`mail list` lists unread messages from `mail/new`; `--all` also includes done
messages from `mail/cur`. `mail read`, `mail done`, and `mail delete` accept a
message id, filename, or full path. `mail unread` is a lower-level helper that
prints unread message file paths.

### Task Commands

```text
orqa task home <pod> <fin>
orqa task send [--from <from>] --to <to> [--title <title>] [body]
orqa task list [--pod <pod>] [--fin <fin>] [--all]
               [--status <status>] [--priority <priority>] [--kind <kind>]
               [--field <key=value>] [--sort <key>] [--reverse]
orqa task read [--pod <pod>] [--fin <fin>] <message>
orqa task done [--pod <pod>] [--fin <fin>] <message>
orqa task delete [--pod <pod>] [--fin <fin>] <message>
```

`task send` requires `--to`. `--from` defaults to
`ORQA_FIN@ORQA_POD.orqa`. If `body` is omitted, `orqa` reads the task body from
stdin. Task bodies are normalized into Markdown with YAML front matter. If
`--title` is omitted, `orqa` uses `title:` from the provided front matter or
falls back to `(untitled task)`.

`task list` lists open tasks from `tasks/new`; `--all` also includes done tasks
from `tasks/cur`. Its output is shell-friendly: state, id, and front matter
properties as `key=value` fields.

```text
new 1778757936473943.33536.0.orqa priority=high status=blocked kind=want title="urgent task"
```

Filters match front matter exactly. `--field` can be repeated because it is a
normal option list:

```sh
orqa task list --status open
orqa task list --priority high
orqa task list --kind need
orqa task list --field owner=amy
orqa task list --status blocked --field owner=amy
```

Sort keys may be front matter keys, `state`, or `id`. Known priorities sort by
severity: `critical`/`urgent`, `high`, `normal`/`medium`, then `low`.

```sh
orqa task list --sort priority
orqa task list --sort title
orqa task list --sort priority --reverse
```

`task read`, `task done`, and `task delete` accept a task id, filename, or full
path. `task done` moves the task from `tasks/new` to `tasks/cur`.

### Wake Loop

```text
orqa loop [--force] [--framework <framework>] <pod> [-- <args>...]
```

`orqa loop` scans a pod for fins with unread mail or open tasks. Wakeable fins
are launched through their configured backend unless `--framework` is provided.

```sh
orqa loop sample-pod
orqa loop --framework codex sample-pod -- "handle your open Orqa mail and tasks"
orqa loop --force sample-pod
```

For each wakeable fin, `orqa loop` creates `run.lock` with the spawned process
PID. Later scans skip that fin while the PID is alive. Stale locks are removed
when the PID no longer exists. Sleeping pods and fins are skipped unless
`--force` is used.

### Service Commands

```text
orqa service install [--interval <seconds>] [--force] [--framework <framework>] [-- <args>...]
orqa service uninstall
orqa service start
orqa service stop
orqa service status
orqa service run [--interval <seconds>] [--force] [--framework <framework>] [-- <args>...]
```

The service command manages one background wake-loop service for the active
`ORQA_HOME`.

`service install` writes a platform service definition for the active
`ORQA_HOME`: a user LaunchAgent on macOS, or a user systemd unit on Linux. The
installed service repeatedly discovers all pods under `ORQA_HOME/pods/` and
runs the equivalent of `orqa loop <pod>` for each pod at the configured
interval. New pods are picked up on the next scan. `--framework` and arguments
after `--` are preserved in the service definition for each pod scan.

Use `service start`, `service stop`, and `service status` to control the
installed service through `launchctl` or `systemctl --user`. Use
`service uninstall` to stop the service and remove its generated service file.
Use `service run` to run the same foreground loop directly when debugging a
service definition or watching scan output in a terminal.

## Status

This is intentionally early and small. The current implementation defines the
filesystem contract, creates pods and fins, delivers local Maildir messages
and tasks, detects wake signals, and shells out to a framework.
