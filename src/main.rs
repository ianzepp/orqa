use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, ExitCode},
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

struct Orqa {
    home: PathBuf,
}

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
            fs::create_dir_all(home.join("mail")).map_err(|error| {
                format!(
                    "failed to create mail directory for {}: {error}",
                    agent.label()
                )
            })?;
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
            println!("{}", orqa.agent_home(&agent).join("mail").display());
            Ok(())
        }
    }
}

fn loop_pod(orqa: &Orqa, args: LoopArgs) -> Result<(), String> {
    let pod = PodRef::new(&args.pod)?;
    println!("loop wake scan for pod {}", pod.slug);
    println!("pod_home={}", orqa.pod_home(&pod).display());
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
