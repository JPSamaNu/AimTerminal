//! Dificultad adaptativa: escalera 3 arriba / 1 abajo, que converge cerca del 80% de aciertos.

use crate::modes::{Outcome, Params};
use std::time::Duration;

/// (ancho, alto, recuadros mín, recuadros máx, tiempo límite ms)
const LEVELS: [(u16, u16, usize, usize, u64); 10] = [
    (10, 5, 3, 3, 2000),
    (9, 5, 3, 3, 1900),
    (8, 4, 3, 4, 1800),
    (7, 4, 3, 4, 1700),
    (6, 3, 4, 4, 1600),
    (6, 3, 4, 5, 1500),
    (5, 3, 4, 5, 1400),
    (5, 3, 5, 5, 1300),
    (4, 3, 5, 5, 1200),
    (3, 3, 5, 5, 1100),
];

pub const MAX_LEVEL: usize = LEVELS.len() - 1;
const UP_AFTER: usize = 3;

pub fn level_params(level: usize) -> Params {
    let (w, h, lo, hi, ms) = LEVELS[level.min(MAX_LEVEL)];
    Params {
        box_w: w,
        box_h: h,
        min_boxes: lo,
        max_boxes: hi,
        timeout: Duration::from_millis(ms),
        level: Some(level.min(MAX_LEVEL)),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Staircase {
    pub level: usize,
    streak: usize,
}

impl Staircase {
    pub fn new(start: usize) -> Staircase {
        Staircase { level: start.min(MAX_LEVEL), streak: 0 }
    }

    /// Sube tras 3 aciertos seguidos y baja tras cualquier fallo (error, omisión, falsa
    /// alarma o anticipación). El rechazo correcto no mueve la escalera ni corta la racha.
    pub fn update(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Hit => self.signal(true),
            Outcome::CorrectRejection => {}
            Outcome::Error | Outcome::Omission | Outcome::FalseAlarm | Outcome::Anticipation | Outcome::Miss => {
                self.signal(false)
            }
        }
    }

    /// Corta la racha sin cambiar de nivel.
    pub fn reset_streak(&mut self) {
        self.streak = 0;
    }

    /// Un éxito suma a la racha (3 seguidos suben un nivel); un fallo baja uno.
    pub fn signal(&mut self, success: bool) {
        if success {
            self.streak += 1;
            if self.streak >= UP_AFTER {
                self.level = (self.level + 1).min(MAX_LEVEL);
                self.streak = 0;
            }
        } else {
            self.level = self.level.saturating_sub(1);
            self.streak = 0;
        }
    }
}

/// Blancos adaptativo: un acierto rápido (hasta `fast_ms`) es un éxito; uno lento (más
/// de 1.5 veces eso) o un clic en el vacío es un fallo; lo de en medio solo corta la racha.
pub fn targets_signal(outcome: Outcome, rt_ms: Option<f64>, fast_ms: u64) -> Option<bool> {
    let fast = fast_ms as f64;
    match (outcome, rt_ms) {
        (Outcome::Hit, Some(rt)) if rt <= fast => Some(true),
        (Outcome::Hit, Some(rt)) if rt > fast * 1.5 => Some(false),
        (Outcome::Hit, _) => None,
        (Outcome::Miss | Outcome::Anticipation, _) => Some(false),
        _ => None,
    }
}

/// Blancos: exponente con que se sortea la distancia al centro (r = u^e, con u uniforme).
/// En el nivel 0 (e = 2.5) casi todos salen cerca del centro; en el 9 (e = 0.35) la
/// mayoría sale cerca del borde.
pub fn spawn_exponent(level: usize) -> f32 {
    let t = level.min(MAX_LEVEL) as f32 / MAX_LEVEL as f32;
    2.5 * (0.35f32 / 2.5).powf(t)
}

/// Distancia media al centro (0 a 1) que da un nivel: 1 / (e + 1).
#[cfg(test)]
pub fn mean_radius(level: usize) -> f32 {
    1.0 / (spawn_exponent(level) + 1.0)
}

impl Staircase {
    /// Nivel inicial a partir del nivel final de la sesión anterior: uno menos, para
    /// calentar sin empezar desde cero.
    pub fn resume(last_final: Option<usize>, start: usize) -> Staircase {
        Staircase::new(last_final.map(|l| l.saturating_sub(1)).unwrap_or(start))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Outcome::*;

    #[test]
    fn three_up_one_down() {
        let mut s = Staircase::new(2);
        s.update(Hit);
        s.update(Hit);
        assert_eq!(s.level, 2);
        s.update(Hit);
        assert_eq!(s.level, 3);
        s.update(Error);
        assert_eq!(s.level, 2);
        // La racha se reinicia tras bajar.
        s.update(Hit);
        s.update(Hit);
        assert_eq!(s.level, 2);
    }

    #[test]
    fn failure_breaks_streak() {
        let mut s = Staircase::new(0);
        s.update(Hit);
        s.update(Hit);
        s.update(Omission);
        s.update(Hit);
        assert_eq!(s.level, 0);
    }

    #[test]
    fn correct_rejection_is_neutral() {
        let mut s = Staircase::new(4);
        s.update(Hit);
        s.update(Hit);
        s.update(CorrectRejection);
        s.update(Hit);
        assert_eq!(s.level, 5);
    }

    #[test]
    fn bounded() {
        let mut s = Staircase::new(0);
        s.update(FalseAlarm);
        s.update(Anticipation);
        assert_eq!(s.level, 0);
        let mut s = Staircase::new(99);
        assert_eq!(s.level, MAX_LEVEL);
        for _ in 0..3 {
            s.update(Hit);
        }
        assert_eq!(s.level, MAX_LEVEL);
    }

    #[test]
    fn spawn_gets_farther_with_level() {
        assert!(mean_radius(0) < 0.3);
        assert!(mean_radius(9) > 0.7);
        for l in 1..=MAX_LEVEL {
            assert!(mean_radius(l) > mean_radius(l - 1));
        }
    }

    #[test]
    fn targets_signal_rewards_fast_hits() {
        assert_eq!(targets_signal(Outcome::Hit, Some(600.0), 900), Some(true));
        assert_eq!(targets_signal(Outcome::Hit, Some(1000.0), 900), None);
        assert_eq!(targets_signal(Outcome::Hit, Some(1500.0), 900), Some(false));
        assert_eq!(targets_signal(Outcome::Miss, Some(300.0), 900), Some(false));
    }

    #[test]
    fn resume_starts_one_below_last_level() {
        assert_eq!(Staircase::resume(Some(5), 0).level, 4);
        assert_eq!(Staircase::resume(Some(0), 0).level, 0);
        assert_eq!(Staircase::resume(None, 2).level, 2);
    }

    #[test]
    fn levels_get_harder() {
        for l in 1..=MAX_LEVEL {
            let (a, b) = (level_params(l - 1), level_params(l));
            assert!(b.timeout < a.timeout);
            assert!(b.box_w * b.box_h <= a.box_w * a.box_h);
            assert!(b.max_boxes >= a.max_boxes);
        }
    }

    #[test]
    fn converges_near_80_percent() {
        // Observador simulado cuya probabilidad de acierto baja con el nivel.
        let mut rng = fastrand::Rng::with_seed(11);
        let mut s = Staircase::new(0);
        let (mut hits, mut n) = (0, 0);
        for i in 0..20_000 {
            let p = 0.98 - 0.07 * s.level as f64;
            let hit = rng.f64() < p;
            if i > 1000 {
                n += 1;
                hits += hit as usize;
            }
            s.update(if hit { Hit } else { Error });
        }
        let rate = hits as f64 / n as f64;
        assert!((0.74..=0.86).contains(&rate), "tasa {rate}");
    }
}
