use clap::{Args, Subcommand};
use std::ffi::OsString;

#[derive(Debug, Args)]
pub(crate) struct LoopCommand {
    /// Pod slug. If omitted, loops all pods.
    pub(crate) pod: Option<String>,
    /// Ignore pod and fin sleep markers for this scan.
    #[arg(long)]
    pub(crate) force: bool,
    /// Print wake decisions without running fins.
    #[arg(long)]
    pub(crate) dry_run: bool,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(crate) json: bool,
    /// Arguments used to build the backend prompt.
    #[arg(last = true)]
    pub(crate) args: Vec<OsString>,
    #[command(subcommand)]
    pub(crate) command: Option<LoopSubcommand>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum LoopSubcommand {
    /// Run the wake loop for one or all pods.
    Run(LoopRunArgs),

    /// Show the wake plan for a pod without running fins.
    Plan(LoopPlanArgs),

    /// Start the wake loop as a background daemon.
    Start(LoopStartArgs),

    /// Stop the running wake loop daemon.
    Stop,

    /// Show status of the wake loop daemon.
    Status,
}

#[derive(Debug, Args)]
pub(crate) struct LoopRunArgs {
    /// Pod slug. If omitted, loops all pods.
    pub(crate) pod: Option<String>,
    /// Ignore pod and fin sleep markers for this scan.
    #[arg(long)]
    pub(crate) force: bool,
    /// Print wake decisions without running fins.
    #[arg(long)]
    pub(crate) dry_run: bool,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(crate) json: bool,
    /// Arguments used to build the backend prompt.
    #[arg(last = true)]
    pub(crate) args: Vec<OsString>,
}

#[derive(Debug, Args)]
pub(crate) struct LoopPlanArgs {
    /// Pod slug.
    pub(crate) pod: String,
    /// Ignore pod and fin sleep markers while planning.
    #[arg(long)]
    pub(crate) force: bool,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Debug, Args)]
pub(crate) struct LoopStartArgs {
    /// Seconds between wake scans.
    #[arg(long, default_value_t = 60)]
    pub(crate) interval: u64,

    /// Ignore pod and fin sleep markers for each scan.
    #[arg(long)]
    pub(crate) force: bool,

    /// Arguments passed to each wake-loop scan.
    #[arg(last = true)]
    pub(crate) args: Vec<OsString>,
}
