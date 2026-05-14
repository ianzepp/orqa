mod storage;
mod tasks;

use std::{fs, path::PathBuf};

use crate::{
    cli::{FinRefArgs, MailListArgs, MailMessageArgs, SendMailArgs, SendTaskArgs, TaskListArgs},
    issues::create_operator_issue,
    model::{FinRef, Orqa},
};

pub(crate) use storage::{
    deliver_mail, ensure_maildir, mail_state, message_id, message_title, read_stdin,
    remove_sleep_marker, resolve_address, resolve_fin, resolve_message_path, resolve_sender,
    sorted_files, unread_count, write_if_missing, write_sleep_marker,
};
pub(crate) use tasks::{
    TaskFilters, canonical_task_body, collect_tasks, field_value, mark_task_done, sort_tasks,
    split_front_matter, upsert_field,
};

#[cfg(test)]
pub(crate) use storage::unique_mail_name;
#[cfg(test)]
pub(crate) use tasks::{priority_sort_value, quote_value};

pub(crate) fn send_mail(orqa: &Orqa, args: SendMailArgs) -> Result<(), String> {
    let from = resolve_sender(args.from.as_deref())?;
    let to = resolve_address(&args.to, Some(&from.pod))?;

    if from.pod != to.pod && !is_operator_mail_bridge(&from.pod, &to.pod) {
        return Err(format!(
            "cross-pod mail is not supported: {} -> {}",
            from.label(),
            to.label()
        ));
    }

    let body = match args.body {
        Some(body) => body,
        None => read_stdin()?,
    };

    if to.fin == "operator" {
        let path = create_operator_issue(orqa, &from, &args.subject, &body)?;
        println!("{}", path.display());
        println!("opened operator issue for {}", from.pod);
        return Ok(());
    }

    let from_fin = FinRef::new(&from.pod, &from.fin)?;
    let to_fin = FinRef::new(&to.pod, &to.fin)?;
    ensure_target_fin(orqa, &to_fin)?;
    let mail_home = orqa.mail_home(&to_fin);
    ensure_maildir(&mail_home)?;

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

pub(crate) fn unread_mail(orqa: &Orqa, args: FinRefArgs) -> Result<(), String> {
    let fin = FinRef::new(&args.pod, &args.fin)?;
    let new_dir = orqa.mail_home(&fin).join("new");

    for path in sorted_files(&new_dir)? {
        println!("{}", path.display());
    }

    Ok(())
}

pub(crate) fn list_mail(orqa: &Orqa, args: MailListArgs) -> Result<(), String> {
    list_items(orqa, args, ItemKind::Mail)
}

pub(crate) fn read_mail(orqa: &Orqa, args: MailMessageArgs) -> Result<(), String> {
    read_item(orqa, args, ItemKind::Mail)
}

pub(crate) fn done_mail(orqa: &Orqa, args: MailMessageArgs) -> Result<(), String> {
    done_item(orqa, args, ItemKind::Mail)
}

pub(crate) fn delete_mail(orqa: &Orqa, args: MailMessageArgs) -> Result<(), String> {
    delete_item(orqa, args, ItemKind::Mail)
}

pub(crate) fn send_task(orqa: &Orqa, args: SendTaskArgs) -> Result<(), String> {
    let from = resolve_sender(args.from.as_deref())?;
    let to = resolve_address(&args.to, Some(&from.pod))?;

    if from.pod != to.pod && !is_operator_bridge(&from.pod) {
        return Err(format!(
            "cross-pod tasks are not supported: {} -> {}",
            from.label(),
            to.label()
        ));
    }

    let to_fin = FinRef::new(&to.pod, &to.fin)?;
    ensure_target_fin(orqa, &to_fin)?;
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

fn is_operator_bridge(pod: &str) -> bool {
    pod == "operator"
}

fn is_operator_mail_bridge(from_pod: &str, to_pod: &str) -> bool {
    from_pod == "operator" || to_pod == "operator"
}

fn ensure_target_fin(orqa: &Orqa, fin: &FinRef) -> Result<(), String> {
    let config = orqa.fin_home(fin).join("fin.toml");
    if config.exists() {
        Ok(())
    } else {
        Err(format!("target fin {} does not exist", fin.label()))
    }
}

pub(crate) fn list_tasks(orqa: &Orqa, args: TaskListArgs) -> Result<(), String> {
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
pub(crate) enum ItemKind {
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

pub(crate) fn list_items(orqa: &Orqa, args: MailListArgs, kind: ItemKind) -> Result<(), String> {
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

pub(crate) fn read_item(orqa: &Orqa, args: MailMessageArgs, kind: ItemKind) -> Result<(), String> {
    let fin = resolve_fin(args.pod.as_deref(), args.fin.as_deref())?;
    let path = resolve_message_path(&kind.home(orqa, &fin), &args.message)?;
    let message = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

    print!("{message}");
    Ok(())
}

pub(crate) fn done_item(orqa: &Orqa, args: MailMessageArgs, kind: ItemKind) -> Result<(), String> {
    let fin = resolve_fin(args.pod.as_deref(), args.fin.as_deref())?;
    let home = kind.home(orqa, &fin);
    let path = resolve_message_path(&home, &args.message)?;
    let id = message_id(&path)?;

    if mail_state(&home, &path)? == "cur" {
        if matches!(kind, ItemKind::Task) {
            update_task_done_status(&path)?;
        }
        println!("{}", path.display());
        return Ok(());
    }

    if matches!(kind, ItemKind::Task) {
        update_task_done_status(&path)?;
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

fn update_task_done_status(path: &std::path::Path) -> Result<(), String> {
    let body = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    fs::write(path, mark_task_done(&body))
        .map_err(|error| format!("failed to mark task done {}: {error}", path.display()))
}

pub(crate) fn delete_item(
    orqa: &Orqa,
    args: MailMessageArgs,
    kind: ItemKind,
) -> Result<(), String> {
    let fin = resolve_fin(args.pod.as_deref(), args.fin.as_deref())?;
    let path = resolve_message_path(&kind.home(orqa, &fin), &args.message)?;

    fs::remove_file(&path)
        .map_err(|error| format!("failed to delete item {}: {error}", path.display()))?;
    println!("deleted {}", path.display());
    Ok(())
}
