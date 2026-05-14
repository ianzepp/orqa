use std::process::Command;

#[test]
fn help_command_prints_embedded_operational_guide() {
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
    assert!(stdout.contains("# Orqa Operational Guide"));
    assert!(stdout.contains("## Mail"));
    assert!(stdout.contains("## Tasks"));
    assert!(stdout.contains("orqa loop sample-pod"));
}
