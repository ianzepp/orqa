use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::tui::{app::App, view::layout::centered_rect};

pub(super) fn render_command_palette(app: &App, frame: &mut Frame, area: Rect) {
    let palette = centered_rect(area, 64, 13);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Commands ")
        .style(Style::default().fg(app.theme.text));
    let inner = block.inner(palette);
    let dim = Style::default().fg(app.theme.muted);
    let key = Style::default()
        .fg(app.theme.accent)
        .add_modifier(Modifier::BOLD);
    let text = Style::default().fg(app.theme.text);
    let rows = vec![
        Line::from(vec![
            Span::styled(" i", key),
            Span::styled(" compose message", text),
        ]),
        Line::from(vec![
            Span::styled(" Ctrl+T", key),
            Span::styled(" choose message target", text),
        ]),
        Line::from(vec![
            Span::styled(" F", key),
            Span::styled(" cycle timeline fin filter", text),
        ]),
        Line::from(vec![
            Span::styled(" o", key),
            Span::styled(" toggle operator filter", text),
        ]),
        Line::from(vec![
            Span::styled(" /", key),
            Span::styled(" toggle thread filter", text),
        ]),
        Line::from(vec![
            Span::styled(" H", key),
            Span::styled(" cycle theme", text),
        ]),
        Line::from(vec![
            Span::styled(" P", key),
            Span::styled(" pause/resume pod wake loop", text),
        ]),
        Line::from(vec![
            Span::styled(" PgUp/PgDn", key),
            Span::styled(" scroll timeline", text),
        ]),
        Line::from(vec![
            Span::styled(" Esc", key),
            Span::styled(" close palette or leave input", text),
        ]),
        Line::from(vec![Span::styled(" q", key), Span::styled(" quit", text)]),
        Line::from(Span::styled(" Ctrl+. closes this palette", dim)),
    ];

    frame.render_widget(Clear, palette);
    frame.render_widget(block, palette);
    frame.render_widget(Paragraph::new(rows), inner);
}

pub(super) fn render_target_picker(app: &App, frame: &mut Frame, area: Rect) {
    let targets = app.target_choices();
    let height = (targets.len() as u16 + 2).clamp(5, 14);
    let picker = centered_rect(area, 42, height);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Target Fin ")
        .style(Style::default().fg(app.theme.text));
    let inner = block.inner(picker);
    let selected = app.target_picker_index.min(targets.len().saturating_sub(1));
    let rows = if targets.is_empty() {
        vec![Line::from(Span::styled(
            " no fins available",
            Style::default().fg(app.theme.muted),
        ))]
    } else {
        targets
            .iter()
            .enumerate()
            .map(|(index, fin)| {
                let style = if index == selected {
                    Style::default()
                        .fg(app.theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(app.theme.text)
                };
                let marker = if index == selected { ">" } else { " " };
                Line::from(vec![
                    Span::styled(format!(" {marker} "), style),
                    Span::styled(format!("@{fin}"), style),
                ])
            })
            .collect()
    };

    frame.render_widget(Clear, picker);
    frame.render_widget(block, picker);
    frame.render_widget(Paragraph::new(rows), inner);
}
