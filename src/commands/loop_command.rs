use crate::{cli::LoopCommand, model::Orqa, model::resolve_pod_context, runtime::wake_pod};

pub(crate) fn loop_command(orqa: &Orqa, command: LoopCommand) -> Result<(), String> {
    if command.interval == 0 {
        return Err("loop interval must be at least 1 second".to_string());
    }

    let (pod, _) = resolve_pod_context(None, orqa)?;
    loop {
        wake_pod(orqa, &pod, command.force, false, false, &command.args)?;
        std::thread::sleep(std::time::Duration::from_secs(command.interval));
    }
}

pub(crate) fn is_process_running(pid_path: &std::path::Path) -> bool {
    if !pid_path.exists() {
        return false;
    }

    if let Ok(pid_str) = std::fs::read_to_string(pid_path) {
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            #[cfg(unix)]
            {
                return std::process::Command::new("kill")
                    .arg("-0")
                    .arg(pid.to_string())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
            }

            #[cfg(not(unix))]
            {
                return true;
            }
        }
    }
    false
}
