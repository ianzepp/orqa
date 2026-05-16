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
pub(crate) const TUI_LOOP_PROMPT: &str = "handle your open Orqa mail and tasks";

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
    let (log_file, err_file) = tui_loop_log_files(reg)?;
    let child = ProcessCommand::new(exe)
        .env("ORQA_LOOP_WORKER", "1")
        .env("ORQA_LOOP_WORKER_POD", &reg.slug)
        .env("ORQA_LOOP_WORKER_PID_PATH", &pid_path)
        .env("ORQA_INTERVAL", TUI_LOOP_INTERVAL.as_secs().to_string())
        .env("ORQA_FORCE", "0")
        .env("ORQA_LOOP_ARGS", tui_loop_prompt_args_json()?)
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

pub(crate) fn trigger_tui_wake(orqa: &Orqa, reg: &PodRegistration) -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|error| format!("failed to get current executable: {error}"))?;
    let (log_file, err_file) = tui_loop_log_files(reg)?;
    let mut command = ProcessCommand::new(exe);
    clear_loop_worker_env(&mut command);
    let mut child = command
        .args(tui_wake_command_args(orqa))
        .current_dir(&reg.path)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(err_file))
        .spawn()
        .map_err(|error| format!("failed to trigger TUI wake: {error}"))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });

    Ok(())
}

pub(crate) fn pod_paused(orqa: &Orqa, reg: &PodRegistration) -> bool {
    let _ = orqa;
    reg.path.join(".orqa").join("sleep.lock").exists()
}

pub(crate) fn toggle_pod_pause(orqa: &Orqa, reg: &PodRegistration) -> Result<bool, String> {
    let _ = orqa;
    let path = reg.path.join(".orqa").join("sleep.lock");
    if path.exists() {
        remove_sleep_marker(&path)?;
        Ok(false)
    } else {
        write_sleep_marker(&path)?;
        Ok(true)
    }
}

fn pod_loop_pid_path(orqa: &Orqa, reg: &PodRegistration) -> std::path::PathBuf {
    let _ = orqa;
    reg.path.join(".orqa").join("tui-loop.pid")
}

pub(crate) fn tui_loop_prompt_args_json() -> Result<String, String> {
    serde_json::to_string(&[TUI_LOOP_PROMPT])
        .map_err(|error| format!("failed to serialize TUI loop prompt args: {error}"))
}

fn tui_loop_log_files(reg: &PodRegistration) -> Result<(fs::File, fs::File), String> {
    let pod_home = reg.path.join(".orqa");
    let log_path = pod_home.join("tui-loop.log");
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
    Ok((log_file, err_file))
}

fn tui_wake_command_args(orqa: &Orqa) -> Vec<std::ffi::OsString> {
    vec![
        "--home".into(),
        orqa.home.as_os_str().into(),
        "wake".into(),
        "--force".into(),
        "--".into(),
        TUI_LOOP_PROMPT.into(),
    ]
}

fn clear_loop_worker_env(command: &mut ProcessCommand) {
    for key in [
        "ORQA_LOOP_WORKER",
        "ORQA_LOOP_WORKER_POD",
        "ORQA_LOOP_WORKER_PID_PATH",
        "ORQA_INTERVAL",
        "ORQA_FORCE",
        "ORQA_LOOP_ARGS",
    ] {
        command.env_remove(key);
    }
}

#[cfg(test)]
#[path = "loopctl_test.rs"]
mod tests;
