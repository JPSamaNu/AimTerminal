//! Estadísticas de una sesión.

use crate::modes::{Outcome, TrialRecord};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stats {
    pub trials: usize,
    pub target_trials: usize,
    pub catch_trials: usize,
    pub hits: usize,
    pub errors: usize,
    pub omissions: usize,
    pub false_alarms: usize,
    pub correct_rejections: usize,
    pub anticipations: usize,
    /// Clics en el vacío (modo Blancos).
    pub misses: usize,
    /// Tiempos de los aciertos (sin anticipaciones).
    pub median_ms: Option<f64>,
    pub mean_ms: Option<f64>,
    pub best_ms: Option<f64>,
    /// Aciertos / intentos con objetivo.
    pub accuracy: Option<f64>,
    /// Eficiencia inversa: mediana de aciertos / precisión. Menor es mejor.
    pub ies_ms: Option<f64>,
}

pub fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    Some(if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 })
}

pub fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() { None } else { Some(values.iter().sum::<f64>() / values.len() as f64) }
}

/// Eficiencia inversa. Sin aciertos no hay puntaje.
pub fn inverse_efficiency(median_ms: Option<f64>, accuracy: Option<f64>) -> Option<f64> {
    match (median_ms, accuracy) {
        (Some(m), Some(a)) if a > 0.0 => Some(m / a),
        _ => None,
    }
}

pub fn compute(records: &[TrialRecord]) -> Stats {
    let mut s = Stats { trials: records.len(), ..Stats::default() };
    let mut hit_rts = Vec::new();
    for r in records {
        if r.target_present {
            s.target_trials += 1;
        } else {
            s.catch_trials += 1;
        }
        match r.outcome {
            Outcome::Hit => {
                s.hits += 1;
                if let Some(ms) = r.rt_ms {
                    hit_rts.push(ms);
                }
            }
            Outcome::Error => s.errors += 1,
            Outcome::Omission => s.omissions += 1,
            Outcome::FalseAlarm => s.false_alarms += 1,
            Outcome::CorrectRejection => s.correct_rejections += 1,
            Outcome::Anticipation => s.anticipations += 1,
            Outcome::Miss => s.misses += 1,
        }
    }
    s.median_ms = median(&hit_rts);
    s.mean_ms = mean(&hit_rts);
    s.best_ms = hit_rts.iter().copied().reduce(f64::min);
    s.accuracy = (s.target_trials > 0).then(|| s.hits as f64 / s.target_trials as f64);
    s.ies_ms = inverse_efficiency(s.median_ms, s.accuracy);
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::Method;

    fn rec(outcome: Outcome, rt: Option<f64>, target: bool) -> TrialRecord {
        TrialRecord {
            index: 0,
            level: None,
            n_boxes: 3,
            target_present: target,
            target_w: 6,
            target_h: 3,
            cursor_dist: None,
            rt_ms: rt,
            outcome,
            method: Method::Keyboard,
            anticipation: outcome == Outcome::Anticipation,
        }
    }

    #[test]
    fn median_odd_even_empty() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[4.0, 1.0, 3.0, 2.0]), Some(2.5));
        assert_eq!(median(&[]), None);
    }

    #[test]
    fn counts_and_times() {
        let rs = vec![
            rec(Outcome::Hit, Some(300.0), true),
            rec(Outcome::Hit, Some(500.0), true),
            rec(Outcome::Hit, Some(400.0), true),
            rec(Outcome::Error, Some(250.0), true),
            rec(Outcome::Omission, None, true),
            rec(Outcome::Anticipation, Some(80.0), true),
            rec(Outcome::FalseAlarm, Some(350.0), false),
            rec(Outcome::CorrectRejection, None, false),
        ];
        let s = compute(&rs);
        assert_eq!((s.trials, s.target_trials, s.catch_trials), (8, 6, 2));
        assert_eq!((s.hits, s.errors, s.omissions), (3, 1, 1));
        assert_eq!((s.false_alarms, s.correct_rejections, s.anticipations), (1, 1, 1));
        // Solo los aciertos entran a los tiempos: ni el error ni la anticipación.
        assert_eq!(s.median_ms, Some(400.0));
        assert_eq!(s.mean_ms, Some(400.0));
        assert_eq!(s.best_ms, Some(300.0));
        assert_eq!(s.accuracy, Some(0.5));
        assert_eq!(s.ies_ms, Some(800.0));
    }

    #[test]
    fn fast_but_sloppy_scores_worse() {
        // 300 ms con 60% de aciertos es peor que 350 ms con 100%.
        let sloppy = inverse_efficiency(Some(300.0), Some(0.6)).unwrap();
        let careful = inverse_efficiency(Some(350.0), Some(1.0)).unwrap();
        assert!(sloppy > careful);
    }

    #[test]
    fn no_hits_no_score() {
        let s = compute(&[rec(Outcome::Error, Some(300.0), true)]);
        assert_eq!(s.accuracy, Some(0.0));
        assert_eq!(s.median_ms, None);
        assert_eq!(s.ies_ms, None);
        assert_eq!(compute(&[]).accuracy, None);
    }
}
