use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::tui::{
    app::{App, InputMode},
    view::style::{bordered_block, fg},
};

/// Three-row input section: one text row plus a border around all sides.
pub(super) fn render(app: &App, frame: &mut Frame, area: Rect) {
    let block = bordered_block(app.theme.muted);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    render_label(app, frame, area);

    if app.mode == InputMode::Normal {
        let text = Line::from(vec![
            Span::styled(" >", fg(app.theme.accent)),
            Span::styled(" press i to chat with the target fin", fg(app.theme.muted)),
        ]);
        frame.render_widget(Paragraph::new(text), inner);
    } else {
        app.composer.render(frame, inner, &app.pod_slug, &app.theme);
    }
}

pub(super) fn render_footer(app: &App, frame: &mut Frame, area: Rect) {
    let help = "c:chat  |  m:mail  |  Shift+Tab:mode  |  Ctrl+T:target  |  Ctrl+.:commands";

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {help}"),
            fg(app.theme.muted),
        ))),
        area,
    );
}

fn render_label(app: &App, frame: &mut Frame, area: Rect) {
    if area.width < 10 || area.height < 3 {
        return;
    }

    let mode = match app.mode {
        InputMode::Normal => "normal",
        InputMode::Chat => "chat",
    };
    let label = format!(" @{} · {} ", app.composer.target_fin, mode);
    let width = label.chars().count() as u16;
    if width + 2 >= area.width {
        return;
    }

    let label_area = Rect {
        x: area.x + area.width - width - 2,
        y: area.y + area.height - 1,
        width,
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(label, fg(app.theme.muted)))),
        label_area,
    );
}
