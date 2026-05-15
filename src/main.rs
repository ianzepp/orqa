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

use std::{env, ffi::OsStr, process::ExitCode};

use clap::{CommandFactory, FromArgMatches};

use cli::{Cli, Command};
use commands::{fin, loop_command, mail, ops, pod, task};
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

        let pid_path = orqa.home.join("loop.pid");
        let my_pid = std::process::id();

        loop {
            let run_args = cli::LoopRunArgs {
                pod: None,
                force,
                dry_run: false,
                json: false,
                args: vec![],
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
    }

    let Some(command) = cli.command else {
        Cli::command().print_help().ok();
        return ExitCode::SUCCESS;
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
        Command::Pod(command) => pod(orqa, command),
        Command::Fin(command) => fin(orqa, command),
        Command::Mail(command) => mail(orqa, command),
        Command::Task(command) => task(orqa, command),
        Command::Ops(command) => ops(orqa, command),
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
