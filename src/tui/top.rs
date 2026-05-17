//! Ratatui global top view for all registered pods.

use std::{
    ffi::OsString,
    fs,
    io::{self, stdout},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crossterm::{
    event::{self, Event as CrosstermEvent, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table},
};

use crate::{
    global_loop::{DEFAULT_GLOBAL_LOOP_INTERVAL, DEFAULT_GLOBAL_LOOP_PROMPT, wake_all_pods},
    mailbox::{remove_sleep_marker, write_sleep_marker},
    model::{FinRef, Orqa, PodRef, load_registry},
    runtime::wake_pod_quiet,
    status::pod_status,
    tui::theme::{Theme, default_theme},
};

const TOP_LOOP_INTERVAL: Duration = Duration::from_secs(DEFAULT_GLOBAL_LOOP_INTERVAL);
const TOP_INITIAL_LOOP_DELAY: Duration = Duration::from_secs(10);

#[derive(Clone, Debug)]
pub(super) struct TopFin {
    pub(super) pod: String,
    pub(super) fin: String,
    pub(super) running: bool,
    pub(super) sleeping: bool,
    pub(super) wakeable: bool,
    pub(super) duration_secs: u64,
    pub(super) pid: Option<u32>,
    pub(super) stdout_bytes: u64,
    pub(super) stderr_bytes: u64,
    pub(super) unread_mail: usize,
    pub(super) open_tasks: usize,
}

#[derive(Clone, Debug)]
pub(super) struct TopPod {
    pub(super) pod: String,
    pub(super) sleeping: bool,
    pub(super) fins: usize,
    pub(super) running: usize,
    pub(super) paused: usize,
    pub(super) wakeable: usize,
    pub(super) unread_mail: usize,
    pub(super) open_tasks: usize,
    pub(super) error: Option<String>,
}

#[derive(Clone, Debug)]
struct TopSnapshot {
    pods: Vec<TopPod>,
    fins: Vec<TopFin>,
    error: Option<String>,
}

struct TopState {
    selected_pod: usize,
    last_wake: Instant,
    message: String,
}

impl TopState {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            selected_pod: 0,
            last_wake: initial_last_wake(now),
            message: String::new(),
        }
    }

    fn clamp_selection(&mut self, snapshot: &TopSnapshot) {
        if snapshot.pods.is_empty() {
            self.selected_pod = 0;
        } else if self.selected_pod >= snapshot.pods.len() {
            self.selected_pod = snapshot.pods.len() - 1;
        }
    }

    fn selected_pod<'a>(&self, snapshot: &'a TopSnapshot) -> Option<&'a TopPod> {
        snapshot.pods.get(self.selected_pod)
    }
}

impl TopSnapshot {
    fn collect(orqa: &Orqa) -> Self {
        let registry = match load_registry(orqa) {
            Ok(registry) => registry,
            Err(error) => {
                return Self {
                    pods: Vec::new(),
                    fins: Vec::new(),
                    error: Some(error),
                };
            }
        };

        let mut pods = Vec::new();
        let mut fins = Vec::new();

        for reg in registry.values().filter(|reg| reg.enabled) {
            let pod_ref = match PodRef::new(&reg.slug) {
                Ok(pod) => pod,
                Err(error) => {
                    pods.push(error_pod(&reg.slug, error));
                    continue;
                }
            };

            let status = match pod_status(orqa, &pod_ref) {
                Ok(status) => status,
                Err(error) => {
                    pods.push(error_pod(&reg.slug, error));
                    continue;
                }
            };

            let paused = if status.sleeping { 1 } else { 0 }
                + status.fins.iter().filter(|fin| fin.sleeping).count();

            pods.push(TopPod {
                pod: status.pod.clone(),
                sleeping: status.sleeping,
                fins: status.fin_count,
                running: status.running,
                paused,
                wakeable: status.wakeable,
                unread_mail: status.unread_mail,
                open_tasks: status.open_tasks,
                error: None,
            });

            for fin_status in status.fins {
                let fin_slug = fin_status
                    .fin
                    .split_once('/')
                    .map(|(_, fin)| fin.to_string())
                    .unwrap_or_else(|| fin_status.fin.clone());
                let duration_secs = running_duration(orqa, &status.pod, &fin_slug);
                let (stdout_bytes, stderr_bytes) = fin_status
                    .last_run
                    .as_ref()
                    .map(|run| {
                        (
                            fs::metadata(&run.stdout_log).map(|m| m.len()).unwrap_or(0),
                            fs::metadata(&run.stderr_log).map(|m| m.len()).unwrap_or(0),
                        )
                    })
                    .unwrap_or((0, 0));

                fins.push(TopFin {
                    pod: status.pod.clone(),
                    fin: fin_slug,
                    running: fin_status.running,
                    sleeping: fin_status.sleeping,
                    wakeable: !fin_status.sleeping
                        && !fin_status.running
                        && (fin_status.unread_mail > 0 || fin_status.open_tasks > 0),
                    duration_secs,
                    pid: fin_status.pid,
                    stdout_bytes,
                    stderr_bytes,
                    unread_mail: fin_status.unread_mail,
                    open_tasks: fin_status.open_tasks,
                });
            }
        }

        fins.sort_by(|a, b| {
            b.running
                .cmp(&a.running)
                .then_with(|| b.wakeable.cmp(&a.wakeable))
                .then_with(|| b.duration_secs.cmp(&a.duration_secs))
                .then_with(|| a.pod.cmp(&b.pod))
                .then_with(|| a.fin.cmp(&b.fin))
        });

        Self {
            pods,
            fins,
            error: None,
        }
    }

    fn totals(&self) -> (usize, usize, usize, usize, usize, usize) {
        self.pods.iter().fold(
            (0, 0, 0, 0, 0, 0),
            |(fins, running, paused, wakeable, mail, tasks), pod| {
                (
                    fins + pod.fins,
                    running + pod.running,
                    paused + pod.paused,
                    wakeable + pod.wakeable,
                    mail + pod.unread_mail,
                    tasks + pod.open_tasks,
                )
            },
        )
    }
}

pub fn run_top(orqa: &Orqa) -> Result<(), String> {
    if let Err(error) = enable_raw_mode() {
        return Err(format!("failed to enable raw mode for top: {error}"));
    }

    let mut stdout = stdout();
    if let Err(error) = execute!(stdout, EnterAlternateScreen) {
        let _ = disable_raw_mode();
        return Err(format!("failed to enter alternate screen: {error}"));
    }

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = match Terminal::new(backend) {
        Ok(terminal) => terminal,
        Err(error) => {
            let _ = disable_raw_mode();
            let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
            return Err(format!("failed to create ratatui terminal: {error}"));
        }
    };

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_top_loop(&mut terminal, orqa)
    }));

    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(error),
        Err(payload) => {
            let message = if let Some(message) = payload.downcast_ref::<&str>() {
                (*message).to_string()
            } else if let Some(message) = payload.downcast_ref::<String>() {
                message.clone()
            } else {
                "top TUI panicked (terminal was restored)".to_string()
            };
            Err(format!("top TUI error: {message}"))
        }
    }
}

fn run_top_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    orqa: &Orqa,
) -> Result<(), String> {
    let theme = default_theme();
    let args = vec![OsString::from(DEFAULT_GLOBAL_LOOP_PROMPT)];
    let mut state = TopState::new();

    loop {
        run_top_loop_tick(orqa, &args, &mut state)?;
        let snapshot = TopSnapshot::collect(orqa);
        state.clamp_selection(&snapshot);
        terminal
            .draw(|frame| render_top(frame, frame.area(), &theme, &snapshot, &state, orqa))
            .map_err(|error| format!("terminal draw failed: {error}"))?;

        if handle_top_input(orqa, &args, &snapshot, &mut state)? {
            return Ok(());
        }
    }
}

fn run_top_loop_tick(orqa: &Orqa, args: &[OsString], state: &mut TopState) -> Result<(), String> {
    if state.last_wake.elapsed() < TOP_LOOP_INTERVAL {
        return Ok(());
    }

    let mut errors = Vec::new();
    for (pod, result) in wake_all_pods(orqa, args, true)? {
        if let Err(error) = result {
            errors.push(format!("{pod}: {error}"));
        }
    }
    state.last_wake = Instant::now();
    state.message = if errors.is_empty() {
        "loop tick".to_string()
    } else {
        format!("loop errors: {}", errors.join("; "))
    };
    Ok(())
}

fn handle_top_input(
    orqa: &Orqa,
    args: &[OsString],
    snapshot: &TopSnapshot,
    state: &mut TopState,
) -> Result<bool, String> {
    if !event::poll(Duration::from_millis(250)).unwrap_or(false) {
        return Ok(false);
    }

    let event = event::read().map_err(|error| format!("event read failed: {error}"))?;
    let CrosstermEvent::Key(key) = event else {
        return Ok(false);
    };
    if key.kind != KeyEventKind::Press {
        return Ok(false);
    }

    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => Ok(true),
        KeyCode::Up => {
            state.selected_pod = state.selected_pod.saturating_sub(1);
            Ok(false)
        }
        KeyCode::Down => {
            if !snapshot.pods.is_empty() {
                state.selected_pod = (state.selected_pod + 1).min(snapshot.pods.len() - 1);
            }
            Ok(false)
        }
        KeyCode::Char('p') | KeyCode::Char('P') => {
            toggle_selected_pod(orqa, args, snapshot, state)?;
            Ok(false)
        }
        KeyCode::Char('w') | KeyCode::Char('W') => {
            if let Some(pod) = state.selected_pod(snapshot) {
                wake_pod_quiet(orqa, &pod.pod, false, false, false, args)?;
                state.last_wake = Instant::now();
                state.message = format!("wake {}", pod.pod);
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}

fn toggle_selected_pod(
    orqa: &Orqa,
    args: &[OsString],
    snapshot: &TopSnapshot,
    state: &mut TopState,
) -> Result<(), String> {
    let Some(pod) = state.selected_pod(snapshot) else {
        return Ok(());
    };
    let pod_ref = PodRef::new(&pod.pod)?;
    let path = orqa.pod_sleep_path(&pod_ref)?;
    if path.exists() {
        remove_sleep_marker(&path)?;
        wake_pod_quiet(orqa, &pod.pod, false, false, false, args)?;
        state.last_wake = Instant::now();
        state.message = format!("resume {} and wake", pod.pod);
    } else {
        write_sleep_marker(&path)?;
        state.message = format!("pause {}", pod.pod);
    }
    Ok(())
}

fn render_top(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    snapshot: &TopSnapshot,
    state: &TopState,
    orqa: &Orqa,
) {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(8),
            Constraint::Length(1),
            Constraint::Length(7),
            Constraint::Length(1),
        ])
        .split(area);

    render_header(frame, vertical[0], theme, snapshot, orqa);
    render_summary(frame, vertical[1], theme, snapshot);
    render_blank(frame, vertical[2]);
    render_fins(frame, vertical[3], theme, snapshot);
    render_blank(frame, vertical[4]);
    render_pods(frame, vertical[5], theme, snapshot, state);
    render_footer(frame, vertical[6], theme, state);
}

fn render_header(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    snapshot: &TopSnapshot,
    orqa: &Orqa,
) {
    let icon = if snapshot.fins.iter().any(|fin| fin.running) {
        spinner_frame()
    } else {
        "idle"
    };
    let left = vec![
        Span::styled(" orqa top ", Style::default().fg(theme.accent)),
        Span::styled(icon, Style::default().fg(theme.ok)),
        Span::raw("  "),
        Span::styled(
            format!("home {}", orqa.home.display()),
            Style::default().fg(theme.muted),
        ),
    ];
    let right =
        format!("loop {DEFAULT_GLOBAL_LOOP_INTERVAL}s  up/down select  p pause  w wake  q quit");
    let spacer = area
        .width
        .saturating_sub(line_width(&left) as u16)
        .saturating_sub(right.len() as u16) as usize;
    let mut spans = left;
    spans.push(Span::raw(" ".repeat(spacer)));
    spans.push(Span::styled(right, Style::default().fg(theme.muted)));

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_summary(frame: &mut Frame, area: Rect, theme: &Theme, snapshot: &TopSnapshot) {
    let (fins, running, paused, wakeable, mail, tasks) = snapshot.totals();
    let bg = theme.header_bg;
    let text = if let Some(error) = &snapshot.error {
        Line::from(vec![
            Span::styled(" registry error ", Style::default().fg(theme.error).bg(bg)),
            Span::styled(error.clone(), Style::default().fg(theme.text).bg(bg)),
        ])
    } else {
        Line::from(vec![
            metric("pods", snapshot.pods.len(), theme.text, bg),
            metric("fins", fins, theme.text, bg),
            metric("running", running, theme.ok, bg),
            metric("paused", paused, theme.warn, bg),
            metric("wakeable", wakeable, theme.accent, bg),
            metric("mail", mail, theme.mail, bg),
            metric("tasks", tasks, theme.event, bg),
        ])
    };
    let width = area.width as usize;
    let padding = width.saturating_sub(line_width(text.spans.as_slice()));
    let mut spans = text.spans;
    spans.push(Span::styled(" ".repeat(padding), Style::default().bg(bg)));

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_blank(frame: &mut Frame, area: Rect) {
    frame.render_widget(Paragraph::new(""), area);
}

fn render_fins(frame: &mut Frame, area: Rect, theme: &Theme, snapshot: &TopSnapshot) {
    let rows: Vec<Row<'_>> = if snapshot.fins.is_empty() {
        vec![Row::new(vec![padded_cell("No fins registered")])]
    } else {
        snapshot
            .fins
            .iter()
            .map(|fin| {
                let style = if fin.running {
                    Style::default().fg(theme.ok).bg(theme.panel_bg)
                } else if fin.sleeping {
                    Style::default().fg(theme.warn)
                } else if fin.wakeable {
                    Style::default().fg(theme.accent).bg(theme.operator_bg)
                } else {
                    Style::default().fg(theme.text)
                };

                Row::new(vec![
                    padded_cell(fin.pod.clone()),
                    padded_cell(fin.fin.clone()),
                    padded_cell(fin_status_symbol(fin)),
                    padded_cell(format_duration(fin.duration_secs)),
                    padded_cell(fin.pid.map_or("-".to_string(), |pid| pid.to_string())),
                    padded_cell(format_bytes(fin.stdout_bytes)),
                    padded_cell(format_bytes(fin.stderr_bytes)),
                    padded_cell(fin.unread_mail.to_string()),
                    padded_cell(fin.open_tasks.to_string()),
                ])
                .style(style)
            })
            .collect()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(19),
            Constraint::Length(11),
            Constraint::Length(2),
            Constraint::Length(5),
            Constraint::Length(6),
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Length(5),
            Constraint::Length(6),
        ],
    )
    .header(header_row(
        [
            "Pod", "Fin", "S", "Age", "PID", "Out", "Err", "Mail", "Tasks",
        ],
        theme,
    ));

    frame.render_widget(table, area);
}

fn render_pods(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    snapshot: &TopSnapshot,
    state: &TopState,
) {
    let rows: Vec<Row<'_>> = if snapshot.pods.is_empty() {
        vec![Row::new(vec![padded_cell("No pods registered")])]
    } else {
        snapshot
            .pods
            .iter()
            .enumerate()
            .map(|(index, pod)| {
                if let Some(error) = &pod.error {
                    let style = if index == state.selected_pod {
                        Style::default().fg(theme.error).bg(theme.operator_bg)
                    } else {
                        Style::default().fg(theme.error)
                    };
                    return Row::new(vec![
                        padded_cell(pod.pod.clone()),
                        padded_cell("error"),
                        padded_cell(error.clone()),
                    ])
                    .style(style);
                }

                let mut style = if pod.sleeping {
                    Style::default().fg(theme.warn)
                } else if pod.running > 0 {
                    Style::default().fg(theme.ok).bg(theme.panel_bg)
                } else if pod.wakeable > 0 {
                    Style::default().fg(theme.accent).bg(theme.operator_bg)
                } else {
                    Style::default().fg(theme.text)
                };
                if index == state.selected_pod {
                    style = style.bg(theme.operator_bg).add_modifier(Modifier::BOLD);
                }
                Row::new(vec![
                    padded_cell(pod.pod.clone()),
                    padded_cell(pod_status_symbol(pod)),
                    padded_cell(pod.fins.to_string()),
                    padded_cell(pod.running.to_string()),
                    padded_cell(pod.paused.to_string()),
                    padded_cell(pod.wakeable.to_string()),
                    padded_cell(pod.unread_mail.to_string()),
                    padded_cell(pod.open_tasks.to_string()),
                ])
                .style(style)
            })
            .collect()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(19),
            Constraint::Length(2),
            Constraint::Length(5),
            Constraint::Length(4),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Length(6),
        ],
    )
    .header(header_row(
        ["Pod", "S", "Fins", "Run", "P", "W", "Mail", "Tasks"],
        theme,
    ));

    frame.render_widget(table, area);
}

fn render_footer(frame: &mut Frame, area: Rect, theme: &Theme, state: &TopState) {
    let next = next_loop_label(state.last_wake, Instant::now());
    let right = if state.message.is_empty() || state.message == "loop tick" {
        format!(" {next}")
    } else {
        format!(" {}  {next}", state.message)
    };
    let left = vec![
        Span::raw(" "),
        Span::styled("↑/↓", Style::default().fg(theme.accent)),
        Span::styled(" select   ", Style::default().fg(theme.muted)),
        Span::styled("p", Style::default().fg(theme.accent)),
        Span::styled(" pause/resume   ", Style::default().fg(theme.muted)),
        Span::styled("w", Style::default().fg(theme.accent)),
        Span::styled(" wake   ", Style::default().fg(theme.muted)),
        Span::styled("q", Style::default().fg(theme.accent)),
        Span::styled(" quit", Style::default().fg(theme.muted)),
    ];
    let spacer = area
        .width
        .saturating_sub(line_width(&left) as u16)
        .saturating_sub(right.chars().count() as u16) as usize;
    let mut spans = left;
    spans.push(Span::raw(" ".repeat(spacer)));
    spans.push(Span::styled(right, Style::default().fg(theme.muted)));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn header_row<const N: usize>(labels: [&'static str; N], theme: &Theme) -> Row<'static> {
    Row::new(labels.map(padded_cell)).style(
        Style::default()
            .fg(theme.text)
            .bg(theme.bar_bg)
            .add_modifier(Modifier::BOLD),
    )
}

fn padded_cell(value: impl Into<String>) -> Cell<'static> {
    Cell::from(format!(" {}", value.into()))
}

fn metric(
    label: &'static str,
    value: usize,
    color: ratatui::style::Color,
    bg: ratatui::style::Color,
) -> Span<'static> {
    Span::styled(
        format!(" {label} {value} "),
        Style::default()
            .fg(color)
            .bg(bg)
            .add_modifier(Modifier::BOLD),
    )
}

fn error_pod(slug: &str, error: String) -> TopPod {
    TopPod {
        pod: slug.to_string(),
        sleeping: false,
        fins: 0,
        running: 0,
        paused: 0,
        wakeable: 0,
        unread_mail: 0,
        open_tasks: 0,
        error: Some(error),
    }
}

pub(super) fn fin_status_symbol(fin: &TopFin) -> &'static str {
    if fin.running {
        "R"
    } else if fin.sleeping {
        "P"
    } else if fin.wakeable {
        "W"
    } else {
        "-"
    }
}

pub(super) fn pod_status_symbol(pod: &TopPod) -> &'static str {
    if pod.error.is_some() {
        "E"
    } else if pod.sleeping {
        "P"
    } else if pod.running > 0 {
        "R"
    } else if pod.wakeable > 0 {
        "W"
    } else {
        "-"
    }
}

pub(super) fn next_loop_label(last_wake: Instant, now: Instant) -> String {
    let elapsed = now.saturating_duration_since(last_wake);
    let remaining = TOP_LOOP_INTERVAL.saturating_sub(elapsed).as_secs();
    format!("next: {remaining}s")
}

pub(super) fn initial_last_wake(now: Instant) -> Instant {
    let elapsed_at_start = TOP_LOOP_INTERVAL.saturating_sub(TOP_INITIAL_LOOP_DELAY);
    now.checked_sub(elapsed_at_start).unwrap_or(now)
}

fn running_duration(orqa: &Orqa, pod: &str, fin: &str) -> u64 {
    let Ok(fin_ref) = FinRef::new(pod, fin) else {
        return 0;
    };
    let Ok(lock_path) = orqa.lock_path(&fin_ref) else {
        return 0;
    };

    fs::metadata(lock_path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn format_duration(secs: u64) -> String {
    if secs == 0 {
        "-".to_string()
    } else if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    }
}

fn spinner_frame() -> &'static str {
    const FRAMES: [&str; 4] = ["|", "/", "-", "\\"];
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    FRAMES[((millis / 160) as usize) % FRAMES.len()]
}

fn line_width(spans: &[Span<'_>]) -> usize {
    spans.iter().map(|span| span.content.chars().count()).sum()
}
