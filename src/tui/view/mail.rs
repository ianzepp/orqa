use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::tui::{
    app::{App, MailComposeField, MailMode, OperatorMail},
    view::style::{fg, strong},
};

pub(super) fn render(app: &mut App, frame: &mut Frame, area: Rect) {
    match app.mail_mode {
        MailMode::Index => render_index(app, frame, area),
        MailMode::Pager => render_pager(app, frame, area),
        MailMode::Compose => render_compose(app, frame, area),
    }
}

pub(super) fn render_status(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .style(fg(app.theme.muted));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let unread = app
        .operator_mail
        .iter()
        .filter(|message| message.state == "new")
        .count();
    let label = match app.mail_mode {
        MailMode::Index => "-- Mail: operator inbox",
        MailMode::Pager => "-- Mail: message",
        MailMode::Compose => "-- Mail: compose",
    };
    let line = format!(
        " {label} [Msgs:{} New:{unread}] --",
        app.operator_mail.len()
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(line, fg(app.theme.text)))),
        inner,
    );
}

pub(super) fn render_footer(app: &App, frame: &mut Frame, area: Rect) {
    let help = match app.mail_mode {
        MailMode::Index => "m:mail  r:reply  Enter:open  j/k:move  Tab:timeline",
        MailMode::Pager => "i/q:inbox  r:reply  Tab:timeline",
        MailMode::Compose => "Ctrl+Y:send  Tab:next  Shift+Tab:prev  Esc:abort",
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
        .operator_mail
        .iter()
        .enumerate()
        .map(|(index, message)| ListItem::new(index_row(app, index + 1, message)))
        .collect::<Vec<_>>();
    let selected = if app.operator_mail.is_empty() {
        None
    } else {
        Some(app.mail_cursor.min(app.operator_mail.len() - 1))
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

fn render_pager(app: &App, frame: &mut Frame, area: Rect) {
    let Some(message) = app.selected_mail() else {
        frame.render_widget(Paragraph::new("No message selected"), area);
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(1)])
        .split(area);
    let headers = vec![
        header_line("From", &message.from),
        header_line("To", &message.to),
        header_line("Subject", &message.subject),
        header_line("State", &message.state),
        header_line("Id", &message.id),
        Line::raw(""),
    ];
    frame.render_widget(Paragraph::new(headers), chunks[0]);

    let body_width = usize::from(chunks[1].width).max(1);
    frame.render_widget(
        Paragraph::new(render_mail_body(
            &message.body,
            body_width,
            fg(app.theme.text),
        )),
        chunks[1],
    );
}

fn render_compose(app: &App, frame: &mut Frame, area: Rect) {
    let Some(compose) = app.mail_compose.as_ref() else {
        frame.render_widget(Paragraph::new("No active composition"), area);
        return;
    };

    let lines = vec![
        header_line("From", &format!("operator@{}.orqa", app.pod_slug)),
        compose_line(
            app,
            "To",
            &compose.to,
            compose.field == MailComposeField::To,
        ),
        compose_line(
            app,
            "Subject",
            &compose.subject,
            compose.field == MailComposeField::Subject,
        ),
        Line::raw(""),
        Line::styled(
            "---- Message body ------------------------------------------------------------",
            fg(app.theme.muted),
        ),
        compose_body(app, &compose.body, compose.field == MailComposeField::Body),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn index_row(app: &App, number: usize, message: &OperatorMail) -> Line<'static> {
    let unread = if message.state == "new" { "N" } else { " " };
    let from = truncate(&message.from, 28);
    let subject = truncate(&message.subject, 64);
    let base = if message.state == "new" {
        fg(app.theme.text).add_modifier(Modifier::BOLD)
    } else {
        fg(app.theme.muted)
    };

    Line::from(vec![
        Span::styled(format!("{number:4} "), base),
        Span::styled(
            unread.to_string(),
            fg(app.theme.warn).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", base),
        Span::styled(format!("{from:<28} "), fg(app.theme.mail)),
        Span::styled(subject, base),
    ])
}

fn header_line(label: &'static str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label}: "), strong()),
        Span::raw(value.to_string()),
    ])
}

fn compose_line(app: &App, label: &'static str, value: &str, active: bool) -> Line<'static> {
    let label_style = if active {
        fg(app.theme.accent).add_modifier(Modifier::BOLD)
    } else {
        strong()
    };
    let cursor = if active { "│" } else { "" };
    Line::from(vec![
        Span::styled(format!("{label:<8} "), label_style),
        Span::raw(value.to_string()),
        Span::styled(cursor, fg(app.theme.cursor)),
    ])
}

fn compose_body(app: &App, body: &str, active: bool) -> Line<'static> {
    let cursor = if active { "│" } else { "" };
    if body.is_empty() {
        return Line::from(vec![
            Span::styled("~ ", fg(app.theme.muted)),
            Span::styled(cursor, fg(app.theme.cursor)),
        ]);
    }
    Line::from(vec![
        Span::raw(body.to_string()),
        Span::styled(cursor, fg(app.theme.cursor)),
    ])
}

fn render_mail_body(body: &str, width: usize, style: ratatui::style::Style) -> Vec<Line<'static>> {
    let width = width.max(1);
    let mut lines = Vec::new();
    for raw_line in body.lines() {
        if raw_line.is_empty() {
            lines.push(Line::raw(""));
        } else {
            lines.extend(wrap_mail_line(raw_line, width, style));
        }
    }

    if lines.is_empty() {
        lines.push(Line::raw(""));
    }
    lines
}

fn wrap_mail_line(line: &str, width: usize, style: ratatui::style::Style) -> Vec<Line<'static>> {
    if line.chars().count() <= width {
        return vec![styled_line(line, style)];
    }

    let indent = line
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .collect::<String>();
    let continuation_indent = if indent.chars().count() < width {
        indent
    } else {
        String::new()
    };
    let mut remaining = line.to_string();
    let mut lines = Vec::new();
    let mut first = true;

    while !remaining.is_empty() {
        let prefix = if first { "" } else { &continuation_indent };
        let available = width.saturating_sub(prefix.chars().count()).max(1);
        let split = mail_wrap_split(&remaining, available);
        let chunk = remaining[..split].trim_end().to_string();
        lines.push(styled_line(&format!("{prefix}{chunk}"), style));
        remaining = remaining[split..].trim_start().to_string();
        first = false;
    }

    lines
}

fn mail_wrap_split(line: &str, width: usize) -> usize {
    if line.chars().count() <= width {
        return line.len();
    }

    let hard_split = line
        .char_indices()
        .nth(width)
        .map(|(index, _)| index)
        .unwrap_or(line.len());
    line[..hard_split]
        .char_indices()
        .rev()
        .find(|(_, ch)| ch.is_whitespace())
        .map(|(index, ch)| index + ch.len_utf8())
        .filter(|index| *index > 0)
        .unwrap_or(hard_split)
}

fn styled_line(text: &str, style: ratatui::style::Style) -> Line<'static> {
    Line::from(Span::styled(text.to_string(), style))
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
#[path = "mail_test.rs"]
mod tests;
