use std::path::PathBuf;

use serde::Serialize;

use crate::{
    commands::list_dirs,
    mailbox::unread_count,
    model::{FinRef, Orqa, PodRef},
    runs::{RunRecord, read_run_record_for},
    runtime::{FinLock, process_is_alive},
};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PodStatus {
    pub(crate) pod: String,
    pub(crate) home: PathBuf,
    pub(crate) sleeping: bool,
    pub(crate) fins: Vec<FinStatus>,
    pub(crate) fin_count: usize,
    pub(crate) wakeable: usize,
    pub(crate) running: usize,
    pub(crate) unread_mail: usize,
    pub(crate) open_tasks: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct FinStatus {
    pub(crate) fin: String,
    pub(crate) home: PathBuf,
    pub(crate) sleeping: bool,
    pub(crate) running: bool,
    pub(crate) pid: Option<u32>,
    pub(crate) unread_mail: usize,
    pub(crate) open_tasks: usize,
    pub(crate) last_run: Option<RunRecord>,
}

pub(crate) fn pod_status(orqa: &Orqa, pod: &PodRef) -> Result<PodStatus, String> {
    let root = orqa.pod_root_for_slug(&pod.slug);
    let data_home = orqa.effective_pod_home(pod);
    let fins_dir = data_home.join("fins");

    let mut fins = Vec::new();
    for fin in list_dirs(&fins_dir)? {
        fins.push(fin_status(orqa, &FinRef::new(&pod.slug, &fin)?)?);
    }
    let wakeable = fins
        .iter()
        .filter(|fin| !fin.sleeping && !fin.running && (fin.unread_mail > 0 || fin.open_tasks > 0))
        .count();
    let running = fins.iter().filter(|fin| fin.running).count();
    let unread_mail = fins.iter().map(|fin| fin.unread_mail).sum();
    let open_tasks = fins.iter().map(|fin| fin.open_tasks).sum();
    Ok(PodStatus {
        pod: pod.slug.clone(),
        sleeping: data_home.join("sleep.lock").exists(),
        home: root,
        fin_count: fins.len(),
        wakeable,
        running,
        unread_mail,
        open_tasks,
        fins,
    })
}

pub(crate) fn fin_status(orqa: &Orqa, fin: &FinRef) -> Result<FinStatus, String> {
    let lock = FinLock::try_existing(orqa, fin)?;
    let pid = lock.as_ref().map(|lock| lock.pid());
    let running = pid.is_some_and(process_is_alive);
    Ok(FinStatus {
        fin: fin.label(),
        home: orqa.effective_fin_home(fin),
        sleeping: orqa.effective_fin_home(fin).join("sleep.lock").exists(),
        running,
        pid,
        unread_mail: unread_count(&orqa.effective_fin_home(fin).join("mail"))?,
        open_tasks: unread_count(&orqa.effective_fin_home(fin).join("tasks"))?,
        last_run: read_run_record_for(orqa, fin, None).ok(),
    })
}

pub(crate) fn print_pod_status(status: &PodStatus) {
    println!("pod {}", status.pod);
    println!("home={}", status.home.display());
    println!("sleeping={}", status.sleeping);
    println!("fins={}", status.fin_count);
    println!("wakeable={}", status.wakeable);
    println!("running={}", status.running);
    println!("unread_mail={}", status.unread_mail);
    println!("open_tasks={}", status.open_tasks);
    for fin in &status.fins {
        println!(
            "fin {} sleeping={} running={} unread_mail={} open_tasks={}",
            fin.fin, fin.sleeping, fin.running, fin.unread_mail, fin.open_tasks
        );
    }
}

pub(crate) fn print_pod_list_status(status: &PodStatus) {
    println!(
        "{} fins={} sleeping={} wakeable={} running={} unread_mail={} open_tasks={}",
        status.pod,
        status.fin_count,
        status.sleeping,
        status.wakeable,
        status.running,
        status.unread_mail,
        status.open_tasks
    );
}

pub(crate) fn print_fin_status(status: &FinStatus) {
    println!("fin {}", status.fin);
    println!("home={}", status.home.display());
    println!("sleeping={}", status.sleeping);
    println!("running={}", status.running);
    if let Some(pid) = status.pid {
        println!("pid={pid}");
    }
    println!("unread_mail={}", status.unread_mail);
    println!("open_tasks={}", status.open_tasks);
    if let Some(run) = &status.last_run {
        println!("last_run={}", run.id);
        println!("last_status={}", run.status);
        if let Some(exit_code) = run.exit_code {
            println!("last_exit={exit_code}");
        }
    }
}

pub(crate) fn print_json<T: Serialize>(value: &T) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(value)
            .map_err(|error| format!("failed to encode JSON: {error}"))?
    );
    Ok(())
}
