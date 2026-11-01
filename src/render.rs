//! Dibujo de las pantallas y de los elementos del juego.

use crate::app::{App, MENU, PauseReason, ProgressView, Screen};
use crate::config::{Difficulty, Fill};
use crate::history::SessionRecord;
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

pub const MIN_W: u16 = 40;
pub const MIN_H: u16 = 12;

/// Zona de juego: toda la pantalla menos la línea de estado inferior.
pub fn play_area(area: Rect) -> Rect {
    Rect { height: area.height.saturating_sub(1), ..area }
}

fn status_area(area: Rect) -> Rect {
    Rect { y: area.y + area.height.saturating_sub(1), height: 1.min(area.height), ..area }
}

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    if area.width < MIN_W || area.height < MIN_H {
        too_small(f, app, area);
        return;
    }
    match app.screen {
        Screen::Menu => menu(f, app, area),
        Screen::Countdown { left, .. } => {
            centered(f, play_area(area), vec![Line::styled(left.to_string(), app.theme.dim())]);
            status(f, app, area);
        }
        Screen::Game => {
            if let Some(s) = &app.session {
                s.mode.render(f.buffer_mut(), play_area(area), &app.theme);
            }
            status(f, app, area);
        }
        Screen::Paused(reason) => paused(f, app, area, reason),
        Screen::Summary => summary(f, app, area),
        Screen::Progress => {
            if let Some(p) = &app.progress {
                progress(f, app, p, area);
            }
        }
    }
}

fn too_small(f: &mut Frame, app: &App, area: Rect) {
    let lines = vec![
        Line::styled(format!("{}x{}", area.width, area.height), app.theme.dim()),
        Line::styled(format!("mínimo {MIN_W}x{MIN_H}"), app.theme.dim()),
    ];
    centered(f, area, lines);
}

/// Bloque de líneas centrado vertical y horizontalmente.
pub fn centered(f: &mut Frame, area: Rect, lines: Vec<Line>) {
    let h = (lines.len() as u16).min(area.height);
    let w = lines.iter().map(|l| l.width() as u16).max().unwrap_or(0).min(area.width);
    let r = Rect { x: area.x + (area.width - w) / 2, y: area.y + (area.height - h) / 2, width: w, height: h };
    f.render_widget(Paragraph::new(lines), r);
}

fn menu(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let mut lines = vec![Line::styled("rt", t.accent()), Line::raw("")];
    for (i, item) in MENU.iter().enumerate() {
        let label = if i == 3 { format!("{item}: {}", app.cfg.difficulty.label()) } else { item.to_string() };
        let (mark, style) = if i == app.menu_idx { ("> ", t.accent()) } else { ("  ", t.dim()) };
        lines.push(Line::from(vec![Span::styled(mark, t.dim()), Span::styled(label, style)]));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled("↑↓ Enter · q salir", t.dim()));
    centered(f, area, lines);
}

fn status(f: &mut Frame, app: &App, area: Rect) {
    let Some(s) = &app.session else { return };
    let mut text = format!("{}/{}", s.records.len(), s.cfg.trials);
    if s.baseline {
        text.push_str("  base");
    }
    if let Some(r) = &s.last {
        text.push_str("  ");
        text.push_str(r.outcome.label());
        if let (Some(ms), false) = (r.rt_ms, r.anticipation) {
            text.push_str(&format!(" {ms:.0}ms"));
        }
    }
    let line = Line::from(vec![Span::styled(text, app.theme.dim())]);
    f.render_widget(Paragraph::new(line), status_area(area));
    let hint = "Esc pausa";
    let a = status_area(area);
    if a.width as usize > hint.len() + 30 {
        let r = Rect { x: a.x + a.width - hint.len() as u16, width: hint.len() as u16, ..a };
        f.render_widget(Paragraph::new(Line::styled(hint, app.theme.dim())), r);
    }
}

fn paused(f: &mut Frame, app: &App, area: Rect, reason: PauseReason) {
    let t = &app.theme;
    let why = match reason {
        PauseReason::User => "pausa",
        PauseReason::FocusLost => "pausa (sin foco)",
        PauseReason::TooSmall => "pausa (ventana pequeña)",
    };
    let lines = vec![
        Line::styled(why, t.accent()),
        Line::raw(""),
        Line::styled("Enter seguir", t.dim()),
        Line::styled("Esc terminar sesión", t.dim()),
        Line::styled("q salir", t.dim()),
    ];
    centered(f, play_area(area), lines);
    status(f, app, area);
}

fn ms(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.0} ms")).unwrap_or_else(|| "-".into())
}

fn pct(v: Option<f64>) -> String {
    v.map(|x| format!("{:.0}%", x * 100.0)).unwrap_or_else(|| "-".into())
}

/// Quita líneas vacías (y luego las últimas) si no cabe todo.
fn fit(mut lines: Vec<Line<'static>>, height: u16) -> Vec<Line<'static>> {
    while lines.len() > height as usize {
        match lines.iter().rposition(|l| l.width() == 0) {
            Some(i) => {
                lines.remove(i);
            }
            None => lines.truncate(height as usize),
        }
    }
    lines
}

fn summary(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let Some(s) = &app.session else { return };
    let Some(st) = &s.stats else { return };
    let mut head = format!("{} intentos · {}", st.trials, s.cfg.difficulty.label());
    if s.baseline {
        head = format!("baseline · {head}");
    }
    let mut lines = vec![Line::styled(head, t.accent())];
    if let (Difficulty::Adaptive, Some(l)) = (s.cfg.difficulty, s.final_level()) {
        lines.push(Line::styled(format!("nivel final {l}"), t.dim()));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled(format!("mediana  {:>7}   media     {:>6}", ms(st.median_ms), ms(st.mean_ms)), t.text()));
    lines.push(Line::styled(format!("mejor    {:>7}   precisión {:>6}", ms(st.best_ms), pct(st.accuracy)), t.text()));
    lines.push(Line::styled(
        format!("errores {}  omisiones {}  f.alarmas {}", st.errors, st.omissions, st.false_alarms),
        t.text(),
    ));
    lines.push(Line::styled(
        format!("rech.corr. {}/{}  anticip. {}  desc. {}", st.correct_rejections, st.catch_trials, st.anticipations, s.discarded),
        t.text(),
    ));
    lines.push(Line::styled(format!("puntaje  {:>7}   (menor es mejor)", ms(st.ies_ms)), t.accent()));
    lines.push(Line::raw(""));
    lines.push(Line::styled("los tiempos incluyen la latencia de la", t.dim()));
    lines.push(Line::styled("terminal: compáralos solo contigo.", t.dim()));
    if let Some(n) = &app.notice {
        lines.push(Line::styled(n.clone(), t.accent()));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled("Enter otra · p progreso · Esc menú", t.dim()));
    centered(f, area, fit(lines, area.height));
}

const SPARK: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Gráfica de una línea: cada carácter es una sesión, más alto = valor mayor.
pub fn sparkline(values: &[Option<f64>]) -> String {
    let present: Vec<f64> = values.iter().flatten().copied().collect();
    let (Some(lo), Some(hi)) = (present.iter().copied().reduce(f64::min), present.iter().copied().reduce(f64::max))
    else {
        return " ".repeat(values.len());
    };
    values
        .iter()
        .map(|v| match v {
            None => ' ',
            Some(_) if (hi - lo).abs() < f64::EPSILON => SPARK[3],
            Some(x) => SPARK[(((x - lo) / (hi - lo)) * 7.0).round() as usize],
        })
        .collect()
}

fn short_date(d: &str) -> String {
    d.get(5..16).map(|x| x.replace('T', " ")).unwrap_or_else(|| d.to_string())
}

fn progress(f: &mut Frame, app: &App, p: &ProgressView, area: Rect) {
    let t = &app.theme;
    let list: Vec<&SessionRecord> = p.sessions.iter().filter(|s| !s.incomplete && s.baseline == p.baselines).collect();
    let title = if p.baselines { "progreso · baselines" } else { "progreso · sesiones" };
    let mut lines = vec![Line::from(vec![
        Span::styled(title, t.accent()),
        Span::styled(format!("  ({})  Tab cambia", list.len()), t.dim()),
    ])];
    lines.push(Line::raw(""));
    let footer = if p.bad_lines > 0 {
        format!("{} líneas dañadas ignoradas · Esc menú", p.bad_lines)
    } else {
        "menor es mejor salvo precisión · Esc".to_string()
    };
    if list.is_empty() {
        lines.push(Line::styled("sin sesiones completas todavía", t.dim()));
    } else {
        let spark_w = area.width.saturating_sub(11 + 8 + 2).clamp(4, 60) as usize;
        let recent = &list[list.len().saturating_sub(spark_w)..];
        let last = list[list.len() - 1];
        let rows: [(&str, Vec<Option<f64>>, String); 3] = [
            ("mediana", recent.iter().map(|s| s.stats.median_ms).collect(), ms(last.stats.median_ms)),
            ("precisión", recent.iter().map(|s| s.stats.accuracy).collect(), pct(last.stats.accuracy)),
            ("puntaje", recent.iter().map(|s| s.stats.ies_ms).collect(), ms(last.stats.ies_ms)),
        ];
        for (label, vals, now) in rows {
            lines.push(Line::from(vec![
                Span::styled(format!("{label:<10} "), t.dim()),
                Span::styled(format!("{:<w$}", sparkline(&vals), w = spark_w), t.text()),
                Span::styled(format!("{now:>8}"), t.accent()),
            ]));
        }
        lines.push(Line::raw(""));
        lines.push(Line::styled(format!("{:<11} {:>7} {:>5} {:>8}", "fecha UTC", "mediana", "prec", "puntaje"), t.dim()));
        let room = (area.height as usize).saturating_sub(lines.len() + 2);
        for s in list.iter().rev().take(room) {
            lines.push(Line::styled(
                format!(
                    "{:<11} {:>7} {:>5} {:>8}",
                    short_date(&s.date),
                    ms(s.stats.median_ms),
                    pct(s.stats.accuracy),
                    ms(s.stats.ies_ms)
                ),
                t.text(),
            ));
        }
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled(footer, t.dim()));
    let lines = fit(lines, area.height);
    let w = lines.iter().map(|l| l.width() as u16).max().unwrap_or(0).min(area.width);
    let r = Rect { x: area.x + (area.width - w) / 2, y: area.y, width: w, height: area.height };
    f.render_widget(Paragraph::new(lines), r);
}

/// Punto de fijación en el centro.
pub fn fixation(buf: &mut Buffer, area: Rect, theme: &Theme) {
    let x = area.x + area.width / 2;
    let y = area.y + area.height / 2;
    if let Some(c) = buf.cell_mut((x, y)) {
        c.set_char('•').set_style(theme.text());
    }
}

/// Recuadro relleno o hueco hecho con caracteres de bloque, con su número al centro.
pub fn draw_box(buf: &mut Buffer, r: Rect, fill: Fill, color: Color, label: char) {
    let r = r.intersection(buf.area);
    if r.width == 0 || r.height == 0 {
        return;
    }
    let style = Style::default().fg(color);
    let (right, bottom) = (r.x + r.width - 1, r.y + r.height - 1);
    for y in r.y..=bottom {
        for x in r.x..=right {
            let edge_x = x == r.x || x == right;
            let ch = match fill {
                Fill::Filled => '█',
                Fill::Hollow if edge_x => '█',
                Fill::Hollow if y == r.y => '▀',
                Fill::Hollow if y == bottom => '▄',
                Fill::Hollow => ' ',
            };
            if let Some(c) = buf.cell_mut((x, y)) {
                c.set_char(ch).set_style(style);
            }
        }
    }
    let (lx, ly) = (r.x + r.width / 2, r.y + r.height / 2);
    if let Some(c) = buf.cell_mut((lx, ly)) {
        let st = match fill {
            // Número en negativo sobre el bloque relleno.
            Fill::Filled => Style::default().fg(Color::Rgb(16, 16, 16)).bg(color),
            Fill::Hollow => style,
        };
        c.set_char(label).set_style(st);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::app::App;
    use crate::config::Config;
    use crate::modes::{Method, Outcome, TrialRecord};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::time::Instant;

    fn screen_text(app: &App, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| draw(f, app)).unwrap();
        let buf = t.backend().buffer();
        (0..h)
            .map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>())
            .collect::<Vec<_>>()
            .join("
")
    }

    fn trial(outcome: Outcome, rt: Option<f64>) -> TrialRecord {
        TrialRecord {
            index: 0,
            level: Some(1),
            n_boxes: 3,
            target_present: outcome != Outcome::CorrectRejection,
            target_w: 6,
            target_h: 3,
            cursor_dist: None,
            rt_ms: rt,
            outcome,
            method: Method::Mouse,
            anticipation: false,
        }
    }

    #[test]
    fn summary_fits_minimum_panel() {
        let mut app = App::new(Config::default(), Rect::new(0, 0, 40, 12));
        app.start_session(false, Instant::now());
        let s = app.session.as_mut().unwrap();
        s.records = vec![trial(Outcome::Hit, Some(400.0)), trial(Outcome::Error, Some(300.0))];
        s.stats = Some(crate::stats::compute(&s.records));
        app.screen = Screen::Summary;
        let text = screen_text(&app, 40, 12);
        assert!(text.contains("mediana"), "{text}");
        assert!(text.contains("puntaje"), "{text}");
        assert!(text.contains("latencia"), "{text}");
        assert!(text.contains("Esc"), "{text}");
    }

    #[test]
    fn progress_separates_baselines_and_skips_incomplete() {
        let mut app = App::new(Config::default(), Rect::new(0, 0, 40, 12));
        let mk = |baseline, incomplete, med| crate::history::SessionRecord {
            version: 1,
            date: "2026-11-01T10:00:00Z".into(),
            mode: "decision".into(),
            difficulty: crate::config::Difficulty::Fixed,
            baseline,
            incomplete,
            discarded: 0,
            final_level: None,
            pixel_mouse: false,
            config: Config::default(),
            stats: crate::stats::Stats { median_ms: Some(med), accuracy: Some(1.0), ies_ms: Some(med), ..Default::default() },
            trials: vec![],
        };
        app.progress = Some(ProgressView {
            sessions: vec![mk(false, false, 410.0), mk(false, true, 999.0), mk(true, false, 520.0)],
            bad_lines: 0,
            baselines: false,
        });
        app.screen = Screen::Progress;
        let text = screen_text(&app, 40, 12);
        assert!(text.contains("(1)") && text.contains("410 ms"), "{text}");
        assert!(!text.contains("999") && !text.contains("520"), "{text}");
        app.progress.as_mut().unwrap().baselines = true;
        let text = screen_text(&app, 40, 12);
        assert!(text.contains("baselines") && text.contains("520 ms"), "{text}");
    }

    #[test]
    fn sparkline_scales() {
        assert_eq!(sparkline(&[Some(1.0), Some(2.0), None, Some(3.0)]), "▁▅ █");
        assert_eq!(sparkline(&[Some(5.0), Some(5.0)]), "▄▄");
        assert_eq!(sparkline(&[None, None]), "  ");
    }

    #[test]
    fn hollow_box_shape() {
        let area = Rect::new(0, 0, 5, 3);
        let mut buf = Buffer::empty(area);
        draw_box(&mut buf, area, Fill::Hollow, Color::Red, '2');
        let rows: Vec<String> =
            (0..3).map(|y| (0..5).map(|x| buf[(x, y)].symbol().to_string()).collect()).collect();
        assert_eq!(rows, vec!["█▀▀▀█", "█ 2 █", "█▄▄▄█"]);
    }
}
