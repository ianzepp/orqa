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
    /// Create or inspect pods.
    Pod(PodCommand),
    /// Create or run fins inside a pod.
    Fin(FinCommand),
    /// Mail helpers for pod-local fin messages.
    Mail(MailCommand),
    /// Task helpers for pod-local work items.
    Task(TaskCommand),
    /// Run the wake loop for a pod.
    Loop(LoopArgs),
    /// Show the wake plan for a pod without running fins.
    Plan(PlanArgs),
    /// Manage the background wake-loop service.
    Service(ServiceCommand),
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
    /// Create a pod home directory.
    Create(SlugArgs),
    /// Print the home directory for a pod.
    Home(SlugArgs),
    /// Print pod runtime status.
    Status(PodStatusArgs),
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
    Create(FinRefArgs),
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
    /// Run a fin through the configured framework.
    Run(RunArgs),
    /// Internal supervised runner used by wake loops.
    #[command(hide = true)]
    Supervise(RunArgs),
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
pub(crate) struct ServiceCommand {
    #[command(subcommand)]
    pub(crate) command: ServiceSubcommand,
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

#[derive(Debug, Subcommand)]
pub(crate) enum ServiceSubcommand {
    /// Install a platform service for ORQA_HOME.
    Install(ServiceInstallArgs),
    /// Uninstall the platform service for ORQA_HOME.
    Uninstall,
    /// Start the service for ORQA_HOME.
    Start,
    /// Stop the service for ORQA_HOME.
    Stop,
    /// Print platform service status for ORQA_HOME.
    Status,
    /// Run the foreground service loop for debugging.
    Run(ServiceRunArgs),
}

#[derive(Debug, Args)]
pub(crate) struct LoopArgs {
    /// Pod slug.
    pub(crate) pod: String,
    /// Ignore pod and fin sleep markers for this scan.
    #[arg(long)]
    pub(crate) force: bool,
    /// Print wake decisions without running fins.
    #[arg(long)]
    pub(crate) dry_run: bool,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(crate) json: bool,
    /// Framework executable.
    #[arg(long)]
    pub(crate) framework: Option<OsString>,
    /// Arguments passed to the framework.
    #[arg(last = true)]
    pub(crate) args: Vec<OsString>,
}

#[derive(Debug, Args)]
pub(crate) struct PlanArgs {
    /// Pod slug.
    pub(crate) pod: String,
    /// Ignore pod and fin sleep markers while planning.
    #[arg(long)]
    pub(crate) force: bool,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Debug, Args)]
pub(crate) struct ServiceInstallArgs {
    /// Seconds between wake scans.
    #[arg(long, default_value_t = 60)]
    pub(crate) interval: u64,
    /// Ignore pod and fin sleep markers for each scan.
    #[arg(long)]
    pub(crate) force: bool,
    /// Framework executable override.
    #[arg(long)]
    pub(crate) framework: Option<OsString>,
    /// Arguments passed to each wake-loop scan.
    #[arg(last = true)]
    pub(crate) args: Vec<OsString>,
}

#[derive(Debug, Args)]
pub(crate) struct ServiceRunArgs {
    /// Seconds between wake scans.
    #[arg(long, default_value_t = 60)]
    pub(crate) interval: u64,
    /// Ignore pod and fin sleep markers for each scan.
    #[arg(long)]
    pub(crate) force: bool,
    /// Framework executable override.
    #[arg(long)]
    pub(crate) framework: Option<OsString>,
    /// Arguments passed to each wake-loop scan.
    #[arg(last = true)]
    pub(crate) args: Vec<OsString>,
}

#[derive(Debug, Args)]
pub(crate) struct SlugArgs {
    /// Pod slug.
    pub(crate) slug: String,
}

#[derive(Debug, Args)]
pub(crate) struct FinListArgs {
    /// Pod slug. Defaults to ORQA_POD.
    pub(crate) pod: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct FinRefArgs {
    /// Pod slug.
    pub(crate) pod: String,
    /// Fin slug inside the pod.
    pub(crate) fin: String,
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
pub(crate) struct RunArgs {
    /// Pod slug.
    pub(crate) pod: String,
    /// Fin slug inside the pod.
    pub(crate) fin: String,
    /// Framework executable.
    #[arg(long)]
    pub(crate) framework: Option<OsString>,
    /// Arguments passed to the framework.
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
use std::{ffi::OsString, path::PathBuf};

use clap::{Args, Parser, Subcommand};
