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
fn pod_and_fin_create_seed_agents_files() {
    let root = temp_root();

    orqa(&root, ["pod", "create", "test-pod"]);
    orqa(&root, ["fin", "create", "test-pod", "planner"]);

    let pod_agents = fs::read_to_string(root.join("pods/test-pod/AGENTS.md")).unwrap();
    let fin_agents = fs::read_to_string(root.join("pods/test-pod/fins/planner/AGENTS.md")).unwrap();
    let charter = fs::read_to_string(root.join("pods/test-pod/CHARTER.md")).unwrap();
    let role = fs::read_to_string(root.join("pods/test-pod/fins/planner/ROLE.md")).unwrap();

    assert!(charter.contains("No pod charter has been set yet."));
    assert!(role.contains("No fin role has been set yet."));
    assert!(pod_agents.contains("No pod charter has been set yet."));
    assert!(pod_agents.contains("orqa fin list"));
    assert!(pod_agents.contains("orqa mail send --to <fin>"));
    assert!(pod_agents.contains("orqa task send --to <fin>"));
    assert!(fin_agents.contains("You are the `planner` fin"));
    assert!(fin_agents.contains("No fin role has been set yet."));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn pod_create_and_charter_commands_manage_charter_agents_injection() {
    let root = temp_root();

    orqa(
        &root,
        [
            "pod",
            "create",
            "test-pod",
            "--charter",
            "Build a tiny orchestration tool.",
        ],
    );

    let charter_path = root.join("pods/test-pod/CHARTER.md");
    let agents_path = root.join("pods/test-pod/AGENTS.md");
    assert_eq!(
        fs::read_to_string(&charter_path).unwrap(),
        "Build a tiny orchestration tool.\n"
    );
    assert!(
        fs::read_to_string(&agents_path)
            .unwrap()
            .contains("Build a tiny orchestration tool.")
    );

    let get = orqa_output(&root, ["pod", "charter", "get", "test-pod"]);
    assert_eq!(get, "Build a tiny orchestration tool.\n");

    let charter_source = root.join("charter.md");
    fs::write(
        &charter_source,
        "Keep the fins aligned around product work.\n",
    )
    .unwrap();
    let charter_arg = format!("@{}", charter_source.display());
    orqa(&root, ["pod", "charter", "set", "test-pod", &charter_arg]);
    assert_eq!(
        fs::read_to_string(&charter_path).unwrap(),
        "Keep the fins aligned around product work.\n"
    );
    assert!(
        fs::read_to_string(&agents_path)
            .unwrap()
            .contains("Keep the fins aligned around product work.")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn fin_create_and_role_commands_manage_role_agents_injection() {
    let root = temp_root();

    orqa(&root, ["pod", "create", "test-pod"]);
    orqa(
        &root,
        [
            "fin",
            "create",
            "test-pod",
            "planner",
            "--role",
            "Turn charter goals into concrete tasks.",
        ],
    );

    let role_path = root.join("pods/test-pod/fins/planner/ROLE.md");
    let agents_path = root.join("pods/test-pod/fins/planner/AGENTS.md");
    assert_eq!(
        fs::read_to_string(&role_path).unwrap(),
        "Turn charter goals into concrete tasks.\n"
    );
    assert!(
        fs::read_to_string(&agents_path)
            .unwrap()
            .contains("Turn charter goals into concrete tasks.")
    );

    let get = orqa_output(&root, ["fin", "role", "get", "test-pod", "planner"]);
    assert_eq!(get, "Turn charter goals into concrete tasks.\n");

    orqa(
        &root,
        [
            "fin",
            "role",
            "set",
            "test-pod",
            "planner",
            "Review work and send precise follow-up mail.",
        ],
    );
    assert_eq!(
        fs::read_to_string(&role_path).unwrap(),
        "Review work and send precise follow-up mail.\n"
    );
    assert!(
        fs::read_to_string(&agents_path)
            .unwrap()
            .contains("Review work and send precise follow-up mail.")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn fin_create_symlinks_existing_codex_auth() {
    let root = temp_root();
    let user_home = root.join("user-home");
    let user_codex_home = user_home.join(".codex");
    fs::create_dir_all(&user_codex_home).unwrap();
    let user_auth = user_codex_home.join("auth.json");
    fs::write(&user_auth, "{}\n").unwrap();

    orqa(&root, ["pod", "create", "test-pod"]);
    let output = command(&root, ["fin", "create", "test-pod", "builder"])
        .env("HOME", &user_home)
        .env_remove("CODEX_HOME")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "orqa failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let fin_auth = root.join("pods/test-pod/fins/builder/.codex/auth.json");
    assert_eq!(fs::read_link(fin_auth).unwrap(), user_auth);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn fin_exec_runs_from_fin_home_for_agents_discovery() {
    let root = temp_root();

    orqa(&root, ["pod", "create", "test-pod"]);
    orqa(&root, ["fin", "create", "test-pod", "amy"]);
    set_pwd_backend(&root, "test-pod");

    orqa(&root, ["fin", "exec", "test-pod", "amy"]);

    let cwd = fs::read_to_string(root.join("pods/test-pod/fins/amy/cwd.txt")).unwrap();
    assert_eq!(
        fs::canonicalize(Path::new(cwd.trim())).unwrap(),
        fs::canonicalize(root.join("pods/test-pod/fins/amy")).unwrap()
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn fin_exec_adds_orqa_binary_directory_to_child_path() {
    let root = temp_root();

    orqa(&root, ["pod", "create", "test-pod"]);
    orqa(&root, ["fin", "create", "test-pod", "amy"]);
    set_path_backend(&root, "test-pod");

    let output = command(&root, ["fin", "exec", "test-pod", "amy"])
        .env("PATH", "/usr/bin")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "orqa failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let path = fs::read_to_string(root.join("pods/test-pod/fins/amy/path.txt")).unwrap();
    let orqa_bin_dir = Path::new(env!("CARGO_BIN_EXE_orqa")).parent().unwrap();
    let child_paths = env::split_paths(path.trim()).collect::<Vec<_>>();
    assert_eq!(child_paths.first().unwrap(), orqa_bin_dir);
    assert!(child_paths.iter().any(|path| path == Path::new("/usr/bin")));

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
    fs::write(
        root.join("pods/test-pod/pod.toml"),
        r#"
[pod]
slug = "test-pod"
default_backend = "missing"
debounce = "0"
exec_always = "0"
"#,
    )
    .unwrap();

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
fn plan_debounces_recent_runs_even_with_queued_work() {
    let root = temp_root();

    orqa(&root, ["pod", "create", "test-pod"]);
    orqa(&root, ["fin", "create", "test-pod", "amy"]);
    set_echo_backend(&root, "test-pod");
    set_pod_run_policy(&root, "test-pod", "debounce = \"1h\"\n");

    orqa_output(&root, ["fin", "exec", "test-pod", "amy", "--", "recent"]);
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
    assert!(plan.contains("decision=would-skip"));
    assert!(plan.contains("reason=debounced"));
    assert!(plan.contains("debounce=1h"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn zero_debounce_allows_queued_work_after_recent_runs() {
    let root = temp_root();

    orqa(&root, ["pod", "create", "test-pod"]);
    orqa(&root, ["fin", "create", "test-pod", "amy"]);
    set_echo_backend(&root, "test-pod");
    set_pod_run_policy(&root, "test-pod", "debounce = \"0\"\n");

    orqa_output(&root, ["fin", "exec", "test-pod", "amy", "--", "recent"]);
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

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn plan_wakes_idle_fin_when_exec_always_is_due() {
    let root = temp_root();

    orqa(&root, ["pod", "create", "test-pod"]);
    orqa(&root, ["fin", "create", "test-pod", "amy"]);
    set_echo_backend(&root, "test-pod");
    set_pod_run_policy(&root, "test-pod", "exec_always = \"3h\"\n");

    let plan = orqa_output(&root, ["plan", "test-pod"]);
    assert!(plan.contains("decision=would-wake"));
    assert!(plan.contains("reason=exec-always"));
    assert!(plan.contains("exec_always=3h"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn zero_exec_always_does_not_wake_idle_fin() {
    let root = temp_root();

    orqa(&root, ["pod", "create", "test-pod"]);
    orqa(&root, ["fin", "create", "test-pod", "amy"]);
    set_echo_backend(&root, "test-pod");
    set_pod_run_policy(&root, "test-pod", "exec_always = \"0\"\n");

    let plan = orqa_output(&root, ["plan", "test-pod"]);
    assert!(plan.contains("decision=would-skip"));
    assert!(plan.contains("reason=no-action"));

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
fn pod_doctor_checks_files_config_and_backend_probe() {
    let root = temp_root();

    orqa(&root, ["pod", "create", "test-pod"]);
    orqa(&root, ["fin", "create", "test-pod", "amy"]);
    set_echo_backend(&root, "test-pod");

    let output = orqa_output(
        &root,
        [
            "pod",
            "doctor",
            "test-pod",
            "--fin",
            "amy",
            "--timeout",
            "5",
        ],
    );

    assert!(output.contains("ok check=pod home"));
    assert!(output.contains("ok test-pod/amy check=backend backend=echo"));
    assert!(output.contains("ok test-pod/amy check=probe"));
    assert!(output.contains("doctor pod=test-pod status=ok"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn pod_doctor_reports_backend_spawn_failures() {
    let root = temp_root();

    orqa(&root, ["pod", "create", "test-pod"]);
    orqa(&root, ["fin", "create", "test-pod", "amy"]);
    set_missing_backend(&root, "test-pod");

    let output = command(
        &root,
        [
            "pod",
            "doctor",
            "test-pod",
            "--fin",
            "amy",
            "--timeout",
            "1",
        ],
    )
    .output()
    .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stdout.contains("ok test-pod/amy check=backend backend=missing"));
    assert!(stdout.contains("fail test-pod/amy check=probe"));
    assert!(stderr.contains("pod test-pod doctor failed"));

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

#[test]
fn mail_to_operator_opens_issue_and_resolution_mails_fin() {
    let root = temp_root();

    orqa(&root, ["pod", "create", "deploy"]);
    orqa(&root, ["fin", "create", "deploy", "release"]);
    orqa(
        &root,
        [
            "mail",
            "send",
            "--from",
            "release@deploy.orqa",
            "--to",
            "operator@deploy.orqa",
            "--subject",
            "Railway auth expired",
            "--",
            "---\nseverity: blocked\nkind: auth\n---\n\nRailway CLI is not logged in.",
        ],
    );

    let issues = orqa_output(&root, ["ops", "issues"]);
    assert!(issues.contains("pod=deploy"));
    assert!(issues.contains("fin=release"));
    assert!(issues.contains("severity=blocked"));
    assert!(issues.contains("kind=auth"));
    assert!(issues.contains("title=\"Railway auth expired\""));
    assert!(orqa_output(&root, ["ops", "issues", "--pod", "deploy"]).contains("pod=deploy"));
    assert!(orqa_output(&root, ["ops", "issues", "--fin", "release"]).contains("fin=release"));
    assert!(
        orqa_output(&root, ["ops", "issues", "--severity", "blocked"]).contains("severity=blocked")
    );
    assert!(orqa_output(&root, ["ops", "issues", "--kind", "auth"]).contains("kind=auth"));
    assert!(
        orqa_output(&root, ["ops", "issues", "--field", "status=open"]).contains("status=open")
    );
    assert_eq!(
        orqa_output(&root, ["ops", "issues", "--pod", "other-pod"]),
        ""
    );

    let issue_id = issues
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap();
    let issue = orqa_output(&root, ["ops", "issue", "read", issue_id]);
    assert!(issue.contains("status: open"));
    assert!(issue.contains("Railway CLI is not logged in."));

    orqa(&root, ["fin", "sleep", "deploy", "release"]);
    assert!(orqa_output(&root, ["fin", "status", "deploy", "release"]).contains("sleeping=true"));
    orqa(
        &root,
        [
            "ops",
            "issue",
            "resolve",
            issue_id,
            "--note",
            "Re-authenticated Railway. Try deploy again.",
            "--wake",
        ],
    );
    assert!(orqa_output(&root, ["fin", "status", "deploy", "release"]).contains("sleeping=false"));

    let closed = orqa_output(&root, ["ops", "issues", "--all"]);
    assert!(closed.contains("closed"));
    assert!(closed.contains("status=resolved"));

    let mail = orqa_output(
        &root,
        ["mail", "list", "--pod", "deploy", "--fin", "release"],
    );
    assert!(mail.contains("Re: Railway auth expired"));
    let message_id = mail
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap();
    let message = orqa_output(
        &root,
        [
            "mail", "read", "--pod", "deploy", "--fin", "release", message_id,
        ],
    );
    assert!(message.contains("From: operator@deploy.orqa"));
    assert!(message.contains("Issue resolved."));
    assert!(message.contains("Re-authenticated Railway. Try deploy again."));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn operator_issue_ack_and_dismiss_move_issue_and_mail_fin() {
    let root = temp_root();

    orqa(&root, ["pod", "create", "support"]);
    orqa(&root, ["fin", "create", "support", "helper"]);
    orqa(
        &root,
        [
            "mail",
            "send",
            "--from",
            "helper@support.orqa",
            "--to",
            "operator@support.orqa",
            "--subject",
            "Need policy call",
            "I need a human decision.",
        ],
    );

    let issues = orqa_output(&root, ["ops", "issues"]);
    let issue_id = issues
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap();
    orqa(&root, ["ops", "issue", "ack", issue_id]);
    let acknowledged = orqa_output(&root, ["ops", "issues", "--status", "acknowledged"]);
    assert!(acknowledged.contains("status=acknowledged"));

    orqa(
        &root,
        [
            "ops",
            "issue",
            "dismiss",
            issue_id,
            "--note",
            "Operator decided this is not needed.",
        ],
    );
    let closed = orqa_output(&root, ["ops", "issues", "--all", "--status", "dismissed"]);
    assert!(closed.contains("closed"));
    assert!(closed.contains("status=dismissed"));

    let mail = orqa_output(
        &root,
        ["mail", "list", "--pod", "support", "--fin", "helper"],
    );
    assert!(mail.contains("Re: Need policy call"));

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

fn set_pod_run_policy(root: &Path, pod: &str, policy: &str) {
    let pod_config = root.join(format!("pods/{pod}/pod.toml"));
    let config = fs::read_to_string(&pod_config).unwrap();
    let config = config.replace("exec_always = \"0\"\n", "");
    let config = config.replace("debounce = \"5m\"\n", policy);
    fs::write(&pod_config, config).unwrap();
}

fn set_pwd_backend(root: &Path, pod: &str) {
    let pod_config = root.join(format!("pods/{pod}/pod.toml"));
    let config = fs::read_to_string(&pod_config).unwrap();
    let config = config.replace("default_backend = \"codex\"", "default_backend = \"pwd\"");
    fs::write(
        &pod_config,
        format!(
            r#"{config}

[backends.pwd]
enabled = true
command = "/bin/sh"
exec_args = ["-c", "pwd > {{fin_home}}/cwd.txt"]
"#
        ),
    )
    .unwrap();
}

fn set_path_backend(root: &Path, pod: &str) {
    let pod_config = root.join(format!("pods/{pod}/pod.toml"));
    let config = fs::read_to_string(&pod_config).unwrap();
    let config = config.replace("default_backend = \"codex\"", "default_backend = \"path\"");
    fs::write(
        &pod_config,
        format!(
            r#"{config}

[backends.path]
enabled = true
command = "/bin/sh"
exec_args = ["-c", "printf '%s' \"$PATH\" > {{fin_home}}/path.txt"]
"#
        ),
    )
    .unwrap();
}

fn set_missing_backend(root: &Path, pod: &str) {
    let pod_config = root.join(format!("pods/{pod}/pod.toml"));
    let config = fs::read_to_string(&pod_config).unwrap();
    let config = config.replace(
        "default_backend = \"codex\"",
        "default_backend = \"missing\"",
    );
    fs::write(
        &pod_config,
        format!(
            r#"{config}

[backends.missing]
enabled = true
command = "/definitely/missing/orqa-test-backend"
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
