use std::{env, ffi::OsString, fs, path::Path};

use crate::{
    mailbox::unique_mail_name,
    model::{FinRef, Orqa},
};

use super::{FinLock, lock_field, process_is_alive};

#[test]
fn fin_lock_claim_blocks_second_live_claim() {
    let fixture = LockFixture::new("claim-blocks-second");
    let first = FinLock::claim(
        &fixture.orqa,
        &fixture.fin,
        "test",
        &OsString::from("codex"),
    )
    .expect("first claim should acquire lock");

    let error = FinLock::claim(
        &fixture.orqa,
        &fixture.fin,
        "test",
        &OsString::from("codex"),
    )
    .unwrap_err();

    assert!(error.contains("already starting or running"));
    assert!(fixture.lock_path.exists());
    first.release();
    assert!(!fixture.lock_path.exists());
}

#[test]
fn fin_lock_claim_removes_stale_lock() {
    let fixture = LockFixture::new("claim-removes-stale");
    fs::write(
        &fixture.lock_path,
        "state=running\npid=999999\npod=sample-pod\nfin=amy\ntoken=old\n",
    )
    .unwrap();

    let lock = FinLock::claim(
        &fixture.orqa,
        &fixture.fin,
        "test",
        &OsString::from("codex"),
    )
    .expect("stale lock should be replaced");
    let contents = fs::read_to_string(&fixture.lock_path).unwrap();

    assert_eq!(lock_field(&contents, "state").as_deref(), Some("claimed"));
    assert_eq!(
        lock_field(&contents, "pid").as_deref(),
        Some(std::process::id().to_string().as_str())
    );
    assert!(process_is_alive(lock.pid()));
}

#[test]
fn fin_lock_mark_running_updates_pid_and_preserves_token() {
    let fixture = LockFixture::new("mark-running");
    let mut lock = FinLock::claim(
        &fixture.orqa,
        &fixture.fin,
        "test",
        &OsString::from("codex"),
    )
    .expect("claim should acquire lock");
    let claimed = fs::read_to_string(&fixture.lock_path).unwrap();
    let token = lock_field(&claimed, "token").unwrap();

    lock.mark_running(12345, "run-1", &OsString::from("codex"))
        .unwrap();
    let running = fs::read_to_string(&fixture.lock_path).unwrap();

    assert_eq!(lock_field(&running, "state").as_deref(), Some("running"));
    assert_eq!(lock_field(&running, "pid").as_deref(), Some("12345"));
    assert_eq!(lock_field(&running, "run_id").as_deref(), Some("run-1"));
    assert_eq!(
        lock_field(&running, "token").as_deref(),
        Some(token.as_str())
    );
}

#[test]
fn fin_lock_release_does_not_remove_successor_lock() {
    let fixture = LockFixture::new("release-preserves-successor");
    let lock = FinLock::claim(
        &fixture.orqa,
        &fixture.fin,
        "test",
        &OsString::from("codex"),
    )
    .expect("claim should acquire lock");
    fs::write(
        &fixture.lock_path,
        "state=claimed\npid=999999\npod=sample-pod\nfin=amy\ntoken=successor\n",
    )
    .unwrap();

    lock.release();

    let contents = fs::read_to_string(&fixture.lock_path).unwrap();
    assert_eq!(lock_field(&contents, "token").as_deref(), Some("successor"));
}

struct LockFixture {
    root: std::path::PathBuf,
    orqa: Orqa,
    fin: FinRef,
    lock_path: std::path::PathBuf,
}

impl LockFixture {
    fn new(name: &str) -> Self {
        let root = env::temp_dir().join(format!(
            "orqa-runtime-{name}-{}",
            unique_mail_name().unwrap()
        ));
        let pod_root = root.join("pod");
        let orqa_home = root.join("home");
        let fin_home = pod_root.join(".orqa").join("fins").join("amy");
        fs::create_dir_all(&fin_home).unwrap();
        fs::create_dir_all(&orqa_home).unwrap();
        fs::write(
            orqa_home.join("config.toml"),
            format!(
                "[registry]\nversion = 1\n\n[pods.sample-pod]\nenabled = true\npath = {:?}\n",
                pod_root
            ),
        )
        .unwrap();

        let orqa = Orqa::new(Some(orqa_home));
        let fin = FinRef::new("sample-pod", "amy").unwrap();
        let lock_path = orqa.lock_path(&fin).unwrap();

        Self {
            root,
            orqa,
            fin,
            lock_path,
        }
    }
}

impl Drop for LockFixture {
    fn drop(&mut self) {
        remove_dir_all_best_effort(&self.root);
    }
}

fn remove_dir_all_best_effort(path: &Path) {
    let _ = fs::remove_dir_all(path);
}
