use std::{env, ffi::OsString, path::Path, path::PathBuf, time::Duration};

use crate::{
    model::{Orqa, resolve_pod_context},
    runtime::wake_pod,
};

#[derive(Debug, Clone)]
pub(crate) struct LoopWorkerConfig {
    pub(crate) interval: u64,
    pub(crate) force: bool,
    pub(crate) pid_path: PathBuf,
    pub(crate) pod: String,
    pub(crate) prompt_args: Vec<OsString>,
}

pub(crate) fn run(orqa: &Orqa) -> Result<(), String> {
    let config = load_config(orqa)?;
    let my_pid = std::process::id();
    claim_pidfile(&config.pid_path, my_pid)?;

    loop {
        if let Err(error) = wake_pod(
            orqa,
            &config.pod,
            config.force,
            false,
            false,
            &config.prompt_args,
        ) {
            eprintln!("loop worker error: {error}");
        }

        if !pidfile_matches(&config.pid_path, my_pid) {
            if !config.pid_path.exists() {
                eprintln!("pidfile removed; shutting down loop worker.");
            } else {
                eprintln!("another process took over the pidfile; shutting down.");
            }
            break;
        }

        std::thread::sleep(Duration::from_secs(config.interval));
    }

    Ok(())
}

fn claim_pidfile(pid_path: &Path, pid: u32) -> Result<(), String> {
    let parent = pid_path
        .parent()
        .ok_or_else(|| format!("loop pid path has no parent: {}", pid_path.display()))?;
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create loop pid directory {}: {error}",
            parent.display()
        )
    })?;
    std::fs::write(pid_path, pid.to_string()).map_err(|error| {
        format!(
            "failed to write loop pidfile {}: {error}",
            pid_path.display()
        )
    })
}

pub(crate) fn load_config(orqa: &Orqa) -> Result<LoopWorkerConfig, String> {
    let interval = env::var("ORQA_INTERVAL")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(60);
    let force = env::var("ORQA_FORCE")
        .map(|value| value == "1")
        .unwrap_or(false);
    let pid_path = env::var_os("ORQA_LOOP_WORKER_PID_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| orqa.home.join("tui-loop.pid"));
    let pod = match env::var("ORQA_LOOP_WORKER_POD") {
        Ok(pod) => pod,
        Err(_) => resolve_pod_context(None, orqa)
            .map(|(pod, _)| pod)
            .map_err(|error| format!("loop worker error: {error}"))?,
    };
    let prompt_args = match env::var("ORQA_LOOP_ARGS").ok() {
        Some(raw) => match parse_prompt_args(Some(raw.as_str())) {
            Ok(args) => args,
            Err(()) => {
                eprintln!("warning: failed to parse ORQA_LOOP_ARGS; using empty prompt");
                Vec::new()
            }
        },
        None => Vec::new(),
    };

    Ok(LoopWorkerConfig {
        interval,
        force,
        pid_path,
        pod,
        prompt_args,
    })
}

pub(crate) fn parse_prompt_args(raw: Option<&str>) -> Result<Vec<OsString>, ()> {
    let json = raw.ok_or(())?;
    serde_json::from_str::<Vec<String>>(json)
        .map(|values| values.into_iter().map(OsString::from).collect())
        .map_err(|_| ())
}

pub(crate) fn pidfile_matches(pid_path: &Path, pid: u32) -> bool {
    if !pid_path.exists() {
        return false;
    }

    match std::fs::read_to_string(pid_path) {
        Ok(contents) => contents.trim().parse::<u32>().unwrap_or(0) == pid,
        Err(_) => false,
    }
}

#[cfg(test)]
#[path = "loop_worker_test.rs"]
mod tests;
