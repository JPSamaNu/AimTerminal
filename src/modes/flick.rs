//! Modo Flick: un solo blanco pequeño lejos del cursor. Al acertarlo aparece otro en
//! otra dirección; las 8 direcciones salen repartidas por igual para que el análisis
//! por ángulo tenga datos de todas. Un clic fuera es un fallo y el blanco se queda.

use super::layout::{cell_distance, center, contains, find_spot, inner};
use super::{Flash, FlashKind, GameMode, Method, Outcome, Params, Response, Step, TrialRecord, pt};
use crate::config::{Config, Fill};
use crate::render;
use crate::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

/// Distancia mínima del cursor al nuevo blanco, en anchos de celda.
const MIN_DIST: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Idle,
    Pending,
    Active { since: Instant, from: Option<(f32, f32)>, deadline: Instant },
}

pub struct Flick {
    rng: fastrand::Rng,
    anticipation: Duration,
    timeout: Duration,
    size: (u16, u16),
    fill: Fill,
    area: Rect,
    rect: Option<Rect>,
    /// Direcciones (0..8) que faltan en la ronda actual.
    bag: Vec<usize>,
    phase: Phase,
    completed: usize,
    flashes: Vec<Flash>,
}

impl Flick {
    pub fn new(cfg: &Config, rng: fastrand::Rng) -> Flick {
        Flick {
            rng,
            anticipation: Duration::from_millis(cfg.anticipation_ms),
            timeout: Duration::from_millis(cfg.flick_timeout_ms),
            size: (cfg.flick_w, cfg.flick_h),
            fill: cfg.target_fill,
            area: Rect::default(),
            rect: None,
            bag: Vec::new(),
            phase: Phase::Idle,
            completed: 0,
            flashes: Vec::new(),
        }
    }

    fn next_direction(&mut self) -> usize {
        if self.bag.is_empty() {
            self.bag = (0..8).collect();
            self.rng.shuffle(&mut self.bag);
        }
        self.bag.pop().unwrap_or(0)
    }

    /// Coloca el blanco en la dirección `dir` (0 = derecha, en sentido antihorario de
    /// 45 en 45 grados) a una distancia al azar, dentro del área.
    fn place(&mut self, from: (f32, f32)) -> Rect {
        let area = inner(self.area);
        let (w, h) = (self.size.0.min(area.width), self.size.1.min(area.height));
        let dir = self.next_direction();
        let max_d = (area.width as f32).hypot(2.0 * area.height as f32);
        for _ in 0..60 {
            let angle = (dir as f32 * 45.0 + self.rng.f32() * 30.0 - 15.0).to_radians();
            let d = MIN_DIST + self.rng.f32() * (max_d * 0.6 - MIN_DIST).max(1.0);
            // La vertical se divide entre 2 porque una celda es el doble de alta que ancha.
            let (cx, cy) = (from.0 + angle.cos() * d, from.1 - angle.sin() * d / 2.0);
            let x = cx - w as f32 / 2.0;
            let y = cy - h as f32 / 2.0;
            if x >= area.x as f32
                && y >= area.y as f32
                && x + w as f32 <= area.right() as f32
                && y + h as f32 <= area.bottom() as f32
            {
                return Rect { x: x.round() as u16, y: y.round() as u16, width: w, height: h }.intersection(area);
            }
        }
        find_spot(area, &[], w, h, Some(from), &mut self.rng)
            .unwrap_or(Rect { x: area.x, y: area.y, width: w, height: h })
    }

    fn record(&mut self, outcome: Outcome, rt: Option<Duration>, from: Option<(f32, f32)>, click: Option<(f32, f32)>) -> Step {
        let rect = self.rect.unwrap_or_default();
        let c = center(&rect);
        let rec = TrialRecord {
            index: self.completed,
            level: None,
            n_boxes: 1,
            target_present: true,
            target_w: rect.width,
            target_h: rect.height,
            cursor_dist: from.map(|f| cell_distance(f, c)),
            rt_ms: rt.map(|d| d.as_secs_f64() * 1000.0),
            outcome,
            method: if click.is_some() { Method::Mouse } else { Method::None },
            anticipation: outcome == Outcome::Anticipation,
            start: from.map(pt),
            target_pos: Some(pt(c)),
            click: click.map(pt),
            score: None,
            radius: None,
            warmup: false,
        };
        let flash = match outcome {
            Outcome::Hit | Outcome::Anticipation => Some(Flash { kind: FlashKind::Hit, rect }),
            Outcome::Omission => Some(Flash { kind: FlashKind::Reveal, rect }),
            Outcome::Miss => click.map(|(x, y)| Flash {
                kind: FlashKind::Miss,
                rect: Rect { x: x.max(0.0) as u16, y: y.max(0.0) as u16, width: 1, height: 1 },
            }),
            _ => None,
        };
        self.flashes.extend(flash);
        if outcome != Outcome::Miss {
            // El blanco se resolvió: el próximo intento coloca uno nuevo.
            self.rect = None;
            self.completed += 1;
        }
        self.phase = Phase::Idle;
        Step::Done(rec)
    }
}

impl GameMode for Flick {
    fn id(&self) -> &'static str {
        "flick"
    }

    fn start_trial(&mut self, area: Rect, _params: &Params, _now: Instant, cursor: Option<(f32, f32)>) {
        if self.area != area {
            self.area = area;
            self.rect = None;
        }
        if self.rect.is_none() {
            let from = cursor.unwrap_or_else(|| center(&area));
            self.rect = Some(self.place(from));
        }
        self.phase = Phase::Pending;
    }

    fn deadline(&self) -> Option<Instant> {
        match self.phase {
            Phase::Active { deadline, .. } => Some(deadline),
            _ => None,
        }
    }

    fn tick(&mut self, now: Instant) -> Step {
        match self.phase {
            Phase::Active { from, deadline, .. } if now >= deadline => self.record(Outcome::Omission, None, from, None),
            _ => Step::Idle,
        }
    }

    fn drawn(&mut self, now: Instant, cursor: Option<(f32, f32)>) {
        if self.phase == Phase::Pending {
            self.phase = Phase::Active { since: now, from: cursor, deadline: now + self.timeout };
        }
    }

    fn respond(&mut self, r: Response, at: Instant) -> Step {
        let (Response::Click { x, y }, Phase::Active { since, from, .. }, Some(rect)) = (r, self.phase, self.rect) else {
            return Step::Idle;
        };
        let rt = at.saturating_duration_since(since);
        let outcome = if !contains(&rect, x, y) {
            Outcome::Miss
        } else if rt < self.anticipation {
            Outcome::Anticipation
        } else {
            Outcome::Hit
        };
        self.record(outcome, Some(rt), from, Some((x, y)))
    }

    fn abort(&mut self) {
        self.phase = Phase::Idle;
        self.rect = None;
    }

    fn in_trial(&self) -> bool {
        self.phase != Phase::Idle
    }

    fn render(&self, buf: &mut Buffer, _area: Rect, theme: &Theme) {
        if let (Some(r), true) = (self.rect, self.phase != Phase::Idle) {
            render::draw_box_themed(buf, r, self.fill, theme.target, None, theme);
        }
    }

    fn take_flashes(&mut self) -> Vec<Flash> {
        std::mem::take(&mut self.flashes)
    }

    /// Los clics fallidos no cuentan para terminar la sesión: el blanco sigue ahí.
    fn counts(&self, rec: &TrialRecord) -> bool {
        rec.outcome != Outcome::Miss
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> Params {
        Params { box_w: 4, box_h: 2, min_boxes: 1, max_boxes: 1, timeout: Duration::from_secs(3), level: None }
    }

    fn setup(cursor: (f32, f32)) -> (Flick, Instant) {
        let mut m = Flick::new(&Config::default(), fastrand::Rng::with_seed(8));
        let t0 = Instant::now();
        m.start_trial(Rect::new(0, 0, 120, 40), &params(), t0, Some(cursor));
        m.drawn(t0, Some(cursor));
        (m, t0)
    }

    #[test]
    fn targets_appear_far_and_cover_all_directions() {
        let mut m = Flick::new(&Config::default(), fastrand::Rng::with_seed(4));
        m.area = Rect::new(0, 0, 160, 50);
        let from = (80.0, 25.0);
        let mut sectors = [0; 8];
        for _ in 0..80 {
            let r = m.place(from);
            let c = center(&r);
            assert!(cell_distance(from, c) >= MIN_DIST - 3.0, "{r:?}");
            let ang = (-(c.1 - from.1) * 2.0).atan2(c.0 - from.0).to_degrees().rem_euclid(360.0);
            sectors[((ang + 22.5) / 45.0) as usize % 8] += 1;
        }
        assert!(sectors.iter().all(|n| *n >= 6), "{sectors:?}");
    }

    #[test]
    fn hit_records_geometry_and_moves_target() {
        let (mut m, t0) = setup((60.0, 20.0));
        let r = m.rect.unwrap();
        let (x, y) = center(&r);
        let Step::Done(rec) = m.respond(Response::Click { x, y }, t0 + Duration::from_millis(500)) else { panic!() };
        assert_eq!(rec.outcome, Outcome::Hit);
        assert_eq!(rec.start, Some([60.0, 20.0]));
        assert_eq!(rec.target_pos, Some(pt((x, y))));
        assert_eq!(rec.click, Some(pt((x, y))));
        assert!(m.counts(&rec));
        assert!(m.rect.is_none());
        m.start_trial(Rect::new(0, 0, 120, 40), &params(), t0, Some((x, y)));
        assert!(m.rect.is_some());
    }

    #[test]
    fn miss_keeps_target_and_does_not_count() {
        let (mut m, t0) = setup((60.0, 20.0));
        let r = m.rect.unwrap();
        let away = if r.x > 10 { (1.5, 1.5) } else { (118.5, 38.5) };
        let Step::Done(rec) = m.respond(Response::Click { x: away.0, y: away.1 }, t0 + Duration::from_millis(400))
        else {
            panic!()
        };
        assert_eq!(rec.outcome, Outcome::Miss);
        assert!(!m.counts(&rec));
        assert_eq!(m.rect, Some(r));
        assert_eq!(m.take_flashes()[0].kind, FlashKind::Miss);
    }

    #[test]
    fn timeout_is_omission() {
        let (mut m, _) = setup((60.0, 20.0));
        let Step::Done(rec) = m.tick(m.deadline().unwrap()) else { panic!() };
        assert_eq!(rec.outcome, Outcome::Omission);
        assert!(m.counts(&rec));
    }
}
