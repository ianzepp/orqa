use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

use toml::{Table, Value};

use crate::{
    cli::{PodHookAddArgs, PodHookListArgs, PodHookRefArgs, PodHookRunArgs},
    model::{Orqa, PodRef, validate_slug},
};

const PRE_PLAN: &str = "pre-plan";

pub(crate) fn list_hooks(orqa: &Orqa, args: PodHookListArgs) -> Result<(), String> {
    let pod = PodRef::new(&args.pod)?;
    orqa.ensure_pod_exists(&pod)?;
    let hooks = read_phase_hooks(orqa, &pod, PRE_PLAN)?;
    for hook in hooks {
        println!(
            "{} {} enabled={} timeout={} command={} path={}",
            hook.phase,
            hook.id,
            hook.enabled,
            hook.timeout,
            hook.command,
            hook.path.display()
        );
    }
    Ok(())
}

pub(crate) fn add_hook(orqa: &Orqa, args: PodHookAddArgs) -> Result<(), String> {
    let pod = PodRef::new(&args.pod)?;
    orqa.ensure_pod_exists(&pod)?;
    validate_phase(&args.phase)?;
    validate_slug(&args.hook)?;
    let timeout = parse_duration(&args.timeout)
        .map_err(|error| format!("invalid hook timeout {:?}: {error}", args.timeout))?;
    if timeout.is_zero() {
        return Err("hook timeout must be at least 1 second".to_string());
    }

    let phase_home = orqa.pod_hook_phase_home(&pod, &args.phase);
    fs::create_dir_all(&phase_home).map_err(|error| {
        format!(
            "failed to create hook phase directory {}: {error}",
            phase_home.display()
        )
    })?;

    let command = shell_join(&args.command)?;
    let path = phase_home.join(format!("{}.toml", args.hook));
    let script = phase_home.join(format!("{}.sh", args.hook));
    let toml = format!(
        "[hook]\nenabled = true\ncommand = {:?}\ntimeout = {:?}\n",
        command, args.timeout
    );
    write_file(&path, toml.as_bytes())?;
    if !script.exists() {
        write_file(
            &script,
            b"#!/usr/bin/env sh\nset -eu\n\n# Fill in this hook script, or update command in the adjacent TOML.\n",
        )?;
        make_executable(&script)?;
    }

    println!("{}", path.display());
    println!("{}", script.display());
    Ok(())
}

pub(crate) fn enable_hook(orqa: &Orqa, args: PodHookRefArgs) -> Result<(), String> {
    set_hook_enabled(orqa, args, true)
}

pub(crate) fn disable_hook(orqa: &Orqa, args: PodHookRefArgs) -> Result<(), String> {
    set_hook_enabled(orqa, args, false)
}

pub(crate) fn remove_hook(orqa: &Orqa, args: PodHookRefArgs) -> Result<(), String> {
    let pod = PodRef::new(&args.pod)?;
    orqa.ensure_pod_exists(&pod)?;
    validate_phase(&args.phase)?;
    validate_slug(&args.hook)?;
    let phase_home = orqa.pod_hook_phase_home(&pod, &args.phase);
    let path = phase_home.join(format!("{}.toml", args.hook));
    let script = phase_home.join(format!("{}.sh", args.hook));
    remove_if_exists(&path)?;
    remove_if_exists(&script)?;
    println!("removed {} {}/{}", pod.slug, args.phase, args.hook);
    Ok(())
}

pub(crate) fn run_hooks(orqa: &Orqa, args: PodHookRunArgs) -> Result<(), String> {
    let pod = PodRef::new(&args.pod)?;
    orqa.ensure_pod_exists(&pod)?;
    run_hook_phase(orqa, &pod, &args.phase)
}

pub(crate) fn run_hook_phase(orqa: &Orqa, pod: &PodRef, phase: &str) -> Result<(), String> {
    validate_phase(phase)?;
    for hook in read_phase_hooks(orqa, pod, phase)? {
        if !hook.enabled {
            continue;
        }
        match run_one_hook(orqa, pod, &hook) {
            Ok(HookRun::Ok) => println!("hook {} {}/{} status=ok", pod.slug, phase, hook.id),
            Ok(HookRun::Failed(status)) => println!(
                "hook {} {}/{} status=failed exit={}",
                pod.slug,
                phase,
                hook.id,
                exit_label(status)
            ),
            Ok(HookRun::TimedOut) => {
                println!("hook {} {}/{} status=timeout", pod.slug, phase, hook.id)
            }
            Err(error) => println!(
                "hook {} {}/{} status=error detail={error}",
                pod.slug, phase, hook.id
            ),
        }
    }
    Ok(())
}

fn set_hook_enabled(orqa: &Orqa, args: PodHookRefArgs, enabled: bool) -> Result<(), String> {
    let pod = PodRef::new(&args.pod)?;
    validate_phase(&args.phase)?;
    validate_slug(&args.hook)?;
    let path = orqa
        .pod_hook_phase_home(&pod, &args.phase)
        .join(format!("{}.toml", args.hook));
    let mut table = read_hook_table(&path)?;
    let hook = hook_table_mut(&mut table)?;
    hook.insert("enabled".to_string(), Value::Boolean(enabled));
    write_file(&path, table.to_string().as_bytes())?;
    println!(
        "{} {} {}/{}",
        if enabled { "enabled" } else { "disabled" },
        pod.slug,
        args.phase,
        args.hook
    );
    Ok(())
}

fn read_phase_hooks(orqa: &Orqa, pod: &PodRef, phase: &str) -> Result<Vec<Hook>, String> {
    validate_phase(phase)?;
    let phase_home = orqa.pod_hook_phase_home(pod, phase);
    if !phase_home.exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(&phase_home).map_err(|error| {
        format!(
            "failed to read hook directory {}: {error}",
            phase_home.display()
        )
    })? {
        let entry = entry.map_err(|error| format!("failed to read hook entry: {error}"))?;
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "toml")
        {
            paths.push(path);
        }
    }
    paths.sort();

    let mut hooks = Vec::new();
    for path in paths {
        hooks.push(read_hook(&path, phase)?);
    }
    Ok(hooks)
}

fn read_hook(path: &Path, phase: &str) -> Result<Hook, String> {
    let table = read_hook_table(path)?;
    let hook = hook_table(&table)?;
    let command = string_field(hook, "command")?;
    let timeout = string_field(hook, "timeout")?;
    parse_duration(&timeout).map_err(|error| {
        format!(
            "invalid hook timeout {:?} in {}: {error}",
            timeout,
            path.display()
        )
    })?;
    let id = path
        .file_stem()
        .ok_or_else(|| format!("hook path has no filename: {}", path.display()))?
        .to_string_lossy()
        .to_string();
    validate_slug(&id)?;
    Ok(Hook {
        id,
        phase: phase.to_string(),
        enabled: bool_field(hook, "enabled").unwrap_or(true),
        command,
        timeout,
        path: path.to_path_buf(),
    })
}

fn run_one_hook(orqa: &Orqa, pod: &PodRef, hook: &Hook) -> Result<HookRun, String> {
    let timeout = parse_duration(&hook.timeout)?;
    let phase_home = orqa.pod_hook_phase_home(pod, &hook.phase);
    let state_home = orqa.pod_hook_state_home(pod, &hook.id);
    fs::create_dir_all(&state_home).map_err(|error| {
        format!(
            "failed to create hook state directory {}: {error}",
            state_home.display()
        )
    })?;

    let mut child = ProcessCommand::new("/bin/sh")
        .arg("-c")
        .arg(&hook.command)
        .current_dir(&phase_home)
        .env("ORQA_HOME", &orqa.home)
        .env("ORQA_POD", &pod.slug)
        .env("ORQA_POD_HOME", orqa.pod_home(pod))
        .env("ORQA_HOOK", &hook.id)
        .env("ORQA_HOOK_PHASE", &hook.phase)
        .env("ORQA_HOOK_HOME", &phase_home)
        .env("ORQA_HOOK_STATE", &state_home)
        .stdin(Stdio::null())
        .spawn()
        .map_err(|error| format!("failed to start hook command {:?}: {error}", hook.command))?;

    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("failed to wait for hook {}: {error}", hook.id))?
        {
            return Ok(if status.success() {
                HookRun::Ok
            } else {
                HookRun::Failed(status)
            });
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(HookRun::TimedOut);
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn read_hook_table(path: &Path) -> Result<Table, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    text.parse::<Table>()
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn hook_table(table: &Table) -> Result<&Table, String> {
    table
        .get("hook")
        .and_then(Value::as_table)
        .ok_or_else(|| "hook TOML must contain a [hook] table".to_string())
}

fn hook_table_mut(table: &mut Table) -> Result<&mut Table, String> {
    table
        .get_mut("hook")
        .and_then(Value::as_table_mut)
        .ok_or_else(|| "hook TOML must contain a [hook] table".to_string())
}

fn string_field(table: &Table, key: &str) -> Result<String, String> {
    table
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| format!("hook {key} must be a string"))
}

fn bool_field(table: &Table, key: &str) -> Option<bool> {
    table.get(key)?.as_bool()
}

fn validate_phase(phase: &str) -> Result<(), String> {
    if phase == PRE_PLAN {
        Ok(())
    } else {
        Err(format!(
            "invalid hook phase {phase:?}; currently supported: {PRE_PLAN}"
        ))
    }
}

fn parse_duration(value: &str) -> Result<Duration, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("duration cannot be empty".to_string());
    }
    let split = value
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(value.len());
    let number = value[..split]
        .parse::<u64>()
        .map_err(|_| "duration must start with a positive integer".to_string())?;
    let unit = value[split..].trim().to_ascii_lowercase();
    let seconds = match unit.as_str() {
        "" | "s" | "sec" | "secs" | "second" | "seconds" => number,
        "m" | "min" | "mins" | "minute" | "minutes" => number * 60,
        "h" | "hr" | "hrs" | "hour" | "hours" => number * 60 * 60,
        "d" | "day" | "days" => number * 60 * 60 * 24,
        _ => return Err("use a duration like 30s, 5m, 3h, or 1 day".to_string()),
    };
    Ok(Duration::from_secs(seconds))
}

fn shell_join(args: &[OsString]) -> Result<String, String> {
    if args.is_empty() {
        return Err("hook command is required after --".to_string());
    }
    args.iter()
        .map(shell_quote)
        .collect::<Result<Vec<_>, _>>()
        .map(|parts| parts.join(" "))
}

fn shell_quote(arg: &OsString) -> Result<String, String> {
    let text = arg
        .to_str()
        .ok_or_else(|| "hook command arguments must be valid UTF-8".to_string())?;
    if text.bytes().all(|byte| {
        byte.is_ascii_alphanumeric()
            || matches!(byte, b'/' | b'.' | b'-' | b'_' | b':' | b'=' | b'+')
    }) {
        Ok(text.to_string())
    } else {
        Ok(format!("'{}'", text.replace('\'', "'\\''")))
    }
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::write(path, bytes).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn remove_if_exists(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("failed to remove {}: {error}", path.display())),
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)
        .map_err(|error| format!("failed to read {} metadata: {error}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .map_err(|error| format!("failed to chmod {}: {error}", path.display()))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn exit_label(status: ExitStatus) -> String {
    status
        .code()
        .map_or_else(|| "signal".to_string(), |code| code.to_string())
}

struct Hook {
    id: String,
    phase: String,
    enabled: bool,
    command: String,
    timeout: String,
    path: PathBuf,
}

enum HookRun {
    Ok,
    Failed(ExitStatus),
    TimedOut,
}
