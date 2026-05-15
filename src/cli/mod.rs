mod loop_command;
mod pod;

pub(crate) use loop_command::{
    LoopCommand, LoopPlanArgs, LoopRunArgs, LoopStartArgs, LoopSubcommand,
};
pub(crate) use pod::{
    PodCreateArgs, PodDoctorArgs, PodHookAddArgs, PodHookListArgs, PodHookRefArgs, PodHookRunArgs,
    PodStatusArgs, PodTailArgs, PodWakeArgs, SlugArgs,
};

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
    /// Without --path, creates a legacy pod under ORQA_HOME/pods/.
    /// With --path, creates a new-style pod rooted in the given directory
    /// (equivalent to `orqa init`, but requires an explicit slug).
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
    /// Pause all wake-loop runs for a pod.
    Sleep(SlugArgs),
    /// Clear a pod sleep marker.
    Wake(PodWakeArgs),
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
pub(crate) struct MailCommand {
    #[command(subcommand)]
    pub(crate) command: MailSubcommand,
}

#[derive(Debug, Args)]
pub(crate) struct TaskCommand {
    #[command(subcommand)]
    pub(crate) command: TaskSubcommand,
}

#[derive(Debug, Args)]
#[command(subcommand_required = true, arg_required_else_help = true)]
pub(crate) struct OpsCommand {
    #[command(subcommand)]
    pub(crate) command: OpsSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum OpsSubcommand {
    /// Generate a Markdown report of pods, tasks, and mail.
    Report(OpsReportArgs),
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

#[derive(Debug, Subcommand)]
pub(crate) enum MailSubcommand {
    /// Print the mail directory for a fin.
    Home(FinRefArgs),
    /// Send a pod-local message.
    Send(SendMailArgs),
    /// List messages for a fin.
    List(MailListArgs),
    /// Read a message for a fin.
    Read(MailMessageArgs),
    /// Mark an unread message as done.
    Done(MailMessageArgs),
    /// Delete a message.
    Delete(MailMessageArgs),
    /// List unread messages for a fin.
    Unread(FinRefArgs),
}

#[derive(Debug, Subcommand)]
pub(crate) enum TaskSubcommand {
    /// Print the task directory for a fin.
    Home(FinRefArgs),
    /// Assign a pod-local task.
    Send(SendTaskArgs),
    /// List tasks for a fin.
    List(TaskListArgs),
    /// Read a task for a fin.
    Read(MailMessageArgs),
    /// Mark an open task as done.
    Done(MailMessageArgs),
    /// Delete a task.
    Delete(MailMessageArgs),
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
    /// Pod slug.
    pub(crate) pod: String,
    /// Fin slug inside the pod.
    pub(crate) fin: String,
    /// Fin role text, @file path, or - for stdin.
    #[arg(value_name = "PROMPT|@FILE|-")]
    pub(crate) role: String,
}

#[derive(Debug, Args)]
pub(crate) struct FinRefArgs {
    /// Pod slug.
    pub(crate) pod: String,
    /// Fin slug inside the pod.
    pub(crate) fin: String,
}

#[derive(Debug, Args)]
pub(crate) struct FinStatusArgs {
    /// Pod slug.
    pub(crate) pod: String,
    /// Fin slug inside the pod.
    pub(crate) fin: String,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Debug, Args)]
pub(crate) struct FinRunsArgs {
    /// Pod slug.
    pub(crate) pod: String,
    /// Fin slug inside the pod.
    pub(crate) fin: String,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Debug, Args)]
pub(crate) struct FinRunReadArgs {
    /// Pod slug.
    pub(crate) pod: String,
    /// Fin slug inside the pod.
    pub(crate) fin: String,
    /// Run id. Defaults to latest.
    pub(crate) run: Option<String>,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Debug, Args)]
pub(crate) struct FinTailArgs {
    /// Pod slug.
    pub(crate) pod: String,
    /// Fin slug inside the pod.
    pub(crate) fin: String,
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
pub(crate) struct FinWakeArgs {
    /// Pod slug.
    pub(crate) pod: String,
    /// Fin slug inside the pod.
    pub(crate) fin: String,
    /// Required to clear sleep state.
    #[arg(long)]
    pub(crate) force: bool,
}

#[derive(Debug, Args)]
pub(crate) struct ExecArgs {
    /// Pod slug.
    pub(crate) pod: String,
    /// Fin slug inside the pod.
    pub(crate) fin: String,
    /// Arguments used to build the backend prompt.
    #[arg(last = true)]
    pub(crate) args: Vec<OsString>,
}

#[derive(Debug, Args)]
pub(crate) struct ChatArgs {
    /// Pod slug.
    pub(crate) pod: String,
    /// Fin slug inside the pod.
    pub(crate) fin: String,
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
pub(crate) struct SendMailArgs {
    /// Sender address. Defaults to ORQA_FIN@ORQA_POD.orqa.
    #[arg(long)]
    pub(crate) from: Option<String>,
    /// Recipient address, such as bob-jones or bob-jones@sample-pod.orqa.
    #[arg(long)]
    pub(crate) to: String,
    /// Message subject.
    #[arg(long, default_value = "(no subject)")]
    pub(crate) subject: String,
    /// Message body. Reads stdin when omitted.
    pub(crate) body: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct SendTaskArgs {
    /// Sender address. Defaults to ORQA_FIN@ORQA_POD.orqa.
    #[arg(long)]
    pub(crate) from: Option<String>,
    /// Assignee address, such as bob-jones or bob-jones@sample-pod.orqa.
    #[arg(long)]
    pub(crate) to: String,
    /// Task title.
    #[arg(long)]
    pub(crate) title: Option<String>,
    /// Task body. Reads stdin when omitted.
    pub(crate) body: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct MailListArgs {
    /// Pod slug. Defaults to ORQA_POD.
    #[arg(long)]
    pub(crate) pod: Option<String>,
    /// Fin slug. Defaults to ORQA_FIN.
    #[arg(long)]
    pub(crate) fin: Option<String>,
    /// Include done items from cur.
    #[arg(long)]
    pub(crate) all: bool,
}

#[derive(Debug, Args)]
pub(crate) struct TaskListArgs {
    /// Pod slug. Defaults to ORQA_POD.
    #[arg(long)]
    pub(crate) pod: Option<String>,
    /// Fin slug. Defaults to ORQA_FIN.
    #[arg(long)]
    pub(crate) fin: Option<String>,
    /// Include done items from cur.
    #[arg(long)]
    pub(crate) all: bool,
    /// Filter by status front matter.
    #[arg(long)]
    pub(crate) status: Option<String>,
    /// Filter by priority front matter.
    #[arg(long)]
    pub(crate) priority: Option<String>,
    /// Filter by kind front matter.
    #[arg(long)]
    pub(crate) kind: Option<String>,
    /// Filter by arbitrary front matter field, as key=value.
    #[arg(long = "field")]
    pub(crate) fields: Vec<String>,
    /// Sort by a front matter key, or by state/id.
    #[arg(long)]
    pub(crate) sort: Option<String>,
    /// Reverse sort order.
    #[arg(long)]
    pub(crate) reverse: bool,
}

#[derive(Debug, Args)]
pub(crate) struct MailMessageArgs {
    /// Pod slug. Defaults to ORQA_POD.
    #[arg(long)]
    pub(crate) pod: Option<String>,
    /// Fin slug. Defaults to ORQA_FIN.
    #[arg(long)]
    pub(crate) fin: Option<String>,
    /// Message id, filename, or path.
    pub(crate) message: String,
}

#[derive(Debug, Args, Default)]
pub(crate) struct OpsReportArgs {
    /// Include only records at or after this time. Accepts Unix seconds or relative durations like 30m, 2h, 1d.
    #[arg(long)]
    pub(crate) since: Option<String>,
}
use std::{ffi::OsString, path::PathBuf};

use clap::{Args, Parser, Subcommand};
