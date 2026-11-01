//! Dibujo de las pantallas y de los elementos del juego.

use crate::aim;
use crate::app::{self, App, EFFECT, MENU, MenuItem, PROGRESS_VIEWS, PauseReason, Screen, Session};
use crate::report::{self, Metric};
use crate::config::{Fill, GameKind};
use crate::modes::{FlashKind, Outcome};
use crate::stats;
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use std::time::Instant;

pub const MIN_W: u16 = 40;
pub const MIN_H: u16 = 12;

/// Zona de juego: toda la pantalla menos la línea de estado inferior.
pub fn play_area(area: Rect) -> Rect {
    Rect { height: area.height.saturating_sub(1), ..area }
}

pub(crate) fn status_area(area: Rect) -> Rect {
    Rect { y: area.y + area.height.saturating_sub(1), height: 1.min(area.height), ..area }
}

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    paint_background(f.buffer_mut(), area, &app.theme);
    if area.width < MIN_W || area.height < MIN_H {
        too_small(f, app, area);
        return;
    }
    if app.theme.code {
        crate::render_code::draw(f, app, area);
        return;
    }
    match app.screen {
        Screen::Menu => menu(f, app, area),
        Screen::Countdown { left, .. } => countdown(f, app, area, left),
        Screen::Game => {
            if let Some(s) = &app.session {
                s.mode.render(f.buffer_mut(), play_area(area), &app.theme);
            }
            draw_effects(f.buffer_mut(), play_area(area), app, Instant::now());
            status(f, app, area);
        }
        Screen::Paused(reason) => paused(f, app, area, reason),
        Screen::Summary => summary(f, app, area),
        Screen::Progress => progress(f, app, area),
        Screen::Aim => aim_screen(f, app, area),
        Screen::Plan => plan_screen(f, app, area),
        Screen::PlanDone => plan_done(f, app, area),
    }
}

/// Minutos aproximados de un plan, redondeando hacia arriba.
pub(crate) fn plan_minutes(blocks: &[crate::plan::Block]) -> u64 {
    blocks.iter().map(|b| b.est_secs()).sum::<u64>().div_ceil(60)
}

fn plan_screen(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let Some(p) = &app.plan else { return };
    let mut body: Vec<PLine> = Vec::new();
    for (i, b) in p.blocks.iter().enumerate() {
        let head = format!("{}  {} · {}", i + 1, b.kind().label(), b.size_label());
        let level = b.cfg.difficulty_label().map(|l| format!("  {l}")).unwrap_or_default();
        body.push((Line::from(vec![Span::styled(head, t.text()), Span::styled(level, t.dim())]), 0));
        body.push((Line::styled(format!("   {}", b.reason), t.dim()), 1));
    }
    body.push((Line::raw(""), 3));
    body.push((Line::styled(format!("unos {} min · cada bloque se guarda", plan_minutes(&p.blocks)), t.dim()), 2));
    panel(f, area, t, "plan de hoy", "Enter empezar · Esc", body);
}

fn plan_done(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let Some(p) = &app.plan else { return };
    let mut body: Vec<PLine> = Vec::new();
    for (mode, name, value) in &p.results {
        body.push((row(mode, &format!("{name} {value}"), 30, t.text(), t.accent()), 0));
    }
    body.push((Line::raw(""), 3));
    body.push((Line::styled("el detalle de cada bloque está en el progreso", t.dim()), 2));
    panel(f, area, t, "plan completo", "Enter menú", body);
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

/// Línea con prioridad: 0 siempre se muestra; las de número mayor se quitan primero
/// cuando no hay espacio.
pub(crate) type PLine = (Line<'static>, u8);

/// Deja como mucho `height` líneas quitando primero las de mayor prioridad.
pub(crate) fn fit(mut lines: Vec<PLine>, height: usize) -> Vec<Line<'static>> {
    while lines.len() > height {
        let worst = lines.iter().map(|(_, p)| *p).max().unwrap_or(0);
        if worst == 0 {
            lines.truncate(height);
            break;
        }
        let i = lines.iter().rposition(|(_, p)| *p == worst).unwrap_or(lines.len() - 1);
        lines.remove(i);
    }
    lines.into_iter().map(|(l, _)| l).collect()
}

/// Panel centrado con marco fino, título arriba y ayuda abajo. Si no cabe con marco,
/// se dibuja sin él y las líneas opcionales se quitan.
fn panel(f: &mut Frame, area: Rect, t: &Theme, title: &str, footer: &str, body: Vec<PLine>) {
    let avail = area.height.saturating_sub(4) as usize;
    let required = body.iter().filter(|(_, p)| *p == 0).count();
    if required <= avail {
        let lines = fit(body.clone(), avail);
        let body_w = lines.iter().map(|l| l.width()).max().unwrap_or(0);
        let w = body_w.max(title.chars().count() + 4).max(footer.chars().count() + 4) as u16;
        let (framed_w, framed_h) = (w + 6, lines.len() as u16 + 4);
        if framed_w <= area.width {
            let r = Rect {
                x: area.x + (area.width - framed_w) / 2,
                y: area.y + (area.height - framed_h) / 2,
                width: framed_w,
                height: framed_h,
            };
            let block = frame_block(t, title, footer);
            let inner = block.inner(r);
            f.render_widget(Clear, r);
            f.render_widget(block, r);
            let content =
                Rect { x: inner.x + 2, y: inner.y + 1, width: inner.width.saturating_sub(4), height: inner.height - 1 };
            f.render_widget(Paragraph::new(lines), content);
            return;
        }
    }
    let mut lines: Vec<PLine> = vec![(Line::styled(title.to_string(), t.accent()), 0), (Line::raw(""), 3)];
    lines.extend(body);
    lines.push((Line::raw(""), 3));
    lines.push((Line::styled(footer.to_string(), t.dim()), 0));
    let lines = fit(lines, area.height as usize);
    let width = lines.iter().map(|l| l.width() as u16).max().unwrap_or(0).min(area.width);
    let h = lines.len() as u16;
    let r = Rect { x: area.x + (area.width - width) / 2, y: area.y + (area.height - h) / 2, width, height: h };
    f.render_widget(Paragraph::new(lines), r);
}

/// Marco redondeado con fondo de panel, título arriba y ayuda abajo.
fn frame_block<'a>(t: &Theme, title: &str, footer: &str) -> Block<'a> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(t.panel_bg()))
        .border_style(t.dim())
        .title(Line::styled(format!(" {title} "), t.accent()))
        .title_bottom(Line::styled(format!(" {footer} "), t.dim()))
}

/// Degradado vertical oscuro con una rejilla de puntos muy tenue.
fn paint_background(buf: &mut Buffer, area: Rect, t: &Theme) {
    let Some(s) = &t.surface else { return };
    for y in area.top()..area.bottom() {
        let bg = s.bg_at(y - area.y, area.height);
        for x in area.left()..area.right() {
            let cell = &mut buf[(x, y)];
            cell.set_bg(bg);
            if let Some(g) = s.grid
                && (x - area.x) % 6 == 3
                && (y - area.y) % 3 == 1
            {
                cell.set_char('·').set_fg(g);
            }
        }
    }
}

/// Oscurece un color RGB (k = 1 lo deja igual).
pub(crate) fn fade(c: Color, k: f32) -> Color {
    let k = k.clamp(0.0, 1.0);
    match c {
        Color::Rgb(r, g, b) => Color::Rgb((r as f32 * k) as u8, (g as f32 * k) as u8, (b as f32 * k) as u8),
        other => other,
    }
}

fn put(buf: &mut Buffer, area: Rect, x: f32, y: f32, ch: char, color: Color) {
    if x < area.x as f32 || y < area.y as f32 {
        return;
    }
    let (x, y) = (x as u16, y as u16);
    if x < area.right() && y < area.bottom() {
        buf[(x, y)].set_char(ch).set_fg(color);
    }
}

const RAYS: [(f32, f32); 8] =
    [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0), (0.7, 0.7), (-0.7, 0.7), (0.7, -0.7), (-0.7, -0.7)];

/// Efectos breves: acierto (anillo y destello de puntos), error (sombreado),
/// objetivo no elegido (contorno tenue) y clic en el vacío (aspa).
pub(crate) fn draw_effects(buf: &mut Buffer, area: Rect, app: &App, now: Instant) {
    let t = &app.theme;
    let code = t.code;
    for (fl, start) in &app.effects {
        let p = now.saturating_duration_since(*start).as_secs_f32() / EFFECT.as_secs_f32();
        if p >= 1.0 {
            continue;
        }
        let r = fl.rect;
        let (cx, cy) = (r.x as f32 + r.width as f32 / 2.0, r.y as f32 + r.height as f32 / 2.0);
        match fl.kind {
            FlashKind::Hit => {
                let col = fade(t.target, 1.0 - 0.6 * p);
                if p < 0.45 {
                    draw_box_themed(buf, r, Fill::Hollow, col, None, t);
                }
                let (rx, ry) = (r.width as f32 / 2.0 + 1.0 + p * 5.0, r.height as f32 / 2.0 + 0.5 + p * 2.5);
                let ch = match (code, p < 0.5) {
                    (true, true) => '1',
                    (true, false) => '0',
                    (false, true) => '•',
                    (false, false) => '·',
                };
                for (dx, dy) in RAYS {
                    put(buf, area, cx + dx * rx, cy + dy * ry, ch, col);
                }
            }
            FlashKind::Wrong => {
                let ch = match (code, p < 0.5) {
                    (true, true) => '#',
                    (true, false) => ':',
                    (false, true) => '▒',
                    (false, false) => '░',
                };
                let col = fade(t.distractor, 1.0 - 0.5 * p);
                for y in r.top()..r.bottom() {
                    for x in r.left()..r.right() {
                        put(buf, area, x as f32, y as f32, ch, col);
                    }
                }
            }
            FlashKind::Reveal => draw_box_themed(buf, r, Fill::Hollow, fade(t.target, 0.6 - 0.3 * p), None, t),
            FlashKind::Miss => {
                let col = fade(t.distractor, 1.0 - 0.6 * p);
                let ch = match (code, p < 0.5) {
                    (true, true) => 'x',
                    (false, true) => '×',
                    (true, false) => '.',
                    (false, false) => '·',
                };
                put(buf, area, r.x as f32, r.y as f32, ch, col);
                if p > 0.2 {
                    let d = 1.0 + p * 2.0;
                    for (dx, dy) in &RAYS[4..] {
                        let dot = if code { '.' } else { '·' };
                        put(buf, area, r.x as f32 + dx * d * 2.0, r.y as f32 + dy * d, dot, col);
                    }
                }
            }
        }
    }
}

/// Fuente de 3x5 puntos para cifras grandes, dibujada con medios bloques.
fn glyph(c: char) -> [u8; 5] {
    match c {
        '0' => [7, 5, 5, 5, 7],
        '1' => [2, 6, 2, 2, 7],
        '2' => [7, 1, 7, 4, 7],
        '3' => [7, 1, 7, 1, 7],
        '4' => [5, 5, 7, 1, 1],
        '5' => [7, 4, 7, 1, 7],
        '6' => [7, 4, 7, 5, 7],
        '7' => [7, 1, 1, 1, 1],
        '8' => [7, 5, 7, 5, 7],
        '9' => [7, 5, 7, 1, 7],
        'R' => [6, 5, 6, 5, 5],
        'T' => [7, 2, 2, 2, 2],
        '-' => [0, 0, 7, 0, 0],
        _ => [0; 5],
    }
}

/// Texto en cifras grandes: 3 filas de alto, 4 columnas por carácter.
pub fn big(text: &str) -> [String; 3] {
    let mut rows: [String; 3] = Default::default();
    for (i, c) in text.chars().enumerate() {
        let g = glyph(c);
        for (r, row) in rows.iter_mut().enumerate() {
            if i > 0 {
                row.push(' ');
            }
            let top = g[r * 2];
            let bottom = if r * 2 + 1 < 5 { g[r * 2 + 1] } else { 0 };
            for bit in [4u8, 2, 1] {
                row.push(match (top & bit != 0, bottom & bit != 0) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (false, false) => ' ',
                });
            }
        }
    }
    rows
}

/// Etiqueta a la izquierda y valor alineado a la derecha en un ancho fijo.
pub(crate) fn row(label: &str, value: &str, width: usize, ls: Style, vs: Style) -> Line<'static> {
    let gap = width.saturating_sub(label.chars().count() + value.chars().count()).max(1);
    Line::from(vec![Span::styled(label.to_string(), ls), Span::raw(" ".repeat(gap)), Span::styled(value.to_string(), vs)])
}

fn menu(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    const W: usize = 28;
    let mut body: Vec<PLine> = Vec::new();
    // El logo solo si cabe entero.
    if area.height >= 18 {
        for l in big("RT") {
            body.push((Line::styled(format!("  {l}"), Style::default().fg(t.target)), 0));
        }
        body.push((Line::raw(""), 3));
    }
    for (i, item) in MENU.iter().enumerate() {
        let selected = i == app.menu_idx;
        let (label, value) = match item {
            MenuItem::Start => ("empezar", String::new()),
            MenuItem::Mode => ("modo", app.cfg.mode.label().to_string()),
            MenuItem::Difficulty => ("dificultad", app.cfg.difficulty_label().unwrap_or_else(|| "—".into())),
            MenuItem::Baseline => ("baseline", app.cfg.baseline_mode().label().to_string()),
            MenuItem::Plan => ("plan de hoy", String::new()),
            MenuItem::Progress => ("progreso", String::new()),
            MenuItem::Aim => ("puntería", String::new()),
            MenuItem::Quit => ("salir", String::new()),
        };
        let (mark, ls) = if selected { ("› ", t.accent()) } else { ("  ", t.dim()) };
        let value = if selected && !value.is_empty() && value != "—" { format!("‹ {value} › ") } else { format!("{value} ") };
        let mut line = row(&format!("{mark}{label}"), &value, W, ls, if selected { t.accent() } else { t.text() });
        if selected {
            line = line.style(Style::default().bg(t.select_bg()));
        }
        body.push((line, 0));
    }
    let last = app::last_comparable(&app.history, &app.cfg, false);
    if let Some(s) = last {
        body.push((Line::raw(""), 2));
        body.push((row("  último puntaje", &format!("{} ", ms(s.stats.ies_ms)), W, t.dim(), t.text()), 1));
    }
    panel(f, area, t, "rt", "↑↓ Enter · ←→ cambia · q", body);
}

pub(crate) fn mode_hint(kind: GameKind) -> &'static str {
    report::hint(kind)
}

fn countdown(f: &mut Frame, app: &App, area: Rect, left: u8) {
    let t = &app.theme;
    let mut lines: Vec<Line> = big(&left.to_string()).into_iter().map(|l| Line::styled(l, t.accent())).collect();
    lines.push(Line::raw(""));
    if let Some(s) = &app.session {
        lines.push(Line::styled(s.cfg.mode.label(), t.text()));
        lines.push(Line::styled(mode_hint(s.cfg.mode), t.dim()));
    }
    let lines: Vec<Line> = lines.into_iter().map(|l| l.centered()).collect();
    let h = lines.len() as u16;
    let a = play_area(area);
    let r = Rect { y: a.y + a.height.saturating_sub(h) / 2, height: h.min(a.height), ..a };
    f.render_widget(Paragraph::new(lines), r);
    status(f, app, area);
}

/// Texto de avance de la sesión: tiempo restante o intentos hechos.
pub(crate) fn progress_text(s: &Session) -> String {
    if let Some((done, total)) = s.warming_up() {
        return format!("calentamiento {done}/{total} · no cuenta");
    }
    match (s.timer, s.cfg.mode) {
        (Some(timer), GameKind::Tracking) => {
            let st = stats::compute(&s.records);
            format!("{:>2} s  encima {}", timer.shown_secs(std::time::Instant::now()), pct(st.on_target))
        }
        (Some(timer), _) => format!(
            "{:>2} s  {} aciertos  {} fallos",
            timer.shown_secs(std::time::Instant::now()),
            s.count(Outcome::Hit),
            s.count(Outcome::Miss) + s.count(Outcome::Anticipation)
        ),
        (None, _) => format!("{}/{}", s.counted, s.cfg.session_trials()),
    }
}

fn status(f: &mut Frame, app: &App, area: Rect) {
    let Some(s) = &app.session else { return };
    let mut text = progress_text(s);
    if s.baseline {
        text.push_str("  base");
    }
    if let Some(l) = s.level() {
        text.push_str(&format!("  niv {l}"));
    }
    if let (Some(r), false) = (&s.last, s.cfg.mode == GameKind::Tracking) {
        text.push_str("  · ");
        text.push_str(r.outcome.label());
        if let (Some(ms), false) = (r.rt_ms, r.anticipation) {
            text.push_str(&format!(" {ms:.0}ms"));
        }
    }
    let a = status_area(area);
    f.render_widget(Clear, a);
    f.render_widget(Block::default().style(Style::default().bg(app.theme.panel_bg())), a);
    f.render_widget(Paragraph::new(Line::styled(format!(" {text}"), app.theme.dim())), a);
    let hint = "Esc pausa";
    if a.width as usize > hint.len() + 44 {
        let r = Rect { x: a.x + a.width - hint.len() as u16 - 1, width: hint.len() as u16, ..a };
        f.render_widget(Paragraph::new(Line::styled(hint, app.theme.dim())), r);
    }
}

fn paused(f: &mut Frame, app: &App, area: Rect, reason: PauseReason) {
    let t = &app.theme;
    let why = match reason {
        PauseReason::User => "pausa",
        PauseReason::FocusLost => "pausa · sin foco",
        PauseReason::TooSmall => "pausa · ventana pequeña",
    };
    let body: Vec<PLine> = vec![
        (row("seguir", "Enter", 24, t.text(), t.dim()), 0),
        (row("terminar sesión", "Esc", 24, t.text(), t.dim()), 0),
        (row("salir", "q", 24, t.text(), t.dim()), 0),
        (Line::raw(""), 2),
        (Line::styled("el intento en curso no cuenta", t.dim()), 1),
    ];
    panel(f, play_area(area), t, why, "Enter · Esc · q", body);
    status(f, app, area);
}

pub(crate) fn ms(v: Option<f64>) -> String {
    report::ms(v)
}

pub(crate) fn pct(v: Option<f64>) -> String {
    report::pct(v)
}

/// Diferencia frente a la sesión anterior, en verde si mejora y en rojo si empeora.
pub(crate) fn delta(t: &Theme, m: &Metric) -> Span<'static> {
    let (Some(a), Some(b)) = (m.now, m.before) else { return Span::raw("") };
    let d = (a - b) * m.scale;
    let tiny = 0.5 / 10f64.powi(m.decimals as i32);
    if d.abs() < tiny {
        return Span::styled("  =", t.dim());
    }
    let better = (d < 0.0) == m.lower_is_better;
    let style = if better { Style::default().fg(t.target) } else { Style::default().fg(t.distractor) };
    Span::styled(format!(" {d:>+5.prec$}", prec = m.decimals), style)
}

const COL: usize = 22;

/// Fila de métrica con su diferencia.
fn metric_line(t: &Theme, m: &Metric) -> Line<'static> {
    let vs = if m.strong { t.accent().add_modifier(Modifier::BOLD) } else { t.text() };
    let mut l = row(m.label, &m.value, COL, t.dim(), vs);
    l.spans.push(delta(t, m));
    l
}

fn summary(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let Some(s) = &app.session else { return };
    let Some(st) = &s.stats else { return };
    let title = report::title(s, st);
    let (metrics, notes) = report::summary(s, st);
    let mut body: Vec<PLine> = Vec::new();
    // Cifra grande de la métrica principal, solo si cabe entera.
    let main = metrics.iter().find(|m| m.strong);
    if let (Some(m), true) = (main, area.height >= 24) {
        let digits: String = m.value.chars().filter(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            for (i, r) in big(&digits).into_iter().enumerate() {
                let mut spans = vec![Span::styled(r, Style::default().fg(t.target))];
                if i == 1 {
                    let unit = m.value.trim_start_matches(|c: char| c.is_ascii_digit()).trim();
                    spans.push(Span::styled(format!("  {unit} · {}", m.label), t.dim()));
                }
                body.push((Line::from(spans), 4));
            }
            body.push((Line::raw(""), 4));
        }
    }
    if s.prev.is_some() {
        body.push((Line::styled(format!("{:>w$}", "vs anterior", w = COL + 12), t.dim()), 2));
    }
    for m in &metrics {
        body.push((metric_line(t, m), m.prio));
    }
    body.push((Line::raw(""), 3));
    for (n, p) in notes {
        if !n.is_empty() {
            body.push((Line::styled(n, t.dim()), p));
        }
    }
    body.push((Line::raw(""), 3));
    body.push((Line::styled("los tiempos incluyen la latencia de la", t.dim()), 0));
    body.push((Line::styled("terminal: compáralos solo contigo.", t.dim()), 0));
    if let Some(n) = &app.notice {
        body.push((Line::styled(n.clone(), Style::default().fg(t.distractor)), 0));
    }
    let footer = match (&app.plan, app.next_block()) {
        (Some(p), Some(i)) => format!("Enter bloque {}/{} · Esc salir", i + 1, p.blocks.len()),
        (Some(_), None) => "Enter terminar plan · Esc".to_string(),
        (None, _) => "Enter otra · p progreso · a aim · Esc".to_string(),
    };
    let title = match &app.plan {
        Some(p) => format!("plan {}/{} · {title}", p.current + 1, p.blocks.len()),
        None => title,
    };
    panel(f, area, t, &title, &footer, body);
}

const BARS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Gráfica de barras de `rows` filas: cada columna es una sesión, más alta = valor mayor.
/// Devuelve las filas de arriba hacia abajo.
pub fn bars(values: &[Option<f64>], rows: usize) -> Vec<String> {
    let rows = rows.max(1);
    let levels = rows * 8;
    let present: Vec<f64> = values.iter().flatten().copied().collect();
    let lo = present.iter().copied().reduce(f64::min);
    let hi = present.iter().copied().reduce(f64::max);
    let heights: Vec<usize> = values
        .iter()
        .map(|v| match (v, lo, hi) {
            (Some(x), Some(lo), Some(hi)) if (hi - lo).abs() >= f64::EPSILON => {
                1 + (((x - lo) / (hi - lo)) * (levels - 1) as f64).round() as usize
            }
            (Some(_), _, _) => levels / 2,
            (None, _, _) => 0,
        })
        .collect();
    (0..rows)
        .map(|r| {
            let base = (rows - 1 - r) * 8;
            heights.iter().map(|h| BARS[h.saturating_sub(base).min(8)]).collect()
        })
        .collect()
}

/// Compara la última sesión con la mediana de las 5 anteriores.
pub(crate) fn trend(t: &Theme, vals: &[f64], lower_is_better: bool) -> Span<'static> {
    let Some((&last, before)) = vals.split_last() else { return Span::raw("") };
    let window = &before[before.len().saturating_sub(5)..];
    let Some(base) = stats::median(window) else { return Span::raw("") };
    if base.abs() < f64::EPSILON || ((last - base) / base).abs() < 0.02 {
        return Span::styled("estable", t.dim());
    }
    if (last < base) == lower_is_better {
        Span::styled("mejora", Style::default().fg(t.target))
    } else {
        Span::styled("empeora", Style::default().fg(t.distractor))
    }
}

/// Nombre de una vista del progreso.
pub(crate) fn view_name(i: usize) -> String {
    let (k, b) = PROGRESS_VIEWS[i % PROGRESS_VIEWS.len()];
    if b { format!("baseline {}", k.label()) } else { k.label().to_string() }
}

fn progress(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let (kind, _) = PROGRESS_VIEWS[app.progress_view % PROGRESS_VIEWS.len()];
    let list = app.progress_list();
    let rows = app.progress_rows();

    let framed = area.width >= 50 && area.height >= 18;
    let inner = if framed {
        let block = frame_block(t, "progreso", "←→ vista · d dificultad · w semanas · Esc");
        let r = block.inner(area);
        f.render_widget(Clear, area);
        f.render_widget(block, area);
        Rect { x: r.x + 2, y: r.y + 1, width: r.width.saturating_sub(4), height: r.height.saturating_sub(1) }
    } else {
        area
    };
    let width = inner.width as usize;

    let tabs = vec![
        Span::styled("‹ ", t.dim()),
        Span::styled(view_name(app.progress_view), t.accent().add_modifier(Modifier::BOLD)),
        Span::styled(" ›", t.dim()),
        Span::styled(
            format!("  {}/{} · {} sesiones", app.progress_view + 1, PROGRESS_VIEWS.len(), list.len()),
            t.dim(),
        ),
    ];
    let mut lines: Vec<Line> = vec![Line::from(tabs)];
    let mut opts = Vec::new();
    if !app.progress_keys().is_empty() {
        opts.push(Span::styled("dificultad ", t.dim()));
        opts.push(Span::styled(app.progress_filter.clone().unwrap_or_else(|| "todas".into()), t.accent()));
        opts.push(Span::styled(" (d) · ", t.dim()));
    }
    opts.push(Span::styled(if app.progress_weekly { "por semana" } else { "por sesión" }, t.accent()));
    opts.push(Span::styled(" (w)", t.dim()));
    lines.push(Line::from(opts));

    if list.is_empty() {
        lines.push(Line::styled("sin sesiones completas todavía", t.dim()));
    } else {
        let chart_rows = match inner.height {
            h if h >= 30 => 3,
            h if h >= 20 => 2,
            _ => 1,
        };
        let recent = &rows[rows.len().saturating_sub(width)..];
        for m in report::series(kind) {
            let vals: Vec<Option<f64>> = recent.iter().map(|r| (m.get)(&r.stats)).collect();
            let present: Vec<f64> = vals.iter().flatten().copied().collect();
            let lo = present.iter().copied().reduce(f64::min);
            let hi = present.iter().copied().reduce(f64::max);
            let head = format!("{:<15}{:>9}  ", m.label, (m.fmt)(vals.last().copied().flatten()));
            let range = format!("{} – {}", (m.fmt)(lo), (m.fmt)(hi));
            let mut spans = vec![Span::styled(head, t.text()), trend(t, &present, m.lower_is_better)];
            let used: usize = spans.iter().map(|s| s.width()).sum();
            if width > used + range.chars().count() + 2 {
                spans.push(Span::raw(" ".repeat(width - used - range.chars().count())));
                spans.push(Span::styled(range, t.dim()));
            }
            lines.push(Line::from(spans));
            // Con pocas sesiones cada barra ocupa varias columnas para que se lea.
            let col_w = (width / vals.len().max(1)).clamp(1, 3);
            for b in bars(&vals, chart_rows) {
                let wide: String = b.chars().flat_map(|c| std::iter::repeat_n(c, col_w)).collect();
                lines.push(Line::styled(wide, Style::default().fg(t.chart)));
            }
        }
        lines.push(Line::raw(""));
        let (second, main) = report::main_column(kind);
        let (last_name, last) = report::score_column(kind);
        let first = if app.progress_weekly { "semana" } else { "fecha UTC" };
        lines.push(Line::styled(format!("{first:<12}{second:>9}{:>7}{last_name:>10}", "prec"), t.dim()));
        let reserve = if framed { 0 } else { 1 };
        let room = (inner.height as usize).saturating_sub(lines.len() + reserve);
        for r in rows.iter().rev().take(room) {
            lines.push(Line::styled(
                format!("{:<12}{:>9}{:>7}{:>10}", r.label, main(&r.stats), pct(r.stats.accuracy), last(&r.stats)),
                t.text(),
            ));
        }
    }
    if app.bad_lines > 0 {
        lines.push(Line::styled(format!("{} líneas dañadas ignoradas", app.bad_lines), t.dim()));
    }
    lines.truncate(inner.height as usize);
    if !framed {
        let last = inner.height as usize - 1;
        while lines.len() < last {
            lines.push(Line::raw(""));
        }
        lines.truncate(last);
        lines.push(Line::styled("←→ vista · d dific. · w semanas · Esc", t.dim()));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

/// Color según la tasa de acierto: verde si es buena, rojo si es baja.
fn rate_style(t: &Theme, rate: Option<f64>) -> Style {
    match rate {
        Some(r) if r >= 0.85 => Style::default().fg(t.target),
        Some(r) if r < 0.7 => Style::default().fg(t.distractor),
        Some(_) => t.text(),
        None => t.dim(),
    }
}

/// Cuerpo de la pantalla de puntería: rosa de 8 direcciones, tabla y conclusiones.
pub(crate) fn aim_body(t: &Theme, r: &aim::AimReport, width: u16) -> Vec<PLine> {
    let cell = |d: usize| -> Vec<Span<'static>> {
        let ds = &r.dirs[d];
        let txt = if ds.n == 0 {
            format!("{} --", aim::ARROWS[d])
        } else {
            format!("{} {} {}", aim::ARROWS[d], pct(ds.hit_rate), ms(ds.median_ms).replace(" ms", ""))
        };
        vec![Span::styled(format!("{txt:<13}"), rate_style(t, ds.hit_rate))]
    };
    let line = |parts: Vec<Vec<Span<'static>>>| Line::from(parts.into_iter().flatten().collect::<Vec<_>>());
    let pad = |n: usize| vec![Span::raw(" ".repeat(n))];
    let mut out: Vec<PLine> = vec![
        (
            Line::styled(
                format!("{} movimientos · {} sesiones · acierto% y ms por dirección", r.total, r.sessions),
                t.dim(),
            ),
            1,
        ),
        (Line::raw(""), 3),
        (line(vec![pad(13), cell(2)]), 0),
        (line(vec![pad(2), cell(3), pad(9), cell(1)]), 0),
        (line(vec![cell(4), pad(1), vec![Span::styled("·", t.accent())], pad(12), cell(0)]), 0),
        (line(vec![pad(2), cell(5), pad(9), cell(7)]), 0),
        (line(vec![pad(13), cell(6)]), 0),
        (Line::raw(""), 3),
    ];
    if width >= 50 {
        out.push((
            Line::styled(format!("{:<14}{:>4}{:>8}{:>8}{:>9}{:>9}", "dirección", "n", "acierto", "ms", "pasarse", "lateral"), t.dim()),
            4,
        ));
        for d in 0..8 {
            let ds = &r.dirs[d];
            let f1 = |v: Option<f64>| v.map(|x| format!("{x:+.1}")).unwrap_or_else(|| "-".into());
            out.push((
                Line::styled(
                    format!(
                        "{:<14}{:>4}{:>8}{:>8}{:>9}{:>9}",
                        format!("{} {}", aim::ARROWS[d], aim::NAMES[d]),
                        ds.n,
                        pct(ds.hit_rate),
                        ms(ds.median_ms).replace(" ms", ""),
                        f1(ds.along),
                        f1(ds.across)
                    ),
                    rate_style(t, ds.hit_rate),
                ),
                4,
            ));
        }
        out.push((Line::raw(""), 4));
    }
    for (i, tip) in aim::insights(r).into_iter().enumerate() {
        out.push((Line::from(vec![Span::styled("• ", t.accent()), Span::styled(tip, t.text())]), if i == 0 { 0 } else { 1 }));
    }
    out
}

fn aim_screen(f: &mut Frame, app: &App, area: Rect) {
    let Some(r) = &app.aim else { return };
    let body = aim_body(&app.theme, r, area.width);
    panel(f, area, &app.theme, "puntería por dirección", "pasarse: + te pasas, − corto · Esc", body);
}

/// Punto de fijación en el centro.
pub fn fixation(buf: &mut Buffer, area: Rect, theme: &Theme) {
    let x = area.x + area.width / 2;
    let y = area.y + area.height / 2;
    if let Some(c) = buf.cell_mut((x, y)) {
        if theme.code {
            // Como el cursor de una terminal.
            c.set_char('_').set_style(theme.accent().add_modifier(Modifier::BOLD));
        } else {
            c.set_char('•').set_style(theme.text());
        }
    }
}

/// Dibuja el recuadro con el estilo del tema.
pub fn draw_box_themed(buf: &mut Buffer, r: Rect, fill: Fill, color: Color, label: Option<char>, theme: &Theme) {
    if theme.code {
        draw_box_code(buf, r, fill, color, label);
    } else {
        draw_box(buf, r, fill, color, label);
    }
}

/// Texto con el que se rellenan los recuadros del estilo de código.
const CODE: &str = "let mut buf=Vec::with_capacity(n);for i in 0..n{buf.push(f(i)?);}Ok(buf)";

/// Estilo de código: el relleno es un bloque de texto "seleccionado" (fondo de color) y
/// el hueco es un marco de líneas finas con el interior vacío.
pub fn draw_box_code(buf: &mut Buffer, r: Rect, fill: Fill, color: Color, label: Option<char>) {
    let r = r.intersection(buf.area);
    if r.width == 0 || r.height == 0 {
        return;
    }
    let code: Vec<char> = CODE.chars().collect();
    let dark = Color::Rgb(8, 14, 10);
    let (right, bottom) = (r.x + r.width - 1, r.y + r.height - 1);
    for y in r.y..=bottom {
        for x in r.x..=right {
            let (top, bot, left, rgt) = (y == r.y, y == bottom, x == r.x, x == right);
            let (ch, st) = match fill {
                Fill::Filled => {
                    let i = (x as usize * 7 + y as usize * 31) % code.len();
                    (code[i], Style::default().fg(dark).bg(color))
                }
                Fill::Hollow => {
                    let ch = match (top, bot, left, rgt) {
                        (true, _, true, _) => '┌',
                        (true, _, _, true) => '┐',
                        (_, true, true, _) => '└',
                        (_, true, _, true) => '┘',
                        (true, _, _, _) | (_, true, _, _) => '─',
                        (_, _, true, _) | (_, _, _, true) => '│',
                        _ => ' ',
                    };
                    (ch, Style::default().fg(color))
                }
            };
            if let Some(c) = buf.cell_mut((x, y)) {
                c.set_char(ch).set_style(st);
            }
        }
    }
    let Some(label) = label else { return };
    let (lx, ly) = (r.x + r.width / 2, r.y + r.height / 2);
    if let Some(c) = buf.cell_mut((lx, ly)) {
        let st = match fill {
            Fill::Filled => Style::default().fg(dark).bg(color).add_modifier(Modifier::BOLD),
            Fill::Hollow => Style::default().fg(color).add_modifier(Modifier::BOLD),
        };
        c.set_char(label).set_style(st);
    }
}

/// Recuadro con caracteres de bloque: relleno macizo o marco fino de medio bloque,
/// con su número (si lo tiene) al centro.
pub fn draw_box(buf: &mut Buffer, r: Rect, fill: Fill, color: Color, label: Option<char>) {
    let r = r.intersection(buf.area);
    if r.width == 0 || r.height == 0 {
        return;
    }
    let style = Style::default().fg(color);
    let (right, bottom) = (r.x + r.width - 1, r.y + r.height - 1);
    for y in r.y..=bottom {
        for x in r.x..=right {
            let (top, bot, left, rgt) = (y == r.y, y == bottom, x == r.x, x == right);
            let ch = match fill {
                Fill::Filled => '█',
                Fill::Hollow => match (top, bot, left, rgt) {
                    (true, _, true, _) => '▛',
                    (true, _, _, true) => '▜',
                    (_, true, true, _) => '▙',
                    (_, true, _, true) => '▟',
                    (true, _, _, _) => '▀',
                    (_, true, _, _) => '▄',
                    (_, _, true, _) => '▌',
                    (_, _, _, true) => '▐',
                    _ => ' ',
                },
            };
            if let Some(c) = buf.cell_mut((x, y)) {
                c.set_char(ch).set_style(style);
            }
        }
    }
    let Some(label) = label else { return };
    let (lx, ly) = (r.x + r.width / 2, r.y + r.height / 2);
    if let Some(c) = buf.cell_mut((lx, ly)) {
        let st = match fill {
            // Número en negativo sobre el bloque relleno.
            Fill::Filled => Style::default().fg(Color::Rgb(16, 16, 16)).bg(color).add_modifier(Modifier::BOLD),
            Fill::Hollow => style.add_modifier(Modifier::BOLD),
        };
        c.set_char(label).set_style(st);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::history::SessionRecord;
    use crate::config::Difficulty;
    use crate::stats::Stats;
    use crate::config::Config;
    use crate::modes::{Method, TrialRecord};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::time::Instant;

    pub(crate) fn screen_text(app: &App, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| draw(f, app)).unwrap();
        let buf = t.backend().buffer();
        (0..h)
            .map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// App con el tema clásico (estas pruebas son del estilo de recuadros).
    pub(crate) fn app_with(cfg: Config, w: u16, h: u16) -> App {
        App::with_history(cfg, Rect::new(0, 0, w, h), std::env::temp_dir().join("rt-no-existe.jsonl"))
    }

    fn app(cfg: Config, w: u16, h: u16) -> App {
        app_with(Config { theme: crate::config::ThemeName::Dark, ..cfg }, w, h)
    }

    pub(crate) fn trial(outcome: Outcome, rt: Option<f64>) -> TrialRecord {
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
            start: None,
            target_pos: None,
            click: None,
            score: None,
            radius: None,
            warmup: false,
        }
    }

    pub(crate) fn session_rec(mode: GameKind, baseline: bool, incomplete: bool, med: f64) -> SessionRecord {
        SessionRecord {
            version: 1,
            date: "2026-11-01T10:00:00Z".into(),
            mode: mode.id().into(),
            difficulty: Difficulty::Adaptive,
            baseline,
            incomplete,
            discarded: 0,
            final_level: None,
            duration_s: None,
            pixel_mouse: false,
            config: Config::default(),
            stats: Stats { median_ms: Some(med), accuracy: Some(1.0), ies_ms: Some(med), hits: 7, ..Default::default() },
            trials: vec![],
        }
    }

    fn summary_text(mode: GameKind, w: u16, h: u16) -> String {
        let mut a = app(Config { mode, ..Config::default() }, w, h);
        a.history.push(session_rec(mode, false, false, 500.0));
        a.start_session(false, Instant::now());
        let s = a.session.as_mut().unwrap();
        s.records = vec![trial(Outcome::Hit, Some(400.0)), trial(Outcome::Error, Some(300.0))];
        s.stats = Some(stats::compute(&s.records));
        a.screen = Screen::Summary;
        screen_text(&a, w, h)
    }

    #[test]
    fn summary_fits_minimum_panel_and_large_one() {
        for (w, h) in [(40, 12), (100, 30)] {
            let text = summary_text(GameKind::Decision, w, h);
            for needle in ["mediana", "precisión", "puntaje", "latencia", "Esc", "-100"] {
                assert!(text.contains(needle), "falta {needle} en {w}x{h}:\n{text}");
            }
        }
        let text = summary_text(GameKind::Targets, 40, 12);
        for needle in ["blancos", "aciertos", "puntaje", "latencia"] {
            assert!(text.contains(needle), "falta {needle}:\n{text}");
        }
    }

    #[test]
    fn menu_shows_mode_and_frames_when_room() {
        let mut a = app(Config::default(), 80, 24);
        a.menu_idx = 1;
        let text = screen_text(&a, 80, 24);
        assert!(text.contains("‹ decisión ›") && text.contains("╭") && text.contains("▀"), "{text}");
        let text = screen_text(&a, 40, 12);
        assert!(text.contains("empezar") && text.contains("salir"), "{text}");
    }

    #[test]
    fn progress_separates_modes_and_baselines() {
        let mut a = app(Config::default(), 40, 12);
        a.history = vec![
            session_rec(GameKind::Decision, false, false, 410.0),
            session_rec(GameKind::Decision, false, true, 999.0),
            session_rec(GameKind::Decision, true, false, 520.0),
            session_rec(GameKind::Targets, false, false, 640.0),
        ];
        a.screen = Screen::Progress;
        for (view, has, hasnt) in [(0, "410 ms", ["999", "520", "640"]), (1, "520 ms", ["999", "410", "640"]), (2, "640 ms", ["999", "410", "520"])] {
            a.progress_view = view;
            for (w, h) in [(40, 12), (90, 30)] {
                let text = screen_text(&a, w, h);
                assert!(text.contains(has), "vista {view} {w}x{h}:\n{text}");
                assert!(hasnt.iter().all(|x| !text.contains(x)), "vista {view} {w}x{h}:\n{text}");
            }
        }
    }

    #[test]
    fn bars_scale() {
        let sparkline = |v: &[Option<f64>]| bars(v, 1).remove(0);
        assert_eq!(sparkline(&[Some(1.0), Some(2.0), None, Some(3.0)]), "▁▅ █");
        assert_eq!(sparkline(&[Some(5.0), Some(5.0)]), "▄▄");
        assert_eq!(sparkline(&[None, None]), "  ");
        assert_eq!(bars(&[Some(0.0), Some(1.0)], 2), vec![" █", "▁█"]);
    }

    #[test]
    fn big_digits() {
        assert_eq!(big("1"), [String::from("▄█ "), String::from(" █ "), String::from("▀▀▀")]);
        assert_eq!(big("70")[0], "▀▀█ █▀█");
    }

    #[test]
    fn effects_draw_inside_area() {
        use crate::modes::Flash;
        let mut a = app(Config::default(), 40, 12);
        let now = Instant::now();
        for kind in [FlashKind::Hit, FlashKind::Wrong, FlashKind::Reveal, FlashKind::Miss] {
            a.effects.push((Flash { kind, rect: Rect::new(0, 0, 4, 3) }, now));
            a.effects.push((Flash { kind, rect: Rect::new(36, 8, 4, 3) }, now));
        }
        let area = Rect::new(0, 0, 40, 11);
        let mut buf = Buffer::empty(area);
        draw_effects(&mut buf, area, &a, now + EFFECT / 2);
        assert!((0..40).any(|x| buf[(x, 0)].symbol() != " "));
    }

    #[test]
    fn background_paints_gradient_and_grid() {
        let a = app(Config::default(), 40, 12);
        let area = Rect::new(0, 0, 40, 12);
        let mut buf = Buffer::empty(area);
        paint_background(&mut buf, area, &a.theme);
        assert_ne!(buf[(0, 0)].bg, buf[(0, 11)].bg);
        assert_eq!(buf[(3, 1)].symbol(), "·");
        let plain = app(Config { background: crate::config::Background::None, ..Config::default() }, 40, 12);
        let mut buf = Buffer::empty(area);
        paint_background(&mut buf, area, &plain.theme);
        assert_eq!(buf[(0, 0)].bg, Color::Reset);
    }

    #[test]
    fn fit_drops_optional_lines_first() {
        let l = |s: &str| Line::raw(s.to_string());
        let out = fit(vec![(l("a"), 0), (l(""), 3), (l("b"), 1), (l("c"), 0)], 3);
        assert_eq!(out.iter().map(|x| x.to_string()).collect::<Vec<_>>(), vec!["a", "b", "c"]);
    }

    #[test]
    fn hollow_box_shape() {
        let area = Rect::new(0, 0, 5, 3);
        let mut buf = Buffer::empty(area);
        draw_box(&mut buf, area, Fill::Hollow, Color::Red, Some('2'));
        let rows: Vec<String> =
            (0..3).map(|y| (0..5).map(|x| buf[(x, y)].symbol().to_string()).collect()).collect();
        assert_eq!(rows, vec!["▛▀▀▀▜", "▌ 2 ▐", "▙▄▄▄▟"]);
    }
}
