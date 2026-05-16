use ratatui::style::Style;

use super::render_markdown;
use crate::tui::theme::OPERATOR_DARK;

#[test]
fn renders_bullets_as_wrapped_lines() {
    let lines = render_markdown(
        "- first item with several words",
        12,
        Style::default(),
        &OPERATOR_DARK,
    );

    assert_eq!(
        plain_lines(&lines),
        vec!["- first item", "with several", "words"]
    );
}

#[test]
fn renders_heading_prefix() {
    let lines = render_markdown("# Summary", 80, Style::default(), &OPERATOR_DARK);

    assert_eq!(plain_lines(&lines), vec!["# Summary"]);
}

#[test]
fn keeps_code_block_lines() {
    let lines = render_markdown("```sh\necho hi\n```", 80, Style::default(), &OPERATOR_DARK);

    assert_eq!(plain_lines(&lines), vec!["│ echo hi"]);
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
