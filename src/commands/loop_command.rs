use crate::{
    cli::{LoopCommand, LoopStartArgs, LoopSubcommand},
    model::Orqa,
    runtime::{loop_pod, plan},
};

pub(crate) fn loop_command(orqa: &Orqa, command: LoopCommand) -> Result<(), String> {
    match command.command {
        LoopSubcommand::Run(args) => loop_pod(orqa, args),
        LoopSubcommand::Plan(args) => plan(orqa, args),
        LoopSubcommand::Start(args) => loop_start(orqa, args),
        LoopSubcommand::Stop => loop_stop(orqa),
        LoopSubcommand::Status => loop_status(orqa),
    }
}

fn loop_start(orqa: &Orqa, args: LoopStartArgs) -> Result<(), String> {
    let pid_path = orqa.home.join("loop.pid");

    // Prevent multiple startups + clean up stale pidfile
    if pid_path.exists() {
        if is_process_running(&pid_path) {
            return Err(
                "Loop daemon is already running. Use `orqa loop status` to check.".to_string(),
            );
        } else {
            // Stale pidfile — remove it
            let _ = std::fs::remove_file(&pid_path);
        }
    }

    let exe =
        std::env::current_exe().map_err(|e| format!("failed to get current executable: {}", e))?;

    let mut cmd = std::process::Command::new(exe);
    cmd.env("ORQA_DAEMON", "1")
        .env("ORQA_INTERVAL", args.interval.to_string())
        .env("ORQA_FORCE", if args.force { "1" } else { "0" })
        .arg("--home")
        .arg(&orqa.home);

    // Forward user prompt args via env var so the child daemon never sees them
    // as top-level CLI arguments (which would cause clap parse failure before
    // the ORQA_DAEMON branch is reached).
    let loop_args: Vec<String> = args
        .args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let args_json = serde_json::to_string(&loop_args)
        .map_err(|e| format!("failed to serialize loop prompt args: {}", e))?;
    cmd.env("ORQA_LOOP_ARGS", args_json);

    let child = cmd
        .spawn()
        .map_err(|e| format!("failed to start loop daemon: {}", e))?;

    std::fs::write(&pid_path, child.id().to_string())
        .map_err(|e| format!("failed to write pidfile: {}", e))?;

    println!("Loop daemon started (pid {})", child.id());
    Ok(())
}

fn loop_stop(orqa: &Orqa) -> Result<(), String> {
    let pid_path = orqa.home.join("loop.pid");

    if !pid_path.exists() {
        println!("No loop daemon is running.");
        return Ok(());
    }

    let pid_str =
        std::fs::read_to_string(&pid_path).map_err(|e| format!("failed to read pidfile: {}", e))?;
    let pid: u32 = pid_str
        .trim()
        .parse()
        .map_err(|_| "invalid PID in pidfile".to_string())?;

    println!("Stopping loop daemon (pid {})...", pid);

    #[cfg(unix)]
    {
        // Send graceful SIGTERM first
        let _ = std::process::Command::new("kill")
            .arg(pid.to_string())
            .status();

        // Wait up to 10 seconds for graceful shutdown
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(10) {
            if !is_process_running(&pid_path) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }

        // If still running after timeout, force kill
        if is_process_running(&pid_path) {
            println!("Daemon did not exit gracefully — sending SIGKILL...");
            let _ = std::process::Command::new("kill")
                .arg("-9")
                .arg(pid.to_string())
                .status();

            // Give it a moment
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
    }

    #[cfg(not(unix))]
    {
        // Windows fallback
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status();
    }

    let _ = std::fs::remove_file(&pid_path);
    println!("Loop daemon stopped.");
    Ok(())
}

fn loop_status(orqa: &Orqa) -> Result<(), String> {
    let pid_path = orqa.home.join("loop.pid");

    if !pid_path.exists() {
        println!("Loop daemon is not running");
        return Ok(());
    }

    if is_process_running(&pid_path) {
        let pid_str = std::fs::read_to_string(&pid_path).unwrap_or_default();
        println!("Loop daemon is running (pid {})", pid_str.trim());
    } else {
        println!("Loop daemon is not running (stale pidfile)");
        let _ = std::fs::remove_file(&pid_path);
    }

    Ok(())
}

pub(crate) fn is_process_running(pid_path: &std::path::Path) -> bool {
    if !pid_path.exists() {
        return false;
    }

    if let Ok(pid_str) = std::fs::read_to_string(pid_path) {
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            #[cfg(unix)]
            {
                // kill -0 just checks if process exists
                return std::process::Command::new("kill")
                    .arg("-0")
                    .arg(pid.to_string())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
            }

            #[cfg(not(unix))]
            {
                // On Windows, we can use tasklist or assume running if pidfile exists
                return true;
            }
        }
    }
    false
}
