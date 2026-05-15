use std::{
    fs,
    io::{self, Read},
    path::Path,
};

use crate::{
    cli::{
        FinCommand, FinRoleSubcommand, FinSubcommand, InitArgs, LoopCommand, LoopStartArgs,
        LoopSubcommand, MailCommand, MailSubcommand, OpsCommand, OpsSubcommand,
        PodCharterSubcommand, PodCommand, PodSubcommand, TaskCommand, TaskSubcommand,
    },
    config::{
        DEFAULT_CHARTER, DEFAULT_ROLE, fin_agents_template, fin_config_template,
        pod_agents_template, pod_config_template,
    },
    doctor::pod_doctor,
    hooks::{add_hook, disable_hook, enable_hook, list_hooks, remove_hook, run_hooks},
    mailbox::{
        ItemKind, delete_item, delete_mail, done_item, done_mail, ensure_maildir, list_mail,
        list_tasks, read_item, read_mail, remove_sleep_marker, send_mail, send_task, unread_mail,
        write_if_missing, write_sleep_marker,
    },
    model::resolve_pod_context,
    model::{FinRef, Orqa, PodRef},
    report::ops_report,
    runs::{list_runs, read_run_logs, read_run_record_for, tail_fin, tail_pod},
    runtime::{chat_fin, exec_fin, loop_pod, plan, supervise_fin},
    runtime_home::ensure_fin_runtime_homes,
    status::{
        fin_status, pod_status, print_fin_status, print_json, print_pod_list_status,
        print_pod_status,
    },
};

pub(crate) fn pod(orqa: &Orqa, command: PodCommand) -> Result<(), String> {
    match command.command {
        PodSubcommand::List => list_pods(orqa),
        PodSubcommand::Create(args) => {
            let pod = PodRef::new(&args.slug)?;
            let home = orqa.pod_home(&pod);
            fs::create_dir_all(home.join("fins")).map_err(|error| {
                format!("failed to create pod directory {}: {error}", home.display())
            })?;
            let charter = read_optional_markdown_source(args.charter.as_deref(), DEFAULT_CHARTER)?;
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

/// Initialize a new pod in the given (or current) directory.
/// This is the primary friendly onboarding command (`orqa init`).
pub(crate) fn pod_init(orqa: &Orqa, args: InitArgs) -> Result<(), String> {
    let target_dir = match args.path {
        Some(p) => p,
        None => {
            std::env::current_dir().map_err(|e| format!("failed to get current directory: {e}"))?
        }
    };

    let slug = match args.slug {
        Some(s) => s,
        None => {
            // Default to directory name
            target_dir
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
                .ok_or(
                    "could not determine slug from directory name; please provide one explicitly",
                )?
        }
    };

    // Validate slug
    validate_slug_for_init(&slug)?;

    let root = target_dir; // the user's project folder
    let orqa_dir = root.join(".orqa");

    if orqa_dir.join("pod.toml").exists() {
        return Err(format!(
            "orqa is already initialized in this directory (found {}). Run 'orqa pod list' or 'orqa init' with a different directory.",
            orqa_dir.display()
        ));
    }

    // Create structure
    fs::create_dir_all(orqa_dir.join("fins"))
        .map_err(|e| format!("failed to create .orqa/fins directory: {e}"))?;

    let charter = read_optional_markdown_source(args.charter.as_deref(), DEFAULT_CHARTER)?;

    // Write files using the new data-home style where possible
    let reg = crate::model::PodRegistration {
        slug: slug.clone(),
        path: root.clone(),
        enabled: true,
    };

    let pod_data = orqa.pod_data_home(&reg);

    write_if_missing(&pod_data.join("pod.txt"), &format!("slug={}\n", slug))?;
    write_if_missing(
        &pod_data.join("pod.toml"),
        &pod_config_template(&PodRef::new(&slug)?),
    )?;
    write_if_missing(&pod_data.join("CHARTER.md"), &charter)?;
    write_if_missing(
        &pod_data.join("AGENTS.md"),
        &pod_agents_template(&PodRef::new(&slug)?, &charter),
    )?;

    // Register in global config
    register_pod(orqa, &slug, &root)?;

    // Auto-append to .gitignore if one exists (very common desire)
    if ensure_orqa_gitignored(&root)? {
        println!("Updated .gitignore to ignore /.orqa");
    }

    println!("Initialized pod '{}' in {}", slug, root.display());
    println!("Next steps:");
    println!("  orqa fin create planner");
    println!("  orqa loop");

    Ok(())
}

fn validate_slug_for_init(slug: &str) -> Result<(), String> {
    // Reuse existing validation but with a nicer message
    crate::model::validate_slug(slug).map_err(|e| format!("invalid pod slug: {e}"))
}

/// If a `.gitignore` file exists in `target_dir`, append `/.orqa` to it
/// (unless it's already ignored). Returns `true` if we modified the file.
fn ensure_orqa_gitignored(target_dir: &Path) -> Result<bool, String> {
    let gitignore_path = target_dir.join(".gitignore");
    if !gitignore_path.exists() {
        return Ok(false);
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

pub(crate) fn fin(orqa: &Orqa, command: FinCommand) -> Result<(), String> {
    match command.command {
        FinSubcommand::List(args) => {
            let (pod_slug, pod_root) = resolve_pod_context(args.pod.clone(), orqa)?;
            // For Phase 05-2 we use the new data-home path when we have a root from detection/env/CLI
            // while still supporting the old layout for explicit old-style pods.
            // In this phase we prioritize the new path if we have a root.
            let fins_dir = if pod_root.exists() {
                // Treat as new-style pod root
                let reg = crate::model::PodRegistration {
                    slug: pod_slug.clone(),
                    path: pod_root,
                    enabled: true,
                };
                orqa.pod_data_home(&reg).join("fins")
            } else {
                // Fall back to legacy behavior (for transition)
                let pod_ref = PodRef::new(&pod_slug)?;
                orqa.pod_home(&pod_ref).join("fins")
            };
            print_dirs(&fins_dir)
        }
        FinSubcommand::Create(args) => {
            let (pod_slug, pod_root) = resolve_pod_context(args.pod.clone(), orqa)?;

            // If we have a real root (from detection or explicit new-style), use new paths
            let (fin_home, _use_new_paths) = if pod_root.join(".orqa").exists() {
                let reg = crate::model::PodRegistration {
                    slug: pod_slug.clone(),
                    path: pod_root.clone(),
                    enabled: true,
                };
                (orqa.fin_data_home(&reg, &args.fin), true)
            } else {
                let fin_ref = FinRef::new(&pod_slug, &args.fin)?;
                (orqa.fin_home(&fin_ref), false)
            };

            // For existence check, we still use the legacy PodRef for now (will be cleaned in later phases)
            let pod_ref = PodRef::new(&pod_slug)?;
            orqa.ensure_pod_exists(&pod_ref)?;

            let fin = FinRef::new(&pod_slug, &args.fin)?;
            ensure_fin_runtime_homes(orqa, &fin)?;

            let role = read_optional_markdown_source(args.role.as_deref(), DEFAULT_ROLE)?;
            ensure_maildir(&fin_home.join("mail"))?;
            ensure_maildir(&fin_home.join("tasks"))?;

            write_if_missing(&fin_home.join("fin.txt"), &format!("slug={}\n", fin.fin))?;
            write_if_missing(&fin_home.join("fin.toml"), &fin_config_template(&fin))?;
            write_if_missing(&fin_home.join("ROLE.md"), &role)?;
            write_if_missing(
                &fin_home.join("AGENTS.md"),
                &fin_agents_template(&fin, &role),
            )?;

            println!("{}", fin_home.display());
            Ok(())
        }
        FinSubcommand::Role(command) => match command.command {
            FinRoleSubcommand::Get(args) => {
                let fin = FinRef::new(&args.pod, &args.fin)?;
                orqa.ensure_fin_exists(&fin)?;
                print_file(&orqa.fin_home(&fin).join("ROLE.md"))
            }
            FinRoleSubcommand::Set(args) => {
                let fin = FinRef::new(&args.pod, &args.fin)?;
                orqa.ensure_fin_exists(&fin)?;
                let role = read_markdown_source(&args.role)?;
                let home = orqa.fin_home(&fin);
                write_text(&home.join("ROLE.md"), &role)?;
                write_text(&home.join("AGENTS.md"), &fin_agents_template(&fin, &role))?;
                println!("{}", home.join("ROLE.md").display());
                Ok(())
            }
        },
        FinSubcommand::Home(args) => {
            let fin = FinRef::new(&args.pod, &args.fin)?;
            orqa.ensure_fin_exists(&fin)?;
            println!("{}", orqa.fin_home(&fin).display());
            Ok(())
        }
        FinSubcommand::Status(args) => {
            let fin = FinRef::new(&args.pod, &args.fin)?;
            orqa.ensure_fin_exists(&fin)?;
            let status = fin_status(orqa, &fin)?;
            if args.json {
                print_json(&status)
            } else {
                print_fin_status(&status);
                Ok(())
            }
        }
        FinSubcommand::Runs(args) => {
            let fin = FinRef::new(&args.pod, &args.fin)?;
            orqa.ensure_fin_exists(&fin)?;
            let runs = list_runs(orqa, &fin)?;
            if args.json {
                print_json(&runs)
            } else {
                for run in runs.runs {
                    println!(
                        "{} status={} exit_code={} mode={} backend={} command={}",
                        run.id,
                        run.status,
                        run.exit_code
                            .map_or_else(|| "-".to_string(), |code| code.to_string()),
                        run.mode,
                        run.backend,
                        run.command
                    );
                }
                Ok(())
            }
        }
        FinSubcommand::RunStatus(args) => {
            let fin = FinRef::new(&args.pod, &args.fin)?;
            orqa.ensure_fin_exists(&fin)?;
            let run = read_run_record_for(orqa, &fin, args.run.as_deref())?;
            if args.json {
                print_json(&run)
            } else {
                println!("run {}", run.id);
                println!("fin={}", run.fin);
                println!("status={}", run.status);
                println!("mode={}", run.mode);
                println!("backend={}", run.backend);
                println!("command={}", run.command);
                if let Some(exit_code) = run.exit_code {
                    println!("exit_code={exit_code}");
                }
                println!("run_dir={}", run.run_dir.display());
                Ok(())
            }
        }
        FinSubcommand::RunLog(args) => {
            let fin = FinRef::new(&args.pod, &args.fin)?;
            let logs = read_run_logs(orqa, &fin, args.run.as_deref())?;
            if args.json {
                print_json(&logs)
            } else {
                print!("{}", logs.stdout);
                eprint!("{}", logs.stderr);
                print!("{}", logs.events);
                Ok(())
            }
        }
        FinSubcommand::Tail(args) => {
            let fin = FinRef::new(&args.pod, &args.fin)?;
            orqa.ensure_fin_exists(&fin)?;
            tail_fin(orqa, &fin, args.run.as_deref(), args.lines, args.follow)
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
        FinSubcommand::Exec(args) => exec_fin(orqa, args),
        FinSubcommand::Chat(args) => chat_fin(orqa, args),
        FinSubcommand::Supervise(args) => supervise_fin(orqa, args),
    }
}

fn list_pods(orqa: &Orqa) -> Result<(), String> {
    // Phase 05-3+: Prefer registry (new pod roots) — basic listing for now
    let regs = crate::model::load_registry(orqa).unwrap_or_default();
    if !regs.is_empty() {
        for reg in regs.values().filter(|r| r.enabled) {
            println!("- {} @ {}", reg.slug, reg.path.display());
        }
    } else {
        // Legacy fallback
        for pod in list_dirs(&orqa.home.join("pods"))? {
            let pod = PodRef::new(&pod)?;
            print_pod_list_status(&pod_status(orqa, &pod)?);
        }
    }
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

fn print_dirs(dir: &Path) -> Result<(), String> {
    let names = list_dirs(dir)?;
    for name in names {
        println!("{name}");
    }

    Ok(())
}

fn print_file(path: &Path) -> Result<(), String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    print!("{contents}");
    Ok(())
}

fn write_text(path: &Path, contents: &str) -> Result<(), String> {
    fs::write(path, contents)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn read_optional_markdown_source(
    source: Option<&str>,
    default_contents: &str,
) -> Result<String, String> {
    match source {
        Some(source) => read_markdown_source(source),
        None => Ok(markdown_with_trailing_newline(default_contents)),
    }
}

fn read_markdown_source(source: &str) -> Result<String, String> {
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

pub(crate) fn mail(orqa: &Orqa, command: MailCommand) -> Result<(), String> {
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

pub(crate) fn task(orqa: &Orqa, command: TaskCommand) -> Result<(), String> {
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

pub(crate) fn ops(orqa: &Orqa, command: OpsCommand) -> Result<(), String> {
    match command.command {
        OpsSubcommand::Report(args) => ops_report(orqa, args),
    }
}

pub(crate) fn loop_command(orqa: &Orqa, command: LoopCommand) -> Result<(), String> {
    match command.command {
        LoopSubcommand::Run(args) => loop_pod(orqa, args),
        LoopSubcommand::Plan(args) => plan(orqa, args),
        LoopSubcommand::Start(args) => loop_start(orqa, args),
        LoopSubcommand::Stop => loop_stop(orqa),
        LoopSubcommand::Status => loop_status(orqa),
    }
}

fn loop_start(orqa: &Orqa, args: LoopStartArgs) -> Result<(), String> {
    let pid_path = orqa.home.join("loop.pid");

    // Prevent multiple startups + clean up stale pidfile
    if pid_path.exists() {
        if is_process_running(&pid_path) {
            return Err(
                "Loop daemon is already running. Use `orqa loop status` to check.".to_string(),
            );
        } else {
            // Stale pidfile — remove it
            let _ = std::fs::remove_file(&pid_path);
        }
    }

    let exe =
        std::env::current_exe().map_err(|e| format!("failed to get current executable: {}", e))?;

    let mut cmd = std::process::Command::new(exe);
    cmd.env("ORQA_DAEMON", "1")
        .env("ORQA_INTERVAL", args.interval.to_string())
        .env("ORQA_FORCE", if args.force { "1" } else { "0" })
        .arg("--home")
        .arg(&orqa.home);

    // Forward user prompt args via env var so the child daemon never sees them
    // as top-level CLI arguments (which would cause clap parse failure before
    // the ORQA_DAEMON branch is reached).
    let loop_args: Vec<String> = args
        .args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let args_json = serde_json::to_string(&loop_args)
        .map_err(|e| format!("failed to serialize loop prompt args: {}", e))?;
    cmd.env("ORQA_LOOP_ARGS", args_json);

    let child = cmd
        .spawn()
        .map_err(|e| format!("failed to start loop daemon: {}", e))?;

    std::fs::write(&pid_path, child.id().to_string())
        .map_err(|e| format!("failed to write pidfile: {}", e))?;

    println!("Loop daemon started (pid {})", child.id());
    Ok(())
}

fn loop_stop(orqa: &Orqa) -> Result<(), String> {
    let pid_path = orqa.home.join("loop.pid");

    if !pid_path.exists() {
        println!("No loop daemon is running.");
        return Ok(());
    }

    let pid_str =
        std::fs::read_to_string(&pid_path).map_err(|e| format!("failed to read pidfile: {}", e))?;
    let pid: u32 = pid_str
        .trim()
        .parse()
        .map_err(|_| "invalid PID in pidfile".to_string())?;

    println!("Stopping loop daemon (pid {})...", pid);

    #[cfg(unix)]
    {
        // Send graceful SIGTERM first
        let _ = std::process::Command::new("kill")
            .arg(pid.to_string())
            .status();

        // Wait up to 10 seconds for graceful shutdown
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(10) {
            if !is_process_running(&pid_path) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }

        // If still running after timeout, force kill
        if is_process_running(&pid_path) {
            println!("Daemon did not exit gracefully — sending SIGKILL...");
            let _ = std::process::Command::new("kill")
                .arg("-9")
                .arg(pid.to_string())
                .status();

            // Give it a moment
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
    }

    #[cfg(not(unix))]
    {
        // Windows fallback
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status();
    }

    let _ = std::fs::remove_file(&pid_path);
    println!("Loop daemon stopped.");
    Ok(())
}

fn loop_status(orqa: &Orqa) -> Result<(), String> {
    let pid_path = orqa.home.join("loop.pid");

    if !pid_path.exists() {
        println!("Loop daemon is not running");
        return Ok(());
    }

    if is_process_running(&pid_path) {
        let pid_str = std::fs::read_to_string(&pid_path).unwrap_or_default();
        println!("Loop daemon is running (pid {})", pid_str.trim());
    } else {
        println!("Loop daemon is not running (stale pidfile)");
        let _ = std::fs::remove_file(&pid_path);
    }

    Ok(())
}

pub(crate) fn is_process_running(pid_path: &std::path::Path) -> bool {
    if !pid_path.exists() {
        return false;
    }

    if let Ok(pid_str) = std::fs::read_to_string(pid_path) {
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            #[cfg(unix)]
            {
                // kill -0 just checks if process exists
                return std::process::Command::new("kill")
                    .arg("-0")
                    .arg(pid.to_string())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
            }

            #[cfg(not(unix))]
            {
                // On Windows, we can use tasklist or assume running if pidfile exists
                return true;
            }
        }
    }
    false
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
