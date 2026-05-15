//! Minimal Ratatui application entry point for the Operator Cockpit (Phase 1).
//!
//! This is a skeleton that proves we can enter a full-screen TUI from bare `orqa`
//! when a pod is detected, show basic information, and exit cleanly.
//! Later phases will replace the body with the real timeline + composer.

use std::io::{self, stdout};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend, layout::Alignment, widgets::Paragraph};

use crate::model::{Orqa, PodRegistration};

/// Run the Phase 1 TUI skeleton for the given detected pod.
///
/// Returns when the user presses `q`, `Esc`, or the app decides to exit.
pub fn run_tui(pod_slug: &str, pod_root: &std::path::Path) -> Result<(), String> {
    // Best-effort terminal setup. If this fails we want a clean error, not a
    // corrupted terminal.
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

    // Make sure we restore the terminal even if the event loop panics.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_event_loop(&mut terminal, pod_slug, pod_root)
    }));

    // Always attempt to restore the terminal.
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(panic_payload) => {
            // Convert panic into a friendly error.
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
    // Phase 2: create a real PodWatcher so we can prove the event system works
    // even while the UI is still the minimal skeleton.
    let orqa = Orqa::new(None);
    let reg = PodRegistration {
        slug: pod_slug.to_string(),
        path: pod_root.to_path_buf(),
        enabled: true,
    };
    let mut watcher = match crate::tui::PodWatcher::new(orqa, reg) {
        Ok(w) => Some(w),
        Err(e) => {
            eprintln!("warning: failed to create PodWatcher for Phase 2 demo: {e}");
            None
        }
    };
    let mut event_count: usize = 0;

    loop {
        // Phase 2: poll the watcher and count events (we don't render them yet)
        if let Some(w) = &mut watcher {
            if let Ok(new_events) = w.poll() {
                event_count += new_events.len();
            }
        }

        terminal
            .draw(|frame| {
                let area = frame.area();

                let title = format!("orqa • {} — Operator Cockpit (Phase 2)", pod_slug);
                let root_line = format!("root: {}", pod_root.display());
                let events_line = format!("events captured (Phase 2 watcher): {}", event_count);

                let text = format!(
                    "{}\n\n{}\n\n{}\n\n\
                     Phase 2: Event model + PodWatcher are live.\n\
                     Log lines, mail arrivals, and run state changes are being detected.\n\n\
                     (Full timeline rendering arrives in Phase 3)\n\n\
                     Press q, Esc, or Ctrl-C to exit.",
                    title, root_line, events_line
                );

                let paragraph = Paragraph::new(text).alignment(Alignment::Center);

                frame.render_widget(paragraph, area);
            })
            .map_err(|e| format!("terminal draw failed: {e}"))?;

        // Non-blocking event poll with a short timeout so the watcher keeps running.
        if event::poll(std::time::Duration::from_millis(250)).unwrap_or(false) {
            if let Event::Key(key) = event::read().map_err(|e| format!("event read failed: {e}"))? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                            return Ok(());
                        }
                        KeyCode::Char('c')
                            if key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            return Ok(());
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
