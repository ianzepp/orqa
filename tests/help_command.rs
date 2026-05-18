use std::process::Command;

#[test]
fn guide_command_prints_embedded_operational_guide() {
    let output = Command::new(env!("CARGO_BIN_EXE_orqa"))
        .arg("guide")
        .output()
        .expect("run orqa guide");

    assert!(
        output.status.success(),
        "orqa guide failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("help output is utf-8");
    assert!(stdout.contains("# Orqa Operational Guide"));
    assert!(stdout.contains("## Mail"));
    assert!(stdout.contains("## Tasks"));
    assert!(stdout.contains("orqa wake --dry-run"));
}

#[test]
fn help_command_prints_cli_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_orqa"))
        .arg("help")
        .output()
        .expect("run orqa help");

    assert!(
        output.status.success(),
        "orqa help failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("help output is utf-8");
    assert!(stdout.contains("Coordinate local agent pods and fins"));
    assert!(stdout.contains("Usage: orqa [OPTIONS] [COMMAND]"));
    assert!(stdout.contains("Options:\n"));
    assert!(stdout.contains("  -v, --version     Print version"));
    assert!(stdout.contains("Commands:"));
    assert!(stdout.contains("guide"));
    assert!(!stdout.contains("# Orqa Operational Guide"));
}

#[test]
fn public_command_tree_has_help_text() {
    let commands: &[&[&str]] = &[
        &["doctor"],
        &["top"],
        &["daemon"],
        &["init"],
        &["pod"],
        &["pod", "create"],
        &["pod", "charter"],
        &["pod", "charter", "get"],
        &["pod", "charter", "set"],
        &["pod", "hook"],
        &["pod", "hook", "add"],
        &["pod", "hook", "enable"],
        &["pod", "hook", "disable"],
        &["pod", "hook", "remove"],
        &["pod", "hook", "run"],
        &["pod", "tail"],
        &["fin"],
        &["fin", "create"],
        &["fin", "role"],
        &["fin", "role", "get"],
        &["fin", "role", "set"],
        &["fin", "runs"],
        &["fin", "run-status"],
        &["fin", "run-log"],
        &["fin", "tail"],
        &["fin", "exec"],
        &["fin", "chat"],
        &["mail"],
        &["mail", "send"],
        &["mail", "list"],
        &["mail", "read"],
        &["mail", "done"],
        &["mail", "delete"],
        &["mail", "unread"],
        &["task"],
        &["task", "send"],
        &["task", "list"],
        &["task", "read"],
        &["task", "done"],
        &["task", "delete"],
        &["template"],
        &["template", "create"],
        &["template", "sync"],
        &["template", "fin"],
        &["template", "fin", "list"],
        &["template", "fin", "create"],
        &["ops"],
        &["ops", "report"],
        &["wake"],
        &["loop"],
    ];

    for command in commands {
        let output = Command::new(env!("CARGO_BIN_EXE_orqa"))
            .args(*command)
            .arg("--help")
            .output()
            .unwrap_or_else(|error| panic!("run orqa {} --help: {error}", command.join(" ")));

        assert!(
            output.status.success(),
            "orqa {} --help failed: {}",
            command.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8(output.stdout).expect("help output is utf-8");
        assert!(
            stdout.contains("Usage:"),
            "orqa {} --help did not print usage:\n{stdout}",
            command.join(" ")
        );
        assert!(
            stdout.contains("Options:") || stdout.contains("Commands:"),
            "orqa {} --help did not print options or commands:\n{stdout}",
            command.join(" ")
        );
    }
}

#[test]
fn short_version_matches_version_output() {
    let output = Command::new(env!("CARGO_BIN_EXE_orqa"))
        .arg("-v")
        .output()
        .expect("run orqa -v");

    assert!(
        output.status.success(),
        "orqa -v failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("version output is utf-8");
    assert!(stdout.starts_with("orqa "));
}
