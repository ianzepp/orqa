//! Pod-local loop and pause controls for the TUI.

use std::{
    fs::{self, OpenOptions},
    path::PathBuf,
    process::{Child, Command as ProcessCommand, Stdio},
};

use crate::{
    commands::is_process_running,
    mailbox::{remove_sleep_marker, write_sleep_marker},
    model::{Orqa, PodRegistration},
};

pub(crate) const TUI_LOOP_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);

pub(crate) struct PodLoopWorker {
    child: Option<Child>,
    pid_path: PathBuf,
}

impl Drop for PodLoopWorker {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = fs::remove_file(&self.pid_path);
    }
}

pub(crate) fn start_tui_loop_worker(
    orqa: &Orqa,
    reg: &PodRegistration,
) -> Result<PodLoopWorker, String> {
    let pid_path = pod_loop_pid_path(orqa, reg);
    if pid_path.exists() {
        if is_process_running(&pid_path) {
            return Err(format!(
                "pod loop already running for {} (pidfile {})",
                reg.slug,
                pid_path.display()
            ));
        }
        let _ = fs::remove_file(&pid_path);
    }

    let exe = std::env::current_exe()
        .map_err(|error| format!("failed to get current executable: {error}"))?;
    let log_path = orqa.pod_data_home(reg).join("tui-loop.log");
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|error| {
            format!(
                "failed to open TUI loop log {}: {error}",
                log_path.display()
            )
        })?;
    let err_file = log_file
        .try_clone()
        .map_err(|error| format!("failed to clone TUI loop log handle: {error}"))?;
    let child = ProcessCommand::new(exe)
        .env("ORQA_DAEMON", "1")
        .env("ORQA_DAEMON_POD", &reg.slug)
        .env("ORQA_DAEMON_PID_PATH", &pid_path)
        .env("ORQA_INTERVAL", TUI_LOOP_INTERVAL.as_secs().to_string())
        .env("ORQA_FORCE", "0")
        .arg("--home")
        .arg(&orqa.home)
        .current_dir(&reg.path)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(err_file))
        .spawn()
        .map_err(|error| format!("failed to start TUI loop worker: {error}"))?;

    let parent = pid_path
        .parent()
        .ok_or_else(|| format!("loop pid path has no parent: {}", pid_path.display()))?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create loop pid directory {}: {error}",
            parent.display()
        )
    })?;
    fs::write(&pid_path, child.id().to_string()).map_err(|error| {
        format!(
            "failed to write loop pidfile {}: {error}",
            pid_path.display()
        )
    })?;

    Ok(PodLoopWorker {
        child: Some(child),
        pid_path,
    })
}

pub(crate) fn pod_paused(orqa: &Orqa, reg: &PodRegistration) -> bool {
    orqa.pod_sleep_data_path(reg).exists()
}

pub(crate) fn toggle_pod_pause(orqa: &Orqa, reg: &PodRegistration) -> Result<bool, String> {
    let path = orqa.pod_sleep_data_path(reg);
    if path.exists() {
        remove_sleep_marker(&path)?;
        Ok(false)
    } else {
        write_sleep_marker(&path)?;
        Ok(true)
    }
}

fn pod_loop_pid_path(orqa: &Orqa, reg: &PodRegistration) -> std::path::PathBuf {
    orqa.pod_data_home(reg).join("tui-loop.pid")
}
