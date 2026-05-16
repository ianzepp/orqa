use super::{TUI_LOOP_PROMPT, tui_loop_prompt_args_json};

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
