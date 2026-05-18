use std::{env, fs};

use clap::Parser;

use crate::{
    cli::{Cli, Command, FinSubcommand, MailSubcommand, TaskListArgs, TemplateSubcommand},
    config::{fin_agents_template, fin_config_template, pod_agents_template, pod_config_template},
    mailbox::{
        TaskFilters, canonical_task_body, deliver_mail, ensure_maildir, mark_task_done, message_id,
        priority_sort_value, quote_value, remove_sleep_marker, resolve_address,
        resolve_message_path, unique_mail_name, write_sleep_marker,
    },
    model::{FinRef, MailAddress, PodRef, validate_slug},
    runtime::lock_pid,
};

#[test]
fn cli_command_help_is_compact_and_direct() {
    let help = {
        let mut command = crate::cli_command();
        command.render_help().to_string()
    };

    assert!(help.contains("Coordinate local agent pods and fins"));
    assert!(help.contains("Usage: orqa [OPTIONS] [COMMAND]"));
    assert!(help.contains("Options:"));
    assert!(help.contains("  -v, --version     Print version"));
    assert!(help.contains("Commands:"));
    assert!(help.contains("guide"));
    assert!(help.contains("Print the operational guide"));
    assert!(help.contains("template"));
    assert!(help.contains("Manage pod templates"));
}

#[test]
fn parses_global_pod_and_fin_flags_at_subcommand_depth() {
    let cli = Cli::try_parse_from([
        "orqa",
        "mail",
        "list",
        "--pod",
        "sample-pod",
        "--fin",
        "builder",
    ])
    .unwrap();

    assert_eq!(cli.context_pod.as_deref(), Some("sample-pod"));
    assert_eq!(cli.context_fin.as_deref(), Some("builder"));
    assert!(matches!(
        cli.command,
        Some(Command::Mail(command))
            if matches!(command.command, MailSubcommand::List(_))
    ));
}

#[test]
fn fin_commands_can_omit_positional_context_when_global_flags_are_present() {
    let cli = Cli::try_parse_from([
        "orqa",
        "--pod",
        "sample-pod",
        "--fin",
        "builder",
        "fin",
        "status",
    ])
    .unwrap();

    assert_eq!(cli.context_pod.as_deref(), Some("sample-pod"));
    assert_eq!(cli.context_fin.as_deref(), Some("builder"));
    assert!(matches!(
        cli.command,
        Some(Command::Fin(command))
            if matches!(command.command, FinSubcommand::Status(_))
    ));
}

#[test]
fn parses_global_context_flags_after_nested_fin_commands() {
    let cli = Cli::try_parse_from([
        "orqa",
        "fin",
        "status",
        "--pod",
        "sample-pod",
        "--fin",
        "builder",
    ])
    .unwrap();

    assert_eq!(cli.context_pod.as_deref(), Some("sample-pod"));
    assert_eq!(cli.context_fin.as_deref(), Some("builder"));
    assert!(matches!(
        cli.command,
        Some(Command::Fin(command))
            if matches!(command.command, FinSubcommand::Status(_))
    ));
}

#[test]
fn parses_global_context_flags_after_nested_pod_commands() {
    let cli = Cli::try_parse_from([
        "orqa",
        "pod",
        "tail",
        "--pod",
        "sample-pod",
        "--fin",
        "builder",
    ])
    .unwrap();

    assert_eq!(cli.context_pod.as_deref(), Some("sample-pod"));
    assert_eq!(cli.context_fin.as_deref(), Some("builder"));
    assert!(matches!(
        cli.command,
        Some(Command::Pod(command))
            if matches!(command.command, crate::cli::PodSubcommand::Tail(_))
    ));
}

#[test]
fn parses_pod_create_template_flag() {
    let cli = Cli::try_parse_from([
        "orqa",
        "pod",
        "create",
        "new-co",
        "--template",
        "executive",
        "--path",
        "/tmp/new-co",
    ])
    .unwrap();

    assert!(matches!(
        cli.command,
        Some(Command::Pod(command))
            if matches!(&command.command, crate::cli::PodSubcommand::Create(args)
                if args.template.as_deref() == Some("executive") && args.slug == "new-co")
    ));
}

#[test]
fn parses_init_template_flag() {
    let cli = Cli::try_parse_from([
        "orqa",
        "init",
        "new-co",
        "--template",
        "executive",
        "--path",
        "/tmp/new-co",
    ])
    .unwrap();

    assert!(matches!(
        cli.command,
        Some(Command::Init(args))
            if args.template.as_deref() == Some("executive") && args.slug.as_deref() == Some("new-co")
    ));
}

#[test]
fn parses_template_authoring_commands() {
    let create = Cli::try_parse_from(["orqa", "template", "create", "executive"]).unwrap();
    assert!(matches!(
        create.command,
        Some(Command::Template(command))
            if matches!(&command.command, TemplateSubcommand::Create(args)
                if args.template == "executive")
    ));

    let fin = Cli::try_parse_from([
        "orqa",
        "template",
        "fin",
        "create",
        "executive",
        "ceo",
        "--role",
        "Own company direction.",
    ])
    .unwrap();
    assert!(matches!(
        fin.command,
        Some(Command::Template(command))
            if matches!(&command.command, TemplateSubcommand::Fin(_))
    ));

    let sync = Cli::try_parse_from([
        "orqa",
        "--pod",
        "launch-team",
        "template",
        "sync",
        "executive",
        "--dry-run",
    ])
    .unwrap();
    assert_eq!(sync.context_pod.as_deref(), Some("launch-team"));
    assert!(matches!(
        sync.command,
        Some(Command::Template(command))
            if matches!(&command.command, TemplateSubcommand::Sync(args)
                if args.template == "executive" && args.dry_run)
    ));
}

#[test]
fn parses_daemon_command() {
    let cli =
        Cli::try_parse_from(["orqa", "daemon", "--interval", "30", "--", "handle work"]).unwrap();

    assert!(matches!(
        cli.command,
        Some(Command::Daemon(args))
            if args.interval == 30 && args.args == vec![std::ffi::OsString::from("handle work")]
    ));
}

#[test]
fn parses_templates_alias() {
    let cli = Cli::try_parse_from(["orqa", "templates", "list"]).unwrap();

    assert!(matches!(
        cli.command,
        Some(Command::Template(command))
            if matches!(command.command, TemplateSubcommand::List)
    ));
}

#[test]
fn fin_commands_reject_positional_pod_context() {
    let error = Cli::try_parse_from(["orqa", "fin", "status", "sample-pod", "builder"])
        .unwrap_err()
        .to_string();

    assert!(error.contains("unexpected argument 'builder'"));
}

#[test]
fn accepts_lowercase_slug_parts() {
    assert!(validate_slug("sample-pod").is_ok());
    assert!(validate_slug("bob-jones").is_ok());
    assert!(validate_slug("amy2").is_ok());
}

#[test]
fn rejects_path_like_slugs() {
    assert!(validate_slug("../sample-pod").is_err());
    assert!(validate_slug("SamplePod").is_err());
    assert!(validate_slug("").is_err());
}

#[test]
fn parses_local_mail_addresses() {
    let address = MailAddress::parse("amy@sample-pod.orqa").unwrap();

    assert_eq!(address.fin, "amy");
    assert_eq!(address.pod, "sample-pod");
    assert_eq!(address.label(), "amy@sample-pod.orqa");
}

#[test]
fn qualifies_bare_mail_addresses_with_pod_hint() {
    let address = resolve_address("bob-jones", Some("sample-pod")).unwrap();

    assert_eq!(address.fin, "bob-jones");
    assert_eq!(address.pod, "sample-pod");
    assert_eq!(address.label(), "bob-jones@sample-pod.orqa");
}

#[test]
fn bare_mail_addresses_need_pod_context() {
    assert!(resolve_address("bob-jones", None).is_err());
}

#[test]
fn rejects_non_orqa_mail_addresses() {
    assert!(MailAddress::parse("amy@example.com").is_err());
    assert!(MailAddress::parse("amy").is_err());
    assert!(MailAddress::parse("Amy@sample-pod.orqa").is_err());
}

#[test]
fn resolves_message_ids_in_maildir_states() {
    let root = env::temp_dir().join(format!("orqa-test-{}", unique_mail_name().unwrap()));
    let mail_home = root.join("mail");
    ensure_maildir(&mail_home).unwrap();
    let path = deliver_mail(&mail_home, "Subject: test\n\nbody\n").unwrap();
    let id = message_id(&path).unwrap();

    assert_eq!(resolve_message_path(&mail_home, &id).unwrap(), path);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn canonicalizes_plain_task_bodies() {
    let from = MailAddress::parse("amy@sample-pod.orqa").unwrap();
    let to = MailAddress::parse("bob-jones@sample-pod.orqa").unwrap();
    let task = canonical_task_body(&from, &to, Some("update-settings"), "Do the thing.");

    assert!(task.starts_with("---\n"));
    assert!(task.contains("from: amy@sample-pod.orqa\n"));
    assert!(task.contains("to: bob-jones@sample-pod.orqa\n"));
    assert!(task.contains("title: update-settings\n"));
    assert!(task.contains("priority: normal\n"));
    assert!(task.contains("status: open\n"));
    assert!(task.contains("kind: need\n"));
    assert!(task.contains("depends_on: []\n"));
    assert!(task.ends_with("Do the thing.\n"));
}

#[test]
fn preserves_and_fills_task_front_matter() {
    let from = MailAddress::parse("amy@sample-pod.orqa").unwrap();
    let to = MailAddress::parse("bob-jones@sample-pod.orqa").unwrap();
    let task = canonical_task_body(
        &from,
        &to,
        None,
        "---\ntitle: supplied-title\npriority: high\ncustom: keep-me\n---\n\nDetails.",
    );

    assert!(task.contains("title: supplied-title\n"));
    assert!(task.contains("priority: high\n"));
    assert!(task.contains("custom: keep-me\n"));
    assert!(task.contains("status: open\n"));
    assert!(task.contains("kind: need\n"));
    assert!(task.ends_with("Details.\n"));
}

#[test]
fn marks_task_front_matter_done() {
    let task = mark_task_done(
        "---\ntitle: Ship it\nstatus: open\npriority: high\n---\n\nComplete the work.",
    );

    assert!(task.contains("title: Ship it\n"));
    assert!(task.contains("status: done\n"));
    assert!(task.contains("priority: high\n"));
    assert!(task.ends_with("Complete the work.\n"));
}

#[test]
fn parses_yaml_task_front_matter_values() {
    let from = MailAddress::parse("amy@sample-pod.orqa").unwrap();
    let to = MailAddress::parse("bob-jones@sample-pod.orqa").unwrap();
    let task = canonical_task_body(
        &from,
        &to,
        None,
        "---\ntitle: \"fix: parser\"\ndepends_on: [first, second]\n---\n\nDetails.",
    );

    assert!(task.contains("title: \"fix: parser\"\n"));
    assert!(task.contains("depends_on: [first, second]\n"));
    assert!(task.ends_with("Details.\n"));
}

#[test]
fn parses_task_field_filters() {
    let args = TaskListArgs {
        all: false,
        status: Some("open".to_string()),
        priority: Some("high".to_string()),
        kind: None,
        fields: vec!["owner=amy".to_string()],
        sort: None,
        reverse: false,
    };
    let filters = TaskFilters::new(&args).unwrap();

    assert_eq!(
        filters.fields,
        vec![
            ("status".to_string(), "open".to_string()),
            ("priority".to_string(), "high".to_string()),
            ("owner".to_string(), "amy".to_string())
        ]
    );
}

#[test]
fn quotes_shell_unfriendly_values() {
    assert_eq!(quote_value("high"), "high");
    assert_eq!(quote_value("update settings"), "\"update settings\"");
    assert_eq!(quote_value("say \"hi\""), "\"say \\\"hi\\\"\"");
}

#[test]
fn sorts_known_priorities_by_severity() {
    assert!(priority_sort_value("high") < priority_sort_value("normal"));
    assert!(priority_sort_value("normal") < priority_sort_value("low"));
}

#[test]
fn parses_lock_pid() {
    assert_eq!(lock_pid("pid=123\nfin=amy\n"), Some(123));
    assert_eq!(lock_pid("fin=amy\n"), None);
}

#[test]
fn writes_and_removes_sleep_markers() {
    let root = env::temp_dir().join(format!("orqa-test-{}", unique_mail_name().unwrap()));
    let marker = root.join("sleep.lock");

    write_sleep_marker(&marker).unwrap();
    assert!(marker.exists());
    remove_sleep_marker(&marker).unwrap();
    assert!(!marker.exists());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn pod_config_template_enables_builtin_backends() {
    let pod = PodRef::new("sample-pod").unwrap();
    let toml = pod_config_template(&pod);

    assert!(toml.contains("[pod]"));
    assert!(toml.contains("slug = \"sample-pod\""));
    assert!(toml.contains("debounce = \"5m\""));
    assert!(toml.contains("exec_always = \"0\""));
    assert!(toml.contains("[backends.codex]"));
    assert!(toml.contains("command = \"codex\""));
    assert!(toml.contains("\"--skip-git-repo-check\""));
    assert!(toml.contains("\"--sandbox\", \"workspace-write\""));
    assert!(toml.contains("\"--cd\", \"{pod_root}\""));
    assert!(toml.contains(
        "chat_args = [\n    \"--sandbox\", \"workspace-write\",\n    \"--cd\", \"{pod_root}\",\n    \"--model\", \"{model}\",\n]"
    ));
    assert!(toml.contains("[backends.opencode]"));
    assert!(toml.contains("[backends.hermes]"));
    assert!(toml.contains("exec_args = [\"--model\", \"{model}\", \"--oneshot\", \"{prompt}\"]"));
    assert!(toml.contains("[backends.pi]"));
    assert!(toml.contains("    \"--session-dir\", \"{fin_home}/.pi/sessions\","));
    assert!(toml.contains("[backends.grok]"));
    assert!(toml.contains(
        "exec_args = [\"-p\", \"{prompt}\", \"--output-format\", \"streaming-json\", \"--always-approve\"]"
    ));
    assert!(toml.contains("[backends.ollama_codex]"));
    assert!(toml.contains("    \"launch\", \"codex\","));
    assert!(toml.contains("[backends.ollama_codex.defaults]"));
    assert!(toml.contains("# [backends.custom]"));
}

#[test]
fn pod_agents_template_documents_orqa_commands() {
    let pod = PodRef::new("sample-pod").unwrap();
    let markdown = pod_agents_template(&pod, "Build the thing.");

    assert!(markdown.contains("sample-pod"));
    assert!(markdown.contains("Build the thing."));
    assert!(markdown.contains("orqa fin list"));
    assert!(markdown.contains("orqa mail send --to <fin>"));
    assert!(markdown.contains("operator@$ORQA_POD.orqa"));
    assert!(markdown.contains("operator@ops.orqa"));
    assert!(markdown.contains("orqa task send --to <fin>"));
}

#[test]
fn fin_config_template_inherits_pod_backend_by_default() {
    let fin = FinRef::new("sample-pod", "amy").unwrap();
    let toml = fin_config_template(&fin);

    assert!(toml.contains("[fin]"));
    assert!(toml.contains("slug = \"amy\""));
    assert!(toml.contains("# backend = \"codex\""));
    assert!(toml.contains("# debounce = \"5m\""));
    assert!(toml.contains("# exec_always = \"3h\""));
}

#[test]
fn fin_agents_template_names_fin_role_stub() {
    let fin = FinRef::new("sample-pod", "planner").unwrap();
    let markdown = fin_agents_template(&fin, "Plan the work.");

    assert!(markdown.contains("planner"));
    assert!(markdown.contains("sample-pod"));
    assert!(markdown.contains("required_context:"));
    assert!(markdown.contains(".orqa/CHARTER.md"));
    assert!(markdown.contains("Before acting, read every path"));
    assert!(markdown.contains("Plan the work."));
}
