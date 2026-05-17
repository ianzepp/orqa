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
        key_row(" ?", "full help & key reference", key, text),
        key_row(" i", "chat with target", key, text),
        key_row(" c", "open chat history", key, text),
        key_row(" m", "open mail", key, text),
        key_row(" Ctrl+T", "choose target fin", key, text),
        key_row(" F", "cycle timeline fin filter", key, text),
        key_row(" o", "toggle operator filter", key, text),
        key_row(" /", "toggle thread filter", key, text),
        key_row(" H", "cycle theme", key, text),
        key_row(" P", "pause/resume pod wake loop", key, text),
        key_row(" PgUp/PgDn", "scroll timeline", key, text),
        key_row(" Esc", "close palette or leave chat", key, text),
        key_row(" q", "quit", key, text),
        Line::from(Span::styled(
            " Ctrl+. closes this palette  •  ? for the full guide",
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

pub(super) fn render_help(app: &App, frame: &mut Frame, area: Rect) {
    // A tall centered panel for the full key reference
    let panel = centered_rect(area, 68, 32);
    let block = titled_block(" Help — orqa Operator Cockpit ", app.theme.text);
    let inner = block.inner(panel);

    let key = bold(app.theme.accent);
    let text = fg(app.theme.text);
    let section = bold(app.theme.ok);
    let mut rows: Vec<Line<'static>> = Vec::new();

    // Global / Navigation
    rows.push(Line::from(Span::styled("Global & Navigation", section)));
    rows.push(key_row(" ?", "toggle this help", key, text));
    rows.push(key_row(" q / Esc / Ctrl-C", "quit TUI", key, text));
    rows.push(key_row(" Ctrl+.", "command palette (quick reference)", key, text));
    rows.push(key_row(" Ctrl+T  or  f", "target fin picker / change composer target", key, text));
    rows.push(key_row(" i", "enter chat mode (composer)", key, text));
    rows.push(key_row(" Shift+Tab", "toggle normal/chat input mode", key, text));
    rows.push(key_row(" H", "cycle theme (dark/light)", key, text));
    rows.push(Line::from(Span::styled("", text)));

    // Timeline
    rows.push(Line::from(Span::styled("Timeline (main view)", section)));
    rows.push(key_row(" F", "cycle fin filter", key, text));
    rows.push(key_row(" o / O", "toggle operator-mail filter", key, text));
    rows.push(key_row(" /  or  t", "toggle thread/subject filter", key, text));
    rows.push(key_row(" ↑ / ↓  PgUp/PgDn", "scroll (pauses follow while scrolling)", key, text));
    rows.push(key_row(" P", "pause/resume the pod wake loop", key, text));
    rows.push(Line::from(Span::styled("", text)));

    // Composer (when in chat input mode)
    rows.push(Line::from(Span::styled("Composer (chat with a fin)", section)));
    rows.push(key_row(" Enter", "send prompt directly to target fin (spawns run)", key, text));
    rows.push(key_row(" Tab / f / F", "open target picker while typing", key, text));
    rows.push(key_row(" ↑ / ↓ (with text)", "browse send history", key, text));
    rows.push(key_row(" Ctrl+W", "delete previous word", key, text));
    rows.push(key_row(" Esc", "leave chat input (back to normal)", key, text));
    rows.push(Line::from(Span::styled("", text)));

    // Mail surface
    rows.push(Line::from(Span::styled("Mail surface (m)", section)));
    rows.push(key_row(" m", "open/close mail (operator inbox)", key, text));
    rows.push(key_row(" j/k  Enter  g/G", "navigate / open / top-bottom", key, text));
    rows.push(key_row(" r", "reply to selected mail", key, text));
    rows.push(key_row(" Ctrl+Y (in compose)", "send composed mail + wake target", key, text));
    rows.push(key_row(" Tab (in compose)", "next field (To → Subject → Body)", key, text));
    rows.push(Line::from(Span::styled("", text)));

    // Other surfaces
    rows.push(Line::from(Span::styled("Other surfaces", section)));
    rows.push(key_row(" c / C", "open chat history surface", key, text));
    rows.push(key_row(" Tab (in mail/chat)", "return to timeline", key, text));
    rows.push(Line::from(Span::styled("", text)));

    rows.push(Line::from(Span::styled(
        "Tip: the bottom composer target is shown as @planner · chat. Use f to change who you address.",
        fg(app.theme.muted),
    )));

    frame.render_widget(Clear, panel);
    frame.render_widget(block, panel);
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
        Span::styled("   ", text), // visual padding between hotkey and description
        Span::styled(description, text),
    ])
}
