//! Reacción pura y Go/No-Go: un solo recuadro en el centro tras una espera al azar.
//! En Reacción pura siempre hay que responder. En Go/No-Go el recuadro relleno (go) pide
//! responder y el hueco (no-go) pide no hacerlo. Responde con espacio, un número o un clic.

use super::decision::catch_schedule;
use super::{Flash, FlashKind, GameMode, Method, Outcome, Params, Response, Step, TrialRecord};
use crate::config::{Config, Fill};
use crate::render;
use crate::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Idle,
    Fixation { until: Instant },
    Pending,
    Stimulus { onset: Instant, deadline: Instant },
}

pub struct Single {
    rng: fastrand::Rng,
    gonogo: bool,
    anticipation: Duration,
    fixation: (u64, u64),
    timeout: Duration,
    size: (u16, u16),
    go_fill: Fill,
    nogo_fill: Fill,
    /// Qué intentos son no-go, en orden.
    nogo_schedule: Vec<bool>,
    completed: usize,
    phase: Phase,
    rect: Rect,
    nogo: bool,
    flashes: Vec<Flash>,
}

impl Single {
    /// `nogo_ratio` 0 es Reacción pura.
    pub fn new(cfg: &Config, mut rng: fastrand::Rng, nogo_ratio: f64) -> Single {
        let mut nogo_schedule = vec![false; cfg.warmup_trials];
        nogo_schedule.extend(catch_schedule(cfg.session_trials(), nogo_ratio, &mut rng));
        Single {
            rng,
            gonogo: nogo_ratio > 0.0,
            anticipation: Duration::from_millis(cfg.anticipation_ms),
            fixation: (cfg.fixation_min_ms.max(800), cfg.fixation_max_ms.max(2500)),
            timeout: Duration::from_millis(cfg.go_timeout_ms),
            size: (cfg.box_w, cfg.box_h),
            go_fill: cfg.target_fill,
            nogo_fill: cfg.distractor_fill,
            nogo_schedule,
            completed: 0,
            phase: Phase::Idle,
            rect: Rect::default(),
            nogo: false,
            flashes: Vec::new(),
        }
    }

    fn record(&mut self, outcome: Outcome, rt: Option<Duration>, method: Method) -> Step {
        let rec = TrialRecord {
            index: self.completed,
            level: None,
            n_boxes: 1,
            target_present: !self.nogo,
            target_w: self.rect.width,
            target_h: self.rect.height,
            cursor_dist: None,
            rt_ms: rt.map(|d| d.as_secs_f64() * 1000.0),
            outcome,
            method,
            anticipation: outcome == Outcome::Anticipation,
            start: None,
            target_pos: None,
            click: None,
            score: None,
            radius: None,
            warmup: false,
        };
        let kind = match outcome {
            Outcome::Hit => Some(FlashKind::Hit),
            Outcome::FalseAlarm | Outcome::Anticipation => Some(FlashKind::Wrong),
            Outcome::Omission => Some(FlashKind::Reveal),
            _ => None,
        };
        if let Some(kind) = kind {
            self.flashes.push(Flash { kind, rect: self.rect });
        }
        self.completed += 1;
        self.phase = Phase::Idle;
        Step::Done(rec)
    }
}

impl GameMode for Single {
    fn id(&self) -> &'static str {
        if self.gonogo { "gonogo" } else { "reaction" }
    }

    fn start_trial(&mut self, area: Rect, _params: &Params, now: Instant, _cursor: Option<(f32, f32)>) {
        let (w, h) = (self.size.0.min(area.width), self.size.1.min(area.height));
        self.rect = Rect { x: area.x + (area.width - w) / 2, y: area.y + (area.height - h) / 2, width: w, height: h };
        self.nogo = self.nogo_schedule.get(self.completed).copied().unwrap_or(false);
        let wait = self.rng.u64(self.fixation.0..=self.fixation.1);
        self.phase = Phase::Fixation { until: now + Duration::from_millis(wait) };
    }

    fn deadline(&self) -> Option<Instant> {
        match self.phase {
            Phase::Fixation { until } => Some(until),
            Phase::Stimulus { deadline, .. } => Some(deadline),
            _ => None,
        }
    }

    fn tick(&mut self, now: Instant) -> Step {
        match self.phase {
            Phase::Fixation { until } if now >= until => {
                self.phase = Phase::Pending;
                Step::Redraw
            }
            Phase::Stimulus { deadline, .. } if now >= deadline => {
                let outcome = if self.nogo { Outcome::CorrectRejection } else { Outcome::Omission };
                self.record(outcome, None, Method::None)
            }
            _ => Step::Idle,
        }
    }

    fn drawn(&mut self, now: Instant, _cursor: Option<(f32, f32)>) {
        if self.phase == Phase::Pending {
            self.phase = Phase::Stimulus { onset: now, deadline: now + self.timeout };
        }
    }

    fn respond(&mut self, r: Response, at: Instant) -> Step {
        let method = match r {
            Response::Click { .. } => Method::Mouse,
            Response::Key(_) | Response::Press => Method::Keyboard,
        };
        match self.phase {
            Phase::Fixation { .. } | Phase::Pending => self.record(Outcome::Anticipation, None, method),
            Phase::Stimulus { onset, .. } => {
                let rt = at.saturating_duration_since(onset);
                let outcome = if rt < self.anticipation {
                    Outcome::Anticipation
                } else if self.nogo {
                    Outcome::FalseAlarm
                } else {
                    Outcome::Hit
                };
                self.record(outcome, Some(rt), method)
            }
            Phase::Idle => Step::Idle,
        }
    }

    fn abort(&mut self) {
        self.phase = Phase::Idle;
    }

    fn in_trial(&self) -> bool {
        self.phase != Phase::Idle
    }

    fn render(&self, buf: &mut Buffer, area: Rect, theme: &Theme) {
        match self.phase {
            Phase::Fixation { .. } => render::fixation(buf, area, theme),
            Phase::Pending | Phase::Stimulus { .. } => {
                let (fill, color) =
                    if self.nogo { (self.nogo_fill, theme.distractor) } else { (self.go_fill, theme.target) };
                render::draw_box_themed(buf, self.rect, fill, color, None, theme);
            }
            Phase::Idle => {}
        }
    }

    fn take_flashes(&mut self) -> Vec<Flash> {
        std::mem::take(&mut self.flashes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GameKind;

    fn params() -> Params {
        Params { box_w: 6, box_h: 3, min_boxes: 1, max_boxes: 1, timeout: Duration::from_secs(1), level: None }
    }

    fn run(m: &mut Single, t0: Instant, response: Option<Duration>) -> TrialRecord {
        m.start_trial(Rect::new(0, 0, 80, 24), &params(), t0, None);
        let until = m.deadline().unwrap();
        assert_eq!(m.tick(until), Step::Redraw);
        m.drawn(until, None);
        let step = match response {
            Some(d) => m.respond(Response::Press, until + d),
            None => m.tick(m.deadline().unwrap()),
        };
        let Step::Done(rec) = step else { panic!("sin resultado") };
        rec
    }

    #[test]
    fn reaction_hit_omission_and_early_press() {
        let cfg = Config { mode: GameKind::Reaction, ..Config::default() };
        let mut m = Single::new(&cfg, fastrand::Rng::with_seed(1), 0.0);
        let t0 = Instant::now();
        let rec = run(&mut m, t0, Some(Duration::from_millis(260)));
        assert_eq!(rec.outcome, Outcome::Hit);
        assert!((rec.rt_ms.unwrap() - 260.0).abs() < 0.01);
        assert_eq!(run(&mut m, t0, None).outcome, Outcome::Omission);
        assert_eq!(run(&mut m, t0, Some(Duration::from_millis(50))).outcome, Outcome::Anticipation);
        m.start_trial(Rect::new(0, 0, 80, 24), &params(), t0, None);
        let Step::Done(rec) = m.respond(Response::Key(1), t0) else { panic!() };
        assert_eq!(rec.outcome, Outcome::Anticipation);
        assert_eq!(m.id(), "reaction");
    }

    #[test]
    fn gonogo_schedule_and_outcomes() {
        let cfg = Config { mode: GameKind::GoNoGo, gonogo_trials: 20, warmup_trials: 0, ..Config::default() };
        let mut m = Single::new(&cfg, fastrand::Rng::with_seed(3), 0.25);
        assert_eq!(m.nogo_schedule.iter().filter(|x| **x).count(), 5);
        let t0 = Instant::now();
        let (mut fa, mut cr, mut hits) = (0, 0, 0);
        for i in 0..20 {
            let respond = i % 2 == 0;
            let rec = run(&mut m, t0, respond.then_some(Duration::from_millis(300)));
            match rec.outcome {
                Outcome::FalseAlarm => fa += 1,
                Outcome::CorrectRejection => cr += 1,
                Outcome::Hit => hits += 1,
                Outcome::Omission => {}
                o => panic!("inesperado {o:?}"),
            }
            assert_eq!(rec.target_present, !m.nogo);
        }
        assert_eq!(fa + cr, 5);
        assert!(hits > 0);
        assert_eq!(m.id(), "gonogo");
    }
}
