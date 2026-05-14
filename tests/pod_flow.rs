use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[test]
fn fin_exec_uses_generated_pod_config_backend() {
    let root = temp_root();

    orqa(&root, ["pod", "create", "test-pod"]);
    orqa(&root, ["fin", "create", "test-pod", "amy"]);

    let pod_config = root.join("pods/test-pod/pod.toml");
    let config = fs::read_to_string(&pod_config).unwrap();
    let config = config.replace("default_backend = \"codex\"", "default_backend = \"echo\"");
    fs::write(
        &pod_config,
        format!(
            r#"{config}

[backends.echo]
enabled = true
command = "/bin/echo"
exec_args = ["pod={{pod}}", "fin={{fin}}", "prompt={{prompt}}"]
"#
        ),
    )
    .unwrap();

    let output = orqa_output(&root, ["fin", "exec", "test-pod", "amy", "--", "hello"]);

    assert_eq!(output.trim(), "pod=test-pod fin=amy prompt=hello");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn loop_uses_generated_pod_config_backend_for_wakeable_fin() {
    let root = temp_root();

    orqa(&root, ["pod", "create", "test-pod"]);
    orqa(&root, ["fin", "create", "test-pod", "amy"]);

    let pod_config = root.join("pods/test-pod/pod.toml");
    let config = fs::read_to_string(&pod_config).unwrap();
    let config = config.replace(
        "default_backend = \"codex\"",
        "default_backend = \"writer\"",
    );
    fs::write(
        &pod_config,
        format!(
            r#"{config}

[backends.writer]
enabled = true
command = "/bin/sh"
exec_args = ["-c", "printf '%s' 'pod={{pod}} fin={{fin}} prompt={{prompt}}' > {{fin_home}}/ran.txt"]
"#
        ),
    )
    .unwrap();

    orqa(
        &root,
        [
            "mail",
            "send",
            "--from",
            "amy@test-pod.orqa",
            "--to",
            "amy@test-pod.orqa",
            "wake",
        ],
    );
    orqa(&root, ["loop", "test-pod", "--", "from-loop"]);

    let marker = root.join("pods/test-pod/fins/amy/ran.txt");
    for _ in 0..20 {
        if marker.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }

    assert_eq!(
        fs::read_to_string(&marker).unwrap(),
        "pod=test-pod fin=amy prompt=from-loop"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn plan_and_dry_run_explain_wake_decisions_without_running() {
    let root = temp_root();

    orqa(&root, ["pod", "create", "test-pod"]);
    orqa(&root, ["fin", "create", "test-pod", "amy"]);
    set_writer_backend(&root, "test-pod");
    orqa(
        &root,
        [
            "mail",
            "send",
            "--from",
            "amy@test-pod.orqa",
            "--to",
            "amy@test-pod.orqa",
            "wake",
        ],
    );

    let plan = orqa_output(&root, ["plan", "test-pod"]);
    assert!(plan.contains("decision=would-wake"));
    assert!(plan.contains("reason=mail"));

    let dry_run = orqa_output(&root, ["loop", "--dry-run", "test-pod"]);
    assert!(dry_run.contains("decision=would-wake"));
    assert!(!root.join("pods/test-pod/fins/amy/ran.txt").exists());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn plan_ignores_backend_errors_until_fin_is_wakeable() {
    let root = temp_root();

    orqa(&root, ["pod", "create", "test-pod"]);
    orqa(&root, ["fin", "create", "test-pod", "amy"]);
    fs::remove_file(root.join("pods/test-pod/pod.toml")).unwrap();

    let idle = orqa_output(&root, ["plan", "test-pod"]);
    assert!(idle.contains("decision=would-skip"));
    assert!(idle.contains("reason=no-action"));

    orqa(
        &root,
        [
            "mail",
            "send",
            "--from",
            "amy@test-pod.orqa",
            "--to",
            "amy@test-pod.orqa",
            "wake",
        ],
    );
    let wakeable = orqa_output(&root, ["plan", "test-pod"]);
    assert!(wakeable.contains("decision=would-skip"));
    assert!(wakeable.contains("reason=backend-error"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn fin_exec_records_status_and_tail_output() {
    let root = temp_root();

    orqa(&root, ["pod", "create", "test-pod"]);
    orqa(&root, ["fin", "create", "test-pod", "amy"]);
    set_echo_backend(&root, "test-pod");

    orqa_output(&root, ["fin", "exec", "test-pod", "amy", "--", "from-run"]);

    let runs = orqa_output(&root, ["fin", "runs", "test-pod", "amy"]);
    assert!(runs.contains("status=finished"));

    let status = orqa_output(&root, ["fin", "status", "test-pod", "amy"]);
    assert!(status.contains("last_status=finished"));

    let tail = orqa_output(&root, ["fin", "tail", "test-pod", "amy"]);
    assert!(tail.contains("from-run"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn repeated_fin_execs_get_distinct_finished_records() {
    let root = temp_root();

    orqa(&root, ["pod", "create", "test-pod"]);
    orqa(&root, ["fin", "create", "test-pod", "amy"]);
    set_echo_backend(&root, "test-pod");

    for body in ["first", "second"] {
        orqa_output(&root, ["fin", "exec", "test-pod", "amy", "--", body]);
    }

    let runs = orqa_output(&root, ["fin", "runs", "test-pod", "amy"]);
    let run_lines = runs.lines().collect::<Vec<_>>();
    assert_eq!(run_lines.len(), 2);
    assert_ne!(
        run_lines[0].split_once(' ').map(|(id, _)| id),
        run_lines[1].split_once(' ').map(|(id, _)| id)
    );
    assert!(
        run_lines
            .iter()
            .all(|line| line.contains("status=finished"))
    );

    let ledger = fs::read_to_string(root.join("pods/test-pod/fins/amy/runs.jsonl")).unwrap();
    assert_eq!(ledger.lines().count(), 2);
    assert!(
        ledger
            .lines()
            .all(|line| line.contains("\"status\":\"finished\"")
                || line.contains("\"status\": \"finished\""))
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn fin_chat_uses_chat_args_with_interactive_stdio() {
    let root = temp_root();

    orqa(&root, ["pod", "create", "test-pod"]);
    orqa(&root, ["fin", "create", "test-pod", "amy"]);

    let pod_config = root.join("pods/test-pod/pod.toml");
    let config = fs::read_to_string(&pod_config).unwrap();
    let config = config.replace("default_backend = \"codex\"", "default_backend = \"echo\"");
    fs::write(
        &pod_config,
        format!(
            r#"{config}

[backends.echo]
enabled = true
command = "/bin/echo"
exec_args = ["exec"]
chat_args = ["chat", "pod={{pod}}", "fin={{fin}}"]
"#
        ),
    )
    .unwrap();

    let output = orqa_output(&root, ["fin", "chat", "test-pod", "amy"]);
    assert_eq!(output.trim(), "chat pod=test-pod fin=amy");

    let runs = orqa_output(&root, ["fin", "runs", "test-pod", "amy"]);
    assert!(runs.contains("mode=chat"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn pod_and_fin_list_print_sorted_slugs() {
    let root = temp_root();

    orqa(&root, ["pod", "create", "zeta-pod"]);
    orqa(&root, ["pod", "create", "alpha-pod"]);
    orqa(&root, ["fin", "create", "alpha-pod", "zoe"]);
    orqa(&root, ["fin", "create", "alpha-pod", "amy"]);

    assert_eq!(orqa_output(&root, ["pod", "list"]), "alpha-pod\nzeta-pod\n");
    assert_eq!(
        orqa_output(&root, ["fin", "list", "alpha-pod"]),
        "amy\nzoe\n"
    );

    let output = command(&root, ["fin", "list"])
        .env("ORQA_POD", "alpha-pod")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "orqa failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "amy\nzoe\n");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn fin_list_without_pod_context_explains_missing_pod() {
    let root = temp_root();

    let output = command(&root, ["fin", "list"]).output().unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("missing pod"));

    fs::remove_dir_all(root).unwrap_or(());
}

#[test]
fn service_run_scans_all_pods() {
    let root = temp_root();

    for pod in ["alpha-pod", "beta-pod"] {
        orqa(&root, ["pod", "create", pod]);
        orqa(&root, ["fin", "create", pod, "amy"]);
        set_writer_backend(&root, pod);
        orqa(
            &root,
            [
                "mail",
                "send",
                "--from",
                &format!("amy@{pod}.orqa"),
                "--to",
                &format!("amy@{pod}.orqa"),
                "wake",
            ],
        );
    }

    let mut child = command(
        &root,
        ["service", "run", "--interval", "1", "--", "from-service"],
    )
    .spawn()
    .unwrap();

    let alpha_marker = root.join("pods/alpha-pod/fins/amy/ran.txt");
    let beta_marker = root.join("pods/beta-pod/fins/amy/ran.txt");
    for _ in 0..40 {
        if alpha_marker.exists() && beta_marker.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    stop_child(&mut child);

    assert_eq!(
        fs::read_to_string(&alpha_marker).unwrap(),
        "pod=alpha-pod fin=amy prompt=from-service"
    );
    assert_eq!(
        fs::read_to_string(&beta_marker).unwrap(),
        "pod=beta-pod fin=amy prompt=from-service"
    );

    fs::remove_dir_all(root).unwrap();
}

fn orqa<const N: usize>(root: &Path, args: [&str; N]) {
    let output = command(root, args).output().unwrap();
    assert!(
        output.status.success(),
        "orqa failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn orqa_output<const N: usize>(root: &Path, args: [&str; N]) -> String {
    let output = command(root, args).output().unwrap();
    assert!(
        output.status.success(),
        "orqa failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn command<const N: usize>(root: &Path, args: [&str; N]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_orqa"));
    command.arg("--home").arg(root).args(args);
    command
}

fn set_writer_backend(root: &Path, pod: &str) {
    let pod_config = root.join(format!("pods/{pod}/pod.toml"));
    let config = fs::read_to_string(&pod_config).unwrap();
    let config = config.replace(
        "default_backend = \"codex\"",
        "default_backend = \"writer\"",
    );
    fs::write(
        &pod_config,
        format!(
            r#"{config}

[backends.writer]
enabled = true
command = "/bin/sh"
exec_args = ["-c", "printf '%s' 'pod={{pod}} fin={{fin}} prompt={{prompt}}' > {{fin_home}}/ran.txt"]
"#
        ),
    )
    .unwrap();
}

fn set_echo_backend(root: &Path, pod: &str) {
    let pod_config = root.join(format!("pods/{pod}/pod.toml"));
    let config = fs::read_to_string(&pod_config).unwrap();
    let config = config.replace("default_backend = \"codex\"", "default_backend = \"echo\"");
    fs::write(
        &pod_config,
        format!(
            r#"{config}

[backends.echo]
enabled = true
command = "/bin/echo"
exec_args = ["{{prompt}}"]
"#
        ),
    )
    .unwrap();
}

fn stop_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn temp_root() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    env::temp_dir().join(format!(
        "orqa-pod-flow-test-{}-{suffix}-{counter}",
        std::process::id(),
    ))
}
