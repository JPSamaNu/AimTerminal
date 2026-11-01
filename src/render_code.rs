//! Estilo de código (temas `matrix` y `cyber`): cada pantalla imita salida de terminal.
//! El menú parece un `--help`, la barra de estado el progreso de `cargo`, la pausa un
//! proceso detenido, el resumen la salida de `cargo test` y el progreso un `git log`.

use crate::app::{App, MENU, MenuItem, PROGRESS_VIEWS, PauseReason, Screen};
use crate::config::{Config, Difficulty, GameKind};
use crate::modes::Outcome;
use crate::render::{
    PLine, aim_body, bars, delta, draw_effects, fit, mode_hint, ms, pct, play_area, status_area, trend,
    view_name,
};
use crate::report::{self, Metric};
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use std::time::Instant;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    paint_ambient(f.buffer_mut(), area, app);
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

/// Salida falsa de fondo, alineada abajo como en una terminal.
fn paint_ambient(buf: &mut Buffer, area: Rect, app: &App) {
    let Some(color) = app.theme.ambient else { return };
    let h = area.height as usize;
    let lines: Vec<&str> = app.ambient.tail(h).collect();
    let start = area.y + (h - lines.len()) as u16;
    for (i, l) in lines.iter().enumerate() {
        buf.set_stringn(area.x, start + i as u16, l, area.width as usize, Style::default().fg(color));
    }
}

/// Escribe líneas desde arriba a la izquierda, como salida de un comando, borrando
/// el fondo de esas filas (y una más de margen).
fn output(f: &mut Frame, area: Rect, lines: Vec<PLine>) {
    let lines = fit(lines, area.height as usize);
    let h = (lines.len() as u16 + 1).min(area.height);
    let r = Rect { height: h, ..area };
    f.render_widget(Clear, r);
    f.render_widget(Paragraph::new(lines), r);
}

/// Prompt y comando. Si no cabe, el prompt se acorta a la última carpeta, como hacen
/// muchos prompts configurados.
fn prompt(app: &App, cmd: &str, width: u16) -> Line<'static> {
    let full = app.prompt.chars().count() + cmd.chars().count();
    let p = if full <= width as usize { app.prompt.clone() } else { short_prompt(&app.prompt) };
    Line::from(vec![Span::styled(p, app.theme.text()), Span::styled(cmd.to_string(), app.theme.accent())])
}

fn short_prompt(p: &str) -> String {
    let (head, tail) = if let Some(rest) = p.strip_prefix("PS ") { ("PS ", rest) } else { ("", p) };
    let end = tail.trim_end().trim_end_matches(['>', '$']).trim_end();
    let last = end.rsplit(['\\', '/']).next().unwrap_or(end);
    if head.is_empty() { format!("{last} $ ") } else { format!("PS {last}> ") }
}

/// Verbo alineado a la derecha en 12 columnas, en negrita, como `cargo`.
fn verb(t: &Theme, v: &str, rest: impl Into<String>, rest_style: Style) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{v:>12} "), t.accent().add_modifier(Modifier::BOLD)),
        Span::styled(rest.into(), rest_style),
    ])
}

fn run_cmd(cfg: &Config, baseline: bool) -> String {
    let mut cmd = format!("rt run --mode {}", cfg.mode.id());
    if baseline {
        cmd.push_str(" --baseline");
    } else if cfg.mode.adaptive() {
        match (cfg.mode, cfg.mode_difficulty()) {
            (_, Difficulty::Adaptive) => cmd.push_str(" --adaptive"),
            (GameKind::Targets | GameKind::Flick, Difficulty::Fixed) => {
                cmd.push_str(&format!(" --level {}", cfg.fixed_level().unwrap_or(0)))
            }
            (_, Difficulty::Fixed) => cmd.push_str(" --fixed"),
        }
    }
    cmd
}

fn menu(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let mut lines: Vec<PLine> = vec![
        (prompt(app, "rt --help", area.width), 0),
        (Line::styled(format!("rt {} · decision & reflex trainer", env!("CARGO_PKG_VERSION")), t.text()), 2),
        (Line::raw(""), 3),
        (Line::styled("USAGE:", t.accent()), 3),
        (Line::styled("    rt [COMMAND] [OPTIONS]", t.text()), 4),
        (Line::raw(""), 3),
        (Line::styled("COMMANDS:", t.accent()), 1),
    ];
    for (i, item) in MENU.iter().enumerate() {
        let selected = i == app.menu_idx;
        let (cmd, desc) = match item {
            MenuItem::Start => ("run", "empezar sesión".to_string()),
            MenuItem::Mode => ("--mode", format!("{}  ←→", app.cfg.mode.id())),
            MenuItem::Difficulty => (
                "--level",
                app.cfg.difficulty_label().map(|l| format!("{l}  ←→")).unwrap_or_else(|| "-".to_string()),
            ),
            MenuItem::Baseline => ("baseline", format!("prueba base fija ({})", app.cfg.baseline_mode().id())),
            MenuItem::Plan => ("plan", "rutina según tus puntos débiles".to_string()),
            MenuItem::Progress => ("log", "progreso".to_string()),
            MenuItem::Aim => ("aim", "puntería por dirección".to_string()),
            MenuItem::Quit => ("exit", "salir".to_string()),
        };
        let mark = if selected { ">" } else { " " };
        let text = format!("  {mark} {cmd:<10} {desc:<22}");
        let mut line = Line::styled(text, if selected { t.accent() } else { t.text() });
        if selected {
            line = line.style(Style::default().fg(t.accent).bg(t.select_bg()));
        }
        lines.push((line, 0));
    }
    let last = crate::app::last_comparable(&app.history, &app.cfg, false);
    if let Some(s) = last {
        lines.push((Line::raw(""), 3));
        lines.push((
            Line::styled(
                format!("last run: score={} median={}", ms(s.stats.ies_ms), ms(s.stats.median_ms)).replace(" ms", "ms"),
                t.dim(),
            ),
            2,
        ));
    }
    lines.push((Line::raw(""), 3));
    lines.push((Line::styled("↑↓ Enter · ←→ cambia · q", t.dim()), 1));
    output(f, area, lines);
}

fn countdown(f: &mut Frame, app: &App, area: Rect, left: u8) {
    let t = &app.theme;
    let Some(s) = &app.session else { return };
    let what = match s.cfg.session_secs() {
        Some(secs) => format!("{} session · {secs}s", s.cfg.mode.id()),
        None => format!("{} session · {} trials", s.cfg.mode.id(), s.cfg.session_trials()),
    };
    let lines: Vec<PLine> = vec![
        (prompt(app, &run_cmd(&s.cfg, s.baseline), area.width), 0),
        (verb(t, "Starting", what, t.text()), 0),
        (verb(t, "Waiting", format!("{left}s"), t.accent()), 0),
        (verb(t, "Note", mode_hint(s.cfg.mode), t.dim()), 1),
    ];
    output(f, play_area(area), lines);
    status(f, app, area);
}

/// Barra de progreso como la de `cargo`.
fn progress_bar(done: f64, total: f64, width: usize) -> String {
    let frac = if total > 0.0 { (done / total).clamp(0.0, 1.0) } else { 0.0 };
    let filled = (frac * width as f64).round() as usize;
    let mut s = "=".repeat(filled.saturating_sub(1));
    if filled > 0 {
        s.push(if filled >= width { '=' } else { '>' });
    }
    format!("[{s:<width$}]")
}

fn status(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let Some(s) = &app.session else { return };
    let a = status_area(area);
    let bar_w = (a.width as usize).saturating_sub(52).clamp(8, 30);
    let (v, bar, rest) = match (s.warming_up(), s.timer) {
        (Some((done, total)), _) => (
            "Warming",
            progress_bar(done as f64, total as f64, bar_w),
            format!(" {done}/{total} warm-up (no cuenta)"),
        ),
        (None, Some(timer)) => {
            let now = Instant::now();
            let left = timer.shown_secs(now);
            let total = timer.total.as_secs_f64();
            let fails = s.count(Outcome::Miss) + s.count(Outcome::Anticipation);
            let rest = if s.cfg.mode == GameKind::Tracking {
                let on = crate::stats::compute(&s.records).on_target;
                format!(" {left}s left: on target {}", pct(on))
            } else {
                format!(" {left}s left: {} ok, {fails} failed", s.count(Outcome::Hit))
            };
            ("Running", progress_bar(timer.elapsed(now).as_secs_f64(), total, bar_w), rest)
        }
        (None, None) => (
            "Building",
            progress_bar(s.counted as f64, s.cfg.session_trials() as f64, bar_w),
            format!(" {}/{}", s.counted, s.cfg.session_trials()),
        ),
    };
    let mut rest = rest;
    if let Some(l) = s.level() {
        rest.push_str(&format!(" lvl{l}"));
    }
    if app.mouse.pixel == crate::input::PixelMode::On {
        rest.push_str(" px");
    }
    if let (Some(r), false) = (&s.last, s.cfg.mode == GameKind::Tracking) {
        rest.push_str(&format!(": {}", r.outcome.label()));
        if let (Some(ms), false) = (r.rt_ms, r.anticipation) {
            rest.push_str(&format!(" {ms:.0}ms"));
        }
    }
    let line = Line::from(vec![
        Span::styled(format!("{v:>12} "), t.accent().add_modifier(Modifier::BOLD)),
        Span::styled(bar, t.text()),
        Span::styled(rest, t.dim()),
    ]);
    f.render_widget(Clear, a);
    f.render_widget(Paragraph::new(line), a);
}

fn paused(f: &mut Frame, app: &App, area: Rect, reason: PauseReason) {
    let t = &app.theme;
    let cmd = app.session.as_ref().map(|s| run_cmd(&s.cfg, s.baseline)).unwrap_or_default();
    let why = match reason {
        PauseReason::User => "",
        PauseReason::FocusLost => "  (sin foco)",
        PauseReason::TooSmall => "  (ventana pequeña)",
    };
    let lines: Vec<PLine> = vec![
        (Line::styled("^Z", t.text()), 1),
        (Line::styled(format!("[1]+  Stopped                 {cmd}{why}"), t.text()), 0),
        (Line::raw(""), 3),
        (Line::styled("      Enter fg · Esc kill · q exit", t.dim()), 0),
        (Line::styled("      el intento en curso no cuenta", t.dim()), 2),
    ];
    output(f, play_area(area), lines);
    status(f, app, area);
}

/// `test nombre ........ valor  (diferencia)`
fn test_line(t: &Theme, m: &Metric) -> Line<'static> {
    let name = m.label.replace(' ', "_");
    let dots = ".".repeat(15usize.saturating_sub(name.chars().count()).max(2));
    let vs = if m.strong { t.accent().add_modifier(Modifier::BOLD) } else { t.text() };
    Line::from(vec![
        Span::styled("test ", t.dim()),
        Span::styled(format!("{name} {dots} "), t.text()),
        Span::styled(format!("{:>8}", m.value), vs),
        delta(t, m),
    ])
}

fn summary(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let Some(s) = &app.session else { return };
    let Some(st) = &s.stats else { return };
    let mut lines: Vec<PLine> = vec![(prompt(app, &run_cmd(&s.cfg, s.baseline), area.width), 1)];
    lines.push((Line::raw(""), 3));
    lines.push((Line::styled(format!("running {}", report::title(s, st)), t.text()), 0));
    let (metrics, notes) = report::summary(s, st);
    for m in &metrics {
        lines.push((test_line(t, m), m.prio));
    }
    lines.push((Line::raw(""), 3));
    for (n, p) in notes {
        if !n.is_empty() {
            lines.push((Line::styled(n, t.dim()), p));
        }
    }
    lines.push((Line::styled("note: tiempos con latencia de terminal;", t.dim()), 0));
    lines.push((Line::styled("      compáralos solo contigo", t.dim()), 0));
    if let Some(n) = &app.notice {
        lines.push((Line::styled(format!("error: {n}"), Style::default().fg(t.distractor)), 0));
    }
    lines.push((Line::raw(""), 3));
    let ok = match s.cfg.mode {
        GameKind::Tracking => st.on_target.unwrap_or(0.0) >= 0.5,
        _ => st.accuracy.unwrap_or(0.0) >= 0.8,
    };
    let failed = st.trials.saturating_sub(st.hits + st.correct_rejections);
    let elapsed = s.timer.map(|tm| tm.total.as_secs_f64()).unwrap_or_else(|| s.started.elapsed().as_secs_f64());
    lines.push((
        Line::from(vec![
            Span::styled("test result: ", t.text()),
            Span::styled(
                if ok { "ok" } else { "FAILED" },
                Style::default().fg(if ok { t.target } else { t.distractor }),
            ),
            Span::styled(
                format!(". {} passed; {failed} failed; finished in {elapsed:.1}s", st.hits + st.correct_rejections),
                t.text(),
            ),
        ]),
        0,
    ));
    let footer = match (&app.plan, app.next_block()) {
        (Some(p), Some(i)) => format!("Enter next block ({}/{}) · Esc abort plan", i + 1, p.blocks.len()),
        (Some(_), None) => "Enter finish plan · Esc".to_string(),
        (None, _) => "Enter rerun · p log · a aim · Esc".to_string(),
    };
    lines.push((Line::styled(footer, t.dim()), 0));
    output(f, area, lines);
}

fn progress(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let (kind, baseline) = PROGRESS_VIEWS[app.progress_view % PROGRESS_VIEWS.len()];
    let list = app.progress_list();
    let rows = app.progress_rows();
    let mut cmd = format!("rt log --mode {}", kind.id());
    if baseline {
        cmd.push_str(" --baseline");
    }
    if let Some(f) = &app.progress_filter {
        cmd.push_str(&format!(" --level {f}"));
    }
    if app.progress_weekly {
        cmd.push_str(" --weekly");
    }
    let mut lines: Vec<PLine> = vec![(prompt(app, &cmd, area.width), 0)];
    lines.push((
        Line::from(vec![
            Span::styled("view ", t.dim()),
            Span::styled(format!("[{}/{}]", app.progress_view + 1, PROGRESS_VIEWS.len()), t.accent()),
            Span::styled(format!(" {} · {} sessions", view_name(app.progress_view), list.len()), t.dim()),
            Span::styled(if app.progress_weekly { " · por semana" } else { "" }, t.dim()),
        ]),
        1,
    ));
    if list.is_empty() {
        lines.push((Line::styled("fatal: no sessions yet", t.dim()), 0));
    } else {
        let width = area.width as usize;
        let chart_w = width.saturating_sub(34).clamp(6, 60);
        let recent = &rows[rows.len().saturating_sub(chart_w)..];
        for m in report::series(kind) {
            let vals: Vec<Option<f64>> = recent.iter().map(|r| (m.get)(&r.stats)).collect();
            let present: Vec<f64> = vals.iter().flatten().copied().collect();
            let col_w = (chart_w / vals.len().max(1)).clamp(1, 3);
            let chart: String = bars(&vals, 1)
                .remove(0)
                .chars()
                .flat_map(|c| std::iter::repeat_n(c, col_w))
                .collect();
            lines.push((
                Line::from(vec![
                    Span::styled(format!("{:<15}", m.label), t.dim()),
                    Span::styled(format!("{chart:<chart_w$}"), Style::default().fg(t.chart)),
                    Span::styled(format!(" {:>7} ", (m.fmt)(vals.last().copied().flatten())), t.accent()),
                    trend(t, &present, m.lower_is_better),
                ]),
                0,
            ));
        }
        lines.push((Line::raw(""), 3));
        let (main_name, main) = report::main_column(kind);
        let (last_name, last) = report::score_column(kind);
        for r in rows.iter().rev() {
            let kv = |k: &str, v: String| format!("{k}={}", v.replace(' ', ""));
            lines.push((
                Line::from(vec![
                    Span::styled(format!("{:<7} ", r.tag), Style::default().fg(t.target)),
                    Span::styled(format!("{} ", r.label), t.dim()),
                    Span::styled(
                        format!(
                            "{} {} {}",
                            kv(main_name, main(&r.stats)),
                            kv("acc", pct(r.stats.accuracy)),
                            kv(last_name, last(&r.stats))
                        ),
                        t.text(),
                    ),
                ]),
                5,
            ));
        }
    }
    if app.bad_lines > 0 {
        lines.push((Line::styled(format!("warning: {} líneas dañadas ignoradas", app.bad_lines), t.dim()), 1));
    }
    // El pie siempre visible: va al final y las filas del log (prioridad 5) ceden primero.
    lines.push((Line::styled("←→ vista · d dific. · w semanas · Esc", t.dim()), 0));
    let lines = fit_keep_order(lines, area.height as usize);
    output(f, area, lines);
}

fn plan_screen(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let Some(p) = &app.plan else { return };
    let mut lines: Vec<PLine> = vec![(prompt(app, "rt plan", area.width), 0)];
    lines.push((
        verb(
            t,
            "Planning",
            format!("{} blocks (~{} min)", p.blocks.len(), crate::render::plan_minutes(&p.blocks)),
            t.text(),
        ),
        0,
    ));
    for (i, b) in p.blocks.iter().enumerate() {
        let level = b.cfg.difficulty_label().filter(|_| area.width >= 60).map(|l| format!(" [{l}]")).unwrap_or_default();
        lines.push((
            Line::from(vec![
                Span::styled(format!("{:>12} ", format!("{}/{}", i + 1, p.blocks.len())), t.accent()),
                Span::styled(format!("{} ({}){level}", b.kind().id(), b.size_label()), t.text()),
            ]),
            0,
        ));
        lines.push((Line::styled(format!("{:>13}# {}", "", b.reason), t.dim()), 1));
    }
    lines.push((Line::raw(""), 3));
    lines.push((Line::styled("Enter run · Esc", t.dim()), 0));
    output(f, area, lines);
}

fn plan_done(f: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let Some(p) = &app.plan else { return };
    let mut lines: Vec<PLine> = vec![(prompt(app, "rt plan", area.width), 1)];
    for (mode, name, value) in &p.results {
        lines.push((
            Line::from(vec![
                Span::styled(format!("{:>12} ", "ok"), Style::default().fg(t.target).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{mode:<12} "), t.text()),
                Span::styled(format!("{}={}", name.replace(' ', "_"), value.replace(' ', "")), t.accent()),
            ]),
            0,
        ));
    }
    lines.push((verb(t, "Finished", format!("plan: {} blocks", p.results.len()), t.text()), 0));
    lines.push((Line::styled("Enter menu", t.dim()), 0));
    output(f, area, lines);
}

fn aim_screen(f: &mut Frame, app: &App, area: Rect) {
    let Some(r) = &app.aim else { return };
    let mut lines: Vec<PLine> = vec![(prompt(app, &format!("rt aim --last {}", r.sessions.max(1)), area.width), 0)];
    lines.extend(aim_body(&app.theme, r, area.width));
    lines.push((Line::raw(""), 3));
    lines.push((Line::styled("pasarse: + te pasas, - corto · Esc", app.theme.dim()), 0));
    let lines = fit_keep_order(lines, area.height as usize);
    output(f, area, lines);
}

/// Como `fit`, pero entre las filas de log quita primero las más viejas (las de abajo).
fn fit_keep_order(lines: Vec<PLine>, height: usize) -> Vec<PLine> {
    let mut lines = lines;
    while lines.len() > height {
        let worst = lines.iter().map(|(_, p)| *p).max().unwrap_or(0);
        if worst == 0 {
            lines.truncate(height);
            break;
        }
        let i = lines.iter().rposition(|(_, p)| *p == worst).unwrap_or(lines.len() - 1);
        lines.remove(i);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::Stats;

    use crate::config::Config;
    use crate::render::tests::{app_with, screen_text, session_rec, trial};
    use crate::stats;

    /// Sin texto de fondo: es aleatorio y podría coincidir con lo que se busca.
    fn quiet(cfg: Config) -> Config {
        Config { background: crate::config::Background::None, ..cfg }
    }

    fn summary_text(mode: GameKind, w: u16, h: u16) -> String {
        let mut a = app_with(quiet(Config { mode, ..Config::default() }), w, h);
        a.history.push(session_rec(mode, false, false, 500.0));
        a.start_session(false, Instant::now());
        let s = a.session.as_mut().unwrap();
        s.records = vec![trial(Outcome::Hit, Some(400.0)), trial(Outcome::Error, Some(300.0))];
        s.stats = Some(stats::compute(&s.records));
        a.screen = Screen::Summary;
        screen_text(&a, w, h)
    }

    #[test]
    fn summary_reads_like_cargo_test() {
        for (w, h) in [(40, 12), (100, 30)] {
            let text = summary_text(GameKind::Decision, w, h);
            for needle in ["test mediana", "test puntaje", "test result: FAILED", "latencia", "Esc", "-100"] {
                assert!(text.contains(needle), "falta {needle} en {w}x{h}:
{text}");
            }
        }
        let text = summary_text(GameKind::Targets, 40, 12);
        assert!(text.contains("test aciertos") && text.contains("test result"), "{text}");
    }

    #[test]
    fn menu_reads_like_help_and_fits() {
        for (w, h) in [(40, 12), (100, 30)] {
            let a = app_with(quiet(Config::default()), w, h);
            let text = screen_text(&a, w, h);
            for needle in ["rt --help", "> run", "--mode", "exit"] {
                assert!(text.contains(needle), "falta {needle} en {w}x{h}:
{text}");
            }
        }
    }

    #[test]
    fn progress_reads_like_git_log() {
        let mut a = app_with(quiet(Config::default()), 40, 12);
        a.history = (0..20).map(|i| session_rec(GameKind::Decision, false, false, 400.0 + i as f64)).collect();
        a.history.push(session_rec(GameKind::Targets, false, false, 640.0));
        a.screen = Screen::Progress;
        for (w, h) in [(40, 12), (100, 30)] {
            let text = screen_text(&a, w, h);
            assert!(text.contains("rt log --mode decision") && text.contains("mediana=419ms"), "{w}x{h}:
{text}");
            assert!(text.contains("←→ vista") && !text.contains("640"), "{w}x{h}:
{text}");
        }
    }

    #[test]
    fn game_shows_code_boxes_and_cargo_status() {
        let mut a = app_with(Config { catch_ratio: 0.0, warmup_trials: 0, ..Config::default() }, 80, 24);
        let t0 = Instant::now();
        a.start_session(false, t0);
        a.screen = Screen::Game;
        let area = render_area(&a);
        let s = a.session.as_mut().unwrap();
        let params = crate::modes::Params {
            box_w: 8,
            box_h: 4,
            min_boxes: 3,
            max_boxes: 3,
            timeout: std::time::Duration::from_secs(2),
            level: None,
        };
        s.mode.start_trial(area, &params, t0, None);
        let d = s.mode.deadline().unwrap();
        s.mode.tick(d);
        let text = screen_text(&a, 80, 24);
        assert!(text.contains("Building [") && text.contains("┌"), "{text}");
    }

    fn render_area(a: &App) -> Rect {
        play_area(a.size)
    }

    #[test]
    fn prompt_shortens_to_last_folder() {
        assert_eq!(short_prompt("PS C:\\Proyectos\\SukaTrainer> "), "PS SukaTrainer> ");
        assert_eq!(short_prompt("/home/u/proj $ "), "proj $ ");
    }

    #[test]
    fn cargo_like_progress_bar() {
        assert_eq!(progress_bar(0.0, 20.0, 10), "[          ]");
        assert_eq!(progress_bar(10.0, 20.0, 10), "[====>     ]");
        assert_eq!(progress_bar(20.0, 20.0, 10), "[==========]");
    }

    #[test]
    fn fake_hash_is_stable_and_short() {
        let mut s = crate::history::SessionRecord {
            version: 1,
            date: "2026-11-01T10:00:00Z".into(),
            mode: "decision".into(),
            difficulty: Difficulty::Fixed,
            baseline: false,
            incomplete: false,
            discarded: 0,
            final_level: None,
            duration_s: None,
            pixel_mouse: false,
            config: crate::config::Config::default(),
            stats: Stats::default(),
            trials: vec![],
        };
        let a = report::session_hash(&s);
        assert_eq!(a.len(), 7);
        assert_eq!(a, report::session_hash(&s));
        s.date.push('x');
        assert_ne!(a, report::session_hash(&s));
    }
}
