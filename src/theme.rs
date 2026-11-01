//! Paleta sobria de bajo contraste.

use crate::config::{Config, ThemeName};
use ratatui::style::{Color, Style};

#[derive(Clone, Debug)]
pub struct Theme {
    pub fg: Color,
    pub dim: Color,
    pub accent: Color,
    pub target: Color,
    pub distractor: Color,
}

impl Theme {
    pub fn from_config(cfg: &Config) -> Theme {
        match cfg.theme {
            ThemeName::Dark => Theme {
                fg: Color::Rgb(150, 150, 150),
                dim: Color::Rgb(92, 92, 92),
                accent: Color::Rgb(190, 190, 190),
                target: parse_color(&cfg.target_color).unwrap_or(Color::Rgb(88, 150, 96)),
                distractor: parse_color(&cfg.distractor_color).unwrap_or(Color::Rgb(160, 82, 82)),
            },
            ThemeName::Mono => {
                let c = Color::Rgb(160, 160, 160);
                Theme { fg: c, dim: c, accent: c, target: c, distractor: c }
            }
        }
    }

    pub fn text(&self) -> Style {
        Style::default().fg(self.fg)
    }

    pub fn dim(&self) -> Style {
        Style::default().fg(self.dim)
    }

    pub fn accent(&self) -> Style {
        Style::default().fg(self.accent)
    }
}

/// Nombres apagados (no los colores ANSI brillantes) o `#rrggbb`.
pub fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim().to_ascii_lowercase();
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() != 6 {
            return None;
        }
        let v = u32::from_str_radix(hex, 16).ok()?;
        return Some(Color::Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8));
    }
    Some(match s.as_str() {
        "green" => Color::Rgb(88, 150, 96),
        "red" => Color::Rgb(160, 82, 82),
        "blue" => Color::Rgb(90, 118, 168),
        "yellow" => Color::Rgb(168, 152, 82),
        "cyan" => Color::Rgb(82, 148, 158),
        "magenta" => Color::Rgb(148, 96, 152),
        "gray" | "grey" => Color::Rgb(150, 150, 150),
        "white" => Color::Rgb(200, 200, 200),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_and_names() {
        assert_eq!(parse_color("#102030"), Some(Color::Rgb(16, 32, 48)));
        assert!(parse_color("Green").is_some());
        assert_eq!(parse_color("#12"), None);
        assert_eq!(parse_color("pink"), None);
    }
}
