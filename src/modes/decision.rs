//! Modo Decisión: punto de fijación, espera aleatoria y de 3 a 5 recuadros; uno es el
//! objetivo. En una parte de los intentos solo hay distractores y lo correcto es no responder.

use super::layout::{center, cell_distance, contains, inner, layout};
use super::{Flash, FlashKind, GameMode, pt, Method, Outcome, Params, Response, Step, TrialRecord};
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
    /// El estímulo está listo y se dibuja en el próximo cuadro.
    Pending,
    Stimulus { onset: Instant, deadline: Instant },
}

pub struct Decision {
    rng: fastrand::Rng,
    anticipation: Duration,
    fixation: (u64, u64),
    target_fill: Fill,
    distractor_fill: Fill,
    /// Qué intentos son sin objetivo, en orden.
    catch_schedule: Vec<bool>,
    completed: usize,
    phase: Phase,
    params: Params,
    boxes: Vec<Rect>,
    target: Option<usize>,
    cursor_dist: Option<f32>,
    start: Option<(f32, f32)>,
    click: Option<(f32, f32)>,
    flashes: Vec<Flash>,
}

impl Decision {
    pub fn new(cfg: &Config, mut rng: fastrand::Rng) -> Decision {
        let mut catch_schedule = vec![false; cfg.warmup_trials];
        catch_schedule.extend(self::catch_schedule(cfg.trials, cfg.catch_ratio, &mut rng));
        Decision {
            rng,
            anticipation: Duration::from_millis(cfg.anticipation_ms),
            fixation: (cfg.fixation_min_ms, cfg.fixation_max_ms),
            target_fill: cfg.target_fill,
            distractor_fill: cfg.distractor_fill,
            catch_schedule,
            completed: 0,
            phase: Phase::Idle,
            params: Params {
                box_w: cfg.box_w,
                box_h: cfg.box_h,
                min_boxes: cfg.min_boxes,
                max_boxes: cfg.max_boxes,
                timeout: Duration::from_millis(cfg.timeout_ms),
                level: None,
            },
            boxes: Vec::new(),
            target: None,
            cursor_dist: None,
            start: None,
            click: None,
            flashes: Vec::new(),
        }
    }

    fn flash(&mut self, kind: FlashKind, i: usize) {
        if let Some(&rect) = self.boxes.get(i) {
            self.flashes.push(Flash { kind, rect });
        }
    }

    fn record(&mut self, outcome: Outcome, rt: Option<Duration>, method: Method) -> Step {
        let (w, h) = self.boxes.first().map(|b| (b.width, b.height)).unwrap_or((0, 0));
        let rec = TrialRecord {
            index: self.completed,
            level: self.params.level,
            n_boxes: self.boxes.len(),
            target_present: self.target.is_some(),
            target_w: w,
            target_h: h,
            cursor_dist: self.cursor_dist,
            rt_ms: rt.map(|d| d.as_secs_f64() * 1000.0),
            outcome,
            method,
            anticipation: outcome == Outcome::Anticipation,
            start: self.start.map(pt),
            target_pos: self.target.and_then(|t| self.boxes.get(t)).map(|b| pt(center(b))),
            click: self.click.map(pt),
            score: None,
            radius: None,
            warmup: false,
        };
        self.completed += 1;
        self.phase = Phase::Idle;
        Step::Done(rec)
    }
}

/// Reparte `round(n * ratio)` intentos sin objetivo al azar entre `n`.
pub fn catch_schedule(n: usize, ratio: f64, rng: &mut fastrand::Rng) -> Vec<bool> {
    let k = ((n as f64) * ratio).round() as usize;
    let mut v: Vec<bool> = (0..n).map(|i| i < k.min(n)).collect();
    rng.shuffle(&mut v);
    v
}

/// Clasifica una respuesta dada durante el estímulo.
pub fn classify(target: Option<usize>, chosen: usize, rt: Duration, anticipation: Duration) -> Outcome {
    if rt < anticipation {
        Outcome::Anticipation
    } else {
        match target {
            None => Outcome::FalseAlarm,
            Some(t) if t == chosen => Outcome::Hit,
            Some(_) => Outcome::Error,
        }
    }
}

impl GameMode for Decision {
    fn id(&self) -> &'static str {
        "decision"
    }

    fn start_trial(&mut self, area: Rect, params: &Params, now: Instant, _cursor: Option<(f32, f32)>) {
        self.params = *params;
        let lo = params.min_boxes.min(params.max_boxes);
        let n = self.rng.usize(lo..=params.max_boxes);
        self.boxes = layout(inner(area), n, params.box_w, params.box_h, &mut self.rng);
        let is_catch = self.catch_schedule.get(self.completed).copied().unwrap_or(false);
        self.target = if is_catch || self.boxes.is_empty() { None } else { Some(self.rng.usize(..self.boxes.len())) };
        self.cursor_dist = None;
        self.start = None;
        self.click = None;
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
                let outcome = match self.target {
                    Some(t) => {
                        self.flash(FlashKind::Reveal, t);
                        Outcome::Omission
                    }
                    None => Outcome::CorrectRejection,
                };
                self.record(outcome, None, Method::None)
            }
            _ => Step::Idle,
        }
    }

    fn drawn(&mut self, now: Instant, cursor: Option<(f32, f32)>) {
        if self.phase != Phase::Pending {
            return;
        }
        self.phase = Phase::Stimulus { onset: now, deadline: now + self.params.timeout };
        self.start = cursor;
        self.cursor_dist = match (cursor, self.target) {
            (Some((cx, cy)), Some(t)) => {
                Some(cell_distance((cx, cy), center(&self.boxes[t])))
            }
            _ => None,
        };
    }

    fn respond(&mut self, r: Response, at: Instant) -> Step {
        if r == Response::Press {
            // El espacio no elige ningún recuadro.
            return Step::Idle;
        }
        let method = match r {
            Response::Key(_) | Response::Press => Method::Keyboard,
            Response::Click { x, y } => {
                self.click = Some((x, y));
                Method::Mouse
            }
        };
        match self.phase {
            Phase::Fixation { .. } | Phase::Pending => self.record(Outcome::Anticipation, None, method),
            Phase::Stimulus { onset, .. } => {
                let chosen = match r {
                    Response::Key(k) if (1..=self.boxes.len()).contains(&k) => k - 1,
                    Response::Click { x, y } => match self.boxes.iter().position(|b| contains(b, x, y)) {
                        Some(i) => i,
                        None => return Step::Idle,
                    },
                    _ => return Step::Idle,
                };
                let rt = at.saturating_duration_since(onset);
                let outcome = classify(self.target, chosen, rt, self.anticipation);
                match (outcome, self.target) {
                    (Outcome::Hit, _) => self.flash(FlashKind::Hit, chosen),
                    (_, Some(t)) => {
                        self.flash(FlashKind::Wrong, chosen);
                        if t != chosen {
                            self.flash(FlashKind::Reveal, t);
                        }
                    }
                    (_, None) => self.flash(FlashKind::Wrong, chosen),
                }
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

    fn take_flashes(&mut self) -> Vec<Flash> {
        std::mem::take(&mut self.flashes)
    }

    fn render(&self, buf: &mut Buffer, area: Rect, theme: &Theme) {
        match self.phase {
            Phase::Fixation { .. } => render::fixation(buf, area, theme),
            Phase::Pending | Phase::Stimulus { .. } => {
                for (i, b) in self.boxes.iter().enumerate() {
                    let label = char::from_digit((i + 1) as u32, 10).unwrap_or(' ');
                    if Some(i) == self.target {
                        render::draw_box_themed(buf, *b, self.target_fill, theme.target, Some(label), theme);
                    } else {
                        render::draw_box_themed(buf, *b, self.distractor_fill, theme.distractor, Some(label), theme);
                    }
                }
            }
            Phase::Idle => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(w: u16, h: u16) -> Rect {
        Rect { x: 0, y: 0, width: w, height: h }
    }

    #[test]
    fn catch_schedule_has_expected_count() {
        let mut rng = fastrand::Rng::with_seed(3);
        assert_eq!(catch_schedule(20, 0.17, &mut rng).iter().filter(|c| **c).count(), 3);
        assert_eq!(catch_schedule(30, 0.17, &mut rng).iter().filter(|c| **c).count(), 5);
    }

    #[test]
    fn warmup_trials_come_first_and_always_have_target() {
        let cfg = Config { trials: 20, warmup_trials: 3, ..Config::default() };
        let m = Decision::new(&cfg, fastrand::Rng::with_seed(5));
        assert_eq!(m.catch_schedule.len(), 23);
        assert!(m.catch_schedule[..3].iter().all(|c| !c));
        assert_eq!(m.catch_schedule.iter().filter(|c| **c).count(), 3);
    }

    #[test]
    fn anticipation_threshold() {
        let a = Duration::from_millis(120);
        assert_eq!(classify(Some(0), 0, Duration::from_millis(119), a), Outcome::Anticipation);
        assert_eq!(classify(Some(0), 0, Duration::from_millis(120), a), Outcome::Hit);
        assert_eq!(classify(Some(0), 1, Duration::from_millis(300), a), Outcome::Error);
        assert_eq!(classify(None, 1, Duration::from_millis(300), a), Outcome::FalseAlarm);
        assert_eq!(classify(None, 1, Duration::from_millis(50), a), Outcome::Anticipation);
    }

    fn mode() -> Decision {
        let cfg = Config { catch_ratio: 0.0, ..Config::default() };
        Decision::new(&cfg, fastrand::Rng::with_seed(9))
    }

    fn params() -> Params {
        Params { box_w: 6, box_h: 3, min_boxes: 3, max_boxes: 5, timeout: Duration::from_secs(2), level: Some(2) }
    }

    #[test]
    fn full_trial_hit_and_rt() {
        let mut m = mode();
        let t0 = Instant::now();
        m.start_trial(area(80, 24), &params(), t0, None);
        let until = m.deadline().unwrap();
        assert_eq!(m.tick(until), Step::Redraw);
        m.drawn(until, Some((0.5, 0.5)));
        let target = m.target.unwrap();
        let Step::Done(rec) = m.respond(Response::Key(target + 1), until + Duration::from_millis(350)) else {
            panic!("sin resultado")
        };
        assert_eq!(rec.outcome, Outcome::Hit);
        assert_eq!(m.take_flashes(), vec![Flash { kind: FlashKind::Hit, rect: m.boxes[target] }]);
        assert!(m.take_flashes().is_empty());
        assert!((rec.rt_ms.unwrap() - 350.0).abs() < 0.01);
        assert_eq!(rec.method, Method::Keyboard);
        assert_eq!(rec.level, Some(2));
        assert!(rec.cursor_dist.unwrap() > 0.0);
        assert!(!m.in_trial());
    }

    #[test]
    fn response_during_fixation_is_anticipation() {
        let mut m = mode();
        let t0 = Instant::now();
        m.start_trial(area(80, 24), &params(), t0, None);
        let Step::Done(rec) = m.respond(Response::Key(1), t0) else { panic!() };
        assert_eq!(rec.outcome, Outcome::Anticipation);
        assert!(rec.anticipation);
    }

    #[test]
    fn timeout_is_omission_and_click_outside_is_ignored() {
        let mut m = mode();
        let t0 = Instant::now();
        m.start_trial(area(80, 24), &params(), t0, None);
        let until = m.deadline().unwrap();
        m.tick(until);
        m.drawn(until, None);
        assert_eq!(m.respond(Response::Click { x: 0.1, y: 0.1 }, until), Step::Idle);
        assert_eq!(m.respond(Response::Key(9), until), Step::Idle);
        let Step::Done(rec) = m.tick(m.deadline().unwrap()) else { panic!() };
        assert_eq!(rec.outcome, Outcome::Omission);
        assert_eq!(rec.rt_ms, None);
        assert_eq!(m.take_flashes()[0].kind, FlashKind::Reveal);
    }

    #[test]
    fn catch_trial_outcomes() {
        let cfg = Config { catch_ratio: 1.0, trials: 2, warmup_trials: 0, ..Config::default() };
        let mut m = Decision::new(&cfg, fastrand::Rng::with_seed(4));
        let t0 = Instant::now();
        m.start_trial(area(80, 24), &params(), t0, None);
        assert_eq!(m.target, None);
        let until = m.deadline().unwrap();
        m.tick(until);
        m.drawn(until, None);
        let b = m.boxes[0];
        let Step::Done(rec) =
            m.respond(Response::Click { x: b.x as f32 + 0.5, y: b.y as f32 + 0.5 }, until + Duration::from_millis(400))
        else {
            panic!()
        };
        assert_eq!(rec.outcome, Outcome::FalseAlarm);
        assert_eq!(rec.method, Method::Mouse);

        m.start_trial(area(80, 24), &params(), t0, None);
        let until = m.deadline().unwrap();
        m.tick(until);
        m.drawn(until, None);
        let Step::Done(rec) = m.tick(m.deadline().unwrap()) else { panic!() };
        assert_eq!(rec.outcome, Outcome::CorrectRejection);
    }
}
