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
use super::loopctl::start_tui_loop_worker;
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
    let _loop_worker = match start_tui_loop_worker(&orqa, &reg) {
        Ok(worker) => Some(worker),
        Err(error) => {
            eprintln!("warning: failed to start TUI loop worker: {error}");
            None
        }
    };
    let app_orqa = Orqa::new(Some(orqa.home.clone()));
    let app_reg = reg.clone();
    let watcher = PodWatcher::new(orqa, reg)?;
    let mut app = App::new(
        pod_slug.to_string(),
        pod_root.to_path_buf(),
        app_orqa,
        app_reg,
        watcher,
    );

    loop {
        // Poll watcher for new events
        app.refresh_loop_countdown();
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
                        KeyCode::Char('.')
                            if key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            app.toggle_command_palette();
                        }
                        KeyCode::Char('t')
                            if key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            app.open_target_picker();
                        }
                        KeyCode::Esc if app.show_target_picker => {
                            app.show_target_picker = false;
                        }
                        KeyCode::Up if app.show_target_picker => {
                            app.target_picker_prev();
                        }
                        KeyCode::Down if app.show_target_picker => {
                            app.target_picker_next();
                        }
                        KeyCode::Enter if app.show_target_picker => {
                            app.select_target_picker();
                        }
                        _ if app.show_target_picker => {}
                        KeyCode::Esc if app.show_command_palette => {
                            app.show_command_palette = false;
                        }
                        _ if app.show_command_palette => {}
                        KeyCode::BackTab => {
                            app.toggle_input_mode();
                        }
                        KeyCode::Esc if app.mode == super::app::InputMode::Input => {
                            app.mode = super::app::InputMode::Normal;
                        }
                        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Char('c')
                            if key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            return Ok(());
                        }

                        KeyCode::Char('i') | KeyCode::Char('I')
                            if app.mode == super::app::InputMode::Normal =>
                        {
                            app.mode = super::app::InputMode::Input;
                        }

                        KeyCode::Char('H') if app.mode == super::app::InputMode::Normal => {
                            app.cycle_theme();
                        }
                        KeyCode::Char('p') | KeyCode::Char('P')
                            if app.mode == super::app::InputMode::Normal =>
                        {
                            if let Err(error) = app.toggle_pod_pause() {
                                app.events.push(crate::tui::events::Event::OperatorAction {
                                    text: format!("failed to toggle pod pause: {error}"),
                                });
                            }
                        }

                        KeyCode::Char('w')
                            if app.mode == super::app::InputMode::Input
                                && key
                                    .modifiers
                                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            app.composer.delete_previous_word();
                        }

                        KeyCode::Char('F') if app.mode == super::app::InputMode::Normal => {
                            let mut fins: Vec<String> = app.known_fins.iter().cloned().collect();
                            fins.sort();
                            let next = if fins.is_empty() {
                                None
                            } else if let Some(current) = &app.filters.fin_filter {
                                if let Some(pos) = fins.iter().position(|f| f == current) {
                                    if pos + 1 < fins.len() {
                                        Some(fins[pos + 1].clone())
                                    } else {
                                        None
                                    }
                                } else {
                                    Some(fins[0].clone())
                                }
                            } else {
                                Some(fins[0].clone())
                            };
                            app.set_fin_filter(next);
                        }

                        KeyCode::Tab | KeyCode::Char('f') | KeyCode::Char('F')
                            if app.mode == super::app::InputMode::Input =>
                        {
                            app.open_target_picker();
                        }

                        KeyCode::Char('f') if app.mode == super::app::InputMode::Normal => {
                            app.open_target_picker();
                        }

                        KeyCode::Char('o') | KeyCode::Char('O')
                            if app.mode == super::app::InputMode::Normal =>
                        {
                            app.toggle_operator_filter()
                        }
                        KeyCode::Char('/') | KeyCode::Char('t') | KeyCode::Char('T')
                            if app.mode == super::app::InputMode::Normal =>
                        {
                            if app.filters.thread_query.is_some() {
                                app.set_thread_query(None);
                            } else {
                                app.set_thread_query(Some("auth".into()));
                            }
                        }

                        // === Composer input (Phase 4) ===
                        #[allow(clippy::collapsible_match)]
                        KeyCode::Char(c) if app.mode == super::app::InputMode::Input => {
                            if !key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL)
                            {
                                app.composer.insert_char(c);
                            }
                        }
                        KeyCode::Backspace if app.mode == super::app::InputMode::Input => {
                            app.composer.backspace()
                        }
                        KeyCode::Delete if app.mode == super::app::InputMode::Input => {
                            app.composer.delete()
                        }
                        KeyCode::Left if app.mode == super::app::InputMode::Input => {
                            app.composer.move_left()
                        }
                        KeyCode::Right if app.mode == super::app::InputMode::Input => {
                            app.composer.move_right()
                        }
                        KeyCode::Home if app.mode == super::app::InputMode::Input => {
                            app.composer.move_home()
                        }
                        KeyCode::End if app.mode == super::app::InputMode::Input => {
                            app.composer.move_end()
                        }

                        KeyCode::Up => {
                            // If input is empty, scroll timeline; otherwise history
                            if app.mode == super::app::InputMode::Normal
                                || app.composer.input.is_empty()
                            {
                                app.scroll_up(1);
                            } else {
                                app.composer.history_prev();
                            }
                        }
                        KeyCode::Down => {
                            if app.mode == super::app::InputMode::Normal
                                || app.composer.input.is_empty()
                            {
                                app.scroll_down(1);
                            } else {
                                app.composer.history_next();
                            }
                        }

                        KeyCode::Enter if app.mode == super::app::InputMode::Input => {
                            if let Some(msg) = app.composer.submit() {
                                if let Err(error) = app.send_operator_message(&msg) {
                                    app.composer.set_status(format!("send failed: {error}"));
                                    app.events.push(crate::tui::events::Event::OperatorAction {
                                        text: format!("failed to mail target fin: {error}"),
                                    });
                                }
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
