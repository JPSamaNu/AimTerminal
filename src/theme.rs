//! Paletas y superficies del fondo. `matrix` y `cyber` usan el estilo de código:
//! pantallas con aspecto de salida de terminal y un fondo de logs falsos.

use crate::config::{Background, Config, ThemeName};
use ratatui::style::{Color, Style};

/// Colores del fondo pintado: degradado vertical, rejilla y paneles.
#[derive(Clone, Debug)]
pub struct Surface {
    pub top: (u8, u8, u8),
    pub bottom: (u8, u8, u8),
    pub grid: Option<Color>,
    pub panel: Color,
    pub select: Color,
}

impl Surface {
    /// Color de fondo de la fila `y` de un total de `h`.
    pub fn bg_at(&self, y: u16, h: u16) -> Color {
        let t = if h > 1 { y as f32 / (h - 1) as f32 } else { 0.0 };
        let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Color::Rgb(lerp(self.top.0, self.bottom.0), lerp(self.top.1, self.bottom.1), lerp(self.top.2, self.bottom.2))
    }
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub fg: Color,
    pub dim: Color,
    pub accent: Color,
    pub target: Color,
    pub distractor: Color,
    /// Barras de las gráficas.
    pub chart: Color,
    /// `None` deja el fondo de la terminal.
    pub surface: Option<Surface>,
    pub effects: bool,
    /// Estilo de código (camuflaje de terminal).
    pub code: bool,
    /// Color del texto de fondo en estilo de código; `None` sin texto de fondo.
    pub ambient: Option<Color>,
    /// Fondo de la fila seleccionada en estilo de código.
    pub code_select: Color,
}

impl Theme {
    pub fn from_config(cfg: &Config) -> Theme {
        if let Some(t) = Theme::code_theme(cfg) {
            return t;
        }
        let mono = cfg.theme == ThemeName::Mono;
        let surface = match cfg.background {
            Background::None => None,
            bg => {
                let grid = bg == Background::Grid;
                Some(if mono {
                    Surface {
                        top: (24, 24, 24),
                        bottom: (12, 12, 12),
                        grid: grid.then_some(Color::Rgb(46, 46, 46)),
                        panel: Color::Rgb(28, 28, 28),
                        select: Color::Rgb(44, 44, 44),
                    }
                } else {
                    Surface {
                        top: (21, 24, 31),
                        bottom: (10, 11, 15),
                        grid: grid.then_some(Color::Rgb(40, 45, 56)),
                        panel: Color::Rgb(25, 28, 36),
                        select: Color::Rgb(38, 44, 56),
                    }
                })
            }
        };
        if mono {
            let c = Color::Rgb(160, 160, 160);
            return Theme {
                fg: c,
                dim: c,
                accent: c,
                target: c,
                distractor: c,
                chart: c,
                surface,
                effects: cfg.effects,
                code: false,
                ambient: None,
                code_select: Color::Reset,
            };
        }
        Theme {
            fg: Color::Rgb(150, 152, 158),
            dim: Color::Rgb(92, 96, 106),
            accent: Color::Rgb(196, 198, 204),
            target: parse_color(&cfg.target_color).unwrap_or(Color::Rgb(88, 150, 96)),
            distractor: parse_color(&cfg.distractor_color).unwrap_or(Color::Rgb(160, 82, 82)),
            chart: Color::Rgb(96, 128, 150),
            surface,
            effects: cfg.effects,
            code: false,
            ambient: None,
            code_select: Color::Reset,
        }
    }

    /// Temas de camuflaje: usan el fondo de la propia terminal para mezclarse con ella.
    fn code_theme(cfg: &Config) -> Option<Theme> {
        let rgb = |r, g, b| Color::Rgb(r, g, b);
        let (fg, dim, accent, target, distractor, chart, ambient, select) = match cfg.theme {
            ThemeName::Matrix => (
                rgb(78, 168, 102),
                rgb(44, 102, 62),
                rgb(150, 226, 168),
                rgb(84, 214, 124),
                rgb(196, 128, 62),
                rgb(62, 146, 92),
                rgb(26, 60, 38),
                rgb(16, 42, 26),
            ),
            ThemeName::Cyber => (
                rgb(128, 156, 178),
                rgb(72, 88, 108),
                rgb(120, 214, 226),
                rgb(84, 204, 222),
                rgb(204, 86, 172),
                rgb(156, 98, 188),
                rgb(36, 42, 64),
                rgb(26, 28, 48),
            ),
            _ => return None,
        };
        Some(Theme {
            fg,
            dim,
            accent,
            target,
            distractor,
            chart,
            surface: None,
            effects: cfg.effects,
            code: true,
            ambient: (cfg.background == Background::Grid).then_some(ambient),
            code_select: select,
        })
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

    /// Fondo de los paneles (o el de la terminal si no se pinta fondo).
    pub fn panel_bg(&self) -> Color {
        self.surface.as_ref().map(|s| s.panel).unwrap_or(Color::Reset)
    }

    pub fn select_bg(&self) -> Color {
        if self.code {
            return self.code_select;
        }
        self.surface.as_ref().map(|s| s.select).unwrap_or(Color::Reset)
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

    #[test]
    fn gradient_goes_from_top_to_bottom() {
        let s = Surface { top: (20, 20, 20), bottom: (10, 0, 30), grid: None, panel: Color::Reset, select: Color::Reset };
        assert_eq!(s.bg_at(0, 11), Color::Rgb(20, 20, 20));
        assert_eq!(s.bg_at(10, 11), Color::Rgb(10, 0, 30));
        assert_eq!(s.bg_at(5, 11), Color::Rgb(15, 10, 25));
    }

    #[test]
    fn plain_flag_style_has_no_surface() {
        let dark = Config { theme: ThemeName::Dark, ..Config::default() };
        let cfg = Config { background: Background::None, ..dark.clone() };
        assert!(Theme::from_config(&cfg).surface.is_none());
        assert!(Theme::from_config(&dark).surface.unwrap().grid.is_some());
    }

    #[test]
    fn code_themes_blend_with_terminal() {
        for theme in [ThemeName::Matrix, ThemeName::Cyber] {
            let t = Theme::from_config(&Config { theme, ..Config::default() });
            assert!(t.code && t.surface.is_none() && t.ambient.is_some());
            assert_ne!(t.target, t.distractor);
        }
        let plain = Config { background: Background::None, effects: false, ..Config::default() };
        let t = Theme::from_config(&plain);
        assert!(t.code && t.ambient.is_none() && !t.effects);
    }
}
