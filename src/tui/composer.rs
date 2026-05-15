//! Composer widget for the Operator Cockpit (Phase 4+).
//!
//! Handles the bottom input line, target fin selection, command history,
//! and the actual "send mail + wake" flow.

use std::collections::VecDeque;

use ratatui::layout::Rect;
use ratatui::{
    Frame,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::theme::Theme;

// use super::events::Event; // not needed yet in composer

/// State for the bottom composer.
#[derive(Default)]
pub struct Composer {
    /// The text the user is currently typing.
    pub input: String,
    /// Cursor position within `input`.
    pub cursor: usize,
    /// History of previously sent messages (most recent last).
    history: VecDeque<String>,
    /// Current position when browsing history with Up/Down (-1 = not browsing).
    history_index: isize,
    /// The fin we are currently addressing (e.g. "planner").
    pub target_fin: String,
    /// Transient status message shown after send (e.g. "sent ✓", "woke planner", error).
    status: Option<(String, std::time::Instant)>,
}

impl Composer {
    pub fn new(default_target: String) -> Self {
        Self {
            target_fin: default_target,
            history: VecDeque::with_capacity(30),
            history_index: -1,
            ..Default::default()
        }
    }

    /// Set a new target fin (called when user presses `f`).
    pub fn set_target(&mut self, fin: String) {
        self.target_fin = fin;
        self.clear_status();
    }

    /// Insert a character at the cursor.
    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor, c);
        self.cursor += 1;
        self.history_index = -1;
        self.clear_status();
    }

    /// Delete character before cursor (Backspace).
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.input.remove(self.cursor);
        }
        self.clear_status();
    }

    /// Delete character at cursor (Delete).
    pub fn delete(&mut self) {
        if self.cursor < self.input.len() {
            self.input.remove(self.cursor);
        }
        self.clear_status();
    }

    #[allow(dead_code)]
    /// Delete the previous word (Ctrl+W behavior).
    pub fn delete_previous_word(&mut self) {
        if self.cursor == 0 {
            return;
        }

        // Find the start of the previous word
        let mut pos = self.cursor;

        // Skip trailing whitespace
        while pos > 0
            && self
                .input
                .chars()
                .nth(pos - 1)
                .is_some_and(|c| c.is_whitespace())
        {
            pos -= 1;
        }

        // Skip the word
        while pos > 0
            && self
                .input
                .chars()
                .nth(pos - 1)
                .is_some_and(|c| !c.is_whitespace())
        {
            pos -= 1;
        }

        self.input.drain(pos..self.cursor);
        self.cursor = pos;
        self.clear_status();
    }

    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor < self.input.len() {
            self.cursor += 1;
        }
    }

    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor = self.input.len();
    }

    /// Browse command history (Up arrow).
    pub fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        if self.history_index == -1 {
            self.history_index = self.history.len() as isize - 1;
        } else if self.history_index > 0 {
            self.history_index -= 1;
        }
        if let Some(cmd) = self.history.get(self.history_index as usize) {
            self.input = cmd.clone();
            self.cursor = self.input.len();
        }
    }

    /// Browse command history (Down arrow).
    pub fn history_next(&mut self) {
        if self.history_index == -1 {
            return;
        }
        self.history_index += 1;
        if self.history_index as usize >= self.history.len() {
            self.history_index = -1;
            self.input.clear();
            self.cursor = 0;
        } else if let Some(cmd) = self.history.get(self.history_index as usize) {
            self.input = cmd.clone();
            self.cursor = self.input.len();
        }
    }

    /// Called when the user presses Enter with a non-empty message.
    /// Returns the message that was sent (so the caller can create an OperatorAction event).
    pub fn submit(&mut self) -> Option<String> {
        let msg = self.input.trim().to_string();
        if msg.is_empty() {
            return None;
        }

        // Save to history (avoid consecutive duplicates)
        if self.history.back() != Some(&msg) {
            self.history.push_back(msg.clone());
            if self.history.len() > 25 {
                self.history.pop_front();
            }
        }

        self.input.clear();
        self.cursor = 0;
        self.history_index = -1;
        self.set_status(format!("sent to {}", self.target_fin));

        Some(msg)
    }

    pub fn set_status(&mut self, text: String) {
        self.status = Some((text, std::time::Instant::now()));
    }

    fn clear_status(&mut self) {
        self.status = None;
    }

    /// Render the composer line. Spans include background so the row is solid when used
    /// over a pre-filled bg or standalone.
    pub fn render(&self, frame: &mut Frame, area: Rect, pod_slug: &str, theme: &Theme) {
        let prompt_style = Style::default().fg(theme.accent).bg(theme.bar_bg);
        let input_style = Style::default().fg(theme.text).bg(theme.bar_bg);
        let cursor_style = Style::default()
            .fg(theme.cursor)
            .bg(theme.bar_bg)
            .add_modifier(Modifier::SLOW_BLINK);
        let status_style = Style::default().fg(theme.ok).bg(theme.bar_bg);

        let prompt = format!("operator@{} → {} > ", pod_slug, self.target_fin);

        let status = if let Some((ref s, ts)) = self.status {
            // Show status for ~2.5 seconds
            if ts.elapsed().as_secs() < 3 {
                format!("  [{}]", s)
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let before_cursor = &self.input[..self.cursor];
        let after_cursor = &self.input[self.cursor..];

        let line = Line::from(vec![
            Span::styled(prompt, prompt_style),
            Span::styled(before_cursor, input_style),
            Span::styled("│", cursor_style),
            Span::styled(after_cursor, input_style),
            Span::styled(status, status_style),
        ]);

        let para = Paragraph::new(line);
        frame.render_widget(para, area);
    }
}
