use clap::{Args, Subcommand};
use std::{ffi::OsString, path::PathBuf};

#[derive(Debug, Args)]
pub(crate) struct PodTailArgs {
    /// Pod slug.
    pub(crate) pod: String,
    /// Restrict output to one fin.
    #[arg(long)]
    pub(crate) fin: Option<String>,
    /// Number of lines per stream.
    #[arg(long, default_value_t = 80)]
    pub(crate) lines: usize,
    /// Continue printing as logs grow.
    #[arg(short = 'f', long)]
    pub(crate) follow: bool,
}

#[derive(Debug, Args)]
pub(crate) struct PodResumeArgs {
    /// Pod slug.
    pub(crate) slug: String,
    /// Required to clear pause state.
    #[arg(long)]
    pub(crate) force: bool,
}
#[derive(Debug, Args)]
pub(crate) struct PodHookListArgs {
    /// Pod slug.
    pub(crate) pod: String,
}

#[derive(Debug, Args)]
pub(crate) struct PodHookAddArgs {
    /// Pod slug.
    pub(crate) pod: String,
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
    /// Pod slug.
    pub(crate) pod: String,
    /// Hook phase. Currently only pre-plan is supported.
    pub(crate) phase: String,
    /// Hook id.
    pub(crate) hook: String,
}

#[derive(Debug, Args)]
pub(crate) struct PodHookRunArgs {
    /// Pod slug.
    pub(crate) pod: String,
    /// Hook phase. Currently only pre-plan is supported.
    pub(crate) phase: String,
}
#[derive(Debug, Args)]
pub(crate) struct PodStatusArgs {
    /// Pod slug.
    pub(crate) pod: String,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Debug, Args)]
pub(crate) struct PodDoctorArgs {
    /// Pod slug.
    pub(crate) pod: String,
    /// Restrict checks to one fin.
    #[arg(long)]
    pub(crate) fin: Option<String>,
    /// Probe prompt passed to each fin backend.
    #[arg(long, default_value = "Reply with exactly: orqa-ok")]
    pub(crate) prompt: String,
    /// Seconds to wait for each backend probe.
    #[arg(long, default_value_t = 120)]
    pub(crate) timeout: u64,
}
#[derive(Debug, Args)]
pub(crate) struct SlugArgs {
    /// Pod slug.
    pub(crate) slug: String,
}

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
    /// Pod slug.
    pub(crate) slug: String,
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
    /// List hooks for a pod.
    List(PodHookListArgs),
    /// Add a hook definition and adjacent script stub.
    Add(PodHookAddArgs),
    /// Enable a hook.
    Enable(PodHookRefArgs),
    /// Disable a hook.
    Disable(PodHookRefArgs),
    /// Remove a hook definition and adjacent script.
    Remove(PodHookRefArgs),
    /// Run enabled hooks for one phase.
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
    ///
    /// Without --path, uses the current directory as the pod root.
    /// With --path, uses the given directory as the pod root.
    Create(PodCreateArgs),
    /// Get or set a pod charter.
    Charter(PodCharterCommand),
    /// Print the home directory for a pod.
    Home(SlugArgs),
    /// Print pod runtime status.
    Status(PodStatusArgs),
    /// Check pod filesystem, config, backend command, and LLM connectivity.
    Doctor(PodDoctorArgs),
    /// Manage pod lifecycle hooks.
    Hook(PodHookCommand),
    /// Print recent run output for fins in a pod.
    Tail(PodTailArgs),
    /// Pause wake runs for a pod.
    Pause(SlugArgs),
    /// Resume wake eligibility for a pod.
    Resume(PodResumeArgs),
}
