//! Modo Seguimiento: un recuadro se mueve suave, rebota en los bordes y cambia de rumbo
//! cada tanto; hay que mantener el cursor encima. Cada segundo se guarda un intento con
//! la fracción del tiempo que el cursor estuvo encima (`score`) y la distancia media.
//! Solo en este modo se redibuja seguido (unas 30 veces por segundo).

use super::layout::{cell_distance, center, contains, inner};
use super::{GameMode, Method, Outcome, Params, Response, Step, TrialRecord, pt};
use crate::config::{Config, Fill};
use crate::render;
use crate::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

const FRAME: Duration = Duration::from_millis(33);
const WINDOW: Duration = Duration::from_secs(1);

pub struct Tracking {
    rng: fastrand::Rng,
    size: (u16, u16),
    speed: f32,
    fill: Fill,
    area: Rect,
    /// Esquina superior izquierda, en celdas.
    pos: (f32, f32),
    /// Rumbo en radianes (0 = derecha).
    heading: f32,
    next_turn: Instant,
    cursor: Option<(f32, f32)>,
    active: bool,
    last_frame: Instant,
    window_start: Instant,
    on_time: f32,
    dist_time: f32,
    total: f32,
    completed: usize,
}

impl Tracking {
    pub fn new(cfg: &Config, rng: fastrand::Rng) -> Tracking {
        let now = Instant::now();
        Tracking {
            rng,
            size: (cfg.tracking_w, cfg.tracking_h),
            speed: cfg.tracking_speed,
            fill: cfg.target_fill,
            area: Rect::default(),
            pos: (0.0, 0.0),
            heading: 0.0,
            next_turn: now,
            cursor: None,
            active: false,
            last_frame: now,
            window_start: now,
            on_time: 0.0,
            dist_time: 0.0,
            total: 0.0,
            completed: 0,
        }
    }

    fn rect(&self) -> Rect {
        Rect { x: self.pos.0.round() as u16, y: self.pos.1.round() as u16, width: self.size.0, height: self.size.1 }
    }

    /// Avanza la física `dt` segundos y suma el tiempo con el cursor encima.
    fn step(&mut self, now: Instant, dt: f32) {
        let a = inner(self.area);
        if now >= self.next_turn {
            self.heading += (self.rng.f32() - 0.5) * std::f32::consts::PI;
            self.next_turn = now + Duration::from_millis(self.rng.u64(600..=1800));
        }
        // En vertical se avanza la mitad: las celdas miden el doble de alto.
        let (mut x, mut y) = (
            self.pos.0 + self.heading.cos() * self.speed * dt,
            self.pos.1 - self.heading.sin() * self.speed * dt / 2.0,
        );
        let (max_x, max_y) = (
            (a.right() as f32 - self.size.0 as f32).max(a.x as f32),
            (a.bottom() as f32 - self.size.1 as f32).max(a.y as f32),
        );
        if x < a.x as f32 || x > max_x {
            self.heading = std::f32::consts::PI - self.heading;
            x = x.clamp(a.x as f32, max_x);
        }
        if y < a.y as f32 || y > max_y {
            self.heading = -self.heading;
            y = y.clamp(a.y as f32, max_y);
        }
        self.pos = (x, y);
        if let Some(c) = self.cursor {
            let r = self.rect();
            if contains(&r, c.0, c.1) {
                self.on_time += dt;
            }
            self.dist_time += cell_distance(c, center(&r)) * dt;
        }
        self.total += dt;
    }

    fn close_window(&mut self) -> Step {
        let score = if self.total > 0.0 { self.on_time / self.total } else { 0.0 };
        let r = self.rect();
        let rec = TrialRecord {
            index: self.completed,
            level: None,
            n_boxes: 1,
            target_present: true,
            target_w: r.width,
            target_h: r.height,
            cursor_dist: (self.total > 0.0 && self.cursor.is_some()).then(|| self.dist_time / self.total),
            rt_ms: None,
            outcome: if score >= 0.5 { Outcome::Hit } else { Outcome::Miss },
            method: Method::Mouse,
            anticipation: false,
            start: None,
            target_pos: Some(pt(center(&r))),
            click: None,
            score: Some(score),
            radius: None,
            warmup: false,
        };
        self.completed += 1;
        self.on_time = 0.0;
        self.dist_time = 0.0;
        self.total = 0.0;
        Step::Done(rec)
    }
}

impl GameMode for Tracking {
    fn id(&self) -> &'static str {
        "tracking"
    }

    fn start_trial(&mut self, area: Rect, _params: &Params, now: Instant, cursor: Option<(f32, f32)>) {
        if cursor.is_some() {
            self.cursor = cursor;
        }
        if !self.active || self.area != area {
            self.area = area;
            let a = inner(area);
            self.pos = (
                a.x as f32 + (a.width.saturating_sub(self.size.0)) as f32 / 2.0,
                a.y as f32 + (a.height.saturating_sub(self.size.1)) as f32 / 2.0,
            );
            self.heading = self.rng.f32() * std::f32::consts::TAU;
            self.next_turn = now + Duration::from_millis(800);
            self.on_time = 0.0;
            self.dist_time = 0.0;
            self.total = 0.0;
        }
        self.active = true;
        self.last_frame = now;
        self.window_start = now;
    }

    fn deadline(&self) -> Option<Instant> {
        self.active.then(|| (self.last_frame + FRAME).min(self.window_start + WINDOW))
    }

    fn tick(&mut self, now: Instant) -> Step {
        match self.deadline() {
            Some(d) if now >= d => {}
            _ => return Step::Idle,
        }
        let dt = now.saturating_duration_since(self.last_frame).as_secs_f32().min(0.1);
        self.step(now, dt);
        self.last_frame = now;
        if now >= self.window_start + WINDOW {
            return self.close_window();
        }
        Step::Redraw
    }

    fn drawn(&mut self, _now: Instant, _cursor: Option<(f32, f32)>) {}

    fn respond(&mut self, _r: Response, _at: Instant) -> Step {
        Step::Idle
    }

    fn abort(&mut self) {
        self.active = false;
    }

    fn in_trial(&self) -> bool {
        self.active
    }

    fn render(&self, buf: &mut Buffer, _area: Rect, theme: &Theme) {
        if self.active {
            render::draw_box_themed(buf, self.rect(), self.fill, theme.target, None, theme);
        }
    }

    fn on_move(&mut self, pos: (f32, f32), _at: Instant) {
        self.cursor = Some(pos);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> Params {
        Params { box_w: 6, box_h: 3, min_boxes: 1, max_boxes: 1, timeout: Duration::from_secs(30), level: None }
    }

    #[test]
    fn stays_inside_area_and_moves() {
        let mut m = Tracking::new(&Config { tracking_speed: 40.0, ..Config::default() }, fastrand::Rng::with_seed(6));
        let t0 = Instant::now();
        m.start_trial(Rect::new(0, 0, 60, 20), &params(), t0, None);
        let start = m.pos;
        let mut t = t0;
        for _ in 0..600 {
            t += FRAME;
            m.step(t, FRAME.as_secs_f32());
            let r = m.rect();
            assert!(r.x >= 1 && r.right() <= 59 && r.y >= 1 && r.bottom() <= 19, "{r:?}");
        }
        assert_ne!(m.pos, start);
    }

    #[test]
    fn window_scores_time_on_target() {
        let mut m = Tracking::new(&Config { tracking_speed: 1.0, ..Config::default() }, fastrand::Rng::with_seed(2));
        let t0 = Instant::now();
        m.start_trial(Rect::new(0, 0, 80, 24), &params(), t0, None);
        // Cursor siempre sobre el centro del recuadro.
        let mut t = t0;
        let rec = loop {
            t += FRAME;
            let c = center(&m.rect());
            m.on_move(c, t);
            if let Step::Done(rec) = m.tick(t) {
                break rec;
            }
        };
        assert_eq!(rec.outcome, Outcome::Hit);
        assert!(rec.score.unwrap() > 0.9, "{rec:?}");
        assert!(rec.cursor_dist.unwrap() < 1.5);

        // Ahora lejos: casi nada encima.
        m.start_trial(Rect::new(0, 0, 80, 24), &params(), t, None);
        m.on_move((0.5, 0.5), t);
        let rec = loop {
            t += FRAME;
            if let Step::Done(rec) = m.tick(t) {
                break rec;
            }
        };
        assert_eq!(rec.outcome, Outcome::Miss);
        assert!(rec.score.unwrap() < 0.1);
    }
}
