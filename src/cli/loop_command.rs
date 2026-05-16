use clap::Args;
use std::ffi::OsString;

#[derive(Debug, Args)]
pub(crate) struct WakeArgs {
    /// Ignore pause markers.
    #[arg(long)]
    pub(crate) force: bool,
    /// Print decisions without running fins.
    #[arg(long)]
    pub(crate) dry_run: bool,
    /// Emit JSON.
    #[arg(long)]
    pub(crate) json: bool,
    /// Backend prompt arguments.
    #[arg(last = true)]
    pub(crate) args: Vec<OsString>,
}

#[derive(Debug, Args)]
pub(crate) struct LoopCommand {
    /// Seconds between wake scans.
    #[arg(long, default_value_t = 60)]
    pub(crate) interval: u64,

    /// Ignore pause markers.
    #[arg(long)]
    pub(crate) force: bool,

    /// Backend prompt arguments.
    #[arg(last = true)]
    pub(crate) args: Vec<OsString>,
}
