//! Estado de la aplicación, pantallas y bucle de eventos.

use crate::config::{Config, Difficulty};
use crate::difficulty::{self, Staircase};
use crate::history::{self, SessionRecord};
use crate::input::{self, Input, Mouse};
use crate::modes::decision::Decision;
use crate::modes::{GameMode, Params, Response, Step, TrialRecord};
use crate::render;
use crate::stats::{self, Stats};
use crate::term::{self, Tui};
use crate::theme::Theme;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub const MENU: [&str; 5] = ["Empezar", "Baseline", "Progreso", "Dificultad", "Salir"];
const COUNTDOWN: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Menu,
    Countdown { left: u8, next: Instant },
    Game,
    Paused(PauseReason),
    Summary,
    Progress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseReason {
    User,
    FocusLost,
    TooSmall,
}

pub struct Session {
    pub mode: Box<dyn GameMode>,
    pub cfg: Config,
    pub baseline: bool,
    pub records: Vec<TrialRecord>,
    /// Intentos descartados por pérdida de foco, cambio de tamaño o pausa.
    pub discarded: usize,
    pub last: Option<TrialRecord>,
    pub stats: Option<Stats>,
    /// Solo en dificultad adaptativa.
    pub stair: Option<Staircase>,
}

impl Session {
    fn params(&self) -> Params {
        if let Some(st) = &self.stair {
            return difficulty::level_params(st.level);
        }
        Params {
            box_w: self.cfg.box_w,
            box_h: self.cfg.box_h,
            min_boxes: self.cfg.min_boxes,
            max_boxes: self.cfg.max_boxes,
            timeout: Duration::from_millis(self.cfg.timeout_ms),
            level: None,
        }
    }

    pub fn done(&self) -> bool {
        self.records.len() >= self.cfg.trials
    }

    pub fn final_level(&self) -> Option<usize> {
        self.stair.as_ref().map(|s| s.level)
    }

    fn to_record(&self, stats: Stats, incomplete: bool, pixel_mouse: bool) -> SessionRecord {
        SessionRecord {
            version: 1,
            date: history::now_iso(),
            mode: self.mode.id().to_string(),
            difficulty: self.cfg.difficulty,
            baseline: self.baseline,
            incomplete,
            discarded: self.discarded,
            final_level: self.final_level(),
            pixel_mouse,
            config: self.cfg.clone(),
            stats,
            trials: self.records.clone(),
        }
    }
}

/// Datos de la pantalla de progreso.
pub struct ProgressView {
    pub sessions: Vec<SessionRecord>,
    pub bad_lines: usize,
    pub baselines: bool,
}

pub struct App {
    pub cfg: Config,
    pub theme: Theme,
    pub screen: Screen,
    pub menu_idx: usize,
    pub session: Option<Session>,
    pub mouse: Mouse,
    pub size: Rect,
    pub progress: Option<ProgressView>,
    /// Aviso de una sola vez (p. ej. un error al guardar).
    pub notice: Option<String>,
    history_path: PathBuf,
    /// Tipo de la última sesión, para repetirla desde el resumen.
    last_baseline: bool,
    pub quit: bool,
    dirty: bool,
}

impl App {
    pub fn new(cfg: Config, size: Rect) -> App {
        let theme = Theme::from_config(&cfg);
        let mouse = Mouse::new(cfg.pixel_mouse, size.width, size.height);
        App {
            cfg,
            theme,
            screen: Screen::Menu,
            menu_idx: 0,
            session: None,
            mouse,
            size,
            progress: None,
            notice: None,
            history_path: history::history_path(),
            last_baseline: false,
            quit: false,
            dirty: true,
        }
    }

    fn too_small(&self) -> bool {
        self.size.width < render::MIN_W || self.size.height < render::MIN_H
    }

    pub fn start_session(&mut self, baseline: bool, now: Instant) {
        let cfg = if baseline { self.cfg.baseline() } else { self.cfg.clone() };
        let mode = Box::new(Decision::new(&cfg, fastrand::Rng::new()));
        let stair = (cfg.difficulty == Difficulty::Adaptive).then(|| Staircase::new(cfg.start_level));
        self.session =
            Some(Session { mode, cfg, baseline, records: Vec::new(), discarded: 0, last: None, stats: None, stair });
        self.last_baseline = baseline;
        self.screen = Screen::Countdown { left: COUNTDOWN, next: now + Duration::from_secs(1) };
        self.dirty = true;
    }

    fn begin_trial(&mut self, now: Instant) {
        let area = render::play_area(self.size);
        if let Some(s) = self.session.as_mut() {
            let params = s.params();
            s.mode.start_trial(area, &params, now);
        }
        self.dirty = true;
    }

    /// Cancela el intento en curso; si de verdad había uno, cuenta como descartado.
    fn discard_trial(&mut self) {
        if let Some(s) = self.session.as_mut().filter(|s| s.mode.in_trial()) {
            s.mode.abort();
            s.discarded += 1;
        }
    }

    fn pause(&mut self, reason: PauseReason) {
        self.discard_trial();
        self.screen = Screen::Paused(reason);
        self.dirty = true;
    }

    fn resume(&mut self, now: Instant) {
        if self.too_small() {
            return;
        }
        self.screen = Screen::Countdown { left: COUNTDOWN, next: now + Duration::from_secs(1) };
        self.dirty = true;
    }

    fn on_step(&mut self, step: Step, now: Instant) {
        match step {
            Step::Idle => {}
            Step::Redraw => self.dirty = true,
            Step::Done(mut rec) => {
                let Some(s) = self.session.as_mut() else { return };
                rec.index = s.records.len();
                if let Some(st) = s.stair.as_mut() {
                    st.update(rec.outcome);
                }
                s.last = Some(rec.clone());
                s.records.push(rec);
                if s.done() {
                    self.finish(true);
                } else {
                    self.begin_trial(now);
                }
                self.dirty = true;
            }
        }
    }

    /// Cierra la sesión activa. Las incompletas se guardan marcadas para excluirlas del progreso.
    fn finish(&mut self, complete: bool) {
        let pixel = self.mouse.pixel == crate::input::PixelMode::On;
        let Some(s) = self.session.as_mut() else { return };
        let st = stats::compute(&s.records);
        if complete || !s.records.is_empty() {
            let rec = s.to_record(st.clone(), !complete, pixel);
            if let Err(e) = history::append(&self.history_path, &rec) {
                self.notice = Some(format!("no se pudo guardar: {e}"));
            }
        }
        if complete {
            s.stats = Some(st);
            self.screen = Screen::Summary;
        } else {
            self.session = None;
        }
    }

    fn open_progress(&mut self) {
        let (sessions, bad_lines) = history::load(&self.history_path);
        self.progress = Some(ProgressView { sessions, bad_lines, baselines: false });
        self.screen = Screen::Progress;
    }

    fn leave_session(&mut self) {
        self.discard_trial();
        if self.session.is_some() && !matches!(self.screen, Screen::Summary) {
            self.finish(false);
        }
        self.session = None;
    }

    fn request_quit(&mut self) {
        self.leave_session();
        self.quit = true;
    }

    fn next_deadline(&self) -> Option<Instant> {
        match self.screen {
            Screen::Countdown { next, .. } => Some(next),
            Screen::Game => self.session.as_ref().and_then(|s| s.mode.deadline()),
            _ => None,
        }
    }

    fn tick(&mut self, now: Instant) {
        match self.screen {
            Screen::Countdown { left, next } if now >= next => {
                if left <= 1 {
                    self.screen = Screen::Game;
                    self.begin_trial(now);
                } else {
                    self.screen = Screen::Countdown { left: left - 1, next: next + Duration::from_secs(1) };
                }
                self.dirty = true;
            }
            Screen::Game => {
                if let Some(s) = self.session.as_mut() {
                    let step = s.mode.tick(now);
                    self.on_step(step, now);
                }
            }
            _ => {}
        }
    }

    /// Justo después de dibujar y vaciar el cuadro.
    fn after_draw(&mut self, now: Instant) {
        if self.screen == Screen::Game {
            let cursor = self.mouse.pos;
            if let Some(s) = self.session.as_mut() {
                s.mode.drawn(now, cursor);
            }
        }
    }

    fn session_active(&self) -> bool {
        matches!(self.screen, Screen::Countdown { .. } | Screen::Game | Screen::Paused(_))
    }

    fn handle(&mut self, ev: input::Timed) {
        match ev.input {
            Input::Key(k) => {
                if k.code == KeyCode::Char('q')
                    || (k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL))
                {
                    self.request_quit();
                    return;
                }
                self.on_key(k.code, ev.at);
            }
            Input::Click { x, y } => {
                if let (Screen::Game, Some(s)) = (self.screen, self.session.as_mut()) {
                    let step = s.mode.respond(Response::Click { x, y }, ev.at);
                    self.on_step(step, ev.at);
                }
            }
            // Solo actualiza la posición del cursor (ya hecho en input); no redibuja.
            Input::Move => {}
            Input::Resize(w, h) => {
                self.size = Rect { x: 0, y: 0, width: w, height: h };
                self.dirty = true;
                if self.session_active() {
                    if self.too_small() {
                        self.pause(PauseReason::TooSmall);
                    } else if self.screen == Screen::Game {
                        // El intento se dibujó con otro tamaño: no vale.
                        self.discard_trial();
                        self.begin_trial(ev.at);
                    }
                }
            }
            Input::FocusLost => {
                if matches!(self.screen, Screen::Game | Screen::Countdown { .. }) {
                    self.pause(PauseReason::FocusLost);
                }
            }
            Input::FocusGained => self.dirty = true,
        }
    }

    fn on_key(&mut self, code: KeyCode, at: Instant) {
        match self.screen {
            Screen::Menu => match code {
                KeyCode::Up | KeyCode::Char('k') => self.menu_idx = (self.menu_idx + MENU.len() - 1) % MENU.len(),
                KeyCode::Down | KeyCode::Char('j') => self.menu_idx = (self.menu_idx + 1) % MENU.len(),
                KeyCode::Enter | KeyCode::Char(' ') => self.select(at),
                KeyCode::Esc => self.quit = true,
                _ => return,
            },
            Screen::Countdown { .. } => match code {
                KeyCode::Esc => self.pause(PauseReason::User),
                _ => return,
            },
            Screen::Game => match code {
                KeyCode::Esc => self.pause(PauseReason::User),
                KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
                    if let Some(s) = self.session.as_mut() {
                        let step = s.mode.respond(Response::Key(c as usize - '0' as usize), at);
                        self.on_step(step, at);
                    }
                    return;
                }
                _ => return,
            },
            Screen::Paused(_) => match code {
                KeyCode::Enter | KeyCode::Char(' ') => self.resume(at),
                KeyCode::Esc => {
                    self.leave_session();
                    self.screen = Screen::Menu;
                }
                _ => return,
            },
            Screen::Summary => match code {
                KeyCode::Enter | KeyCode::Char(' ') => self.start_session(self.last_baseline, at),
                KeyCode::Char('p') => {
                    self.session = None;
                    self.open_progress();
                }
                KeyCode::Esc => {
                    self.session = None;
                    self.screen = Screen::Menu;
                }
                _ => return,
            },
            Screen::Progress => match code {
                KeyCode::Tab | KeyCode::Left | KeyCode::Right | KeyCode::Char('b') => {
                    if let Some(p) = self.progress.as_mut() {
                        p.baselines = !p.baselines;
                    }
                }
                KeyCode::Esc | KeyCode::Enter => {
                    self.progress = None;
                    self.screen = Screen::Menu;
                }
                _ => return,
            },
        }
        self.dirty = true;
    }

    fn select(&mut self, now: Instant) {
        match self.menu_idx {
            0 => self.start_session(false, now),
            1 => self.start_session(true, now),
            2 => self.open_progress(),
            3 => {
                self.cfg.difficulty = match self.cfg.difficulty {
                    Difficulty::Fixed => Difficulty::Adaptive,
                    Difficulty::Adaptive => Difficulty::Fixed,
                }
            }
            _ => self.quit = true,
        }
    }
}

/// Bucle dirigido por eventos: dibuja solo cuando algo cambió y duerme en `poll` hasta
/// el próximo evento o plazo, así que en reposo no consume CPU.
pub fn run(term: &mut Tui, app: &mut App, start: Option<bool>) -> io::Result<()> {
    if app.cfg.pixel_mouse {
        term::request_pixel_mouse();
    }
    if let Some(baseline) = start {
        app.start_session(baseline, Instant::now());
    }
    while !app.quit {
        if app.dirty {
            term.draw(|f| render::draw(f, app))?;
            // `draw` vacía (flush) el búfer antes de volver: este es el inicio del estímulo.
            app.after_draw(Instant::now());
            app.dirty = false;
        }
        let timeout = app
            .next_deadline()
            .map(|d| d.saturating_duration_since(Instant::now()))
            .unwrap_or(Duration::from_secs(3600));
        if let Some(ev) = input::read(timeout, &mut app.mouse)? {
            app.handle(ev);
        }
        app.tick(Instant::now());
    }
    Ok(())
}
