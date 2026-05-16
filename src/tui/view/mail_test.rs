use ratatui::style::Style;

use super::render_mail_body;

#[test]
fn mail_body_preserves_source_line_breaks() {
    let lines = render_mail_body("alpha\nbeta\ngamma", 80, Style::default());

    assert_eq!(plain_lines(&lines), vec!["alpha", "beta", "gamma"]);
}

#[test]
fn mail_body_preserves_indentation_on_wrapped_lines() {
    let lines = render_mail_body("  alpha beta gamma", 10, Style::default());

    assert_eq!(plain_lines(&lines), vec!["  alpha", "  beta", "  gamma"]);
}

#[test]
fn mail_body_keeps_tree_lines_separate() {
    let lines = render_mail_body("orqa/\n  src/\n    tui/", 80, Style::default());

    assert_eq!(plain_lines(&lines), vec!["orqa/", "  src/", "    tui/"]);
}

fn plain_lines(lines: &[ratatui::text::Line<'_>]) -> Vec<String> {
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
