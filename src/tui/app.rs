//! Main application state and rendering for the Operator Cockpit.
//!
//! This module owns the live event buffer, filters, scroll state, and
//! integration with the `PodWatcher`.

use std::{
    collections::{HashMap, HashSet},
    fs,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use crate::{
    mailbox::{deliver_mail, ensure_maildir},
    model::{FinRef, Orqa, PodRegistration},
};

use super::composer::Composer;
use super::events::{Event, LogStream};
use super::loopctl::{TUI_LOOP_INTERVAL, pod_paused, toggle_pod_pause};
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

/// The main TUI application state.
pub struct App {
    pub pod_slug: String,
    pub pod_root: std::path::PathBuf,
    pub orqa: Orqa,
    pub pod: PodRegistration,
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

    /// The bottom message composer.
    pub composer: Composer,

    /// Current input mode (Normal = monitoring hotkeys, Input = composer owns keys)
    pub mode: InputMode,

    pub theme: Theme,
    pub expanded: bool,
    pub show_command_palette: bool,
    pub show_target_picker: bool,
    pub target_picker_index: usize,
    pub pod_paused: bool,
    pub next_loop_at: Instant,
}

impl App {
    pub fn new(
        pod_slug: String,
        pod_root: std::path::PathBuf,
        orqa: Orqa,
        pod: PodRegistration,
        watcher: PodWatcher,
    ) -> Self {
        let paused = pod_paused(&orqa, &pod);
        let known_fins = discover_known_fins(&orqa, &pod);
        let default_target = default_target_fin(&known_fins);
        let mut app = Self {
            pod_slug,
            pod_root,
            orqa,
            pod,
            watcher: Some(watcher),
            events: Vec::new(),
            filters: FilterState::default(),
            list_state: ListState::default(),
            follow: true,
            known_fins,
            locked_fins: HashSet::new(),
            active_fins: HashSet::new(),
            active_since: HashMap::new(),
            max_events: 2000,
            composer: Composer::new(default_target),
            mode: InputMode::Normal,
            theme: default_theme(),
            expanded: true,
            show_command_palette: false,
            show_target_picker: false,
            target_picker_index: 0,
            pod_paused: paused,
            next_loop_at: Instant::now() + TUI_LOOP_INTERVAL,
        };
        app.list_state.select(Some(0));
        app
    }

    /// Poll the watcher and append any new events.
    pub fn poll_watcher(&mut self) {
        let Some(watcher) = &mut self.watcher else {
            return;
        };
        let Ok(new_events) = watcher.poll() else {
            return;
        };

        for event in new_events {
            self.record_event(event);
        }
    }

    fn record_event(&mut self, event: Event) {
        if let Some(fin) = event.fin() {
            self.known_fins.insert(fin.to_string());
        }

        self.apply_event_state(&event);
        self.events.push(event);

        while self.events.len() > self.max_events {
            self.events.remove(0);
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
        if self.show_command_palette {
            self.show_target_picker = false;
        }
    }

    pub fn open_target_picker(&mut self) {
        let targets = self.target_choices();
        if targets.is_empty() {
            self.composer.set_status("no fins available".to_string());
            return;
        }

        self.target_picker_index = targets
            .iter()
            .position(|fin| fin == &self.composer.target_fin)
            .unwrap_or(0);
        self.show_target_picker = true;
        self.show_command_palette = false;
    }

    pub fn target_picker_next(&mut self) {
        let len = self.target_choices().len();
        if len > 0 {
            self.target_picker_index = (self.target_picker_index + 1) % len;
        }
    }

    pub fn target_picker_prev(&mut self) {
        let len = self.target_choices().len();
        if len > 0 {
            self.target_picker_index = if self.target_picker_index == 0 {
                len - 1
            } else {
                self.target_picker_index - 1
            };
        }
    }

    pub fn select_target_picker(&mut self) {
        if let Some(target) = self.target_choices().get(self.target_picker_index).cloned() {
            self.composer.set_target(target);
        }
        self.show_target_picker = false;
    }

    pub fn send_operator_message(&mut self, body: &str) -> Result<(), String> {
        let to_fin = FinRef::new(&self.pod_slug, &self.composer.target_fin)?;
        self.orqa.ensure_fin_exists(&to_fin)?;
        let mail_home = self.orqa.mail_home(&to_fin)?;
        ensure_maildir(&mail_home)?;

        let from = format!("operator@{}.orqa", self.pod_slug);
        let to = format!("{}@{}.orqa", self.composer.target_fin, self.pod_slug);
        let message = format!("From: {from}\nTo: {to}\nSubject: Operator message\n\n{body}\n");
        deliver_mail(&mail_home, &message)?;

        self.events.push(Event::OperatorAction {
            text: format!("mailed {}: \"{}\"", self.composer.target_fin, body),
        });
        self.known_fins.insert(self.composer.target_fin.clone());
        self.follow = true;
        Ok(())
    }

    pub fn target_choices(&self) -> Vec<String> {
        let mut fins: Vec<String> = self
            .known_fins
            .iter()
            .filter(|fin| fin.as_str() != "operator")
            .cloned()
            .collect();
        if fins.is_empty() {
            fins = self.known_fins.iter().cloned().collect();
        }
        fins.sort();
        fins
    }

    pub fn toggle_pod_pause(&mut self) -> Result<(), String> {
        self.pod_paused = toggle_pod_pause(&self.orqa, &self.pod)?;
        if !self.pod_paused {
            self.next_loop_at = Instant::now() + TUI_LOOP_INTERVAL;
        }
        Ok(())
    }

    pub fn refresh_loop_countdown(&mut self) {
        if self.pod_paused {
            return;
        }

        let now = Instant::now();
        while self.next_loop_at <= now {
            self.next_loop_at += TUI_LOOP_INTERVAL;
        }
    }

    /// Render the cockpit as four main sections: header, content, input, footer.
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let gap = u16::from(self.expanded);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(gap), // expanded top spacing
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

        self.render_header(frame, self.section_area(chunks[1]));
        self.render_timeline(frame, self.section_area(chunks[3]));
        self.render_input_area(frame, self.section_area(chunks[5]));
        self.render_footer(frame, self.section_area(chunks[7]));

        if self.show_command_palette {
            self.render_command_palette(frame, area);
        }
        if self.show_target_picker {
            self.render_target_picker(frame, area);
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
        let base = Style::default().fg(self.theme.text);
        let accent = Style::default().fg(self.theme.accent);
        let dim = Style::default().fg(self.theme.muted);
        let icon = if self.any_fin_active() {
            self.spinner_frame()
        } else {
            "✓"
        };
        let icon_style = if self.any_fin_active() {
            accent
        } else {
            Style::default().fg(self.theme.ok)
        };
        let pod_path = display_path(&self.pod_root);
        let paused_width = if self.pod_paused { " paused".len() } else { 0 };
        let left_text_width =
            5 + self.pod_slug.chars().count() + paused_width + pod_path.chars().count();
        let right = self.header_right_text(area.width.saturating_sub(left_text_width as u16));
        let spacer_width = area
            .width
            .saturating_sub(left_text_width as u16)
            .saturating_sub(right.chars().count() as u16) as usize;

        let spans = vec![
            Span::styled(" ", base),
            Span::styled(icon, icon_style),
            Span::styled(" ", base),
            Span::styled(&self.pod_slug, accent),
            if self.pod_paused {
                Span::styled(" paused", Style::default().fg(self.theme.warn))
            } else {
                Span::styled("", base)
            },
            Span::styled("  ", base),
            Span::styled(pod_path, dim),
            Span::styled(" ".repeat(spacer_width), base),
            Span::styled(right, self.header_right_style()),
        ];

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

    fn header_right_text(&self, available_width: u16) -> String {
        if self.pod_paused {
            return "loop paused".to_string();
        }

        let countdown = self.next_loop_countdown().as_secs().max(1);
        let text = format!("next wake {countdown}s");
        if (text.chars().count() as u16) >= available_width {
            return format!("{countdown}s");
        }

        let mut text = text;
        let running = self.running_summary();
        if !running.is_empty() {
            let combined = format!("{text}  {running}");
            if (combined.chars().count() as u16) < available_width {
                text = combined;
            }
        }
        text
    }

    fn header_right_style(&self) -> Style {
        if self.pod_paused {
            Style::default().fg(self.theme.warn)
        } else {
            Style::default().fg(self.theme.accent)
        }
    }

    fn next_loop_countdown(&self) -> Duration {
        self.next_loop_at
            .checked_duration_since(Instant::now())
            .unwrap_or_default()
    }

    fn running_summary(&self) -> String {
        if self.active_since.is_empty() {
            return String::new();
        }

        let mut fins: Vec<_> = self.active_since.iter().collect();
        fins.sort_by_key(|(fin, _)| *fin);
        let summary = fins
            .into_iter()
            .take(3)
            .map(|(fin, since)| format!("{fin} {}", abbreviated_duration(since.elapsed())))
            .collect::<Vec<_>>()
            .join("  ");
        format!("running {summary}")
    }

    /// Three-row input section: one text row plus a border around all sides.
    fn render_input_area(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(self.theme.muted));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        self.render_input_label(frame, area);

        if self.mode == InputMode::Normal {
            let text = Line::from(vec![
                Span::styled(" >", Style::default().fg(self.theme.accent)),
                Span::styled(
                    " press i to write to the target fin",
                    Style::default().fg(self.theme.muted),
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
        let style = Style::default().fg(self.theme.muted);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(label, style))),
            label_area,
        );
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let dim = Style::default().fg(self.theme.muted);
        let help = "Shift+Tab:mode  |  Ctrl+T:target  |  Ctrl+.:commands";

        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(format!(" {help}"), dim))),
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

        let list = List::new(items).highlight_style(
            Style::default()
                .fg(self.theme.text)
                .add_modifier(Modifier::BOLD),
        );

        // Keep selection in bounds
        if let Some(selected) = self.list_state.selected() {
            if selected >= visible_count && visible_count > 0 {
                self.list_state.select(Some(visible_count - 1));
            }
        }

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
            frame.render_widget(Paragraph::new(empty), area);
        } else {
            frame.render_stateful_widget(list, area, &mut self.list_state);
        }
    }

    fn event_to_item(&self, ev: &Event) -> ListItem<'static> {
        ListItem::new(self.event_to_lines(ev))
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
                    Span::raw(" inbox ← "),
                    Span::styled(from_str, Style::default().fg(self.theme.warn)),
                    Span::raw("  "),
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
            .style(Style::default().fg(self.theme.text));
        let inner = block.inner(palette);
        let dim = Style::default().fg(self.theme.muted);
        let key = Style::default()
            .fg(self.theme.accent)
            .add_modifier(Modifier::BOLD);
        let text = Style::default().fg(self.theme.text);
        let rows = vec![
            Line::from(vec![
                Span::styled(" i", key),
                Span::styled(" compose message", text),
            ]),
            Line::from(vec![
                Span::styled(" Ctrl+T", key),
                Span::styled(" choose message target", text),
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
                Span::styled(" P", key),
                Span::styled(" pause/resume pod wake loop", text),
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

    fn render_target_picker(&self, frame: &mut Frame, area: Rect) {
        let targets = self.target_choices();
        let height = (targets.len() as u16 + 2).clamp(5, 14);
        let picker = centered_rect(area, 42, height);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Target Fin ")
            .style(Style::default().fg(self.theme.text));
        let inner = block.inner(picker);
        let selected = self
            .target_picker_index
            .min(targets.len().saturating_sub(1));
        let rows = if targets.is_empty() {
            vec![Line::from(Span::styled(
                " no fins available",
                Style::default().fg(self.theme.muted),
            ))]
        } else {
            targets
                .iter()
                .enumerate()
                .map(|(index, fin)| {
                    let style = if index == selected {
                        Style::default()
                            .fg(self.theme.accent)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(self.theme.text)
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

fn display_path(path: &std::path::Path) -> String {
    let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from) else {
        return path.display().to_string();
    };

    match path.strip_prefix(&home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

fn discover_known_fins(_orqa: &Orqa, pod: &PodRegistration) -> HashSet<String> {
    let fins_dir = pod.path.join(".orqa").join("fins");
    let Ok(entries) = fs::read_dir(fins_dir) else {
        return HashSet::new();
    };

    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect()
}

fn default_target_fin(fins: &HashSet<String>) -> String {
    if fins.contains("grok") {
        return "grok".to_string();
    }

    fins.iter()
        .filter(|fin| fin.as_str() != "operator")
        .min()
        .cloned()
        .or_else(|| fins.iter().min().cloned())
        .unwrap_or_else(|| "operator".to_string())
}
