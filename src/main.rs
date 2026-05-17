mod cli;
mod commands;
mod config;
mod doctor;
mod global_loop;
mod hooks;
mod loop_worker;
mod mailbox;
mod model;
mod report;
mod runs;
mod runtime;
mod runtime_home;
mod status;
mod tui;

use std::{env, process::ExitCode};

use clap::{Arg, ArgAction, Command as ClapCommand, CommandFactory, FromArgMatches};

#[allow(unused_imports)]
use cli::{Cli, Command, CommandContext, InitArgs};
use commands::{fin, loop_command, mail, ops, pod, pod_init, task, template};
use model::Orqa;

#[cfg(test)]
#[path = "main_test.rs"]
mod tests;

const TOP_LEVEL_HELP_TEMPLATE: &str =
    "{about}\n\nUsage: {usage}\n\nOptions:\n{options}\n\nCommands:\n{subcommands}";

fn main() -> ExitCode {
    let matches = cli_command().get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|error| error.exit());
    let context = CommandContext::new(cli.context_pod.clone(), cli.context_fin.clone());
    let orqa = Orqa::new(cli.home);

    // Internal loop worker mode used by the TUI.
    if env::var("ORQA_LOOP_WORKER").is_ok() {
        if let Err(error) = loop_worker::run(&orqa) {
            eprintln!("loop worker error: {error}");
            return ExitCode::FAILURE;
        }
        return ExitCode::SUCCESS;
    }

    let Some(command) = cli.command else {
        // Bare `orqa` (no subcommand) is now the primary way to enter the TUI.
        // It only makes sense inside a pod root.
        match context.resolve_pod(None, &orqa) {
            Ok((pod_slug, pod_root)) => {
                if let Err(error) = crate::tui::run_tui(&pod_slug, &pod_root) {
                    eprintln!("orqa tui: {error}");
                    return ExitCode::FAILURE;
                }
                return ExitCode::SUCCESS;
            }
            Err(_) => {
                // Running bare `orqa` outside any project should be a clear error,
                // similar to `git` outside a repository.
                let cwd = std::env::current_dir()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| "<unknown>".to_string());

                eprintln!("orqa: project is not initialized");
                eprintln!();
                eprintln!("  {}", cwd);
                eprintln!();
                eprintln!("Run `orqa init` to initialize a pod here.");
                eprintln!("Run `orqa --help` for a full set of commands.");
                return ExitCode::FAILURE;
            }
        }
    };

    match run(&orqa, &context, command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("orqa: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(orqa: &Orqa, context: &CommandContext, command: Command) -> Result<(), String> {
    match command {
        Command::Doctor => doctor(orqa),
        Command::Top => crate::tui::run_top(orqa),
        Command::Daemon(args) => crate::global_loop::run_daemon(orqa, args.interval, args.args),
        Command::Guide => {
            print_operational_help();
            Ok(())
        }
        Command::Init(args) => pod_init(orqa, args),
        Command::Pod(command) => pod(orqa, context, command),
        Command::Fin(command) => fin(orqa, context, command),
        Command::Mail(command) => mail(orqa, context, command),
        Command::Task(command) => task(orqa, context, command),
        Command::Template(command) => template(orqa, context, command),
        Command::Ops(command) => ops(orqa, command),
        Command::Wake(args) => runtime::wake_current_pod(orqa, context, args),
        Command::Loop(command) => loop_command(orqa, context, command),
    }
}

fn doctor(orqa: &Orqa) -> Result<(), String> {
    crate::doctor::global_doctor(orqa)
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
