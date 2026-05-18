use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
};
use serde_json::Value;

use crate::tui::{
    app::App,
    events::{Event, LogStream},
    view::{
        markdown::render_markdown,
        style::{bold, fg, strong},
    },
};

pub(super) fn render(app: &mut App, frame: &mut Frame, area: Rect) {
    let visible_events = app.visible_events();
    let items: Vec<ListItem> = visible_events
        .into_iter()
        .flat_map(|event| event_to_items(app, event, area.width))
        .collect();
    let visible_count = items.len();
    app.timeline_rows = visible_count;

    let list = List::new(items).highlight_style(bold(app.theme.text));

    if app.follow && visible_count > 0 {
        app.list_state.select(Some(visible_count - 1));
    } else if let Some(selected) = app.list_state.selected() {
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

fn event_to_items(app: &App, event: &Event, width: u16) -> Vec<ListItem<'static>> {
    event_to_lines(app, event, width)
        .into_iter()
        .map(ListItem::new)
        .collect()
}

fn event_to_lines(app: &App, event: &Event, width: u16) -> Vec<Line<'static>> {
    match event {
        Event::LogLine { fin, stream, line } => {
            let color = match stream {
                LogStream::Stdout => app.theme.stdout,
                LogStream::Stderr => app.theme.error,
                LogStream::Event => app.theme.event,
            };
            let prefix = vec![fin_tag(fin, fg(app.theme.accent)), Span::raw(" ")];
            let prefix_width = fin_tag_width(fin) + 1;
            if *stream == LogStream::Stdout {
                // Preferred path: use structured segments so we can give thoughts
                // a distinct muted/italic treatment with their own prefix.
                if let Some(segments) = grok_streaming_to_segments(line) {
                    if segments.is_empty() {
                        return Vec::new();
                    }
                    let mut out_lines = Vec::new();
                    for seg in segments {
                        match seg {
                            GrokSegment::Thought(text) => {
                                out_lines.extend(render_grok_thought(fin, &text, width, app));
                            }
                            GrokSegment::Response(text) => {
                                if text.trim().is_empty() {
                                    continue;
                                }
                                let content_width =
                                    usize::from(width).saturating_sub(prefix_width).max(1);
                                let md =
                                    render_markdown(&text, content_width, fg(color), &app.theme);
                                out_lines.extend(prefixed_lines(prefix.clone(), prefix_width, md));
                            }
                        }
                    }
                    out_lines
                } else {
                    // Try to summarize Codex-style tool call output.
                    if let Some(summary) = codex_tool_output_to_summary(line) {
                        let status_color = if summary.failed {
                            fg(app.theme.error)
                        } else {
                            fg(app.theme.ok)
                        };
                        prefixed_wrapped_lines(
                            prefix,
                            prefix_width,
                            &summary.text,
                            status_color,
                            width,
                        )
                    } else {
                        // Fallback for non-Grok, non-Codex stdout: old behavior.
                        let content_width =
                            usize::from(width).saturating_sub(prefix_width).max(1);
                        let rendered_line =
                            grok_streaming_json_to_markdown(line)
                                .unwrap_or_else(|| line.to_string());
                        if rendered_line.trim().is_empty() {
                            return Vec::new();
                        }
                        prefixed_lines(
                            prefix,
                            prefix_width,
                            render_markdown(&rendered_line, content_width, fg(color), &app.theme),
                        )
                    }
                }
            } else if *stream == LogStream::Event {
                let rendered_line = match backend_event_json_to_summary(line) {
                    Some(rendered) if rendered.trim().is_empty() => return Vec::new(),
                    Some(rendered) => rendered,
                    None => line.to_string(),
                };
                prefixed_wrapped_lines(prefix, prefix_width, &rendered_line, fg(color), width)
            } else {
                prefixed_wrapped_lines(prefix, prefix_width, line, fg(color), width)
            }
        }
        Event::MailArrived {
            fin, from, subject, ..
        } => {
            let subject = subject.clone().unwrap_or_else(|| "(no subject)".into());
            let from = from.clone().unwrap_or_else(|| "?".into());
            let prefix_width =
                fin_tag_width(fin) + " inbox ← ".chars().count() + from.chars().count() + 2;
            prefixed_wrapped_lines(
                vec![
                    fin_tag(fin, fg(app.theme.mail)),
                    Span::raw(" inbox ← "),
                    Span::styled(from, fg(app.theme.warn)),
                    Span::raw("  "),
                ],
                prefix_width,
                &subject,
                strong(),
                width,
            )
        }
        Event::RunStarted { fin, run_id } => prefixed_wrapped_lines(
            vec![fin_tag(fin, fg(app.theme.ok)), Span::raw(" run started ")],
            fin_tag_width(fin) + " run started ".chars().count(),
            run_id,
            fg(app.theme.text),
            width,
        ),
        Event::RunFinished {
            fin,
            run_id,
            exit_code,
        } => {
            let status = exit_code.map_or("?".to_string(), |code| code.to_string());
            prefixed_wrapped_lines(
                vec![fin_tag(fin, fg(app.theme.ok)), Span::raw(" run finished ")],
                fin_tag_width(fin) + " run finished ".chars().count(),
                &format!("{run_id} (exit {status})"),
                fg(app.theme.text),
                width,
            )
        }
        Event::LockAcquired { fin } => vec![Line::from(vec![
            fin_tag(fin, fg(app.theme.warn)),
            Span::raw(" acquired lock"),
        ])],
        Event::LockReleased { fin } => vec![Line::from(vec![
            fin_tag(fin, fg(app.theme.warn)),
            Span::raw(" released lock"),
        ])],
        Event::OperatorAction { text } => {
            let mut lines = vec![Line::from(Span::styled(" operator", fg(app.theme.muted)))];
            lines.extend(wrapped_plain_lines(text, 2, fg(app.theme.text), width));
            lines.push(Line::from(Span::raw("")));
            lines
        }
    }
}

fn fin_tag(fin: &str, style: ratatui::style::Style) -> Span<'static> {
    Span::styled(format!("[{}]", fin), style)
}

fn fin_tag_width(fin: &str) -> usize {
    fin.chars().count() + 2
}

/// Render a thought block with a distinct muted+italic prefix.
///
/// First line gets `* {fin} thought: ` (per the current simple convention).
/// Continuation lines are indented only (no repeated marker) so wrapped
/// thoughts don't look like IRC spam.
fn render_grok_thought(fin: &str, text: &str, width: u16, app: &App) -> Vec<Line<'static>> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return vec![];
    }

    let style = fg(app.theme.muted).add_modifier(Modifier::ITALIC);

    let header = format!("* {} thought: ", fin);
    let header_width = header.chars().count();

    // Leave some room; fall back to a reasonable minimum.
    let content_width = usize::from(width).saturating_sub(header_width).max(20);

    let chunks = wrap_text(trimmed, content_width);
    if chunks.is_empty() {
        return vec![];
    }

    let mut lines = vec![Line::from(vec![
        Span::styled(header, style),
        Span::styled(chunks[0].clone(), style),
    ])];

    for chunk in chunks.into_iter().skip(1) {
        let indent = " ".repeat(header_width);
        lines.push(Line::from(vec![
            Span::styled(indent, style),
            Span::styled(chunk, style),
        ]));
    }
    lines
}

fn prefixed_wrapped_lines(
    prefix: Vec<Span<'static>>,
    prefix_width: usize,
    text: &str,
    text_style: ratatui::style::Style,
    width: u16,
) -> Vec<Line<'static>> {
    let text_width = usize::from(width).saturating_sub(prefix_width).max(1);
    let chunks = wrap_text(text, text_width);
    let mut lines = Vec::new();

    for (index, chunk) in chunks.into_iter().enumerate() {
        if index == 0 {
            let mut spans = prefix.clone();
            spans.push(Span::styled(chunk, text_style));
            lines.push(Line::from(spans));
        } else {
            lines.push(Line::from(vec![
                Span::raw(" ".repeat(prefix_width)),
                Span::styled(chunk, text_style),
            ]));
        }
    }

    lines
}

/// Structured output from a Grok streaming-json run.
///
/// This is the foundation for differentiated rendering of internal reasoning
/// ("thoughts") vs final user-facing responses in the timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum GrokSegment {
    /// Internal chain-of-thought / reasoning from the model.
    Thought(String),
    /// Final response text intended for the human.
    Response(String),
}

pub(super) fn grok_streaming_to_segments(raw: &str) -> Option<Vec<GrokSegment>> {
    let mut segments: Vec<GrokSegment> = Vec::new();
    let mut current_thought = String::new();
    let mut current_response = String::new();
    let mut saw_stream_event = false;

    for raw_line in raw.lines() {
        let raw_line = raw_line.trim();
        if raw_line.is_empty() {
            continue;
        }

        let Ok(value) = serde_json::from_str::<Value>(raw_line) else {
            return None;
        };
        let kind = value.get("type").and_then(Value::as_str)?;
        saw_stream_event = true;

        match kind {
            "text" => {
                // Flush any pending thought before starting/continuing a response.
                if !current_thought.is_empty() {
                    let normalized = normalize_streamed_text(&current_thought);
                    if !normalized.is_empty() {
                        segments.push(GrokSegment::Thought(normalized));
                    }
                    current_thought.clear();
                }
                if let Some(data) = value.get("data").and_then(Value::as_str) {
                    current_response.push_str(data);
                }
            }
            "thought" => {
                // If we were accumulating a response, flush it first so ordering is preserved.
                if !current_response.is_empty() {
                    segments.push(GrokSegment::Response(std::mem::take(&mut current_response)));
                }
                if let Some(data) = streaming_event_detail(&value) {
                    current_thought.push_str(&data);
                }
            }
            "end" => {
                if !current_thought.is_empty() {
                    let normalized = normalize_streamed_text(&current_thought);
                    if !normalized.is_empty() {
                        segments.push(GrokSegment::Thought(normalized));
                    }
                    current_thought.clear();
                }
                if !current_response.is_empty() {
                    segments.push(GrokSegment::Response(std::mem::take(&mut current_response)));
                }
            }
            other => {
                // Flush anything pending, then record the unknown event as response content.
                if !current_thought.is_empty() {
                    let normalized = normalize_streamed_text(&current_thought);
                    if !normalized.is_empty() {
                        segments.push(GrokSegment::Thought(normalized));
                    }
                    current_thought.clear();
                }
                if !current_response.is_empty() {
                    segments.push(GrokSegment::Response(std::mem::take(&mut current_response)));
                }

                let mut s = format!("`{}`", other);
                if let Some(data) = streaming_event_detail(&value) {
                    s.push(' ');
                    s.push_str(&data);
                }
                s.push('\n');
                segments.push(GrokSegment::Response(s));
            }
        }
    }

    // Final flush of whatever is left
    if !current_thought.is_empty() {
        let normalized = normalize_streamed_text(&current_thought);
        if !normalized.is_empty() {
            segments.push(GrokSegment::Thought(normalized));
        }
    }
    if !current_response.is_empty() {
        segments.push(GrokSegment::Response(current_response));
    }

    saw_stream_event.then_some(segments)
}

/// Legacy string renderer kept for the chat history surface and existing tests.
/// It reconstructs the previous `> thinking: ...` + response format from segments.
pub(super) fn grok_streaming_json_to_markdown(raw: &str) -> Option<String> {
    let segments = grok_streaming_to_segments(raw)?;
    let mut rendered = String::new();

    for seg in segments {
        match seg {
            GrokSegment::Thought(t) => {
                if !rendered.is_empty() && !rendered.ends_with("\n\n") {
                    rendered.push_str("\n\n");
                }
                rendered.push_str("> thinking: ");
                rendered.push_str(&t);
                rendered.push_str("\n\n");
            }
            GrokSegment::Response(t) => {
                rendered.push_str(&t);
            }
        }
    }

    Some(rendered)
}

pub(super) fn backend_event_json_to_summary(raw: &str) -> Option<String> {
    let mut rendered = Vec::new();
    let mut saw_backend_event = false;

    for raw_line in raw.lines() {
        let raw_line = raw_line.trim();
        if raw_line.is_empty() {
            continue;
        }

        let Ok(value) = serde_json::from_str::<Value>(raw_line) else {
            return None;
        };
        let event = value.get("event").and_then(Value::as_str)?;
        saw_backend_event = true;

        match event {
            "planned" => {
                if let Some(command) = value.get("command").and_then(Value::as_str) {
                    rendered.push(format!("planned: {command}"));
                } else {
                    rendered.push("planned run".to_string());
                }
            }
            "spawned" => {
                if let Some(pid) = value.get("pid").and_then(Value::as_str) {
                    rendered.push(format!("spawned pid {pid}"));
                } else {
                    rendered.push("spawned".to_string());
                }
            }
            "finished" => {}
            other => {
                let detail = backend_event_detail(&value);
                if detail.is_empty() {
                    rendered.push(other.to_string());
                } else {
                    rendered.push(format!("{other}: {detail}"));
                }
            }
        }
    }

    saw_backend_event.then_some(rendered.join("\n"))
}

fn normalize_streamed_text(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let tightened_quotes = tighten_quote_spacing(&collapsed);
    remove_space_before_punctuation(&tightened_quotes)
}

fn tighten_quote_spacing(text: &str) -> String {
    let mut out = String::new();
    let mut in_quote = false;
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '"' {
            if in_quote {
                while out.ends_with(' ') {
                    out.pop();
                }
                out.push(ch);
                in_quote = false;
            } else {
                out.push(ch);
                in_quote = true;
                while chars.peek().is_some_and(|next| next.is_whitespace()) {
                    chars.next();
                }
            }
        } else {
            out.push(ch);
        }
    }

    out
}

fn remove_space_before_punctuation(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        if matches!(ch, '.' | ',' | '!' | '?' | ';' | ':' | ')' | ']' | '}') {
            while out.ends_with(' ') {
                out.pop();
            }
        }
        out.push(ch);
    }
    out
}

fn streaming_event_detail(value: &Value) -> Option<String> {
    let data = value.get("data")?;
    if let Some(text) = data.as_str() {
        return Some(text.to_string());
    }

    for key in ["name", "tool", "command", "description"] {
        if let Some(text) = data.get(key).and_then(Value::as_str) {
            return Some(text.to_string());
        }
    }

    if data.is_null() {
        None
    } else {
        serde_json::to_string(data).ok()
    }
}

fn backend_event_detail(value: &Value) -> String {
    for key in ["command", "pid", "exit_code", "run"] {
        if let Some(text) = value.get(key).and_then(Value::as_str) {
            return text.to_string();
        }
        if let Some(number) = value.get(key).and_then(Value::as_i64) {
            return number.to_string();
        }
    }

    String::new()
}

fn prefixed_lines(
    prefix: Vec<Span<'static>>,
    prefix_width: usize,
    content_lines: Vec<Line<'static>>,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (index, line) in content_lines.into_iter().enumerate() {
        if index == 0 {
            let mut spans = prefix.clone();
            spans.extend(line.spans);
            lines.push(Line::from(spans));
        } else {
            let mut spans = vec![Span::raw(" ".repeat(prefix_width))];
            spans.extend(line.spans);
            lines.push(Line::from(spans));
        }
    }
    lines
}

fn wrapped_plain_lines(
    text: &str,
    indent: usize,
    style: ratatui::style::Style,
    width: u16,
) -> Vec<Line<'static>> {
    let text_width = usize::from(width).saturating_sub(indent).max(1);
    wrap_text(text, text_width)
        .into_iter()
        .map(|chunk| {
            Line::from(vec![
                Span::raw(" ".repeat(indent)),
                Span::styled(chunk, style),
            ])
        })
        .collect()
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    for raw_line in text.lines() {
        if raw_line.is_empty() {
            lines.push(String::new());
            continue;
        }

        lines.extend(wrap_text_line(raw_line, width));
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

fn wrap_text_line(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        let current_width = current.chars().count();
        let word_width = word.chars().count();
        if current_width == 0 {
            push_wrapped_word(&mut lines, &mut current, word, width);
        } else if current_width + 1 + word_width <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            push_wrapped_word(&mut lines, &mut current, word, width);
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}

fn push_wrapped_word(lines: &mut Vec<String>, current: &mut String, word: &str, width: usize) {
    let mut remaining = word;
    while remaining.chars().count() > width {
        let split_at = remaining
            .char_indices()
            .nth(width)
            .map(|(index, _)| index)
            .unwrap_or(remaining.len());
        lines.push(remaining[..split_at].to_string());
        remaining = &remaining[split_at..];
    }
    current.push_str(remaining);
}

/// Summarize a block of Codex CLI verbose tool-call output into a single line.
///
/// Codex prints each tool call like:
///
/// ```text
/// exec
///       /bin/zsh -lc "orqa mail list ..." in /some/path
///       succeeded in 0ms:
///       (output lines)
///
/// exec
///       /bin/zsh -lc "orqa fin status ..." in /some/path
///       failed in 1ms:
///       (error output)
/// ```
///
/// This collapses each tool call into one line:
///   - Success: `[fin] ✓ exec: orqa mail list ...`
///   - Failure: `[fin] ✗ exec: orqa fin status ... (failed)`
///
/// Returns `None` if the input doesn't look like Codex tool output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CodexToolSummary {
    pub(super) text: String,
    pub(super) failed: bool,
}

pub(super) fn codex_tool_output_to_summary(raw: &str) -> Option<CodexToolSummary> {
    let lines: Vec<&str> = raw.lines().collect();
    if lines.is_empty() {
        return None;
    }

    // Quick heuristic: if the first non-empty line is a known tool name
    // and we see "succeeded in" or "failed in" later, it's Codex output.
    let first_nonempty = lines.iter().find(|l| !l.trim().is_empty())?;
    let first_trimmed = first_nonempty.trim();

    // Known Codex tool names (extensible).
    let known_tools = ["exec", "read", "write", "edit", "bash", "grep", "find", "ls"];
    if !known_tools.contains(&first_trimmed) {
        return None;
    }

    // Parse tool calls from the block.
    let mut tool_calls = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        // Look for a tool name line (not indented, matches known tool).
        if !trimmed.is_empty()
            && line.chars().next().map(|c| !c.is_whitespace()).unwrap_or(true)
            && known_tools.contains(&trimmed)
        {
            let tool_name = trimmed.to_string();
            i += 1;

            // Next non-empty line should be the command with "in <path>".
            let mut command = String::new();
            let mut succeeded = true;
            let mut output_lines = Vec::new();

            while i < lines.len() {
                let l = lines[i];
                let lt = l.trim();

                if lt.is_empty() {
                    i += 1;
                    continue;
                }

                // Check for status line: "succeeded in Nms:" or "failed in Nms:"
                if lt.starts_with("succeeded in") || lt.starts_with("failed in") {
                    succeeded = !lt.starts_with("failed");
                    i += 1;
                    // Remaining lines are the tool's output.
                    // Collect up to the next blank line or next tool call.
                    while i < lines.len() {
                        let ol = lines[i];
                        let ot = ol.trim();
                        // Stop if we hit another tool call (unindented known tool name).
                        if !ol.chars().next().map(|c| c.is_whitespace()).unwrap_or(false)
                            && !ot.is_empty()
                            && known_tools.contains(&ot)
                        {
                            break;
                        }
                        if ot.is_empty() {
                            break;
                        }
                        output_lines.push(ot.to_string());
                        i += 1;
                    }
                    break;
                }

                // Accumulate the command line (may span multiple indented lines).
                if !command.is_empty() {
                    command.push(' ');
                }
                command.push_str(lt);
                i += 1;
            }

            // Truncate command to a reasonable length for display.
            let display_cmd = if command.len() > 80 {
                format!("{}...", &command[..80])
            } else {
                command
            };

            tool_calls.push(CodexToolCall {
                tool_name,
                command: display_cmd,
                succeeded,
                output: output_lines,
            });
        } else {
            i += 1;
        }
    }

    if tool_calls.is_empty() {
        return None;
    }

    // Build summary.
    let mut text = String::new();
    let mut any_failed = false;

    for (idx, tc) in tool_calls.iter().enumerate() {
        if idx > 0 {
            text.push(' ');
        }
        if tc.succeeded {
            text.push_str(&format!("✓ {}", tc.tool_name));
        } else {
            any_failed = true;
            text.push_str(&format!("✗ {}", tc.tool_name));
        }
        text.push_str(&format!(": {}", tc.command));
        if !tc.succeeded {
            text.push_str(" (failed)");
        }
    }

    Some(CodexToolSummary {
        text,
        failed: any_failed,
    })
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct CodexToolCall {
    tool_name: String,
    command: String,
    succeeded: bool,
    output: Vec<String>,
}

#[cfg(test)]
#[path = "timeline_test.rs"]
mod tests;
