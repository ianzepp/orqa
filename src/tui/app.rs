//! Main application state and rendering for the Operator Cockpit (Phase 3+).
//!
//! This module owns the live event buffer, filters, scroll state, and
//! integrates the `PodWatcher` from Phase 2.

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

#[allow(unused_imports)]
use crate::model::PodRegistration;

use super::composer::Composer;
use super::events::{Event, LogStream};
use super::theme::{THEMES, Theme, default_theme};
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
    pub active_since: HashMap<String, Instant>,
    pub max_events: usize,

    /// The bottom composer (Phase 4)
    pub composer: Composer,

    /// Current input mode (Normal = monitoring hotkeys, Input = composer owns keys)
    pub mode: InputMode,

    pub theme: Theme,
    pub expanded: bool,
    pub show_command_palette: bool,
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
            active_since: HashMap::new(),
            max_events: 2000,
            composer: Composer::new("planner".to_string()), // temporary default; will be improved
            mode: InputMode::Normal,
            theme: default_theme(),
            expanded: true,
            show_command_palette: false,
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
                self.active_since
                    .entry(fin.clone())
                    .or_insert_with(Instant::now);
            }
            Event::RunFinished { fin, .. } => {
                self.active_fins.remove(fin);
                if !self.locked_fins.contains(fin) {
                    self.active_since.remove(fin);
                }
            }
            Event::LockAcquired { fin } => {
                self.locked_fins.insert(fin.clone());
                self.active_since
                    .entry(fin.clone())
                    .or_insert_with(Instant::now);
            }
            Event::LockReleased { fin } => {
                self.locked_fins.remove(fin);
                if !self.active_fins.contains(fin) {
                    self.active_since.remove(fin);
                }
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

    pub fn toggle_input_mode(&mut self) {
        self.mode = match self.mode {
            InputMode::Normal => InputMode::Input,
            InputMode::Input => InputMode::Normal,
        };
    }

    pub fn toggle_command_palette(&mut self) {
        self.show_command_palette = !self.show_command_palette;
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

        if self.show_command_palette {
            self.render_command_palette(frame, area);
        }
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
        let icon = if self.any_fin_active() {
            self.spinner_frame()
        } else {
            "◦"
        };
        let left_text_width =
            4 + self.pod_slug.chars().count() + self.pod_root.display().to_string().chars().count();
        let right = self.running_summary();
        let spacer_width = area
            .width
            .saturating_sub(left_text_width as u16)
            .saturating_sub(right.chars().count() as u16)
            .max(1) as usize;

        let spans = vec![
            Span::styled(" ", base),
            Span::styled(icon, accent),
            Span::styled("  ", base),
            Span::styled(&self.pod_slug, accent),
            Span::styled("  ", base),
            Span::styled(self.pod_root.display().to_string(), dim),
            Span::styled(" ".repeat(spacer_width), base),
            Span::styled(right, dim),
            Span::styled(" ", base),
        ];

        self.fill(frame, area, self.theme.header_bg);
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    fn any_fin_active(&self) -> bool {
        !self.active_since.is_empty()
    }

    fn spinner_frame(&self) -> &'static str {
        const FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        FRAMES[((millis / 160) as usize) % FRAMES.len()]
    }

    fn running_summary(&self) -> String {
        if self.active_since.is_empty() {
            return "idle".to_string();
        }

        let mut fins: Vec<_> = self.active_since.iter().collect();
        fins.sort_by_key(|(fin, _)| *fin);
        fins.into_iter()
            .take(3)
            .map(|(fin, since)| format!("{fin} {}", abbreviated_duration(since.elapsed())))
            .collect::<Vec<_>>()
            .join("  ")
    }

    /// Three-row input section: one text row plus a border around all sides.
    fn render_input_area(&self, frame: &mut Frame, area: Rect) {
        self.fill(frame, area, self.theme.bar_bg);
        let block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(self.theme.muted).bg(self.theme.bar_bg));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        self.render_input_label(frame, area);

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

    fn render_input_label(&self, frame: &mut Frame, area: Rect) {
        if area.width < 10 || area.height < 3 {
            return;
        }

        let mode = match self.mode {
            InputMode::Normal => "normal",
            InputMode::Input => "input",
        };
        let label = format!(" @{} · {} ", self.composer.target_fin, mode);
        let width = label.chars().count() as u16;
        if width + 2 >= area.width {
            return;
        }

        let label_area = Rect {
            x: area.x + area.width - width - 2,
            y: area.y + area.height - 1,
            width,
            height: 1,
        };
        let style = Style::default().fg(self.theme.muted).bg(self.theme.bar_bg);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(label, style))),
            label_area,
        );
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let dim = Style::default().fg(self.theme.muted).bg(self.theme.bar_bg);
        let help = "Shift+Tab:mode  |  Ctrl+.:commands";

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
            .map(|ev| self.event_to_item(ev))
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

    fn event_to_item(&self, ev: &Event) -> ListItem<'static> {
        let bg = if matches!(ev, Event::OperatorAction { .. }) {
            self.theme.operator_bg
        } else {
            self.theme.panel_bg
        };

        ListItem::new(self.event_to_lines(ev)).style(Style::default().bg(bg))
    }

    fn event_to_lines(&self, ev: &Event) -> Vec<Line<'static>> {
        match ev {
            Event::LogLine { fin, stream, line } => {
                let color = match stream {
                    LogStream::Stdout => self.theme.stdout,
                    LogStream::Stderr => self.theme.error,
                    LogStream::Event => self.theme.event,
                };
                vec![Line::from(vec![
                    Span::styled(format!("[{}]", fin), Style::default().fg(self.theme.accent)),
                    Span::raw(" "),
                    Span::styled(line.clone(), Style::default().fg(color)),
                ])]
            }
            Event::MailArrived {
                fin, from, subject, ..
            } => {
                let subj = subject.clone().unwrap_or_else(|| "(no subject)".into());
                let from_str = from.clone().unwrap_or_else(|| "?".into());
                vec![Line::from(vec![
                    Span::styled(format!("[{}]", fin), Style::default().fg(self.theme.mail)),
                    Span::raw(" mail "),
                    Span::styled(from_str, Style::default().fg(self.theme.warn)),
                    Span::raw(" → "),
                    Span::styled(subj, Style::default().add_modifier(Modifier::BOLD)),
                ])]
            }
            Event::RunStarted { fin, run_id } => vec![Line::from(vec![
                Span::styled(format!("[{}]", fin), Style::default().fg(self.theme.ok)),
                Span::raw(format!(" run started {}", run_id)),
            ])],
            Event::RunFinished {
                fin,
                run_id,
                exit_code,
            } => {
                let status = exit_code.map_or("?".to_string(), |c| c.to_string());
                vec![Line::from(vec![
                    Span::styled(format!("[{}]", fin), Style::default().fg(self.theme.ok)),
                    Span::raw(format!(" run finished {} (exit {})", run_id, status)),
                ])]
            }
            Event::LockAcquired { fin } => vec![Line::from(vec![
                Span::styled(format!("[{}]", fin), Style::default().fg(self.theme.warn)),
                Span::raw(" acquired lock"),
            ])],
            Event::LockReleased { fin } => vec![Line::from(vec![
                Span::styled(format!("[{}]", fin), Style::default().fg(self.theme.warn)),
                Span::raw(" released lock"),
            ])],
            Event::OperatorAction { text } => vec![
                Line::from(Span::styled(
                    " operator",
                    Style::default().fg(self.theme.muted),
                )),
                Line::from(Span::styled(
                    format!("  {}", text),
                    Style::default().fg(self.theme.text),
                )),
                Line::from(Span::raw("")),
            ],
        }
    }

    fn render_command_palette(&self, frame: &mut Frame, area: Rect) {
        let palette = centered_rect(area, 64, 13);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Commands ")
            .style(
                Style::default()
                    .fg(self.theme.text)
                    .bg(self.theme.overlay_bg),
            );
        let inner = block.inner(palette);
        let dim = Style::default()
            .fg(self.theme.muted)
            .bg(self.theme.overlay_bg);
        let key = Style::default()
            .fg(self.theme.accent)
            .bg(self.theme.overlay_bg)
            .add_modifier(Modifier::BOLD);
        let text = Style::default()
            .fg(self.theme.text)
            .bg(self.theme.overlay_bg);
        let rows = vec![
            Line::from(vec![
                Span::styled(" i", key),
                Span::styled(" compose message", text),
            ]),
            Line::from(vec![
                Span::styled(" f", key),
                Span::styled(" cycle target fin", text),
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

fn centered_rect(area: Rect, max_width: u16, height: u16) -> Rect {
    let width = if area.width > 24 {
        max_width.min(area.width - 4).max(20)
    } else {
        area.width
    };
    let height = if area.height > 9 {
        height.min(area.height - 4).max(5)
    } else {
        area.height
    };
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

fn abbreviated_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds < 60 {
        format!("{}s", seconds.max(1))
    } else if seconds < 60 * 60 {
        format!("{}m", seconds / 60)
    } else {
        format!("{}h", seconds / (60 * 60))
    }
}
