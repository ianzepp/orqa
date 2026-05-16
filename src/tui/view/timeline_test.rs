use super::{grok_streaming_json_to_markdown, wrap_text};

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
        Some("> thinking: checking mail\n".to_string())
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
