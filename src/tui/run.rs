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

                        // `f` now changes the *composer target fin* (Phase 4)
                        KeyCode::Char('f') | KeyCode::Char('F') => {
                            let mut fins: Vec<String> = app.known_fins.iter().cloned().collect();
                            fins.sort();
                            let current = &app.composer.target_fin;
                            let next = if fins.is_empty() {
                                "planner".to_string()
                            } else if let Some(pos) = fins.iter().position(|f| f == current) {
                                if pos + 1 < fins.len() {
                                    fins[pos + 1].clone()
                                } else {
                                    fins[0].clone()
                                }
                            } else {
                                fins[0].clone()
                            };
                            app.composer.set_target(next);
                        }
                        KeyCode::Char('o') | KeyCode::Char('O') => app.toggle_operator_filter(),
                        KeyCode::Char('/') | KeyCode::Char('t') | KeyCode::Char('T') => {
                            if app.filters.thread_query.is_some() {
                                app.set_thread_query(None);
                            } else {
                                app.set_thread_query(Some("auth".into()));
                            }
                        }

                        // === Composer input (Phase 4) ===
                        KeyCode::Char(c) => {
                            if !key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL)
                            {
                                app.composer.insert_char(c);
                            }
                        }
                        KeyCode::Backspace => app.composer.backspace(),
                        KeyCode::Delete => app.composer.delete(),
                        KeyCode::Left => app.composer.move_left(),
                        KeyCode::Right => app.composer.move_right(),
                        KeyCode::Home => app.composer.move_home(),
                        KeyCode::End => app.composer.move_end(),

                        KeyCode::Up => {
                            // If input is empty, scroll timeline; otherwise history
                            if app.composer.input.is_empty() {
                                app.scroll_up(1);
                            } else {
                                app.composer.history_prev();
                            }
                        }
                        KeyCode::Down => {
                            if app.composer.input.is_empty() {
                                app.scroll_down(1);
                            } else {
                                app.composer.history_next();
                            }
                        }

                        KeyCode::Enter => {
                            if let Some(msg) = app.composer.submit() {
                                // TODO in this phase: actually send the mail + wake
                                // For now just create a local OperatorAction so the user sees something
                                let action_text =
                                    format!("mailed {}: \"{}\"", app.composer.target_fin, msg);
                                app.events.push(crate::tui::events::Event::OperatorAction {
                                    text: action_text,
                                });
                                app.auto_follow_if_needed();
                            }
                        }

                        // Scrolling (Ctrl+arrows as fallback)
                        KeyCode::PageUp => app.scroll_up(12),
                        KeyCode::PageDown => app.scroll_down(12),

                        _ => {}
                    }
                }
            }
        }
    }
}
