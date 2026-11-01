//! Estado de la aplicación, pantallas y bucle de eventos.

use crate::aim::{self, AimReport};
use crate::ambient::Ambient;
use crate::config::{Config, Difficulty, GameKind};
use crate::difficulty::{self, Staircase};
use crate::history::{self, SessionRecord};
use crate::input::{self, Input, Mouse};
use crate::modes::{self, Flash, GameMode, Outcome, Params, Response, Step, TrialRecord};
use crate::render;
use crate::report;
use crate::stats::{self, Stats};
use crate::term::{self, Tui};
use crate::theme::Theme;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItem {
    Start,
    Mode,
    Difficulty,
    Baseline,
    Plan,
    Progress,
    Aim,
    Quit,
}

pub const MENU: [MenuItem; 8] = [
    MenuItem::Start,
    MenuItem::Mode,
    MenuItem::Difficulty,
    MenuItem::Baseline,
    MenuItem::Plan,
    MenuItem::Progress,
    MenuItem::Aim,
    MenuItem::Quit,
];

/// Sesiones recientes que entran al análisis de puntería.
const AIM_SESSIONS: usize = 30;

/// Vistas de la pantalla de progreso: (modo, baseline).
pub const PROGRESS_VIEWS: [(GameKind, bool); 8] = [
    (GameKind::Decision, false),
    (GameKind::Decision, true),
    (GameKind::Targets, false),
    (GameKind::Flick, false),
    (GameKind::Flick, true),
    (GameKind::Tracking, false),
    (GameKind::Reaction, false),
    (GameKind::GoNoGo, false),
];

const COUNTDOWN: u8 = 3;
/// Duración de un efecto y separación entre sus cuadros.
pub const EFFECT: Duration = Duration::from_millis(380);
const EFFECT_FRAME: Duration = Duration::from_millis(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Menu,
    Countdown { left: u8, next: Instant },
    Game,
    Paused(PauseReason),
    Summary,
    Progress,
    Aim,
    /// Vista previa del plan de entrenamiento.
    Plan,
    /// Plan terminado: resumen de los bloques.
    PlanDone,
}

/// Plan de entrenamiento en curso.
pub struct PlanState {
    pub blocks: Vec<crate::plan::Block>,
    pub current: usize,
    /// Métrica principal de cada bloque terminado: (modo, nombre, valor).
    pub results: Vec<(String, String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseReason {
    User,
    FocusLost,
    TooSmall,
}

/// Reloj de una sesión por tiempo; se detiene durante la pausa y la cuenta regresiva.
#[derive(Debug, Clone, Copy)]
pub struct Timer {
    pub total: Duration,
    remaining: Duration,
    running_since: Option<Instant>,
}

impl Timer {
    pub fn new(total: Duration) -> Timer {
        Timer { total, remaining: total, running_since: None }
    }

    pub fn start(&mut self, now: Instant) {
        self.running_since.get_or_insert(now);
    }

    pub fn stop(&mut self, now: Instant) {
        if let Some(t) = self.running_since.take() {
            self.remaining = self.remaining.saturating_sub(now.saturating_duration_since(t));
        }
    }

    pub fn left(&self, now: Instant) -> Duration {
        match self.running_since {
            Some(t) => self.remaining.saturating_sub(now.saturating_duration_since(t)),
            None => self.remaining,
        }
    }

    pub fn elapsed(&self, now: Instant) -> Duration {
        self.total - self.left(now)
    }

    /// Segundos enteros que se muestran (redondeo hacia arriba).
    pub fn shown_secs(&self, now: Instant) -> u64 {
        self.left(now).as_millis().div_ceil(1000) as u64
    }

    /// Próximo cambio del segundo mostrado o fin del tiempo.
    pub fn next_change(&self, now: Instant) -> Option<Instant> {
        self.running_since?;
        let left = self.left(now).as_millis() as u64;
        let step = match left % 1000 {
            0 => left.min(1000),
            r => r,
        };
        Some(now + Duration::from_millis(step))
    }
}

pub struct Session {
    pub mode: Box<dyn GameMode>,
    pub cfg: Config,
    pub baseline: bool,
    pub records: Vec<TrialRecord>,
    /// Intentos que cuentan para terminar (en Flick los clics fallidos no cuentan).
    pub counted: usize,
    /// Intentos descartados por pérdida de foco, cambio de tamaño o pausa.
    pub discarded: usize,
    pub last: Option<TrialRecord>,
    pub stats: Option<Stats>,
    /// Estadísticas de la sesión comparable anterior, para mostrar la diferencia.
    pub prev: Option<Stats>,
    /// Solo en dificultad adaptativa.
    pub stair: Option<Staircase>,
    /// Solo en sesiones por tiempo (modo Blancos).
    pub timer: Option<Timer>,
    /// Intentos de calentamiento que faltan.
    pub warmup_left: usize,
    shown_secs: u64,
    pub started: Instant,
}

impl Session {
    fn params(&self) -> Params {
        if self.cfg.mode == GameKind::Flick {
            let fl = difficulty::flick_level(self.level().unwrap_or(3));
            return Params {
                box_w: fl.w,
                box_h: fl.h,
                min_boxes: 1,
                max_boxes: 1,
                timeout: Duration::from_millis(self.cfg.flick_timeout_ms),
                level: self.level(),
            };
        }
        if matches!(self.cfg.mode, GameKind::Reaction | GameKind::GoNoGo) {
            let timeout = match self.cfg.mode {
                GameKind::GoNoGo => difficulty::gonogo_window(self.level().unwrap_or(3)),
                _ => Duration::from_millis(self.cfg.go_timeout_ms),
            };
            return Params {
                box_w: self.cfg.box_w,
                box_h: self.cfg.box_h,
                min_boxes: 1,
                max_boxes: 1,
                timeout,
                level: self.level(),
            };
        }
        if self.cfg.mode == GameKind::Tracking {
            let tl = difficulty::tracking_level(self.level().unwrap_or(3));
            return Params {
                box_w: tl.w,
                box_h: tl.h,
                min_boxes: 1,
                max_boxes: 1,
                timeout: Duration::from_secs(self.cfg.tracking_secs),
                level: self.level(),
            };
        }
        if self.cfg.mode == GameKind::Targets {
            return Params {
                box_w: self.cfg.targets_w,
                box_h: self.cfg.targets_h,
                min_boxes: self.cfg.targets_count,
                max_boxes: self.cfg.targets_count,
                timeout: Duration::from_secs(self.cfg.targets_secs),
                level: self.level(),
            };
        }
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
        self.timer.is_none() && self.counted >= self.cfg.session_trials()
    }

    pub fn final_level(&self) -> Option<usize> {
        self.stair.as_ref().map(|s| s.level)
    }

    /// Nivel actual: el de la escalera o, en Blancos fijo, el elegido.
    pub fn level(&self) -> Option<usize> {
        match (&self.stair, self.cfg.mode) {
            (Some(st), _) => Some(st.level),
            (None, _) => self.cfg.fixed_level(),
        }
    }

    /// Mueve la escalera con el resultado de un intento.
    fn update_stair(&mut self, rec: &TrialRecord) {
        let Some(st) = self.stair.as_mut() else { return };
        match self.cfg.mode {
            GameKind::Targets | GameKind::Flick => {
                match difficulty::speed_signal(rec.outcome, rec.rt_ms, self.cfg.adapt_ms().unwrap_or(900)) {
                    Some(ok) => st.signal(ok),
                    None => st.reset_streak(),
                }
            }
            GameKind::Tracking => match difficulty::tracking_signal(rec.score) {
                Some(ok) => st.signal(ok),
                None => st.reset_streak(),
            },
            _ => st.update(rec.outcome),
        }
    }

    /// Intentos contados (sin calentamiento) con este resultado.
    pub fn count(&self, o: Outcome) -> usize {
        self.records.iter().filter(|r| !r.warmup && r.outcome == o).count()
    }

    /// Calentamiento en curso: (hechos, total).
    pub fn warming_up(&self) -> Option<(usize, usize)> {
        (self.warmup_left > 0).then(|| (self.cfg.warmup_trials - self.warmup_left, self.cfg.warmup_trials))
    }

    fn to_record(&self, stats: Stats, incomplete: bool, pixel_mouse: bool, now: Instant) -> SessionRecord {
        SessionRecord {
            version: 1,
            date: history::now_iso(),
            mode: self.mode.id().to_string(),
            difficulty: self.cfg.mode_difficulty(),
            baseline: self.baseline,
            incomplete,
            discarded: self.discarded,
            final_level: self.final_level(),
            duration_s: self.timer.map(|t| t.elapsed(now).as_secs_f64()),
            pixel_mouse,
            config: self.cfg.clone(),
            stats,
            trials: self.records.clone(),
        }
    }
}

/// Última sesión completa comparable: mismo modo, mismo tipo y misma dificultad (en
/// Blancos fijo, también el mismo nivel, porque cambia qué tan lejos salen los blancos).
pub fn last_comparable<'a>(history: &'a [SessionRecord], cfg: &Config, baseline: bool) -> Option<&'a SessionRecord> {
    let mode = cfg.mode;
    let difficulty = cfg.mode_difficulty();
    history.iter().rev().find(|s| {
        let same_difficulty = match mode {
            GameKind::Decision => baseline || s.difficulty == difficulty,
            // Con niveles: misma dificultad y, si es fija, el mismo nivel.
            _ if cfg.levels().is_some() => {
                let their_level = Config { mode, ..s.config.clone() }.fixed_level();
                s.difficulty == difficulty && (difficulty == Difficulty::Adaptive || their_level == cfg.fixed_level())
            }
            _ => true,
        };
        // Los bloques del plan son más cortos: solo se comparan entre ellos.
        let same_kind = s.config.plan_block == cfg.plan_block;
        !s.incomplete && s.mode == mode.id() && s.baseline == baseline && same_difficulty && same_kind
    })
}

/// Nivel final de la última sesión adaptativa del modo, para empezar desde ahí.
pub fn last_adaptive_level(history: &[SessionRecord], mode: GameKind) -> Option<usize> {
    history
        .iter()
        .rev()
        .find(|s| !s.incomplete && !s.baseline && s.mode == mode.id() && s.difficulty == Difficulty::Adaptive)
        .and_then(|s| s.final_level)
}

pub struct App {
    pub cfg: Config,
    pub theme: Theme,
    pub screen: Screen,
    pub menu_idx: usize,
    pub session: Option<Session>,
    pub mouse: Mouse,
    pub size: Rect,
    /// Sesiones guardadas (sin el detalle de intentos), para progreso y comparaciones.
    pub history: Vec<SessionRecord>,
    pub bad_lines: usize,
    pub progress_view: usize,
    /// Dificultad que filtra el progreso (`None`: todas).
    pub progress_filter: Option<String>,
    /// Progreso agrupado por semana en vez de por sesión.
    pub progress_weekly: bool,
    pub plan: Option<PlanState>,
    /// Análisis de puntería abierto y a qué pantalla volver.
    pub aim: Option<AimReport>,
    aim_back: Screen,
    /// Aviso de una sola vez (p. ej. un error al guardar).
    pub notice: Option<String>,
    /// Efectos en curso y cuándo empezaron.
    pub effects: Vec<(Flash, Instant)>,
    last_effect_frame: Option<Instant>,
    /// Salida falsa de fondo del estilo de código.
    pub ambient: Ambient,
    /// Prompt que imita al de la terminal en el estilo de código.
    pub prompt: String,
    history_path: PathBuf,
    /// Tipo de la última sesión, para repetirla desde el resumen.
    last_baseline: bool,
    pub quit: bool,
    dirty: bool,
}

impl App {
    pub fn new(cfg: Config, size: Rect) -> App {
        App::with_history(cfg, size, history::history_path())
    }

    pub fn with_history(cfg: Config, size: Rect, history_path: PathBuf) -> App {
        let theme = Theme::from_config(&cfg);
        let mouse = Mouse::new(cfg.pixel_mouse, size.width, size.height);
        let (mut history, bad_lines) = history::load(&history_path);
        for s in &mut history {
            s.trials = Vec::new();
        }
        App {
            cfg,
            theme,
            screen: Screen::Menu,
            menu_idx: 0,
            session: None,
            mouse,
            size,
            history,
            bad_lines,
            progress_view: 0,
            progress_filter: None,
            progress_weekly: false,
            plan: None,
            aim: None,
            aim_back: Screen::Menu,
            notice: None,
            effects: Vec::new(),
            last_effect_frame: None,
            ambient: Ambient::new(fastrand::Rng::new()),
            prompt: shell_prompt(),
            history_path,
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
        self.start_with(cfg, baseline, now);
    }

    fn start_with(&mut self, cfg: Config, baseline: bool, now: Instant) {
        let mode = modes::create(&cfg, fastrand::Rng::new());
        // La escalera empieza un nivel por debajo de donde terminó la sesión anterior.
        let stair = (cfg.mode.adaptive() && cfg.mode_difficulty() == Difficulty::Adaptive)
            .then(|| Staircase::resume(last_adaptive_level(&self.history, cfg.mode), cfg.start_level));
        let timer = cfg.session_secs().map(|secs| Timer::new(Duration::from_secs(secs)));
        let prev = last_comparable(&self.history, &cfg, baseline).map(|s| s.stats.clone());
        let cfg_warmup = cfg.warmup_trials;
        self.session = Some(Session {
            mode,
            cfg,
            baseline,
            records: Vec::new(),
            counted: 0,
            discarded: 0,
            last: None,
            stats: None,
            prev,
            stair,
            timer,
            warmup_left: cfg_warmup,
            shown_secs: 0,
            started: now,
        });
        self.last_baseline = baseline;
        self.notice = None;
        self.screen = Screen::Countdown { left: COUNTDOWN, next: now + Duration::from_secs(1) };
        self.dirty = true;
    }

    fn begin_trial(&mut self, now: Instant) {
        let area = render::play_area(self.size);
        let cursor = self.mouse.pos;
        if let Some(s) = self.session.as_mut() {
            let params = s.params();
            s.mode.start_trial(area, &params, now, cursor);
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

    fn pause(&mut self, reason: PauseReason, now: Instant) {
        self.discard_trial();
        self.effects.clear();
        if let Some(t) = self.session.as_mut().and_then(|s| s.timer.as_mut()) {
            t.stop(now);
        }
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
                let flashes = s.mode.take_flashes();
                if self.theme.effects {
                    self.effects.extend(flashes.into_iter().map(|f| (f, now)));
                }
                s.update_stair(&rec);
                if s.warmup_left > 0 {
                    rec.warmup = true;
                    if s.mode.counts(&rec) {
                        s.warmup_left -= 1;
                        if s.warmup_left == 0
                            && let Some(t) = s.timer.as_mut()
                        {
                            t.start(now);
                        }
                    }
                } else if s.mode.counts(&rec) {
                    s.counted += 1;
                }
                s.last = Some(rec.clone());
                s.records.push(rec);
                if s.done() {
                    self.finish(true, now);
                } else {
                    self.begin_trial(now);
                }
                self.dirty = true;
            }
        }
    }

    /// Cierra la sesión activa. Las incompletas se guardan marcadas para excluirlas del progreso.
    fn finish(&mut self, complete: bool, now: Instant) {
        let pixel = self.mouse.pixel == crate::input::PixelMode::On;
        let Some(s) = self.session.as_mut() else { return };
        if let Some(t) = s.timer.as_mut() {
            t.stop(now);
        }
        s.mode.abort();
        self.effects.clear();
        let st = stats::compute(&s.records);
        if complete || !s.records.is_empty() {
            let mut rec = s.to_record(st.clone(), !complete, pixel, now);
            match history::append(&self.history_path, &rec) {
                Ok(()) => {
                    rec.trials = Vec::new();
                    self.history.push(rec);
                }
                Err(e) => self.notice = Some(format!("no se pudo guardar: {e}")),
            }
        }
        if complete {
            if let Some(p) = self.plan.as_mut() {
                let (metrics, _) = report::summary(s, &st);
                let main = metrics.iter().find(|m| m.strong).or(metrics.first());
                p.results.push((
                    s.cfg.mode.label().to_string(),
                    main.map(|m| m.label.to_string()).unwrap_or_default(),
                    main.map(|m| m.value.clone()).unwrap_or_default(),
                ));
            }
            s.stats = Some(st);
            self.screen = Screen::Summary;
        } else {
            self.session = None;
            // Salir a mitad de un bloque abandona el plan.
            self.plan = None;
        }
    }

    /// Arma el plan de hoy con el historial y el análisis de puntería.
    pub fn open_plan(&mut self) {
        let aim = self.recent_aim();
        let blocks = crate::plan::build(&self.cfg, &self.history, &aim, &history::now_iso());
        self.plan = Some(PlanState { blocks, current: 0, results: Vec::new() });
        self.screen = Screen::Plan;
    }

    fn start_block(&mut self, i: usize, now: Instant) {
        let Some(cfg) = self.plan.as_mut().and_then(|p| {
            p.current = i;
            p.blocks.get(i).map(|b| b.cfg.clone())
        }) else {
            return;
        };
        self.start_with(cfg, false, now);
    }

    /// Bloque siguiente del plan, si queda alguno.
    pub fn next_block(&self) -> Option<usize> {
        let p = self.plan.as_ref()?;
        (p.current + 1 < p.blocks.len()).then_some(p.current + 1)
    }

    fn open_progress(&mut self) {
        self.progress_view = PROGRESS_VIEWS.iter().position(|(m, _)| *m == self.cfg.mode).unwrap_or(0);
        self.progress_filter = None;
        self.screen = Screen::Progress;
    }

    /// Sesiones completas de la vista de progreso actual, sin filtrar por dificultad.
    fn progress_sessions(&self) -> impl Iterator<Item = &SessionRecord> {
        let (kind, baseline) = PROGRESS_VIEWS[self.progress_view % PROGRESS_VIEWS.len()];
        self.history.iter().filter(move |s| !s.incomplete && s.baseline == baseline && s.mode == kind.id())
    }

    /// Sesiones que muestra el progreso: las de la vista, con el filtro de dificultad.
    pub fn progress_list(&self) -> Vec<&SessionRecord> {
        self.progress_sessions()
            .filter(|s| self.progress_filter.is_none() || report::difficulty_key(s) == self.progress_filter)
            .collect()
    }

    /// Filas del progreso: una por sesión o, con `w`, una por semana.
    pub fn progress_rows(&self) -> Vec<report::ProgressRow> {
        let list = self.progress_list();
        if self.progress_weekly { report::weekly_rows(&list) } else { report::session_rows(&list) }
    }

    /// Dificultades presentes en la vista, en orden de aparición.
    pub fn progress_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = Vec::new();
        for k in self.progress_sessions().filter_map(report::difficulty_key) {
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
        keys
    }

    /// Todas → cada dificultad presente → todas.
    fn cycle_progress_filter(&mut self) {
        let keys = self.progress_keys();
        self.progress_filter = match &self.progress_filter {
            None => keys.first().cloned(),
            Some(cur) => keys.iter().position(|k| k == cur).and_then(|i| keys.get(i + 1)).cloned(),
        };
    }

    /// Analiza la puntería con los intentos de las últimas sesiones completas (de todos
    /// los modos con mouse), leyendo el historial con el detalle de cada intento.
    fn open_aim(&mut self) {
        self.aim = Some(self.recent_aim());
        self.aim_back = self.screen;
        self.screen = Screen::Aim;
    }

    fn recent_aim(&self) -> AimReport {
        let (all, _) = history::load(&self.history_path);
        let recent: Vec<&SessionRecord> = all
            .iter()
            .rev()
            .filter(|s| !s.incomplete && s.trials.iter().any(|t| t.start.is_some()))
            .take(AIM_SESSIONS)
            .collect();
        aim::analyze(recent.iter().flat_map(|s| s.trials.iter()), recent.len())
    }

    fn leave_session(&mut self, now: Instant) {
        self.discard_trial();
        if self.session.is_some() && !matches!(self.screen, Screen::Summary) {
            self.finish(false, now);
        }
        self.session = None;
    }

    fn request_quit(&mut self, now: Instant) {
        self.leave_session(now);
        self.quit = true;
    }

    /// El texto de fondo avanza solo fuera del juego, para no distraer durante los intentos.
    fn sync_ambient(&mut self, now: Instant) {
        if self.theme.ambient.is_some() && (self.screen != Screen::Game || self.cfg.ambient_motion) {
            self.ambient.schedule(now);
        } else {
            self.ambient.stop();
        }
    }

    fn next_deadline(&self, now: Instant) -> Option<Instant> {
        let own = self.screen_deadline(now);
        [own, self.ambient.due()].into_iter().flatten().min()
    }

    fn screen_deadline(&self, now: Instant) -> Option<Instant> {
        match self.screen {
            Screen::Countdown { next, .. } => Some(next),
            Screen::Game => {
                let s = self.session.as_ref()?;
                let timer = s.timer.and_then(|t| t.next_change(now));
                let effect = (!self.effects.is_empty()).then(|| now + EFFECT_FRAME);
                [s.mode.deadline(), timer, effect].into_iter().flatten().min()
            }
            _ => None,
        }
    }

    fn tick(&mut self, now: Instant) {
        if self.ambient.advance(now) {
            self.dirty = true;
        }
        match self.screen {
            Screen::Countdown { left, next } if now >= next => {
                if left <= 1 {
                    self.screen = Screen::Game;
                    // Con calentamiento pendiente, el reloj espera a que termine.
                    if let Some(s) = self.session.as_mut().filter(|s| s.warmup_left == 0)
                        && let Some(t) = s.timer.as_mut()
                    {
                        t.start(now);
                    }
                    self.begin_trial(now);
                } else {
                    self.screen = Screen::Countdown { left: left - 1, next: next + Duration::from_secs(1) };
                }
                self.dirty = true;
            }
            Screen::Game => {
                self.advance_effects(now);
                let Some(s) = self.session.as_mut() else { return };
                if let Some(t) = s.timer {
                    if t.left(now).is_zero() {
                        self.finish(true, now);
                        self.dirty = true;
                        return;
                    }
                    let secs = t.shown_secs(now);
                    if secs != s.shown_secs {
                        s.shown_secs = secs;
                        self.dirty = true;
                    }
                }
                let step = s.mode.tick(now);
                self.on_step(step, now);
            }
            _ => {}
        }
    }

    /// Quita los efectos terminados y pide un cuadro nuevo como mucho cada `EFFECT_FRAME`,
    /// aunque lleguen muchos eventos de movimiento del mouse.
    fn advance_effects(&mut self, now: Instant) {
        if self.effects.is_empty() {
            return;
        }
        let before = self.effects.len();
        self.effects.retain(|(_, t)| now.saturating_duration_since(*t) < EFFECT);
        let due = self.last_effect_frame.is_none_or(|t| now.saturating_duration_since(t) >= EFFECT_FRAME);
        if due || self.effects.len() != before {
            self.last_effect_frame = Some(now);
            self.dirty = true;
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
                    self.request_quit(ev.at);
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
            // La posición ya quedó en `self.mouse`; solo la pasan los modos que la siguen.
            Input::Move => {
                if let (Screen::Game, Some(s), Some(p)) = (self.screen, self.session.as_mut(), self.mouse.pos) {
                    s.mode.on_move(p, ev.at);
                }
            }
            Input::Resize(w, h) => {
                self.size = Rect { x: 0, y: 0, width: w, height: h };
                self.dirty = true;
                if self.session_active() {
                    if self.too_small() {
                        self.pause(PauseReason::TooSmall, ev.at);
                    } else if self.screen == Screen::Game {
                        // El intento se dibujó con otro tamaño: no vale.
                        self.discard_trial();
                        self.effects.clear();
                        self.begin_trial(ev.at);
                    }
                }
            }
            Input::FocusLost => {
                if matches!(self.screen, Screen::Game | Screen::Countdown { .. }) {
                    self.pause(PauseReason::FocusLost, ev.at);
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
                KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right => self.select(code, at),
                KeyCode::Esc => self.quit = true,
                _ => return,
            },
            Screen::Countdown { .. } | Screen::Game => match code {
                KeyCode::Esc => self.pause(PauseReason::User, at),
                KeyCode::Char(' ') if self.screen == Screen::Game => {
                    if let Some(s) = self.session.as_mut() {
                        let step = s.mode.respond(Response::Press, at);
                        self.on_step(step, at);
                    }
                    return;
                }
                KeyCode::Char(c) if self.screen == Screen::Game && c.is_ascii_digit() && c != '0' => {
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
                    self.leave_session(at);
                    self.screen = Screen::Menu;
                }
                _ => return,
            },
            Screen::Summary if self.plan.is_some() => match code {
                KeyCode::Enter | KeyCode::Char(' ') => match self.next_block() {
                    Some(i) => self.start_block(i, at),
                    None => {
                        self.session = None;
                        self.screen = Screen::PlanDone;
                    }
                },
                KeyCode::Char('a') => self.open_aim(),
                KeyCode::Esc => {
                    self.session = None;
                    self.plan = None;
                    self.screen = Screen::Menu;
                }
                _ => return,
            },
            Screen::Plan => match code {
                KeyCode::Enter | KeyCode::Char(' ') => self.start_block(0, at),
                KeyCode::Esc => {
                    self.plan = None;
                    self.screen = Screen::Menu;
                }
                _ => return,
            },
            Screen::PlanDone => match code {
                KeyCode::Enter | KeyCode::Esc | KeyCode::Char(' ') => {
                    self.plan = None;
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
                KeyCode::Char('a') => self.open_aim(),
                KeyCode::Esc => {
                    self.session = None;
                    self.screen = Screen::Menu;
                }
                _ => return,
            },
            Screen::Progress => match code {
                KeyCode::Tab | KeyCode::Right => {
                    self.progress_view = (self.progress_view + 1) % PROGRESS_VIEWS.len();
                    self.progress_filter = None;
                }
                KeyCode::BackTab | KeyCode::Left => {
                    self.progress_view = (self.progress_view + PROGRESS_VIEWS.len() - 1) % PROGRESS_VIEWS.len();
                    self.progress_filter = None;
                }
                KeyCode::Char('d') => self.cycle_progress_filter(),
                KeyCode::Char('w') => self.progress_weekly = !self.progress_weekly,
                KeyCode::Esc | KeyCode::Enter => self.screen = Screen::Menu,
                _ => return,
            },
            Screen::Aim => match code {
                KeyCode::Esc | KeyCode::Enter => {
                    self.aim = None;
                    self.screen = self.aim_back;
                }
                _ => return,
            },
        }
        self.dirty = true;
    }

    fn select(&mut self, code: KeyCode, now: Instant) {
        let toggle = matches!(code, KeyCode::Left | KeyCode::Right);
        match MENU[self.menu_idx] {
            MenuItem::Mode if code == KeyCode::Left => self.cfg.mode = self.cfg.mode.prev(),
            MenuItem::Mode => self.cfg.mode = self.cfg.mode.next(),
            MenuItem::Difficulty if self.cfg.mode.adaptive() => self.cfg.cycle_difficulty(code != KeyCode::Left),
            _ if toggle => {}
            MenuItem::Start => self.start_session(false, now),
            MenuItem::Baseline => self.start_session(true, now),
            MenuItem::Progress => self.open_progress(),
            MenuItem::Aim => self.open_aim(),
            MenuItem::Plan => self.open_plan(),
            MenuItem::Quit => self.quit = true,
            MenuItem::Difficulty => {}
        }
    }
}

/// Prompt parecido al de la terminal: `PS C:\ruta>` en Windows, `~/ruta $` fuera.
fn shell_prompt() -> String {
    let dir = std::env::current_dir().map(|d| d.display().to_string()).unwrap_or_default();
    if cfg!(windows) { format!("PS {dir}> ") } else { format!("{dir} $ ") }
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
        let now = Instant::now();
        app.sync_ambient(now);
        let timeout =
            app.next_deadline(now).map(|d| d.saturating_duration_since(now)).unwrap_or(Duration::from_secs(3600));
        if let Some(ev) = input::read(timeout, &mut app.mouse)? {
            app.handle(ev);
        }
        app.tick(Instant::now());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_pauses_and_counts_down() {
        let t0 = Instant::now();
        let mut t = Timer::new(Duration::from_secs(30));
        assert_eq!(t.left(t0 + Duration::from_secs(5)), Duration::from_secs(30));
        t.start(t0);
        assert_eq!(t.shown_secs(t0 + Duration::from_millis(1500)), 29);
        t.stop(t0 + Duration::from_secs(10));
        assert_eq!(t.left(t0 + Duration::from_secs(99)), Duration::from_secs(20));
        t.start(t0 + Duration::from_secs(100));
        assert_eq!(t.left(t0 + Duration::from_secs(125)), Duration::ZERO);
        assert_eq!(t.elapsed(t0 + Duration::from_secs(125)), Duration::from_secs(30));
    }

    #[test]
    fn warmup_is_marked_and_does_not_count() {
        let cfg = Config { mode: GameKind::Reaction, reaction_trials: 2, warmup_trials: 2, ..Config::default() };
        let mut a = App::with_history(cfg, Rect::new(0, 0, 80, 24), std::env::temp_dir().join("rt-no-existe.jsonl"));
        let t0 = Instant::now();
        a.start_session(false, t0);
        a.screen = Screen::Game;
        let mk = |outcome| TrialRecord {
            index: 0,
            level: None,
            n_boxes: 1,
            target_present: true,
            target_w: 6,
            target_h: 3,
            cursor_dist: None,
            rt_ms: Some(300.0),
            outcome,
            method: crate::modes::Method::Keyboard,
            anticipation: false,
            start: None,
            target_pos: None,
            click: None,
            score: None,
            radius: None,
            warmup: false,
        };
        for _ in 0..3 {
            a.on_step(Step::Done(mk(Outcome::Hit)), t0);
        }
        let s = a.session.as_ref().unwrap();
        assert_eq!(s.warmup_left, 0);
        assert_eq!(s.counted, 1);
        assert_eq!(s.records.iter().filter(|r| r.warmup).count(), 2);
        assert!(!s.done());
    }

    #[test]
    fn timed_session_clock_starts_after_warmup() {
        let cfg = Config { mode: GameKind::Targets, warmup_trials: 1, ..Config::default() };
        let mut a = App::with_history(cfg, Rect::new(0, 0, 80, 24), std::env::temp_dir().join("rt-no-existe.jsonl"));
        let t0 = Instant::now();
        a.start_session(false, t0);
        a.screen = Screen::Countdown { left: 1, next: t0 };
        a.tick(t0);
        let left = |a: &App, at| a.session.as_ref().unwrap().timer.unwrap().left(at);
        assert_eq!(left(&a, t0 + Duration::from_secs(10)), Duration::from_secs(30));
        let rec = TrialRecord {
            index: 0,
            level: None,
            n_boxes: 3,
            target_present: true,
            target_w: 6,
            target_h: 3,
            cursor_dist: None,
            rt_ms: Some(400.0),
            outcome: Outcome::Hit,
            method: crate::modes::Method::Mouse,
            anticipation: false,
            start: None,
            target_pos: None,
            click: None,
            score: None,
            radius: None,
            warmup: false,
        };
        let t1 = t0 + Duration::from_secs(10);
        a.on_step(Step::Done(rec), t1);
        assert_eq!(left(&a, t1 + Duration::from_secs(5)), Duration::from_secs(25));
    }

    #[test]
    fn timer_wakes_on_each_second() {
        let t0 = Instant::now();
        let mut t = Timer::new(Duration::from_secs(30));
        assert_eq!(t.next_change(t0), None);
        t.start(t0);
        assert_eq!(t.next_change(t0), Some(t0 + Duration::from_secs(1)));
        let at = t0 + Duration::from_millis(1300);
        assert_eq!(t.next_change(at), Some(at + Duration::from_millis(700)));
    }

    fn rec(mode: GameKind, baseline: bool, incomplete: bool, difficulty: Difficulty, med: f64) -> SessionRecord {
        SessionRecord {
            version: 1,
            date: String::new(),
            mode: mode.id().into(),
            difficulty,
            baseline,
            incomplete,
            discarded: 0,
            final_level: None,
            duration_s: None,
            pixel_mouse: false,
            config: Config::default(),
            stats: Stats { median_ms: Some(med), ..Stats::default() },
            trials: vec![],
        }
    }

    #[test]
    fn comparable_session_matches_mode_type_and_difficulty() {
        use Difficulty::*;
        use GameKind::*;
        let h = vec![
            rec(Decision, false, false, Adaptive, 1.0),
            rec(Decision, false, false, Fixed, 2.0),
            rec(Targets, false, false, Adaptive, 3.0),
            rec(Decision, true, false, Fixed, 4.0),
            rec(Decision, false, true, Adaptive, 5.0),
        ];
        let med = |mode, baseline, difficulty| {
            let cfg = Config { mode, difficulty, targets_difficulty: difficulty, ..Config::default() };
            last_comparable(&h, &cfg, baseline).and_then(|s| s.stats.median_ms)
        };
        assert_eq!(med(Decision, false, Adaptive), Some(1.0));
        assert_eq!(med(Decision, false, Fixed), Some(2.0));
        assert_eq!(med(Targets, false, Adaptive), Some(3.0));
        // Blancos fijo no se compara con adaptativo, ni con otro nivel fijo.
        assert_eq!(med(Targets, false, Fixed), None);
        assert_eq!(med(Decision, true, Adaptive), Some(4.0));

        let mut fixed6 = rec(Targets, false, false, Fixed, 6.0);
        fixed6.config.targets_level = 6;
        let h = vec![fixed6];
        let at = |level| {
            let cfg = Config { mode: Targets, targets_difficulty: Fixed, targets_level: level, ..Config::default() };
            last_comparable(&h, &cfg, false).and_then(|s| s.stats.median_ms)
        };
        assert_eq!(at(6), Some(6.0));
        assert_eq!(at(3), None);
    }

    #[test]
    fn progress_filter_cycles_through_difficulties() {
        let mut a = App::with_history(Config::default(), Rect::new(0, 0, 80, 24), std::env::temp_dir().join("rt-no-existe.jsonl"));
        let mut easy = rec(GameKind::Targets, false, false, Difficulty::Fixed, 1.0);
        easy.config.targets_level = 1;
        let adaptive = rec(GameKind::Targets, false, false, Difficulty::Adaptive, 2.0);
        let mut easy2 = rec(GameKind::Targets, false, false, Difficulty::Fixed, 3.0);
        easy2.config.targets_level = 1;
        a.history = vec![easy, adaptive, easy2, rec(GameKind::Decision, false, false, Difficulty::Fixed, 9.0)];
        a.cfg.mode = GameKind::Targets;
        a.open_progress();
        assert_eq!(a.progress_keys(), vec!["fácil", "adaptativa"]);
        assert_eq!(a.progress_list().len(), 3);
        a.cycle_progress_filter();
        assert_eq!(a.progress_filter.as_deref(), Some("fácil"));
        assert_eq!(a.progress_list().iter().map(|s| s.stats.median_ms.unwrap()).collect::<Vec<_>>(), vec![1.0, 3.0]);
        a.cycle_progress_filter();
        assert_eq!(a.progress_list().len(), 1);
        a.cycle_progress_filter();
        assert_eq!(a.progress_filter, None);
    }

    #[test]
    fn plan_runs_blocks_in_order_and_collects_results() {
        let mut a = App::with_history(Config::default(), Rect::new(0, 0, 80, 24), std::env::temp_dir().join("rt-plan-no-existe.jsonl"));
        a.history_path = std::env::temp_dir().join(format!("rt-plan-{}.jsonl", std::process::id()));
        a.open_plan();
        assert_eq!(a.screen, Screen::Plan);
        let n = a.plan.as_ref().unwrap().blocks.len();
        assert_eq!(n, crate::plan::BLOCKS);
        let t0 = Instant::now();
        for i in 0..n {
            if i == 0 {
                a.on_key(KeyCode::Enter, t0);
            }
            let s = a.session.as_ref().unwrap();
            assert!(s.cfg.plan_block);
            assert_eq!(s.cfg.mode, a.plan.as_ref().unwrap().blocks[i].kind());
            a.finish(true, t0);
            assert_eq!(a.screen, Screen::Summary);
            assert_eq!(a.plan.as_ref().unwrap().results.len(), i + 1);
            a.on_key(KeyCode::Enter, t0);
        }
        assert_eq!(a.screen, Screen::PlanDone);
        a.on_key(KeyCode::Enter, t0);
        assert_eq!(a.screen, Screen::Menu);
        assert!(a.plan.is_none());
        let _ = std::fs::remove_file(&a.history_path);
    }

    #[test]
    fn adaptive_resumes_from_last_final_level() {
        let mut a = rec(GameKind::Targets, false, false, Difficulty::Adaptive, 1.0);
        a.final_level = Some(7);
        let mut b = rec(GameKind::Decision, false, false, Difficulty::Adaptive, 1.0);
        b.final_level = Some(2);
        let h = vec![a, b];
        assert_eq!(last_adaptive_level(&h, GameKind::Targets), Some(7));
        assert_eq!(last_adaptive_level(&h, GameKind::Decision), Some(2));
        assert_eq!(last_adaptive_level(&h, GameKind::Flick), None);
    }
}
