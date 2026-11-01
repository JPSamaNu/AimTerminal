//! Modo Decisión: punto de fijación, espera aleatoria y de 3 a 5 recuadros; uno es el
//! objetivo. En una parte de los intentos solo hay distractores y lo correcto es no responder.

use super::{GameMode, Method, Outcome, Params, Response, Step, TrialRecord};
use crate::config::{Config, Fill};
use crate::render;
use crate::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

/// Separación mínima entre recuadros, en celdas.
const GAP: u16 = 1;
/// Altura mínima: con 3 filas el número cabe dentro de un recuadro hueco.
const MIN_H: u16 = 3;
const MIN_W: u16 = 3;

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
}

impl Decision {
    pub fn new(cfg: &Config, mut rng: fastrand::Rng) -> Decision {
        let catch_schedule = catch_schedule(cfg.trials, cfg.catch_ratio, &mut rng);
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

/// Coloca `n` recuadros de `w`x`h` dentro de `area` sin solaparse (con separación `GAP`).
pub fn place_boxes(area: Rect, n: usize, w: u16, h: u16, rng: &mut fastrand::Rng) -> Option<Vec<Rect>> {
    if w > area.width || h > area.height {
        return None;
    }
    for _ in 0..30 {
        let mut placed: Vec<Rect> = Vec::with_capacity(n);
        for _ in 0..n {
            let mut ok = None;
            for _ in 0..200 {
                let x = area.x + rng.u16(0..=area.width - w);
                let y = area.y + rng.u16(0..=area.height - h);
                let r = Rect { x, y, width: w, height: h };
                if placed.iter().all(|p| !overlaps_with_gap(p, &r)) {
                    ok = Some(r);
                    break;
                }
            }
            match ok {
                Some(r) => placed.push(r),
                None => break,
            }
        }
        if placed.len() == n {
            return Some(placed);
        }
    }
    None
}

fn overlaps_with_gap(a: &Rect, b: &Rect) -> bool {
    let ax2 = a.x + a.width + GAP;
    let ay2 = a.y + a.height + GAP;
    let bx2 = b.x + b.width + GAP;
    let by2 = b.y + b.height + GAP;
    a.x < bx2 && b.x < ax2 && a.y < by2 && b.y < ay2
}

/// Intenta el tamaño pedido y lo reduce si no cabe (paneles pequeños).
pub fn layout(area: Rect, n: usize, w: u16, h: u16, rng: &mut fastrand::Rng) -> Vec<Rect> {
    let (mut w, mut h, mut n) = (w, h, n);
    loop {
        if let Some(v) = place_boxes(area, n, w, h, rng) {
            return v;
        }
        if w > MIN_W {
            w -= 1;
        } else if h > MIN_H {
            h -= 1;
        } else if n > 2 {
            n -= 1;
        } else {
            return place_boxes(area, 1, w.min(area.width), h.min(area.height), rng).unwrap_or_default();
        }
    }
}

fn inner(area: Rect) -> Rect {
    if area.width > 2 && area.height > 2 {
        Rect { x: area.x + 1, y: area.y + 1, width: area.width - 2, height: area.height - 2 }
    } else {
        area
    }
}

fn center(r: &Rect) -> (f32, f32) {
    (r.x as f32 + r.width as f32 / 2.0, r.y as f32 + r.height as f32 / 2.0)
}

fn contains(r: &Rect, x: f32, y: f32) -> bool {
    x >= r.x as f32 && x < (r.x + r.width) as f32 && y >= r.y as f32 && y < (r.y + r.height) as f32
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

    fn start_trial(&mut self, area: Rect, params: &Params, now: Instant) {
        self.params = *params;
        let lo = params.min_boxes.min(params.max_boxes);
        let n = self.rng.usize(lo..=params.max_boxes);
        self.boxes = layout(inner(area), n, params.box_w, params.box_h, &mut self.rng);
        let is_catch = self.catch_schedule.get(self.completed).copied().unwrap_or(false);
        self.target = if is_catch || self.boxes.is_empty() { None } else { Some(self.rng.usize(..self.boxes.len())) };
        self.cursor_dist = None;
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
                let outcome = if self.target.is_some() { Outcome::Omission } else { Outcome::CorrectRejection };
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
        self.cursor_dist = match (cursor, self.target) {
            (Some((cx, cy)), Some(t)) => {
                let (tx, ty) = center(&self.boxes[t]);
                Some(((tx - cx).powi(2) + (2.0 * (ty - cy)).powi(2)).sqrt())
            }
            _ => None,
        };
    }

    fn respond(&mut self, r: Response, at: Instant) -> Step {
        let method = match r {
            Response::Key(_) => Method::Keyboard,
            Response::Click { .. } => Method::Mouse,
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
                for (i, b) in self.boxes.iter().enumerate() {
                    let label = char::from_digit((i + 1) as u32, 10).unwrap_or(' ');
                    if Some(i) == self.target {
                        render::draw_box(buf, *b, self.target_fill, theme.target, label);
                    } else {
                        render::draw_box(buf, *b, self.distractor_fill, theme.distractor, label);
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
    fn boxes_never_overlap_and_stay_inside() {
        let mut rng = fastrand::Rng::with_seed(7);
        for seed in 0..300 {
            rng.seed(seed);
            let a = area(rng.u16(40..=200), rng.u16(11..=60));
            let n = rng.usize(3..=5);
            let boxes = layout(a, n, 10, 5, &mut rng);
            assert!(!boxes.is_empty());
            for (i, b) in boxes.iter().enumerate() {
                assert!(b.x + b.width <= a.width && b.y + b.height <= a.height, "fuera: {b:?} en {a:?}");
                for c in &boxes[i + 1..] {
                    assert!(!overlaps_with_gap(b, c), "solapados {b:?} {c:?}");
                }
            }
        }
    }

    #[test]
    fn small_panel_shrinks_instead_of_failing() {
        let mut rng = fastrand::Rng::with_seed(1);
        let boxes = layout(area(38, 9), 5, 10, 5, &mut rng);
        assert_eq!(boxes.len(), 5);
    }

    #[test]
    fn gap_counts_as_overlap() {
        let a = Rect { x: 0, y: 0, width: 4, height: 3 };
        let touching = Rect { x: 4, y: 0, width: 4, height: 3 };
        let apart = Rect { x: 5, y: 0, width: 4, height: 3 };
        assert!(overlaps_with_gap(&a, &touching));
        assert!(!overlaps_with_gap(&a, &apart));
    }

    #[test]
    fn catch_schedule_has_expected_count() {
        let mut rng = fastrand::Rng::with_seed(3);
        assert_eq!(catch_schedule(20, 0.17, &mut rng).iter().filter(|c| **c).count(), 3);
        assert_eq!(catch_schedule(30, 0.17, &mut rng).iter().filter(|c| **c).count(), 5);
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
        m.start_trial(area(80, 24), &params(), t0);
        let until = m.deadline().unwrap();
        assert_eq!(m.tick(until), Step::Redraw);
        m.drawn(until, Some((0.5, 0.5)));
        let target = m.target.unwrap();
        let Step::Done(rec) = m.respond(Response::Key(target + 1), until + Duration::from_millis(350)) else {
            panic!("sin resultado")
        };
        assert_eq!(rec.outcome, Outcome::Hit);
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
        m.start_trial(area(80, 24), &params(), t0);
        let Step::Done(rec) = m.respond(Response::Key(1), t0) else { panic!() };
        assert_eq!(rec.outcome, Outcome::Anticipation);
        assert!(rec.anticipation);
    }

    #[test]
    fn timeout_is_omission_and_click_outside_is_ignored() {
        let mut m = mode();
        let t0 = Instant::now();
        m.start_trial(area(80, 24), &params(), t0);
        let until = m.deadline().unwrap();
        m.tick(until);
        m.drawn(until, None);
        assert_eq!(m.respond(Response::Click { x: 0.1, y: 0.1 }, until), Step::Idle);
        assert_eq!(m.respond(Response::Key(9), until), Step::Idle);
        let Step::Done(rec) = m.tick(m.deadline().unwrap()) else { panic!() };
        assert_eq!(rec.outcome, Outcome::Omission);
        assert_eq!(rec.rt_ms, None);
    }

    #[test]
    fn catch_trial_outcomes() {
        let cfg = Config { catch_ratio: 1.0, trials: 2, ..Config::default() };
        let mut m = Decision::new(&cfg, fastrand::Rng::with_seed(4));
        let t0 = Instant::now();
        m.start_trial(area(80, 24), &params(), t0);
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

        m.start_trial(area(80, 24), &params(), t0);
        let until = m.deadline().unwrap();
        m.tick(until);
        m.drawn(until, None);
        let Step::Done(rec) = m.tick(m.deadline().unwrap()) else { panic!() };
        assert_eq!(rec.outcome, Outcome::CorrectRejection);
    }
}
