# orqa

`orqa` is a small local coordinator for groups of local agent runtimes.

It does not try to be a full orchestration platform. Its job is to keep a pod
and fin filesystem layout, give fins simple local mail and task channels,
scan for wake signals, and shell out to the configured agent runtime when a fin
should execute or chat.

## Concepts

A **pod** is a registration over an existing directory on disk (typically a git
repository or research project folder). Orqa stores its coordination data inside
a `.orqa/` subdirectory of that folder. The pod owns backend definitions and
pod-local mail/task channels.

A **fin** is one agent runtime identity inside a pod. Each fin gets its own
isolated state under `.orqa/fins/<fin>/` (mail, tasks, run history, and
runtime-specific directories such as `.grok/` or `.codex/`).

When a fin runs, Orqa sets its working directory and `HOME` to the real pod
root. This means the agent operates directly inside your project files while
still keeping per-fin state isolated.

`ORQA_HOME` (defaults to `~/.orqa`) stores the registry (`config.toml`) that
maps pod slugs to their roots. Pod data lives in each pod root under `.orqa/`.
Reusable pod templates live under `ORQA_HOME/templates/<template-slug>/`.

**Recommended onboarding:** `cd my-project && orqa init`

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
my-project/
  .orqa/
    AGENTS.md      # pod-level runtime instructions
    CHARTER.md     # shared goal and operating charter
    pod.txt
    pod.toml
    fins/
      operator/    # seeded local human/operator identity
      planner/
        AGENTS.md  # fin-specific role instructions
        ROLE.md    # fin purpose inside the pod
        fin.txt
        fin.toml
        .codex/       # Codex state
        .hermes/      # Hermes state
        .pi/
          agent/
          sessions/
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
        .codex/
        .hermes/
        .pi/
          agent/
          sessions/
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

## Filesystem Architecture

Orqa has one global home and many pod roots.

The global home is selected by `--home`, then `ORQA_HOME`, then `~/.orqa`.
It stores the pod registry and reusable pod templates:

```text
~/.orqa/
  config.toml
  templates/
    executive/
      fins/
        ceo/
          ROLE.md
        cto/
          ROLE.md
```

The registry is the global map from pod slug to real project directory:

```toml
[registry]
version = 1

[pods.sample-pod]
path = "/Users/me/work/sample-pod"
enabled = true
```

Each pod root is an ordinary project directory with local Orqa state under
`.orqa/`. The pod root itself is where backend agents run and edit files.

```text
my-project/
  .orqa/
    AGENTS.md
    CHARTER.md
    pod.txt
    pod.toml
    sleep.lock             # optional pod pause marker
    hooks/
      pre-plan/
        10-sync.toml
        10-sync.sh
      state/
        10-sync/
    fins/
      operator/
        AGENTS.md
        ROLE.md
        fin.txt
        fin.toml
        sleep.lock         # operator is seeded paused
        mail/
          cur/
          new/
          tmp/
        tasks/
          cur/
          new/
          tmp/
        runs/
      planner/
        AGENTS.md
        ROLE.md
        fin.txt
        fin.toml
        sleep.lock         # optional fin pause marker
        run.lock           # present while a fin process is considered running
        latest-run         # latest run id pointer
        runs.jsonl         # append-only finished/spawn-failed run ledger
        .codex/
          auth.json        # symlinked from user auth when available
        .grok/
          auth.json        # symlinked from user auth when available
        .hermes/
        .pi/
          agent/
          sessions/
        mail/
          cur/
          new/
          tmp/
        tasks/
          cur/
          new/
          tmp/
        runs/
          <run-id>/
            command.txt
            events.jsonl
            status.json
            stdout.log
            stderr.log
```

`pod create` and `init` create the pod root state, seed the local `operator`
fin, register the pod in global config, and add `/.orqa` to the project
`.gitignore` when needed. `fin create` creates a runtime-ready fin under
`.orqa/fins/<fin>/`. `template create` only creates reusable template files in
the global home; template fins do not get runtime homes, maildirs, tasks, or run
state until a real pod is created with `pod create --template`.

Pod context resolution uses this order: explicit `--pod`, then `ORQA_POD`, then
the nearest ancestor containing `.orqa/pod.toml`. Registered pods use
`~/.orqa/config.toml`; local filesystem detection currently derives the detected
slug from the pod root directory name, so app code that needs the canonical
registry slug should prefer the registry when available.

## Configuration

Pods and fins have TOML config files:

```text
<pod-root>/.orqa/pod.toml
<pod-root>/.orqa/fins/<fin>/fin.toml
```

`pod.toml` owns backend definitions. This keeps command formats and backend
policy in one place for the whole pod. `pod create` enables built-in backend
definitions up front; a backend does not run unless a fin selects it. Custom
runner examples stay commented because they need a site-specific command:

```toml
# Orqa pod configuration.

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

[backends.grok]
enabled = true
command = "grok"
exec_args = ["-p", "{prompt}", "--output-format", "streaming-json", "--always-approve"]
chat_args = []

[backends.grok.defaults]
model = "grok-code-latest"

[backends.ollama_codex]
enabled = true
command = "ollama"
exec_args = [
    "launch", "codex",
    "--model", "{model}",
    "--",
    "exec",
    "--skip-git-repo-check",
    "--sandbox", "workspace-write",
    "--cd", "{pod_root}",
    "{prompt}",
]
chat_args = [
    "launch", "codex",
    "--model", "{model}",
    "--",
    "--sandbox", "workspace-write",
    "--cd", "{pod_root}",
]
```

`fin.toml` records per-fin backend values. A fin inherits the pod default
backend unless `fin.backend` is uncommented:

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
from running more often than the configured interval when mail or tasks are
waiting. `exec_always` wakes an idle fin after the configured interval even when
there is no mail or task. Pod values are defaults; fin values override them.
Durations accept plain seconds or units such as `30s`, `5m`, `3h`, or `1d`.
Use `debounce = "0"` to run any time there is work, and `exec_always = "0"` to
run only when there is work.

## Templates

Templates let you predefine a set of fins and roles once, then create new pods
with that starting roster already in place.

Template directories live in the global Orqa home:

```text
~/.orqa/
  templates/
    executive/
      fins/
        ceo/
          AGENTS.md
        cto/
          AGENTS.md
        cmo/
          AGENTS.md
        coo/
          AGENTS.md
```

Create an empty template and add fins with predefined roles:

```sh
orqa template create executive
orqa template fin create executive ceo --role "Own company direction and executive decisions."
orqa template fin create executive cto --role "Own technical architecture and delivery quality."
orqa template fin list executive
```

The full pod-style layout is also accepted:

```text
~/.orqa/templates/executive/.orqa/fins/ceo/AGENTS.md
```

Create a new pod from a template:

```sh
orqa pod create launch-team --path /path/to/project --template executive
```

The command creates the normal pod files, seeds the built-in `operator` fin,
then creates each template fin from its predefined template `AGENTS.md`,
generated runtime `AGENTS.md`, compatibility `ROLE.md`, `fin.toml`, maildir,
task queue, and runtime state directories. The generated runtime `AGENTS.md`
adds Orqa front matter and required-context instructions before the copied
template role content.
Templates may not include an `operator` fin because every pod owns that local
human identity automatically.

Backend argument lists are stored as argv arrays instead of shell strings. That
keeps quoting behavior predictable when prompts or paths contain spaces.

The generated examples follow the installed CLI shapes on this machine:

```text
Backend   exec_args shape                         chat_args shape
Codex     codex exec --skip-git... <prompt>      codex --sandbox ...
Grok      grok -p <prompt> --output-format ...   grok
OpenCode  opencode run ... <prompt>              opencode ...
Hermes    hermes --oneshot <prompt>              hermes chat ...
Pi        pi --print <prompt>                    pi ...
Ollama    ollama launch codex -- exec ...         ollama launch codex -- ...
```

Runtime state is fin-local. Orqa sets the standard `HOME` environment variable
to each fin's home directory so that every backend automatically discovers its
state under the usual dot-directory (`.codex`, `.grok`, `.hermes`, `.pi`, etc.).
For Codex, Orqa also sets `CODEX_HOME` to the fin home so Codex loads the fin's
generated `AGENTS.md` as its home-level instructions.

The generated Ollama + Codex example keeps Codex owning the tool loop and
fin-local state while Ollama supplies the model. OpenCode and raw Ollama server
state use their normal user-level locations unless you customize the backend
definition.

When `~/.codex/auth.json` (or `~/.grok/auth.json`) exists, Orqa symlinks it
into the corresponding fin directory (`.codex/auth.json` or `.grok/auth.json`)
if the fin does not already have one. This lets Codex and Grok reuse your
existing login while keeping other state isolated under the fin home.

The config files are seeded by `pod create` and `fin create`. `orqa fin exec`,
`orqa wake`, and `orqa loop` use them to choose and launch each fin's backend.

## Quick Start

The recommended way to start a pod inside a project:

```sh
cd my-project
orqa init
orqa fin create planner
orqa fin create builder
```

For explicit control (or when scripting), use:

```sh
orqa pod create my-project --path .
orqa pod create my-project --path /path/to/project --charter "..."
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

Escalate to the human/operator pod by mailing the reserved operator address:

```sh
orqa mail send \
  --from builder@sample-pod.orqa \
  --to operator@sample-pod.orqa \
  --subject "Cloudflare auth expired" \
  "Cloudflare deploy is blocked until the operator logs in again."
orqa mail list --pod ops --fin operator
```

Run one wake turn for the current pod:

```sh
cd /path/to/sample-pod
orqa wake
```

Preview wake decisions without launching fins:

```sh
orqa wake --dry-run
```

Run a fin directly through the configured backend:

```sh
orqa --pod sample-pod fin exec planner -- --help
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
  doctor.rs           pod readiness and backend connectivity checks
  mailbox/
    mod.rs            mail and task command behavior
    storage.rs        Maildir storage, addresses, ids, and pause markers
    tasks.rs          task front matter, filtering, sorting, and formatting
  model.rs            Orqa paths plus pod, fin, and address types
  runs.rs             run records, logs, latest pointers, and tailing
  runtime.rs          wake loop, process spawning, and run locks
  runtime_home.rs     fin-local runtime home setup
  status.rs           pod and fin status summaries
  main_test.rs        unit tests loaded from src/main.rs
tests/
  help_command.rs     embedded operational guide smoke test
  hygiene.rs          source hygiene ratchet
  pod_flow.rs         CLI integration flows
```

Run the normal checks with:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
```

## Fin Execution

`orqa fin exec` shells out to the backend selected by `pod.toml` and
`fin.toml`. A fin inherits `[pod].default_backend` unless `fin.toml` sets
`fin.backend`.

```sh
orqa --pod sample-pod fin exec planner -- "work on the next task"
```

Start an interactive backend chat as a fin with the backend's `chat_args`:

```sh
orqa --pod sample-pod fin chat planner
```

`fin chat` attaches stdin, stdout, and stderr directly to the terminal while
using the same fin environment and lock behavior as `fin exec`.

Backend processes start with their current directory set to the pod root and
`HOME` set to the fin home. The project workspace and agent identity home stay
separate.

When a fin runs, `orqa` sets these environment variables:

```text
ORQA_HOME=<home>
ORQA_POD=<pod-slug>
ORQA_FIN=<fin-slug>
HOME=<pod-root>/.orqa/fins/<fin-slug>
CODEX_HOME=<pod-root>/.orqa/fins/<fin-slug>
```

The `ORQA_*` variables give commands executed by the fin enough context to use
short mail addresses. Setting the standard `HOME` variable lets supported
backends keep state isolated under the fin data home instead of sharing your
global user profile. Backends can also reference `{fin_home}` or `{home}` from
`exec_args` or `chat_args`.

For Codex and Grok, Orqa automatically links the user's existing
`~/.codex/auth.json` or `~/.grok/auth.json` into the fin-local copy when the
source exists and the fin does not already have an auth file.

Direct fin runs and loop-launched runs use a per-fin lock file:

```text
<pod-root>/.orqa/fins/<fin>/run.lock
```

The lock records the child process PID. If the lock exists and the PID is still
alive, another wake scan skips that fin. If the PID is gone, `orqa` treats the
lock as stale, removes it, and the fin can run again.

Pods and fins can also be paused manually:

```sh
orqa --pod sample-pod pod pause
orqa --pod sample-pod fin pause planner
```

Paused pods and fins are skipped by `orqa wake` and `orqa loop`. Clear pause
state with an explicit forced resume:

```sh
orqa --pod sample-pod pod resume --force
orqa --pod sample-pod fin resume planner --force
```

Use `wake --force` to run one wake turn while ignoring pause markers and
debounce without removing pause state:

```sh
orqa wake --force
```

## Status, Runs, And Tail

Runtime status commands summarize wake signals, pause state, live locks, and
the latest recorded run:

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

Each direct or loop-launched fin exec records logs and status under the fin:

```text
<pod-root>/.orqa/fins/<fin>/runs/<run-id>/
  stdout.log
  stderr.log
  events.jsonl
  command.txt
  status.json
```

Inspect run history and logs:

```sh
orqa --pod sample-pod fin runs planner
orqa --pod sample-pod fin run-status planner
orqa --pod sample-pod fin run-log planner
```

`fin tail` prints the latest run output for one fin. `pod tail` prints the
latest run output for every fin in a pod, or one fin with `--fin`:

```sh
orqa --pod sample-pod fin tail planner
orqa --pod sample-pod pod tail
orqa --pod sample-pod pod tail --fin planner --follow
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
<sample-pod-root>/.orqa/fins/builder/mail/new/
```

Unread messages in `mail/new` are wake signals. `orqa wake` scans the current
pod's fin inboxes and prints fins that should run:

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

Each pod has a local `operator` fin. To escalate work to the central ops pod,
send mail explicitly to `operator@ops.orqa`:

```sh
orqa mail send \
  --from release@sample-pod.orqa \
  --to operator@ops.orqa \
  --subject "Railway auth expired" \
  "Railway CLI is not logged in."
```

The ops pod can receive cross-pod mail from any pod; ordinary cross-pod mail
between non-ops pods remains blocked.

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
<sample-pod-root>/.orqa/fins/builder/tasks/new/
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
With no command, `orqa` prints a live status overview (pods, wake signals, and
totals) plus a hint to run `--help`.

### Top Level

```text
Coordinate local agent pods and fins

Usage: orqa [OPTIONS] [COMMAND]

Options:
      --home <DIR>  Override ORQA_HOME for this command
      --pod <SLUG>  Explicit pod context for commands that operate inside a pod
      --fin <SLUG>  Explicit fin context for commands that operate on one fin
  -v, --version     Print version
  -h, --help        Print help

Commands:
  doctor    Show runtime diagnostics
  guide     Print the operational guide
  init      Initialize a pod in this directory
  pod       Manage pods
  fin       Manage fins
  mail      Send and read fin mail
  task      Assign and track fin tasks
  template  Manage pod templates
  ops       Monitor pods
  wake      Run one wake cycle
  loop      Run wake cycles repeatedly
  help      Print this message or the help of the given subcommand(s)
```

`orqa doctor` prints basic runtime information, including the active
`ORQA_HOME`.

`orqa guide` prints an embedded Markdown operational guide for agents and humans
who need the runtime overview without install or development notes.

### Pod Commands

```text
orqa init [slug] [--path <dir>] [--charter <...>]
orqa pod create <slug> [--path <dir>] [--charter <prompt|@file|->] [--template <template>]
orqa pod list
orqa pod home
orqa pod charter get
orqa pod charter set <prompt|@file|->
orqa pod status
orqa pod doctor [--fin <fin>] [--prompt <prompt>] [--timeout <seconds>]
orqa pod hook list
orqa pod hook add pre-plan <hook-id> [--timeout <duration>] -- <command>
orqa pod hook enable pre-plan <hook-id>
orqa pod hook disable pre-plan <hook-id>
orqa pod hook remove pre-plan <hook-id>
orqa pod hook run pre-plan
orqa pod tail [--fin <fin>] [--lines <n>] [--follow]
orqa pod pause
orqa pod resume --force
```

**Recommended:** Use `orqa init` when working inside a project directory.
`orqa pod create --path` is the explicit form for scripts and power-user flows.
For commands that operate on an existing pod, the pod context is resolved from
`--pod`, then `ORQA_POD`, then the nearest `.orqa/pod.toml` in the current
directory tree. Pod slugs are positional only when creating or initializing a pod.

`pod create` creates `.orqa/`, `pod.toml`, `AGENTS.md`, and the seeded operator
fin inside the target pod root, then registers that root in global config. If
`--template <template>` is passed, Orqa validates `ORQA_HOME/templates/<template>`
before creating the pod, then seeds each template fin after the pod is
registered. If `--path` is omitted, the current directory is the pod root. The
charter is the shared goal and operating context for the pod; pass it inline,
from `@file.md`, or from stdin with `-`. The pod-level `AGENTS.md` injects that
charter and tells backend runtimes how to use Orqa mail, tasks, status, and fin
discovery. `pod charter set` replaces both `CHARTER.md` and the generated pod
`AGENTS.md`.

`pod list` prints one status line per pod with fin count, pause state,
wakeable/running counts, unread mail, and open tasks. `pod doctor` checks
required pod and fin files, resolves each fin's backend command, and runs a
short backend probe to verify connectivity. `pod hook` manages shell hooks under
`hooks/<phase>/` for lifecycle work around the wake loop. `pod pause` writes a
pod-level pause marker, and `pod resume` requires `--force` before it removes that
marker.

### Template Commands

```text
orqa template list
orqa template create <template>
orqa template fin list <template>
orqa template fin create <template> <fin> --role <prompt|@file|->
orqa pod create <slug> --template <template> [--path <dir>] [--charter <prompt|@file|->]
```

`template create` initializes `ORQA_HOME/templates/<template>/fins/` without
creating any real pod or fin runtime state. `template fin create` adds a fin
definition to that template by writing `fins/<fin>/AGENTS.md` and a baseline
`fins/<fin>/fin.toml`; pass the role inline, from `@file.md`, or from stdin
with `-`. `template list` prints each template with its fin count and fin slugs;
`template fin list` prints the fin slugs defined by one template. To materialize
the template, use the regular pod command with `--template`; Orqa uses each
template fin's `fin.toml` as the baseline generated config when present.

### Fin Commands

```text
orqa fin create <fin>
orqa fin create <fin> --role <prompt|@file|->
orqa fin list
orqa fin home [fin]
orqa fin role get [fin]
orqa fin role set [fin] <prompt|@file|->
orqa fin status [fin]
orqa fin runs [fin]
orqa fin run-status [fin] [run-id|latest]
orqa fin run-log [fin] [run-id|latest]
orqa fin tail [fin] [run-id|latest] [--lines <n>] [--follow]
orqa fin pause [fin]
orqa fin resume [fin] --force
orqa fin exec [fin] [-- <args>...]
orqa fin chat [fin] [-- <args>...]
```

`fin create` creates the fin home, `ROLE.md`, fin-level `AGENTS.md`, runtime state
directories such as `.codex/`, `.hermes/`, and `.pi/`, `mail/`, `tasks/`,
`fin.txt`, and `fin.toml`. The role is the fin-specific purpose inside the pod;
pass it inline, from `@file.md`, or from stdin with `-`. The fin-level
`AGENTS.md` injects that role for the backend runtime. `fin role set` replaces
both `ROLE.md` and the generated fin `AGENTS.md`. `fin list` prints fin slugs for
the current pod context, resolved from `--pod`, `ORQA_POD`, or the current
directory. `fin exec`
launches the configured backend and passes any arguments after `--` as the
`{prompt}` template value:

```sh
orqa --pod sample-pod fin exec planner -- "work on the next task"
orqa --pod sample-pod fin chat planner
```

`fin resume` requires `--force` before it removes a fin-level pause marker.

### Mail Commands

```text
orqa mail home [fin]
orqa mail send [--from <from>] --to <to> [--subject <subject>] [body]
orqa mail list [--pod <pod>] [--fin <fin>] [--all]
orqa mail read [--pod <pod>] [--fin <fin>] <message>
orqa mail done [--pod <pod>] [--fin <fin>] <message>
orqa mail delete [--pod <pod>] [--fin <fin>] <message>
orqa mail unread [fin]
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
orqa task home [fin]
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
orqa ops report [--since <when>]
```

`orqa ops` is an alias for `orqa ops report`. The `ops` namespace is reserved
for human/operator visibility and control commands.

`ops report` prints a Markdown evidence bundle for the current pod: fins, task
records, mail records, file paths, statuses, and clipped context. `--since`
accepts Unix seconds or relative durations such as `30m`, `2h`, or `1d`.

### Wake

```text
orqa wake [--dry-run] [--force] [--json] [-- <args>...]
```

`orqa wake` runs one wake turn for the current pod. It scans fins with unread
mail or open tasks and launches eligible fins through their configured backend.
`--dry-run` prints the wake plan without launching fins. The run policy in
`pod.toml` and `fin.toml` can debounce repeated runs or wake idle fins
periodically with `exec_always`.

### Pod Hooks

```text
orqa pod hook list
orqa pod hook add pre-plan <hook-id> [--timeout <duration>] -- <command>
orqa pod hook enable pre-plan <hook-id>
orqa pod hook disable pre-plan <hook-id>
orqa pod hook remove pre-plan <hook-id>
orqa pod hook run pre-plan
```

Hooks are pod-local shell commands stored under
`<pod-root>/.orqa/hooks/<phase>/`. The first supported phase is `pre-plan`,
which runs at the start of `orqa wake` and each `orqa loop` turn before Orqa
checks mail, tasks, debounce, or `exec_always`. This is intended for cheap
local synchronization, such as syncing an external inbox to disk and delivering
new messages into the operator fin before wake planning.

`pod hook add` writes `<hook-id>.toml` and an adjacent `<hook-id>.sh` script
stub. Hook TOML is intentionally small:

```toml
[hook]
enabled = true
command = "./10-sync-external-mail.sh"
timeout = "30s"
```

Commands run from the phase directory in lexicographic filename order, so ids
like `10-sync-mail` and `20-import-events` give stable priority. Failed or timed
out hooks are reported and the loop continues to normal wake planning.

Hook commands receive these environment variables: `ORQA_HOME`, `ORQA_POD`,
`ORQA_POD_ROOT`, `ORQA_POD_HOME`, `ORQA_HOOK`, `ORQA_HOOK_PHASE`,
`ORQA_HOOK_HOME`, and `ORQA_HOOK_STATE`. `ORQA_POD_HOME` is the pod data
directory (`<pod-root>/.orqa`). The state directory is
`<pod-root>/.orqa/hooks/state/<hook-id>/`.

```sh
orqa wake
orqa wake -- "handle your open Orqa mail and tasks"
orqa wake --force
```

For each wakeable fin, `orqa wake` creates `run.lock` with the spawned process
PID. Later scans skip that fin while the PID is alive. Stale locks are removed
when the PID no longer exists. Paused pods and fins are skipped unless
`--force` is used.

### Running the Wake Loop

`orqa loop` repeatedly wakes the current pod in the foreground until it is
interrupted:

```sh
orqa loop --interval 60
orqa loop --interval 60 -- "handle your open Orqa mail and tasks"
```

## Status

This is intentionally early and small. The current implementation defines the
filesystem contract, creates pods and fins, delivers local Maildir messages
and tasks, detects wake signals, and shells out to a configured backend.
