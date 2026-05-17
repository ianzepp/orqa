use super::{
    print_file, read_markdown_source, read_optional_markdown_source,
    validate_backend_name, write_text,
};
use crate::{
    cli::{CommandContext, FinCommand, FinRoleSubcommand, FinSubcommand},
    config::{DEFAULT_ROLE, fin_agents_template, fin_config_template_with_backend},
    mailbox::{ensure_maildir, remove_sleep_marker, write_if_missing, write_sleep_marker},
    model::{FinRef, Orqa, PodRef},
    runs::{list_runs, read_run_logs, read_run_record_for, tail_fin},
    runtime::{chat_fin, exec_fin, supervise_fin},
    runtime_home::ensure_fin_runtime_homes,
    status::{fin_status, print_fin_status, print_json},
};

pub(super) fn create_fin_in_current_pod(
    orqa: &Orqa,
    context: &CommandContext,
    fin_slug: &str,
    role_source: Option<&str>,
    backend: Option<&str>,
) -> Result<(), String> {
    let (pod_slug, pod_root) = context.resolve_pod(None, orqa)?;
    let pod_ref = PodRef::new(&pod_slug)?;
    orqa.ensure_pod_exists(&pod_ref)?;

    create_fin_in_pod(orqa, &pod_slug, &pod_root, fin_slug, role_source, backend)
}

pub(in crate::commands) fn create_fin_in_pod(
    orqa: &Orqa,
    pod_slug: &str,
    pod_root: &std::path::Path,
    fin_slug: &str,
    role_source: Option<&str>,
    backend: Option<&str>,
) -> Result<(), String> {
    let fin = FinRef::new(pod_slug, fin_slug)?;
    let fin_home = pod_root.join(".orqa").join("fins").join(fin_slug);
    ensure_fin_runtime_homes(orqa, &fin)?;

    let role = read_optional_markdown_source(role_source, DEFAULT_ROLE)?;
    if let Some(backend) = backend {
        validate_backend_name(backend)?;
    }
    ensure_maildir(&fin_home.join("mail"))?;
    ensure_maildir(&fin_home.join("tasks"))?;

    write_if_missing(&fin_home.join("fin.txt"), &format!("slug={}\n", fin.fin))?;
    write_if_missing(
        &fin_home.join("fin.toml"),
        &fin_config_template_with_backend(&fin, backend),
    )?;
    write_if_missing(&fin_home.join("ROLE.md"), &role)?;
    write_if_missing(
        &fin_home.join("AGENTS.md"),
        &fin_agents_template(&fin, &role),
    )?;

    println!("{}", fin_home.display());
    Ok(())
}

pub(crate) fn fin(
    orqa: &Orqa,
    context: &CommandContext,
    command: FinCommand,
) -> Result<(), String> {
    match command.command {
        FinSubcommand::List(_args) => {
            let (pod_slug, pod_root) = context.resolve_pod(None, orqa)?;
            let pod_ref = PodRef::new(&pod_slug)?;
            let fins_dir = pod_root.join(".orqa").join("fins");
            orqa.ensure_pod_exists(&pod_ref)?;

            let names = super::list_dirs(&fins_dir)?;
            if names.is_empty() {
                println!("No fins found in pod '{}'.", pod_slug);
                println!();
                println!("Create one with: orqa fin create <fin> [--role <prompt|@file|->]");
                return Ok(());
            }

            for name in names {
                println!("{name}");
            }
            Ok(())
        }
        FinSubcommand::Create(args) => create_fin_in_current_pod(
            orqa,
            context,
            &args.fin,
            args.role.as_deref(),
            args.backend.as_deref(),
        ),
        FinSubcommand::Role(command) => match command.command {
            FinRoleSubcommand::Get(args) => {
                let fin = context.resolve_fin(None, args.fin, orqa)?;
                orqa.ensure_fin_exists(&fin)?;
                print_file(&orqa.fin_data_home(&fin)?.join("ROLE.md"))
            }
            FinRoleSubcommand::Set(args) => {
                let (fin_arg, role_arg) = args.resolve_refs()?;
                let fin = context.resolve_fin(None, fin_arg, orqa)?;
                orqa.ensure_fin_exists(&fin)?;
                let role = read_markdown_source(&role_arg)?;
                let home = orqa.fin_data_home(&fin)?;
                write_text(&home.join("ROLE.md"), &role)?;
                write_text(&home.join("AGENTS.md"), &fin_agents_template(&fin, &role))?;
                println!("{}", home.join("ROLE.md").display());
                Ok(())
            }
        },
        FinSubcommand::Home(args) => {
            let fin = context.resolve_fin(None, args.fin, orqa)?;
            orqa.ensure_fin_exists(&fin)?;
            println!("{}", orqa.fin_data_home(&fin)?.display());
            Ok(())
        }
        FinSubcommand::Status(args) => {
            let fin = context.resolve_fin(None, args.fin, orqa)?;
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
            let fin = context.resolve_fin(None, args.fin, orqa)?;
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
            let fin = context.resolve_fin(None, args.fin, orqa)?;
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
            let fin = context.resolve_fin(None, args.fin, orqa)?;
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
            let fin = context.resolve_fin(None, args.fin, orqa)?;
            orqa.ensure_fin_exists(&fin)?;
            tail_fin(orqa, &fin, args.run.as_deref(), args.lines, args.follow)
        }
        FinSubcommand::Pause(args) => {
            let fin = context.resolve_fin(None, args.fin, orqa)?;
            write_sleep_marker(&orqa.fin_sleep_path(&fin)?)?;
            println!("pause {}", fin.label());
            Ok(())
        }
        FinSubcommand::Resume(args) => {
            if !args.force {
                return Err("fin resume requires --force".to_string());
            }
            let fin = context.resolve_fin(None, args.fin, orqa)?;
            remove_sleep_marker(&orqa.fin_sleep_path(&fin)?)?;
            println!("resume {}", fin.label());
            Ok(())
        }
        FinSubcommand::Exec(args) => exec_fin(orqa, context, args),
        FinSubcommand::Chat(args) => chat_fin(orqa, context, args),
        FinSubcommand::Supervise(args) => supervise_fin(orqa, args),
    }
}
