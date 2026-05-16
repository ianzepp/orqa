use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::tui::app::App;

pub(super) fn render(app: &App, frame: &mut Frame, area: Rect) {
    let base = Style::default().fg(app.theme.text);
    let accent = Style::default().fg(app.theme.accent);
    let dim = Style::default().fg(app.theme.muted);
    let icon = if any_fin_active(app) {
        spinner_frame()
    } else {
        "✓"
    };
    let icon_style = if any_fin_active(app) {
        accent
    } else {
        Style::default().fg(app.theme.ok)
    };
    let pod_path = display_path(&app.pod_root);
    let paused_width = if app.pod_paused { " paused".len() } else { 0 };
    let left_text_width =
        5 + app.pod_slug.chars().count() + paused_width + pod_path.chars().count();
    let right = header_right_text(app, area.width.saturating_sub(left_text_width as u16));
    let spacer_width = area
        .width
        .saturating_sub(left_text_width as u16)
        .saturating_sub(right.chars().count() as u16) as usize;

    let spans = vec![
        Span::styled(" ", base),
        Span::styled(icon, icon_style),
        Span::styled(" ", base),
        Span::styled(&app.pod_slug, accent),
        if app.pod_paused {
            Span::styled(" paused", Style::default().fg(app.theme.warn))
        } else {
            Span::styled("", base)
        },
        Span::styled("  ", base),
        Span::styled(pod_path, dim),
        Span::styled(" ".repeat(spacer_width), base),
        Span::styled(right, header_right_style(app)),
    ];

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn any_fin_active(app: &App) -> bool {
    !app.active_since.is_empty()
}

fn spinner_frame() -> &'static str {
    const FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    FRAMES[((millis / 160) as usize) % FRAMES.len()]
}

fn header_right_text(app: &App, available_width: u16) -> String {
    if app.pod_paused {
        return "loop paused".to_string();
    }

    let countdown = next_loop_countdown(app).as_secs().max(1);
    let text = format!("next wake {countdown}s");
    if (text.chars().count() as u16) >= available_width {
        return format!("{countdown}s");
    }

    let mut text = text;
    let running = running_summary(app);
    if !running.is_empty() {
        let combined = format!("{text}  {running}");
        if (combined.chars().count() as u16) < available_width {
            text = combined;
        }
    }
    text
}

fn header_right_style(app: &App) -> Style {
    if app.pod_paused {
        Style::default().fg(app.theme.warn)
    } else {
        Style::default().fg(app.theme.accent)
    }
}

fn next_loop_countdown(app: &App) -> Duration {
    app.next_loop_at
        .checked_duration_since(Instant::now())
        .unwrap_or_default()
}

fn running_summary(app: &App) -> String {
    if app.active_since.is_empty() {
        return String::new();
    }

    let mut fins: Vec<_> = app.active_since.iter().collect();
    fins.sort_by_key(|(fin, _)| *fin);
    let summary = fins
        .into_iter()
        .take(3)
        .map(|(fin, since)| format!("{fin} {}", abbreviated_duration(since.elapsed())))
        .collect::<Vec<_>>()
        .join("  ");
    format!("running {summary}")
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
