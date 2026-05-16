use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

use crate::tui::{
    app::App,
    view::{
        layout::centered_rect,
        style::{bold, fg, titled_block},
    },
};

pub(super) fn render_command_palette(app: &App, frame: &mut Frame, area: Rect) {
    let palette = centered_rect(area, 64, 15);
    let block = titled_block(" Commands ", app.theme.text);
    let inner = block.inner(palette);
    let key = bold(app.theme.accent);
    let text = fg(app.theme.text);
    let rows = vec![
        key_row(" i", " chat with target", key, text),
        key_row(" c", " open chat history", key, text),
        key_row(" m", " open mail", key, text),
        key_row(" Ctrl+T", " choose target fin", key, text),
        key_row(" F", " cycle timeline fin filter", key, text),
        key_row(" o", " toggle operator filter", key, text),
        key_row(" /", " toggle thread filter", key, text),
        key_row(" H", " cycle theme", key, text),
        key_row(" P", " pause/resume pod wake loop", key, text),
        key_row(" PgUp/PgDn", " scroll timeline", key, text),
        key_row(" Esc", " close palette or leave chat", key, text),
        key_row(" q", " quit", key, text),
        Line::from(Span::styled(
            " Ctrl+. closes this palette",
            fg(app.theme.muted),
        )),
    ];

    frame.render_widget(Clear, palette);
    frame.render_widget(block, palette);
    frame.render_widget(Paragraph::new(rows), inner);
}

pub(super) fn render_target_picker(app: &App, frame: &mut Frame, area: Rect) {
    let targets = app.target_choices();
    let height = (targets.len() as u16 + 2).clamp(5, 14);
    let picker = centered_rect(area, 42, height);
    let block = titled_block(" Target Fin ", app.theme.text);
    let inner = block.inner(picker);
    let selected = app.target_picker_index.min(targets.len().saturating_sub(1));
    let rows = if targets.is_empty() {
        vec![Line::from(Span::styled(
            " no fins available",
            fg(app.theme.muted),
        ))]
    } else {
        targets
            .iter()
            .enumerate()
            .map(|(index, fin)| {
                let style = if index == selected {
                    bold(app.theme.accent)
                } else {
                    fg(app.theme.text)
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

fn key_row(
    key: &'static str,
    description: &'static str,
    key_style: Style,
    text: Style,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(key, key_style),
        Span::styled(description, text),
    ])
}
