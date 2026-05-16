use clap::Args;
use std::ffi::OsString;

#[derive(Debug, Args)]
pub(crate) struct WakeArgs {
    /// Ignore pod and fin pause markers for this scan.
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
pub(crate) struct LoopCommand {
    /// Seconds between wake scans.
    #[arg(long, default_value_t = 60)]
    pub(crate) interval: u64,

    /// Ignore pod and fin pause markers for each scan.
    #[arg(long)]
    pub(crate) force: bool,

    /// Arguments passed to each wake scan.
    #[arg(last = true)]
    pub(crate) args: Vec<OsString>,
}
