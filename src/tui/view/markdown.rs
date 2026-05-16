use pulldown_cmark::{CodeBlockKind, CowStr, Event, HeadingLevel, Parser, Tag, TagEnd};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::tui::{theme::Theme, view::style::fg};

#[derive(Clone)]
struct Segment {
    text: String,
    style: Style,
}

#[derive(Clone, Copy)]
struct StyleState {
    base: Style,
    strong: bool,
    emphasis: bool,
    code: bool,
}

impl StyleState {
    fn new(base: Style) -> Self {
        Self {
            base,
            strong: false,
            emphasis: false,
            code: false,
        }
    }

    fn style(self, theme: &Theme) -> Style {
        let mut style = if self.code {
            fg(theme.cursor)
        } else {
            self.base
        };
        if self.strong {
            style = style.add_modifier(Modifier::BOLD);
        }
        if self.emphasis {
            style = style.add_modifier(Modifier::ITALIC);
        }
        style
    }
}

pub(super) fn render_markdown(
    markdown: &str,
    width: usize,
    base_style: Style,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let mut renderer = MarkdownRenderer::new(width.max(1), base_style, theme);
    renderer.render(markdown);
    renderer.finish()
}

struct MarkdownRenderer<'a> {
    width: usize,
    theme: &'a Theme,
    style: StyleState,
    lines: Vec<Vec<Segment>>,
    current: Vec<Segment>,
    in_code_block: bool,
    code_block_lines: Vec<String>,
    list_depth: usize,
}

impl<'a> MarkdownRenderer<'a> {
    fn new(width: usize, base_style: Style, theme: &'a Theme) -> Self {
        Self {
            width,
            theme,
            style: StyleState::new(base_style),
            lines: Vec::new(),
            current: Vec::new(),
            in_code_block: false,
            code_block_lines: Vec::new(),
            list_depth: 0,
        }
    }

    fn render(&mut self, markdown: &str) {
        for event in Parser::new(markdown) {
            self.handle_event(event);
        }
    }

    fn handle_event(&mut self, event: Event<'_>) {
        if self.in_code_block {
            match event {
                Event::End(TagEnd::CodeBlock) => self.end_code_block(),
                Event::Text(text) => self.code_block_lines.push(text.to_string()),
                Event::SoftBreak | Event::HardBreak => self.code_block_lines.push("\n".into()),
                _ => {}
            }
            return;
        }

        match event {
            Event::Start(tag) => self.handle_start(tag),
            Event::End(tag) => self.handle_end(tag),
            Event::Text(text) => self.push_text(&text),
            Event::Code(text) => self.push_inline_code(&text),
            Event::SoftBreak => self.push_text(" "),
            Event::HardBreak => self.flush_current(),
            Event::Rule => self.push_rule(),
            Event::Html(text) | Event::InlineHtml(text) => self.push_text(&text),
            Event::FootnoteReference(text) => self.push_text(&format!("[{text}]")),
            Event::TaskListMarker(checked) => self.push_text(if checked { "[x] " } else { "[ ] " }),
            _ => {}
        }
    }

    fn handle_start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Heading { level, .. } => self.start_heading(level),
            Tag::Paragraph => {}
            Tag::List(_) => self.list_depth += 1,
            Tag::Item => self.start_item(),
            Tag::Emphasis => self.style.emphasis = true,
            Tag::Strong => self.style.strong = true,
            Tag::CodeBlock(kind) => self.start_code_block(kind),
            Tag::BlockQuote(_) => self.push_text("> "),
            Tag::Link { .. } => self.style.strong = true,
            _ => {}
        }
    }

    fn handle_end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::Item => self.flush_current(),
            TagEnd::Heading(_) => {
                self.flush_current();
                self.style.strong = false;
            }
            TagEnd::List(_) => {
                self.flush_current();
                self.list_depth = self.list_depth.saturating_sub(1);
            }
            TagEnd::Emphasis => self.style.emphasis = false,
            TagEnd::Strong => self.style.strong = false,
            TagEnd::Link => self.style.strong = false,
            TagEnd::BlockQuote(_) => self.flush_current(),
            _ => {}
        }
    }

    fn start_heading(&mut self, level: HeadingLevel) {
        self.flush_current();
        self.style.strong = true;
        self.push_segment(
            format!("{} ", "#".repeat(heading_level_number(level))),
            fg(self.theme.accent).add_modifier(Modifier::BOLD),
        );
    }

    fn start_item(&mut self) {
        self.flush_current();
        let indent = "  ".repeat(self.list_depth.saturating_sub(1));
        self.push_segment(format!("{indent}- "), fg(self.theme.muted));
    }

    fn start_code_block(&mut self, _kind: CodeBlockKind<'_>) {
        self.flush_current();
        self.in_code_block = true;
        self.code_block_lines.clear();
    }

    fn end_code_block(&mut self) {
        let text = self.code_block_lines.join("");
        for line in text.lines() {
            self.lines.push(vec![Segment {
                text: format!("│ {line}"),
                style: fg(self.theme.cursor),
            }]);
        }
        self.in_code_block = false;
        self.code_block_lines.clear();
    }

    fn push_inline_code(&mut self, text: &CowStr<'_>) {
        self.push_segment(text.to_string(), fg(self.theme.cursor));
    }

    fn push_rule(&mut self) {
        self.flush_current();
        self.lines.push(vec![Segment {
            text: "─".repeat(self.width.min(80)),
            style: fg(self.theme.muted),
        }]);
    }

    fn push_text(&mut self, text: &str) {
        self.push_segment(text.to_string(), self.style.style(self.theme));
    }

    fn push_segment(&mut self, text: String, style: Style) {
        if text.is_empty() {
            return;
        }
        self.current.push(Segment { text, style });
    }

    fn flush_current(&mut self) {
        if self.current.is_empty() {
            return;
        }
        self.lines.push(std::mem::take(&mut self.current));
    }

    fn finish(mut self) -> Vec<Line<'static>> {
        self.flush_current();
        let mut out = Vec::new();
        for line in self.lines {
            out.extend(wrap_segments(&line, self.width));
        }
        if out.is_empty() {
            out.push(Line::from(""));
        }
        out
    }
}

fn heading_level_number(level: HeadingLevel) -> usize {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn wrap_segments(segments: &[Segment], width: usize) -> Vec<Line<'static>> {
    let mut lines: Vec<Vec<Segment>> = vec![Vec::new()];
    let mut current_width = 0usize;

    for segment in segments {
        for token in split_wrapping_tokens(&segment.text) {
            push_wrapped_token(&mut lines, &mut current_width, &token, segment.style, width);
        }
    }

    lines
        .into_iter()
        .map(trim_trailing_space_segments)
        .map(|line| {
            Line::from(
                line.into_iter()
                    .map(|segment| Span::styled(segment.text, segment.style))
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

fn trim_trailing_space_segments(mut line: Vec<Segment>) -> Vec<Segment> {
    while let Some(last) = line.last_mut() {
        let trimmed = last.text.trim_end().to_string();
        if trimmed.is_empty() {
            line.pop();
        } else {
            last.text = trimmed;
            break;
        }
    }
    line
}

fn push_token(lines: &mut [Vec<Segment>], token: String, style: Style) {
    if let Some(line) = lines.last_mut() {
        line.push(Segment { text: token, style });
    }
}

fn push_wrapped_token(
    lines: &mut Vec<Vec<Segment>>,
    current_width: &mut usize,
    token: &str,
    style: Style,
    width: usize,
) {
    let mut remaining = token;
    while !remaining.is_empty() {
        if *current_width == 0 && remaining.chars().all(char::is_whitespace) {
            break;
        }

        let available = width.saturating_sub(*current_width).max(1);
        let remaining_width = remaining.chars().count();
        if *current_width > 0 && remaining_width > available {
            lines.push(Vec::new());
            *current_width = 0;
            continue;
        }

        if remaining_width <= available {
            push_token(lines, remaining.to_string(), style);
            *current_width += remaining_width;
            break;
        }

        let split_at = remaining
            .char_indices()
            .nth(available)
            .map(|(index, _)| index)
            .unwrap_or(remaining.len());
        push_token(lines, remaining[..split_at].to_string(), style);
        lines.push(Vec::new());
        *current_width = 0;
        remaining = &remaining[split_at..];
    }
}

fn split_wrapping_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut current_whitespace = None;

    for ch in text.chars() {
        let whitespace = ch.is_whitespace();
        match current_whitespace {
            Some(kind) if kind == whitespace => current.push(ch),
            Some(_) => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                current.push(ch);
                current_whitespace = Some(whitespace);
            }
            None => {
                current.push(ch);
                current_whitespace = Some(whitespace);
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

#[cfg(test)]
#[path = "markdown_test.rs"]
mod tests;
