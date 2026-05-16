use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
};

use crate::tui::{
    app::App,
    events::{Event, LogStream},
    view::{
        markdown::render_markdown,
        style::{bold, fg, strong},
    },
};

pub(super) fn render(app: &mut App, frame: &mut Frame, area: Rect) {
    let visible_events = app.visible_events();
    let visible_count = visible_events.len();
    let items: Vec<ListItem> = visible_events
        .into_iter()
        .map(|event| event_to_item(app, event, area.width))
        .collect();

    let list = List::new(items).highlight_style(bold(app.theme.text));

    if let Some(selected) = app.list_state.selected() {
        if selected >= visible_count && visible_count > 0 {
            app.list_state.select(Some(visible_count - 1));
        }
    }

    if visible_count == 0 {
        render_empty(app, frame, area);
    } else {
        frame.render_stateful_widget(list, area, &mut app.list_state);
    }
}

fn render_empty(app: &App, frame: &mut Frame, area: Rect) {
    let empty = Line::from(vec![
        Span::styled(" waiting for pod activity", fg(app.theme.muted)),
        Span::styled("  |  ", fg(app.theme.muted)),
        Span::styled(
            "new mail, locks, runs, and logs will appear here",
            fg(app.theme.muted),
        ),
    ]);
    frame.render_widget(Paragraph::new(empty), area);
}

fn event_to_item(app: &App, event: &Event, width: u16) -> ListItem<'static> {
    ListItem::new(event_to_lines(app, event, width))
}

fn event_to_lines(app: &App, event: &Event, width: u16) -> Vec<Line<'static>> {
    match event {
        Event::LogLine { fin, stream, line } => {
            let color = match stream {
                LogStream::Stdout => app.theme.stdout,
                LogStream::Stderr => app.theme.error,
                LogStream::Event => app.theme.event,
            };
            let prefix = vec![fin_tag(fin, fg(app.theme.accent)), Span::raw(" ")];
            let prefix_width = fin_tag_width(fin) + 1;
            if *stream == LogStream::Stdout {
                let content_width = usize::from(width).saturating_sub(prefix_width).max(1);
                prefixed_lines(
                    prefix,
                    prefix_width,
                    render_markdown(line, content_width, fg(color), &app.theme),
                )
            } else {
                prefixed_wrapped_lines(prefix, prefix_width, line, fg(color), width)
            }
        }
        Event::MailArrived {
            fin, from, subject, ..
        } => {
            let subject = subject.clone().unwrap_or_else(|| "(no subject)".into());
            let from = from.clone().unwrap_or_else(|| "?".into());
            let prefix_width =
                fin_tag_width(fin) + " inbox ← ".chars().count() + from.chars().count() + 2;
            prefixed_wrapped_lines(
                vec![
                    fin_tag(fin, fg(app.theme.mail)),
                    Span::raw(" inbox ← "),
                    Span::styled(from, fg(app.theme.warn)),
                    Span::raw("  "),
                ],
                prefix_width,
                &subject,
                strong(),
                width,
            )
        }
        Event::RunStarted { fin, run_id } => prefixed_wrapped_lines(
            vec![fin_tag(fin, fg(app.theme.ok)), Span::raw(" run started ")],
            fin_tag_width(fin) + " run started ".chars().count(),
            run_id,
            fg(app.theme.text),
            width,
        ),
        Event::RunFinished {
            fin,
            run_id,
            exit_code,
        } => {
            let status = exit_code.map_or("?".to_string(), |code| code.to_string());
            prefixed_wrapped_lines(
                vec![fin_tag(fin, fg(app.theme.ok)), Span::raw(" run finished ")],
                fin_tag_width(fin) + " run finished ".chars().count(),
                &format!("{run_id} (exit {status})"),
                fg(app.theme.text),
                width,
            )
        }
        Event::LockAcquired { fin } => vec![Line::from(vec![
            fin_tag(fin, fg(app.theme.warn)),
            Span::raw(" acquired lock"),
        ])],
        Event::LockReleased { fin } => vec![Line::from(vec![
            fin_tag(fin, fg(app.theme.warn)),
            Span::raw(" released lock"),
        ])],
        Event::OperatorAction { text } => {
            let mut lines = vec![Line::from(Span::styled(" operator", fg(app.theme.muted)))];
            lines.extend(wrapped_plain_lines(text, 2, fg(app.theme.text), width));
            lines.push(Line::from(Span::raw("")));
            lines
        }
    }
}

fn fin_tag(fin: &str, style: ratatui::style::Style) -> Span<'static> {
    Span::styled(format!("[{}]", fin), style)
}

fn fin_tag_width(fin: &str) -> usize {
    fin.chars().count() + 2
}

fn prefixed_wrapped_lines(
    prefix: Vec<Span<'static>>,
    prefix_width: usize,
    text: &str,
    text_style: ratatui::style::Style,
    width: u16,
) -> Vec<Line<'static>> {
    let text_width = usize::from(width).saturating_sub(prefix_width).max(1);
    let chunks = wrap_text(text, text_width);
    let mut lines = Vec::new();

    for (index, chunk) in chunks.into_iter().enumerate() {
        if index == 0 {
            let mut spans = prefix.clone();
            spans.push(Span::styled(chunk, text_style));
            lines.push(Line::from(spans));
        } else {
            lines.push(Line::from(vec![
                Span::raw(" ".repeat(prefix_width)),
                Span::styled(chunk, text_style),
            ]));
        }
    }

    lines
}

fn prefixed_lines(
    prefix: Vec<Span<'static>>,
    prefix_width: usize,
    content_lines: Vec<Line<'static>>,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (index, line) in content_lines.into_iter().enumerate() {
        if index == 0 {
            let mut spans = prefix.clone();
            spans.extend(line.spans);
            lines.push(Line::from(spans));
        } else {
            let mut spans = vec![Span::raw(" ".repeat(prefix_width))];
            spans.extend(line.spans);
            lines.push(Line::from(spans));
        }
    }
    lines
}

fn wrapped_plain_lines(
    text: &str,
    indent: usize,
    style: ratatui::style::Style,
    width: u16,
) -> Vec<Line<'static>> {
    let text_width = usize::from(width).saturating_sub(indent).max(1);
    wrap_text(text, text_width)
        .into_iter()
        .map(|chunk| {
            Line::from(vec![
                Span::raw(" ".repeat(indent)),
                Span::styled(chunk, style),
            ])
        })
        .collect()
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    for raw_line in text.lines() {
        if raw_line.is_empty() {
            lines.push(String::new());
            continue;
        }

        lines.extend(wrap_text_line(raw_line, width));
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

fn wrap_text_line(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        let current_width = current.chars().count();
        let word_width = word.chars().count();
        if current_width == 0 {
            push_wrapped_word(&mut lines, &mut current, word, width);
        } else if current_width + 1 + word_width <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            push_wrapped_word(&mut lines, &mut current, word, width);
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}

fn push_wrapped_word(lines: &mut Vec<String>, current: &mut String, word: &str, width: usize) {
    let mut remaining = word;
    while remaining.chars().count() > width {
        let split_at = remaining
            .char_indices()
            .nth(width)
            .map(|(index, _)| index)
            .unwrap_or(remaining.len());
        lines.push(remaining[..split_at].to_string());
        remaining = &remaining[split_at..];
    }
    current.push_str(remaining);
}

#[cfg(test)]
#[path = "timeline_test.rs"]
mod tests;
