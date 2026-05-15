//! TUI color themes.

use ratatui::style::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub name: &'static str,
    pub mode: ThemeMode,
    pub bar_bg: Color,
    pub header_bg: Color,
    pub panel_bg: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub ok: Color,
    pub warn: Color,
    pub error: Color,
    pub mail: Color,
    pub event: Color,
    pub stdout: Color,
    pub cursor: Color,
}

pub const OPERATOR_DARK: Theme = Theme {
    name: "operator-dark",
    mode: ThemeMode::Dark,
    bar_bg: Color::Rgb(0x16, 0x1A, 0x1F),
    header_bg: Color::Rgb(0x20, 0x35, 0x38),
    panel_bg: Color::Rgb(0x0F, 0x12, 0x16),
    text: Color::Rgb(0xE6, 0xE6, 0xE6),
    muted: Color::Rgb(0x8B, 0x94, 0x9E),
    accent: Color::Rgb(0x67, 0xE8, 0xF9),
    ok: Color::Rgb(0x7B, 0xD8, 0x8F),
    warn: Color::Rgb(0xF4, 0xA2, 0x61),
    error: Color::Rgb(0xEF, 0x44, 0x44),
    mail: Color::Rgb(0xD8, 0xA7, 0xFF),
    event: Color::Rgb(0x7D, 0xD3, 0xFC),
    stdout: Color::Rgb(0xC7, 0xD0, 0xD9),
    cursor: Color::Rgb(0xFD, 0xD6, 0x6A),
};

pub const PAPER_LIGHT: Theme = Theme {
    name: "paper-light",
    mode: ThemeMode::Light,
    bar_bg: Color::Rgb(0xEA, 0xEC, 0xEF),
    header_bg: Color::Rgb(0xD8, 0xE8, 0xE6),
    panel_bg: Color::Rgb(0xFB, 0xFC, 0xFD),
    text: Color::Rgb(0x1E, 0x24, 0x2A),
    muted: Color::Rgb(0x67, 0x70, 0x7A),
    accent: Color::Rgb(0x06, 0x72, 0x8A),
    ok: Color::Rgb(0x1D, 0x7A, 0x42),
    warn: Color::Rgb(0xA8, 0x5E, 0x12),
    error: Color::Rgb(0xB4, 0x23, 0x18),
    mail: Color::Rgb(0x7A, 0x3E, 0x9D),
    event: Color::Rgb(0x1C, 0x63, 0x9A),
    stdout: Color::Rgb(0x37, 0x42, 0x4D),
    cursor: Color::Rgb(0xB4, 0x53, 0x09),
};

pub const THEMES: &[Theme] = &[OPERATOR_DARK, PAPER_LIGHT];

pub fn default_theme() -> Theme {
    match system_theme_mode() {
        Some(ThemeMode::Light) => PAPER_LIGHT,
        Some(ThemeMode::Dark) | None => OPERATOR_DARK,
    }
}

#[cfg(target_os = "macos")]
fn system_theme_mode() -> Option<ThemeMode> {
    let output = std::process::Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output()
        .ok()?;

    if output.status.success() {
        let value = String::from_utf8_lossy(&output.stdout);
        if value.trim().eq_ignore_ascii_case("dark") {
            return Some(ThemeMode::Dark);
        }
    }

    Some(ThemeMode::Light)
}

#[cfg(not(target_os = "macos"))]
fn system_theme_mode() -> Option<ThemeMode> {
    None
}
