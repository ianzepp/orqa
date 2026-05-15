# Feedback on Using Orqa CLI (Grok Backend Testing Session)

**Date:** During Grok backend integration testing  
**Context:** First real-world usage of the `orqa` CLI to set up and exercise a brand new backend (Grok) while a background scheduler was already running.

## Major Issues Encountered

### 1. Permission / Provenance Issue on `~/.orqa` (Biggest Blocker)

Attempting to create a new pod in the default home failed with:

```
orqa: failed to create pod directory /Users/ianzepp/.orqa/pods/grok: Operation not permitted (os error 1)
```

Even a manual `mkdir` failed. The root cause was `com.apple.provenance` extended attributes applied across the entire `~/.orqa/pods/` tree by the running LaunchAgent service.

**Impact:**
- Could not create pods in the "live" home where the background scheduler was watching.
- Forced use of `--home /tmp/orqa-grok-test` for testing.
- The background scheduler never automatically picked up the new pod.

**Suggestion:** 
- Detect this situation and surface a clearer error message with recommended workarounds (`--home` or instructions for clearing provenance attributes).
- Document this macOS + LaunchAgent interaction.

### 2. `orqa loop` Prompt Requirement Not Obvious

When running:
```bash
orqa --home ... loop grok
```

The fin woke up, but Grok received an empty prompt (`grok -p  --always-approve`), resulting in:
> Error: --single: prompt is empty

It was not immediately clear that the correct invocation for manual/scheduler-style execution requires a prompt after `--`:

```bash
orqa loop <pod> -- "handle your open Orqa mail and tasks..."
```

This led to initial failed invocations until discovered through error messages.

**Suggestion:** Improve documentation or help text around `orqa loop` usage, especially the role of arguments after `--`.

### 3. Debounce UX During Active Testing

After an initial failed Grok run, the fin entered a debounced state. Using `--force` did not always bypass it cleanly. Manual cleanup (deleting `latest-run` and the old run directory) was required to continue testing.

While correct for production use, this added friction during iterative backend debugging.

**Suggestion:** Consider a `--no-debounce` or development-oriented flag for testing scenarios.

## What Worked Well

- The `--home` global flag is excellent for isolation. It made testing a new backend in a completely separate environment straightforward and safe.
- `orqa pod create --charter ...` and `orqa fin create` felt natural and worked reliably once the home permission issue was bypassed.
- `orqa task send` (with full addresses) worked cleanly.
- Observability commands were very effective:
  - `orqa ... status`
  - `orqa ... plan`
  - `orqa fin tail --follow`
  - `orqa ... loop --force`
- Once the invocation was correct, the wake loop, task queuing, and backend dispatch all worked as designed. The architecture held up well under real usage.

## Other Notes

- The installed `orqa` binary (from `~/.cargo/bin`) did not yet include the Grok backend example in the generated `pod.toml` (expected during active development).
- Manually editing `pod.toml` to enable and configure the Grok backend was straightforward once the template structure was understood.
- Using `HOME` as the unifying environment variable for backend isolation (instead of only tool-specific vars) worked as intended when properly set by the runtime.

---

**Overall Assessment:**  
The core CLI experience is solid and developer-friendly once environmental hurdles (macOS provenance + LaunchAgent) are cleared. The main friction points were macOS-specific security behaviors and discoverability of `loop` invocation patterns. The tool successfully demonstrated its ability to orchestrate a completely new backend (Grok) with minimal configuration.

This session was valuable for identifying real-world usage gaps in a brand new project.