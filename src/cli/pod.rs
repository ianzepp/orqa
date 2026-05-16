use clap::{Args, Subcommand};
use std::{ffi::OsString, path::PathBuf};

#[derive(Debug, Args)]
pub(crate) struct PodTailArgs {
    /// Pod slug. Defaults to global --pod or ORQA_POD.
    pub(crate) pod: Option<String>,
    /// Number of lines per stream.
    #[arg(long, default_value_t = 80)]
    pub(crate) lines: usize,
    /// Continue printing as logs grow.
    #[arg(short = 'f', long)]
    pub(crate) follow: bool,
}

#[derive(Debug, Args)]
pub(crate) struct PodResumeArgs {
    /// Pod slug. Defaults to global --pod or ORQA_POD.
    pub(crate) slug: Option<String>,
    /// Required to clear pause state.
    #[arg(long)]
    pub(crate) force: bool,
}
#[derive(Debug, Args)]
pub(crate) struct PodHookListArgs {
    /// Pod slug. Defaults to global --pod or ORQA_POD.
    pub(crate) pod: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct PodHookAddArgs {
    /// Either <phase> <hook>, or explicit <pod> <phase> <hook>.
    #[arg(value_name = "POD_OR_PHASE", num_args = 2..=3)]
    pub(crate) refs: Vec<String>,
    /// Hook timeout, such as 30s, 5m, or 1h.
    #[arg(long, default_value = "30s")]
    pub(crate) timeout: String,
    /// Shell command to execute from the hook phase directory.
    #[arg(last = true, required = true)]
    pub(crate) command: Vec<OsString>,
}

impl PodHookAddArgs {
    pub(crate) fn resolve_refs(&self) -> Result<(Option<String>, String, String), String> {
        match self.refs.as_slice() {
            [phase, hook] => Ok((None, phase.clone(), hook.clone())),
            [pod, phase, hook] => Ok((Some(pod.clone()), phase.clone(), hook.clone())),
            _ => Err("usage: orqa pod hook add [pod] <phase> <hook> -- <command>".to_string()),
        }
    }
}

#[derive(Debug, Args)]
pub(crate) struct PodHookRefArgs {
    /// Either <phase> <hook>, or explicit <pod> <phase> <hook>.
    #[arg(value_name = "POD_OR_PHASE", num_args = 2..=3)]
    pub(crate) refs: Vec<String>,
}

impl PodHookRefArgs {
    pub(crate) fn resolve_refs(&self) -> Result<(Option<String>, String, String), String> {
        match self.refs.as_slice() {
            [phase, hook] => Ok((None, phase.clone(), hook.clone())),
            [pod, phase, hook] => Ok((Some(pod.clone()), phase.clone(), hook.clone())),
            _ => {
                Err("usage: orqa pod hook <enable|disable|remove> [pod] <phase> <hook>".to_string())
            }
        }
    }
}

#[derive(Debug, Args)]
pub(crate) struct PodHookRunArgs {
    /// Either <phase>, or explicit <pod> <phase>.
    #[arg(value_name = "POD_OR_PHASE", num_args = 1..=2)]
    pub(crate) refs: Vec<String>,
}

impl PodHookRunArgs {
    pub(crate) fn resolve_refs(&self) -> Result<(Option<String>, String), String> {
        match self.refs.as_slice() {
            [phase] => Ok((None, phase.clone())),
            [pod, phase] => Ok((Some(pod.clone()), phase.clone())),
            _ => Err("usage: orqa pod hook run [pod] <phase>".to_string()),
        }
    }
}
#[derive(Debug, Args)]
pub(crate) struct PodStatusArgs {
    /// Pod slug. Defaults to global --pod or ORQA_POD.
    pub(crate) pod: Option<String>,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Debug, Args)]
pub(crate) struct PodDoctorArgs {
    /// Pod slug. Defaults to global --pod or ORQA_POD.
    pub(crate) pod: Option<String>,
    /// Probe prompt passed to each fin backend.
    #[arg(long, default_value = "Reply with exactly: orqa-ok")]
    pub(crate) prompt: String,
    /// Seconds to wait for each backend probe.
    #[arg(long, default_value_t = 120)]
    pub(crate) timeout: u64,
}
#[derive(Debug, Args)]
pub(crate) struct SlugArgs {
    /// Pod slug. Defaults to global --pod or ORQA_POD.
    pub(crate) slug: Option<String>,
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
    /// Either <charter>, or explicit <pod> <charter>.
    #[arg(value_name = "POD_OR_PROMPT", num_args = 1..=2)]
    pub(crate) refs: Vec<String>,
}

impl PodCharterSetArgs {
    pub(crate) fn resolve_refs(&self) -> Result<(Option<String>, String), String> {
        match self.refs.as_slice() {
            [charter] => Ok((None, charter.clone())),
            [pod, charter] => Ok((Some(pod.clone()), charter.clone())),
            _ => Err("usage: orqa pod charter set [pod] <charter>".to_string()),
        }
    }
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
