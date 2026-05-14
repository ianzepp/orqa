# orqa

`orqa` is a small local coordinator for background agents.

It does not try to be a full orchestration platform. Its job is to keep a pod
and agent filesystem layout, give agents a simple local mail channel, scan for
wake signals, and shell out to the configured agent framework when an agent
should run.

## Concepts

A **pod** is a collection of agents that can communicate with each other.

An **agent** belongs to exactly one pod. Each agent has its own home directory
inside that pod, including an isolated `.codex` directory for Codex state and a
Maildir inbox for pod-local messages.

`ORQA_HOME` is the root directory for all pods. It defaults to `~/.orqa`.

```text
ORQA_HOME/
  pods/
    sample-pod/
      pod.txt
      agents/
        amy/
          agent.txt
          .codex/
          mail/
            cur/
            new/
            tmp/
        bob-jones/
          agent.txt
          .codex/
          mail/
            cur/
            new/
            tmp/
```

Pods and agents are referenced by slug. Slugs may contain lowercase letters,
digits, and hyphens.

## Quick Start

```sh
orqa pod create sample-pod
orqa agent create sample-pod amy
orqa agent create sample-pod bob-jones
```

Send a fully qualified pod-local message:

```sh
orqa mail send \
  --from amy@sample-pod.orqa \
  --to bob-jones@sample-pod.orqa \
  --subject hello \
  "wake up"
```

Scan the pod for agents with unread mail:

```sh
orqa loop sample-pod
```

Run an agent through the default framework, currently `codex`:

```sh
orqa agent run sample-pod amy -- --help
```

Everything can run against a temporary or alternate root with `--home`:

```sh
orqa --home /tmp/orqa-demo pod create sample-pod
```

## Agent Execution

`orqa agent run` shells out to an agent framework. By default, that executable
is `codex`.

```sh
orqa agent run sample-pod amy -- "work on the next task"
```

Use `--framework` to run another executable:

```sh
orqa agent run sample-pod amy --framework /bin/echo -- "hello"
```

When an agent runs, `orqa` sets these environment variables:

```text
ORQA_HOME=<home>
ORQA_POD=<pod-slug>
ORQA_AGENT=<agent-slug>
CODEX_HOME=<home>/pods/<pod-slug>/agents/<agent-slug>/.codex
```

That lets Codex use the agent-specific `.codex` directory as its config home.
It also gives commands executed by the agent enough context to use short mail
addresses.

## Mail

Agents communicate through pod-local Maildir inboxes.

An address has the form:

```text
agent@pod.orqa
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
ORQA_HOME/pods/sample-pod/agents/bob-jones/mail/new/
```

Unread messages in `mail/new` are wake signals. `orqa loop sample-pod` scans
agent inboxes and prints agents that should run:

```text
wake sample-pod/bob-jones unread=1
```

### Short Addresses

Inside an Orqa-launched agent process, `ORQA_POD` and `ORQA_AGENT` are already
set. In that context, an agent can send mail with just the recipient slug:

```sh
orqa mail send --to bob-jones --subject hello "wake up"
```

That resolves to:

```text
from: amy@sample-pod.orqa
to:   bob-jones@sample-pod.orqa
```

Outside an agent context, either use fully qualified addresses or provide a
fully qualified sender so the pod can be inferred:

```sh
orqa mail send \
  --from amy@sample-pod.orqa \
  --to bob-jones \
  --subject hello \
  "wake up"
```

If `orqa` cannot infer the pod, it returns an error asking for `agent@pod.orqa`
or the relevant environment variables.

## Commands

### `orqa doctor`

Prints basic runtime information, including the active `ORQA_HOME`.

```sh
orqa doctor
```

### `orqa pod create <pod>`

Creates a pod home directory and its `agents` directory.

```sh
orqa pod create sample-pod
```

### `orqa pod home <pod>`

Prints the filesystem path for a pod.

```sh
orqa pod home sample-pod
```

### `orqa agent create <pod> <agent>`

Creates an agent home directory, its `.codex` directory, and its Maildir inbox.

```sh
orqa agent create sample-pod amy
```

### `orqa agent home <pod> <agent>`

Prints the filesystem path for an agent.

```sh
orqa agent home sample-pod amy
```

### `orqa agent run <pod> <agent>`

Runs an agent through the configured framework.

```sh
orqa agent run sample-pod amy -- "work on the next task"
orqa agent run sample-pod amy --framework codex -- "work on the next task"
```

Arguments after `--` are passed through to the framework.

### `orqa mail home <pod> <agent>`

Prints the Maildir path for an agent.

```sh
orqa mail home sample-pod amy
```

### `orqa mail send`

Sends a pod-local message.

```sh
orqa mail send --from amy@sample-pod.orqa --to bob-jones@sample-pod.orqa "hello"
orqa mail send --to bob-jones --subject hello "hello from agent context"
```

If no message body is provided as an argument, `orqa` reads the body from stdin:

```sh
cat message.txt | orqa mail send --to bob-jones --subject hello
```

### `orqa mail unread <pod> <agent>`

Lists unread message files in an agent's `mail/new` inbox.

```sh
orqa mail unread sample-pod bob-jones
```

### `orqa loop <pod>`

Scans a pod for agents with unread mail. Today this reports wakeable agents;
future versions can use the same wake criteria to run them.

```sh
orqa loop sample-pod
```

## Status

This is intentionally early and small. The current implementation defines the
filesystem contract, creates pods and agents, delivers local Maildir messages,
detects unread-mail wake signals, and shells out to an agent framework.
