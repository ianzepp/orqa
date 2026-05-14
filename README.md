# orqa

Fan out work to background agents.

## Model

`orqa` keeps a small filesystem model:

```text
ORQA_HOME/
  pods/
    sample-pod/
      agents/
        amy/
          .codex/
          mail/
            cur/
            new/
            tmp/
```

`ORQA_HOME` defaults to `~/.orqa`. A pod is referenced by slug. An agent is
referenced by slug inside a pod.

## Usage

```sh
orqa --help
orqa doctor
orqa pod create sample-pod
orqa agent create sample-pod amy
orqa agent create sample-pod bob-jones
orqa mail send --from amy@sample-pod.orqa --to bob-jones@sample-pod.orqa --subject hello "wake up"
ORQA_POD=sample-pod ORQA_AGENT=amy orqa mail send --to bob-jones --subject hello "wake up"
orqa loop sample-pod
orqa agent run sample-pod amy -- --help
```

When an agent runs, `orqa` sets `CODEX_HOME` to that agent's `.codex`
directory before shelling out to the configured framework.

Agents communicate through pod-local Maildir inboxes. For example,
`amy@sample-pod.orqa` can send to `bob-jones@sample-pod.orqa`, and the message
lands in `bob-jones/mail/new`. The loop command treats unread messages in
`mail/new` as a wake signal.
