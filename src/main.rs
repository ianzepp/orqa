use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "orqa",
    version,
    about = "Fan out work to background agents",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show basic runtime information.
    Doctor,
}

fn main() {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Command::Doctor) {
        Command::Doctor => doctor(),
    }
}

fn doctor() {
    println!("orqa is installed and ready.");
}
