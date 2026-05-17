use std::time::{Duration, Instant};

use super::top::{
    TopFin, TopPod, fin_status_symbol, initial_last_wake, next_loop_label, pod_header,
    pod_status_symbol, pod_window,
};

fn fin(running: bool, sleeping: bool, wakeable: bool) -> TopFin {
    TopFin {
        pod: "sample-pod".to_string(),
        fin: "builder".to_string(),
        running,
        sleeping,
        wakeable,
        duration_secs: 0,
        pid: None,
        stdout_bytes: 0,
        stderr_bytes: 0,
        unread_mail: 0,
        open_tasks: 0,
    }
}

fn pod(sleeping: bool, running: usize, wakeable: usize, error: Option<String>) -> TopPod {
    TopPod {
        pod: "sample-pod".to_string(),
        sleeping,
        fins: 0,
        running,
        paused: 0,
        wakeable,
        unread_mail: 0,
        open_tasks: 0,
        error,
    }
}

#[test]
fn top_status_symbols_are_compact() {
    assert_eq!(fin_status_symbol(&fin(false, false, false)), "-");
    assert_eq!(fin_status_symbol(&fin(false, true, false)), "P");
    assert_eq!(fin_status_symbol(&fin(true, false, false)), "R");
    assert_eq!(fin_status_symbol(&fin(false, false, true)), "W");

    assert_eq!(pod_status_symbol(&pod(false, 0, 0, None)), "-");
    assert_eq!(pod_status_symbol(&pod(true, 0, 0, None)), "P");
    assert_eq!(pod_status_symbol(&pod(false, 1, 0, None)), "R");
    assert_eq!(pod_status_symbol(&pod(false, 0, 1, None)), "W");
    assert_eq!(
        pod_status_symbol(&pod(false, 0, 0, Some("bad".to_string()))),
        "E"
    );
}

#[test]
fn top_next_loop_label_counts_down_from_last_wake() {
    let now = Instant::now();

    assert_eq!(next_loop_label(now, now), "next: 10s");
    assert_eq!(
        next_loop_label(now, now + Duration::from_secs(4)),
        "next: 6s"
    );
    assert_eq!(
        next_loop_label(now, now + Duration::from_secs(11)),
        "next: 0s"
    );
}

#[test]
fn top_first_loop_starts_after_ten_seconds() {
    let now = Instant::now();

    assert_eq!(next_loop_label(initial_last_wake(now), now), "next: 10s");
}

#[test]
fn pod_window_keeps_selected_pod_visible() {
    assert_eq!(pod_window(0, 0, 6), (0, 0));
    assert_eq!(pod_window(4, 0, 6), (0, 4));
    assert_eq!(pod_window(10, 0, 6), (0, 6));
    assert_eq!(pod_window(10, 5, 6), (2, 8));
    assert_eq!(pod_window(10, 9, 6), (4, 10));
}

#[test]
fn pod_header_shows_range_when_pods_are_hidden() {
    assert_eq!(pod_header(0, 0, 0), "Pod");
    assert_eq!(pod_header(0, 4, 4), "Pod");
    assert_eq!(pod_header(0, 6, 7), "Pod 1-6/7");
    assert_eq!(pod_header(4, 10, 10), "Pod 5-10/10");
}
