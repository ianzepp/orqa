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

    create_pod(&root, "test-pod");
    orqa(&root, ["fin", "create", "test-pod", "amy"]);

    let pod_config = pod_home(&root, "test-pod").join("pod.toml");
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

    remove_temp_root(root);
}

#[test]
fn pod_and_fin_create_seed_agents_files() {
    let root = temp_root();

    create_pod(&root, "test-pod");
    orqa(&root, ["fin", "create", "test-pod", "planner"]);

    let pod_agents = fs::read_to_string(pod_home(&root, "test-pod").join("AGENTS.md")).unwrap();
    let fin_agents =
        fs::read_to_string(fin_home(&root, "test-pod", "planner").join("AGENTS.md")).unwrap();
    let charter = fs::read_to_string(pod_home(&root, "test-pod").join("CHARTER.md")).unwrap();
    let role = fs::read_to_string(fin_home(&root, "test-pod", "planner").join("ROLE.md")).unwrap();

    assert!(charter.contains("No pod charter has been set yet."));
    assert!(role.contains("No fin role has been set yet."));
    assert!(pod_agents.contains("No pod charter has been set yet."));
    assert!(pod_agents.contains("orqa fin list"));
    assert!(pod_agents.contains("orqa mail send --to <fin>"));
    assert!(pod_agents.contains("orqa task send --to <fin>"));
    assert!(fin_agents.contains("You are the `planner` fin"));
    assert!(fin_agents.contains("No fin role has been set yet."));

    remove_temp_root(root);
}

#[test]
fn init_seeds_operator_fin_for_tui_identity() {
    let root = temp_root();
    let project = root.join("sample-project");
    fs::create_dir_all(&project).unwrap();

    orqa(
        &root,
        [
            "init",
            "sample-project",
            "--path",
            project.to_str().unwrap(),
        ],
    );

    let operator = project.join(".orqa/fins/operator");
    let fin_config = fs::read_to_string(operator.join("fin.toml")).unwrap();
    let role = fs::read_to_string(operator.join("ROLE.md")).unwrap();
    let agents = fs::read_to_string(operator.join("AGENTS.md")).unwrap();
    let gitignore = fs::read_to_string(project.join(".gitignore")).unwrap();

    assert!(project.join(".orqa/pod.toml").exists());
    assert!(operator.join("mail/new").exists());
    assert!(operator.join("tasks/new").exists());
    assert!(operator.join("sleep.lock").exists());
    assert!(fin_config.contains("slug = \"operator\""));
    assert!(role.contains("Human operator identity for the TUI"));
    assert!(agents.contains("You are the `operator` fin"));
    assert!(gitignore.contains("/.orqa"));

    remove_temp_root(root);
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
            "--path",
            pod_root(&root, "test-pod").to_str().unwrap(),
            "--charter",
            "Build a tiny orchestration tool.",
        ],
    );

    let charter_path = pod_home(&root, "test-pod").join("CHARTER.md");
    let agents_path = pod_home(&root, "test-pod").join("AGENTS.md");
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

    remove_temp_root(root);
}

#[test]
fn fin_create_and_role_commands_manage_role_agents_injection() {
    let root = temp_root();

    create_pod(&root, "test-pod");
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

    let role_path = fin_home(&root, "test-pod", "planner").join("ROLE.md");
    let agents_path = fin_home(&root, "test-pod", "planner").join("AGENTS.md");
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

    remove_temp_root(root);
}

#[test]
fn fin_create_symlinks_existing_codex_auth() {
    let root = temp_root();
    let user_home = root.join("user-home");
    let user_codex_home = user_home.join(".codex");
    fs::create_dir_all(&user_codex_home).unwrap();
    let user_auth = user_codex_home.join("auth.json");
    fs::write(&user_auth, "{}\n").unwrap();

    create_pod(&root, "test-pod");
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

    let fin_auth = fin_home(&root, "test-pod", "builder").join(".codex/auth.json");
    assert_eq!(fs::read_link(fin_auth).unwrap(), user_auth);

    remove_temp_root(root);
}

#[test]
fn fin_exec_runs_from_pod_root_for_agents_discovery() {
    let root = temp_root();

    create_pod(&root, "test-pod");
    orqa(&root, ["fin", "create", "test-pod", "amy"]);
    set_pwd_backend(&root, "test-pod");

    orqa(&root, ["fin", "exec", "test-pod", "amy"]);

    let cwd = fs::read_to_string(fin_home(&root, "test-pod", "amy").join("cwd.txt")).unwrap();
    assert_eq!(
        fs::canonicalize(Path::new(cwd.trim())).unwrap(),
        fs::canonicalize(pod_root(&root, "test-pod")).unwrap()
    );

    remove_temp_root(root);
}

#[test]
fn fin_exec_adds_orqa_binary_directory_to_child_path() {
    let root = temp_root();

    create_pod(&root, "test-pod");
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

    let path = fs::read_to_string(fin_home(&root, "test-pod", "amy").join("path.txt")).unwrap();
    let orqa_bin_dir = Path::new(env!("CARGO_BIN_EXE_orqa")).parent().unwrap();
    let child_paths = env::split_paths(path.trim()).collect::<Vec<_>>();
    assert_eq!(child_paths.first().unwrap(), orqa_bin_dir);
    assert!(child_paths.iter().any(|path| path == Path::new("/usr/bin")));

    remove_temp_root(root);
}

#[test]
fn wake_uses_generated_pod_config_backend_for_wakeable_fin() {
    let root = temp_root();

    create_pod(&root, "test-pod");
    orqa(&root, ["fin", "create", "test-pod", "amy"]);

    let pod_config = pod_home(&root, "test-pod").join("pod.toml");
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
    orqa_in_pod(&root, "test-pod", ["wake", "--", "from-wake"]);

    let marker = fin_home(&root, "test-pod", "amy").join("ran.txt");
    for _ in 0..20 {
        if marker.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }

    assert_eq!(
        fs::read_to_string(&marker).unwrap(),
        "pod=test-pod fin=amy prompt=from-wake"
    );

    remove_temp_root(root);
}

#[test]
fn wake_dry_run_explains_wake_decisions_without_running() {
    let root = temp_root();

    create_pod(&root, "test-pod");
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

    let plan = orqa_output_in_pod(&root, "test-pod", ["wake", "--dry-run"]);
    assert!(plan.contains("decision=would-wake"));
    assert!(plan.contains("reason=mail"));

    let dry_run = orqa_output_in_pod(&root, "test-pod", ["wake", "--dry-run"]);
    assert!(dry_run.contains("decision=would-wake"));
    assert!(!fin_home(&root, "test-pod", "amy").join("ran.txt").exists());

    remove_temp_root(root);
}

#[test]
fn pod_hook_add_list_run_and_toggle_manage_pre_plan_hooks() {
    let root = temp_root();

    create_pod(&root, "test-pod");
    orqa(
        &root,
        [
            "pod",
            "hook",
            "add",
            "test-pod",
            "pre-plan",
            "10-env",
            "--",
            "./10-env.sh",
        ],
    );

    let hook_home = pod_home(&root, "test-pod").join("hooks/pre-plan");
    let script = hook_home.join("10-env.sh");
    fs::write(
        &script,
        r#"#!/usr/bin/env sh
set -eu
printf '%s|%s|%s|%s|%s' "$ORQA_HOME" "$ORQA_POD" "$ORQA_HOOK" "$ORQA_HOOK_PHASE" "$PWD" > "$ORQA_HOOK_STATE/env.txt"
"#,
    )
    .unwrap();

    let list = orqa_output(&root, ["pod", "hook", "list", "test-pod"]);
    assert!(list.contains("pre-plan 10-env enabled=true timeout=30s command=./10-env.sh"));

    orqa(
        &root,
        ["pod", "hook", "disable", "test-pod", "pre-plan", "10-env"],
    );
    let disabled = orqa_output(&root, ["pod", "hook", "list", "test-pod"]);
    assert!(disabled.contains("enabled=false"));

    orqa(
        &root,
        ["pod", "hook", "enable", "test-pod", "pre-plan", "10-env"],
    );
    let run = orqa_output(&root, ["pod", "hook", "run", "test-pod", "pre-plan"]);
    assert!(run.contains("hook test-pod pre-plan/10-env status=ok"));

    let state =
        fs::read_to_string(pod_home(&root, "test-pod").join("hooks/state/10-env/env.txt")).unwrap();
    let parts = state.split('|').collect::<Vec<_>>();
    assert_eq!(Path::new(parts[0]), root.as_path());
    assert_eq!(parts[1], "test-pod");
    assert_eq!(parts[2], "10-env");
    assert_eq!(parts[3], "pre-plan");
    assert_eq!(
        fs::canonicalize(Path::new(parts[4])).unwrap(),
        fs::canonicalize(&hook_home).unwrap()
    );

    orqa(
        &root,
        ["pod", "hook", "remove", "test-pod", "pre-plan", "10-env"],
    );
    assert!(!hook_home.join("10-env.toml").exists());
    assert!(!script.exists());

    remove_temp_root(root);
}

#[test]
fn wake_runs_pre_plan_hooks_and_continues_after_hook_failure() {
    let root = temp_root();

    create_pod(&root, "test-pod");
    orqa(&root, ["fin", "create", "test-pod", "amy"]);
    set_writer_backend(&root, "test-pod");
    orqa(
        &root,
        [
            "pod",
            "hook",
            "add",
            "test-pod",
            "pre-plan",
            "10-fail",
            "--",
            "./10-fail.sh",
        ],
    );
    fs::write(
        pod_home(&root, "test-pod").join("hooks/pre-plan/10-fail.sh"),
        "#!/usr/bin/env sh\nexit 7\n",
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

    let output = orqa_output_in_pod(&root, "test-pod", ["wake", "--", "from-wake"]);
    assert!(output.contains("hook test-pod pre-plan/10-fail status=failed exit=7"));
    assert!(output.contains("wake test-pod/amy"));

    let marker = fin_home(&root, "test-pod", "amy").join("ran.txt");
    for _ in 0..20 {
        if marker.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(
        fs::read_to_string(&marker).unwrap(),
        "pod=test-pod fin=amy prompt=from-wake"
    );

    remove_temp_root(root);
}

#[test]
fn wake_dry_run_ignores_backend_errors_until_fin_is_wakeable() {
    let root = temp_root();

    create_pod(&root, "test-pod");
    orqa(&root, ["fin", "create", "test-pod", "amy"]);
    fs::write(
        pod_home(&root, "test-pod").join("pod.toml"),
        r#"
[pod]
slug = "test-pod"
default_backend = "missing"
debounce = "0"
exec_always = "0"
"#,
    )
    .unwrap();

    let idle = orqa_output_in_pod(&root, "test-pod", ["wake", "--dry-run"]);
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
    let wakeable = orqa_output_in_pod(&root, "test-pod", ["wake", "--dry-run"]);
    assert!(wakeable.contains("decision=would-skip"));
    assert!(wakeable.contains("reason=backend-error"));

    remove_temp_root(root);
}

#[test]
fn wake_dry_run_debounces_recent_runs_even_with_queued_work() {
    let root = temp_root();

    create_pod(&root, "test-pod");
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

    let plan = orqa_output_in_pod(&root, "test-pod", ["wake", "--dry-run"]);
    assert!(plan.contains("decision=would-skip"));
    assert!(plan.contains("reason=debounced"));
    assert!(plan.contains("debounce=1h"));

    remove_temp_root(root);
}

#[test]
fn zero_debounce_allows_queued_work_after_recent_runs() {
    let root = temp_root();

    create_pod(&root, "test-pod");
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

    let plan = orqa_output_in_pod(&root, "test-pod", ["wake", "--dry-run"]);
    assert!(plan.contains("decision=would-wake"));
    assert!(plan.contains("reason=mail"));

    remove_temp_root(root);
}

#[test]
fn wake_dry_run_wakes_idle_fin_when_exec_always_is_due() {
    let root = temp_root();

    create_pod(&root, "test-pod");
    orqa(&root, ["fin", "create", "test-pod", "amy"]);
    set_echo_backend(&root, "test-pod");
    set_pod_run_policy(&root, "test-pod", "exec_always = \"3h\"\n");

    let plan = orqa_output_in_pod(&root, "test-pod", ["wake", "--dry-run"]);
    assert!(plan.contains("decision=would-wake"));
    assert!(plan.contains("reason=exec-always"));
    assert!(plan.contains("exec_always=3h"));

    remove_temp_root(root);
}

#[test]
fn zero_exec_always_does_not_wake_idle_fin() {
    let root = temp_root();

    create_pod(&root, "test-pod");
    orqa(&root, ["fin", "create", "test-pod", "amy"]);
    set_echo_backend(&root, "test-pod");
    set_pod_run_policy(&root, "test-pod", "exec_always = \"0\"\n");

    let plan = orqa_output_in_pod(&root, "test-pod", ["wake", "--dry-run"]);
    assert!(plan.contains("decision=would-skip"));
    assert!(plan.contains("reason=no-action"));

    remove_temp_root(root);
}

#[test]
fn fin_exec_records_status_and_tail_output() {
    let root = temp_root();

    create_pod(&root, "test-pod");
    orqa(&root, ["fin", "create", "test-pod", "amy"]);
    set_echo_backend(&root, "test-pod");

    orqa_output(&root, ["fin", "exec", "test-pod", "amy", "--", "from-run"]);

    let runs = orqa_output(&root, ["fin", "runs", "test-pod", "amy"]);
    assert!(runs.contains("status=finished"));

    let status = orqa_output(&root, ["fin", "status", "test-pod", "amy"]);
    assert!(status.contains("last_status=finished"));

    let tail = orqa_output(&root, ["fin", "tail", "test-pod", "amy"]);
    assert!(tail.contains("from-run"));

    remove_temp_root(root);
}

#[test]
fn repeated_fin_execs_get_distinct_finished_records() {
    let root = temp_root();

    create_pod(&root, "test-pod");
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

    let ledger = fs::read_to_string(fin_home(&root, "test-pod", "amy").join("runs.jsonl")).unwrap();
    assert_eq!(ledger.lines().count(), 2);
    assert!(
        ledger
            .lines()
            .all(|line| line.contains("\"status\":\"finished\"")
                || line.contains("\"status\": \"finished\""))
    );

    remove_temp_root(root);
}

#[test]
fn fin_chat_uses_chat_args_with_interactive_stdio() {
    let root = temp_root();

    create_pod(&root, "test-pod");
    orqa(&root, ["fin", "create", "test-pod", "amy"]);

    let pod_config = pod_home(&root, "test-pod").join("pod.toml");
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

    remove_temp_root(root);
}

#[test]
fn task_done_marks_front_matter_status_done() {
    let root = temp_root();

    create_pod(&root, "test-pod");
    orqa(&root, ["fin", "create", "test-pod", "amy"]);
    orqa(
        &root,
        [
            "task",
            "send",
            "--from",
            "amy@test-pod.orqa",
            "--to",
            "amy@test-pod.orqa",
            "--title",
            "Finish paperwork",
            "Close the loop.",
        ],
    );
    let tasks = orqa_output(&root, ["task", "list", "--pod", "test-pod", "--fin", "amy"]);
    let task_id = tasks
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap();

    let done_path = orqa_output(
        &root,
        ["task", "done", "--pod", "test-pod", "--fin", "amy", task_id],
    );
    let done = fs::read_to_string(done_path.trim()).unwrap();
    assert!(done.contains("status: done\n"));
    assert!(done_path.contains("/tasks/cur/"));

    let done_again = orqa_output(
        &root,
        ["task", "done", "--pod", "test-pod", "--fin", "amy", task_id],
    );
    assert_eq!(done_again.trim(), done_path.trim());

    remove_temp_root(root);
}

#[test]
fn ops_pod_can_bridge_mail_and_tasks_cross_pod() {
    let root = temp_root();

    create_pod(&root, "ops");
    orqa(&root, ["fin", "create", "ops", "operator"]);
    create_pod(&root, "target-pod");
    orqa(&root, ["fin", "create", "target-pod", "ceo"]);

    orqa(
        &root,
        [
            "mail",
            "send",
            "--from",
            "operator@ops.orqa",
            "--to",
            "ceo@target-pod.orqa",
            "--subject",
            "External update",
            "Please summarize status.",
        ],
    );
    let mail = orqa_output(
        &root,
        ["mail", "list", "--pod", "target-pod", "--fin", "ceo"],
    );
    assert!(mail.contains("External update"));
    orqa(
        &root,
        [
            "mail",
            "send",
            "--from",
            "ceo@target-pod.orqa",
            "--to",
            "operator@ops.orqa",
            "--subject",
            "Re: External update",
            "Status is ready.",
        ],
    );
    let operator_mail = orqa_output(&root, ["mail", "list", "--pod", "ops", "--fin", "operator"]);
    assert!(operator_mail.contains("Re: External update"));

    orqa(
        &root,
        [
            "task",
            "send",
            "--from",
            "operator@ops.orqa",
            "--to",
            "ceo@target-pod.orqa",
            "--title",
            "External request",
            "Reply to dispatcher.",
        ],
    );
    let tasks = orqa_output(
        &root,
        ["task", "list", "--pod", "target-pod", "--fin", "ceo"],
    );
    assert!(tasks.contains("External request"));

    let blocked = orqa_output_failing(
        &root,
        [
            "task",
            "send",
            "--from",
            "ceo@target-pod.orqa",
            "--to",
            "operator@ops.orqa",
            "--title",
            "Not allowed",
            "This should fail.",
        ],
    );
    assert!(blocked.contains("cross-pod tasks are not supported"));

    remove_temp_root(root);
}

#[test]
fn mail_and_tasks_reject_unknown_target_fins() {
    let root = temp_root();

    create_pod(&root, "target-pod");
    orqa(&root, ["fin", "create", "target-pod", "ceo"]);

    let mail = orqa_output_failing(
        &root,
        [
            "mail",
            "send",
            "--from",
            "ceo@target-pod.orqa",
            "--to",
            "dispatcher",
            "--subject",
            "Wrong target",
            "This should fail.",
        ],
    );
    assert!(mail.contains("fin 'target-pod/dispatcher' does not exist"));
    assert!(
        !pod_home(&root, "target-pod")
            .join("fins/dispatcher")
            .exists()
    );

    let task = orqa_output_failing(
        &root,
        [
            "task",
            "send",
            "--from",
            "ceo@target-pod.orqa",
            "--to",
            "dispatcher",
            "--title",
            "Wrong target",
            "This should fail.",
        ],
    );
    assert!(task.contains("fin 'target-pod/dispatcher' does not exist"));
    assert!(
        !pod_home(&root, "target-pod")
            .join("fins/dispatcher")
            .exists()
    );

    remove_temp_root(root);
}

#[test]
fn pod_doctor_checks_files_config_and_backend_probe() {
    let root = temp_root();

    create_pod(&root, "test-pod");
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

    assert!(output.contains("ok check=pod root"));
    assert!(output.contains("ok test-pod/amy check=backend backend=echo"));
    assert!(output.contains("ok test-pod/amy check=probe"));
    assert!(output.contains("doctor pod=test-pod status=ok"));

    remove_temp_root(root);
}

#[test]
fn pod_doctor_reports_backend_spawn_failures() {
    let root = temp_root();

    create_pod(&root, "test-pod");
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

    remove_temp_root(root);
}

#[test]
fn pod_list_prints_sorted_status_and_fin_list_prints_sorted_slugs() {
    let root = temp_root();

    create_pod(&root, "zeta-pod");
    create_pod(&root, "alpha-pod");
    orqa(&root, ["fin", "create", "alpha-pod", "zoe"]);
    orqa(&root, ["fin", "create", "alpha-pod", "amy"]);
    orqa(&root, ["pod", "pause", "zeta-pod"]);

    let pods = orqa_output(&root, ["pod", "list"]);
    let lines = pods.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("alpha-pod "));
    assert!(lines[0].contains("fins=3"));
    assert!(lines[0].contains("sleeping=false"));
    assert!(lines[0].contains("wakeable=0"));
    assert!(lines[0].contains("running=0"));
    assert!(lines[0].contains("unread_mail=0"));
    assert!(lines[0].contains("open_tasks=0"));
    assert!(lines[1].starts_with("zeta-pod "));
    assert!(lines[1].contains("fins=1"));
    assert!(lines[1].contains("sleeping=true"));
    assert_eq!(
        orqa_output(&root, ["fin", "list", "alpha-pod"]),
        "amy\noperator\nzoe\n"
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
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "amy\noperator\nzoe\n"
    );

    remove_temp_root(root);
}

#[test]
fn fin_list_without_pod_context_explains_missing_pod() {
    let root = temp_root();

    let output = command(&root, ["fin", "list"]).output().unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("missing pod"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn foreground_loop_repeats_current_pod_wakes() {
    let root = temp_root();

    for pod in ["alpha-pod", "beta-pod"] {
        create_pod(&root, pod);
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

    let mut child = command_in_pod(
        &root,
        "alpha-pod",
        ["loop", "--interval", "1", "--", "from-loop"],
    )
    .spawn()
    .unwrap();

    let alpha_marker = fin_home(&root, "alpha-pod", "amy").join("ran.txt");
    let beta_marker = fin_home(&root, "beta-pod", "amy").join("ran.txt");
    for _ in 0..40 {
        if alpha_marker.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    stop_child(&mut child);

    assert_eq!(
        fs::read_to_string(&alpha_marker).unwrap(),
        "pod=alpha-pod fin=amy prompt=from-loop"
    );
    assert!(!beta_marker.exists());

    remove_temp_root(root);
}

#[test]
fn mail_to_ops_operator_records_cross_pod_message() {
    let root = temp_root();

    create_pod(&root, "ops");
    orqa(&root, ["fin", "create", "ops", "operator"]);
    create_pod(&root, "deploy");
    orqa(&root, ["fin", "create", "deploy", "release"]);
    orqa(
        &root,
        [
            "mail",
            "send",
            "--from",
            "release@deploy.orqa",
            "--to",
            "operator@ops.orqa",
            "--subject",
            "Railway auth expired",
            "--",
            "---\nseverity: blocked\nkind: auth\n---\n\nRailway CLI is not logged in.",
        ],
    );

    let mail = orqa_output(&root, ["mail", "list", "--pod", "ops", "--fin", "operator"]);
    assert!(mail.contains("Railway auth expired"));
    let message_id = mail
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap();
    let message = orqa_output(
        &root,
        [
            "mail", "read", "--pod", "ops", "--fin", "operator", message_id,
        ],
    );
    assert!(message.contains("From: release@deploy.orqa"));
    assert!(message.contains("To: operator@ops.orqa"));
    assert!(message.contains("severity: blocked"));
    assert!(message.contains("kind: auth"));
    assert!(message.contains("Railway CLI is not logged in."));

    remove_temp_root(root);
}

#[test]
fn ops_report_prints_current_pod_tasks_and_mail() {
    let root = temp_root();

    create_pod(&root, "ops");
    orqa(&root, ["fin", "create", "ops", "operator"]);
    create_pod(&root, "deploy");
    orqa(&root, ["fin", "create", "deploy", "release"]);
    orqa(
        &root,
        [
            "task",
            "send",
            "--from",
            "release@deploy.orqa",
            "--to",
            "release@deploy.orqa",
            "--title",
            "Ship the release",
            "Cut the next release and verify the service.",
        ],
    );
    orqa(
        &root,
        [
            "mail",
            "send",
            "--from",
            "release@deploy.orqa",
            "--to",
            "release@deploy.orqa",
            "--subject",
            "Release note",
            "Remember to summarize operator mail.",
        ],
    );
    orqa(
        &root,
        [
            "mail",
            "send",
            "--from",
            "release@deploy.orqa",
            "--to",
            "operator@ops.orqa",
            "--subject",
            "Cloudflare auth expired",
            "Need a human to refresh credentials.",
        ],
    );

    let output = command(&root, ["ops", "report", "--since", "1d"])
        .current_dir(pod_root(&root, "deploy"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "orqa failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report = String::from_utf8(output.stdout).unwrap();
    assert!(report.contains("# Orqa Ops Report"));
    assert!(report.contains("- pods: `1`"));
    assert!(!report.contains("## Pod `ops`"));
    assert!(report.contains("## Pod `deploy`"));
    assert!(report.contains("### Fin `release`"));
    assert!(report.contains("#### Tasks"));
    assert!(report.contains("title=`Ship the release`"));
    assert!(report.contains("Cut the next release and verify the service."));
    assert!(report.contains("#### Mail"));
    assert!(report.contains("subject=`Release note`"));
    assert!(report.contains("Remember to summarize operator mail."));
    assert!(!report.contains("Cloudflare auth expired"));
    assert!(!report.contains("to=`operator@ops.orqa`"));
    assert!(!report.contains("Need a human to refresh credentials."));
    assert!(report.contains(" id=`"));
    assert!(report.contains(" path=`"));

    remove_temp_root(root);
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

fn orqa_in_pod<const N: usize>(root: &Path, pod: &str, args: [&str; N]) {
    let output = command_in_pod(root, pod, args).output().unwrap();
    assert!(
        output.status.success(),
        "orqa failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn orqa_output_in_pod<const N: usize>(root: &Path, pod: &str, args: [&str; N]) -> String {
    let output = command_in_pod(root, pod, args).output().unwrap();
    assert!(
        output.status.success(),
        "orqa failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn orqa_output_failing<const N: usize>(root: &Path, args: [&str; N]) -> String {
    let output = command(root, args).output().unwrap();
    assert!(
        !output.status.success(),
        "orqa unexpectedly succeeded: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn command<const N: usize>(root: &Path, args: [&str; N]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_orqa"));
    command.current_dir(root);
    command.arg("--home").arg(root).args(args);
    command
}

fn command_in_pod<const N: usize>(root: &Path, pod: &str, args: [&str; N]) -> Command {
    let mut command = command(root, args);
    command.current_dir(pod_root(root, pod));
    command
}

fn create_pod(root: &Path, pod: &str) {
    let pod_root = pod_root(root, pod);
    fs::create_dir_all(&pod_root).unwrap();
    let path = pod_root.to_str().unwrap();
    let output = command(root, ["pod", "create", pod, "--path", path])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "orqa failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn pod_root(root: &Path, pod: &str) -> PathBuf {
    root.join("projects").join(pod)
}

fn pod_home(root: &Path, pod: &str) -> PathBuf {
    pod_root(root, pod).join(".orqa")
}

fn fin_home(root: &Path, pod: &str, fin: &str) -> PathBuf {
    pod_home(root, pod).join("fins").join(fin)
}

fn set_writer_backend(root: &Path, pod: &str) {
    let pod_config = pod_home(root, pod).join("pod.toml");
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
    let pod_config = pod_home(root, pod).join("pod.toml");
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
    let pod_config = pod_home(root, pod).join("pod.toml");
    let config = fs::read_to_string(&pod_config).unwrap();
    let config = config.replace("exec_always = \"0\"\n", "");
    let config = config.replace("debounce = \"5m\"\n", policy);
    fs::write(&pod_config, config).unwrap();
}

fn set_pwd_backend(root: &Path, pod: &str) {
    let pod_config = pod_home(root, pod).join("pod.toml");
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
    let pod_config = pod_home(root, pod).join("pod.toml");
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
    let pod_config = pod_home(root, pod).join("pod.toml");
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
    let root = env::temp_dir().join(format!(
        "orqa-pod-flow-test-{}-{suffix}-{counter}",
        std::process::id(),
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

fn remove_temp_root(root: PathBuf) {
    for _ in 0..20 {
        match fs::remove_dir_all(&root) {
            Ok(()) => return,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::DirectoryNotEmpty | std::io::ErrorKind::PermissionDenied
                ) =>
            {
                thread::sleep(Duration::from_millis(25));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => panic!("failed to remove temp root {}: {error}", root.display()),
        }
    }
    fs::remove_dir_all(&root)
        .unwrap_or_else(|error| panic!("failed to remove temp root {}: {error}", root.display()));
}
