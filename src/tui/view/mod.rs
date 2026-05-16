//! View components for the Operator Cockpit.

mod header;
mod input;
mod layout;
mod markdown;
mod overlays;
mod style;
mod timeline;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
};

use super::app::App;

impl App {
    /// Render the cockpit as four main sections: header, content, input, footer.
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let gap = u16::from(self.expanded);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(gap),
                Constraint::Length(1),
                Constraint::Length(gap),
                Constraint::Min(0),
                Constraint::Length(gap),
                Constraint::Length(3),
                Constraint::Length(gap),
                Constraint::Length(1),
                Constraint::Length(gap),
            ])
            .split(area);

        header::render(self, frame, layout::section_area(self, chunks[1]));
        timeline::render(self, frame, layout::section_area(self, chunks[3]));
        input::render(self, frame, layout::section_area(self, chunks[5]));
        input::render_footer(self, frame, layout::section_area(self, chunks[7]));

        if self.show_command_palette {
            overlays::render_command_palette(self, frame, area);
        }
        if self.show_target_picker {
            overlays::render_target_picker(self, frame, area);
        }
    }
}
