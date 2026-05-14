use std::{
    env,
    ffi::OsString,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, ExitCode},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "orqa",
    version,
    about = "Fan out work to background agents",
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
    /// Create or run agents inside a pod.
    Agent(AgentCommand),
    /// Mail helpers for pod-local agent messages.
    Mail(MailCommand),
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
}

#[derive(Debug, Args)]
struct AgentCommand {
    #[command(subcommand)]
    command: AgentSubcommand,
}

#[derive(Debug, Subcommand)]
enum AgentSubcommand {
    /// Create an agent inside a pod.
    Create(AgentRefArgs),
    /// Print the home directory for an agent.
    Home(AgentRefArgs),
    /// Run an agent through the configured framework.
    Run(RunArgs),
}

#[derive(Debug, Args)]
struct MailCommand {
    #[command(subcommand)]
    command: MailSubcommand,
}

#[derive(Debug, Subcommand)]
enum MailSubcommand {
    /// Print the mail directory for an agent.
    Home(AgentRefArgs),
    /// Send a pod-local message.
    Send(SendMailArgs),
    /// List unread messages for an agent.
    Unread(AgentRefArgs),
}

#[derive(Debug, Args)]
struct LoopArgs {
    /// Pod slug.
    pod: String,
}

#[derive(Debug, Args)]
struct SlugArgs {
    /// Pod slug.
    slug: String,
}

#[derive(Debug, Args)]
struct AgentRefArgs {
    /// Pod slug.
    pod: String,
    /// Agent slug inside the pod.
    agent: String,
}

#[derive(Debug, Args)]
struct RunArgs {
    /// Pod slug.
    pod: String,
    /// Agent slug inside the pod.
    agent: String,
    /// Agent framework executable.
    #[arg(long, default_value = "codex")]
    framework: OsString,
    /// Arguments passed to the agent framework.
    #[arg(last = true)]
    args: Vec<OsString>,
}

#[derive(Debug, Args)]
struct SendMailArgs {
    /// Sender address. Defaults to ORQA_AGENT@ORQA_POD.orqa.
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
        Command::Agent(command) => agent(orqa, command),
        Command::Mail(command) => mail(orqa, command),
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
            fs::create_dir_all(home.join("agents")).map_err(|error| {
                format!("failed to create pod directory {}: {error}", home.display())
            })?;
            write_if_missing(&home.join("pod.txt"), &format!("slug={}\n", pod.slug))?;
            println!("{}", home.display());
            Ok(())
        }
        PodSubcommand::Home(args) => {
            let pod = PodRef::new(&args.slug)?;
            println!("{}", orqa.pod_home(&pod).display());
            Ok(())
        }
    }
}

fn agent(orqa: &Orqa, command: AgentCommand) -> Result<(), String> {
    match command.command {
        AgentSubcommand::Create(args) => {
            let agent = AgentRef::new(&args.pod, &args.agent)?;
            let home = orqa.agent_home(&agent);
            fs::create_dir_all(home.join(".codex")).map_err(|error| {
                format!(
                    "failed to create agent directory {}: {error}",
                    home.display()
                )
            })?;
            ensure_maildir(&orqa.mail_home(&agent))?;
            write_if_missing(&home.join("agent.txt"), &format!("slug={}\n", agent.agent))?;
            println!("{}", home.display());
            Ok(())
        }
        AgentSubcommand::Home(args) => {
            let agent = AgentRef::new(&args.pod, &args.agent)?;
            println!("{}", orqa.agent_home(&agent).display());
            Ok(())
        }
        AgentSubcommand::Run(args) => run_agent(orqa, args),
    }
}

fn mail(orqa: &Orqa, command: MailCommand) -> Result<(), String> {
    match command.command {
        MailSubcommand::Home(args) => {
            let agent = AgentRef::new(&args.pod, &args.agent)?;
            println!("{}", orqa.mail_home(&agent).display());
            Ok(())
        }
        MailSubcommand::Send(args) => send_mail(orqa, args),
        MailSubcommand::Unread(args) => unread_mail(orqa, args),
    }
}

fn loop_pod(orqa: &Orqa, args: LoopArgs) -> Result<(), String> {
    let pod = PodRef::new(&args.pod)?;
    let agents_dir = orqa.pod_home(&pod).join("agents");
    let agents = fs::read_dir(&agents_dir).map_err(|error| {
        format!(
            "failed to read agents directory {}: {error}",
            agents_dir.display()
        )
    })?;

    for entry in agents {
        let entry = entry.map_err(|error| format!("failed to read agent directory: {error}"))?;
        if !entry.path().is_dir() {
            continue;
        }

        let agent_slug = entry.file_name().to_string_lossy().to_string();
        let agent = AgentRef::new(&pod.slug, &agent_slug)?;
        let unread = unread_count(&orqa.mail_home(&agent))?;

        if unread > 0 {
            println!("wake {} unread={}", agent.label(), unread);
        }
    }

    Ok(())
}

fn run_agent(orqa: &Orqa, args: RunArgs) -> Result<(), String> {
    let agent = AgentRef::new(&args.pod, &args.agent)?;
    let home = orqa.agent_home(&agent);
    let codex_home = home.join(".codex");

    fs::create_dir_all(&codex_home).map_err(|error| {
        format!(
            "failed to create agent codex home {}: {error}",
            codex_home.display()
        )
    })?;

    let status = ProcessCommand::new(&args.framework)
        .env("ORQA_HOME", &orqa.home)
        .env("ORQA_POD", &agent.pod)
        .env("ORQA_AGENT", &agent.agent)
        .env("CODEX_HOME", &codex_home)
        .args(&args.args)
        .status()
        .map_err(|error| format!("failed to run {:?}: {error}", args.framework))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{:?} exited with {}",
            args.framework,
            status
                .code()
                .map_or_else(|| "signal".to_string(), |code| code.to_string())
        ))
    }
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

    let from_agent = AgentRef::new(&from.pod, &from.agent)?;
    let to_agent = AgentRef::new(&to.pod, &to.agent)?;
    let mail_home = orqa.mail_home(&to_agent);
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
    println!("queued wake for {}", to_agent.label());

    let _ = from_agent;
    Ok(())
}

fn unread_mail(orqa: &Orqa, args: AgentRefArgs) -> Result<(), String> {
    let agent = AgentRef::new(&args.pod, &args.agent)?;
    let new_dir = orqa.mail_home(&agent).join("new");

    for path in sorted_files(&new_dir)? {
        println!("{}", path.display());
    }

    Ok(())
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
                "missing sender; use --from agent@pod.orqa or run with ORQA_POD and ORQA_AGENT set"
                    .to_string()
            })?;
            let agent = env::var("ORQA_AGENT").map_err(|_| {
                "missing sender; use --from agent@pod.orqa or run with ORQA_POD and ORQA_AGENT set"
                    .to_string()
            })?;

            resolve_address(&agent, Some(&pod))
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
                "bare address {address:?} needs ORQA_POD; use agent@pod.orqa or run with ORQA_POD set"
            )
        })?,
    };

    validate_slug(address)?;
    validate_slug(&pod)?;

    Ok(MailAddress {
        agent: address.to_string(),
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

    fn agent_home(&self, agent: &AgentRef) -> PathBuf {
        self.home
            .join("pods")
            .join(&agent.pod)
            .join("agents")
            .join(&agent.agent)
    }

    fn mail_home(&self, agent: &AgentRef) -> PathBuf {
        self.agent_home(agent).join("mail")
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

struct AgentRef {
    pod: String,
    agent: String,
}

impl AgentRef {
    fn new(pod: &str, agent: &str) -> Result<Self, String> {
        validate_slug(pod)?;
        validate_slug(agent)?;
        Ok(Self {
            pod: pod.to_string(),
            agent: agent.to_string(),
        })
    }

    fn label(&self) -> String {
        format!("{}/{}", self.pod, self.agent)
    }
}

struct MailAddress {
    agent: String,
    pod: String,
}

impl MailAddress {
    fn parse(address: &str) -> Result<Self, String> {
        let (agent, domain) = address
            .split_once('@')
            .ok_or_else(|| format!("invalid local address {address:?}; expected agent@pod.orqa"))?;
        let pod = domain
            .strip_suffix(".orqa")
            .ok_or_else(|| format!("invalid local address {address:?}; expected agent@pod.orqa"))?;

        validate_slug(agent)?;
        validate_slug(pod)?;

        Ok(Self {
            agent: agent.to_string(),
            pod: pod.to_string(),
        })
    }

    fn label(&self) -> String {
        format!("{}@{}.orqa", self.agent, self.pod)
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

        assert_eq!(address.agent, "amy");
        assert_eq!(address.pod, "sample-pod");
        assert_eq!(address.label(), "amy@sample-pod.orqa");
    }

    #[test]
    fn qualifies_bare_mail_addresses_with_pod_hint() {
        let address = resolve_address("bob-jones", Some("sample-pod")).unwrap();

        assert_eq!(address.agent, "bob-jones");
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
}
