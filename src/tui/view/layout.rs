use ratatui::layout::Rect;

use crate::tui::app::App;

pub(super) fn section_area(app: &App, area: Rect) -> Rect {
    if !app.expanded || area.width <= 2 {
        return area;
    }

    Rect {
        x: area.x + 1,
        width: area.width - 2,
        ..area
    }
}

pub(super) fn centered_rect(area: Rect, max_width: u16, height: u16) -> Rect {
    let width = if area.width > 24 {
        max_width.min(area.width - 4).max(20)
    } else {
        area.width
    };
    let height = if area.height > 9 {
        height.min(area.height - 4).max(5)
    } else {
        area.height
    };

    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}
