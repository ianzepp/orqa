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
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

#[allow(unused_imports)]
use crate::model::PodRegistration;

use super::composer::Composer;
use super::events::{Event, LogStream};
use super::theme::{OPERATOR_DARK, THEMES, Theme};
use super::watcher::PodWatcher;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputMode {
    Normal,
    Input,
}

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
    #[allow(dead_code)]
    pub pod_root: std::path::PathBuf,
    pub watcher: Option<PodWatcher>,
    pub events: Vec<Event>,
    pub filters: FilterState,
    pub list_state: ListState,
    pub follow: bool,
    pub known_fins: HashSet<String>,
    pub locked_fins: HashSet<String>,
    pub active_fins: HashSet<String>,
    pub max_events: usize,

    /// The bottom composer (Phase 4)
    pub composer: Composer,

    /// Current input mode (Normal = monitoring hotkeys, Input = composer owns keys)
    pub mode: InputMode,

    pub theme: Theme,
    pub expanded: bool,
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
            locked_fins: HashSet::new(),
            active_fins: HashSet::new(),
            max_events: 2000,
            composer: Composer::new("planner".to_string()), // temporary default; will be improved
            mode: InputMode::Normal,
            theme: OPERATOR_DARK,
            expanded: true,
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
                    self.apply_event_state(&ev);
                    self.events.push(ev);

                    // Bound the buffer
                    if self.events.len() > self.max_events {
                        self.events.remove(0);
                    }
                }
            }
        }
    }

    fn apply_event_state(&mut self, ev: &Event) {
        match ev {
            Event::RunStarted { fin, .. } => {
                self.active_fins.insert(fin.clone());
            }
            Event::RunFinished { fin, .. } => {
                self.active_fins.remove(fin);
            }
            Event::LockAcquired { fin } => {
                self.locked_fins.insert(fin.clone());
            }
            Event::LockReleased { fin } => {
                self.locked_fins.remove(fin);
            }
            Event::LogLine { .. } | Event::MailArrived { .. } | Event::OperatorAction { .. } => {}
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

    #[allow(dead_code)]
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

    pub fn cycle_theme(&mut self) {
        let current = THEMES
            .iter()
            .position(|theme| theme.name == self.theme.name)
            .unwrap_or(0);
        self.theme = THEMES[(current + 1) % THEMES.len()];
    }

    /// Render the cockpit as four main sections: header, content, input, footer.
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let gap = u16::from(self.expanded);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),   // header
                Constraint::Length(gap), // expanded spacing
                Constraint::Min(0),      // content
                Constraint::Length(gap), // expanded spacing
                Constraint::Length(3),   // bordered input
                Constraint::Length(gap), // expanded spacing
                Constraint::Length(1),   // footer
                Constraint::Length(gap), // expanded spacing
            ])
            .split(area);

        self.render_header(frame, self.section_area(chunks[0]));
        self.render_timeline(frame, self.section_area(chunks[2]));
        self.render_input_area(frame, self.section_area(chunks[4]));
        self.render_footer(frame, self.section_area(chunks[6]));
    }

    fn section_area(&self, area: Rect) -> Rect {
        if !self.expanded || area.width <= 2 {
            return area;
        }

        Rect {
            x: area.x + 1,
            width: area.width - 2,
            ..area
        }
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let base = Style::default()
            .fg(self.theme.text)
            .bg(self.theme.header_bg);
        let accent = Style::default()
            .fg(self.theme.accent)
            .bg(self.theme.header_bg);
        let dim = Style::default()
            .fg(self.theme.muted)
            .bg(self.theme.header_bg);
        let warn = Style::default()
            .fg(self.theme.warn)
            .bg(self.theme.header_bg);
        let mode = match self.mode {
            InputMode::Normal => ("monitor", dim),
            InputMode::Input => ("compose", warn),
        };

        let spans = vec![
            Span::styled(" ", base),
            Span::styled(&self.pod_slug, accent),
            Span::styled("  ", base),
            Span::styled(mode.0, mode.1),
            Span::styled("  |  target ", dim),
            Span::styled(&self.composer.target_fin, accent),
            Span::styled(format!("  |  {} events", self.events.len()), base),
        ];

        self.fill(frame, area, self.theme.header_bg);
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    /// Three-row input section: one text row plus a border around all sides.
    fn render_input_area(&self, frame: &mut Frame, area: Rect) {
        self.fill(frame, area, self.theme.bar_bg);
        let block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(self.theme.muted).bg(self.theme.bar_bg));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.mode == InputMode::Normal {
            let text = Line::from(vec![
                Span::styled(
                    " >",
                    Style::default().fg(self.theme.accent).bg(self.theme.bar_bg),
                ),
                Span::styled(
                    " press i to write to the target fin",
                    Style::default().fg(self.theme.muted).bg(self.theme.bar_bg),
                ),
            ]);
            frame.render_widget(Paragraph::new(text), inner);
        } else {
            self.composer
                .render(frame, inner, &self.pod_slug, &self.theme);
        }
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let dim = Style::default().fg(self.theme.muted).bg(self.theme.bar_bg);
        let help = match self.mode {
            InputMode::Normal => {
                "i compose  f target  F filter  o operator  / thread  H theme  q quit"
            }
            InputMode::Input => "Enter send  Esc monitor  Tab target",
        };

        self.fill(frame, area, self.theme.bar_bg);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(format!(" {help}"), dim))),
            area,
        );
    }

    fn fill(&self, frame: &mut Frame, area: Rect, color: Color) {
        let bg_fill = " ".repeat(area.width as usize);
        frame.render_widget(
            Paragraph::new(bg_fill).style(Style::default().bg(color)),
            area,
        );
    }

    fn render_timeline(&mut self, frame: &mut Frame, area: Rect) {
        let visible_events = self.visible_events();
        let visible_count = visible_events.len();
        let items: Vec<ListItem> = visible_events
            .into_iter()
            .map(|ev| {
                ListItem::new(self.event_to_line(ev))
                    .style(Style::default().bg(self.theme.panel_bg))
            })
            .collect();

        let list = List::new(items)
            .style(Style::default().bg(self.theme.panel_bg))
            .highlight_style(
                Style::default()
                    .fg(self.theme.text)
                    .bg(self.theme.header_bg)
                    .add_modifier(Modifier::BOLD),
            );

        // Keep selection in bounds
        if let Some(selected) = self.list_state.selected() {
            if selected >= visible_count && visible_count > 0 {
                self.list_state.select(Some(visible_count - 1));
            }
        }

        self.fill(frame, area, self.theme.panel_bg);
        if visible_count == 0 {
            let empty = Line::from(vec![
                Span::styled(
                    " waiting for pod activity",
                    Style::default().fg(self.theme.muted),
                ),
                Span::styled("  |  ", Style::default().fg(self.theme.muted)),
                Span::styled(
                    "new mail, locks, runs, and logs will appear here",
                    Style::default().fg(self.theme.muted),
                ),
            ]);
            frame.render_widget(
                Paragraph::new(empty).style(Style::default().bg(self.theme.panel_bg)),
                area,
            );
        } else {
            frame.render_stateful_widget(list, area, &mut self.list_state);
        }
    }

    fn event_to_line(&self, ev: &Event) -> Line<'static> {
        match ev {
            Event::LogLine { fin, stream, line } => {
                let color = match stream {
                    LogStream::Stdout => self.theme.stdout,
                    LogStream::Stderr => self.theme.error,
                    LogStream::Event => self.theme.event,
                };
                Line::from(vec![
                    Span::styled(format!("[{}]", fin), Style::default().fg(self.theme.accent)),
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
                    Span::styled(format!("[{}]", fin), Style::default().fg(self.theme.mail)),
                    Span::raw(" mail "),
                    Span::styled(from_str, Style::default().fg(self.theme.warn)),
                    Span::raw(" → "),
                    Span::styled(subj, Style::default().add_modifier(Modifier::BOLD)),
                ])
            }
            Event::RunStarted { fin, run_id } => Line::from(vec![
                Span::styled(format!("[{}]", fin), Style::default().fg(self.theme.ok)),
                Span::raw(format!(" run started {}", run_id)),
            ]),
            Event::RunFinished {
                fin,
                run_id,
                exit_code,
            } => {
                let status = exit_code.map_or("?".to_string(), |c| c.to_string());
                Line::from(vec![
                    Span::styled(format!("[{}]", fin), Style::default().fg(self.theme.ok)),
                    Span::raw(format!(" run finished {} (exit {})", run_id, status)),
                ])
            }
            Event::LockAcquired { fin } => Line::from(vec![
                Span::styled(format!("[{}]", fin), Style::default().fg(self.theme.warn)),
                Span::raw(" acquired lock"),
            ]),
            Event::LockReleased { fin } => Line::from(vec![
                Span::styled(format!("[{}]", fin), Style::default().fg(self.theme.warn)),
                Span::raw(" released lock"),
            ]),
            Event::OperatorAction { text } => Line::from(vec![
                Span::styled(
                    "[operator]",
                    Style::default()
                        .fg(self.theme.mail)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(" {}", text)),
            ]),
        }
    }

    #[allow(dead_code)]
    fn render_status(&self, frame: &mut Frame, area: Rect) {
        let help = match self.mode {
            InputMode::Normal => {
                "i = input mode   |   q = quit   |   f = target fin   |   o = operator filter"
            }
            InputMode::Input => "Esc = monitoring   |   Ctrl+W = delete word   |   Enter = send",
        };

        let status = Paragraph::new(vec![Line::from(vec![
            Span::styled(help, Style::default().fg(Color::DarkGray)),
            Span::raw("   |   events: "),
            Span::styled(
                self.events.len().to_string(),
                Style::default().fg(Color::Cyan),
            ),
        ])]);

        frame.render_widget(status, area);
    }
}
