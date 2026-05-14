use std::{
    env,
    ffi::OsString,
    fs, io,
    path::PathBuf,
    process::{Command as ProcessCommand, Stdio},
    time::Duration,
};

use serde::Serialize;

use crate::{
    cli::{ChatArgs, ExecArgs, LoopArgs, PlanArgs, SuperviseArgs},
    config::{BackendCommand, BackendMode, backend_chat_command, backend_command, run_policy},
    hooks::run_hook_phase,
    mailbox::unread_count,
    model::{FinRef, Orqa, PodRef},
    runs::{RunFiles, latest_run_started_at},
    runtime_home::ensure_fin_runtime_homes,
    status::print_json,
};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct WakePlan {
    pub(crate) pod: String,
    pub(crate) pod_sleeping: bool,
    pub(crate) fins: Vec<FinWakePlan>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct FinWakePlan {
    pub(crate) fin: String,
    pub(crate) decision: WakeDecision,
    pub(crate) reason: WakeReason,
    pub(crate) fin_sleeping: bool,
    pub(crate) running: bool,
    pub(crate) pid: Option<u32>,
    pub(crate) unread_mail: usize,
    pub(crate) open_tasks: usize,
    pub(crate) backend: Option<String>,
    pub(crate) detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WakeDecision {
    WouldWake,
    WouldSkip,
}

impl std::fmt::Display for WakeDecision {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WouldWake => formatter.write_str("would-wake"),
            Self::WouldSkip => formatter.write_str("would-skip"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WakeReason {
    Mail,
    Task,
    MailAndTask,
    PodSleeping,
    FinSleeping,
    AlreadyRunning,
    NoAction,
    Debounced,
    ExecAlways,
    ConfigError,
    BackendError,
}

impl std::fmt::Display for WakeReason {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mail => formatter.write_str("mail"),
            Self::Task => formatter.write_str("task"),
            Self::MailAndTask => formatter.write_str("mail-and-task"),
            Self::PodSleeping => formatter.write_str("pod-sleeping"),
            Self::FinSleeping => formatter.write_str("fin-sleeping"),
            Self::AlreadyRunning => formatter.write_str("already-running"),
            Self::NoAction => formatter.write_str("no-action"),
            Self::Debounced => formatter.write_str("debounced"),
            Self::ExecAlways => formatter.write_str("exec-always"),
            Self::ConfigError => formatter.write_str("config-error"),
            Self::BackendError => formatter.write_str("backend-error"),
        }
    }
}

pub(crate) fn loop_pod(orqa: &Orqa, args: LoopArgs) -> Result<(), String> {
    let pod = PodRef::new(&args.pod)?;
    if !args.dry_run {
        run_hook_phase(orqa, &pod, "pre-plan")?;
    }
    let plan = plan_pod(orqa, &args.pod, args.force, &args.args)?;
    if args.dry_run {
        return print_plan(&plan, args.json);
    }

    for fin in &plan.fins {
        if fin.decision != WakeDecision::WouldWake {
            if fin.reason != WakeReason::NoAction {
                println!(
                    "skip {} reason={} unread_mail={} open_tasks={}",
                    fin.fin, fin.reason, fin.unread_mail, fin.open_tasks
                );
            }
            continue;
        }

        let fin_ref = FinRef::new(&plan.pod, &fin.fin)?;
        let command = resolve_exec_command(orqa, &fin_ref, &args.args)?;
        spawn_supervised_wake(orqa, &fin_ref, &command, fin)?;
    }

    Ok(())
}

pub(crate) fn plan(orqa: &Orqa, args: PlanArgs) -> Result<(), String> {
    let plan = plan_pod(orqa, &args.pod, args.force, &[])?;
    print_plan(&plan, args.json)
}

pub(crate) fn plan_pod(
    orqa: &Orqa,
    pod: &str,
    force: bool,
    args: &[OsString],
) -> Result<WakePlan, String> {
    let pod = PodRef::new(pod)?;
    let pod_sleeping = orqa.pod_sleep_path(&pod).exists();
    let mut fins = Vec::new();
    let fins_dir = orqa.pod_home(&pod).join("fins");

    let entries = fs::read_dir(&fins_dir).map_err(|error| {
        format!(
            "failed to read fins directory {}: {error}",
            fins_dir.display()
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| format!("failed to read fin directory: {error}"))?;
        if !entry.path().is_dir() {
            continue;
        }

        let fin_slug = entry.file_name().to_string_lossy().to_string();
        let fin = FinRef::new(&pod.slug, &fin_slug)?;
        fins.push(plan_fin(orqa, &fin, pod_sleeping, force, args)?);
    }
    fins.sort_by(|left, right| left.fin.cmp(&right.fin));

    Ok(WakePlan {
        pod: pod.slug,
        pod_sleeping,
        fins,
    })
}

fn plan_fin(
    orqa: &Orqa,
    fin: &FinRef,
    pod_sleeping: bool,
    force: bool,
    args: &[OsString],
) -> Result<FinWakePlan, String> {
    let fin_sleeping = orqa.fin_sleep_path(fin).exists();
    let unread_mail = unread_count(&orqa.mail_home(fin))?;
    let open_tasks = unread_count(&orqa.task_home(fin))?;
    let lock = FinLock::try_existing(orqa, fin)?;
    let pid = lock.as_ref().map(|lock| lock.pid);
    let running = lock.as_ref().is_some_and(FinLock::is_live);
    let policy = match run_policy(orqa, fin) {
        Ok(policy) => policy,
        Err(error) => {
            return Ok(FinWakePlan {
                fin: fin.fin.clone(),
                decision: WakeDecision::WouldSkip,
                reason: WakeReason::ConfigError,
                fin_sleeping,
                running,
                pid,
                unread_mail,
                open_tasks,
                backend: None,
                detail: Some(error),
            });
        }
    };
    let last_run_age = latest_run_age(orqa, fin)?;

    let (decision, reason, detail) = if pod_sleeping && !force {
        (WakeDecision::WouldSkip, WakeReason::PodSleeping, None)
    } else if fin_sleeping && !force {
        (WakeDecision::WouldSkip, WakeReason::FinSleeping, None)
    } else if running {
        (WakeDecision::WouldSkip, WakeReason::AlreadyRunning, None)
    } else if let Some(debounce) = policy.debounce {
        if has_work(unread_mail, open_tasks) && last_run_age.is_some_and(|age| age < debounce) {
            (
                WakeDecision::WouldSkip,
                WakeReason::Debounced,
                Some(format!(
                    "age={} debounce={}",
                    format_duration(last_run_age.unwrap_or_default()),
                    format_duration(debounce)
                )),
            )
        } else {
            work_or_idle_decision(unread_mail, open_tasks, policy.exec_always, last_run_age)
        }
    } else {
        work_or_idle_decision(unread_mail, open_tasks, policy.exec_always, last_run_age)
    };
    let backend = if decision == WakeDecision::WouldWake {
        match resolve_exec_command(orqa, fin, args) {
            Ok(command) => Some(command.backend),
            Err(error) => {
                return Ok(FinWakePlan {
                    fin: fin.fin.clone(),
                    decision: WakeDecision::WouldSkip,
                    reason: WakeReason::BackendError,
                    fin_sleeping,
                    running,
                    pid,
                    unread_mail,
                    open_tasks,
                    backend: None,
                    detail: Some(error),
                });
            }
        }
    } else {
        None
    };

    Ok(FinWakePlan {
        fin: fin.fin.clone(),
        decision,
        reason,
        fin_sleeping,
        running,
        pid,
        unread_mail,
        open_tasks,
        backend,
        detail,
    })
}

fn work_or_idle_decision(
    unread_mail: usize,
    open_tasks: usize,
    exec_always: Option<Duration>,
    last_run_age: Option<Duration>,
) -> (WakeDecision, WakeReason, Option<String>) {
    if unread_mail > 0 && open_tasks > 0 {
        (WakeDecision::WouldWake, WakeReason::MailAndTask, None)
    } else if unread_mail > 0 {
        (WakeDecision::WouldWake, WakeReason::Mail, None)
    } else if open_tasks > 0 {
        (WakeDecision::WouldWake, WakeReason::Task, None)
    } else if let Some(exec_always) = exec_always {
        if last_run_age.is_none_or(|age| age >= exec_always) {
            (
                WakeDecision::WouldWake,
                WakeReason::ExecAlways,
                Some(format!("exec_always={}", format_duration(exec_always))),
            )
        } else {
            (
                WakeDecision::WouldSkip,
                WakeReason::NoAction,
                Some(format!(
                    "age={} exec_always={}",
                    format_duration(last_run_age.unwrap_or_default()),
                    format_duration(exec_always)
                )),
            )
        }
    } else {
        (WakeDecision::WouldSkip, WakeReason::NoAction, None)
    }
}

fn has_work(unread_mail: usize, open_tasks: usize) -> bool {
    unread_mail > 0 || open_tasks > 0
}

fn latest_run_age(orqa: &Orqa, fin: &FinRef) -> Result<Option<Duration>, String> {
    let Some(started_at) = latest_run_started_at(orqa, fin)? else {
        return Ok(None);
    };
    Ok(started_at.elapsed().ok())
}

fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds >= 60 * 60 && seconds % (60 * 60) == 0 {
        format!("{}h", seconds / (60 * 60))
    } else if seconds >= 60 && seconds % 60 == 0 {
        format!("{}m", seconds / 60)
    } else {
        format!("{seconds}s")
    }
}

fn print_plan(plan: &WakePlan, json: bool) -> Result<(), String> {
    if json {
        return print_json(plan);
    }

    if plan.pod_sleeping {
        println!("pod {} sleeping=true", plan.pod);
    }
    for fin in &plan.fins {
        let detail = fin
            .detail
            .as_ref()
            .map(|detail| format!(" detail={detail}"))
            .unwrap_or_default();
        println!(
            "pod={} fin={} decision={} reason={} unread_mail={} open_tasks={} sleeping={} running={}{}",
            plan.pod,
            fin.fin,
            fin.decision,
            fin.reason,
            fin.unread_mail,
            fin.open_tasks,
            fin.fin_sleeping,
            fin.running,
            detail
        );
    }
    Ok(())
}

pub(crate) fn exec_fin(orqa: &Orqa, args: ExecArgs) -> Result<(), String> {
    let fin = FinRef::new(&args.pod, &args.fin)?;
    let command = resolve_exec_command(orqa, &fin, &args.args)?;
    exec_fin_foreground(orqa, &fin, &command)
}

pub(crate) fn chat_fin(orqa: &Orqa, args: ChatArgs) -> Result<(), String> {
    let fin = FinRef::new(&args.pod, &args.fin)?;
    let command = resolve_chat_command(orqa, &fin, &args.args)?;
    fin_chat_interactive(orqa, &fin, &command)
}

pub(crate) fn supervise_fin(orqa: &Orqa, args: SuperviseArgs) -> Result<(), String> {
    let fin = FinRef::new(&args.pod, &args.fin)?;
    let command = BackendCommand {
        backend: args.backend,
        command: args.backend_command,
        args: args.args,
        mode: BackendMode::Exec,
    };
    let outcome = exec_fin_logged(orqa, &fin, &command, false)?;
    if outcome.success {
        Ok(())
    } else {
        Err(exit_error(&command.command, outcome.code))
    }
}

fn spawn_supervised_wake(
    orqa: &Orqa,
    fin: &FinRef,
    command: &BackendCommand,
    wake: &FinWakePlan,
) -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|error| format!("failed to resolve current executable: {error}"))?;
    let mut child = ProcessCommand::new(exe)
        .arg("--home")
        .arg(&orqa.home)
        .arg("fin")
        .arg("supervise")
        .arg(&fin.pod)
        .arg(&fin.fin)
        .args(supervisor_args(command))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            format!(
                "failed to spawn supervised wake for {}: {error}",
                fin.label()
            )
        })?;
    let pid = child.id();
    let _ = child.try_wait();
    println!(
        "wake {} pid={} unread_mail={} open_tasks={}",
        fin.label(),
        pid,
        wake.unread_mail,
        wake.open_tasks
    );
    Ok(())
}

fn supervisor_args(command: &BackendCommand) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("--backend"),
        OsString::from(&command.backend),
        OsString::from("--backend-command"),
        command.command.clone(),
    ];
    if !command.args.is_empty() {
        args.push(OsString::from("--"));
        args.extend(command.args.clone());
    }
    args
}

fn exec_fin_foreground(orqa: &Orqa, fin: &FinRef, command: &BackendCommand) -> Result<(), String> {
    let outcome = exec_fin_logged(orqa, fin, command, true)?;
    io::copy(&mut outcome.stdout.as_slice(), &mut io::stdout())
        .map_err(|error| format!("failed to write stdout: {error}"))?;
    io::copy(&mut outcome.stderr.as_slice(), &mut io::stderr())
        .map_err(|error| format!("failed to write stderr: {error}"))?;

    if outcome.success {
        return Ok(());
    }

    Err(exit_error(&command.command, outcome.code))
}

fn exit_error(command: &OsString, code: Option<i32>) -> String {
    format!(
        "{command:?} exited with {}",
        code.map_or_else(|| "signal".to_string(), |code| code.to_string())
    )
}

pub(crate) fn resolve_exec_command(
    orqa: &Orqa,
    fin: &FinRef,
    args: &[OsString],
) -> Result<BackendCommand, String> {
    backend_command(orqa, fin, args)
}

fn resolve_chat_command(
    orqa: &Orqa,
    fin: &FinRef,
    args: &[OsString],
) -> Result<BackendCommand, String> {
    let mut command = backend_chat_command(orqa, fin)?;
    if !args.is_empty() {
        command.args.extend(args.iter().cloned());
    }
    Ok(command)
}

pub(crate) fn exec_fin_logged(
    orqa: &Orqa,
    fin: &FinRef,
    command: &BackendCommand,
    capture_output: bool,
) -> Result<RunOutcome, String> {
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

    ensure_runtime_homes(orqa, fin)?;
    let run = RunFiles::create(
        orqa,
        fin,
        command.mode.as_str(),
        &command.backend,
        &command.command,
        &command.args,
    )?;

    let output = if capture_output {
        let mut child = fin_process(orqa, fin, command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                let _ = run.mark_spawn_failed(&error.to_string());
                format!("failed to run {:?}: {error}", command.command)
            })?;
        let lock = write_child_lock(orqa, fin, &mut child, &command.command, &run)?;
        run.mark_spawned(child.id())?;
        let output = child
            .wait_with_output()
            .map_err(|error| format!("failed to wait for {:?}: {error}", command.command));
        lock.release();
        let output = output?;
        run.mark_finished(&output)?;
        RunOutcome {
            success: output.status.success(),
            code: output.status.code(),
            stdout: output.stdout,
            stderr: output.stderr,
        }
    } else {
        let stdout = run.stdout_file()?;
        let stderr = run.stderr_file()?;
        let mut child = fin_process(orqa, fin, command)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .map_err(|error| {
                let _ = run.mark_spawn_failed(&error.to_string());
                format!("failed to spawn {:?}: {error}", command.command)
            })?;
        let lock = write_child_lock(orqa, fin, &mut child, &command.command, &run)?;
        run.mark_spawned(child.id())?;
        let status = child
            .wait()
            .map_err(|error| format!("failed to wait for {:?}: {error}", command.command));
        lock.release();
        let status = status?;
        run.mark_finished_status(status)?;
        RunOutcome {
            success: status.success(),
            code: status.code(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        }
    };

    Ok(output)
}

pub(crate) struct RunOutcome {
    success: bool,
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn fin_chat_interactive(orqa: &Orqa, fin: &FinRef, command: &BackendCommand) -> Result<(), String> {
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

    ensure_runtime_homes(orqa, fin)?;
    let run = RunFiles::create(
        orqa,
        fin,
        command.mode.as_str(),
        &command.backend,
        &command.command,
        &command.args,
    )?;
    let mut child = fin_process(orqa, fin, command).spawn().map_err(|error| {
        let _ = run.mark_spawn_failed(&error.to_string());
        format!(
            "failed to start interactive chat {:?}: {error}",
            command.command
        )
    })?;
    let lock = write_child_lock(orqa, fin, &mut child, &command.command, &run)?;
    run.mark_spawned(child.id())?;
    let status = child
        .wait()
        .map_err(|error| format!("failed to wait for {:?}: {error}", command.command));
    lock.release();
    let status = status?;
    run.mark_finished_status(status)?;
    if status.success() {
        Ok(())
    } else {
        Err(exit_error(&command.command, status.code()))
    }
}

fn ensure_runtime_homes(orqa: &Orqa, fin: &FinRef) -> Result<(), String> {
    ensure_fin_runtime_homes(orqa, fin)
}

fn fin_process(orqa: &Orqa, fin: &FinRef, command: &BackendCommand) -> ProcessCommand {
    let mut process = ProcessCommand::new(&command.command);
    let fin_home = orqa.fin_home(fin);
    process
        .current_dir(&fin_home)
        .env("ORQA_HOME", &orqa.home)
        .env("ORQA_POD", &fin.pod)
        .env("ORQA_FIN", &fin.fin)
        .env("CODEX_HOME", fin_home.join(".codex"))
        .env("HERMES_HOME", fin_home.join(".hermes"))
        .env("PI_CODING_AGENT_DIR", fin_home.join(".pi/agent"))
        .args(&command.args);
    if let Some(path) = child_path_with_orqa_bin() {
        process.env("PATH", path);
    }
    process
}

fn child_path_with_orqa_bin() -> Option<OsString> {
    let exe = env::current_exe().ok()?;
    let bin_dir = exe.parent()?;
    let mut paths = vec![bin_dir.to_path_buf()];
    if let Some(path) = env::var_os("PATH") {
        paths.extend(env::split_paths(&path));
    }
    env::join_paths(paths).ok()
}

fn write_child_lock(
    orqa: &Orqa,
    fin: &FinRef,
    child: &mut std::process::Child,
    command: &OsString,
    run: &RunFiles,
) -> Result<FinLock, String> {
    match FinLock::write(orqa, fin, child.id(), command) {
        Ok(lock) => Ok(lock),
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = run.mark_spawn_failed(&error);
            Err(error)
        }
    }
}

pub(crate) struct FinLock {
    path: PathBuf,
    pid: u32,
}

impl FinLock {
    pub(crate) fn try_existing(orqa: &Orqa, fin: &FinRef) -> Result<Option<Self>, String> {
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

    fn write(orqa: &Orqa, fin: &FinRef, pid: u32, command: &OsString) -> Result<Self, String> {
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
            "pid={pid}\npod={}\nfin={}\ncommand={:?}\n",
            fin.pod, fin.fin, command
        );
        fs::write(&path, contents)
            .map_err(|error| format!("failed to write lock {}: {error}", path.display()))?;

        Ok(Self { path, pid })
    }

    pub(crate) fn pid(&self) -> u32 {
        self.pid
    }

    pub(crate) fn is_live(&self) -> bool {
        process_is_alive(self.pid)
    }

    pub(crate) fn remove(&self) -> Result<(), String> {
        if self.path.exists() {
            fs::remove_file(&self.path).map_err(|error| {
                format!("failed to remove lock {}: {error}", self.path.display())
            })?;
        }

        Ok(())
    }

    pub(crate) fn release(self) {
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
