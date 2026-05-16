use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
};

use crate::tui::{
    app::App,
    events::{Event, LogStream},
    view::style::{bold, fg, strong},
};

pub(super) fn render(app: &mut App, frame: &mut Frame, area: Rect) {
    let visible_events = app.visible_events();
    let visible_count = visible_events.len();
    let items: Vec<ListItem> = visible_events
        .into_iter()
        .map(|event| event_to_item(app, event))
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

fn event_to_item(app: &App, event: &Event) -> ListItem<'static> {
    ListItem::new(event_to_lines(app, event))
}

fn event_to_lines(app: &App, event: &Event) -> Vec<Line<'static>> {
    match event {
        Event::LogLine { fin, stream, line } => {
            let color = match stream {
                LogStream::Stdout => app.theme.stdout,
                LogStream::Stderr => app.theme.error,
                LogStream::Event => app.theme.event,
            };
            vec![Line::from(vec![
                fin_tag(fin, fg(app.theme.accent)),
                Span::raw(" "),
                Span::styled(line.clone(), fg(color)),
            ])]
        }
        Event::MailArrived {
            fin, from, subject, ..
        } => {
            let subject = subject.clone().unwrap_or_else(|| "(no subject)".into());
            let from = from.clone().unwrap_or_else(|| "?".into());
            vec![Line::from(vec![
                fin_tag(fin, fg(app.theme.mail)),
                Span::raw(" inbox ← "),
                Span::styled(from, fg(app.theme.warn)),
                Span::raw("  "),
                Span::styled(subject, strong()),
            ])]
        }
        Event::RunStarted { fin, run_id } => vec![Line::from(vec![
            fin_tag(fin, fg(app.theme.ok)),
            Span::raw(format!(" run started {}", run_id)),
        ])],
        Event::RunFinished {
            fin,
            run_id,
            exit_code,
        } => {
            let status = exit_code.map_or("?".to_string(), |code| code.to_string());
            vec![Line::from(vec![
                fin_tag(fin, fg(app.theme.ok)),
                Span::raw(format!(" run finished {} (exit {})", run_id, status)),
            ])]
        }
        Event::LockAcquired { fin } => vec![Line::from(vec![
            fin_tag(fin, fg(app.theme.warn)),
            Span::raw(" acquired lock"),
        ])],
        Event::LockReleased { fin } => vec![Line::from(vec![
            fin_tag(fin, fg(app.theme.warn)),
            Span::raw(" released lock"),
        ])],
        Event::OperatorAction { text } => vec![
            Line::from(Span::styled(" operator", fg(app.theme.muted))),
            Line::from(Span::styled(format!("  {}", text), fg(app.theme.text))),
            Line::from(Span::raw("")),
        ],
    }
}

fn fin_tag(fin: &str, style: ratatui::style::Style) -> Span<'static> {
    Span::styled(format!("[{}]", fin), style)
}
