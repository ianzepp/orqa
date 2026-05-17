use super::top::{TopFin, TopPod, fin_status_symbol, pod_status_symbol};

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
