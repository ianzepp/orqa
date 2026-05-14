use std::{env, fs, path::Path};

use crate::{
    cli::{
        FinCommand, FinSubcommand, MailCommand, MailSubcommand, PodCommand, PodSubcommand,
        TaskCommand, TaskSubcommand,
    },
    config::{fin_config_template, pod_config_template},
    mailbox::{
        ItemKind, delete_item, delete_mail, done_item, done_mail, ensure_maildir, list_mail,
        list_tasks, read_item, read_mail, remove_sleep_marker, send_mail, send_task, unread_mail,
        write_if_missing, write_sleep_marker,
    },
    model::{FinRef, Orqa, PodRef},
    runtime::run_fin,
};

pub(crate) fn pod(orqa: &Orqa, command: PodCommand) -> Result<(), String> {
    match command.command {
        PodSubcommand::List => list_dirs(&orqa.home.join("pods")),
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

pub(crate) fn fin(orqa: &Orqa, command: FinCommand) -> Result<(), String> {
    match command.command {
        FinSubcommand::List(args) => {
            let pod = match args.pod {
                Some(pod) => pod,
                None => env::var("ORQA_POD")
                    .map_err(|_| "missing pod; pass a pod or run with ORQA_POD set".to_string())?,
            };
            let pod = PodRef::new(&pod)?;
            list_dirs(&orqa.pod_home(&pod).join("fins"))
        }
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

fn list_dirs(dir: &Path) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
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
    for name in names {
        println!("{name}");
    }

    Ok(())
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
