use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{ExitStatus, Output},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::commands::list_dirs;
use crate::model::{FinRef, Orqa, PodRef};

static RUN_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct RunRecord {
    pub(crate) id: String,
    pub(crate) fin: String,
    pub(crate) backend: String,
    pub(crate) mode: String,
    pub(crate) command: String,
    pub(crate) args: Vec<String>,
    pub(crate) status: String,
    pub(crate) exit_code: Option<i32>,
    pub(crate) run_dir: PathBuf,
    pub(crate) stdout_log: PathBuf,
    pub(crate) stderr_log: PathBuf,
    pub(crate) events_log: PathBuf,
}

impl RunRecord {
    fn with_status(&self, status: &str, exit_code: Option<i32>) -> Self {
        let mut record = self.clone();
        record.status = status.to_string();
        record.exit_code = exit_code;
        record
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RunList {
    pub(crate) fin: String,
    pub(crate) runs: Vec<RunRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RunLogs {
    pub(crate) fin: String,
    pub(crate) run: String,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) events: String,
}

pub(crate) struct RunFiles {
    pub(crate) record: RunRecord,
    status_path: PathBuf,
    ledger_path: PathBuf,
}

impl RunFiles {
    pub(crate) fn create(
        orqa: &Orqa,
        fin: &FinRef,
        mode: &str,
        backend: &str,
        command: &OsString,
        args: &[OsString],
    ) -> Result<Self, String> {
        let id = run_id()?;
        let run_dir = orqa.run_home(fin, &id);
        fs::create_dir_all(&run_dir).map_err(|error| {
            format!(
                "failed to create run directory {}: {error}",
                run_dir.display()
            )
        })?;

        let record = RunRecord {
            id,
            fin: fin.label(),
            backend: backend.to_string(),
            mode: mode.to_string(),
            command: command.to_string_lossy().to_string(),
            args: args
                .iter()
                .map(|arg| arg.to_string_lossy().to_string())
                .collect(),
            status: "planned".to_string(),
            exit_code: None,
            stdout_log: run_dir.join("stdout.log"),
            stderr_log: run_dir.join("stderr.log"),
            events_log: run_dir.join("events.jsonl"),
            run_dir: run_dir.clone(),
        };

        write_file(&record.stdout_log, "")?;
        write_file(&record.stderr_log, "")?;
        write_file(&record.events_log, "")?;
        let command_text = command_text(command, args);
        write_file(&run_dir.join("command.txt"), &command_text)?;
        write_latest(orqa, fin, &record.id)?;

        let files = Self {
            status_path: run_dir.join("status.json"),
            ledger_path: orqa.runs_ledger_path(fin),
            record,
        };
        files.write_status("planned", None, None)?;
        files.append_event("planned", &[("command", command_text)])?;
        Ok(files)
    }

    pub(crate) fn mark_spawned(&self, pid: u32) -> Result<(), String> {
        self.write_status("running", None, Some(pid))?;
        self.append_event("spawned", &[("pid", pid.to_string())])
    }

    pub(crate) fn mark_finished(&self, output: &Output) -> Result<(), String> {
        write_file(
            &self.record.stdout_log,
            &String::from_utf8_lossy(&output.stdout),
        )?;
        write_file(
            &self.record.stderr_log,
            &String::from_utf8_lossy(&output.stderr),
        )?;
        self.mark_finished_status(output.status)
    }

    pub(crate) fn mark_finished_status(&self, status: ExitStatus) -> Result<(), String> {
        let exit_code = status.code();
        self.finish_with_exit(exit_code)
    }

    pub(crate) fn mark_spawn_failed(&self, message: &str) -> Result<(), String> {
        self.write_status("spawn-failed", None, None)?;
        self.append_event("spawn-failed", &[("error", message.to_string())])?;
        self.append_ledger(&self.record.with_status("spawn-failed", None))
    }

    pub(crate) fn stdout_file(&self) -> Result<fs::File, String> {
        append_file(&self.record.stdout_log)
    }

    pub(crate) fn stderr_file(&self) -> Result<fs::File, String> {
        append_file(&self.record.stderr_log)
    }

    fn write_status(
        &self,
        status: &str,
        exit_code: Option<i32>,
        pid: Option<u32>,
    ) -> Result<(), String> {
        let record = self.record.with_status(status, exit_code);
        let mut value = serde_json::to_value(&record)
            .map_err(|error| format!("failed to encode run status: {error}"))?;
        if let Some(pid) = pid {
            value["pid"] = serde_json::json!(pid);
        }
        write_file(
            &self.status_path,
            &serde_json::to_string_pretty(&value)
                .map_err(|error| format!("failed to encode run status: {error}"))?,
        )
    }

    fn finish_with_exit(&self, exit_code: Option<i32>) -> Result<(), String> {
        self.write_status("finished", exit_code, None)?;
        self.append_event(
            "finished",
            &[(
                "exit_code",
                exit_code.map_or_else(|| "signal".to_string(), |code| code.to_string()),
            )],
        )?;
        self.append_ledger(&self.record.with_status("finished", exit_code))
    }

    fn append_event(&self, event: &str, fields: &[(&str, String)]) -> Result<(), String> {
        let mut value = serde_json::json!({
            "epoch": now_epoch(),
            "event": event,
            "run": self.record.id,
            "fin": self.record.fin,
        });
        for (key, field_value) in fields {
            value[*key] = serde_json::json!(field_value);
        }
        append_line(
            &self.record.events_log,
            &format!(
                "{}\n",
                serde_json::to_string(&value)
                    .map_err(|error| format!("failed to encode run event: {error}"))?
            ),
        )
    }

    fn append_ledger(&self, record: &RunRecord) -> Result<(), String> {
        append_line(
            &self.ledger_path,
            &format!(
                "{}\n",
                serde_json::to_string(record)
                    .map_err(|error| format!("failed to encode run ledger: {error}"))?
            ),
        )
    }
}

pub(crate) fn list_runs(orqa: &Orqa, fin: &FinRef) -> Result<RunList, String> {
    let mut runs = Vec::new();
    if let Ok(entries) = fs::read_dir(orqa.runs_home(fin)) {
        for entry in entries.flatten() {
            let status = entry.path().join("status.json");
            if status.is_file() {
                match read_run_record(&status) {
                    Ok(record) => runs.push(record),
                    Err(err) => {
                        eprintln!("warning: {err}");
                        // Skip corrupt run record; do not abort the whole listing
                    }
                }
            }
        }
    }
    runs.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(RunList {
        fin: fin.label(),
        runs,
    })
}

pub(crate) fn read_run_record_for(
    orqa: &Orqa,
    fin: &FinRef,
    run: Option<&str>,
) -> Result<RunRecord, String> {
    let run_id = resolve_run_id(orqa, fin, run)?;
    read_run_record(&orqa.run_home(fin, &run_id).join("status.json"))
}

pub(crate) fn latest_run_started_at(
    orqa: &Orqa,
    fin: &FinRef,
) -> Result<Option<SystemTime>, String> {
    let run = match fs::read_to_string(orqa.latest_run_path(fin)) {
        Ok(run) => run.trim().to_string(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            eprintln!(
                "warning: failed to read latest-run pointer for {}: {error} (treating as no last run)",
                fin.label()
            );
            return Ok(None);
        }
    };

    let first = run.split('.').next().unwrap_or("");
    match first.parse::<u64>() {
        Ok(micros) => Ok(Some(UNIX_EPOCH + Duration::from_micros(micros))),
        Err(_) => {
            eprintln!(
                "warning: latest-run pointer for {} is corrupt or unreadable ({}). Treating as no valid last run. Consider removing the pointer file.",
                fin.label(),
                orqa.latest_run_path(fin).display()
            );
            Ok(None)
        }
    }
}

pub(crate) fn read_run_logs(
    orqa: &Orqa,
    fin: &FinRef,
    run: Option<&str>,
) -> Result<RunLogs, String> {
    let run = resolve_run_id(orqa, fin, run)?;
    let run_dir = orqa.run_home(fin, &run);
    Ok(RunLogs {
        fin: fin.label(),
        run,
        stdout: read_optional(&run_dir.join("stdout.log"))?,
        stderr: read_optional(&run_dir.join("stderr.log"))?,
        events: read_optional(&run_dir.join("events.jsonl"))?,
    })
}

pub(crate) fn tail_fin(
    orqa: &Orqa,
    fin: &FinRef,
    run: Option<&str>,
    lines: usize,
    follow: bool,
) -> Result<(), String> {
    let run = resolve_run_id(orqa, fin, run)?;
    let run_dir = orqa.run_home(fin, &run);
    tail_paths(
        &[
            ("stdout", run_dir.join("stdout.log")),
            ("stderr", run_dir.join("stderr.log")),
            ("event", run_dir.join("events.jsonl")),
        ],
        &fin.label(),
        lines,
        follow,
    )
}

pub(crate) fn tail_pod(
    orqa: &Orqa,
    pod: &str,
    fin_filter: Option<&str>,
    lines: usize,
    follow: bool,
) -> Result<(), String> {
    let pod_ref = PodRef::new(pod)?;
    orqa.ensure_pod_exists(&pod_ref)?;
    let fins_dir = orqa.pod_home(&pod_ref).join("fins");
    let fin_slugs = list_dirs(&fins_dir)?;
    let mut paths = Vec::new();
    for fin_slug in fin_slugs {
        if fin_filter.is_some_and(|filter| filter != fin_slug) {
            continue;
        }
        let fin = FinRef::new(&pod_ref.slug, &fin_slug)?;
        let Ok(run) = resolve_run_id(orqa, &fin, None) else {
            continue;
        };
        let run_dir = orqa.run_home(&fin, &run);
        paths.push((format!("{fin_slug} stdout"), run_dir.join("stdout.log")));
        paths.push((format!("{fin_slug} stderr"), run_dir.join("stderr.log")));
        paths.push((format!("{fin_slug} event"), run_dir.join("events.jsonl")));
    }
    let borrowed = paths
        .iter()
        .map(|(label, path)| (label.as_str(), path.clone()))
        .collect::<Vec<_>>();
    tail_paths(&borrowed, pod, lines, follow)
}

fn read_run_record(path: &Path) -> Result<RunRecord, String> {
    let contents = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(error) => {
            return Err(format!(
                "failed to read run status {}: {error} (file may be corrupt; consider removing the run directory)",
                path.display()
            ));
        }
    };
    match serde_json::from_str(&contents) {
        Ok(record) => Ok(record),
        Err(error) => Err(format!(
            "failed to parse run status {}: {error} (file is corrupt; consider removing the run directory)",
            path.display()
        )),
    }
}

fn resolve_run_id(orqa: &Orqa, fin: &FinRef, run: Option<&str>) -> Result<String, String> {
    match run {
        Some("latest") | None => match fs::read_to_string(orqa.latest_run_path(fin)) {
            Ok(value) => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    Err(format!(
                        "fin {} has no valid latest run (pointer is empty). Run the fin at least once.",
                        fin.label()
                    ))
                } else {
                    Ok(trimmed.to_string())
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(format!(
                "fin {} has no latest run yet. Run it at least once before using 'latest'.",
                fin.label()
            )),
            Err(error) => {
                eprintln!(
                    "warning: latest-run pointer for {} is unreadable or corrupt ({error}). Treating as no valid last run.",
                    fin.label()
                );
                Err(format!(
                    "fin {} latest-run pointer is corrupt. Inspect or remove {} and re-run the fin.",
                    fin.label(),
                    orqa.latest_run_path(fin).display()
                ))
            }
        },
        Some(run) => Ok(run.to_string()),
    }
}

fn write_latest(orqa: &Orqa, fin: &FinRef, run: &str) -> Result<(), String> {
    write_file(&orqa.latest_run_path(fin), &format!("{run}\n"))
}

fn run_id() -> Result<String, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?;
    let counter = RUN_COUNTER.fetch_add(1, Ordering::Relaxed);
    Ok(format!(
        "{}.{}.{}",
        now.as_micros(),
        std::process::id(),
        counter
    ))
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn command_text(command: &OsString, args: &[OsString]) -> String {
    let mut parts = vec![command.to_string_lossy().to_string()];
    parts.extend(args.iter().map(|arg| arg.to_string_lossy().to_string()));
    parts.join(" ")
}

fn append_file(path: &Path) -> Result<fs::File, String> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("failed to open {}: {error}", path.display()))
}

fn write_file(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create directory {}: {error}", parent.display()))?;
    }
    fs::write(path, contents)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn append_line(path: &Path, line: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create directory {}: {error}", parent.display()))?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| file.write_all(line.as_bytes()))
        .map_err(|error| format!("failed to append {}: {error}", path.display()))
}

fn read_optional(path: &Path) -> Result<String, String> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(contents),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(format!("failed to read {}: {error}", path.display())),
    }
}

fn tail_paths(
    paths: &[(&str, PathBuf)],
    target: &str,
    lines: usize,
    follow: bool,
) -> Result<(), String> {
    let mut offsets = Vec::new();
    for (label, path) in paths {
        let contents = read_optional(path)?;
        let selected = last_lines(&contents, lines);
        print_tagged(target, label, &selected);
        offsets.push(contents.len());
    }

    if follow {
        loop {
            thread::sleep(Duration::from_millis(500));
            for ((label, path), offset) in paths.iter().zip(offsets.iter_mut()) {
                let contents = read_optional(path)?;
                if contents.len() <= *offset {
                    continue;
                }
                let new = contents[*offset..].to_string();
                *offset = contents.len();
                print_tagged(target, label, &new);
            }
        }
    }

    Ok(())
}

fn last_lines(contents: &str, lines: usize) -> String {
    if lines == 0 {
        return String::new();
    }
    let all = contents.lines().collect::<Vec<_>>();
    let start = all.len().saturating_sub(lines);
    let selected = all[start..].join("\n");
    if selected.is_empty() {
        selected
    } else {
        format!("{selected}\n")
    }
}

fn print_tagged(target: &str, label: &str, contents: &str) {
    for line in contents.lines() {
        println!("[{target} {label}] {line}");
    }
}
