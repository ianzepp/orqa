use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::{
    cli::{CommandContext, PodDoctorArgs},
    commands::list_dirs,
    model::{FinRef, Orqa, PodRef},
    runtime::resolve_exec_command,
};

pub(crate) fn pod_doctor(
    orqa: &Orqa,
    context: &CommandContext,
    args: PodDoctorArgs,
) -> Result<(), String> {
    if args.timeout == 0 {
        return Err("pod doctor timeout must be at least 1 second".to_string());
    }

    let (slug, _) = context.resolve_pod(None, orqa)?;
    let pod = PodRef::new(&slug)?;
    orqa.ensure_pod_exists(&pod)?;
    let mut ok = true;

    let pod_root = orqa.pod_root_for_slug(&pod.slug)?;
    let pod_data = orqa.pod_data_home(&pod)?;

    check_path("pod root", &pod_root, &mut ok);
    check_path("pod config", &pod_data.join("pod.toml"), &mut ok);
    check_path("pod charter", &pod_data.join("CHARTER.md"), &mut ok);
    check_path("pod agents", &pod_data.join("AGENTS.md"), &mut ok);
    check_path("fins dir", &pod_data.join("fins"), &mut ok);

    let fins = doctor_fins(orqa, &pod, context.fin.as_deref())?;
    if fins.is_empty() {
        println!("fail pod={} check=fins detail=no-fins", pod.slug);
        ok = false;
    }

    for fin_slug in fins {
        let fin = FinRef::new(&pod.slug, &fin_slug)?;
        if !doctor_fin(orqa, &fin, &args.prompt, args.timeout)? {
            ok = false;
        }
    }

    if ok {
        println!("doctor pod={} status=ok", pod.slug);
        Ok(())
    } else {
        Err(format!("pod {} doctor failed", pod.slug))
    }
}

fn doctor_fins(orqa: &Orqa, pod: &PodRef, fin: Option<&str>) -> Result<Vec<String>, String> {
    match fin {
        Some(fin) => {
            let f = FinRef::new(&pod.slug, fin)?;
            orqa.ensure_fin_exists(&f)?;
            Ok(vec![fin.to_string()])
        }
        None => list_dirs(&orqa.pod_data_home(pod)?.join("fins")),
    }
}

fn doctor_fin(orqa: &Orqa, fin: &FinRef, prompt: &str, timeout: u64) -> Result<bool, String> {
    let mut ok = true;
    println!("fin {}", fin.label());

    let fin_home = orqa.fin_data_home(fin)?;

    check_path("fin home", &fin_home, &mut ok);
    check_path("fin config", &fin_home.join("fin.toml"), &mut ok);
    check_path("fin role", &fin_home.join("ROLE.md"), &mut ok);
    check_path("fin agents", &fin_home.join("AGENTS.md"), &mut ok);

    for path in [
        fin_home.join("mail").join("cur"),
        fin_home.join("mail").join("new"),
        fin_home.join("mail").join("tmp"),
        fin_home.join("tasks").join("cur"),
        fin_home.join("tasks").join("new"),
        fin_home.join("tasks").join("tmp"),
        fin_home.join(".codex"),
        fin_home.join(".hermes"),
        fin_home.join(".pi/agent"),
        fin_home.join(".pi/sessions"),
    ] {
        check_path("fin runtime path", &path, &mut ok);
    }

    let command = match resolve_exec_command(orqa, fin, &[OsString::from(prompt)]) {
        Ok(command) => {
            println!(
                "ok {} check=backend backend={}",
                fin.label(),
                command.backend
            );
            command
        }
        Err(error) => {
            println!(
                "fail {} check=backend detail={}",
                fin.label(),
                quote(&error)
            );
            return Ok(false);
        }
    };

    match run_probe(orqa, fin, &command.command, &command.args, timeout)? {
        ProbeOutcome::Success => println!("ok {} check=probe", fin.label()),
        ProbeOutcome::Failed(code) => {
            println!("fail {} check=probe exit_code={}", fin.label(), code);
            ok = false;
        }
        ProbeOutcome::Signaled => {
            println!("fail {} check=probe exit_code=signal", fin.label());
            ok = false;
        }
        ProbeOutcome::TimedOut => {
            println!("fail {} check=probe detail=timeout", fin.label());
            ok = false;
        }
        ProbeOutcome::SpawnFailed(error) => {
            println!("fail {} check=probe detail={}", fin.label(), quote(&error));
            ok = false;
        }
    }

    Ok(ok)
}

fn check_path(label: &str, path: &Path, ok: &mut bool) {
    if path.exists() {
        println!("ok check={} path={}", label, path.display());
    } else {
        println!("fail check={} path={}", label, path.display());
        *ok = false;
    }
}

enum ProbeOutcome {
    Success,
    Failed(i32),
    Signaled,
    TimedOut,
    SpawnFailed(String),
}

fn run_probe(
    orqa: &Orqa,
    fin: &FinRef,
    command: &OsString,
    args: &[OsString],
    timeout: u64,
) -> Result<ProbeOutcome, String> {
    let mut child = match fin_process(orqa, fin, command, args)?
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => return Ok(ProbeOutcome::SpawnFailed(error.to_string())),
    };

    let deadline = Instant::now() + Duration::from_secs(timeout);
    loop {
        match child
            .try_wait()
            .map_err(|error| format!("failed to poll backend probe: {error}"))?
        {
            Some(status) if status.success() => return Ok(ProbeOutcome::Success),
            Some(status) => {
                return Ok(match status.code() {
                    Some(code) => ProbeOutcome::Failed(code),
                    None => ProbeOutcome::Signaled,
                });
            }
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(ProbeOutcome::TimedOut);
            }
            None => thread::sleep(Duration::from_millis(100)),
        }
    }
}

fn fin_process(
    orqa: &Orqa,
    fin: &FinRef,
    command: &OsString,
    args: &[OsString],
) -> Result<ProcessCommand, String> {
    let mut process = ProcessCommand::new(command);

    let pod_root = orqa.pod_root(&PodRef {
        slug: fin.pod.clone(),
    })?;
    let fin_home = orqa.fin_data_home(fin)?;

    process
        .current_dir(&pod_root)
        .env("ORQA_HOME", &orqa.home)
        .env("ORQA_POD", &fin.pod)
        .env("ORQA_FIN", &fin.fin)
        .env("HOME", &pod_root)
        .env("CODEX_HOME", fin_home.join(".codex"))
        .env("HERMES_HOME", fin_home.join(".hermes"))
        .env("PI_CODING_AGENT_DIR", fin_home.join(".pi/agent"))
        .args(args);
    if let Some(path) = child_path_with_orqa_bin() {
        process.env("PATH", path);
    }
    Ok(process)
}

fn child_path_with_orqa_bin() -> Option<OsString> {
    let exe = env::current_exe().ok()?;
    let bin_dir = exe.parent()?;
    let mut paths = vec![PathBuf::from(bin_dir)];
    if let Some(path) = env::var_os("PATH") {
        paths.extend(env::split_paths(&path));
    }
    env::join_paths(paths).ok()
}

fn quote(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}
