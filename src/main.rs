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
mod status;
mod tui;

use std::{env, ffi::OsString, process::ExitCode};

use clap::{Arg, ArgAction, Command as ClapCommand, CommandFactory, FromArgMatches};

#[allow(unused_imports)]
use cli::{Cli, Command, InitArgs};
use commands::{fin, loop_command, mail, ops, overview, pod, pod_init, task};
use model::Orqa;
use runtime::wake_pod;

#[cfg(test)]
#[path = "main_test.rs"]
mod tests;

const TOP_LEVEL_HELP_TEMPLATE: &str =
    "{about}\n\nUsage: {usage}\n\nOptions:\n{options}\n\nCommands:\n{subcommands}";

fn main() -> ExitCode {
    let matches = cli_command().get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|error| error.exit());
    let orqa = Orqa::new(cli.home);

    // Internal loop worker mode used by the TUI.
    if env::var("ORQA_LOOP_WORKER").is_ok() {
        let interval: u64 = env::var("ORQA_INTERVAL")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(60);
        let force = env::var("ORQA_FORCE").map(|v| v == "1").unwrap_or(false);

        let pid_path = env::var_os("ORQA_LOOP_WORKER_PID_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| orqa.home.join("tui-loop.pid"));
        let my_pid = std::process::id();
        let worker_pod = match env::var("ORQA_LOOP_WORKER_POD") {
            Ok(pod) => pod,
            Err(_) => match crate::model::resolve_pod_context(None, &orqa) {
                Ok((pod, _)) => pod,
                Err(error) => {
                    eprintln!("loop worker error: {error}");
                    return ExitCode::FAILURE;
                }
            },
        };

        loop {
            // Reconstruct prompt args for the worker. On any deserialization
            // problem, fall back to empty args and keep the worker alive.
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

            if let Err(e) = wake_pod(&orqa, &worker_pod, force, false, false, &prompt_args) {
                eprintln!("loop worker error: {}", e);
            }

            // Liveness check: if the pidfile no longer points to us, shut down gracefully
            if !pid_path.exists() {
                eprintln!("pidfile removed; shutting down loop worker.");
                break;
            }

            if let Ok(content) = std::fs::read_to_string(&pid_path) {
                if content.trim().parse::<u32>().unwrap_or(0) != my_pid {
                    eprintln!("another process took over the pidfile; shutting down.");
                    break;
                }
            }

            std::thread::sleep(std::time::Duration::from_secs(interval));
        }

        return ExitCode::SUCCESS;
    }

    let Some(command) = cli.command else {
        // If we are inside a detectable pod root, launch the Operator Cockpit TUI.
        match crate::model::resolve_pod_context(None, &orqa) {
            Ok((pod_slug, pod_root)) => {
                if let Err(error) = crate::tui::run_tui(&pod_slug, &pod_root) {
                    eprintln!("orqa tui: {error}");
                    return ExitCode::FAILURE;
                }
                return ExitCode::SUCCESS;
            }
            Err(_) => {
                // No pod detected; show global registered pod status.
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
        Command::Wake(args) => runtime::wake_current_pod(orqa, args),
        Command::Loop(command) => loop_command(orqa, command),
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

fn cli_command() -> ClapCommand {
    Cli::command()
        .arg(
            Arg::new("version")
                .short('v')
                .long("version")
                .short_alias('V')
                .action(ArgAction::Version)
                .help("Print version"),
        )
        .help_template(TOP_LEVEL_HELP_TEMPLATE)
}
