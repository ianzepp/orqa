use clap::Args;
use std::ffi::OsString;

#[derive(Debug, Args)]
pub(crate) struct ExecArgs {
    /// Fin slug inside the pod. Defaults to global --fin or ORQA_FIN.
    pub(crate) fin: Option<String>,
    /// Arguments used to build the backend prompt.
    #[arg(last = true)]
    pub(crate) args: Vec<OsString>,
}

#[derive(Debug, Args)]
pub(crate) struct ChatArgs {
    /// Fin slug inside the pod. Defaults to global --fin or ORQA_FIN.
    pub(crate) fin: Option<String>,
    /// Arguments appended to the configured chat command.
    #[arg(last = true)]
    pub(crate) args: Vec<OsString>,
}

#[derive(Debug, Args)]
pub(crate) struct SuperviseArgs {
    /// Pod slug.
    pub(crate) pod: String,
    /// Fin slug inside the pod.
    pub(crate) fin: String,
    /// Resolved backend name.
    #[arg(long)]
    pub(crate) backend: String,
    /// Resolved backend executable.
    #[arg(long = "backend-command")]
    pub(crate) backend_command: OsString,
    /// Resolved backend arguments.
    #[arg(last = true)]
    pub(crate) args: Vec<OsString>,
}
#[derive(Debug, Args)]
pub(crate) struct FinRunsArgs {
    /// Fin slug inside the pod. Defaults to global --fin or ORQA_FIN.
    pub(crate) fin: Option<String>,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Debug, Args)]
pub(crate) struct FinRunReadArgs {
    /// Fin slug inside the pod. Defaults to global --fin or ORQA_FIN.
    pub(crate) fin: Option<String>,
    /// Run id. Defaults to latest.
    pub(crate) run: Option<String>,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Debug, Args)]
pub(crate) struct FinTailArgs {
    /// Fin slug inside the pod. Defaults to global --fin or ORQA_FIN.
    pub(crate) fin: Option<String>,
    /// Run id. Defaults to latest.
    pub(crate) run: Option<String>,
    /// Number of lines per stream.
    #[arg(long, default_value_t = 80)]
    pub(crate) lines: usize,
    /// Continue printing as logs grow.
    #[arg(short = 'f', long)]
    pub(crate) follow: bool,
}

#[derive(Debug, Args)]
pub(crate) struct FinResumeArgs {
    /// Fin slug inside the pod. Defaults to global --fin or ORQA_FIN.
    pub(crate) fin: Option<String>,
    /// Required to clear pause state.
    #[arg(long)]
    pub(crate) force: bool,
}
#[derive(Debug, Args)]
pub(crate) struct FinListArgs {}

#[derive(Debug, Args)]
pub(crate) struct FinCreateArgs {
    /// Fin slug.
    pub(crate) fin: String,
    /// Fin role text, @file path, or - for stdin.
    #[arg(long, value_name = "PROMPT|@FILE|-")]
    pub(crate) role: Option<String>,
    /// Backend name to write into fin.toml.
    #[arg(long, value_name = "BACKEND")]
    pub(crate) backend: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct FinRoleSetArgs {
    /// Either <role>, or explicit <fin> <role>.
    #[arg(value_name = "FIN_OR_PROMPT", num_args = 1..=2)]
    pub(crate) refs: Vec<String>,
}

impl FinRoleSetArgs {
    pub(crate) fn resolve_refs(&self) -> Result<(Option<String>, String), String> {
        match self.refs.as_slice() {
            [role] => Ok((None, role.clone())),
            [fin, role] => Ok((Some(fin.clone()), role.clone())),
            _ => Err("usage: orqa fin role set [fin] <role>".to_string()),
        }
    }
}

#[derive(Debug, Args)]
pub(crate) struct FinRefArgs {
    /// Fin slug inside the pod. Defaults to global --fin or ORQA_FIN.
    pub(crate) fin: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct FinStatusArgs {
    /// Fin slug inside the pod. Defaults to global --fin or ORQA_FIN.
    pub(crate) fin: Option<String>,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(crate) json: bool,
}
