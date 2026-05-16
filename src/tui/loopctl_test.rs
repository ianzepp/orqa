use crate::model::Orqa;

use super::{TUI_LOOP_PROMPT, tui_loop_prompt_args_json, tui_wake_command_args};

#[test]
fn serializes_default_tui_loop_prompt_args() {
    let raw = match tui_loop_prompt_args_json() {
        Ok(raw) => raw,
        Err(error) => panic!("failed to serialize prompt args: {error}"),
    };
    let args: Vec<String> = match serde_json::from_str(&raw) {
        Ok(args) => args,
        Err(error) => panic!("failed to parse prompt args JSON: {error}"),
    };

    assert_eq!(args, vec![TUI_LOOP_PROMPT.to_string()]);
}

#[test]
fn builds_forced_wake_command_for_tui_prompt() {
    let orqa = Orqa::new(Some("/tmp/orqa-home".into()));
    let args: Vec<String> = tui_wake_command_args(&orqa)
        .into_iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();

    assert_eq!(
        args,
        vec![
            "--home",
            "/tmp/orqa-home",
            "wake",
            "--force",
            "--",
            TUI_LOOP_PROMPT,
        ]
    );
}
