use ratatui::text::Line;

use super::{visible_chat_lines, wrap_line};

#[test]
fn visible_chat_lines_clamps_scroll() {
    let lines = vec![
        Line::raw("one"),
        Line::raw("two"),
        Line::raw("three"),
        Line::raw("four"),
    ];
    let (visible, scroll) = visible_chat_lines(lines, usize::MAX, 2);

    assert_eq!(scroll, 2);
    assert_eq!(plain_lines(&visible), vec!["three", "four"]);
}

#[test]
fn wrap_line_prefers_word_boundaries() {
    let lines = wrap_line("alpha beta gamma", 10, ratatui::style::Style::default());

    assert_eq!(plain_lines(&lines), vec!["alpha", "beta gamma"]);
}

fn plain_lines(lines: &[Line<'_>]) -> Vec<String> {
    lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect()
}
