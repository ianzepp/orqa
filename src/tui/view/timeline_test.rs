use super::wrap_text;

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
