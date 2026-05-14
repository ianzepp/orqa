use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[test]
fn fin_run_uses_generated_pod_config_backend() {
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
args = ["pod={{pod}}", "fin={{fin}}", "prompt={{prompt}}"]
"#
        ),
    )
    .unwrap();

    let output = orqa_output(&root, ["fin", "run", "test-pod", "amy", "--", "hello"]);

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
args = ["-c", "printf '%s' 'pod={{pod}} fin={{fin}} prompt={{prompt}}' > {{fin_home}}/ran.txt"]
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

fn temp_root() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    env::temp_dir().join(format!("orqa-pod-flow-test-{suffix}"))
}
