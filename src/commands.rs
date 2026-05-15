use std::{
    env, fs,
    io::{self, Read},
    path::Path,
};

use crate::{
    cli::{
        FinCommand, FinRoleSubcommand, FinSubcommand, LoopCommand, LoopStartArgs, LoopSubcommand,
        MailCommand, MailSubcommand, OpsCommand, OpsSubcommand, PodCharterSubcommand, PodCommand,
        PodSubcommand, TaskCommand, TaskSubcommand,
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

pub(crate) fn fin(orqa: &Orqa, command: FinCommand) -> Result<(), String> {
    match command.command {
        FinSubcommand::List(args) => {
            let pod = match args.pod {
                Some(pod) => pod,
                None => env::var("ORQA_POD")
                    .map_err(|_| "missing pod; pass a pod or run with ORQA_POD set".to_string())?,
            };
            let pod = PodRef::new(&pod)?;
            print_dirs(&orqa.pod_home(&pod).join("fins"))
        }
        FinSubcommand::Create(args) => {
            let fin = FinRef::new(&args.pod, &args.fin)?;
            let home = orqa.fin_home(&fin);
            ensure_fin_runtime_homes(orqa, &fin)?;
            let role = read_optional_markdown_source(args.role.as_deref(), DEFAULT_ROLE)?;
            ensure_maildir(&orqa.mail_home(&fin))?;
            ensure_maildir(&orqa.task_home(&fin))?;
            write_if_missing(&home.join("fin.txt"), &format!("slug={}\n", fin.fin))?;
            write_if_missing(&home.join("fin.toml"), &fin_config_template(&fin))?;
            write_if_missing(&home.join("ROLE.md"), &role)?;
            write_if_missing(&home.join("AGENTS.md"), &fin_agents_template(&fin, &role))?;
            println!("{}", home.display());
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
    for pod in list_dirs(&orqa.home.join("pods"))? {
        let pod = PodRef::new(&pod)?;
        print_pod_list_status(&pod_status(orqa, &pod)?);
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
