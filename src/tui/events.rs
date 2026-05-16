//! Unified event types for the Operator Cockpit timeline.
//!
//! These events are produced by `PodWatcher` and will be consumed by the
//! timeline renderer. All paths and data come from registered pod roots.

#![allow(dead_code)] // Many variants and helpers are for timeline rendering

use std::path::PathBuf;

/// Identifies which log stream a line came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStream {
    Stdout,
    Stderr,
    Event, // events.jsonl
}

impl std::fmt::Display for LogStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogStream::Stdout => write!(f, "stdout"),
            LogStream::Stderr => write!(f, "stderr"),
            LogStream::Event => write!(f, "event"),
        }
    }
}

/// A single item that can appear in the operator timeline.
#[derive(Debug, Clone)]
pub enum Event {
    /// A new line was appended to one of a fin's run log files.
    LogLine {
        fin: String,
        stream: LogStream,
        line: String,
    },

    /// A new mail file appeared in a fin's `mail/new/` directory.
    /// We emit this as soon as the file is visible; richer parsing (subject, from)
    /// can be done by the UI layer when the user focuses the event.
    MailArrived {
        fin: String,
        /// Full path to the mail file in mail/new/
        path: PathBuf,
        /// Best-effort parsed headers (may be None if file is unreadable or empty)
        from: Option<String>,
        subject: Option<String>,
    },

    /// The fin's `latest-run` pointer moved to a new run id.
    RunStarted { fin: String, run_id: String },

    /// A run finished (we detect this either via the run's status.json or by
    /// the lock being released while we were watching that run).
    RunFinished {
        fin: String,
        run_id: String,
        exit_code: Option<i32>,
    },

    /// The fin acquired its run.lock (another process is now executing it).
    LockAcquired { fin: String },

    /// The fin released its run.lock.
    LockReleased { fin: String },

    /// Synthetic note emitted by the TUI itself (e.g. "operator mailed planner...").
    /// Useful for showing the human's own actions in the same stream.
    OperatorAction { text: String },
}

impl Event {
    /// Short human-readable label for the event (used in debug / early UI).
    #[allow(dead_code)]
    pub fn label(&self) -> &'static str {
        match self {
            Event::LogLine { .. } => "log",
            Event::MailArrived { .. } => "mail",
            Event::RunStarted { .. } => "run-start",
            Event::RunFinished { .. } => "run-finish",
            Event::LockAcquired { .. } => "lock",
            Event::LockReleased { .. } => "unlock",
            Event::OperatorAction { .. } => "operator",
        }
    }

    /// Returns the fin this event is associated with, if any.
    pub fn fin(&self) -> Option<&str> {
        match self {
            Event::LogLine { fin, .. } => Some(fin),
            Event::MailArrived { fin, .. } => Some(fin),
            Event::RunStarted { fin, .. } => Some(fin),
            Event::RunFinished { fin, .. } => Some(fin),
            Event::LockAcquired { fin } => Some(fin),
            Event::LockReleased { fin } => Some(fin),
            Event::OperatorAction { .. } => None,
        }
    }

    /// Whether this event is related to the operator (mail to/from operator or explicit OperatorAction).
    pub fn is_operator_related(&self) -> bool {
        match self {
            Event::MailArrived { from, .. } => {
                from.as_deref().is_some_and(|f| f.contains("operator@"))
            }
            Event::OperatorAction { .. } => true,
            _ => false,
        }
    }

    /// Rough match for thread/subject filtering.
    pub fn matches_thread(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        match self {
            Event::MailArrived { subject, .. } => subject
                .as_deref()
                .is_some_and(|s| s.to_lowercase().contains(&q)),
            Event::LogLine { line, .. } => line.to_lowercase().contains(&q),
            _ => false,
        }
    }
}
