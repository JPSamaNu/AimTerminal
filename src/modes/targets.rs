//! Modo Blancos: varios recuadros verdes a la vez. Al hacer clic en uno, solo ese cambia
//! de lugar. Cada clic es un intento; su tiempo se mide desde el cuadro anterior.

use super::layout::{center, cell_distance, contains, find_spot, inner, layout};
use super::{GameMode, Method, Outcome, Params, Response, Step, TrialRecord};
use crate::config::{Config, Fill};
use crate::render;
use crate::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Idle,
    /// Los recuadros se dibujan en el próximo cuadro.
    Pending,
    /// En pantalla desde `since`; `from` es el cursor en ese momento.
    Active { since: Instant, from: Option<(f32, f32)> },
}

pub struct Targets {
    rng: fastrand::Rng,
    anticipation: Duration,
    fill: Fill,
    area: Rect,
    size: (u16, u16),
    boxes: Vec<Rect>,
    phase: Phase,
    completed: usize,
}

impl Targets {
    pub fn new(cfg: &Config, rng: fastrand::Rng) -> Targets {
        Targets {
            rng,
            anticipation: Duration::from_millis(cfg.anticipation_ms),
            fill: cfg.target_fill,
            area: Rect::default(),
            size: (cfg.targets_w, cfg.targets_h),
            boxes: Vec::new(),
            phase: Phase::Idle,
            completed: 0,
        }
    }

    fn record(&mut self, outcome: Outcome, rt: Duration, dist: Option<f32>, w: u16, h: u16) -> Step {
        let rec = TrialRecord {
            index: self.completed,
            level: None,
            n_boxes: self.boxes.len(),
            target_present: true,
            target_w: w,
            target_h: h,
            cursor_dist: dist,
            rt_ms: Some(rt.as_secs_f64() * 1000.0),
            outcome,
            method: Method::Mouse,
            anticipation: outcome == Outcome::Anticipation,
        };
        self.completed += 1;
        self.phase = Phase::Idle;
        Step::Done(rec)
    }
}

impl GameMode for Targets {
    fn id(&self) -> &'static str {
        "targets"
    }

    /// La primera vez (o tras un descarte) coloca todos los recuadros; después de un clic
    /// solo vuelve a mostrar los que ya hay, porque el que se tocó ya cambió de lugar.
    fn start_trial(&mut self, area: Rect, params: &Params, _now: Instant) {
        if self.boxes.is_empty() || self.area != area {
            self.area = area;
            self.size = (params.box_w, params.box_h);
            self.boxes = layout(inner(area), params.max_boxes, params.box_w, params.box_h, &mut self.rng);
            if let Some(b) = self.boxes.first() {
                // `layout` pudo achicarlos para que quepan en un panel pequeño.
                self.size = (b.width, b.height);
            }
        }
        self.phase = Phase::Pending;
    }

    fn deadline(&self) -> Option<Instant> {
        None
    }

    fn tick(&mut self, _now: Instant) -> Step {
        Step::Idle
    }

    fn drawn(&mut self, now: Instant, cursor: Option<(f32, f32)>) {
        if self.phase == Phase::Pending {
            self.phase = Phase::Active { since: now, from: cursor };
        }
    }

    fn respond(&mut self, r: Response, at: Instant) -> Step {
        let (Response::Click { x, y }, Phase::Active { since, from }) = (r, self.phase) else {
            // Solo mouse: las teclas no hacen nada.
            return Step::Idle;
        };
        let rt = at.saturating_duration_since(since);
        let Some(i) = self.boxes.iter().position(|b| contains(b, x, y)) else {
            let (w, h) = self.size;
            return self.record(Outcome::Miss, rt, None, w, h);
        };
        let hit = self.boxes[i];
        let dist = from.map(|f| cell_distance(f, center(&hit)));
        let outcome = if rt < self.anticipation { Outcome::Anticipation } else { Outcome::Hit };
        let others: Vec<Rect> = self.boxes.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, b)| *b).collect();
        if let Some(r) = find_spot(inner(self.area), &others, hit.width, hit.height, Some((x, y)), &mut self.rng) {
            self.boxes[i] = r;
        }
        self.record(outcome, rt, dist, hit.width, hit.height)
    }

    fn abort(&mut self) {
        self.boxes.clear();
        self.phase = Phase::Idle;
    }

    fn in_trial(&self) -> bool {
        self.phase != Phase::Idle
    }

    fn render(&self, buf: &mut Buffer, _area: Rect, theme: &Theme) {
        if self.phase == Phase::Idle {
            return;
        }
        for b in &self.boxes {
            render::draw_box(buf, *b, self.fill, theme.target, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::layout::overlaps_with_gap;

    fn params() -> Params {
        Params { box_w: 6, box_h: 3, min_boxes: 3, max_boxes: 3, timeout: Duration::from_secs(30), level: None }
    }

    fn setup() -> (Targets, Instant) {
        let mut m = Targets::new(&Config::default(), fastrand::Rng::with_seed(2));
        let t0 = Instant::now();
        m.start_trial(Rect::new(0, 0, 80, 24), &params(), t0);
        m.drawn(t0, Some((1.0, 1.0)));
        (m, t0)
    }

    #[test]
    fn clicking_a_target_moves_only_that_one() {
        let (mut m, t0) = setup();
        assert_eq!(m.boxes.len(), 3);
        let before = m.boxes.clone();
        let (x, y) = center(&before[1]);
        let Step::Done(rec) = m.respond(Response::Click { x, y }, t0 + Duration::from_millis(450)) else {
            panic!("sin resultado")
        };
        assert_eq!(rec.outcome, Outcome::Hit);
        assert!((rec.rt_ms.unwrap() - 450.0).abs() < 0.01);
        assert!(rec.cursor_dist.unwrap() > 0.0);
        assert_eq!(m.boxes[0], before[0]);
        assert_eq!(m.boxes[2], before[2]);
        assert_ne!(m.boxes[1], before[1]);
        assert!(!contains(&m.boxes[1], x, y));
        assert!(!overlaps_with_gap(&m.boxes[1], &m.boxes[0]) && !overlaps_with_gap(&m.boxes[1], &m.boxes[2]));

        // El siguiente intento conserva los recuadros.
        m.start_trial(Rect::new(0, 0, 80, 24), &params(), t0);
        assert_eq!(m.boxes[0], before[0]);
    }

    #[test]
    fn empty_click_is_miss_and_keys_are_ignored() {
        let (mut m, t0) = setup();
        let before = m.boxes.clone();
        let empty = (0..80)
            .flat_map(|x| (0..24).map(move |y| (x as f32 + 0.5, y as f32 + 0.5)))
            .find(|(x, y)| !before.iter().any(|b| contains(b, *x, *y)))
            .unwrap();
        assert_eq!(m.respond(Response::Key(1), t0), Step::Idle);
        let Step::Done(rec) = m.respond(Response::Click { x: empty.0, y: empty.1 }, t0 + Duration::from_millis(300))
        else {
            panic!()
        };
        assert_eq!(rec.outcome, Outcome::Miss);
        assert_eq!(m.boxes, before);
    }

    #[test]
    fn too_fast_click_is_anticipation() {
        let (mut m, t0) = setup();
        let (x, y) = center(&m.boxes[0]);
        let Step::Done(rec) = m.respond(Response::Click { x, y }, t0 + Duration::from_millis(60)) else { panic!() };
        assert_eq!(rec.outcome, Outcome::Anticipation);
    }

    #[test]
    fn abort_relayouts() {
        let (mut m, t0) = setup();
        m.abort();
        assert!(!m.in_trial());
        m.start_trial(Rect::new(0, 0, 50, 14), &params(), t0);
        assert_eq!(m.boxes.len(), 3);
        assert!(m.boxes.iter().all(|b| b.x + b.width <= 50 && b.y + b.height <= 14));
    }
}
