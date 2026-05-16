use ratatui::{
    style::{Color, Modifier, Style},
    widgets::{Block, Borders},
};

pub(super) fn fg(color: Color) -> Style {
    Style::default().fg(color)
}

pub(super) fn bold(color: Color) -> Style {
    fg(color).add_modifier(Modifier::BOLD)
}

pub(super) fn strong() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

pub(super) fn bordered_block(color: Color) -> Block<'static> {
    Block::default().borders(Borders::ALL).style(fg(color))
}

pub(super) fn titled_block(title: &'static str, color: Color) -> Block<'static> {
    bordered_block(color).title(title)
}
