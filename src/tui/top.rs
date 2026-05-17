//! Ratatui global top view for all registered pods.

use std::{
    fs,
    io::{self, stdout},
    time::{Duration, SystemTime, UNIX_EPOCH},
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
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

use crate::{
    model::{FinRef, Orqa, PodRef, load_registry},
    status::pod_status,
    tui::theme::{Theme, default_theme},
};

#[derive(Clone, Debug)]
struct TopFin {
    pod: String,
    fin: String,
    running: bool,
    sleeping: bool,
    wakeable: bool,
    duration_secs: u64,
    pid: Option<u32>,
    stdout_bytes: u64,
    stderr_bytes: u64,
    unread_mail: usize,
    open_tasks: usize,
}

#[derive(Clone, Debug)]
struct TopPod {
    pod: String,
    sleeping: bool,
    fins: usize,
    running: usize,
    paused: usize,
    wakeable: usize,
    unread_mail: usize,
    open_tasks: usize,
    error: Option<String>,
}

#[derive(Clone, Debug)]
struct TopSnapshot {
    pods: Vec<TopPod>,
    fins: Vec<TopFin>,
    error: Option<String>,
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

    loop {
        let snapshot = TopSnapshot::collect(orqa);
        terminal
            .draw(|frame| render_top(frame, frame.area(), &theme, &snapshot, orqa))
            .map_err(|error| format!("terminal draw failed: {error}"))?;

        if should_quit()? {
            return Ok(());
        }
    }
}

fn should_quit() -> Result<bool, String> {
    if !event::poll(Duration::from_millis(1000)).unwrap_or(false) {
        return Ok(false);
    }

    let event = event::read().map_err(|error| format!("event read failed: {error}"))?;
    let CrosstermEvent::Key(key) = event else {
        return Ok(false);
    };
    if key.kind != KeyEventKind::Press {
        return Ok(false);
    }

    Ok(matches!(key.code, KeyCode::Char('q') | KeyCode::Esc))
}

fn render_top(frame: &mut Frame, area: Rect, theme: &Theme, snapshot: &TopSnapshot, orqa: &Orqa) {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(8),
            Constraint::Length(1),
        ])
        .split(area);

    render_header(frame, vertical[0], theme, snapshot, orqa);
    render_summary(frame, vertical[1], theme, snapshot);
    render_fins(frame, vertical[2], theme, snapshot);
    render_pods(frame, vertical[3], theme, snapshot);
    render_footer(frame, vertical[4], theme);
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
    let right = "refresh 1s  q/Esc quit";
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
    let text = if let Some(error) = &snapshot.error {
        vec![Line::from(vec![
            Span::styled(" registry error ", Style::default().fg(theme.error)),
            Span::styled(error.clone(), Style::default().fg(theme.text)),
        ])]
    } else {
        vec![Line::from(vec![
            metric("pods", snapshot.pods.len(), theme.text),
            metric("fins", fins, theme.text),
            metric("running", running, theme.ok),
            metric("paused", paused, theme.warn),
            metric("wakeable", wakeable, theme.accent),
            metric("mail", mail, theme.mail),
            metric("tasks", tasks, theme.event),
        ])]
    };

    frame.render_widget(
        Paragraph::new(text).block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(theme.muted)),
        ),
        area,
    );
}

fn render_fins(frame: &mut Frame, area: Rect, theme: &Theme, snapshot: &TopSnapshot) {
    let rows: Vec<Row<'_>> = if snapshot.fins.is_empty() {
        vec![Row::new(vec![Cell::from("No fins registered")])]
    } else {
        snapshot
            .fins
            .iter()
            .map(|fin| {
                let status = if fin.running {
                    "running"
                } else if fin.sleeping {
                    "paused"
                } else if fin.wakeable {
                    "wakeable"
                } else {
                    "idle"
                };
                let style = if fin.running {
                    Style::default().fg(theme.ok)
                } else if fin.sleeping {
                    Style::default().fg(theme.warn)
                } else if fin.wakeable {
                    Style::default().fg(theme.accent)
                } else {
                    Style::default().fg(theme.text)
                };

                Row::new(vec![
                    Cell::from(fin.pod.clone()),
                    Cell::from(fin.fin.clone()),
                    Cell::from(status),
                    Cell::from(format_duration(fin.duration_secs)),
                    Cell::from(fin.pid.map_or("-".to_string(), |pid| pid.to_string())),
                    Cell::from(format_bytes(fin.stdout_bytes)),
                    Cell::from(format_bytes(fin.stderr_bytes)),
                    Cell::from(fin.unread_mail.to_string()),
                    Cell::from(fin.open_tasks.to_string()),
                ])
                .style(style)
            })
            .collect()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(14),
            Constraint::Length(14),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(6),
            Constraint::Length(7),
        ],
    )
    .header(header_row(
        [
            "Pod", "Fin", "Status", "Age", "PID", "Stdout", "Stderr", "Mail", "Tasks",
        ],
        theme,
    ))
    .block(
        Block::default()
            .title(" Fins ")
            .borders(Borders::ALL)
            .style(Style::default().fg(theme.muted)),
    );

    frame.render_widget(table, area);
}

fn render_pods(frame: &mut Frame, area: Rect, theme: &Theme, snapshot: &TopSnapshot) {
    let rows: Vec<Row<'_>> = if snapshot.pods.is_empty() {
        vec![Row::new(vec![Cell::from("No pods registered")])]
    } else {
        snapshot
            .pods
            .iter()
            .map(|pod| {
                if let Some(error) = &pod.error {
                    return Row::new(vec![
                        Cell::from(pod.pod.clone()),
                        Cell::from("error"),
                        Cell::from(error.clone()),
                    ])
                    .style(Style::default().fg(theme.error));
                }

                let status = if pod.sleeping {
                    "paused"
                } else if pod.running > 0 {
                    "running"
                } else if pod.wakeable > 0 {
                    "wakeable"
                } else {
                    "idle"
                };
                Row::new(vec![
                    Cell::from(pod.pod.clone()),
                    Cell::from(status),
                    Cell::from(pod.fins.to_string()),
                    Cell::from(pod.running.to_string()),
                    Cell::from(pod.paused.to_string()),
                    Cell::from(pod.wakeable.to_string()),
                    Cell::from(pod.unread_mail.to_string()),
                    Cell::from(pod.open_tasks.to_string()),
                ])
            })
            .collect()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(16),
            Constraint::Length(10),
            Constraint::Length(6),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(9),
            Constraint::Length(6),
            Constraint::Length(7),
        ],
    )
    .header(header_row(
        [
            "Pod", "Status", "Fins", "Running", "Paused", "Wakeable", "Mail", "Tasks",
        ],
        theme,
    ))
    .block(
        Block::default()
            .title(" Pods ")
            .borders(Borders::ALL)
            .style(Style::default().fg(theme.muted)),
    );

    frame.render_widget(table, area);
}

fn render_footer(frame: &mut Frame, area: Rect, theme: &Theme) {
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled("q", Style::default().fg(theme.accent)),
            Span::styled(" quit   ", Style::default().fg(theme.muted)),
            Span::styled("Esc", Style::default().fg(theme.accent)),
            Span::styled(" quit", Style::default().fg(theme.muted)),
        ])),
        area,
    );
}

fn header_row<const N: usize>(labels: [&'static str; N], theme: &Theme) -> Row<'static> {
    Row::new(labels.map(Cell::from)).style(
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )
}

fn metric(label: &'static str, value: usize, color: ratatui::style::Color) -> Span<'static> {
    Span::styled(
        format!(" {label} {value} "),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
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
