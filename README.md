# orqa

`orqa` is a small local coordinator for groups of local agent runtimes.

It does not try to be a full orchestration platform. Its job is to keep a pod
and fin filesystem layout, give fins simple local mail and task channels,
scan for wake signals, and shell out to the configured agent runtime when a fin
should execute or chat.

## Concepts

A **pod** is a collection of fins, or agents, that are intended to communicate
with each other around a common goal. The pod owns the shared local namespace,
backend definitions, and pod-local mail/task channels.

A **fin** is one agent runtime identity inside a pod. In practice, a fin can be
backed by runtimes such as Claude, Codex, OpenClaw, Hermes, Pi, or any custom
command you configure. Each fin has its own home directory inside the pod,
including isolated runtime state, a Maildir inbox for pod-local messages, and a
Maildir-style task queue.

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
      AGENTS.md      # pod-level runtime instructions
      CHARTER.md     # shared goal and operating charter
      pod.txt
      pod.toml
      fins/
        planner/
          AGENTS.md  # fin-specific role instructions
          ROLE.md    # fin purpose inside the pod
          fin.txt
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
        builder/
          AGENTS.md
          ROLE.md
          fin.txt
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

## Configuration

Pods and fins have TOML config files:

```text
ORQA_HOME/pods/<pod>/pod.toml
ORQA_HOME/pods/<pod>/fins/<fin>/fin.toml
```

`pod.toml` owns backend definitions. This keeps command formats and backend
policy in one place for the whole pod. `pod create` enables Codex by default
and writes commented examples for OpenCode, Hermes, Pi, and a custom runner:

```toml
# Orqa pod configuration.

[pod]
slug = "sample-pod"
default_backend = "codex"

# Codex is enabled by default.
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

# Enable and edit these examples if this pod should allow additional backends.

# [backends.opencode]
# enabled = true
# command = "opencode"
# exec_args = ["run", "--model", "{model}", "{prompt}"]
# chat_args = ["--model", "{model}"]

# [backends.hermes]
# enabled = true
# command = "hermes"
# exec_args = ["--model", "{model}", "--oneshot", "{prompt}"]
# chat_args = ["chat", "--model", "{model}"]

# [backends.pi]
# enabled = true
# command = "pi"
# exec_args = [
#     "--model", "{model}",
#     "--session-dir", "{fin_home}/.pi/sessions",
#     "--print",
#     "{prompt}",
# ]
# chat_args = ["--model", "{model}", "--session-dir", "{fin_home}/.pi/sessions"]
```

`fin.toml` records per-fin backend values. A fin inherits the pod default
backend unless `fin.backend` is uncommented:

```toml
[fin]
slug = "planner"
# backend = "codex"

[backend]
model = "gpt-5.3-codex"
```

Backend argument lists are stored as argv arrays instead of shell strings. That
keeps quoting behavior predictable when prompts or paths contain spaces.

The generated examples follow the installed CLI shapes on this machine:

```text
Backend   exec_args shape                    chat_args shape
Codex     codex exec --skip-git... <prompt> codex --sandbox ...
OpenCode  opencode run ... <prompt>          opencode ...
Hermes    hermes --oneshot <prompt>          hermes chat ...
Pi        pi --print <prompt>                pi ...
```

Runtime state is fin-local where the backend exposes a simple home variable:
Codex uses `.codex/` through `CODEX_HOME`, Hermes uses `.hermes/` through
`HERMES_HOME`, and Pi uses `.pi/agent/` through `PI_CODING_AGENT_DIR` plus
`.pi/sessions/` through the generated `--session-dir` args. OpenCode currently
uses its normal user-level config and data locations unless you customize its
backend definition.

When `~/.codex/auth.json` exists, Orqa symlinks it into a fin's `.codex/`
directory as `auth.json` if the fin does not already have one. This lets Codex
reuse the user's existing login while keeping other Codex state under the fin
home.

The config files are seeded by `pod create` and `fin create`. `orqa fin exec`
and `orqa loop` use them to choose and launch each fin's backend.

## Quick Start

```sh
orqa pod create sample-pod
orqa fin create sample-pod planner
orqa fin create sample-pod builder
```

Send a fully qualified pod-local message:

```sh
orqa mail send \
  --from planner@sample-pod.orqa \
  --to builder@sample-pod.orqa \
  --subject hello \
  "wake up"
ORQA_POD=sample-pod ORQA_FIN=builder orqa mail list
ORQA_POD=sample-pod ORQA_FIN=builder orqa mail read <message-id>
ORQA_POD=sample-pod ORQA_FIN=builder orqa mail done <message-id>
ORQA_POD=sample-pod ORQA_FIN=planner orqa task send --to builder --title update-settings "please do this"
```

Raise an operator issue by mailing the reserved operator address:

```sh
orqa mail send \
  --from builder@sample-pod.orqa \
  --to operator@sample-pod.orqa \
  --subject "Cloudflare auth expired" \
  "Cloudflare deploy is blocked until the operator logs in again."
orqa ops issues
```

Run one wake scan for the pod:

```sh
orqa loop sample-pod
```

Preview wake decisions without launching fins:

```sh
orqa plan sample-pod
orqa loop --dry-run sample-pod
```

Run a fin directly through the configured backend:

```sh
orqa fin exec sample-pod planner -- --help
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

`orqa fin exec` shells out to the backend selected by `pod.toml` and
`fin.toml`. A fin inherits `[pod].default_backend` unless `fin.toml` sets
`fin.backend`.

```sh
orqa fin exec sample-pod planner -- "work on the next task"
```

Start an interactive backend chat as a fin with the backend's `chat_args`:

```sh
orqa fin chat sample-pod planner
```

`fin chat` attaches stdin, stdout, and stderr directly to the terminal while
using the same fin environment and lock behavior as `fin exec`.

Backend processes start with their current directory set to the fin home. That
lets runtimes that discover instruction files from the working directory read
both the fin-level `AGENTS.md` and the pod-level `AGENTS.md` in parent
directories.

When a fin runs, `orqa` sets these environment variables:

```text
ORQA_HOME=<home>
ORQA_POD=<pod-slug>
ORQA_FIN=<fin-slug>
CODEX_HOME=<home>/pods/<pod-slug>/fins/<fin-slug>/.codex
HERMES_HOME=<home>/pods/<pod-slug>/fins/<fin-slug>/.hermes
PI_CODING_AGENT_DIR=<home>/pods/<pod-slug>/fins/<fin-slug>/.pi/agent
```

The `ORQA_*` variables give commands executed by the fin enough context to use
short mail addresses. Runtime-specific home variables let supported backends
keep their own state under the fin home instead of sharing a global user
profile. Backends that do not use one of these variables can still reference
`{fin_home}` from `exec_args` or `chat_args`.

For Codex, Orqa automatically links the user's existing `~/.codex/auth.json`
into the fin-local `.codex/auth.json` when the source exists and the fin does
not already have an auth file.

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
orqa fin sleep sample-pod planner
```

Sleeping pods and fins are skipped by the wake loop. Clear sleep state with an
explicit forced wake:

```sh
orqa pod wake sample-pod --force
orqa fin wake sample-pod planner --force
```

Use `loop --force` to run one wake scan while ignoring sleep markers without
removing them:

```sh
orqa loop --force sample-pod
```

## Status, Runs, And Tail

Runtime status commands summarize wake signals, sleep state, live locks, and
the latest recorded run:

```sh
orqa pod status sample-pod
orqa fin status sample-pod planner
```

Check pod readiness, including filesystem shape, config resolution, backend
execution, and upstream LLM connectivity:

```sh
orqa pod doctor sample-pod
orqa pod doctor sample-pod --fin planner --timeout 60
```

Each direct or loop-launched fin exec records logs and status under the fin:

```text
ORQA_HOME/pods/<pod>/fins/<fin>/runs/<run-id>/
  stdout.log
  stderr.log
  events.jsonl
  command.txt
  status.json
```

Inspect run history and logs:

```sh
orqa fin runs sample-pod planner
orqa fin run-status sample-pod planner
orqa fin run-log sample-pod planner
```

`fin tail` prints the latest run output for one fin. `pod tail` prints the
latest run output for every fin in a pod, or one fin with `--fin`:

```sh
orqa fin tail sample-pod planner
orqa pod tail sample-pod
orqa pod tail sample-pod --fin planner --follow
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
  --from planner@sample-pod.orqa \
  --to builder@sample-pod.orqa \
  --subject update-settings \
  "please update the settings"
```

is delivered to:

```text
ORQA_HOME/pods/sample-pod/fins/builder/mail/new/
```

Unread messages in `mail/new` are wake signals. `orqa loop sample-pod` scans
fin inboxes and prints fins that should run:

```text
wake sample-pod/builder pid=12345 unread_mail=1 open_tasks=0
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

`operator@<pod>.orqa` is reserved. Mail sent to that address is promoted into
an operator issue instead of being delivered to a fin inbox:

```sh
orqa mail send \
  --from release@sample-pod.orqa \
  --to operator@sample-pod.orqa \
  --subject "Railway auth expired" \
  "Railway CLI is not logged in."
```

The issue keeps the original body and derives its pod, fin, and title from the
mail. If the body starts with YAML front matter, fields such as `severity`,
`kind`, or `related_run` are preserved.

### Short Addresses

Inside an Orqa-launched fin process, `ORQA_POD` and `ORQA_FIN` are already
set. In that context, a fin can send mail with just the recipient slug:

```sh
orqa mail send --to builder --subject hello "wake up"
```

That resolves to:

```text
from: planner@sample-pod.orqa
to:   builder@sample-pod.orqa
```

Outside a fin context, either use fully qualified addresses or provide a
fully qualified sender so the pod can be inferred:

```sh
orqa mail send \
  --from planner@sample-pod.orqa \
  --to builder \
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
  --from planner@sample-pod.orqa \
  --to builder@sample-pod.orqa \
  --title update-settings \
  "please update the settings"
```

That task is delivered to:

```text
ORQA_HOME/pods/sample-pod/fins/builder/tasks/new/
```

Task bodies are Markdown documents with YAML front matter. Fins may provide a
complete front matter block, or they may provide plain Markdown and let `orqa`
fill in the canonical metadata.

Required task properties are:

```yaml
from: planner@sample-pod.orqa
to: builder@sample-pod.orqa
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
orqa task send --to builder --title update-settings "please update the settings"
```

is stored as:

```markdown
---
from: planner@sample-pod.orqa
to: builder@sample-pod.orqa
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
cat <<'TASK' | orqa task send --to builder
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
  fin     Create or operate fins inside a pod
  mail    Mail helpers for pod-local fin messages
  task    Task helpers for pod-local work items
  ops     Human operator surface for cross-pod monitoring and issues
  loop    Run the wake loop for a pod
  plan    Show the wake plan for a pod without running fins
  service Manage the background wake-loop service
```

`orqa doctor` prints basic runtime information, including the active
`ORQA_HOME`.

`orqa help` prints an embedded Markdown operational guide for agents and humans
who need the runtime overview without install or development notes.

### Pod Commands

```text
orqa pod create <slug>
orqa pod create <slug> --charter <prompt|@file|->
orqa pod list
orqa pod home <slug>
orqa pod charter get <slug>
orqa pod charter set <slug> <prompt|@file|->
orqa pod status <slug>
orqa pod doctor <slug> [--fin <fin>] [--prompt <prompt>] [--timeout <seconds>]
orqa pod tail <slug> [--fin <fin>] [--lines <n>] [--follow]
orqa pod sleep <slug>
orqa pod wake <slug> --force
```

`pod create` creates `ORQA_HOME/pods/<slug>/`, its `fins/` directory,
`CHARTER.md`, `AGENTS.md`, `pod.txt`, and `pod.toml`. The charter is the shared
goal and operating context for the pod; pass it inline, from `@file.md`, or from
stdin with `-`. The pod-level `AGENTS.md` injects that charter and tells backend
runtimes how to use Orqa mail, tasks, status, and fin discovery from inside the
pod. `pod charter set` replaces both `CHARTER.md` and the generated pod
`AGENTS.md`. `pod list` prints known pod slugs, one per line. `pod doctor`
checks required pod and fin files, resolves each fin's backend command, and
runs a short backend probe to verify connectivity. `pod sleep` writes a
pod-level sleep marker, and `pod wake` requires `--force` before it removes that
marker.

### Fin Commands

```text
orqa fin create <pod> <fin>
orqa fin create <pod> <fin> --role <prompt|@file|->
orqa fin list [pod]
orqa fin home <pod> <fin>
orqa fin role get <pod> <fin>
orqa fin role set <pod> <fin> <prompt|@file|->
orqa fin status <pod> <fin>
orqa fin runs <pod> <fin>
orqa fin run-status <pod> <fin> [run-id|latest]
orqa fin run-log <pod> <fin> [run-id|latest]
orqa fin tail <pod> <fin> [run-id|latest] [--lines <n>] [--follow]
orqa fin sleep <pod> <fin>
orqa fin wake <pod> <fin> --force
orqa fin exec <pod> <fin> [-- <args>...]
orqa fin chat <pod> <fin> [-- <args>...]
```

`fin create` creates the fin home, `ROLE.md`, fin-level `AGENTS.md`, runtime state
directories such as `.codex/`, `.hermes/`, and `.pi/`, `mail/`, `tasks/`,
`fin.txt`, and `fin.toml`. The role is the fin-specific purpose inside the pod;
pass it inline, from `@file.md`, or from stdin with `-`. The fin-level
`AGENTS.md` injects that role for the backend runtime. `fin role set` replaces
both `ROLE.md` and the generated fin `AGENTS.md`. `fin list` prints fin slugs for
the provided pod, or for `ORQA_POD` when the pod argument is omitted. `fin exec`
launches the configured backend and passes any arguments after `--` as the
`{prompt}` template value:

```sh
orqa fin exec sample-pod planner -- "work on the next task"
orqa fin chat sample-pod planner
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
orqa mail send --from planner@sample-pod.orqa --to builder@sample-pod.orqa "hello"
cat message.txt | orqa mail send --to builder --subject hello
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
orqa task list --field owner=planner
orqa task list --status blocked --field owner=planner
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

### Ops Commands

```text
orqa ops
orqa ops issues [--all] [--pod <pod>] [--fin <fin>]
                [--status <status>] [--severity <severity>] [--kind <kind>]
                [--field <key=value>] [--json]
orqa ops issue read <issue> [--json]
orqa ops issue ack <issue> [--json]
orqa ops issue resolve <issue> [--note <note>] [--wake]
orqa ops issue dismiss <issue> [--note <note>] [--wake]
```

`orqa ops` prints operator issue counts. `ops issues` lists open and
acknowledged issues from `operator/issues/new` and `operator/issues/cur`;
`--all` also includes closed issues from `operator/issues/closed`.

Issues are created when a fin mails `operator@<pod>.orqa`. Filters match issue
front matter exactly, and `--field` can be repeated for custom fields such as
`source=operator-mail` or `related_run=<run>`. `ops issue ack` moves an open
issue from `new` to `cur`. `resolve` and `dismiss` move the issue to `closed`,
record the operator note, and send a normal mail reply back to the originating
fin. Pass `--wake` to clear that fin's sleep marker after sending the reply.

### Wake Loop

```text
orqa loop [--force] <pod> [-- <args>...]
```

`orqa loop` scans a pod for fins with unread mail or open tasks. Wakeable fins
are launched through their configured backend.

```sh
orqa loop sample-pod
orqa loop sample-pod -- "handle your open Orqa mail and tasks"
orqa loop --force sample-pod
```

For each wakeable fin, `orqa loop` creates `run.lock` with the spawned process
PID. Later scans skip that fin while the PID is alive. Stale locks are removed
when the PID no longer exists. Sleeping pods and fins are skipped unless
`--force` is used.

### Service Commands

```text
orqa service install [--interval <seconds>] [--force] [-- <args>...]
orqa service uninstall
orqa service start
orqa service stop
orqa service status
orqa service run [--interval <seconds>] [--force] [-- <args>...]
```

The service command manages one background wake-loop service for the active
`ORQA_HOME`.

`service install` writes a platform service definition for the active
`ORQA_HOME`: a user LaunchAgent on macOS, or a user systemd unit on Linux. The
installed service repeatedly discovers all pods under `ORQA_HOME/pods/` and
runs the equivalent of `orqa loop <pod>` for each pod at the configured
interval. New pods are picked up on the next scan. Arguments after `--` are
preserved in the service definition for each pod scan.

Use `service start`, `service stop`, and `service status` to control the
installed service through `launchctl` or `systemctl --user`. Use
`service uninstall` to stop the service and remove its generated service file.
Use `service run` to run the same foreground loop directly when debugging a
service definition or watching scan output in a terminal.

## Status

This is intentionally early and small. The current implementation defines the
filesystem contract, creates pods and fins, delivers local Maildir messages
and tasks, detects wake signals, and shells out to a configured backend.
