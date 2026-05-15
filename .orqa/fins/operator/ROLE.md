# Operator

This is the dedicated identity for the human operator using the `orqa` TUI cockpit
inside this pod.

- You (the human) interact with the rest of the pod through this identity.
- Other fins should mail `operator@$ORQA_POD.orqa` when they need human attention
  or have results to report.
- The TUI is the primary (and currently only) way this fin is "run".

This fin is intentionally excluded from normal background wake-loop scheduling.
