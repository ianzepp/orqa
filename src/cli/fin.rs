use clap::Args;
use std::ffi::OsString;

#[derive(Debug, Args)]
pub(crate) struct ExecArgs {
    /// Pod slug. Defaults to global --pod or ORQA_POD.
    pub(crate) pod: Option<String>,
    /// Fin slug inside the pod. Defaults to global --fin or ORQA_FIN.
    pub(crate) fin: Option<String>,
    /// Arguments used to build the backend prompt.
    #[arg(last = true)]
    pub(crate) args: Vec<OsString>,
}

#[derive(Debug, Args)]
pub(crate) struct ChatArgs {
    /// Pod slug. Defaults to global --pod or ORQA_POD.
    pub(crate) pod: Option<String>,
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
    /// Pod slug. Defaults to global --pod or ORQA_POD.
    pub(crate) pod: Option<String>,
    /// Fin slug inside the pod. Defaults to global --fin or ORQA_FIN.
    pub(crate) fin: Option<String>,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Debug, Args)]
pub(crate) struct FinRunReadArgs {
    /// Pod slug. Defaults to global --pod or ORQA_POD.
    pub(crate) pod: Option<String>,
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
    /// Pod slug. Defaults to global --pod or ORQA_POD.
    pub(crate) pod: Option<String>,
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
    /// Pod slug. Defaults to global --pod or ORQA_POD.
    pub(crate) pod: Option<String>,
    /// Fin slug inside the pod. Defaults to global --fin or ORQA_FIN.
    pub(crate) fin: Option<String>,
    /// Required to clear pause state.
    #[arg(long)]
    pub(crate) force: bool,
}
#[derive(Debug, Args)]
pub(crate) struct FinListArgs {
    /// Pod slug. Defaults to ORQA_POD.
    pub(crate) pod: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct FinCreateArgs {
    /// Fin slug, or explicit pod slug followed by fin slug.
    #[arg(value_name = "POD_OR_FIN", num_args = 1..=2)]
    pub(crate) refs: Vec<String>,
    /// Fin role text, @file path, or - for stdin.
    #[arg(long, value_name = "PROMPT|@FILE|-")]
    pub(crate) role: Option<String>,
    /// Backend name to write into fin.toml.
    #[arg(long, value_name = "BACKEND")]
    pub(crate) backend: Option<String>,
}

impl FinCreateArgs {
    pub(crate) fn resolve_refs(&self) -> Result<(Option<String>, String), String> {
        match self.refs.as_slice() {
            [fin] => Ok((None, fin.clone())),
            [pod, fin] => Ok((Some(pod.clone()), fin.clone())),
            _ => Err("usage: orqa fin create [pod] <fin>".to_string()),
        }
    }
}

#[derive(Debug, Args)]
pub(crate) struct FinRoleSetArgs {
    /// Either <role>, <fin> <role>, or explicit <pod> <fin> <role>.
    #[arg(value_name = "POD_OR_FIN_OR_PROMPT", num_args = 1..=3)]
    pub(crate) refs: Vec<String>,
}

impl FinRoleSetArgs {
    pub(crate) fn resolve_refs(&self) -> Result<(Option<String>, Option<String>, String), String> {
        match self.refs.as_slice() {
            [role] => Ok((None, None, role.clone())),
            [fin, role] => Ok((None, Some(fin.clone()), role.clone())),
            [pod, fin, role] => Ok((Some(pod.clone()), Some(fin.clone()), role.clone())),
            _ => Err("usage: orqa fin role set [pod] [fin] <role>".to_string()),
        }
    }
}

#[derive(Debug, Args)]
pub(crate) struct FinRefArgs {
    /// Pod slug. Defaults to global --pod or ORQA_POD.
    pub(crate) pod: Option<String>,
    /// Fin slug inside the pod. Defaults to global --fin or ORQA_FIN.
    pub(crate) fin: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct FinStatusArgs {
    /// Pod slug. Defaults to global --pod or ORQA_POD.
    pub(crate) pod: Option<String>,
    /// Fin slug inside the pod. Defaults to global --fin or ORQA_FIN.
    pub(crate) fin: Option<String>,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(crate) json: bool,
}
