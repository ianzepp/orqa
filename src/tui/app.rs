//! Main application state for the Operator Cockpit.
//!
//! This module owns the live event buffer, filters, scroll state, and
//! integration with the `PodWatcher`.

use std::{
    collections::{HashMap, HashSet},
    fs,
    time::Instant,
};

use ratatui::widgets::ListState;

use crate::{
    mailbox::{deliver_mail, ensure_maildir},
    model::{FinRef, Orqa, PodRegistration},
};

use super::composer::Composer;
use super::events::Event;
use super::loopctl::{TUI_LOOP_INTERVAL, pod_paused, toggle_pod_pause, trigger_tui_wake};
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

        if let Event::LogLine { fin, stream, line } = &event {
            if let Some(Event::LogLine {
                fin: previous_fin,
                stream: previous_stream,
                line: previous_line,
            }) = self.events.last_mut()
            {
                if previous_fin == fin && previous_stream == stream {
                    if !previous_line.is_empty() && !previous_line.ends_with('\n') {
                        previous_line.push('\n');
                    }
                    previous_line.push_str(line);
                    return;
                }
            }
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

        self.record_operator_action(format!("mailed {}: \"{}\"", self.composer.target_fin, body));
        self.known_fins.insert(self.composer.target_fin.clone());
        if !self.pod_paused {
            if let Err(error) = trigger_tui_wake(&self.orqa, &self.pod) {
                self.record_operator_action(format!("mail sent, but wake trigger failed: {error}"));
            }
            self.next_loop_at = Instant::now() + TUI_LOOP_INTERVAL;
        }
        self.follow = true;
        Ok(())
    }

    pub fn record_operator_action(&mut self, text: impl Into<String>) {
        self.events
            .push(Event::OperatorAction { text: text.into() });
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
