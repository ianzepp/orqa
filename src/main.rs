use std::{
    env,
    ffi::OsString,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, ExitCode, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "orqa",
    version,
    about = "Fan out work to background fins",
    long_about = None
)]
struct Cli {
    /// Override ORQA_HOME for this command.
    #[arg(long, global = true, value_name = "DIR")]
    home: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show basic runtime information.
    Doctor,
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
}

#[derive(Debug, Args)]
struct PodCommand {
    #[command(subcommand)]
    command: PodSubcommand,
}

#[derive(Debug, Subcommand)]
enum PodSubcommand {
    /// Create a pod home directory.
    Create(SlugArgs),
    /// Print the home directory for a pod.
    Home(SlugArgs),
    /// Pause all wake-loop runs for a pod.
    Sleep(SlugArgs),
    /// Clear a pod sleep marker.
    Wake(PodWakeArgs),
}

#[derive(Debug, Args)]
struct FinCommand {
    #[command(subcommand)]
    command: FinSubcommand,
}

#[derive(Debug, Subcommand)]
enum FinSubcommand {
    /// Create a fin inside a pod.
    Create(FinRefArgs),
    /// Print the home directory for a fin.
    Home(FinRefArgs),
    /// Pause wake-loop runs for a fin.
    Sleep(FinRefArgs),
    /// Clear a fin sleep marker.
    Wake(FinWakeArgs),
    /// Run a fin through the configured framework.
    Run(RunArgs),
}

#[derive(Debug, Args)]
struct MailCommand {
    #[command(subcommand)]
    command: MailSubcommand,
}

#[derive(Debug, Args)]
struct TaskCommand {
    #[command(subcommand)]
    command: TaskSubcommand,
}

#[derive(Debug, Subcommand)]
enum MailSubcommand {
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
enum TaskSubcommand {
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
struct LoopArgs {
    /// Pod slug.
    pod: String,
    /// Ignore pod and fin sleep markers for this scan.
    #[arg(long)]
    force: bool,
    /// Framework executable.
    #[arg(long, default_value = "codex")]
    framework: OsString,
    /// Arguments passed to the framework.
    #[arg(last = true)]
    args: Vec<OsString>,
}

#[derive(Debug, Args)]
struct SlugArgs {
    /// Pod slug.
    slug: String,
}

#[derive(Debug, Args)]
struct FinRefArgs {
    /// Pod slug.
    pod: String,
    /// Fin slug inside the pod.
    fin: String,
}

#[derive(Debug, Args)]
struct PodWakeArgs {
    /// Pod slug.
    slug: String,
    /// Required to clear sleep state.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Args)]
struct FinWakeArgs {
    /// Pod slug.
    pod: String,
    /// Fin slug inside the pod.
    fin: String,
    /// Required to clear sleep state.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Args)]
struct RunArgs {
    /// Pod slug.
    pod: String,
    /// Fin slug inside the pod.
    fin: String,
    /// Framework executable.
    #[arg(long, default_value = "codex")]
    framework: OsString,
    /// Arguments passed to the framework.
    #[arg(last = true)]
    args: Vec<OsString>,
}

#[derive(Debug, Args)]
struct SendMailArgs {
    /// Sender address. Defaults to ORQA_FIN@ORQA_POD.orqa.
    #[arg(long)]
    from: Option<String>,
    /// Recipient address, such as bob-jones or bob-jones@sample-pod.orqa.
    #[arg(long)]
    to: String,
    /// Message subject.
    #[arg(long, default_value = "(no subject)")]
    subject: String,
    /// Message body. Reads stdin when omitted.
    body: Option<String>,
}

#[derive(Debug, Args)]
struct SendTaskArgs {
    /// Sender address. Defaults to ORQA_FIN@ORQA_POD.orqa.
    #[arg(long)]
    from: Option<String>,
    /// Assignee address, such as bob-jones or bob-jones@sample-pod.orqa.
    #[arg(long)]
    to: String,
    /// Task title.
    #[arg(long)]
    title: Option<String>,
    /// Task body. Reads stdin when omitted.
    body: Option<String>,
}

#[derive(Debug, Args)]
struct MailListArgs {
    /// Pod slug. Defaults to ORQA_POD.
    #[arg(long)]
    pod: Option<String>,
    /// Fin slug. Defaults to ORQA_FIN.
    #[arg(long)]
    fin: Option<String>,
    /// Include done items from cur.
    #[arg(long)]
    all: bool,
}

#[derive(Debug, Args)]
struct TaskListArgs {
    /// Pod slug. Defaults to ORQA_POD.
    #[arg(long)]
    pod: Option<String>,
    /// Fin slug. Defaults to ORQA_FIN.
    #[arg(long)]
    fin: Option<String>,
    /// Include done items from cur.
    #[arg(long)]
    all: bool,
    /// Filter by status front matter.
    #[arg(long)]
    status: Option<String>,
    /// Filter by priority front matter.
    #[arg(long)]
    priority: Option<String>,
    /// Filter by kind front matter.
    #[arg(long)]
    kind: Option<String>,
    /// Filter by arbitrary front matter field, as key=value.
    #[arg(long = "field")]
    fields: Vec<String>,
    /// Sort by a front matter key, or by state/id.
    #[arg(long)]
    sort: Option<String>,
    /// Reverse sort order.
    #[arg(long)]
    reverse: bool,
}

#[derive(Debug, Args)]
struct MailMessageArgs {
    /// Pod slug. Defaults to ORQA_POD.
    #[arg(long)]
    pod: Option<String>,
    /// Fin slug. Defaults to ORQA_FIN.
    #[arg(long)]
    fin: Option<String>,
    /// Message id, filename, or path.
    message: String,
}

struct Orqa {
    home: PathBuf,
}

static MAIL_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn main() -> ExitCode {
    let cli = Cli::parse();
    let orqa = Orqa::new(cli.home);

    match run(&orqa, cli.command.unwrap_or(Command::Doctor)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("orqa: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(orqa: &Orqa, command: Command) -> Result<(), String> {
    match command {
        Command::Doctor => doctor(orqa),
        Command::Pod(command) => pod(orqa, command),
        Command::Fin(command) => fin(orqa, command),
        Command::Mail(command) => mail(orqa, command),
        Command::Task(command) => task(orqa, command),
        Command::Loop(args) => loop_pod(orqa, args),
    }
}

fn doctor(orqa: &Orqa) -> Result<(), String> {
    println!("orqa is installed and ready.");
    println!("orqa_home={}", orqa.home.display());
    Ok(())
}

fn pod(orqa: &Orqa, command: PodCommand) -> Result<(), String> {
    match command.command {
        PodSubcommand::Create(args) => {
            let pod = PodRef::new(&args.slug)?;
            let home = orqa.pod_home(&pod);
            fs::create_dir_all(home.join("fins")).map_err(|error| {
                format!("failed to create pod directory {}: {error}", home.display())
            })?;
            write_if_missing(&home.join("pod.txt"), &format!("slug={}\n", pod.slug))?;
            write_if_missing(&home.join("pod.toml"), &pod_config_template(&pod))?;
            println!("{}", home.display());
            Ok(())
        }
        PodSubcommand::Home(args) => {
            let pod = PodRef::new(&args.slug)?;
            println!("{}", orqa.pod_home(&pod).display());
            Ok(())
        }
        PodSubcommand::Sleep(args) => {
            let pod = PodRef::new(&args.slug)?;
            write_sleep_marker(&orqa.pod_sleep_path(&pod))?;
            println!("sleep {}", pod.slug);
            Ok(())
        }
        PodSubcommand::Wake(args) => {
            if !args.force {
                return Err("pod wake requires --force".to_string());
            }
            let pod = PodRef::new(&args.slug)?;
            remove_sleep_marker(&orqa.pod_sleep_path(&pod))?;
            println!("wake {}", pod.slug);
            Ok(())
        }
    }
}

fn fin(orqa: &Orqa, command: FinCommand) -> Result<(), String> {
    match command.command {
        FinSubcommand::Create(args) => {
            let fin = FinRef::new(&args.pod, &args.fin)?;
            let home = orqa.fin_home(&fin);
            fs::create_dir_all(home.join(".codex")).map_err(|error| {
                format!("failed to create fin directory {}: {error}", home.display())
            })?;
            ensure_maildir(&orqa.mail_home(&fin))?;
            ensure_maildir(&orqa.task_home(&fin))?;
            write_if_missing(&home.join("fin.txt"), &format!("slug={}\n", fin.fin))?;
            write_if_missing(&home.join("fin.toml"), &fin_config_template(&fin))?;
            println!("{}", home.display());
            Ok(())
        }
        FinSubcommand::Home(args) => {
            let fin = FinRef::new(&args.pod, &args.fin)?;
            println!("{}", orqa.fin_home(&fin).display());
            Ok(())
        }
        FinSubcommand::Sleep(args) => {
            let fin = FinRef::new(&args.pod, &args.fin)?;
            write_sleep_marker(&orqa.fin_sleep_path(&fin))?;
            println!("sleep {}", fin.label());
            Ok(())
        }
        FinSubcommand::Wake(args) => {
            if !args.force {
                return Err("fin wake requires --force".to_string());
            }
            let fin = FinRef::new(&args.pod, &args.fin)?;
            remove_sleep_marker(&orqa.fin_sleep_path(&fin))?;
            println!("wake {}", fin.label());
            Ok(())
        }
        FinSubcommand::Run(args) => run_fin(orqa, args),
    }
}

fn mail(orqa: &Orqa, command: MailCommand) -> Result<(), String> {
    match command.command {
        MailSubcommand::Home(args) => {
            let fin = FinRef::new(&args.pod, &args.fin)?;
            println!("{}", orqa.mail_home(&fin).display());
            Ok(())
        }
        MailSubcommand::Send(args) => send_mail(orqa, args),
        MailSubcommand::List(args) => list_mail(orqa, args),
        MailSubcommand::Read(args) => read_mail(orqa, args),
        MailSubcommand::Done(args) => done_mail(orqa, args),
        MailSubcommand::Delete(args) => delete_mail(orqa, args),
        MailSubcommand::Unread(args) => unread_mail(orqa, args),
    }
}

fn task(orqa: &Orqa, command: TaskCommand) -> Result<(), String> {
    match command.command {
        TaskSubcommand::Home(args) => {
            let fin = FinRef::new(&args.pod, &args.fin)?;
            println!("{}", orqa.task_home(&fin).display());
            Ok(())
        }
        TaskSubcommand::Send(args) => send_task(orqa, args),
        TaskSubcommand::List(args) => list_tasks(orqa, args),
        TaskSubcommand::Read(args) => read_item(orqa, args, ItemKind::Task),
        TaskSubcommand::Done(args) => done_item(orqa, args, ItemKind::Task),
        TaskSubcommand::Delete(args) => delete_item(orqa, args, ItemKind::Task),
    }
}

fn loop_pod(orqa: &Orqa, args: LoopArgs) -> Result<(), String> {
    let pod = PodRef::new(&args.pod)?;
    if !args.force && orqa.pod_sleep_path(&pod).exists() {
        println!("skip {} sleeping=true", pod.slug);
        return Ok(());
    }

    let fins_dir = orqa.pod_home(&pod).join("fins");
    let fins = fs::read_dir(&fins_dir).map_err(|error| {
        format!(
            "failed to read fins directory {}: {error}",
            fins_dir.display()
        )
    })?;

    for entry in fins {
        let entry = entry.map_err(|error| format!("failed to read fin directory: {error}"))?;
        if !entry.path().is_dir() {
            continue;
        }

        let fin_slug = entry.file_name().to_string_lossy().to_string();
        let fin = FinRef::new(&pod.slug, &fin_slug)?;
        if !args.force && orqa.fin_sleep_path(&fin).exists() {
            println!("skip {} sleeping=true", fin.label());
            continue;
        }

        let unread_mail = unread_count(&orqa.mail_home(&fin))?;
        let open_tasks = unread_count(&orqa.task_home(&fin))?;

        if unread_mail > 0 || open_tasks > 0 {
            let wake = Wake {
                unread_mail,
                open_tasks,
            };
            wake_fin(orqa, &fin, &args.framework, &args.args, wake)?;
        }
    }

    Ok(())
}

fn run_fin(orqa: &Orqa, args: RunArgs) -> Result<(), String> {
    let fin = FinRef::new(&args.pod, &args.fin)?;
    run_fin_foreground(orqa, &fin, &args.framework, &args.args)
}

fn run_fin_foreground(
    orqa: &Orqa,
    fin: &FinRef,
    framework: &OsString,
    args: &[OsString],
) -> Result<(), String> {
    if let Some(lock) = FinLock::try_existing(orqa, fin)? {
        if lock.is_live() {
            return Err(format!(
                "fin {} is already running as pid {}",
                fin.label(),
                lock.pid
            ));
        }
        lock.remove()?;
    }

    let home = orqa.fin_home(fin);
    let codex_home = home.join(".codex");

    fs::create_dir_all(&codex_home).map_err(|error| {
        format!(
            "failed to create fin codex home {}: {error}",
            codex_home.display()
        )
    })?;

    let mut child = ProcessCommand::new(framework)
        .env("ORQA_HOME", &orqa.home)
        .env("ORQA_POD", &fin.pod)
        .env("ORQA_FIN", &fin.fin)
        .env("CODEX_HOME", &codex_home)
        .args(args)
        .spawn()
        .map_err(|error| format!("failed to run {framework:?}: {error}"))?;
    let lock = FinLock::write(orqa, fin, child.id(), framework)?;
    let status = child
        .wait()
        .map_err(|error| format!("failed to wait for {framework:?}: {error}"));
    lock.release();
    let status = status?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{framework:?} exited with {}",
            status
                .code()
                .map_or_else(|| "signal".to_string(), |code| code.to_string())
        ))
    }
}

#[derive(Clone, Copy)]
struct Wake {
    unread_mail: usize,
    open_tasks: usize,
}

fn wake_fin(
    orqa: &Orqa,
    fin: &FinRef,
    framework: &OsString,
    args: &[OsString],
    wake: Wake,
) -> Result<(), String> {
    match FinLock::try_existing(orqa, fin)? {
        Some(lock) if lock.is_live() => {
            println!(
                "skip {} pid={} unread_mail={} open_tasks={}",
                fin.label(),
                lock.pid,
                wake.unread_mail,
                wake.open_tasks
            );
            Ok(())
        }
        Some(lock) => {
            lock.remove()?;
            spawn_wake_fin(orqa, fin, framework, args, wake)
        }
        None => spawn_wake_fin(orqa, fin, framework, args, wake),
    }
}

fn spawn_wake_fin(
    orqa: &Orqa,
    fin: &FinRef,
    framework: &OsString,
    args: &[OsString],
    wake: Wake,
) -> Result<(), String> {
    let home = orqa.fin_home(fin);
    let codex_home = home.join(".codex");
    fs::create_dir_all(&codex_home).map_err(|error| {
        format!(
            "failed to create fin codex home {}: {error}",
            codex_home.display()
        )
    })?;

    let child = ProcessCommand::new(framework)
        .env("ORQA_HOME", &orqa.home)
        .env("ORQA_POD", &fin.pod)
        .env("ORQA_FIN", &fin.fin)
        .env("CODEX_HOME", &codex_home)
        .args(args)
        .spawn()
        .map_err(|error| format!("failed to spawn {framework:?}: {error}"))?;
    let pid = child.id();

    FinLock::write(orqa, fin, pid, framework)?;
    println!(
        "wake {} pid={} unread_mail={} open_tasks={}",
        fin.label(),
        pid,
        wake.unread_mail,
        wake.open_tasks
    );

    Ok(())
}

struct FinLock {
    path: PathBuf,
    pid: u32,
}

impl FinLock {
    fn try_existing(orqa: &Orqa, fin: &FinRef) -> Result<Option<Self>, String> {
        let path = orqa.lock_path(fin);
        if !path.exists() {
            return Ok(None);
        }

        let contents = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read lock {}: {error}", path.display()))?;
        let pid = lock_pid(&contents)
            .ok_or_else(|| format!("lock {} does not contain a valid pid", path.display()))?;

        Ok(Some(Self { path, pid }))
    }

    fn write(orqa: &Orqa, fin: &FinRef, pid: u32, framework: &OsString) -> Result<Self, String> {
        let path = orqa.lock_path(fin);
        let parent = path
            .parent()
            .ok_or_else(|| format!("lock path has no parent: {}", path.display()))?;
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create lock directory {}: {error}",
                parent.display()
            )
        })?;

        let contents = format!(
            "pid={pid}\npod={}\nfin={}\nframework={:?}\n",
            fin.pod, fin.fin, framework
        );
        fs::write(&path, contents)
            .map_err(|error| format!("failed to write lock {}: {error}", path.display()))?;

        Ok(Self { path, pid })
    }

    fn is_live(&self) -> bool {
        process_is_alive(self.pid)
    }

    fn remove(&self) -> Result<(), String> {
        if self.path.exists() {
            fs::remove_file(&self.path).map_err(|error| {
                format!("failed to remove lock {}: {error}", self.path.display())
            })?;
        }

        Ok(())
    }

    fn release(self) {
        let _ = self.remove();
    }
}

fn lock_pid(contents: &str) -> Option<u32> {
    contents
        .lines()
        .find_map(|line| line.strip_prefix("pid=")?.parse::<u32>().ok())
}

#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    ProcessCommand::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(not(unix))]
fn process_is_alive(_pid: u32) -> bool {
    false
}

fn send_mail(orqa: &Orqa, args: SendMailArgs) -> Result<(), String> {
    let from = resolve_sender(args.from.as_deref())?;
    let to = resolve_address(&args.to, Some(&from.pod))?;

    if from.pod != to.pod {
        return Err(format!(
            "cross-pod mail is not supported: {} -> {}",
            from.label(),
            to.label()
        ));
    }

    let from_fin = FinRef::new(&from.pod, &from.fin)?;
    let to_fin = FinRef::new(&to.pod, &to.fin)?;
    let mail_home = orqa.mail_home(&to_fin);
    ensure_maildir(&mail_home)?;

    let body = match args.body {
        Some(body) => body,
        None => read_stdin()?,
    };

    let message = format!(
        "From: {}\nTo: {}\nSubject: {}\n\n{}\n",
        from.label(),
        to.label(),
        args.subject,
        body
    );
    let path = deliver_mail(&mail_home, &message)?;

    println!("{}", path.display());
    println!("queued wake for {}", to_fin.label());

    let _ = from_fin;
    Ok(())
}

fn unread_mail(orqa: &Orqa, args: FinRefArgs) -> Result<(), String> {
    let fin = FinRef::new(&args.pod, &args.fin)?;
    let new_dir = orqa.mail_home(&fin).join("new");

    for path in sorted_files(&new_dir)? {
        println!("{}", path.display());
    }

    Ok(())
}

fn list_mail(orqa: &Orqa, args: MailListArgs) -> Result<(), String> {
    list_items(orqa, args, ItemKind::Mail)
}

fn read_mail(orqa: &Orqa, args: MailMessageArgs) -> Result<(), String> {
    read_item(orqa, args, ItemKind::Mail)
}

fn done_mail(orqa: &Orqa, args: MailMessageArgs) -> Result<(), String> {
    done_item(orqa, args, ItemKind::Mail)
}

fn delete_mail(orqa: &Orqa, args: MailMessageArgs) -> Result<(), String> {
    delete_item(orqa, args, ItemKind::Mail)
}

fn send_task(orqa: &Orqa, args: SendTaskArgs) -> Result<(), String> {
    let from = resolve_sender(args.from.as_deref())?;
    let to = resolve_address(&args.to, Some(&from.pod))?;

    if from.pod != to.pod {
        return Err(format!(
            "cross-pod tasks are not supported: {} -> {}",
            from.label(),
            to.label()
        ));
    }

    let to_fin = FinRef::new(&to.pod, &to.fin)?;
    let task_home = orqa.task_home(&to_fin);
    ensure_maildir(&task_home)?;

    let body = match args.body {
        Some(body) => body,
        None => read_stdin()?,
    };
    let task = canonical_task_body(&from, &to, args.title.as_deref(), &body);
    let path = deliver_mail(&task_home, &task)?;

    println!("{}", path.display());
    println!("queued task for {}", to_fin.label());
    Ok(())
}

fn list_tasks(orqa: &Orqa, args: TaskListArgs) -> Result<(), String> {
    let fin = resolve_fin(args.pod.as_deref(), args.fin.as_deref())?;
    let home = orqa.task_home(&fin);
    let filters = TaskFilters::new(&args)?;
    let mut tasks = collect_tasks(&home, args.all)?;

    tasks.retain(|task| filters.matches(task));
    sort_tasks(&mut tasks, args.sort.as_deref(), args.reverse);

    for task in tasks {
        println!("{}", task.format());
    }

    Ok(())
}

#[derive(Clone, Copy)]
enum ItemKind {
    Mail,
    Task,
}

impl ItemKind {
    fn home(self, orqa: &Orqa, fin: &FinRef) -> PathBuf {
        match self {
            Self::Mail => orqa.mail_home(fin),
            Self::Task => orqa.task_home(fin),
        }
    }

    fn title_header(self) -> &'static str {
        match self {
            Self::Mail => "Subject: ",
            Self::Task => "title: ",
        }
    }
}

fn list_items(orqa: &Orqa, args: MailListArgs, kind: ItemKind) -> Result<(), String> {
    let fin = resolve_fin(args.pod.as_deref(), args.fin.as_deref())?;
    let home = kind.home(orqa, &fin);

    for path in sorted_files(&home.join("new"))? {
        println!("new {} {}", message_id(&path)?, message_title(&path, kind)?);
    }

    if args.all {
        for path in sorted_files(&home.join("cur"))? {
            println!("cur {} {}", message_id(&path)?, message_title(&path, kind)?);
        }
    }

    Ok(())
}

struct TaskFilters {
    fields: Vec<(String, String)>,
}

impl TaskFilters {
    fn new(args: &TaskListArgs) -> Result<Self, String> {
        let mut fields = Vec::new();

        if let Some(status) = &args.status {
            fields.push(("status".to_string(), status.to_string()));
        }
        if let Some(priority) = &args.priority {
            fields.push(("priority".to_string(), priority.to_string()));
        }
        if let Some(kind) = &args.kind {
            fields.push(("kind".to_string(), kind.to_string()));
        }
        for field in &args.fields {
            let (key, value) = field
                .split_once('=')
                .ok_or_else(|| format!("invalid --field {field:?}; expected key=value"))?;
            if key.trim().is_empty() {
                return Err(format!("invalid --field {field:?}; key cannot be empty"));
            }
            fields.push((key.trim().to_string(), value.trim().to_string()));
        }

        Ok(Self { fields })
    }

    fn matches(&self, task: &TaskSummary) -> bool {
        self.fields.iter().all(|(key, value)| {
            task.field(key)
                .is_some_and(|task_value| task_value == value)
        })
    }
}

struct TaskSummary {
    state: String,
    id: String,
    fields: Vec<(String, String)>,
}

impl TaskSummary {
    fn field(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(field_key, _)| field_key == key)
            .map(|(_, value)| value.as_str())
    }

    fn sort_value(&self, key: &str) -> String {
        match key {
            "state" => self.state.clone(),
            "id" => self.id.clone(),
            "priority" => priority_sort_value(self.field("priority").unwrap_or("")),
            key => self.field(key).unwrap_or("").to_string(),
        }
    }

    fn format(&self) -> String {
        let mut line = format!("{} {}", self.state, self.id);
        for key in ["priority", "status", "kind", "title"] {
            if let Some(value) = self.field(key) {
                line.push(' ');
                line.push_str(key);
                line.push('=');
                line.push_str(&quote_value(value));
            }
        }
        line
    }
}

fn collect_tasks(task_home: &Path, include_done: bool) -> Result<Vec<TaskSummary>, String> {
    let mut tasks = Vec::new();

    collect_tasks_in_state(task_home, "new", &mut tasks)?;
    if include_done {
        collect_tasks_in_state(task_home, "cur", &mut tasks)?;
    }

    Ok(tasks)
}

fn collect_tasks_in_state(
    task_home: &Path,
    state: &str,
    tasks: &mut Vec<TaskSummary>,
) -> Result<(), String> {
    for path in sorted_files(&task_home.join(state))? {
        let body = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let (fields, _) = split_front_matter(&body);
        tasks.push(TaskSummary {
            state: state.to_string(),
            id: message_id(&path)?,
            fields,
        });
    }

    Ok(())
}

fn sort_tasks(tasks: &mut [TaskSummary], sort: Option<&str>, reverse: bool) {
    let sort = sort.unwrap_or("id");
    tasks.sort_by(|left, right| {
        left.sort_value(sort)
            .cmp(&right.sort_value(sort))
            .then_with(|| left.id.cmp(&right.id))
    });

    if reverse {
        tasks.reverse();
    }
}

fn priority_sort_value(priority: &str) -> String {
    let rank = match priority {
        "critical" | "urgent" => 0,
        "high" => 1,
        "normal" | "medium" => 2,
        "low" => 3,
        _ => 9,
    };

    format!("{rank}:{priority}")
}

fn quote_value(value: &str) -> String {
    if value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric()
            || matches!(byte, b'-' | b'_' | b'.' | b'/' | b':' | b'[' | b']')
    }) {
        return value.to_string();
    }

    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn read_item(orqa: &Orqa, args: MailMessageArgs, kind: ItemKind) -> Result<(), String> {
    let fin = resolve_fin(args.pod.as_deref(), args.fin.as_deref())?;
    let path = resolve_message_path(&kind.home(orqa, &fin), &args.message)?;
    let message = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

    print!("{message}");
    Ok(())
}

fn done_item(orqa: &Orqa, args: MailMessageArgs, kind: ItemKind) -> Result<(), String> {
    let fin = resolve_fin(args.pod.as_deref(), args.fin.as_deref())?;
    let home = kind.home(orqa, &fin);
    let path = resolve_message_path(&home, &args.message)?;
    let id = message_id(&path)?;

    if mail_state(&home, &path)? == "cur" {
        println!("{}", path.display());
        return Ok(());
    }

    let done_path = home.join("cur").join(id);
    fs::rename(&path, &done_path).map_err(|error| {
        format!(
            "failed to mark item done {} -> {}: {error}",
            path.display(),
            done_path.display()
        )
    })?;

    println!("{}", done_path.display());
    Ok(())
}

fn delete_item(orqa: &Orqa, args: MailMessageArgs, kind: ItemKind) -> Result<(), String> {
    let fin = resolve_fin(args.pod.as_deref(), args.fin.as_deref())?;
    let path = resolve_message_path(&kind.home(orqa, &fin), &args.message)?;

    fs::remove_file(&path)
        .map_err(|error| format!("failed to delete item {}: {error}", path.display()))?;
    println!("deleted {}", path.display());
    Ok(())
}

fn canonical_task_body(
    from: &MailAddress,
    to: &MailAddress,
    title_arg: Option<&str>,
    body: &str,
) -> String {
    let (mut fields, description) = split_front_matter(body);
    let title = title_arg
        .map(str::to_string)
        .or_else(|| field_value(&fields, "title"))
        .unwrap_or_else(|| "(untitled task)".to_string());

    upsert_field(&mut fields, "from", &from.label());
    upsert_field(&mut fields, "to", &to.label());
    upsert_field(&mut fields, "title", &title);
    ensure_field(&mut fields, "priority", "normal");
    ensure_field(&mut fields, "status", "open");
    ensure_field(&mut fields, "kind", "need");
    ensure_field(&mut fields, "depends_on", "[]");

    let mut task = String::from("---\n");
    for (key, value) in fields {
        task.push_str(&key);
        task.push_str(": ");
        task.push_str(&value);
        task.push('\n');
    }
    task.push_str("---\n\n");
    task.push_str(description.trim());
    task.push('\n');

    task
}

fn split_front_matter(body: &str) -> (Vec<(String, String)>, &str) {
    let Some(rest) = body.strip_prefix("---\n") else {
        return (Vec::new(), body);
    };
    let Some((front_matter, description)) = rest.split_once("\n---") else {
        return (Vec::new(), body);
    };

    let description = description.strip_prefix('\n').unwrap_or(description);
    (parse_front_matter(front_matter), description)
}

fn parse_front_matter(front_matter: &str) -> Vec<(String, String)> {
    front_matter
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            let key = key.trim();
            if key.is_empty() {
                return None;
            }
            Some((key.to_string(), value.trim().to_string()))
        })
        .collect()
}

fn field_value(fields: &[(String, String)], key: &str) -> Option<String> {
    fields
        .iter()
        .find(|(existing_key, _)| existing_key == key)
        .map(|(_, value)| value.to_string())
}

fn ensure_field(fields: &mut Vec<(String, String)>, key: &str, value: &str) {
    if field_value(fields, key).is_none() {
        fields.push((key.to_string(), value.to_string()));
    }
}

fn upsert_field(fields: &mut Vec<(String, String)>, key: &str, value: &str) {
    if let Some((_, existing_value)) = fields
        .iter_mut()
        .find(|(existing_key, _)| existing_key == key)
    {
        *existing_value = value.to_string();
    } else {
        fields.push((key.to_string(), value.to_string()));
    }
}

fn ensure_maildir(mail_home: &Path) -> Result<(), String> {
    for dir in ["cur", "new", "tmp"] {
        fs::create_dir_all(mail_home.join(dir)).map_err(|error| {
            format!("failed to create maildir {}: {error}", mail_home.display())
        })?;
    }

    Ok(())
}

fn deliver_mail(mail_home: &Path, message: &str) -> Result<PathBuf, String> {
    let name = unique_mail_name()?;
    let tmp_path = mail_home.join("tmp").join(&name);
    let new_path = mail_home.join("new").join(&name);

    fs::write(&tmp_path, message)
        .map_err(|error| format!("failed to write mail {}: {error}", tmp_path.display()))?;
    fs::rename(&tmp_path, &new_path).map_err(|error| {
        format!(
            "failed to move mail into inbox {}: {error}",
            new_path.display()
        )
    })?;

    Ok(new_path)
}

fn unread_count(mail_home: &Path) -> Result<usize, String> {
    Ok(sorted_files(&mail_home.join("new"))?.len())
}

fn sorted_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    for entry in
        fs::read_dir(dir).map_err(|error| format!("failed to read {}: {error}", dir.display()))?
    {
        let entry =
            entry.map_err(|error| format!("failed to read {} entry: {error}", dir.display()))?;
        if entry.path().is_file() {
            paths.push(entry.path());
        }
    }

    paths.sort();
    Ok(paths)
}

fn resolve_fin(pod: Option<&str>, fin: Option<&str>) -> Result<FinRef, String> {
    let pod = match pod {
        Some(pod) => pod.to_string(),
        None => env::var("ORQA_POD")
            .map_err(|_| "missing pod; use --pod or run with ORQA_POD set".to_string())?,
    };
    let fin = match fin {
        Some(fin) => fin.to_string(),
        None => env::var("ORQA_FIN")
            .map_err(|_| "missing fin; use --fin or run with ORQA_FIN set".to_string())?,
    };

    FinRef::new(&pod, &fin)
}

fn resolve_message_path(mail_home: &Path, message: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(message);
    if path.exists() {
        return Ok(path);
    }

    for state in ["new", "cur"] {
        let candidate = mail_home.join(state).join(message);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(format!(
        "message {message:?} not found in {}",
        mail_home.display()
    ))
}

fn message_id(path: &Path) -> Result<String, String> {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .ok_or_else(|| format!("message path has no filename: {}", path.display()))
}

fn message_title(path: &Path, kind: ItemKind) -> Result<String, String> {
    let message = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

    for line in message.lines() {
        if let Some(title) = line.strip_prefix(kind.title_header()) {
            return Ok(title.to_string());
        }
    }

    Ok("(no title)".to_string())
}

fn mail_state(mail_home: &Path, path: &Path) -> Result<&'static str, String> {
    if path.starts_with(mail_home.join("new")) {
        Ok("new")
    } else if path.starts_with(mail_home.join("cur")) {
        Ok("cur")
    } else {
        Err(format!(
            "message {} is not inside {}",
            path.display(),
            mail_home.display()
        ))
    }
}

fn unique_mail_name() -> Result<String, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?;
    let counter = MAIL_COUNTER.fetch_add(1, Ordering::Relaxed);

    Ok(format!(
        "{}.{}.{}.orqa",
        now.as_micros(),
        std::process::id(),
        counter
    ))
}

fn read_stdin() -> Result<String, String> {
    let mut body = String::new();
    io::stdin()
        .read_to_string(&mut body)
        .map_err(|error| format!("failed to read stdin: {error}"))?;
    Ok(body)
}

fn resolve_sender(from: Option<&str>) -> Result<MailAddress, String> {
    match from {
        Some(from) => {
            let pod = env::var("ORQA_POD").ok();
            resolve_address(from, pod.as_deref())
        }
        None => {
            let pod = env::var("ORQA_POD").map_err(|_| {
                "missing sender; use --from fin@pod.orqa or run with ORQA_POD and ORQA_FIN set"
                    .to_string()
            })?;
            let fin = env::var("ORQA_FIN").map_err(|_| {
                "missing sender; use --from fin@pod.orqa or run with ORQA_POD and ORQA_FIN set"
                    .to_string()
            })?;

            resolve_address(&fin, Some(&pod))
        }
    }
}

fn resolve_address(address: &str, pod_hint: Option<&str>) -> Result<MailAddress, String> {
    if address.contains('@') {
        return MailAddress::parse(address);
    }

    let pod = match pod_hint {
        Some(pod) => pod.to_string(),
        None => env::var("ORQA_POD").map_err(|_| {
            format!(
                "bare address {address:?} needs ORQA_POD; use fin@pod.orqa or run with ORQA_POD set"
            )
        })?,
    };

    validate_slug(address)?;
    validate_slug(&pod)?;

    Ok(MailAddress {
        fin: address.to_string(),
        pod,
    })
}

fn write_if_missing(path: &Path, contents: &str) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }

    fs::write(path, contents)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn write_sleep_marker(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("sleep marker path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create sleep marker directory {}: {error}",
            parent.display()
        )
    })?;
    fs::write(path, "sleeping=true\n")
        .map_err(|error| format!("failed to write sleep marker {}: {error}", path.display()))
}

fn remove_sleep_marker(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path).map_err(|error| {
            format!("failed to remove sleep marker {}: {error}", path.display())
        })?;
    }

    Ok(())
}

fn pod_config_template(pod: &PodRef) -> String {
    format!(
        r#"# Orqa pod configuration.
#
# The pod owns backend definitions and the default backend used by fins that do
# not set their own override in fin.toml.
#
# Backend args are argv arrays, not shell strings. Supported template values
# include:
#   {{orqa_home}}, {{pod}}, {{pod_home}}, {{fin}}, {{fin_home}}, {{codex_home}},
#   {{mail_home}}, {{task_home}}, {{model}}, {{prompt}}

[pod]
slug = "{slug}"
default_backend = "codex"

# Codex is enabled by default. Adjust command/args here if the Codex CLI shape
# changes on this machine.
[backends.codex]
enabled = true
command = "codex"
args = ["{{prompt}}"]

[backends.codex.defaults]
model = "gpt-5.3-codex"

# Enable and edit these examples if this pod should allow additional backends.

# [backends.opencode]
# enabled = true
# command = "opencode"
# args = ["run", "--model", "{{model}}", "{{prompt}}"]
#
# [backends.opencode.defaults]
# model = "default"

# [backends.pi]
# enabled = true
# command = "pi"
# args = [
#     "exec",
#     "--home", "{{fin_home}}",
#     "--pod", "{{pod}}",
#     "--fin", "{{fin}}",
#     "{{prompt}}",
# ]

# [backends.custom]
# enabled = true
# command = "custom-fin-runner"
# args = ["{{prompt}}"]
"#,
        slug = pod.slug
    )
}

fn fin_config_template(fin: &FinRef) -> String {
    format!(
        r#"# Orqa fin configuration.
#
# By default a fin inherits the pod default backend from pod.toml.
# Uncomment fin.backend only when this fin should use a different enabled
# backend from its pod.

[fin]
slug = "{slug}"
# backend = "codex"

# Per-fin template values. These can be used by backend args in pod.toml.
[backend]
model = "gpt-5.3-codex"
"#,
        slug = fin.fin
    )
}

impl Orqa {
    fn new(home: Option<PathBuf>) -> Self {
        Self {
            home: home
                .or_else(|| env::var_os("ORQA_HOME").map(PathBuf::from))
                .unwrap_or_else(default_home),
        }
    }

    fn pod_home(&self, pod: &PodRef) -> PathBuf {
        self.home.join("pods").join(&pod.slug)
    }

    fn fin_home(&self, fin: &FinRef) -> PathBuf {
        self.home
            .join("pods")
            .join(&fin.pod)
            .join("fins")
            .join(&fin.fin)
    }

    fn mail_home(&self, fin: &FinRef) -> PathBuf {
        self.fin_home(fin).join("mail")
    }

    fn task_home(&self, fin: &FinRef) -> PathBuf {
        self.fin_home(fin).join("tasks")
    }

    fn lock_path(&self, fin: &FinRef) -> PathBuf {
        self.fin_home(fin).join("run.lock")
    }

    fn pod_sleep_path(&self, pod: &PodRef) -> PathBuf {
        self.pod_home(pod).join("sleep.lock")
    }

    fn fin_sleep_path(&self, fin: &FinRef) -> PathBuf {
        self.fin_home(fin).join("sleep.lock")
    }
}

struct PodRef {
    slug: String,
}

impl PodRef {
    fn new(slug: &str) -> Result<Self, String> {
        validate_slug(slug)?;
        Ok(Self {
            slug: slug.to_string(),
        })
    }
}

struct FinRef {
    pod: String,
    fin: String,
}

impl FinRef {
    fn new(pod: &str, fin: &str) -> Result<Self, String> {
        validate_slug(pod)?;
        validate_slug(fin)?;
        Ok(Self {
            pod: pod.to_string(),
            fin: fin.to_string(),
        })
    }

    fn label(&self) -> String {
        format!("{}/{}", self.pod, self.fin)
    }
}

struct MailAddress {
    fin: String,
    pod: String,
}

impl MailAddress {
    fn parse(address: &str) -> Result<Self, String> {
        let (fin, domain) = address
            .split_once('@')
            .ok_or_else(|| format!("invalid local address {address:?}; expected fin@pod.orqa"))?;
        let pod = domain
            .strip_suffix(".orqa")
            .ok_or_else(|| format!("invalid local address {address:?}; expected fin@pod.orqa"))?;

        validate_slug(fin)?;
        validate_slug(pod)?;

        Ok(Self {
            fin: fin.to_string(),
            pod: pod.to_string(),
        })
    }

    fn label(&self) -> String {
        format!("{}@{}.orqa", self.fin, self.pod)
    }
}

fn validate_slug(slug: &str) -> Result<(), String> {
    let valid = !slug.is_empty()
        && slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');

    if valid {
        Ok(())
    } else {
        Err(format!(
            "invalid slug {slug:?}; use lowercase letters, digits, and hyphens"
        ))
    }
}

fn default_home() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".orqa")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_lowercase_slug_parts() {
        assert!(validate_slug("sample-pod").is_ok());
        assert!(validate_slug("bob-jones").is_ok());
        assert!(validate_slug("amy2").is_ok());
    }

    #[test]
    fn rejects_path_like_slugs() {
        assert!(validate_slug("../sample-pod").is_err());
        assert!(validate_slug("SamplePod").is_err());
        assert!(validate_slug("").is_err());
    }

    #[test]
    fn parses_local_mail_addresses() {
        let address = MailAddress::parse("amy@sample-pod.orqa").unwrap();

        assert_eq!(address.fin, "amy");
        assert_eq!(address.pod, "sample-pod");
        assert_eq!(address.label(), "amy@sample-pod.orqa");
    }

    #[test]
    fn qualifies_bare_mail_addresses_with_pod_hint() {
        let address = resolve_address("bob-jones", Some("sample-pod")).unwrap();

        assert_eq!(address.fin, "bob-jones");
        assert_eq!(address.pod, "sample-pod");
        assert_eq!(address.label(), "bob-jones@sample-pod.orqa");
    }

    #[test]
    fn bare_mail_addresses_need_pod_context() {
        assert!(resolve_address("bob-jones", None).is_err());
    }

    #[test]
    fn rejects_non_orqa_mail_addresses() {
        assert!(MailAddress::parse("amy@example.com").is_err());
        assert!(MailAddress::parse("amy").is_err());
        assert!(MailAddress::parse("Amy@sample-pod.orqa").is_err());
    }

    #[test]
    fn resolves_message_ids_in_maildir_states() {
        let root = env::temp_dir().join(format!("orqa-test-{}", unique_mail_name().unwrap()));
        let mail_home = root.join("mail");
        ensure_maildir(&mail_home).unwrap();
        let path = deliver_mail(&mail_home, "Subject: test\n\nbody\n").unwrap();
        let id = message_id(&path).unwrap();

        assert_eq!(resolve_message_path(&mail_home, &id).unwrap(), path);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn canonicalizes_plain_task_bodies() {
        let from = MailAddress::parse("amy@sample-pod.orqa").unwrap();
        let to = MailAddress::parse("bob-jones@sample-pod.orqa").unwrap();
        let task = canonical_task_body(&from, &to, Some("update-settings"), "Do the thing.");

        assert!(task.starts_with("---\n"));
        assert!(task.contains("from: amy@sample-pod.orqa\n"));
        assert!(task.contains("to: bob-jones@sample-pod.orqa\n"));
        assert!(task.contains("title: update-settings\n"));
        assert!(task.contains("priority: normal\n"));
        assert!(task.contains("status: open\n"));
        assert!(task.contains("kind: need\n"));
        assert!(task.contains("depends_on: []\n"));
        assert!(task.ends_with("Do the thing.\n"));
    }

    #[test]
    fn preserves_and_fills_task_front_matter() {
        let from = MailAddress::parse("amy@sample-pod.orqa").unwrap();
        let to = MailAddress::parse("bob-jones@sample-pod.orqa").unwrap();
        let task = canonical_task_body(
            &from,
            &to,
            None,
            "---\ntitle: supplied-title\npriority: high\ncustom: keep-me\n---\n\nDetails.",
        );

        assert!(task.contains("title: supplied-title\n"));
        assert!(task.contains("priority: high\n"));
        assert!(task.contains("custom: keep-me\n"));
        assert!(task.contains("status: open\n"));
        assert!(task.contains("kind: need\n"));
        assert!(task.ends_with("Details.\n"));
    }

    #[test]
    fn parses_task_field_filters() {
        let args = TaskListArgs {
            pod: None,
            fin: None,
            all: false,
            status: Some("open".to_string()),
            priority: Some("high".to_string()),
            kind: None,
            fields: vec!["owner=amy".to_string()],
            sort: None,
            reverse: false,
        };
        let filters = TaskFilters::new(&args).unwrap();

        assert_eq!(
            filters.fields,
            vec![
                ("status".to_string(), "open".to_string()),
                ("priority".to_string(), "high".to_string()),
                ("owner".to_string(), "amy".to_string())
            ]
        );
    }

    #[test]
    fn quotes_shell_unfriendly_values() {
        assert_eq!(quote_value("high"), "high");
        assert_eq!(quote_value("update settings"), "\"update settings\"");
        assert_eq!(quote_value("say \"hi\""), "\"say \\\"hi\\\"\"");
    }

    #[test]
    fn sorts_known_priorities_by_severity() {
        assert!(priority_sort_value("high") < priority_sort_value("normal"));
        assert!(priority_sort_value("normal") < priority_sort_value("low"));
    }

    #[test]
    fn parses_lock_pid() {
        assert_eq!(lock_pid("pid=123\nfin=amy\n"), Some(123));
        assert_eq!(lock_pid("fin=amy\n"), None);
    }

    #[test]
    fn writes_and_removes_sleep_markers() {
        let root = env::temp_dir().join(format!("orqa-test-{}", unique_mail_name().unwrap()));
        let marker = root.join("sleep.lock");

        write_sleep_marker(&marker).unwrap();
        assert!(marker.exists());
        remove_sleep_marker(&marker).unwrap();
        assert!(!marker.exists());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pod_config_template_includes_commented_backend_examples() {
        let pod = PodRef::new("sample-pod").unwrap();
        let toml = pod_config_template(&pod);

        assert!(toml.contains("[pod]"));
        assert!(toml.contains("slug = \"sample-pod\""));
        assert!(toml.contains("[backends.codex]"));
        assert!(toml.contains("command = \"codex\""));
        assert!(toml.contains("# [backends.opencode]"));
        assert!(toml.contains("# [backends.pi]"));
        assert!(toml.contains("# [backends.custom]"));
    }

    #[test]
    fn fin_config_template_inherits_pod_backend_by_default() {
        let fin = FinRef::new("sample-pod", "amy").unwrap();
        let toml = fin_config_template(&fin);

        assert!(toml.contains("[fin]"));
        assert!(toml.contains("slug = \"amy\""));
        assert!(toml.contains("# backend = \"codex\""));
    }
}
