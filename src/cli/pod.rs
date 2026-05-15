use clap::Args;
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
pub(crate) struct PodWakeArgs {
    /// Pod slug.
    pub(crate) slug: String,
    /// Required to clear sleep state.
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
    /// Create the pod rooted in this directory (new-style pod).
    /// This is the explicit/power-user equivalent of `orqa init`.
    #[arg(long, value_name = "DIR")]
    pub(crate) path: Option<PathBuf>,
    /// Pod charter text, @file path, or - for stdin.
    #[arg(long, value_name = "PROMPT|@FILE|-")]
    pub(crate) charter: Option<String>,
}
