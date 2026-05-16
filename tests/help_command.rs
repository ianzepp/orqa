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
