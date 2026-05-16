//! PodWatcher — produces a stream of `Event`s by polling a project-root pod.
//!
//! This is the core data source for the Operator Cockpit timeline.
//! It uses `PodRegistration` and pod-root `.orqa` paths.

#![allow(dead_code)] // Some watcher helpers are exercised only by the live TUI.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::time::SystemTime;

use serde::Deserialize;

use crate::model::{Orqa, PodRegistration};

use super::events::{Event, LogStream};

/// Tracks per-fin state so we only emit "new" data.
#[derive(Default)]
struct FinState {
    /// Current value of the `latest-run` pointer for this fin.
    current_run: Option<String>,

    /// Byte offsets we have already consumed in the three log files of the
    /// current run. Keyed by "stdout", "stderr", "event".
    log_offsets: HashMap<String, usize>,

    /// Basenames of mail files we have already seen in `mail/new/`.
    seen_mails: HashSet<String>,

    /// Whether we currently believe the fin holds the run.lock.
    has_lock: bool,

    /// Current run id after we have emitted its terminal finish event.
    finished_run: Option<String>,
}

/// Watches a single project-root pod and produces timeline events.
pub struct PodWatcher {
    orqa: Orqa, // we keep a copy so the watcher is self-contained
    reg: PodRegistration,
    fins: Vec<String>,
    states: HashMap<String, FinState>,
    /// Last time we did a full fin discovery refresh.
    last_fin_refresh: Option<SystemTime>,
}

impl PodWatcher {
    /// Create a watcher for the given pod.
    ///
    /// The caller must have already verified that this is a valid pod root
    /// (i.e. `pod_root.join(".orqa/pod.toml")` exists).
    pub fn new(orqa: Orqa, reg: PodRegistration) -> Result<Self, String> {
        let mut watcher = Self {
            orqa,
            reg,
            fins: Vec::new(),
            states: HashMap::new(),
            last_fin_refresh: None,
        };
        watcher.refresh_fins()?;
        Ok(watcher)
    }

    /// Re-scan the `fins/` directory and add any newly created fins.
    /// Existing fin state is preserved.
    pub fn refresh_fins(&mut self) -> Result<(), String> {
        let fins_dir = self.reg.path.join(".orqa").join("fins");
        if !fins_dir.exists() {
            self.fins.clear();
            return Ok(());
        }

        let mut discovered = Vec::new();
        for entry in fs::read_dir(&fins_dir)
            .map_err(|e| format!("failed to read fins dir {}: {e}", fins_dir.display()))?
        {
            let entry = entry.map_err(|e| format!("read_dir entry error: {e}"))?;
            let ft = entry
                .file_type()
                .map_err(|e| format!("file_type error: {e}"))?;
            if ft.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    // Only accept valid slugs
                    if crate::model::validate_slug(name).is_ok() {
                        discovered.push(name.to_string());
                    }
                }
            }
        }

        // Add state for any new fins
        for fin in &discovered {
            self.states.entry(fin.clone()).or_default();
        }

        // Keep a sorted list for deterministic polling order
        discovered.sort();
        self.fins = discovered;
        self.last_fin_refresh = Some(SystemTime::now());
        Ok(())
    }

    /// Poll for new events since the last call.
    ///
    /// Returns events in roughly the order they were discovered. The caller
    /// (the TUI) can render them in the order received or re-sort if desired.
    pub fn poll(&mut self) -> Result<Vec<Event>, String> {
        let mut events = Vec::new();

        // Occasionally refresh the fin list (cheap)
        if self.should_refresh_fins() {
            let _ = self.refresh_fins();
        }

        for fin in &self.fins.clone() {
            // We clone the vec above so we can mutate self.states inside the loop.
            if let Some(evts) = self.poll_fin(fin)? {
                events.extend(evts);
            }
        }

        Ok(events)
    }

    fn should_refresh_fins(&self) -> bool {
        match self.last_fin_refresh {
            None => true,
            Some(ts) => ts.elapsed().map(|d| d.as_secs() > 30).unwrap_or(false),
        }
    }

    /// Poll a single fin and return any new events.
    fn poll_fin(&mut self, fin: &str) -> Result<Option<Vec<Event>>, String> {
        let state = self
            .states
            .get_mut(fin)
            .ok_or_else(|| format!("unknown fin {fin}"))?;
        let mut out = Vec::new();

        let fin_data = self.reg.path.join(".orqa").join("fins").join(fin);

        // 1. Check run.lock existence (LockAcquired / LockReleased)
        let lock_path = fin_data.join("run.lock");
        let lock_exists = lock_path.exists();
        if lock_exists && !state.has_lock {
            state.has_lock = true;
            out.push(Event::LockAcquired {
                fin: fin.to_string(),
            });
        } else if !lock_exists && state.has_lock {
            state.has_lock = false;
            out.push(Event::LockReleased {
                fin: fin.to_string(),
            });
        }

        // 2. Check latest-run pointer
        let latest_run_path = fin_data.join("latest-run");
        let new_run_id = fs::read_to_string(&latest_run_path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        if let Some(ref run_id) = new_run_id {
            if state.current_run.as_ref() != Some(run_id) {
                // Run changed
                let old_run = state.current_run.take();
                state.current_run = Some(run_id.clone());
                state.finished_run = None;
                state.log_offsets.clear(); // reset offsets for new run

                // If we had a previous run we can emit a synthetic finish
                // (best effort — the real exit code is in status.json)
                if let Some(old) = old_run {
                    out.push(Event::RunFinished {
                        fin: fin.to_string(),
                        run_id: old,
                        exit_code: None,
                    });
                }

                out.push(Event::RunStarted {
                    fin: fin.to_string(),
                    run_id: run_id.clone(),
                });
            }
        }

        // 3. Tail the three log files of the current run (if any)
        if let Some(run_id) = state.current_run.clone() {
            let run_dir = fin_data.join("runs").join(&run_id);

            for (stream, filename) in [
                (LogStream::Stdout, "stdout.log"),
                (LogStream::Stderr, "stderr.log"),
                (LogStream::Event, "events.jsonl"),
            ] {
                let path = run_dir.join(filename);
                if let Ok(new_events) = Self::tail_log_file(fin, stream, &path, state) {
                    out.extend(new_events);
                }
            }

            if state.finished_run.as_ref() != Some(&run_id) {
                if let Some(exit_code) = finished_run_exit_code(&run_dir.join("status.json")) {
                    state.finished_run = Some(run_id.clone());
                    out.push(Event::RunFinished {
                        fin: fin.to_string(),
                        run_id: run_id.clone(),
                        exit_code,
                    });
                }
            }
        }

        // 4. New mail in mail/new/
        let mail_new = fin_data.join("mail").join("new");
        if mail_new.exists() {
            if let Ok(entries) = fs::read_dir(&mail_new) {
                for entry in entries.flatten() {
                    let file_name = entry.file_name();
                    if let Some(name) = file_name.to_str() {
                        if !state.seen_mails.contains(name) {
                            state.seen_mails.insert(name.to_string());

                            // Best-effort header parsing (very lightweight)
                            let (from, subject) = parse_mail_headers(&entry.path());

                            out.push(Event::MailArrived {
                                fin: fin.to_string(),
                                path: entry.path(),
                                from,
                                subject,
                            });
                        }
                    }
                }
            }
        }

        if out.is_empty() {
            Ok(None)
        } else {
            Ok(Some(out))
        }
    }

    /// Read any new lines from a log file since the last recorded offset.
    fn tail_log_file(
        fin: &str,
        stream: LogStream,
        path: &Path,
        state: &mut FinState,
    ) -> Result<Vec<Event>, String> {
        if !path.exists() {
            return Ok(Vec::new());
        }

        let key = stream.to_string();
        let offset = state.log_offsets.get(&key).copied().unwrap_or(0);

        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Ok(Vec::new()), // transient read error, skip this poll
        };

        if content.len() <= offset {
            return Ok(Vec::new());
        }

        let new_part = &content[offset..];
        let new_len = content.len();

        state.log_offsets.insert(key, new_len);

        let mut events = Vec::new();
        for line in new_part.lines() {
            if !line.is_empty() {
                events.push(Event::LogLine {
                    fin: fin.to_string(),
                    stream,
                    line: line.to_string(),
                });
            }
        }

        Ok(events)
    }
}

#[derive(Deserialize)]
struct RunStatus {
    status: String,
    exit_code: Option<i32>,
}

fn finished_run_exit_code(path: &Path) -> Option<Option<i32>> {
    let contents = fs::read_to_string(path).ok()?;
    let status: RunStatus = serde_json::from_str(&contents).ok()?;
    matches!(
        status.status.as_str(),
        "finished" | "failed" | "spawn-failed"
    )
    .then_some(status.exit_code)
}

/// Extremely lightweight mail header parser.
/// Looks for the first few lines containing "From:" and "Subject:".
fn parse_mail_headers(path: &Path) -> (Option<String>, Option<String>) {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return (None, None),
    };

    let mut from = None;
    let mut subject = None;

    for line in content.lines().take(20) {
        let lower = line.to_lowercase();
        if from.is_none() && lower.starts_with("from:") {
            from = Some(line[5..].trim().to_string());
        }
        if subject.is_none() && lower.starts_with("subject:") {
            subject = Some(line[8..].trim().to_string());
        }
        if from.is_some() && subject.is_some() {
            break;
        }
    }

    (from, subject)
}

#[cfg(test)]
#[path = "watcher_test.rs"]
mod tests;
