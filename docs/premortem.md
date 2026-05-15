# Premortem: orqa Project

**Date:** 2026-05-15  
**Subject:** Ongoing development and sustainability of orqa as a local agent coordinator  
**Version at time of analysis:** 0.6.1

---

## The Plan

**The Plan:** Maintain and evolve orqa (post-v0.6 daemon refactor) as the reliable, low-friction, local-only coordinator for pods of collaborating AI coding agents ("fins"). The central bet is that a simple, inspectable filesystem contract (Maildir-style mail + task queues, pid-based run locks, sleep markers, run records, and per-fin runtime homes) plus a user-managed pidfile background daemon (`orqa loop start` / `loop stop` / `loop status`) will deliver autonomous wake-loop behavior without heavy infrastructure. Backend definitions live in `pod.toml`/`fin.toml` with argv templates; pod-level hooks (currently `pre-plan`) provide extensibility. Distribution via `cargo install`, GitHub release binaries (macOS arm/intel + Linux), Homebrew tap, and `install.sh`.

**Intended for:** Primarily the solo maintainer (power user who runs multiple local AI coding agents such as Codex, Grok, Hermes, Pi, and Ollama-backed setups). Secondarily, other developers who want named groups of agents that can send each other mail and tasks, escalate to a human operator pod, and have observable run history.

**Success looks like (6 months from now):** orqa remains stable and low-maintenance; wake loops run reliably across macOS updates; backend CLI changes are handled gracefully; documentation is accurate and the pod/fin mental model is learnable; the hygiene ratchet keeps the codebase small and correct; some external usage and feedback exists; no silent missed wakes or state corruption incidents erode trust; the tool continues to deliver outsized value relative to its tiny dependency surface.

---

## Frame

> It is 6 months from now. This failed. We are looking back to understand what went wrong.

---

## Failure Modes

These are specific, evidence-based ways the project could fail, drawn from the current codebase, recent Git history, the daemon refactor, the known macOS provenance issue from prior sessions, documentation state, and architectural choices.

1. **Daemon reliability and persistence regression.** The current pidfile self-daemon (spawned via `orqa loop start` with `ORQA_DAEMON=1`, managed in `src/main.rs:39` and `src/commands.rs:393`) has no launch-on-login, no automatic respawn on crash, and only basic stale-pidfile cleanup + SIGTERM/SIGKILL stop logic. The previous LaunchAgent-based `orqa service` commands (still present as dead code in `src/service.rs` and referenced in outdated README text) provided boot-time persistence. Users lose the "set and forget" experience; the wake loop stops after reboot/logout/crash; mail and tasks pile up unprocessed.

2. **Backend CLI coupling breakage.** The default `exec_args` and `chat_args` templates (seeded by `pod create`, shown in README:217 and generated from `config.rs`) are tightly coupled to the exact current command-line shapes of Codex, Grok, Hermes, Pi, and the Ollama+Codex example. These upstream CLIs change flags, auth flows, sandbox models, and output frequently. `pod doctor` only performs shallow command resolution and a short probe; there is no adapter layer or versioned contract. A single breaking change causes widespread `fin exec` and loop-launched run failures.

3. **Documentation and conceptual debt accumulation (already visible).** README.md still contains paragraphs at lines ~928-932 describing the removed `orqa service` + `launchctl`/`systemctl` flow. `src/service.rs` remains in the tree (full of LaunchAgent plist generation and lifecycle code) but is commented out in `main.rs:12` with the note "Background service logic to be rethought." The embedded `help.md`, `templates/pod-agents.md`, and `templates/fin-agents.md` can drift from the implementation. The mental model (qualified `fin@pod.orqa` addresses, `operator@` forwarding, short-form addresses inside fins, `debounce` vs `exec_always`, sleep markers, hooks, run records vs `latest-run` pointers) is powerful but has high onboarding cost. New or returning users hit "why didn't my fin wake?" friction.

4. **Shared mutable state integrity under concurrency and macOS security.** The core architecture (central `ORQA_HOME/pods/` tree written by the daemon, CLI commands, pre-plan hooks, *and* the fins themselves while they run their backends) still exists. Even after the pivot away from LaunchAgent, Maildir moves, simple file writes for tasks/runs/logs/`latest-run`/`runs.jsonl`, and pid-alive checks have race windows. The exact combination that previously triggered `com.apple.provenance` extended attribute stamping (background execution chain + shared user-writable state tree + agents mutating the same directories) can recur under stricter future macOS releases, iCloud/backup tools, or even normal concurrent access. There is no `orqa fsck`, repair command, or atomic state layer beyond basic Maildir semantics. One corrupted pod directory can make an entire group of agents unusable.

5. **Solo-maintainer sustainability under the hygiene ratchet.** The strict ratchet in `tests/hygiene.rs` (zero `unwrap`, `expect`, `panic!`, `unreachable!`, `todo!`, `unimplemented!` allowed in `src/`) plus `cargo clippy --all-targets -D warnings` and `cargo fmt --check` is excellent for correctness but makes every change expensive. Release automation (` .github/workflows/release.yml` + Homebrew publish step added in recent chore commit) has manual coordination points. After the loop restructure + daemon work, dead code and doc drift were already left behind. This pattern repeats; the maintainer eventually burns out on a "small" project that still demands high rigor on every surface (CLI, runtime, docs, templates, tests, distribution).

6. **Activation energy too high for anyone except the original author.** A new user must install orqa *plus* at least one supported backend CLI, set up auth symlinks, create pods and fins with meaningful charters and roles, author or adapt `AGENTS.md` files that teach the agent how to use `orqa mail`/`task`/`status` inside its sandbox, start the daemon, and learn the address and policy rules. The "local only, bring your own models and runtimes, manage your own pods" model has no one-click demo or hosted fallback path. Result: near-zero external adoption, no community pressure or contributions, and the project remains personal infrastructure that never justifies its own complexity to outsiders.

7. **Half-delivered observability and extensibility.** Pre-plan hooks are a good start but are shell-only and narrow. There are per-run `events.jsonl`, `stdout.log`/`stderr.log`, `status.json`, and the `ops report` Markdown bundle, but no metrics, no post-run hooks, no daemon watchdog, no structured event stream, and no web UI (the `web/` directory is static marketing pages deployed via Cloudflare Pages). Users who wanted a real autonomous multi-agent team still end up bolting on their own cron jobs, tmux sessions, or custom scripts.

---

## Deep Dives on the Most Important Failures

### 1. Daemon reliability and persistence (highest immediate risk after the provenance pivot)

**Failure Story**  
A user (or the maintainer) runs `orqa loop start --interval 60 "handle your Orqa mail and tasks"`. The pidfile daemon starts, the overview dashboard shows "loop: running", and fins wake correctly on mail and tasks for days or weeks. Then the laptop sleeps, the user logs out, the machine reboots, or the daemon process is killed by the system. Because there is no launch-on-login agent, no respawn logic, and no integration with launchd user agents or systemd --user, the loop simply does not come back. `orqa loop status` (when the user remembers to check) shows "not running". Critical tasks and operator escalations sit unprocessed. The autonomous promise that justified the whole pod/fin/mail/task model is broken.

**Underlying Assumption**  
That a self-spawned background process + pidfile (the mechanism introduced in the "restructure loop command tree + add daemon support" commit) would be an adequate long-term replacement for the LaunchAgent/service layer once `com.apple.provenance` stamping made the old persistent scheduler toxic.

**Early Warning Signs**  
- Frequent "stale pidfile" or "loop: not running" messages in the default bare `orqa` dashboard.
- Users asking in issues or feedback how to make the wake loop survive reboot or logout.
- Increasing use of `orqa loop run --forever` in tmux as a workaround.
- `orqa loop status` showing the daemon is down after normal machine use.

**Prevention or Mitigation**  
- Add a proper user-level persistent agent generator (launchd plist for macOS, systemd --user unit for Linux) that the new daemon can opt into, while keeping the simple pidfile mode for development/debugging.
- Add internal watchdog/respawn logic inside the `ORQA_DAEMON` loop.
- Make `orqa loop start --install` (or equivalent) do the right thing for persistence and surface the current status clearly.
- Improve the default dashboard and `orqa doctor` to loudly warn when the daemon is not persistent.

### 2. Backend coupling and doctor shallowness

**Failure Story**  
Codex (or Grok, or a new Ollama integration) changes a required flag, deprecates `--skip-git-repo-check`, alters the auth file location, or tightens sandbox semantics. Every existing pod's `fin exec` and every wake-loop run starts failing with backend errors that are hard to diagnose from inside the fin. `orqa pod doctor sample-pod --fin planner` reports that the binary exists and the probe succeeded on an old code path. Users must hand-edit `pod.toml` or every `fin.toml` (or wait for a new orqa release that updates the examples).

**Underlying Assumption**  
The small set of backend command templates + the `pod create` seeding logic + the examples in README and `config.rs` will remain stable enough relative to the velocity of the supported agent CLIs that the "configure once in pod.toml, it just works" contract holds for months.

**Early Warning Signs**  
- Rising number of "backend command failed" or timeout entries in `fin runs` and `fin tail` output.
- `pod doctor` passing while real executions fail.
- Frequent small edits to the long example blocks in README.md and the default config templates.
- Issues or feedback reports that mention specific model or sandbox flags that no longer work.

**Prevention or Mitigation**  
- Make backend definitions first-class with explicit capability detection or version constraints.
- Deepen `pod doctor` (and add `orqa backend probe`) so it actually exercises the rendered `exec_args` and `chat_args` with a test prompt and reports the exact argv that will be used.
- Store the rendered command lines in run records so failed runs are easier to debug.
- Treat the default templates as data that can be safely updated or overridden per installation.

### 3. State integrity and the lingering provenance/macOS risk

**Failure Story**  
Even in the pidfile-daemon world, a fin process (running under its backend) creates or modifies files inside its home while the daemon is scanning, a pre-plan hook runs, or the user runs `orqa task list` / `orqa mail done`. macOS (newer Sequoia+ rules, a backup tool, or even normal execution provenance) stamps `com.apple.provenance` attributes on directories or files under `pods/<pod>/fins/<fin>/runs/`, `mail/`, `tasks/`, or the hook state dir. Later operations from the CLI or another fin get "operation not permitted". `latest-run` pointers and `runs.jsonl` get out of sync with actual run directories. One pod becomes partially or fully unusable. There is no repair command.

**Underlying Assumption**  
The "everything is just files under a predictable tree" contract (the heart of the model in `model.rs`, `mailbox/`, `runs.rs`, and `runtime.rs`) plus simple pid locks and Maildir moves will remain robust under concurrent writers (daemon + CLI + N agent backends) and under macOS's evolving security model, even after the LaunchAgent pivot.

**Early Warning Signs**  
- Intermittent "operation not permitted" errors on mail, task, or status operations.
- Status and overview showing stale "running" counts or incorrect unread_mail/open_tasks numbers.
- `runs/` directories or `latest-run` files that no longer match the ledger.
- xattr provenance attributes appearing on new artifacts even when using only the pidfile daemon.

**Prevention or Mitigation**  
- Add defensive helpers for atomic writes and explicit lock escalation.
- Create a `orqa doctor --repair` or `orqa fsck` pass that can safely clean provenance attributes (where allowed) and reconcile ledgers.
- Consider moving the hottest mutable state (locks, latest-run pointers, counters, daemon pid) to a small per-pod SQLite or similar file while preserving the human-readable mail/tasks/runs layout.
- Explicitly document the concurrency model and the remaining macOS threat surface.
- Add a provenance-clearing step in the daemon startup and in `pod doctor` if the problem reappears.

### 4. Maintenance, docs, and dead code (the slow rot that is already underway)

**Failure Story**  
The loop + daemon refactor (334ca45) left `src/service.rs` (hundreds of lines of now-unused launchd/systemd code), stale paragraphs in README.md, and references in the development section. Future refactors do the same because there is no automated docs-sync or dead-code gate beyond the hygiene ratchet (which does not cover Markdown or commented-out modules). The high bar for changes makes "just fix the docs" feel expensive. Over six months the gap between what the code does and what the README, `help.md`, and templates claim grows until even the maintainer is slowed down.

**Underlying Assumption**  
A solo project with an extremely high correctness bar will also maintain perfect documentation and code hygiene without dedicated processes, lower-friction contribution paths, or explicit "docs are code" enforcement.

**Early Warning Signs**  
- "But the docs say..." confusion in feedback or issues.
- `cargo test` hygiene passing while README and help.md are wrong about commands.
- Increasing time spent re-explaining the current `loop` model vs the old `service` model to the maintainer or users.
- More commented-out modules or "rethought" notes accumulating in `main.rs` and elsewhere.

**Prevention or Mitigation**  
- Delete `src/service.rs` (and its test) in the next cleanup pass and remove every reference to the old service commands.
- Add a lightweight docs-drift or command-help sync check (or treat the embedded `help.md` as generated where possible).
- Make the hygiene ratchet also cover obvious stale documentation patterns.
- After any refactor that touches CLI surface, treat "update all docs and remove dead code" as a required checklist item before the PR is considered complete.

---

## Synthesis

**Most Likely Failure**  
Documentation, conceptual, and dead-code debt combined with the high activation energy for new users. The project remains a brilliant, personally useful power tool for its author but never gains enough external usage or feedback to justify the ongoing maintenance cost under the strict hygiene and release rules. The maintainer gradually stops investing because every session requires re-cleaning residue from the last "small" change.

**Most Dangerous Failure**  
Silent or hard-to-diagnose loss of wake reliability (daemon death after reboot, or state corruption/provenance issues under the shared mutable tree). Users lose trust when important agent work or operator escalations are missed without clear, actionable errors. This is especially dangerous because the entire value proposition of orqa rests on the autonomous wake loop actually running when mail or tasks appear.

**Hidden Assumption**  
That the elegant but highly opinionated "predictable filesystem contract + simple pidfile daemon + backend shell-outs + Maildir signals" design will remain both simpler *and* more robust than the alternatives (heavier orchestrators, per-agent tmux/cron setups, or project-local agent state) even as macOS security tightens and the supported agent CLIs continue to evolve rapidly. The architecture is treated as mostly stable once the LaunchAgent provenance problem was worked around.

**Revised Plan (concrete changes that increase resilience)**

- Make persistence a first-class, documented concern again: add a user-level launchd plist / systemd --user generator (or a clear, maintained recipe) so the provenance fix does not become a permanent UX regression. Keep the pidfile daemon for foreground/debug use.
- Harden the backend surface: deepen `pod doctor`, add capability probes, and treat the default `exec_args`/`chat_args` as updatable data rather than static examples.
- Run a deliberate dead-code + doc-drift cleanup pass (remove `service.rs` entirely, fix the stale service text in README around line 928, align `help.md` and the agent templates) and add a lightweight ongoing guard.
- Add minimal high-value observability and repair tooling (daemon health in the dashboard, `doctor --repair` for common state issues, better run-record diagnostics) so problems become visible before they become trust-destroying.
- Explicitly decide and document the ambition level for the next 6–12 months: "excellent personal power tool for the maintainer and a few close collaborators" vs "small open-source project that random developers can successfully adopt." The former can optimize ruthlessly for the author's workflow; the latter requires lowering onboarding friction and improving error messages.

**Pre-Commit Checklist (before the next release or major architectural decision)**

1. Delete or fully excise `src/service.rs` (and `service_test.rs` if present) and remove every mention of the old `orqa service` commands, `launchctl`, and `systemctl` from README.md, `src/help.md`, and any other docs. Run the full hygiene + clippy + test suite afterward.
2. Exercise the complete daemon lifecycle on real macOS hardware: `loop start`, `status`, `stop`, simulated crash, logout/login, and reboot scenarios. Confirm with at least two pods that have active mail + tasks + a pre-plan hook that wakes continue to work and that no new provenance stamps appear on artifacts under `pods/`.
3. Expand `pod doctor` (and add a `backend probe` subcommand) so that it actually executes the rendered command lines from the current `pod.toml`/`fin.toml` with a test prompt and reports the exact argv that will be used at runtime. Verify it would have caught the last two known breaking changes in the supported backends.
4. Add a clear, prominent section (in both the default dashboard output and the top of the operational help) titled "Making the wake loop survive reboot and logout" that states the current recommended approach (even if it is still "use an external launcher or the pidfile daemon in a persistent terminal for now").
5. Decide and record the intended long-term persistence model for the daemon (pidfile-only, launchd user agent, systemd --user, or hybrid) and make that decision visible in `orqa loop status`, the overview dashboard, and the README quick-start flow.

---

## Appendix: Evidence Sources Used in This Premortem

- Source layout and current daemon implementation: `src/main.rs`, `src/cli.rs`, `src/commands.rs` (loop_* functions), `src/runtime.rs` (wake planning), `src/status.rs`, `src/model.rs`.
- Dead code and doc drift: `src/service.rs` (still present), `README.md:926` (transition note) and `928–932` (stale service instructions), `src/help.md:461`, `main.rs:12` comment.
- Recent direction: `git log --oneline -20` (dashboard, loop restructure + daemon, hooks, ops report, Homebrew publish, operator mail routing).
- Hygiene and test surface: `tests/hygiene.rs`, `tests/pod_flow.rs`, `tests/help_command.rs`, `Cargo.toml` (minimal deps, edition "2024", rust-version 1.85).
- Historical context: prior session memory on the `com.apple.provenance` stamping issue caused by the combination of persistent LaunchAgent + central shared mutable `ORQA_HOME` tree + agent writes.
- Distribution and operational: `.github/workflows/release.yml`, `install.sh`, `docs/feedback-01.md`, `web/` (marketing only).
- Executable truth: `cargo check --locked` succeeds cleanly.

This premortem is intentionally direct and unsentimental. The goal is to surface the assumptions that must be tested or changed *before* they become expensive failures in production use or maintainer time.

---

*Generated as part of project warm-up + premortem exercise on 2026-05-15. Saved to `docs/premortem.md` at user request.*
