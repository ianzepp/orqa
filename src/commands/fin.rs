use super::{
    print_dirs, print_file, read_markdown_source, read_optional_markdown_source,
    validate_backend_name, write_text,
};
use crate::{
    cli::{FinCommand, FinRoleSubcommand, FinSubcommand},
    config::{DEFAULT_ROLE, fin_agents_template, fin_config_template_with_backend},
    mailbox::{ensure_maildir, remove_sleep_marker, write_if_missing, write_sleep_marker},
    model::{FinRef, Orqa, PodRef, resolve_pod_context},
    runs::{list_runs, read_run_logs, read_run_record_for, tail_fin},
    runtime::{chat_fin, exec_fin, supervise_fin},
    runtime_home::ensure_fin_runtime_homes,
    status::{fin_status, print_fin_status, print_json},
};

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
            let (pod_arg, fin_slug) = args.resolve_refs()?;
            let (pod_slug, pod_root) = resolve_pod_context(pod_arg, orqa)?;

            // If we have a real root (from detection or explicit new-style), use new paths
            let (fin_home, _use_new_paths) = if pod_root.join(".orqa").exists() {
                let reg = crate::model::PodRegistration {
                    slug: pod_slug.clone(),
                    path: pod_root.clone(),
                    enabled: true,
                };
                (orqa.fin_data_home(&reg, &fin_slug), true)
            } else {
                let fin_ref = FinRef::new(&pod_slug, &fin_slug)?;
                (orqa.fin_home(&fin_ref), false)
            };

            // For existence check, we still use the legacy PodRef for now (will be cleaned in later phases)
            let pod_ref = PodRef::new(&pod_slug)?;
            orqa.ensure_pod_exists(&pod_ref)?;

            let fin = FinRef::new(&pod_slug, &fin_slug)?;
            ensure_fin_runtime_homes(orqa, &fin)?;

            let role = read_optional_markdown_source(args.role.as_deref(), DEFAULT_ROLE)?;
            if let Some(backend) = &args.backend {
                validate_backend_name(backend)?;
            }
            ensure_maildir(&fin_home.join("mail"))?;
            ensure_maildir(&fin_home.join("tasks"))?;

            write_if_missing(&fin_home.join("fin.txt"), &format!("slug={}\n", fin.fin))?;
            write_if_missing(
                &fin_home.join("fin.toml"),
                &fin_config_template_with_backend(&fin, args.backend.as_deref()),
            )?;
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
