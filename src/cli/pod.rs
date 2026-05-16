use clap::{Args, Subcommand};
use std::{ffi::OsString, path::PathBuf};

#[derive(Debug, Args)]
pub(crate) struct PodTailArgs {
    /// Number of lines per stream.
    #[arg(long, default_value_t = 80)]
    pub(crate) lines: usize,
    /// Continue printing as logs grow.
    #[arg(short = 'f', long)]
    pub(crate) follow: bool,
}

#[derive(Debug, Args)]
pub(crate) struct PodResumeArgs {
    /// Required to clear pause state.
    #[arg(long)]
    pub(crate) force: bool,
}
#[derive(Debug, Args)]
pub(crate) struct PodHookListArgs {}

#[derive(Debug, Args)]
pub(crate) struct PodHookAddArgs {
    /// Hook phase. Currently only pre-plan is supported.
    pub(crate) phase: String,
    /// Hook id, commonly prefixed for sort order, such as 10-sync-mail.
    pub(crate) hook: String,
    /// Hook timeout, such as 30s, 5m, or 1h.
    #[arg(long, default_value = "30s")]
    pub(crate) timeout: String,
    /// Shell command to execute from the hook phase directory.
    #[arg(last = true, required = true)]
    pub(crate) command: Vec<OsString>,
}

#[derive(Debug, Args)]
pub(crate) struct PodHookRefArgs {
    /// Hook phase. Currently only pre-plan is supported.
    pub(crate) phase: String,
    /// Hook id.
    pub(crate) hook: String,
}

#[derive(Debug, Args)]
pub(crate) struct PodHookRunArgs {
    /// Hook phase. Currently only pre-plan is supported.
    pub(crate) phase: String,
}
#[derive(Debug, Args)]
pub(crate) struct PodStatusArgs {
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Debug, Args)]
pub(crate) struct PodDoctorArgs {
    /// Probe prompt passed to each fin backend.
    #[arg(long, default_value = "Reply with exactly: orqa-ok")]
    pub(crate) prompt: String,
    /// Seconds to wait for each backend probe.
    #[arg(long, default_value_t = 120)]
    pub(crate) timeout: u64,
}
#[derive(Debug, Args)]
pub(crate) struct SlugArgs {}

#[derive(Debug, Args)]
pub(crate) struct PodCreateArgs {
    /// Pod slug (required).
    pub(crate) slug: String,
    /// Create the pod rooted in this directory.
    /// This is the explicit/power-user equivalent of `orqa init`.
    #[arg(long, value_name = "DIR")]
    pub(crate) path: Option<PathBuf>,
    /// Pod charter text, @file path, or - for stdin.
    #[arg(long, value_name = "PROMPT|@FILE|-")]
    pub(crate) charter: Option<String>,
    /// Seed fins from a global template under ORQA_HOME/templates.
    #[arg(long, value_name = "TEMPLATE")]
    pub(crate) template: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct InitArgs {
    /// Pod slug (defaults to the current directory name).
    pub(crate) slug: Option<String>,
    /// Directory in which to initialize the pod (defaults to current directory).
    #[arg(long, value_name = "DIR")]
    pub(crate) path: Option<PathBuf>,
    /// Pod charter text, @file path, or - for stdin.
    #[arg(long, value_name = "PROMPT|@FILE|-")]
    pub(crate) charter: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct PodCharterSetArgs {
    /// Pod charter text, @file path, or - for stdin.
    #[arg(value_name = "PROMPT|@FILE|-")]
    pub(crate) charter: String,
}
#[derive(Debug, Args)]
pub(crate) struct PodCharterCommand {
    #[command(subcommand)]
    pub(crate) command: PodCharterSubcommand,
}

#[derive(Debug, Args)]
pub(crate) struct PodHookCommand {
    #[command(subcommand)]
    pub(crate) command: PodHookSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum PodHookSubcommand {
    /// List hooks.
    List(PodHookListArgs),
    /// Add a hook.
    Add(PodHookAddArgs),
    /// Enable a hook.
    Enable(PodHookRefArgs),
    /// Disable a hook.
    Disable(PodHookRefArgs),
    /// Remove a hook definition and adjacent script.
    Remove(PodHookRefArgs),
    /// Run hooks for one phase.
    Run(PodHookRunArgs),
}

#[derive(Debug, Subcommand)]
pub(crate) enum PodCharterSubcommand {
    /// Print the pod charter.
    Get(SlugArgs),
    /// Replace the pod charter.
    Set(PodCharterSetArgs),
}
#[derive(Debug, Args)]
pub(crate) struct PodCommand {
    #[command(subcommand)]
    pub(crate) command: PodSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum PodSubcommand {
    /// List pods.
    List,
    /// Create a pod.
    Create(PodCreateArgs),
    /// Get or set a pod charter.
    Charter(PodCharterCommand),
    /// Print the home directory for a pod.
    Home(SlugArgs),
    /// Show pod status.
    Status(PodStatusArgs),
    /// Check pod health.
    Doctor(PodDoctorArgs),
    /// Manage hooks.
    Hook(PodHookCommand),
    /// Tail recent fin output.
    Tail(PodTailArgs),
    /// Pause a pod.
    Pause(SlugArgs),
    /// Resume a pod.
    Resume(PodResumeArgs),
}
