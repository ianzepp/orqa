//! Main application state for the Operator Cockpit.
//!
//! This module owns the live event buffer, filters, scroll state, and
//! integration with the `PodWatcher`.

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

use ratatui::widgets::ListState;

use crate::{
    mailbox::{deliver_mail, ensure_maildir, message_id, resolve_address, sorted_files},
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Surface {
    Timeline,
    Mail,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MailMode {
    Index,
    Pager,
    Compose,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MailComposeField {
    To,
    Subject,
    Body,
}

#[derive(Clone, Debug)]
pub struct MailComposeState {
    pub to: String,
    pub subject: String,
    pub body: String,
    pub field: MailComposeField,
    pub reply_to: Option<String>,
}

#[derive(Clone, Debug)]
pub struct OperatorMail {
    pub id: String,
    pub path: PathBuf,
    pub state: String,
    pub from: String,
    pub to: String,
    pub subject: String,
    pub body: String,
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
    pub surface: Surface,
    pub mail_mode: MailMode,
    pub operator_mail: Vec<OperatorMail>,
    pub mail_cursor: usize,
    pub mail_compose: Option<MailComposeState>,
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
            surface: Surface::Timeline,
            mail_mode: MailMode::Index,
            operator_mail: Vec::new(),
            mail_cursor: 0,
            mail_compose: None,
        };
        app.refresh_operator_mail();
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
        self.refresh_operator_mail();
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

    pub fn toggle_surface(&mut self) {
        match self.surface {
            Surface::Timeline => self.show_mail_surface(),
            Surface::Mail => self.show_timeline_surface(),
        }
    }

    pub fn show_mail_surface(&mut self) {
        self.refresh_operator_mail();
        self.surface = Surface::Mail;
        self.mail_mode = MailMode::Index;
        self.mail_compose = None;
    }

    pub fn show_timeline_surface(&mut self) {
        self.surface = Surface::Timeline;
        self.mode = InputMode::Normal;
        self.mail_mode = MailMode::Index;
        self.mail_compose = None;
        self.follow = true;
        self.scroll_to_bottom();
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

    pub fn refresh_operator_mail(&mut self) {
        let Ok(messages) = load_operator_mail(&self.orqa, &self.pod_slug) else {
            return;
        };
        self.operator_mail = messages;
        if self.mail_cursor >= self.operator_mail.len() {
            self.mail_cursor = self.operator_mail.len().saturating_sub(1);
        }
    }

    pub fn mail_cursor_down(&mut self) {
        if !self.operator_mail.is_empty() {
            self.mail_cursor = (self.mail_cursor + 1).min(self.operator_mail.len() - 1);
        }
    }

    pub fn mail_cursor_up(&mut self) {
        self.mail_cursor = self.mail_cursor.saturating_sub(1);
    }

    pub fn mail_cursor_top(&mut self) {
        self.mail_cursor = 0;
    }

    pub fn mail_cursor_bottom(&mut self) {
        self.mail_cursor = self.operator_mail.len().saturating_sub(1);
    }

    pub fn open_selected_mail(&mut self) {
        if self.operator_mail.is_empty() {
            return;
        }
        self.mark_selected_mail_read();
        self.mail_mode = MailMode::Pager;
    }

    pub fn start_mail_compose(&mut self) {
        self.mail_compose = Some(MailComposeState {
            to: self.composer.target_fin.clone(),
            subject: String::new(),
            body: String::new(),
            field: MailComposeField::To,
            reply_to: None,
        });
        self.mail_mode = MailMode::Compose;
    }

    pub fn start_mail_reply(&mut self) {
        let Some(message) = self.selected_mail() else {
            return;
        };
        self.mail_compose = Some(MailComposeState {
            to: message.from.clone(),
            subject: reply_subject(&message.subject),
            body: String::new(),
            field: MailComposeField::Body,
            reply_to: Some(message.id.clone()),
        });
        self.mail_mode = MailMode::Compose;
    }

    pub fn abort_mail_compose(&mut self) {
        self.mail_compose = None;
        self.mail_mode = MailMode::Index;
    }

    pub fn advance_mail_compose_field(&mut self) {
        let Some(compose) = self.mail_compose.as_mut() else {
            return;
        };
        compose.field = match compose.field {
            MailComposeField::To => MailComposeField::Subject,
            MailComposeField::Subject | MailComposeField::Body => MailComposeField::Body,
        };
    }

    pub fn previous_mail_compose_field(&mut self) {
        let Some(compose) = self.mail_compose.as_mut() else {
            return;
        };
        compose.field = match compose.field {
            MailComposeField::To => MailComposeField::To,
            MailComposeField::Subject => MailComposeField::To,
            MailComposeField::Body => MailComposeField::Subject,
        };
    }

    pub fn mail_compose_enter(&mut self) {
        let Some(compose) = self.mail_compose.as_mut() else {
            return;
        };
        match compose.field {
            MailComposeField::To => compose.field = MailComposeField::Subject,
            MailComposeField::Subject => compose.field = MailComposeField::Body,
            MailComposeField::Body => compose.body.push('\n'),
        }
    }

    pub fn mail_compose_push(&mut self, ch: char) {
        let Some(compose) = self.mail_compose.as_mut() else {
            return;
        };
        match compose.field {
            MailComposeField::To => compose.to.push(ch),
            MailComposeField::Subject => compose.subject.push(ch),
            MailComposeField::Body => compose.body.push(ch),
        }
    }

    pub fn mail_compose_backspace(&mut self) {
        let Some(compose) = self.mail_compose.as_mut() else {
            return;
        };
        match compose.field {
            MailComposeField::To => {
                compose.to.pop();
            }
            MailComposeField::Subject => {
                compose.subject.pop();
            }
            MailComposeField::Body => {
                compose.body.pop();
            }
        }
    }

    pub fn send_mail_compose(&mut self) -> Result<(), String> {
        let Some(compose) = self.mail_compose.take() else {
            return Ok(());
        };
        if compose.to.trim().is_empty() {
            self.mail_compose = Some(compose);
            return Err("recipient is required".to_string());
        }

        let to = resolve_address(compose.to.trim(), Some(&self.pod_slug))?;
        let to_fin = FinRef::new(&to.pod, &to.fin)?;
        self.orqa.ensure_fin_exists(&to_fin)?;
        let mail_home = self.orqa.mail_home(&to_fin)?;
        ensure_maildir(&mail_home)?;

        let from = format!("operator@{}.orqa", self.pod_slug);
        let subject = if compose.subject.trim().is_empty() {
            "(no subject)"
        } else {
            compose.subject.trim()
        };
        let message = format!(
            "From: {from}\nTo: {}\nSubject: {}\n\n{}\n",
            to.label(),
            subject,
            compose.body
        );
        deliver_mail(&mail_home, &message)?;
        if !self.pod_paused {
            if let Err(error) = trigger_tui_wake(&self.orqa, &self.pod) {
                self.record_operator_action(format!("mail sent, but wake trigger failed: {error}"));
            }
            self.next_loop_at = Instant::now() + TUI_LOOP_INTERVAL;
        }

        let action = if compose.reply_to.is_some() {
            format!("replied to {}", to.label())
        } else {
            format!("mailed {}", to.label())
        };
        self.record_operator_action(action);
        self.mail_mode = MailMode::Index;
        self.refresh_operator_mail();
        Ok(())
    }

    pub fn selected_mail(&self) -> Option<&OperatorMail> {
        self.operator_mail.get(self.mail_cursor)
    }

    fn mark_selected_mail_read(&mut self) {
        let Some(message) = self.operator_mail.get_mut(self.mail_cursor) else {
            return;
        };
        if message.state != "new" {
            return;
        }
        let Some(id) = message.path.file_name().map(|name| name.to_os_string()) else {
            return;
        };
        let Some(mail_home) = message.path.parent().and_then(Path::parent) else {
            return;
        };
        let done_path = mail_home.join("cur").join(id);
        if fs::rename(&message.path, &done_path).is_ok() {
            message.path = done_path;
            message.state = "cur".to_string();
        }
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

fn load_operator_mail(orqa: &Orqa, pod_slug: &str) -> Result<Vec<OperatorMail>, String> {
    let fin = FinRef::new(pod_slug, "operator")?;
    let mail_home = orqa.mail_home(&fin)?;
    ensure_maildir(&mail_home)?;

    let mut messages = Vec::new();
    for state in ["new", "cur"] {
        for path in sorted_files(&mail_home.join(state))? {
            messages.push(load_operator_message(path, state)?);
        }
    }
    messages.sort_by(|left, right| right.id.cmp(&left.id));
    Ok(messages)
}

fn load_operator_message(path: PathBuf, state: &str) -> Result<OperatorMail, String> {
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read mail {}: {error}", path.display()))?;
    let (headers, body) = split_mail_message(&raw);
    Ok(OperatorMail {
        id: message_id(&path)?,
        path,
        state: state.to_string(),
        from: header_value(&headers, "From").unwrap_or_else(|| "?".to_string()),
        to: header_value(&headers, "To").unwrap_or_else(|| "?".to_string()),
        subject: header_value(&headers, "Subject").unwrap_or_else(|| "(no subject)".to_string()),
        body,
    })
}

fn split_mail_message(raw: &str) -> (Vec<(String, String)>, String) {
    let (header_raw, body) = raw.split_once("\n\n").unwrap_or((raw, ""));
    let headers = header_raw
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            Some((key.trim().to_string(), value.trim().to_string()))
        })
        .collect();
    (headers, body.trim_end().to_string())
}

fn header_value(headers: &[(String, String)], key: &str) -> Option<String> {
    headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(key))
        .map(|(_, value)| value.clone())
}

fn reply_subject(subject: &str) -> String {
    if subject.to_ascii_lowercase().starts_with("re:") {
        subject.to_string()
    } else {
        format!("Re: {subject}")
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
