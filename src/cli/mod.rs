mod fin;
mod loop_command;
mod mail;
mod pod;
mod task;

pub(crate) use fin::{
    ChatArgs, ExecArgs, FinCreateArgs, FinListArgs, FinRefArgs, FinResumeArgs, FinRoleSetArgs,
    FinRunReadArgs, FinRunsArgs, FinStatusArgs, FinTailArgs, SuperviseArgs,
};
pub(crate) use loop_command::{LoopCommand, WakeArgs};
pub(crate) use mail::{MailCommand, MailListArgs, MailMessageArgs, MailSubcommand, SendMailArgs};
#[allow(unused_imports)]
pub(crate) use pod::{
    InitArgs, PodCharterCommand, PodCharterSetArgs, PodCharterSubcommand, PodCommand,
    PodCreateArgs, PodDoctorArgs, PodHookAddArgs, PodHookCommand, PodHookListArgs, PodHookRefArgs,
    PodHookRunArgs, PodHookSubcommand, PodResumeArgs, PodStatusArgs, PodSubcommand, PodTailArgs,
    SlugArgs,
};
pub(crate) use task::{SendTaskArgs, TaskCommand, TaskListArgs, TaskSubcommand};

#[derive(Debug, Parser)]
#[command(
    name = "orqa",
    version,
    about = "Coordinate local agent pods and fins",
    long_about = None,
    disable_version_flag = true
)]
pub(crate) struct Cli {
    /// Override ORQA_HOME for this command.
    #[arg(long, global = true, value_name = "DIR")]
    pub(crate) home: Option<PathBuf>,
    /// Explicit pod context for commands that operate inside a pod.
    #[arg(long, global = true, value_name = "SLUG")]
    pub(crate) pod: Option<String>,
    /// Explicit fin context for commands that operate on one fin.
    #[arg(long, global = true, value_name = "SLUG")]
    pub(crate) fin: Option<String>,

    #[command(subcommand)]
    pub(crate) command: Option<Command>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct CommandContext {
    pub(crate) pod: Option<String>,
    pub(crate) fin: Option<String>,
}

impl CommandContext {
    pub(crate) fn new(pod: Option<String>, fin: Option<String>) -> Self {
        Self { pod, fin }
    }

    pub(crate) fn pod_arg(&self, command_pod: Option<String>) -> Option<String> {
        command_pod.or_else(|| self.pod.clone())
    }

    pub(crate) fn fin_arg(&self, command_fin: Option<String>) -> Option<String> {
        command_fin.or_else(|| self.fin.clone())
    }

    pub(crate) fn resolve_pod(
        &self,
        command_pod: Option<String>,
        orqa: &crate::model::Orqa,
    ) -> Result<(String, PathBuf), String> {
        crate::model::resolve_pod_context(self.pod_arg(command_pod), orqa)
    }

    pub(crate) fn resolve_fin(
        &self,
        command_pod: Option<String>,
        command_fin: Option<String>,
        orqa: &crate::model::Orqa,
    ) -> Result<crate::model::FinRef, String> {
        let (pod, _) = self.resolve_pod(command_pod, orqa)?;
        let fin = self
            .fin_arg(command_fin)
            .or_else(|| std::env::var("ORQA_FIN").ok())
            .ok_or_else(|| {
                "missing fin: pass --fin, set ORQA_FIN, or provide a fin argument".to_string()
            })?;
        crate::model::FinRef::new(&pod, &fin)
    }
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Show runtime diagnostics.
    Doctor,
    /// Print the operational guide.
    Guide,
    /// Initialize a pod in this directory.
    Init(InitArgs),
    /// Manage pods.
    Pod(PodCommand),
    /// Manage fins.
    Fin(FinCommand),
    /// Send and read fin mail.
    Mail(MailCommand),
    /// Assign and track fin tasks.
    Task(TaskCommand),
    /// Monitor pods.
    Ops(OpsCommand),
    /// Run one wake cycle.
    Wake(WakeArgs),
    /// Run wake cycles repeatedly.
    Loop(LoopCommand),
}

#[derive(Debug, Args)]
pub(crate) struct FinCommand {
    #[command(subcommand)]
    pub(crate) command: FinSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum FinSubcommand {
    /// List fins.
    List(FinListArgs),
    /// Create a fin.
    Create(FinCreateArgs),
    /// Get or set a fin role.
    Role(FinRoleCommand),
    /// Print the home directory for a fin.
    Home(FinRefArgs),
    /// Show fin status.
    Status(FinStatusArgs),
    /// List fin runs.
    Runs(FinRunsArgs),
    /// Show run status.
    #[command(name = "run-status")]
    RunStatus(FinRunReadArgs),
    /// Show run logs.
    #[command(name = "run-log")]
    RunLog(FinRunReadArgs),
    /// Tail recent run output.
    Tail(FinTailArgs),
    /// Pause a fin.
    Pause(FinRefArgs),
    /// Resume a fin.
    Resume(FinResumeArgs),
    /// Run a one-shot backend command.
    Exec(ExecArgs),
    /// Start backend chat.
    Chat(ChatArgs),
    /// Internal supervised runner used by wake loops.
    #[command(hide = true)]
    Supervise(SuperviseArgs),
}

#[derive(Debug, Args)]
#[command(subcommand_required = true, arg_required_else_help = true)]
pub(crate) struct OpsCommand {
    #[command(subcommand)]
    pub(crate) command: OpsSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum OpsSubcommand {
    /// Generate a Markdown report.
    Report(OpsReportArgs),
}

#[derive(Debug, Args)]
pub(crate) struct FinRoleCommand {
    #[command(subcommand)]
    pub(crate) command: FinRoleSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum FinRoleSubcommand {
    /// Print the fin role.
    Get(FinRefArgs),
    /// Replace the fin role.
    Set(FinRoleSetArgs),
}

#[derive(Debug, Args, Default)]
pub(crate) struct OpsReportArgs {
    /// Include only records at or after this time. Accepts Unix seconds or relative durations like 30m, 2h, 1d.
    #[arg(long)]
    pub(crate) since: Option<String>,
}
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
