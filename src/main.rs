mod cli;
mod commands;
mod config;
mod mailbox;
mod model;
mod runtime;

use std::process::ExitCode;

use clap::Parser;

use cli::{Cli, Command};
use commands::{fin, mail, pod, task};
use model::Orqa;
use runtime::loop_pod;

#[cfg(test)]
#[path = "main_test.rs"]
mod tests;

fn main() -> ExitCode {
    let cli = Cli::parse();
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
        Command::Pod(command) => pod(orqa, command),
        Command::Fin(command) => fin(orqa, command),
        Command::Mail(command) => mail(orqa, command),
        Command::Task(command) => task(orqa, command),
        Command::Loop(args) => loop_pod(orqa, args),
    }
}

fn doctor(orqa: &Orqa) -> Result<(), String> {
    println!("orqa is installed and ready.");
    println!("orqa_home={}", orqa.home.display());
    Ok(())
}
