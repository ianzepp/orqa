mod cli;
mod commands;
mod config;
mod doctor;
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
use commands::{fin, loop_command, mail, ops, overview, pod, pod_init, task, template};
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
        // If we are inside a detectable pod root, launch the Operator Cockpit TUI.
        match context.resolve_pod(None, &orqa) {
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
        Command::Guide => {
            print_operational_help();
            Ok(())
        }
        Command::Init(args) => pod_init(orqa, args),
        Command::Pod(command) => pod(orqa, context, command),
        Command::Fin(command) => fin(orqa, context, command),
        Command::Mail(command) => mail(orqa, context, command),
        Command::Task(command) => task(orqa, context, command),
        Command::Template(command) => template(orqa, command),
        Command::Ops(command) => ops(orqa, command),
        Command::Wake(args) => runtime::wake_current_pod(orqa, context, args),
        Command::Loop(command) => loop_command(orqa, context, command),
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
