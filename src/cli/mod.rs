mod fin;
mod loop_command;
mod mail;
mod pod;
mod task;

pub(crate) use fin::{
    ChatArgs, ExecArgs, FinCreateArgs, FinListArgs, FinRefArgs, FinRoleSetArgs, FinRunReadArgs,
    FinRunsArgs, FinStatusArgs, FinTailArgs, FinWakeArgs, SuperviseArgs,
};
pub(crate) use loop_command::{
    LoopCommand, LoopPlanArgs, LoopRunArgs, LoopStartArgs, LoopSubcommand,
};
pub(crate) use mail::{MailCommand, MailListArgs, MailMessageArgs, MailSubcommand, SendMailArgs};
#[allow(unused_imports)]
pub(crate) use pod::{
    InitArgs, PodCharterCommand, PodCharterSetArgs, PodCharterSubcommand, PodCommand,
    PodCreateArgs, PodDoctorArgs, PodHookAddArgs, PodHookCommand, PodHookListArgs, PodHookRefArgs,
    PodHookRunArgs, PodHookSubcommand, PodStatusArgs, PodSubcommand, PodTailArgs, PodWakeArgs,
    SlugArgs,
};
pub(crate) use task::{SendTaskArgs, TaskCommand, TaskListArgs, TaskSubcommand};

#[derive(Debug, Parser)]
#[command(
    name = "orqa",
    version,
    about = "Fan out work to background fins",
    long_about = None,
    disable_help_subcommand(true)
)]
pub(crate) struct Cli {
    /// Override ORQA_HOME for this command.
    #[arg(long, global = true, value_name = "DIR")]
    pub(crate) home: Option<PathBuf>,

    #[command(subcommand)]
    pub(crate) command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Show basic runtime information.
    Doctor,
    /// Print the operational guide for agents using Orqa.
    #[command(name = "help")]
    Guide,
    /// Initialize a new pod in the current (or specified) directory.
    /// This is the recommended way to start using Orqa inside a project.
    Init(InitArgs),
    /// Create or inspect pods.
    ///
    /// Use `orqa init` for the most common case of starting a pod inside a project directory.
    Pod(PodCommand),
    /// Create or operate fins inside a pod.
    Fin(FinCommand),
    /// Mail helpers for pod-local fin messages.
    Mail(MailCommand),
    /// Task helpers for pod-local work items.
    Task(TaskCommand),
    /// Human operator surface for cross-pod monitoring.
    Ops(OpsCommand),
    /// Manage the wake loop (run, plan, start, stop, status).
    Loop(LoopCommand),
    /// Show the wake plan for a pod without running fins.
    Plan(LoopPlanArgs),
    /// Backward-compatible service runner.
    Service(ServiceCommand),
}

#[derive(Debug, Args)]
pub(crate) struct FinCommand {
    #[command(subcommand)]
    pub(crate) command: FinSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum FinSubcommand {
    /// List fins inside a pod.
    List(FinListArgs),
    /// Create a fin inside a pod.
    Create(FinCreateArgs),
    /// Get or set a fin role.
    Role(FinRoleCommand),
    /// Print the home directory for a fin.
    Home(FinRefArgs),
    /// Print fin runtime status.
    Status(FinStatusArgs),
    /// List recorded runs for a fin.
    Runs(FinRunsArgs),
    /// Print recorded run status for a fin.
    #[command(name = "run-status")]
    RunStatus(FinRunReadArgs),
    /// Print recorded run logs for a fin.
    #[command(name = "run-log")]
    RunLog(FinRunReadArgs),
    /// Print recent run output for a fin.
    Tail(FinTailArgs),
    /// Pause wake-loop runs for a fin.
    Sleep(FinRefArgs),
    /// Clear a fin sleep marker.
    Wake(FinWakeArgs),
    /// Execute a one-shot fin backend command.
    Exec(ExecArgs),
    /// Start an interactive fin backend chat.
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

#[derive(Debug, Args)]
#[command(subcommand_required = true, arg_required_else_help = true)]
pub(crate) struct ServiceCommand {
    #[command(subcommand)]
    pub(crate) command: ServiceSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ServiceSubcommand {
    /// Run the wake service loop.
    Run(LoopStartArgs),
}

#[derive(Debug, Subcommand)]
pub(crate) enum OpsSubcommand {
    /// Generate a Markdown report of pods, tasks, and mail.
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
