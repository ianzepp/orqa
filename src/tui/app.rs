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

use super::composer::Composer;
use super::events::{Event, LogStream};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputMode {
    Normal,
    Input,
}

// Tasteful dense terminal colors (inspired by trading / operator terminals)
const BAR_BG: Color = Color::Rgb(0x1F, 0x23, 0x2A); // Dark slate
const HEADER_BG: Color = Color::Rgb(0x2A, 0x3F, 0x4A); // Muted teal-slate
const ACCENT: Color = Color::Rgb(0x7D, 0xD3, 0xFC); // Soft cyan
const MUTED: Color = Color::Rgb(0x8B, 0x94, 0x9E);
const HIGHLIGHT: Color = Color::Rgb(0xF4, 0xA2, 0x61); // Warm amber for important items
const WHITE: Color = Color::Rgb(0xE6, 0xE6, 0xE6);
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

    /// The bottom composer (Phase 4)
    pub composer: Composer,

    /// Current input mode (Normal = monitoring hotkeys, Input = composer owns keys)
    pub mode: InputMode,
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
            composer: Composer::new("planner".to_string()), // temporary default; will be improved
            mode: InputMode::Normal,
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

    /// Render the full UI — denser operator cockpit style.
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Shortcut bar
                Constraint::Length(1), // Header bar (pod identity + mode)
                Constraint::Length(1), // Pod status bar (live metrics)
                Constraint::Min(5),    // Main timeline
                Constraint::Length(1), // Composer input line
            ])
            .split(area);

        self.render_shortcut_bar(frame, chunks[0]);
        self.render_header_bar(frame, chunks[1]);
        self.render_pod_status_bar(frame, chunks[2]);
        self.render_timeline(frame, chunks[3]);
        self.composer.render(frame, chunks[4], &self.pod_slug);
    }

    /// Top shortcut bar — compact keyboard legend.
    fn render_shortcut_bar(&self, frame: &mut Frame, area: Rect) {
        let key = Style::default().fg(ACCENT).add_modifier(Modifier::BOLD);
        let label = Style::default().fg(MUTED);

        let spans = vec![
            Span::styled(" ", label),
            Span::styled("[i]", key),
            Span::styled(" Input   ", label),
            Span::styled("[f]", key),
            Span::styled(" Target   ", label),
            Span::styled("[o]", key),
            Span::styled(" Op.Mail   ", label),
            Span::styled("[w]", key),
            Span::styled(" Wake   ", label),
            Span::styled("[q]", key),
            Span::styled(" Quit", label),
        ];

        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    /// Colored header bar with pod name and mode indicator.
    fn render_header_bar(&self, frame: &mut Frame, area: Rect) {
        let mode = match self.mode {
            InputMode::Normal => "[NORMAL]",
            InputMode::Input => "[INPUT]",
        };
        let mode_style = if self.mode == InputMode::Input {
            Style::default().fg(HIGHLIGHT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(MUTED)
        };

        let left = format!(" {} ", self.pod_slug);
        let right = format!(" {}  {} ", mode, self.pod_root.display());

        let width = area.width as usize;
        let display = if left.len() + right.len() <= width {
            format!(
                "{}{}{}",
                left,
                " ".repeat(width - left.len() - right.len()),
                right
            )
        } else {
            left
        };

        let line = Line::from(Span::styled(
            display,
            Style::default()
                .fg(WHITE)
                .bg(HEADER_BG)
                .add_modifier(Modifier::BOLD),
        ));

        frame.render_widget(Paragraph::new(line), area);
    }

    /// Dense pod status bar with live operational metrics.
    fn render_pod_status_bar(&self, frame: &mut Frame, area: Rect) {
        let style = Style::default().fg(WHITE).bg(BAR_BG);
        let dim = Style::default().fg(MUTED).bg(BAR_BG);
        let good = Style::default().fg(ACCENT).bg(BAR_BG);

        let fin_count = self.known_fins.len();

        let left = vec![
            Span::styled(format!(" {} fins", fin_count), style),
            Span::styled("  ·  ", dim),
            Span::styled("2 wakeable", good),
            Span::styled("  ·  ", dim),
            Span::styled("1 locked", dim),
            Span::styled("  ·  ", dim),
            Span::styled("3 op.mail", good),
        ];

        let right = vec![
            Span::styled("Loop: running", style),
            Span::styled("  ·  ", dim),
            Span::styled("Target: ", dim),
            Span::styled(&self.composer.target_fin, good),
        ];

        let left_len: usize = left.iter().map(|s| s.width()).sum();
        let right_len: usize = right.iter().map(|s| s.width()).sum();
        let gap = area.width.saturating_sub((left_len + right_len) as u16) as usize;

        let mut spans = left;
        spans.push(Span::styled(" ".repeat(gap), dim));
        spans.extend(right);

        frame.render_widget(Paragraph::new(Line::from(spans)), area);
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
