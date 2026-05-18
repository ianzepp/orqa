use super::{
    backend_event_json_to_summary, codex_tool_output_to_summary, grok_streaming_json_to_markdown,
    wrap_text,
};

#[test]
fn wraps_text_to_available_width() {
    assert_eq!(
        wrap_text("alpha beta gamma", 10),
        vec!["alpha beta".to_string(), "gamma".to_string()]
    );
}

#[test]
fn keeps_explicit_newlines_as_continuation_lines() {
    assert_eq!(
        wrap_text("first line\nsecond line", 80),
        vec!["first line".to_string(), "second line".to_string()]
    );
}

#[test]
fn splits_words_longer_than_available_width() {
    assert_eq!(
        wrap_text("abcdefghij", 4),
        vec!["abcd".to_string(), "efgh".to_string(), "ij".to_string()]
    );
}

#[test]
fn converts_grok_streaming_text_chunks_to_markdown() {
    assert_eq!(
        grok_streaming_json_to_markdown(r#"{"type":"text","data":"Hello"}"#),
        Some("Hello".to_string())
    );
}

#[test]
fn joins_multiple_grok_streaming_text_events() {
    assert_eq!(
        grok_streaming_json_to_markdown(
            r#"{"type":"text","data":"Hello"}
{"type":"text","data":" world"}"#
        ),
        Some("Hello world".to_string())
    );
}

#[test]
fn renders_grok_thought_events_as_blockquotes() {
    assert_eq!(
        grok_streaming_json_to_markdown(r#"{"type":"thought","data":"checking mail"}"#),
        Some("> thinking: checking mail\n\n".to_string())
    );
}

#[test]
fn joins_tokenized_grok_thought_events() {
    assert_eq!(
        grok_streaming_json_to_markdown(
            r#"{"type":"thought","data":"The"}
{"type":"thought","data":" user"}
{"type":"thought","data":" said"}
{"type":"thought","data":" \""}
{"type":"thought","data":" hi"}
{"type":"thought","data":" \"."}
{"type":"text","data":"Hello!"}"#
        ),
        Some("> thinking: The user said \"hi\".\n\nHello!".to_string())
    );
}

#[test]
fn renders_unknown_grok_streaming_events_with_detail() {
    assert_eq!(
        grok_streaming_json_to_markdown(r#"{"type":"tool_call","data":{"name":"orqa"}}"#),
        Some("`tool_call` orqa\n".to_string())
    );
}

#[test]
fn leaves_non_streaming_output_alone() {
    assert_eq!(grok_streaming_json_to_markdown("plain markdown"), None);
}

#[test]
fn summarizes_backend_lifecycle_events() {
    assert_eq!(
        backend_event_json_to_summary(
            r#"{"command":"grok -p hi --output-format streaming-json --always-approve","event":"planned"}
{"event":"spawned","pid":"48293"}"#
        ),
        Some(
            "planned: grok -p hi --output-format streaming-json --always-approve\nspawned pid 48293"
                .to_string()
        )
    );
}

#[test]
fn hides_duplicate_backend_finished_events() {
    assert_eq!(
        backend_event_json_to_summary(r#"{"event":"finished","exit_code":"0"}"#),
        Some(String::new())
    );
}

#[test]
fn summarizes_single_codex_exec_success() {
    let input = r#"exec
       /bin/zsh -lc "orqa mail list --pod hanta-monitor --fin cxo --all | sed -n '1,120p'" in /Users/ianzepp/work/hanta/hanta-monitor
       succeeded in 0ms:
       some output here"#;
    let summary = codex_tool_output_to_summary(input).unwrap();
    assert!(!summary.failed);
    assert!(summary.text.contains("✓ exec"));
    assert!(summary.text.contains("orqa mail list"));
}

#[test]
fn summarizes_single_codex_exec_failure() {
    let input = r#"exec
       /bin/zsh -lc "bad command" in /some/path
       failed in 1ms:
       error output"#;
    let summary = codex_tool_output_to_summary(input).unwrap();
    assert!(summary.failed);
    assert!(summary.text.contains("✗ exec"));
    assert!(summary.text.contains("(failed)"));
}

#[test]
fn summarizes_multiple_codex_tool_calls() {
    let input = r#"exec
       /bin/zsh -lc "orqa mail list" in /path
       succeeded in 0ms:
       
exec
       /bin/zsh -lc "orqa fin status cxo" in /path
       succeeded in 0ms:
       fin hanta-monitor/cxo
       home=/Users/ianzepp/work/hanta/.orqa/fins/cxo
       sleeping=false
       running=false
       pid=42528
       unread_mail=0
       open_tasks=0
       last_run=1779106984587716.42520.0
       last_status=running
       
exec
       /bin/zsh -lc "orqa task list" in /path
       succeeded in 0ms:
       codex
       Checked and handled."#;
    let summary = codex_tool_output_to_summary(input).unwrap();
    assert!(!summary.failed);
    assert!(summary.text.contains("✓ exec"));
    // Should contain all three tool calls
    assert!(summary.text.contains("orqa mail list"));
    assert!(summary.text.contains("orqa fin status cxo"));
    assert!(summary.text.contains("orqa task list"));
}

#[test]
fn returns_none_for_non_codex_output() {
    assert!(codex_tool_output_to_summary("plain text output").is_none());
    assert!(codex_tool_output_to_summary("").is_none());
    assert!(codex_tool_output_to_summary("not a tool name\n  indented stuff").is_none());
}

#[test]
fn truncates_long_commands() {
    let long_cmd = "a".repeat(200);
    let input = format!("exec\n       {} in /path\n       succeeded in 0ms:", long_cmd);
    let summary = codex_tool_output_to_summary(&input).unwrap();
    // The command portion after "✓ exec: " should be truncated at 80 chars
    let colon_pos = summary.text.find(": ").unwrap();
    let cmd_part = &summary.text[colon_pos + 2..];
    assert!(cmd_part.len() <= 83); // 80 + "..." 
}
