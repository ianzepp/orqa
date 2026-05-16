mod cli;
mod commands;
mod config;
mod doctor;
mod hooks;
mod mailbox;
mod model;
mod report;
mod runs;
mod runtime;
mod runtime_home;
// mod service;  // Service CLI tree removed. Background service logic to be rethought.
mod status;
mod tui;

use std::{
    env,
    ffi::{OsStr, OsString},
    process::ExitCode,
};

use clap::{CommandFactory, FromArgMatches};

#[allow(unused_imports)]
use cli::{Cli, Command, InitArgs};
use commands::{fin, loop_command, mail, ops, overview, pod, pod_init, service, task};
use model::Orqa;
use runtime::loop_pod;

#[cfg(test)]
#[path = "main_test.rs"]
mod tests;

fn main() -> ExitCode {
    if operational_help_requested() {
        print_operational_help();
        return ExitCode::SUCCESS;
    }

    let matches = Cli::command().disable_help_subcommand(true).get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|error| error.exit());
    let orqa = Orqa::new(cli.home);

    // Daemon mode (launched by `orqa loop start`)
    if env::var("ORQA_DAEMON").is_ok() {
        let interval: u64 = env::var("ORQA_INTERVAL")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(60);
        let force = env::var("ORQA_FORCE").map(|v| v == "1").unwrap_or(false);

        let pid_path = env::var_os("ORQA_DAEMON_PID_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| orqa.home.join("loop.pid"));
        let my_pid = std::process::id();
        let daemon_pod = env::var("ORQA_DAEMON_POD").ok();

        loop {
            // Reconstruct prompt args from the env var set by `orqa loop start -- "..."`.
            // On any deserialization problem, fall back to empty args and log a warning
            // so the daemon continues running (best-effort).
            let prompt_args: Vec<OsString> = env::var("ORQA_LOOP_ARGS")
                .ok()
                .and_then(|json| {
                    serde_json::from_str::<Vec<String>>(&json)
                        .map(|v| v.into_iter().map(OsString::from).collect())
                        .ok()
                })
                .unwrap_or_else(|| {
                    if env::var("ORQA_LOOP_ARGS").is_ok() {
                        eprintln!("warning: failed to parse ORQA_LOOP_ARGS; using empty prompt");
                    }
                    vec![]
                });

            let run_args = cli::LoopRunArgs {
                pod: daemon_pod.clone(),
                force,
                dry_run: false,
                json: false,
                args: prompt_args,
            };
            if let Err(e) = loop_pod(&orqa, run_args) {
                eprintln!("daemon loop error: {}", e);
            }

            // Liveness check: if the pidfile no longer points to us, shut down gracefully
            if !pid_path.exists() {
                eprintln!("Pidfile removed — shutting down loop daemon.");
                break;
            }

            if let Ok(content) = std::fs::read_to_string(&pid_path) {
                if content.trim().parse::<u32>().unwrap_or(0) != my_pid {
                    eprintln!("Another process took over the pidfile — shutting down.");
                    break;
                }
            }

            std::thread::sleep(std::time::Duration::from_secs(interval));
        }

        return ExitCode::SUCCESS;
    }

    let Some(command) = cli.command else {
        // Phase 1 TUI integration: if we are inside a detectable Phase 05 pod root,
        // launch the Operator Cockpit TUI instead of the legacy text overview.
        match crate::model::resolve_pod_context(None, &orqa) {
            Ok((pod_slug, pod_root)) => {
                if let Err(error) = crate::tui::run_tui(&pod_slug, &pod_root) {
                    eprintln!("orqa tui: {error}");
                    return ExitCode::FAILURE;
                }
                return ExitCode::SUCCESS;
            }
            Err(_) => {
                // No pod detected — fall back to the classic text overview
                // (still useful for global status, legacy pods, and when the user
                // is not inside any project).
                if let Err(error) = overview(&orqa) {
                    eprintln!("orqa: {error}");
                    return ExitCode::FAILURE;
                }
                return ExitCode::SUCCESS;
            }
        }
    };

    match run(&orqa, command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("orqa: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(orqa: &Orqa, command: Command) -> Result<(), String> {
    match command {
        Command::Doctor => doctor(orqa),
        Command::Guide => {
            print_operational_help();
            Ok(())
        }
        Command::Init(args) => pod_init(orqa, args),
        Command::Pod(command) => pod(orqa, command),
        Command::Fin(command) => fin(orqa, command),
        Command::Mail(command) => mail(orqa, command),
        Command::Task(command) => task(orqa, command),
        Command::Ops(command) => ops(orqa, command),
        Command::Loop(command) => loop_command(orqa, command),
        Command::Plan(args) => runtime::plan(orqa, args),
        Command::Service(command) => service(orqa, command),
    }
}

fn doctor(orqa: &Orqa) -> Result<(), String> {
    println!("orqa is installed and ready.");
    println!("orqa_home={}", orqa.home.display());
    Ok(())
}

fn print_operational_help() {
    print!("{}", include_str!("help.md"));
}

fn operational_help_requested() -> bool {
    let mut args = env::args_os().skip(1);

    while let Some(arg) = args.next() {
        if arg.as_os_str() == OsStr::new("--home") {
            let _ = args.next();
            continue;
        }

        if arg.to_string_lossy().starts_with("--home=") {
            continue;
        }

        return arg.as_os_str() == OsStr::new("help");
    }

    false
}
