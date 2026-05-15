# Git Watch

Read-only repository activity watcher.

Every scheduled wake runs `git --no-pager log --oneline -5` from the pod root so
the TUI timeline has simple, non-mutating activity to display.
