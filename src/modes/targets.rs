//! Modo Blancos: varios recuadros verdes a la vez. Al hacer clic en uno, solo ese cambia
//! de lugar. Cada clic es un intento; su tiempo se mide desde el cuadro anterior.
//! El nivel (0 a 9) decide qué tan lejos del centro suelen aparecer los blancos.

use super::layout::{
    cell_distance, center, contains, find_spot, inner, layout, normalized_radius, spawn_by_radius,
};
use super::{Flash, FlashKind, GameMode, pt, Method, Outcome, Params, Response, Step, TrialRecord};
use crate::config::{Config, Fill};
use crate::difficulty::spawn_exponent;
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
    flashes: Vec<Flash>,
    level: Option<usize>,
    /// Recuadro tocado que hay que mover y dónde fue el clic; se mueve al empezar el
    /// siguiente intento, con el nivel ya actualizado por la escalera.
    relocate: Option<(usize, (f32, f32))>,
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
            flashes: Vec::new(),
            level: None,
            relocate: None,
        }
    }

    /// Lugar para un recuadro según el nivel; sin nivel, en cualquier sitio libre.
    fn spot(&mut self, others: &[Rect], w: u16, h: u16, avoid: Option<(f32, f32)>) -> Option<Rect> {
        let area = inner(self.area);
        let by_level =
            self.level.and_then(|l| spawn_by_radius(area, others, w, h, avoid, spawn_exponent(l), &mut self.rng));
        by_level.or_else(|| find_spot(area, others, w, h, avoid, &mut self.rng))
    }

    fn radius(&self, r: &Rect) -> f32 {
        normalized_radius(inner(self.area), r)
    }

    #[allow(clippy::too_many_arguments)]
    fn record(
        &mut self,
        outcome: Outcome,
        rt: Duration,
        dist: Option<f32>,
        w: u16,
        h: u16,
        start: Option<(f32, f32)>,
        target_rect: Option<Rect>,
        click: (f32, f32),
    ) -> Step {
        let target = target_rect.map(|r| center(&r));
        let rec = TrialRecord {
            index: self.completed,
            level: self.level,
            n_boxes: self.boxes.len(),
            target_present: true,
            target_w: w,
            target_h: h,
            cursor_dist: dist,
            rt_ms: Some(rt.as_secs_f64() * 1000.0),
            outcome,
            method: Method::Mouse,
            anticipation: outcome == Outcome::Anticipation,
            start: start.map(pt),
            target_pos: target.map(pt),
            click: Some(pt(click)),
            score: None,
            radius: target_rect.map(|r| self.radius(&r)),
            warmup: false,
        };
        self.completed += 1;
        self.phase = Phase::Idle;
        Step::Done(rec)
    }
}

/// Recuadro más cercano a un punto (para saber a cuál se apuntaba).
pub fn nearest(boxes: &[Rect], p: (f32, f32)) -> Option<Rect> {
    boxes.iter().copied().min_by(|a, b| cell_distance(p, center(a)).total_cmp(&cell_distance(p, center(b))))
}

impl GameMode for Targets {
    fn id(&self) -> &'static str {
        "targets"
    }

    /// La primera vez (o tras un descarte) coloca todos los recuadros; después de un clic
    /// solo vuelve a mostrar los que ya hay, porque el que se tocó ya cambió de lugar.
    fn start_trial(&mut self, area: Rect, params: &Params, _now: Instant, _cursor: Option<(f32, f32)>) {
        self.level = params.level;
        if self.boxes.is_empty() || self.area != area {
            self.area = area;
            self.relocate = None;
            let mut boxes: Vec<Rect> = Vec::new();
            for _ in 0..params.max_boxes {
                match self.spot(&boxes, params.box_w, params.box_h, None) {
                    Some(r) => boxes.push(r),
                    None => break,
                }
            }
            if boxes.len() < params.max_boxes {
                // No caben con este tamaño: `layout` los achica para paneles pequeños.
                boxes = layout(inner(area), params.max_boxes, params.box_w, params.box_h, &mut self.rng);
            }
            self.size = boxes.first().map(|b| (b.width, b.height)).unwrap_or((params.box_w, params.box_h));
            self.boxes = boxes;
        }
        if let Some((i, click)) = self.relocate.take() {
            let others: Vec<Rect> = self.boxes.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, b)| *b).collect();
            let (w, h) = self.size;
            if let Some(r) = self.spot(&others, w, h, Some(click)) {
                self.boxes[i] = r;
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
            let spot = Rect { x: x.max(0.0) as u16, y: y.max(0.0) as u16, width: 1, height: 1 };
            self.flashes.push(Flash { kind: FlashKind::Miss, rect: spot });
            let near = nearest(&self.boxes, (x, y));
            return self.record(Outcome::Miss, rt, None, w, h, from, near, (x, y));
        };
        let hit = self.boxes[i];
        self.flashes.push(Flash { kind: FlashKind::Hit, rect: hit });
        let dist = from.map(|f| cell_distance(f, center(&hit)));
        let outcome = if rt < self.anticipation { Outcome::Anticipation } else { Outcome::Hit };
        self.relocate = Some((i, (x, y)));
        self.record(outcome, rt, dist, hit.width, hit.height, from, Some(hit), (x, y))
    }

    fn abort(&mut self) {
        self.boxes.clear();
        self.relocate = None;
        self.phase = Phase::Idle;
    }

    fn in_trial(&self) -> bool {
        self.phase != Phase::Idle
    }

    fn take_flashes(&mut self) -> Vec<Flash> {
        std::mem::take(&mut self.flashes)
    }

    fn render(&self, buf: &mut Buffer, _area: Rect, theme: &Theme) {
        if self.phase == Phase::Idle {
            return;
        }
        for b in &self.boxes {
            render::draw_box_themed(buf, *b, self.fill, theme.target, None, theme);
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
        m.start_trial(Rect::new(0, 0, 80, 24), &params(), t0, None);
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
        assert!(rec.radius.is_some());
        // El tocado se mueve al empezar el siguiente intento; los demás se quedan.
        m.start_trial(Rect::new(0, 0, 80, 24), &params(), t0, None);
        assert_eq!(m.boxes[0], before[0]);
        assert_eq!(m.boxes[2], before[2]);
        assert_ne!(m.boxes[1], before[1]);
        assert!(!contains(&m.boxes[1], x, y));
        assert!(!overlaps_with_gap(&m.boxes[1], &m.boxes[0]) && !overlaps_with_gap(&m.boxes[1], &m.boxes[2]));
    }

    /// Radio medio de los blancos tocados a lo largo de muchos clics en un nivel.
    fn mean_radius_at(level: usize) -> f32 {
        let mut m = Targets::new(&Config::default(), fastrand::Rng::with_seed(level as u64 + 1));
        let p = Params { level: Some(level), ..params() };
        let t0 = Instant::now();
        let area = Rect::new(0, 0, 120, 40);
        let mut sum = 0.0;
        let n = 150;
        for i in 0..n {
            m.start_trial(area, &p, t0, None);
            m.drawn(t0, Some((1.0, 1.0)));
            let (x, y) = center(&m.boxes[i % 3]);
            let Step::Done(rec) = m.respond(Response::Click { x, y }, t0 + Duration::from_millis(400)) else {
                panic!()
            };
            assert_eq!(rec.level, Some(level));
            sum += rec.radius.unwrap();
        }
        sum / n as f32
    }

    #[test]
    fn higher_level_spawns_farther_from_center() {
        let (easy, hard) = (mean_radius_at(0), mean_radius_at(9));
        assert!(easy < 0.5 && hard > 0.6 && hard > easy + 0.25, "{easy} {hard}");
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
        assert_eq!(m.take_flashes()[0].kind, FlashKind::Miss);
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
        m.start_trial(Rect::new(0, 0, 50, 14), &params(), t0, None);
        assert_eq!(m.boxes.len(), 3);
        assert!(m.boxes.iter().all(|b| b.x + b.width <= 50 && b.y + b.height <= 14));
    }
}
