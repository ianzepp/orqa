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
mod service;
mod status;

use std::{env, ffi::OsStr, process::ExitCode};

use clap::{CommandFactory, FromArgMatches};

use cli::{Cli, Command};
use commands::{fin, mail, ops, pod, task};
use model::Orqa;
use runtime::{loop_pod, plan};
use service::service;

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

    match run(&orqa, cli.command.unwrap_or(Command::Doctor)) {
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
        Command::Loop(args) => loop_pod(orqa, args),
        Command::Plan(args) => plan(orqa, args),
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
