//! Main application state and rendering for the Operator Cockpit (Phase 3+).
//!
//! This module owns the live event buffer, filters, scroll state, and
//! integrates the `PodWatcher` from Phase 2.

use std::collections::HashSet;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
};

#[allow(unused_imports)]
use crate::model::PodRegistration;

use super::events::{Event, LogStream};
use super::watcher::PodWatcher;

/// Filter state for the timeline.
#[derive(Default, Clone)]
pub struct FilterState {
    pub fin_filter: Option<String>,
    pub only_operator: bool,
    pub thread_query: Option<String>,
}

/// The main TUI application state for Phase 3.
pub struct App {
    pub pod_slug: String,
    pub pod_root: std::path::PathBuf,
    pub watcher: Option<PodWatcher>,
    pub events: Vec<Event>,
    pub filters: FilterState,
    pub list_state: ListState,
    pub follow: bool,
    pub known_fins: HashSet<String>,
    pub max_events: usize,
}

impl App {
    pub fn new(pod_slug: String, pod_root: std::path::PathBuf, watcher: PodWatcher) -> Self {
        let mut app = Self {
            pod_slug,
            pod_root,
            watcher: Some(watcher),
            events: Vec::new(),
            filters: FilterState::default(),
            list_state: ListState::default(),
            follow: true,
            known_fins: HashSet::new(),
            max_events: 2000,
        };
        app.list_state.select(Some(0));
        app
    }

    /// Poll the watcher and append any new events.
    pub fn poll_watcher(&mut self) {
        if let Some(watcher) = &mut self.watcher {
            if let Ok(new_events) = watcher.poll() {
                for ev in new_events {
                    // Track fins we see
                    if let Some(f) = ev.fin() {
                        self.known_fins.insert(f.to_string());
                    }
                    self.events.push(ev);

                    // Bound the buffer
                    if self.events.len() > self.max_events {
                        self.events.remove(0);
                    }
                }
            }
        }
    }

    /// Return the currently visible (filtered) events.
    pub fn visible_events(&self) -> Vec<&Event> {
        self.events
            .iter()
            .filter(|ev| self.event_matches_filters(ev))
            .collect()
    }

    fn event_matches_filters(&self, ev: &Event) -> bool {
        if let Some(ref fin) = self.filters.fin_filter {
            if ev.fin() != Some(fin.as_str()) {
                return false;
            }
        }
        if self.filters.only_operator && !ev.is_operator_related() {
            return false;
        }
        if let Some(ref q) = self.filters.thread_query {
            if !ev.matches_thread(q) {
                return false;
            }
        }
        true
    }

    /// Apply a fin filter (cycle or set).
    pub fn set_fin_filter(&mut self, fin: Option<String>) {
        self.filters.fin_filter = fin;
        self.follow = true;
        self.scroll_to_bottom();
    }

    /// Toggle operator-only filter.
    pub fn toggle_operator_filter(&mut self) {
        self.filters.only_operator = !self.filters.only_operator;
        self.follow = true;
        self.scroll_to_bottom();
    }

    /// Set thread/subject query filter.
    pub fn set_thread_query(&mut self, query: Option<String>) {
        self.filters.thread_query = query;
        self.follow = true;
        self.scroll_to_bottom();
    }

    /// Scroll handling
    pub fn scroll_up(&mut self, amount: usize) {
        self.follow = false;
        let selected = self.list_state.selected().unwrap_or(0);
        let new_sel = selected.saturating_sub(amount);
        self.list_state.select(Some(new_sel));
    }

    pub fn scroll_down(&mut self, amount: usize) {
        let visible = self.visible_events().len();
        if visible == 0 {
            return;
        }
        let selected = self.list_state.selected().unwrap_or(0);
        let new_sel = (selected + amount).min(visible.saturating_sub(1));
        self.list_state.select(Some(new_sel));

        // Resume follow if we reached the bottom
        if new_sel >= visible.saturating_sub(1) {
            self.follow = true;
        }
    }

    pub fn scroll_to_bottom(&mut self) {
        let visible = self.visible_events().len();
        if visible > 0 {
            self.list_state.select(Some(visible - 1));
        }
        self.follow = true;
    }

    /// Call this after new events arrive when in follow mode.
    pub fn auto_follow_if_needed(&mut self) {
        if self.follow {
            self.scroll_to_bottom();
        }
    }

    /// Render the full UI.
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // separator above header
                Constraint::Length(2), // header content
                Constraint::Length(1), // separator below header
                Constraint::Min(5),    // timeline
                Constraint::Length(1), // separator above footer
                Constraint::Length(1), // footer
                Constraint::Length(1), // separator below footer
            ])
            .split(area);

        self.render_separator(frame, chunks[0]);
        self.render_header(frame, chunks[1]);
        self.render_separator(frame, chunks[2]);
        self.render_timeline(frame, chunks[3]);
        self.render_separator(frame, chunks[4]);
        self.render_status(frame, chunks[5]);
        self.render_separator(frame, chunks[6]);
    }

    fn render_separator(&self, frame: &mut Frame, area: Rect) {
        let line = "─".repeat(area.width as usize);
        let sep = Paragraph::new(line).style(Style::default().fg(Color::DarkGray));
        frame.render_widget(sep, area);
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let filter_text = self.filter_summary();

        let header = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("orqa • ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    &self.pod_slug,
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(
                    self.pod_root.display().to_string(),
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
            Line::from(vec![
                Span::raw("filters: "),
                Span::styled(filter_text, Style::default().fg(Color::Yellow)),
                Span::raw("   "),
                Span::styled(
                    if self.follow { "FOLLOW" } else { "PAUSED" },
                    if self.follow {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default().fg(Color::Red)
                    },
                ),
            ]),
        ]);

        frame.render_widget(header, area);
    }

    fn filter_summary(&self) -> String {
        let mut parts = vec![];

        if let Some(ref fin) = self.filters.fin_filter {
            parts.push(format!("fin={}", fin));
        } else {
            parts.push("all".to_string());
        }

        if self.filters.only_operator {
            parts.push("operator".to_string());
        }

        if let Some(ref q) = self.filters.thread_query {
            parts.push(format!("thread~\"{}\"", q));
        }

        parts.join(" | ")
    }

    fn render_timeline(&mut self, frame: &mut Frame, area: Rect) {
        let visible = self.visible_events();
        let items: Vec<ListItem> = visible
            .iter()
            .map(|ev| {
                let line = self.event_to_line(ev);
                ListItem::new(line)
            })
            .collect();

        let list =
            List::new(items).highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        // Keep selection in bounds
        if let Some(selected) = self.list_state.selected() {
            if selected >= visible.len() && !visible.is_empty() {
                self.list_state.select(Some(visible.len() - 1));
            }
        }

        frame.render_stateful_widget(list, area, &mut self.list_state);
    }

    fn event_to_line(&self, ev: &Event) -> Line<'static> {
        match ev {
            Event::LogLine { fin, stream, line } => {
                let color = match stream {
                    LogStream::Stdout => Color::Gray,
                    LogStream::Stderr => Color::Red,
                    LogStream::Event => Color::Blue,
                };
                Line::from(vec![
                    Span::styled(format!("[{}]", fin), Style::default().fg(Color::Cyan)),
                    Span::raw(" "),
                    Span::styled(line.clone(), Style::default().fg(color)),
                ])
            }
            Event::MailArrived {
                fin, from, subject, ..
            } => {
                let subj = subject.clone().unwrap_or_else(|| "(no subject)".into());
                let from_str = from.clone().unwrap_or_else(|| "?".into());
                Line::from(vec![
                    Span::styled(format!("[{}]", fin), Style::default().fg(Color::Magenta)),
                    Span::raw(" mail "),
                    Span::styled(from_str, Style::default().fg(Color::Yellow)),
                    Span::raw(" → "),
                    Span::styled(subj, Style::default().add_modifier(Modifier::BOLD)),
                ])
            }
            Event::RunStarted { fin, run_id } => Line::from(vec![
                Span::styled(format!("[{}]", fin), Style::default().fg(Color::Green)),
                Span::raw(format!(" run started {}", run_id)),
            ]),
            Event::RunFinished {
                fin,
                run_id,
                exit_code,
            } => {
                let status = exit_code.map_or("?".to_string(), |c| c.to_string());
                Line::from(vec![
                    Span::styled(format!("[{}]", fin), Style::default().fg(Color::Green)),
                    Span::raw(format!(" run finished {} (exit {})", run_id, status)),
                ])
            }
            Event::LockAcquired { fin } => Line::from(vec![
                Span::styled(format!("[{}]", fin), Style::default().fg(Color::Yellow)),
                Span::raw(" acquired lock"),
            ]),
            Event::LockReleased { fin } => Line::from(vec![
                Span::styled(format!("[{}]", fin), Style::default().fg(Color::Yellow)),
                Span::raw(" released lock"),
            ]),
            Event::OperatorAction { text } => Line::from(vec![
                Span::styled(
                    "[operator]",
                    Style::default()
                        .fg(Color::LightMagenta)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(" {}", text)),
            ]),
        }
    }

    fn render_status(&self, frame: &mut Frame, area: Rect) {
        let status = Paragraph::new(vec![Line::from(vec![
            Span::raw(
                "q/esc: quit  |  f: fin filter  |  o: operator only  |  ↑↓: scroll  |  events: ",
            ),
            Span::styled(
                self.events.len().to_string(),
                Style::default().fg(Color::Cyan),
            ),
        ])]);

        frame.render_widget(status, area);
    }
}
