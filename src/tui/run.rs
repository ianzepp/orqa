//! Ratatui Operator Cockpit entry point.
//!
//! Scrollable timeline with filters, powered by the event system.

use std::io::{self, stdout};

use crossterm::{
    event::{self, Event as CrosstermEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::model::{Orqa, PodRegistration};

use super::app::{App, ChatMode, InputMode, MailMode, Surface};
use super::loopctl::start_tui_loop_worker;
use super::watcher::PodWatcher;

#[derive(Clone, Copy, PartialEq, Eq)]
enum LoopAction {
    Continue,
    Quit,
}

/// Run the Operator Cockpit TUI for a detected project-root pod.
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
        refresh_app(&mut app);
        draw_app(terminal, &mut app)?;

        if handle_pending_input(&mut app)? == LoopAction::Quit {
            return Ok(());
        }
    }
}

fn refresh_app(app: &mut App) {
    app.refresh_loop_countdown();
    app.poll_watcher();
    app.auto_follow_if_needed();
}

fn draw_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<(), String> {
    terminal
        .draw(|frame| {
            let area = frame.area();
            app.render(frame, area);
        })
        .map(|_| ())
        .map_err(|e| format!("terminal draw failed: {e}"))
}

fn handle_pending_input(app: &mut App) -> Result<LoopAction, String> {
    if !event::poll(std::time::Duration::from_millis(180)).unwrap_or(false) {
        return Ok(LoopAction::Continue);
    }

    let event = event::read().map_err(|e| format!("event read failed: {e}"))?;
    let CrosstermEvent::Key(key) = event else {
        return Ok(LoopAction::Continue);
    };
    if key.kind != KeyEventKind::Press {
        return Ok(LoopAction::Continue);
    }

    Ok(handle_key(app, key))
}

fn handle_key(app: &mut App, key: KeyEvent) -> LoopAction {
    if handle_global_key(app, key) {
        return LoopAction::Continue;
    }
    if app.show_target_picker {
        handle_target_picker_key(app, key);
        return LoopAction::Continue;
    }
    if app.show_command_palette {
        handle_command_palette_key(app, key);
        return LoopAction::Continue;
    }
    if app.surface == Surface::Mail {
        handle_mail_key(app, key);
        return LoopAction::Continue;
    }
    if app.surface == Surface::Chat {
        handle_chat_key(app, key);
        return LoopAction::Continue;
    }

    match app.mode {
        InputMode::Normal => handle_normal_key(app, key),
        InputMode::Chat => handle_input_key(app, key),
    }
}

fn handle_global_key(app: &mut App, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('.') if has_control(key) => {
            app.toggle_command_palette();
            true
        }
        KeyCode::Char('t') if has_control(key) => {
            app.open_target_picker();
            true
        }
        _ => false,
    }
}

fn handle_target_picker_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.show_target_picker = false,
        KeyCode::Up => app.target_picker_prev(),
        KeyCode::Down => app.target_picker_next(),
        KeyCode::Enter => app.select_target_picker(),
        _ => {}
    }
}

fn handle_command_palette_key(app: &mut App, key: KeyEvent) {
    if key.code == KeyCode::Esc {
        app.show_command_palette = false;
    }
}

fn handle_normal_key(app: &mut App, key: KeyEvent) -> LoopAction {
    match key.code {
        KeyCode::BackTab => app.toggle_input_mode(),
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => return LoopAction::Quit,
        KeyCode::Char('c') if has_control(key) => return LoopAction::Quit,
        KeyCode::Char('i') | KeyCode::Char('I') => app.mode = InputMode::Chat,
        KeyCode::Char('c') | KeyCode::Char('C') => app.show_chat_surface(),
        KeyCode::Char('m') | KeyCode::Char('M') => app.toggle_surface(),
        KeyCode::Char('H') => app.cycle_theme(),
        KeyCode::Char('p') | KeyCode::Char('P') => toggle_pod_pause(app),
        KeyCode::Char('F') => cycle_fin_filter(app),
        KeyCode::Char('f') => app.open_target_picker(),
        KeyCode::Char('o') | KeyCode::Char('O') => app.toggle_operator_filter(),
        KeyCode::Char('/') | KeyCode::Char('t') | KeyCode::Char('T') => toggle_thread_query(app),
        KeyCode::Up => app.scroll_up(1),
        KeyCode::Down => app.scroll_down(1),
        KeyCode::PageUp => app.scroll_up(12),
        KeyCode::PageDown => app.scroll_down(12),
        _ => {}
    }

    LoopAction::Continue
}

fn handle_chat_key(app: &mut App, key: KeyEvent) {
    if key.code == KeyCode::Tab {
        app.show_timeline_surface();
        return;
    }

    match app.chat_mode {
        ChatMode::Index => handle_chat_index_key(app, key),
        ChatMode::Detail => handle_chat_detail_key(app, key),
    }
}

fn handle_chat_index_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.show_timeline_surface(),
        KeyCode::Down | KeyCode::Char('j') => app.chat_cursor_down(),
        KeyCode::Up | KeyCode::Char('k') => app.chat_cursor_up(),
        KeyCode::Char('g') | KeyCode::Home => app.chat_cursor_top(),
        KeyCode::Char('G') | KeyCode::End => app.chat_cursor_bottom(),
        KeyCode::Enter | KeyCode::Char(' ') => app.open_selected_chat(),
        _ => {}
    }
}

fn handle_chat_detail_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('i') => app.close_chat_detail(),
        KeyCode::Down | KeyCode::Char('j') => app.chat_scroll_down(1),
        KeyCode::Up | KeyCode::Char('k') => app.chat_scroll_up(1),
        KeyCode::PageDown | KeyCode::Char(' ') => app.chat_scroll_down(12),
        KeyCode::PageUp => app.chat_scroll_up(12),
        KeyCode::Char('g') | KeyCode::Home => app.chat_scroll_top(),
        KeyCode::Char('G') | KeyCode::End => app.chat_scroll_bottom(),
        _ => {}
    }
}

fn handle_mail_key(app: &mut App, key: KeyEvent) {
    if key.code == KeyCode::Tab && app.mail_mode != MailMode::Compose {
        app.show_timeline_surface();
        return;
    }

    match app.mail_mode {
        MailMode::Index => handle_mail_index_key(app, key),
        MailMode::Pager => handle_mail_pager_key(app, key),
        MailMode::Compose => handle_mail_compose_key(app, key),
    }
}

fn handle_mail_index_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.show_timeline_surface(),
        KeyCode::Down | KeyCode::Char('j') => app.mail_cursor_down(),
        KeyCode::Up | KeyCode::Char('k') => app.mail_cursor_up(),
        KeyCode::Char('g') | KeyCode::Home => app.mail_cursor_top(),
        KeyCode::Char('G') | KeyCode::End => app.mail_cursor_bottom(),
        KeyCode::Enter | KeyCode::Char(' ') => app.open_selected_mail(),
        KeyCode::Char('r') => app.start_mail_reply(),
        KeyCode::Char('m') => app.start_mail_compose(),
        _ => {}
    }
}

fn handle_mail_pager_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('i') => app.close_mail_pager(),
        KeyCode::Down | KeyCode::Char('j') => app.mail_scroll_down(1),
        KeyCode::Up | KeyCode::Char('k') => app.mail_scroll_up(1),
        KeyCode::PageDown | KeyCode::Char(' ') => app.mail_scroll_down(12),
        KeyCode::PageUp => app.mail_scroll_up(12),
        KeyCode::Char('g') | KeyCode::Home => app.mail_scroll_top(),
        KeyCode::Char('G') | KeyCode::End => app.mail_scroll_bottom(),
        KeyCode::Char('r') => app.start_mail_reply(),
        _ => {}
    }
}

fn handle_mail_compose_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.abort_mail_compose(),
        KeyCode::Char('c') if has_control(key) => app.abort_mail_compose(),
        KeyCode::Char('y') if has_control(key) => send_mail_compose(app),
        KeyCode::Tab => app.advance_mail_compose_field(),
        KeyCode::BackTab => app.previous_mail_compose_field(),
        KeyCode::Enter => app.mail_compose_enter(),
        KeyCode::Backspace => app.mail_compose_backspace(),
        KeyCode::Char(ch) if !has_control(key) => app.mail_compose_push(ch),
        _ => {}
    }
}

fn send_mail_compose(app: &mut App) {
    if let Err(error) = app.send_mail_compose() {
        app.record_operator_action(format!("failed to send mail compose: {error}"));
    }
}

fn handle_input_key(app: &mut App, key: KeyEvent) -> LoopAction {
    match key.code {
        KeyCode::BackTab => app.toggle_input_mode(),
        KeyCode::Esc => app.mode = InputMode::Normal,
        KeyCode::Char('c') if has_control(key) => return LoopAction::Quit,
        KeyCode::Char('w') if has_control(key) => app.composer.delete_previous_word(),
        KeyCode::Tab | KeyCode::Char('f') | KeyCode::Char('F') => app.open_target_picker(),
        KeyCode::Char(c) if !has_control(key) => app.composer.insert_char(c),
        KeyCode::Backspace => app.composer.backspace(),
        KeyCode::Delete => app.composer.delete(),
        KeyCode::Left => app.composer.move_left(),
        KeyCode::Right => app.composer.move_right(),
        KeyCode::Home => app.composer.move_home(),
        KeyCode::End => app.composer.move_end(),
        KeyCode::Up => input_up(app),
        KeyCode::Down => input_down(app),
        KeyCode::Enter => submit_composer(app),
        KeyCode::PageUp => app.scroll_up(12),
        KeyCode::PageDown => app.scroll_down(12),
        _ => {}
    }

    LoopAction::Continue
}

fn toggle_pod_pause(app: &mut App) {
    if let Err(error) = app.toggle_pod_pause() {
        app.record_operator_action(format!("failed to toggle pod pause: {error}"));
    }
}

fn cycle_fin_filter(app: &mut App) {
    let mut fins: Vec<String> = app.known_fins.iter().cloned().collect();
    fins.sort();

    let next = match app.filters.fin_filter.as_deref() {
        Some(current) => match fins.iter().position(|fin| fin == current) {
            Some(pos) => fins.get(pos + 1).cloned(),
            None => fins.first().cloned(),
        },
        None => fins.first().cloned(),
    };

    app.set_fin_filter(next);
}

fn toggle_thread_query(app: &mut App) {
    if app.filters.thread_query.is_some() {
        app.set_thread_query(None);
    } else {
        app.set_thread_query(Some("auth".into()));
    }
}

fn input_up(app: &mut App) {
    if app.composer.input.is_empty() {
        app.scroll_up(1);
    } else {
        app.composer.history_prev();
    }
}

fn input_down(app: &mut App) {
    if app.composer.input.is_empty() {
        app.scroll_down(1);
    } else {
        app.composer.history_next();
    }
}

fn submit_composer(app: &mut App) {
    if let Some(msg) = app.composer.submit() {
        if let Err(error) = app.send_chat_prompt(&msg) {
            app.composer.set_status(format!("chat failed: {error}"));
            app.record_operator_action(format!("failed to chat with target fin: {error}"));
        }
        app.auto_follow_if_needed();
    }
}

fn has_control(key: KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
}
