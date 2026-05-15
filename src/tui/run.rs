//! Ratatui Operator Cockpit entry point (Phase 3+).
//!
//! Real scrollable timeline with filters, powered by the Phase 2 event system.

use std::io::{self, stdout};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::model::{Orqa, PodRegistration};

use super::app::App;
use super::watcher::PodWatcher;

/// Run the Operator Cockpit TUI for a detected Phase 05 pod.
pub fn run_tui(pod_slug: &str, pod_root: &std::path::Path) -> Result<(), String> {
    if let Err(e) = enable_raw_mode() {
        return Err(format!("failed to enable raw mode for TUI: {e}"));
    }

    let mut stdout = stdout();
    if let Err(e) = execute!(stdout, EnterAlternateScreen) {
        let _ = disable_raw_mode();
        return Err(format!("failed to enter alternate screen: {e}"));
    }

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = match Terminal::new(backend) {
        Ok(t) => t,
        Err(e) => {
            let _ = disable_raw_mode();
            let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
            return Err(format!("failed to create ratatui terminal: {e}"));
        }
    };

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_event_loop(&mut terminal, pod_slug, pod_root)
    }));

    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(panic_payload) => {
            let msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "TUI panicked (terminal was restored)".to_string()
            };
            Err(format!("TUI error: {msg}"))
        }
    }
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    pod_slug: &str,
    pod_root: &std::path::Path,
) -> Result<(), String> {
    // Create watcher (Phase 2) and App (Phase 3 UI state)
    let orqa = Orqa::new(None);
    let reg = PodRegistration {
        slug: pod_slug.to_string(),
        path: pod_root.to_path_buf(),
        enabled: true,
    };
    let watcher = PodWatcher::new(orqa, reg)?;
    let mut app = App::new(pod_slug.to_string(), pod_root.to_path_buf(), watcher);

    loop {
        // Poll watcher for new events
        app.poll_watcher();
        app.auto_follow_if_needed();

        // Draw the real timeline UI
        terminal
            .draw(|frame| {
                let area = frame.area();
                app.render(frame, area);
            })
            .map_err(|e| format!("terminal draw failed: {e}"))?;

        // Input handling
        if event::poll(std::time::Duration::from_millis(180)).unwrap_or(false) {
            if let Event::Key(key) = event::read().map_err(|e| format!("event read failed: {e}"))? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Char('c')
                            if key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            return Ok(());
                        }

                        // Filters (Phase 3)
                        KeyCode::Char('f') | KeyCode::Char('F') => {
                            let mut fins: Vec<String> = app.known_fins.iter().cloned().collect();
                            fins.sort();
                            let next = match &app.filters.fin_filter {
                                None if !fins.is_empty() => Some(fins[0].clone()),
                                Some(cur) => {
                                    if let Some(pos) = fins.iter().position(|f| f == cur) {
                                        if pos + 1 < fins.len() {
                                            Some(fins[pos + 1].clone())
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                }
                                _ => None,
                            };
                            app.set_fin_filter(next);
                        }
                        KeyCode::Char('o') | KeyCode::Char('O') => app.toggle_operator_filter(),
                        KeyCode::Char('/') | KeyCode::Char('t') | KeyCode::Char('T') => {
                            if app.filters.thread_query.is_some() {
                                app.set_thread_query(None);
                            } else {
                                app.set_thread_query(Some("auth".into()));
                            }
                        }

                        // Scrolling
                        KeyCode::Up => app.scroll_up(1),
                        KeyCode::Down => app.scroll_down(1),
                        KeyCode::PageUp => app.scroll_up(12),
                        KeyCode::PageDown => app.scroll_down(12),
                        KeyCode::Home => {
                            app.follow = false;
                            app.list_state.select(Some(0));
                        }
                        KeyCode::End => app.scroll_to_bottom(),

                        _ => {}
                    }
                }
            }
        }
    }
}
