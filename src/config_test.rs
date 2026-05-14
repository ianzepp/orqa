use std::{env, ffi::OsString, fs, time::SystemTime};

use super::*;
use crate::model::{FinRef, Orqa};

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
args = ["pod={pod}", "fin={fin}", "model={model}", "prompt={prompt}"]

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
    assert_eq!(
        command.args,
        vec![
            OsString::from("pod=test-pod"),
            OsString::from("fin=amy"),
            OsString::from("model=fin-model"),
            OsString::from("prompt=hello world"),
        ]
    );

    fs::remove_dir_all(root).unwrap();
}

fn temp_root() -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    env::temp_dir().join(format!("orqa-config-test-{suffix}"))
}
