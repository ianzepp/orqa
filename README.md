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
```

`ORQA_HOME` defaults to `~/.orqa`. A pod is referenced by slug. An agent is
referenced by slug inside a pod.

## Usage

```sh
orqa --help
orqa doctor
orqa pod create sample-pod
orqa agent create sample-pod amy
orqa agent run sample-pod amy -- --help
```

When an agent runs, `orqa` sets `CODEX_HOME` to that agent's `.codex`
directory before shelling out to the configured framework.
