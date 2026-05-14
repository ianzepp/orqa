use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, ExitStatus},
    thread,
    time::Duration,
};

use crate::{
    cli::{
        LoopArgs, ServiceCommand, ServiceInstallArgs, ServicePodArgs, ServiceRunArgs,
        ServiceSubcommand,
    },
    model::{Orqa, PodRef},
    runtime::loop_pod,
};

pub(crate) fn service(orqa: &Orqa, command: ServiceCommand) -> Result<(), String> {
    match command.command {
        ServiceSubcommand::Install(args) => install(orqa, args),
        ServiceSubcommand::Uninstall(args) => uninstall(orqa, args),
        ServiceSubcommand::Start(args) => start(orqa, args),
        ServiceSubcommand::Stop(args) => stop(orqa, args),
        ServiceSubcommand::Status(args) => status(orqa, args),
        ServiceSubcommand::Run(args) => run(orqa, args),
    }
}

fn install(orqa: &Orqa, args: ServiceInstallArgs) -> Result<(), String> {
    validate_interval(args.interval)?;
    let pod = resolve_pod(args.pod.as_deref())?;
    ensure_pod_exists(orqa, &pod)?;
    let spec = ServiceSpec::new(orqa, &pod)?;
    fs::create_dir_all(spec.log_dir()).map_err(|error| {
        format!(
            "failed to create service log directory {}: {error}",
            spec.log_dir().display()
        )
    })?;

    match platform() {
        Platform::Macos => {
            let path = macos_plist_path(&spec)?;
            let parent = path
                .parent()
                .ok_or_else(|| format!("service path has no parent: {}", path.display()))?;
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create launch agent directory {}: {error}",
                    parent.display()
                )
            })?;
            fs::write(&path, macos_plist(&spec, &args)).map_err(|error| {
                format!("failed to write launch agent {}: {error}", path.display())
            })?;
            println!("{}", path.display());
        }
        Platform::Linux => {
            let path = linux_unit_path(&spec)?;
            let parent = path
                .parent()
                .ok_or_else(|| format!("service path has no parent: {}", path.display()))?;
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create systemd user directory {}: {error}",
                    parent.display()
                )
            })?;
            fs::write(&path, linux_unit(&spec, &args)).map_err(|error| {
                format!(
                    "failed to write systemd user unit {}: {error}",
                    path.display()
                )
            })?;
            let _ = run_status(ProcessCommand::new("systemctl").args(["--user", "daemon-reload"]));
            println!("{}", path.display());
        }
        Platform::Unsupported(os) => {
            return Err(format!("service install is not supported on {os}"));
        }
    }

    Ok(())
}

fn uninstall(orqa: &Orqa, args: ServicePodArgs) -> Result<(), String> {
    let pod = resolve_pod(args.pod.as_deref())?;
    let spec = ServiceSpec::new(orqa, &pod)?;
    let _ = stop(
        orqa,
        ServicePodArgs {
            pod: Some(pod.slug.clone()),
        },
    );

    match platform() {
        Platform::Macos => remove_if_exists(&macos_plist_path(&spec)?)?,
        Platform::Linux => {
            remove_if_exists(&linux_unit_path(&spec)?)?;
            let _ = run_status(ProcessCommand::new("systemctl").args(["--user", "daemon-reload"]));
        }
        Platform::Unsupported(os) => {
            return Err(format!("service uninstall is not supported on {os}"));
        }
    }

    println!("uninstalled {}", spec.label);
    Ok(())
}

fn start(orqa: &Orqa, args: ServicePodArgs) -> Result<(), String> {
    let pod = resolve_pod(args.pod.as_deref())?;
    let spec = ServiceSpec::new(orqa, &pod)?;

    match platform() {
        Platform::Macos => {
            let domain = launchctl_domain()?;
            checked_status(
                ProcessCommand::new("launchctl")
                    .arg("bootstrap")
                    .arg(&domain)
                    .arg(macos_plist_path(&spec)?),
            )?;
        }
        Platform::Linux => {
            checked_status(ProcessCommand::new("systemctl").args(["--user", "start", &spec.unit]))?;
        }
        Platform::Unsupported(os) => return Err(format!("service start is not supported on {os}")),
    }

    println!("started {}", spec.label);
    Ok(())
}

fn stop(orqa: &Orqa, args: ServicePodArgs) -> Result<(), String> {
    let pod = resolve_pod(args.pod.as_deref())?;
    let spec = ServiceSpec::new(orqa, &pod)?;

    match platform() {
        Platform::Macos => {
            let domain = launchctl_domain()?;
            let _ = run_status(
                ProcessCommand::new("launchctl")
                    .arg("bootout")
                    .arg(&domain)
                    .arg(macos_plist_path(&spec)?),
            );
        }
        Platform::Linux => {
            let _ =
                run_status(ProcessCommand::new("systemctl").args(["--user", "stop", &spec.unit]));
        }
        Platform::Unsupported(os) => return Err(format!("service stop is not supported on {os}")),
    }

    println!("stopped {}", spec.label);
    Ok(())
}

fn status(orqa: &Orqa, args: ServicePodArgs) -> Result<(), String> {
    let pod = resolve_pod(args.pod.as_deref())?;
    let spec = ServiceSpec::new(orqa, &pod)?;

    match platform() {
        Platform::Macos => {
            let domain = launchctl_domain()?;
            checked_status(
                ProcessCommand::new("launchctl")
                    .arg("print")
                    .arg(format!("{domain}/{}", spec.label)),
            )?;
        }
        Platform::Linux => {
            checked_status(
                ProcessCommand::new("systemctl").args(["--user", "status", &spec.unit]),
            )?;
        }
        Platform::Unsupported(os) => {
            return Err(format!("service status is not supported on {os}"));
        }
    }

    Ok(())
}

fn run(orqa: &Orqa, args: ServiceRunArgs) -> Result<(), String> {
    validate_interval(args.interval)?;
    loop {
        loop_pod(
            orqa,
            LoopArgs {
                pod: args.pod.clone(),
                force: args.force,
                framework: args.framework.clone(),
                args: args.args.clone(),
            },
        )?;
        thread::sleep(Duration::from_secs(args.interval));
    }
}

fn resolve_pod(pod: Option<&str>) -> Result<PodRef, String> {
    let pod = match pod {
        Some(pod) => pod.to_string(),
        None => env::var("ORQA_POD")
            .map_err(|_| "missing pod; pass a pod or run with ORQA_POD set".to_string())?,
    };
    PodRef::new(&pod)
}

fn validate_interval(interval: u64) -> Result<(), String> {
    if interval == 0 {
        Err("service interval must be at least 1 second".to_string())
    } else {
        Ok(())
    }
}

fn ensure_pod_exists(orqa: &Orqa, pod: &PodRef) -> Result<(), String> {
    let home = orqa.pod_home(pod);
    if home.is_dir() {
        Ok(())
    } else {
        Err(format!(
            "pod {} does not exist at {}; create it with `orqa pod create {}`",
            pod.slug,
            home.display(),
            pod.slug
        ))
    }
}

fn remove_if_exists(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path).map_err(|error| {
            format!("failed to remove service file {}: {error}", path.display())
        })?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Platform {
    Macos,
    Linux,
    Unsupported(&'static str),
}

fn platform() -> Platform {
    match env::consts::OS {
        "macos" => Platform::Macos,
        "linux" => Platform::Linux,
        os => Platform::Unsupported(os),
    }
}

struct ServiceSpec {
    label: String,
    unit: String,
    exe: PathBuf,
    home: PathBuf,
    pod: String,
}

impl ServiceSpec {
    fn new(orqa: &Orqa, pod: &PodRef) -> Result<Self, String> {
        let exe = env::current_exe()
            .map_err(|error| format!("failed to resolve current executable: {error}"))?;
        let hash = stable_hash(&orqa.home);
        let label = format!("com.ianzepp.orqa.{}.{}", pod.slug, hash);
        let unit = format!("orqa-{}-{}.service", pod.slug, hash);

        Ok(Self {
            label,
            unit,
            exe,
            home: orqa.home.clone(),
            pod: pod.slug.clone(),
        })
    }

    fn log_dir(&self) -> PathBuf {
        self.home.join("services")
    }

    fn stdout_log(&self) -> PathBuf {
        self.log_dir().join(format!("{}.out.log", self.label))
    }

    fn stderr_log(&self) -> PathBuf {
        self.log_dir().join(format!("{}.err.log", self.label))
    }
}

fn stable_hash(path: &Path) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in path.to_string_lossy().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn macos_plist_path(spec: &ServiceSpec) -> Result<PathBuf, String> {
    Ok(home_dir()?
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{}.plist", spec.label)))
}

fn linux_unit_path(spec: &ServiceSpec) -> Result<PathBuf, String> {
    Ok(home_dir()?
        .join(".config")
        .join("systemd")
        .join("user")
        .join(&spec.unit))
}

fn home_dir() -> Result<PathBuf, String> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set".to_string())
}

fn launchctl_domain() -> Result<String, String> {
    let uid = ProcessCommand::new("id")
        .arg("-u")
        .output()
        .map_err(|error| format!("failed to run id -u: {error}"))?;
    if !uid.status.success() {
        return Err("failed to determine user id with id -u".to_string());
    }
    Ok(format!(
        "gui/{}",
        String::from_utf8_lossy(&uid.stdout).trim()
    ))
}

fn service_args(spec: &ServiceSpec, args: &ServiceInstallArgs) -> Vec<String> {
    let mut command = vec![
        spec.exe.display().to_string(),
        "--home".to_string(),
        spec.home.display().to_string(),
        "service".to_string(),
        "run".to_string(),
        spec.pod.clone(),
        "--interval".to_string(),
        args.interval.to_string(),
    ];

    if args.force {
        command.push("--force".to_string());
    }

    if let Some(framework) = &args.framework {
        command.push("--framework".to_string());
        command.push(framework.to_string_lossy().to_string());
    }

    if !args.args.is_empty() {
        command.push("--".to_string());
        command.extend(
            args.args
                .iter()
                .map(|arg| arg.to_string_lossy().to_string()),
        );
    }

    command
}

fn macos_plist(spec: &ServiceSpec, args: &ServiceInstallArgs) -> String {
    let arguments = service_args(spec, args)
        .into_iter()
        .map(|arg| format!("        <string>{}</string>", xml_escape(&arg)))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
{}
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{}</string>
    <key>StandardErrorPath</key>
    <string>{}</string>
</dict>
</plist>
"#,
        xml_escape(&spec.label),
        arguments,
        xml_escape(&spec.stdout_log().to_string_lossy()),
        xml_escape(&spec.stderr_log().to_string_lossy())
    )
}

fn linux_unit(spec: &ServiceSpec, args: &ServiceInstallArgs) -> String {
    let command = service_args(spec, args)
        .into_iter()
        .map(|arg| shell_quote(&arg))
        .collect::<Vec<_>>()
        .join(" ");

    format!(
        r#"[Unit]
Description=Orqa wake-loop service for pod {}
After=default.target

[Service]
Type=simple
ExecStart=/bin/sh -lc {}
Restart=always
RestartSec=5
Environment=ORQA_HOME={}

[Install]
WantedBy=default.target
"#,
        spec.pod,
        shell_quote(&format!("exec {command}")),
        shell_quote(&spec.home.to_string_lossy()),
    )
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r#"'\''"#))
}

fn run_status(command: &mut ProcessCommand) -> Result<ExitStatus, String> {
    command
        .status()
        .map_err(|error| format!("failed to run {command:?}: {error}"))
}

fn checked_status(command: &mut ProcessCommand) -> Result<(), String> {
    let status = run_status(command)?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{command:?} exited with {status}"))
    }
}

#[cfg(test)]
#[path = "service_test.rs"]
mod tests;
