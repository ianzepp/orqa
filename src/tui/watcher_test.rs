use std::{fs, time::SystemTime};

use crate::{
    model::{Orqa, PodRegistration},
    tui::{events::Event, watcher::PodWatcher},
};

#[test]
fn run_change_finishes_previous_run_before_starting_new_one() {
    let root = temp_root();
    let pod_root = root.join("pod");
    let fin_root = pod_root.join(".orqa").join("fins").join("grok");
    fs::create_dir_all(&fin_root).unwrap();
    fs::write(fin_root.join("latest-run"), "old-run\n").unwrap();

    let reg = PodRegistration {
        slug: "sample-pod".to_string(),
        path: pod_root,
        enabled: true,
    };
    let mut watcher = PodWatcher::new(Orqa::new(Some(root.join("home"))), reg).unwrap();

    let initial = watcher.poll().unwrap();
    assert!(matches!(
        initial.as_slice(),
        [Event::RunStarted { run_id, .. }] if run_id == "old-run"
    ));

    fs::write(fin_root.join("latest-run"), "new-run\n").unwrap();
    let changed = watcher.poll().unwrap();

    assert!(matches!(
        changed.as_slice(),
        [
            Event::RunFinished { run_id: finished, .. },
            Event::RunStarted { run_id: started, .. }
        ] if finished == "old-run" && started == "new-run"
    ));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn batches_new_log_lines_into_one_timeline_event() {
    let root = temp_root();
    let pod_root = root.join("pod");
    let fin_root = pod_root.join(".orqa").join("fins").join("grok");
    let run_root = fin_root.join("runs").join("run-1");
    fs::create_dir_all(&run_root).unwrap();
    fs::write(fin_root.join("latest-run"), "run-1\n").unwrap();
    fs::write(run_root.join("stdout.log"), "first line\nsecond line\n").unwrap();

    let reg = PodRegistration {
        slug: "sample-pod".to_string(),
        path: pod_root,
        enabled: true,
    };
    let mut watcher = PodWatcher::new(Orqa::new(Some(root.join("home"))), reg).unwrap();

    let events = watcher.poll().unwrap();
    let log_events: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Event::LogLine { line, .. } => Some(line.as_str()),
            _ => None,
        })
        .collect();

    assert_eq!(log_events, vec!["first line\nsecond line"]);

    fs::remove_dir_all(root).unwrap();
}

fn temp_root() -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("orqa-watcher-test-{nanos}"))
}
