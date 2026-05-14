pub(crate) fn loop_pod(orqa: &Orqa, args: LoopArgs) -> Result<(), String> {
    let pod = PodRef::new(&args.pod)?;
    if !args.force && orqa.pod_sleep_path(&pod).exists() {
        println!("skip {} sleeping=true", pod.slug);
        return Ok(());
    }

    let fins_dir = orqa.pod_home(&pod).join("fins");
    let fins = fs::read_dir(&fins_dir).map_err(|error| {
        format!(
            "failed to read fins directory {}: {error}",
            fins_dir.display()
        )
    })?;

    for entry in fins {
        let entry = entry.map_err(|error| format!("failed to read fin directory: {error}"))?;
        if !entry.path().is_dir() {
            continue;
        }

        let fin_slug = entry.file_name().to_string_lossy().to_string();
        let fin = FinRef::new(&pod.slug, &fin_slug)?;
        if !args.force && orqa.fin_sleep_path(&fin).exists() {
            println!("skip {} sleeping=true", fin.label());
            continue;
        }

        let unread_mail = unread_count(&orqa.mail_home(&fin))?;
        let open_tasks = unread_count(&orqa.task_home(&fin))?;

        if unread_mail > 0 || open_tasks > 0 {
            let wake = Wake {
                unread_mail,
                open_tasks,
            };
            wake_fin(orqa, &fin, &args.framework, &args.args, wake)?;
        }
    }

    Ok(())
}

pub(crate) fn run_fin(orqa: &Orqa, args: RunArgs) -> Result<(), String> {
    let fin = FinRef::new(&args.pod, &args.fin)?;
    run_fin_foreground(orqa, &fin, &args.framework, &args.args)
}

pub(crate) fn run_fin_foreground(
    orqa: &Orqa,
    fin: &FinRef,
    framework: &OsString,
    args: &[OsString],
) -> Result<(), String> {
    if let Some(lock) = FinLock::try_existing(orqa, fin)? {
        if lock.is_live() {
            return Err(format!(
                "fin {} is already running as pid {}",
                fin.label(),
                lock.pid
            ));
        }
        lock.remove()?;
    }

    let home = orqa.fin_home(fin);
    let codex_home = home.join(".codex");

    fs::create_dir_all(&codex_home).map_err(|error| {
        format!(
            "failed to create fin codex home {}: {error}",
            codex_home.display()
        )
    })?;

    let mut child = ProcessCommand::new(framework)
        .env("ORQA_HOME", &orqa.home)
        .env("ORQA_POD", &fin.pod)
        .env("ORQA_FIN", &fin.fin)
        .env("CODEX_HOME", &codex_home)
        .args(args)
        .spawn()
        .map_err(|error| format!("failed to run {framework:?}: {error}"))?;
    let lock = FinLock::write(orqa, fin, child.id(), framework)?;
    let status = child
        .wait()
        .map_err(|error| format!("failed to wait for {framework:?}: {error}"));
    lock.release();
    let status = status?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{framework:?} exited with {}",
            status
                .code()
                .map_or_else(|| "signal".to_string(), |code| code.to_string())
        ))
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Wake {
    unread_mail: usize,
    open_tasks: usize,
}

pub(crate) fn wake_fin(
    orqa: &Orqa,
    fin: &FinRef,
    framework: &OsString,
    args: &[OsString],
    wake: Wake,
) -> Result<(), String> {
    match FinLock::try_existing(orqa, fin)? {
        Some(lock) if lock.is_live() => {
            println!(
                "skip {} pid={} unread_mail={} open_tasks={}",
                fin.label(),
                lock.pid,
                wake.unread_mail,
                wake.open_tasks
            );
            Ok(())
        }
        Some(lock) => {
            lock.remove()?;
            spawn_wake_fin(orqa, fin, framework, args, wake)
        }
        None => spawn_wake_fin(orqa, fin, framework, args, wake),
    }
}

pub(crate) fn spawn_wake_fin(
    orqa: &Orqa,
    fin: &FinRef,
    framework: &OsString,
    args: &[OsString],
    wake: Wake,
) -> Result<(), String> {
    let home = orqa.fin_home(fin);
    let codex_home = home.join(".codex");
    fs::create_dir_all(&codex_home).map_err(|error| {
        format!(
            "failed to create fin codex home {}: {error}",
            codex_home.display()
        )
    })?;

    let child = ProcessCommand::new(framework)
        .env("ORQA_HOME", &orqa.home)
        .env("ORQA_POD", &fin.pod)
        .env("ORQA_FIN", &fin.fin)
        .env("CODEX_HOME", &codex_home)
        .args(args)
        .spawn()
        .map_err(|error| format!("failed to spawn {framework:?}: {error}"))?;
    let pid = child.id();

    FinLock::write(orqa, fin, pid, framework)?;
    println!(
        "wake {} pid={} unread_mail={} open_tasks={}",
        fin.label(),
        pid,
        wake.unread_mail,
        wake.open_tasks
    );

    Ok(())
}

pub(crate) struct FinLock {
    path: PathBuf,
    pid: u32,
}

impl FinLock {
    fn try_existing(orqa: &Orqa, fin: &FinRef) -> Result<Option<Self>, String> {
        let path = orqa.lock_path(fin);
        if !path.exists() {
            return Ok(None);
        }

        let contents = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read lock {}: {error}", path.display()))?;
        let pid = lock_pid(&contents)
            .ok_or_else(|| format!("lock {} does not contain a valid pid", path.display()))?;

        Ok(Some(Self { path, pid }))
    }

    fn write(orqa: &Orqa, fin: &FinRef, pid: u32, framework: &OsString) -> Result<Self, String> {
        let path = orqa.lock_path(fin);
        let parent = path
            .parent()
            .ok_or_else(|| format!("lock path has no parent: {}", path.display()))?;
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create lock directory {}: {error}",
                parent.display()
            )
        })?;

        let contents = format!(
            "pid={pid}\npod={}\nfin={}\nframework={:?}\n",
            fin.pod, fin.fin, framework
        );
        fs::write(&path, contents)
            .map_err(|error| format!("failed to write lock {}: {error}", path.display()))?;

        Ok(Self { path, pid })
    }

    fn is_live(&self) -> bool {
        process_is_alive(self.pid)
    }

    fn remove(&self) -> Result<(), String> {
        if self.path.exists() {
            fs::remove_file(&self.path).map_err(|error| {
                format!("failed to remove lock {}: {error}", self.path.display())
            })?;
        }

        Ok(())
    }

    fn release(self) {
        let _ = self.remove();
    }
}

pub(crate) fn lock_pid(contents: &str) -> Option<u32> {
    contents
        .lines()
        .find_map(|line| line.strip_prefix("pid=")?.parse::<u32>().ok())
}

#[cfg(unix)]
pub(crate) fn process_is_alive(pid: u32) -> bool {
    ProcessCommand::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(not(unix))]
pub(crate) fn process_is_alive(_pid: u32) -> bool {
    false
}
use std::{
    ffi::OsString,
    fs,
    path::PathBuf,
    process::{Command as ProcessCommand, Stdio},
};

use crate::{
    cli::{LoopArgs, RunArgs},
    mailbox::unread_count,
    model::{FinRef, Orqa, PodRef},
};
