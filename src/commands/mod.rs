use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

mod fin;
mod loop_command;
mod mail;
mod pod;
mod task;

pub(crate) use fin::fin;
pub(crate) use loop_command::{is_process_running, loop_command, service};
pub(crate) use mail::mail;
pub(crate) use task::task;

use crate::{
    cli::{InitArgs, OpsCommand, OpsSubcommand, PodCharterSubcommand, PodCommand, PodSubcommand},
    config::{
        DEFAULT_CHARTER, fin_agents_template, fin_config_template_with_backend,
        pod_agents_template, pod_config_template,
    },
    doctor::pod_doctor,
    hooks::{add_hook, disable_hook, enable_hook, list_hooks, remove_hook, run_hooks},
    mailbox::{ensure_maildir, remove_sleep_marker, write_if_missing, write_sleep_marker},
    model::{FinRef, Orqa, PodRef},
    report::ops_report,
    runs::tail_pod,
    status::{pod_status, print_json, print_pod_list_status, print_pod_status},
};

pub(crate) fn pod(orqa: &Orqa, command: PodCommand) -> Result<(), String> {
    match command.command {
        PodSubcommand::List => pod::list_pods(orqa),
        PodSubcommand::Create(args) => {
            if let Some(target_root) = args.path {
                // New-style pod via explicit path — delegate to shared implementation
                create_pod_in_dir(orqa, &args.slug, target_root, args.charter)
            } else {
                // Legacy behavior (old ~/.orqa/pods/<slug> layout)
                let pod = PodRef::new(&args.slug)?;
                let home = orqa.pod_home(&pod);
                fs::create_dir_all(home.join("fins")).map_err(|error| {
                    format!("failed to create pod directory {}: {error}", home.display())
                })?;
                let charter =
                    read_optional_markdown_source(args.charter.as_deref(), DEFAULT_CHARTER)?;
                write_if_missing(&home.join("pod.txt"), &format!("slug={}\n", pod.slug))?;
                write_if_missing(&home.join("pod.toml"), &pod_config_template(&pod))?;
                write_if_missing(&home.join("CHARTER.md"), &charter)?;
                write_if_missing(
                    &home.join("AGENTS.md"),
                    &pod_agents_template(&pod, &charter),
                )?;
                println!("{}", home.display());
                Ok(())
            }
        }
        PodSubcommand::Charter(command) => match command.command {
            PodCharterSubcommand::Get(args) => {
                let pod = PodRef::new(&args.slug)?;
                orqa.ensure_pod_exists(&pod)?;
                print_file(&orqa.pod_home(&pod).join("CHARTER.md"))
            }
            PodCharterSubcommand::Set(args) => {
                let pod = PodRef::new(&args.slug)?;
                orqa.ensure_pod_exists(&pod)?;
                let charter = read_markdown_source(&args.charter)?;
                let home = orqa.pod_home(&pod);
                write_text(&home.join("CHARTER.md"), &charter)?;
                write_text(
                    &home.join("AGENTS.md"),
                    &pod_agents_template(&pod, &charter),
                )?;
                println!("{}", home.join("CHARTER.md").display());
                Ok(())
            }
        },
        PodSubcommand::Home(args) => {
            let pod = PodRef::new(&args.slug)?;
            println!("{}", orqa.pod_home(&pod).display());
            Ok(())
        }
        PodSubcommand::Status(args) => {
            let pod = PodRef::new(&args.pod)?;
            orqa.ensure_pod_exists(&pod)?;
            let status = pod_status(orqa, &pod)?;
            if args.json {
                print_json(&status)
            } else {
                print_pod_status(&status);
                Ok(())
            }
        }
        PodSubcommand::Doctor(args) => pod_doctor(orqa, args),
        PodSubcommand::Hook(command) => match command.command {
            crate::cli::PodHookSubcommand::List(args) => list_hooks(orqa, args),
            crate::cli::PodHookSubcommand::Add(args) => add_hook(orqa, args),
            crate::cli::PodHookSubcommand::Enable(args) => enable_hook(orqa, args),
            crate::cli::PodHookSubcommand::Disable(args) => disable_hook(orqa, args),
            crate::cli::PodHookSubcommand::Remove(args) => remove_hook(orqa, args),
            crate::cli::PodHookSubcommand::Run(args) => run_hooks(orqa, args),
        },
        PodSubcommand::Tail(args) => {
            let pod = PodRef::new(&args.pod)?;
            if let Some(fin) = &args.fin {
                FinRef::new(&pod.slug, fin)?;
            }
            tail_pod(
                orqa,
                &pod.slug,
                args.fin.as_deref(),
                args.lines,
                args.follow,
            )
        }
        PodSubcommand::Sleep(args) => {
            let pod = PodRef::new(&args.slug)?;
            write_sleep_marker(&orqa.effective_pod_home(&pod).join("sleep.lock"))?;
            println!("sleep {}", pod.slug);
            Ok(())
        }
        PodSubcommand::Wake(args) => {
            if !args.force {
                return Err("pod wake requires --force".to_string());
            }
            let pod = PodRef::new(&args.slug)?;
            remove_sleep_marker(&orqa.effective_pod_home(&pod).join("sleep.lock"))?;
            println!("wake {}", pod.slug);
            Ok(())
        }
    }
}

/// Initialize a new pod in the given (or current) directory.
/// This is the primary friendly onboarding command (`orqa init`).
///
/// It is intentionally a thin layer: it generates the slug (from the directory
/// name if not provided) and delegates to the shared creation logic used by
/// `orqa pod create --path`.
pub(crate) fn pod_init(orqa: &Orqa, args: InitArgs) -> Result<(), String> {
    let root = match args.path {
        Some(p) => p,
        None => {
            std::env::current_dir().map_err(|e| format!("failed to get current directory: {e}"))?
        }
    };

    let slug = match args.slug {
        Some(s) => s,
        None => {
            // Default to directory name — this is the main convenience of `orqa init`
            root.file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
                .ok_or(
                    "could not determine slug from directory name; please provide one explicitly",
                )?
        }
    };

    create_pod_in_dir(orqa, &slug, root, args.charter)
}

fn validate_slug_for_init(slug: &str) -> Result<(), String> {
    // Reuse existing validation but with a nicer message
    crate::model::validate_slug(slug).map_err(|e| format!("invalid pod slug: {e}"))
}

pub(super) fn validate_backend_name(name: &str) -> Result<(), String> {
    if !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        Ok(())
    } else {
        Err(format!(
            "invalid backend name {name:?}; use ASCII letters, numbers, '-' or '_'"
        ))
    }
}

/// Shared implementation for creating a new-style pod inside a user-provided directory.
/// Used by both `orqa init` and `orqa pod create --path`.
fn create_pod_in_dir(
    orqa: &Orqa,
    slug: &str,
    root: PathBuf,
    charter: Option<String>,
) -> Result<(), String> {
    validate_slug_for_init(slug)?;

    let orqa_dir = root.join(".orqa");

    if orqa_dir.join("pod.toml").exists() {
        return Err(format!(
            "orqa is already initialized in this directory (found {}).",
            orqa_dir.display()
        ));
    }

    fs::create_dir_all(orqa_dir.join("fins"))
        .map_err(|e| format!("failed to create .orqa directory: {e}"))?;

    let charter = read_optional_markdown_source(charter.as_deref(), DEFAULT_CHARTER)?;
    let pod_ref = PodRef::new(slug)?;

    let pod_data = orqa_dir;
    write_if_missing(&pod_data.join("pod.txt"), &format!("slug={}\n", slug))?;
    write_if_missing(&pod_data.join("pod.toml"), &pod_config_template(&pod_ref))?;
    write_if_missing(&pod_data.join("CHARTER.md"), &charter)?;
    write_if_missing(
        &pod_data.join("AGENTS.md"),
        &pod_agents_template(&pod_ref, &charter),
    )?;
    seed_operator_fin(&pod_ref, &pod_data)?;

    register_pod(orqa, slug, &root)?;

    if ensure_orqa_gitignored(&root)? {
        println!("Updated .gitignore to ignore /.orqa");
    }

    println!("Initialized pod '{}' in {}", slug, root.display());
    println!("Next steps:");
    println!("  orqa fin create planner");
    println!("  orqa loop");

    Ok(())
}

fn seed_operator_fin(pod: &PodRef, pod_data: &Path) -> Result<(), String> {
    let fin = FinRef::new(&pod.slug, "operator")?;
    let fin_home = pod_data.join("fins").join("operator");
    let role = "\
Human operator identity for the TUI. Use this fin as the stable local address
for messages that require human attention.";

    ensure_maildir(&fin_home.join("mail"))?;
    ensure_maildir(&fin_home.join("tasks"))?;
    fs::create_dir_all(fin_home.join("runs"))
        .map_err(|error| format!("failed to create operator runs directory: {error}"))?;

    write_if_missing(&fin_home.join("fin.txt"), "slug=operator\n")?;
    write_if_missing(
        &fin_home.join("fin.toml"),
        &fin_config_template_with_backend(&fin, None),
    )?;
    write_if_missing(
        &fin_home.join("ROLE.md"),
        &markdown_with_trailing_newline(role),
    )?;
    write_if_missing(
        &fin_home.join("AGENTS.md"),
        &fin_agents_template(&fin, role),
    )?;
    write_sleep_marker(&fin_home.join("sleep.lock"))?;

    Ok(())
}

/// Ensure `target_dir/.gitignore` ignores `/.orqa`.
/// Returns `true` if we created or modified the file.
fn ensure_orqa_gitignored(target_dir: &Path) -> Result<bool, String> {
    let gitignore_path = target_dir.join(".gitignore");
    if !gitignore_path.exists() {
        fs::write(
            &gitignore_path,
            "# Orqa coordination data (local only)\n/.orqa\n",
        )
        .map_err(|e| format!("failed to create .gitignore: {e}"))?;
        return Ok(true);
    }

    let content = fs::read_to_string(&gitignore_path)
        .map_err(|e| format!("failed to read .gitignore: {e}"))?;

    // Check common forms of ignoring .orqa
    let already_ignored = content.lines().any(|line| {
        let t = line.trim();
        t == ".orqa" || t == "/.orqa" || t == ".orqa/" || t == "/.orqa/" || t.starts_with(".orqa")
    });

    if already_ignored {
        return Ok(false);
    }

    let mut new_content = content;
    if !new_content.is_empty() && !new_content.ends_with('\n') {
        new_content.push('\n');
    }

    new_content.push_str("\n# Orqa coordination data (local only)\n/.orqa\n");

    fs::write(&gitignore_path, new_content)
        .map_err(|e| format!("failed to update .gitignore: {e}"))?;

    Ok(true)
}

/// Registers (or updates) a pod in the global ~/.orqa/config.toml
fn register_pod(orqa: &Orqa, slug: &str, root: &Path) -> Result<(), String> {
    let config_path = orqa.home.join("config.toml");

    // Read existing or start fresh
    let mut table: toml::Table = if config_path.exists() {
        let content =
            fs::read_to_string(&config_path).map_err(|e| format!("failed to read config: {e}"))?;
        content.parse().unwrap_or_else(|_| toml::Table::new())
    } else {
        toml::Table::new()
    };

    // Ensure [registry] section exists
    let registry = table
        .entry("registry".to_string())
        .or_insert(toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .ok_or("invalid registry section in config.toml")?;

    registry.insert("version".to_string(), toml::Value::Integer(1));

    // Ensure [pods] section
    let pods = table
        .entry("pods".to_string())
        .or_insert(toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .ok_or("invalid pods section")?;

    // Build the pod entry
    let mut pod_entry = toml::Table::new();
    pod_entry.insert(
        "path".to_string(),
        toml::Value::String(root.display().to_string()),
    );
    pod_entry.insert("enabled".to_string(), toml::Value::Boolean(true));

    pods.insert(slug.to_string(), toml::Value::Table(pod_entry));

    // Write back
    let new_content =
        toml::to_string_pretty(&table).map_err(|e| format!("failed to serialize config: {e}"))?;

    // Simple write (for Phase 05-3; we can improve atomicity later)
    fs::write(&config_path, new_content).map_err(|e| {
        format!(
            "failed to write global config {}: {e}",
            config_path.display()
        )
    })?;

    Ok(())
}

pub(crate) fn list_dirs(dir: &Path) -> Result<Vec<String>, String> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut names = Vec::new();
    for entry in
        fs::read_dir(dir).map_err(|error| format!("failed to read {}: {error}", dir.display()))?
    {
        let entry =
            entry.map_err(|error| format!("failed to read {} entry: {error}", dir.display()))?;
        if entry.path().is_dir() {
            names.push(entry.file_name().to_string_lossy().to_string());
        }
    }

    names.sort();
    Ok(names)
}

pub(super) fn print_dirs(dir: &Path) -> Result<(), String> {
    let names = list_dirs(dir)?;
    for name in names {
        println!("{name}");
    }

    Ok(())
}

pub(super) fn print_file(path: &Path) -> Result<(), String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    print!("{contents}");
    Ok(())
}

pub(super) fn write_text(path: &Path, contents: &str) -> Result<(), String> {
    fs::write(path, contents)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

pub(super) fn read_optional_markdown_source(
    source: Option<&str>,
    default_contents: &str,
) -> Result<String, String> {
    match source {
        Some(source) => read_markdown_source(source),
        None => Ok(markdown_with_trailing_newline(default_contents)),
    }
}

pub(super) fn read_markdown_source(source: &str) -> Result<String, String> {
    let contents = if source == "-" {
        let mut contents = String::new();
        io::stdin()
            .read_to_string(&mut contents)
            .map_err(|error| format!("failed to read stdin: {error}"))?;
        contents
    } else if let Some(path) = source.strip_prefix('@') {
        if path.is_empty() {
            return Err("expected a file path after @".to_string());
        }
        fs::read_to_string(path).map_err(|error| format!("failed to read {path}: {error}"))?
    } else {
        source.to_string()
    };

    Ok(markdown_with_trailing_newline(&contents))
}

fn markdown_with_trailing_newline(contents: &str) -> String {
    if contents.ends_with('\n') {
        contents.to_string()
    } else {
        format!("{contents}\n")
    }
}

pub(crate) fn ops(orqa: &Orqa, command: OpsCommand) -> Result<(), String> {
    match command.command {
        OpsSubcommand::Report(args) => ops_report(orqa, args),
    }
}

pub(crate) fn overview(orqa: &Orqa) -> Result<(), String> {
    println!("orqa — {}", orqa.home.display());

    // Loop daemon status
    let pid_path = orqa.home.join("loop.pid");
    if pid_path.exists() {
        if is_process_running(&pid_path) {
            if let Ok(pid) = std::fs::read_to_string(&pid_path) {
                println!("loop: running (pid {})", pid.trim());
            } else {
                println!("loop: running");
            }
        } else {
            println!("loop: not running (stale pidfile)");
            let _ = std::fs::remove_file(&pid_path);
        }
    } else {
        println!("loop: not running");
    }

    // Pods and wake signals
    let pods_dir = orqa.home.join("pods");
    let pods = list_dirs(&pods_dir)?;

    if pods.is_empty() {
        println!("pods: none");
        println!();
        println!("Create your first pod with: orqa pod create <slug>");
        println!("Run `orqa --help` for a list of commands.");
        return Ok(());
    }

    let mut total_fins = 0usize;
    let mut total_wakeable = 0usize;
    let mut _total_running = 0usize;
    let mut total_mail = 0usize;
    let mut total_tasks = 0usize;

    println!("pods:");
    for pod_name in &pods {
        let pod = PodRef::new(pod_name)?;
        let status = pod_status(orqa, &pod)?;
        total_fins += status.fin_count;
        total_wakeable += status.wakeable;
        _total_running += status.running;
        total_mail += status.unread_mail;
        total_tasks += status.open_tasks;
        print_pod_list_status(&status);
    }

    println!();
    println!(
        "totals: {} pods, {} fins, {} wakeable, {} unread mail, {} open tasks",
        pods.len(),
        total_fins,
        total_wakeable,
        total_mail,
        total_tasks
    );
    println!();
    println!("Run `orqa --help` for commands. Run `orqa help` for the agent operational guide.");

    Ok(())
}
