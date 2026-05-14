use std::{
    env,
    ffi::OsString,
    fs,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, SystemTime},
};

use super::*;
use crate::model::{FinRef, Orqa};

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[test]
fn resolves_backend_command_from_pod_and_fin_config() {
    let root = temp_root();
    let orqa = Orqa::new(Some(root.clone()));
    let fin = FinRef::new("test-pod", "amy").unwrap();
    let fin_home = orqa.fin_home(&fin);

    fs::create_dir_all(&fin_home).unwrap();
    fs::write(
        orqa.pod_home(&PodRef::new("test-pod").unwrap())
            .join("pod.toml"),
        r#"
[pod]
slug = "test-pod"
default_backend = "echo"

[backends.echo]
enabled = true
command = "/bin/echo"
exec_args = ["pod={pod}", "fin={fin}", "model={model}", "prompt={prompt}"]
chat_args = ["chat", "model={model}"]

[backends.echo.defaults]
model = "pod-default"
"#,
    )
    .unwrap();
    fs::write(
        fin_home.join("fin.toml"),
        r#"
[fin]
slug = "amy"

[backend]
model = "fin-model"
"#,
    )
    .unwrap();

    let command = backend_command(
        &orqa,
        &fin,
        &[OsString::from("hello"), OsString::from("world")],
    )
    .unwrap();

    assert_eq!(command.backend, "echo");
    assert_eq!(command.command, OsString::from("/bin/echo"));
    assert_eq!(command.mode, BackendMode::Exec);
    assert_eq!(
        command.args,
        vec![
            OsString::from("pod=test-pod"),
            OsString::from("fin=amy"),
            OsString::from("model=fin-model"),
            OsString::from("prompt=hello world"),
        ]
    );

    let chat = backend_chat_command(&orqa, &fin).unwrap();
    assert_eq!(chat.mode, BackendMode::Chat);
    assert_eq!(
        chat.args,
        vec![OsString::from("chat"), OsString::from("model=fin-model")]
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn resolves_run_policy_with_fin_overrides() {
    let root = temp_root();
    let orqa = Orqa::new(Some(root.clone()));
    let fin = FinRef::new("test-pod", "amy").unwrap();
    let fin_home = orqa.fin_home(&fin);

    fs::create_dir_all(&fin_home).unwrap();
    fs::write(
        orqa.pod_home(&PodRef::new("test-pod").unwrap())
            .join("pod.toml"),
        r#"
[pod]
slug = "test-pod"
default_backend = "echo"
debounce = "15m"
exec_always = "3h"

[backends.echo]
enabled = true
command = "/bin/echo"
exec_args = ["{prompt}"]
chat_args = []
"#,
    )
    .unwrap();
    fs::write(
        fin_home.join("fin.toml"),
        r#"
[fin]
slug = "amy"
debounce = "30s"
"#,
    )
    .unwrap();

    let policy = run_policy(&orqa, &fin).unwrap();
    assert_eq!(policy.debounce, Some(Duration::from_secs(30)));
    assert_eq!(policy.exec_always, Some(Duration::from_secs(3 * 60 * 60)));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn zero_run_policy_values_disable_policy() {
    let root = temp_root();
    let orqa = Orqa::new(Some(root.clone()));
    let fin = FinRef::new("test-pod", "amy").unwrap();
    let fin_home = orqa.fin_home(&fin);

    fs::create_dir_all(&fin_home).unwrap();
    fs::write(
        orqa.pod_home(&PodRef::new("test-pod").unwrap())
            .join("pod.toml"),
        r#"
[pod]
slug = "test-pod"
default_backend = "echo"
debounce = "15m"
exec_always = "3h"

[backends.echo]
enabled = true
command = "/bin/echo"
exec_args = ["{prompt}"]
chat_args = []
"#,
    )
    .unwrap();
    fs::write(
        fin_home.join("fin.toml"),
        r#"
[fin]
slug = "amy"
debounce = "0"
exec_always = "0"
"#,
    )
    .unwrap();

    let policy = run_policy(&orqa, &fin).unwrap();
    assert_eq!(policy.debounce, None);
    assert_eq!(policy.exec_always, None);

    fs::remove_dir_all(root).unwrap();
}

fn temp_root() -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    env::temp_dir().join(format!(
        "orqa-config-test-{}-{suffix}-{counter}",
        std::process::id(),
    ))
}
