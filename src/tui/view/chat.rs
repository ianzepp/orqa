use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::tui::{
    app::{App, ChatItem, ChatMode},
    view::{
        style::{fg, strong},
        timeline::grok_streaming_json_to_markdown,
    },
};

pub(super) fn render(app: &mut App, frame: &mut Frame, area: Rect) {
    match app.chat_mode {
        ChatMode::Index => render_index(app, frame, area),
        ChatMode::Detail => render_detail(app, frame, area),
    }
}

pub(super) fn render_status(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .style(fg(app.theme.muted));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            chat_status_line(app),
            fg(app.theme.text),
        ))),
        inner,
    );
}

pub(super) fn render_footer(app: &App, frame: &mut Frame, area: Rect) {
    let help = match app.chat_mode {
        ChatMode::Index => "Enter:open  j/k:move  g/G:top/bottom  Tab:timeline",
        ChatMode::Detail => "j/k:scroll  PgUp/PgDn:page  g/G:top/bottom  i:index  Tab:timeline",
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {help}"),
            fg(app.theme.muted),
        ))),
        area,
    );
}

fn render_index(app: &mut App, frame: &mut Frame, area: Rect) {
    let items = app
        .chat_items
        .iter()
        .enumerate()
        .map(|(index, item)| ListItem::new(index_row(app, index + 1, item)))
        .collect::<Vec<_>>();
    let selected = if app.chat_items.is_empty() {
        None
    } else {
        Some(app.chat_cursor.min(app.chat_items.len() - 1))
    };
    let mut state = ListState::default().with_selected(selected);
    frame.render_stateful_widget(
        List::new(items)
            .highlight_symbol("")
            .highlight_style(Style::default().fg(app.theme.panel_bg).bg(app.theme.accent)),
        area,
        &mut state,
    );
}

fn render_detail(app: &mut App, frame: &mut Frame, area: Rect) {
    let Some(item) = app.selected_chat().cloned() else {
        frame.render_widget(Paragraph::new("No chat selected"), area);
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(1)])
        .split(area);
    let headers = vec![
        header_line("To", &format!("@{}", item.record.fin)),
        header_line("Prompt", &item.record.prompt),
        header_line("Run", &item.record.run_id),
        header_line("State", &chat_state(&item)),
        Line::raw(""),
    ];
    frame.render_widget(Paragraph::new(headers), chunks[0]);

    let body_width = usize::from(chunks[1].width).max(1);
    let body_height = usize::from(chunks[1].height).max(1);
    let (visible_lines, scroll) = visible_chat_lines(
        render_chat_body(&item, body_width, fg(app.theme.text), fg(app.theme.error)),
        app.chat_scroll,
        body_height,
    );
    app.chat_scroll = scroll;

    frame.render_widget(Paragraph::new(visible_lines), chunks[1]);
}

fn index_row(app: &App, number: usize, item: &ChatItem) -> Line<'static> {
    let status = truncate(&chat_state(item), 10);
    let fin = truncate(&item.record.fin, 16);
    let prompt = truncate(&item.record.prompt, 80);
    let base = match item.run.as_ref().map(|run| run.status.as_str()) {
        Some("finished") => fg(app.theme.muted),
        Some("running") => fg(app.theme.text).add_modifier(Modifier::BOLD),
        _ => fg(app.theme.warn),
    };

    Line::from(vec![
        Span::styled(format!("{number:4} "), base),
        Span::styled(format!("{status:<10} "), fg(app.theme.event)),
        Span::styled(format!("@{fin:<16} "), fg(app.theme.mail)),
        Span::styled(prompt, base),
    ])
}

fn chat_status_line(app: &App) -> String {
    let count = app.chat_items.len();
    let selected = if count == 0 {
        0
    } else {
        app.chat_cursor.min(count - 1) + 1
    };

    match app.chat_mode {
        ChatMode::Index => format!(" -- Chat [Msg:{selected}/{count}] --"),
        ChatMode::Detail => {
            let line = app.chat_scroll.saturating_add(1);
            format!(" -- Chat: response [Msg:{selected}/{count} Line:{line}] --")
        }
    }
}

fn chat_state(item: &ChatItem) -> String {
    item.run
        .as_ref()
        .map(|run| run.status.clone())
        .unwrap_or_else(|| "missing".to_string())
}

fn render_chat_body(
    item: &ChatItem,
    width: usize,
    text_style: ratatui::style::Style,
    error_style: ratatui::style::Style,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.extend(wrap_line("You:", width, strong()));
    lines.extend(wrap_line(&item.record.prompt, width, text_style));
    lines.push(Line::raw(""));
    lines.extend(wrap_line(
        &format!("@{}:", item.record.fin),
        width,
        strong(),
    ));

    if item.stdout.trim().is_empty() && item.stderr.trim().is_empty() {
        lines.extend(wrap_line("(waiting for response)", width, text_style));
    } else {
        let stdout =
            grok_streaming_json_to_markdown(&item.stdout).unwrap_or_else(|| item.stdout.clone());
        lines.extend(render_preserved_text(&stdout, width, text_style));
        if !item.stderr.trim().is_empty() {
            lines.push(Line::raw(""));
            lines.extend(wrap_line("stderr:", width, strong()));
            lines.extend(render_preserved_text(&item.stderr, width, error_style));
        }
    }

    lines
}

fn render_preserved_text(
    text: &str,
    width: usize,
    style: ratatui::style::Style,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for raw_line in text.lines() {
        if raw_line.is_empty() {
            lines.push(Line::raw(""));
        } else {
            lines.extend(wrap_line(raw_line, width, style));
        }
    }
    lines
}

fn visible_chat_lines(
    lines: Vec<Line<'static>>,
    scroll: usize,
    height: usize,
) -> (Vec<Line<'static>>, usize) {
    let height = height.max(1);
    let max_scroll = lines.len().saturating_sub(height);
    let scroll = scroll.min(max_scroll);
    let visible = lines
        .into_iter()
        .skip(scroll)
        .take(height)
        .collect::<Vec<_>>();

    if visible.is_empty() {
        (vec![Line::raw("")], 0)
    } else {
        (visible, scroll)
    }
}

fn wrap_line(text: &str, width: usize, style: ratatui::style::Style) -> Vec<Line<'static>> {
    let width = width.max(1);
    if text.chars().count() <= width {
        return vec![Line::from(Span::styled(text.to_string(), style))];
    }

    let mut remaining = text.to_string();
    let mut lines = Vec::new();
    while !remaining.is_empty() {
        let split = wrap_split(&remaining, width);
        lines.push(Line::from(Span::styled(
            remaining[..split].trim_end().to_string(),
            style,
        )));
        remaining = remaining[split..].trim_start().to_string();
    }
    lines
}

fn wrap_split(text: &str, width: usize) -> usize {
    if text.chars().count() <= width {
        return text.len();
    }

    let hard_split = text
        .char_indices()
        .nth(width)
        .map(|(index, _)| index)
        .unwrap_or(text.len());
    text[..hard_split]
        .char_indices()
        .rev()
        .find(|(_, ch)| ch.is_whitespace())
        .map(|(index, ch)| index + ch.len_utf8())
        .filter(|index| *index > 0)
        .unwrap_or(hard_split)
}

fn header_line(label: &'static str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label}: "), strong()),
        Span::raw(value.to_string()),
    ])
}

fn truncate(value: &str, width: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(width).collect::<String>();
    if chars.next().is_some() && width > 1 {
        let mut shortened = truncated
            .chars()
            .take(width.saturating_sub(1))
            .collect::<String>();
        shortened.push('~');
        shortened
    } else {
        truncated
    }
}

#[cfg(test)]
#[path = "chat_test.rs"]
mod tests;
