//! Qué métricas se muestran de cada modo, en el resumen y en el progreso. Los dos estilos
//! de pantalla (recuadros y código) las dibujan a su manera.

use crate::aim;
use crate::app::Session;
use crate::config::{Difficulty, GameKind};
use crate::modes::{Outcome, TrialRecord};
use crate::stats::{Stats, median};

pub fn ms(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.0} ms")).unwrap_or_else(|| "-".into())
}

pub fn pct(v: Option<f64>) -> String {
    v.map(|x| format!("{:.0}%", x * 100.0)).unwrap_or_else(|| "-".into())
}

pub fn bits(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.1} b/s")).unwrap_or_else(|| "-".into())
}

/// Blancos: mediana de los aciertos cerca del centro (radio < 0.5) y lejos.
pub fn near_far(records: &[TrialRecord]) -> (Option<f64>, Option<f64>) {
    let split = |far: bool| -> Vec<f64> {
        records
            .iter()
            .filter(|r| r.outcome == Outcome::Hit && r.radius.is_some_and(|x| (x >= 0.5) == far))
            .filter_map(|r| r.rt_ms)
            .collect()
    };
    (median(&split(false)), median(&split(true)))
}

pub fn cells(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.1} c")).unwrap_or_else(|| "-".into())
}

/// Una fila del resumen.
pub struct Metric {
    pub label: &'static str,
    pub value: String,
    /// Valor numérico de esta sesión y de la anterior, para la diferencia.
    pub now: Option<f64>,
    pub before: Option<f64>,
    pub lower_is_better: bool,
    /// Multiplica la diferencia (100 para porcentajes).
    pub scale: f64,
    /// 0 siempre visible; mayor se oculta antes en paneles pequeños.
    pub prio: u8,
    /// La métrica principal del modo.
    pub strong: bool,
    /// Decimales de la diferencia.
    pub decimals: usize,
}

impl Metric {
    fn plain(label: &'static str, value: String, prio: u8) -> Metric {
        Metric {
            label,
            value,
            now: None,
            before: None,
            lower_is_better: true,
            scale: 1.0,
            prio,
            strong: false,
            decimals: 0,
        }
    }

    fn cmp(label: &'static str, value: String, now: Option<f64>, before: Option<f64>, lower: bool, scale: f64) -> Metric {
        Metric { label, value, now, before, lower_is_better: lower, scale, prio: 0, strong: false, decimals: 0 }
    }

    fn strong(mut self) -> Metric {
        self.strong = true;
        self
    }
}

pub fn hint(kind: GameKind) -> &'static str {
    match kind {
        GameKind::Decision => "elige el relleno · clic o 1-9",
        GameKind::Targets => "clic en los verdes · solo mouse",
        GameKind::Flick => "un blanco a la vez · solo mouse",
        GameKind::Tracking => "mantén el cursor encima",
        GameKind::Reaction => "espacio, número o clic al verlo",
        GameKind::GoNoGo => "relleno: responde · hueco: no",
    }
}

/// Título de la sesión.
pub fn title(s: &Session, st: &Stats) -> String {
    let kind = s.cfg.mode;
    let mut t = match kind {
        GameKind::Decision if s.baseline => format!("baseline · {} · {}", kind.label(), st.trials),
        GameKind::Decision => format!("{} · {} · {}", kind.label(), s.cfg.difficulty.label(), st.trials),
        GameKind::Targets => format!(
            "{} · {} s · {}",
            kind.label(),
            s.cfg.session_secs().unwrap_or(0),
            s.cfg.difficulty_label().unwrap_or_default()
        ),
        GameKind::Tracking => format!("{} · {} s", kind.label(), s.cfg.session_secs().unwrap_or(0)),
        GameKind::Flick => {
            format!("{} · {} · {}", kind.label(), s.counted, s.cfg.difficulty_label().unwrap_or_default())
        }
        _ => format!("{} · {}", kind.label(), s.counted),
    };
    if let (Difficulty::Adaptive, Some(l)) = (s.cfg.mode_difficulty(), s.final_level()) {
        t.push_str(&format!(" · niv {l}"));
    }
    t
}

/// Filas del resumen y líneas de detalle (con su prioridad).
pub fn summary(s: &Session, st: &Stats) -> (Vec<Metric>, Vec<(String, u8)>) {
    let (metrics, mut notes) = mode_summary(s, st);
    let warm = s.records.iter().filter(|r| r.warmup).count();
    if warm > 0 {
        notes.push((format!("calentamiento: {warm} intentos, no cuentan"), 2));
    }
    (metrics, notes)
}

fn mode_summary(s: &Session, st: &Stats) -> (Vec<Metric>, Vec<(String, u8)>) {
    let p = s.prev.as_ref();
    let before = |f: fn(&Stats) -> Option<f64>| p.and_then(f);
    let hits = |x: &Stats| Some(x.hits as f64);
    let median = Metric::cmp("mediana", ms(st.median_ms), st.median_ms, before(|x| x.median_ms), true, 1.0);
    let accuracy = Metric::cmp("precisión", pct(st.accuracy), st.accuracy, before(|x| x.accuracy), false, 100.0);
    let score = Metric::cmp("puntaje", ms(st.ies_ms), st.ies_ms, before(|x| x.ies_ms), true, 1.0).strong();
    let mean = Metric::plain("media", ms(st.mean_ms), 2);
    let throughput = Metric {
        prio: 1,
        decimals: 1,
        ..Metric::cmp("rendimiento", bits(st.throughput), st.throughput, before(|x| x.throughput), false, 1.0)
    };
    let best = Metric::plain("mejor", ms(st.best_ms), 1);
    match s.cfg.mode {
        GameKind::Decision => (
            vec![median, mean, best, accuracy, score],
            vec![
                (format!("errores {} · omisiones {} · f.alarmas {}", st.errors, st.omissions, st.false_alarms), 0),
                (
                    format!(
                        "rech.corr. {}/{} · anticip. {} · desc. {}",
                        st.correct_rejections, st.catch_trials, st.anticipations, s.discarded
                    ),
                    1,
                ),
            ],
        ),
        GameKind::Targets => {
            let secs = s.cfg.session_secs().unwrap_or(1).max(1) as f64;
            (
                vec![
                    Metric::cmp("aciertos", st.hits.to_string(), hits(st), before(hits), false, 1.0),
                    Metric::plain("fallos", (st.misses + st.anticipations).to_string(), 0),
                    accuracy,
                    Metric::plain("ritmo", format!("{:.1}/s", st.hits as f64 / secs), 1),
                    Metric { label: "entre aciertos", ..median },
                    Metric { prio: 2, ..best },
                    throughput,
                    score,
                ],
                vec![
                    match near_far(&s.records) {
                        (Some(n), Some(f)) => (format!("cerca del centro {n:.0} ms · lejos {f:.0} ms"), 1),
                        _ => (String::new(), 9),
                    },
                    (format!("anticip. {} · desc. {}", st.anticipations, s.discarded), 2),
                ],
            )
        }
        GameKind::Flick => {
            let report = aim::analyze(&s.records, 1);
            let tip = aim::insights(&report).into_iter().next().unwrap_or_default();
            (
                vec![
                    Metric::cmp("aciertos", st.hits.to_string(), hits(st), before(hits), false, 1.0),
                    Metric::plain("fallos", st.misses.to_string(), 0),
                    Metric::plain("omisiones", st.omissions.to_string(), 1),
                    accuracy,
                    median,
                    Metric { prio: 2, ..best },
                    throughput,
                    score,
                ],
                vec![(tip, 1), ("a: análisis de puntería por dirección".to_string(), 2)],
            )
        }
        GameKind::Tracking => (
            vec![
                Metric::cmp("encima", pct(st.on_target), st.on_target, before(|x| x.on_target), false, 100.0).strong(),
                Metric { decimals: 1, ..Metric::cmp("distancia", cells(st.mean_dist), st.mean_dist, before(|x| x.mean_dist), true, 1.0) },
                Metric { prio: 1, ..Metric::plain("seg. ≥50%", pct(st.accuracy), 1) },
            ],
            vec![(format!("{} segundos medidos · desc. {}", st.trials, s.discarded), 1)],
        ),
        GameKind::Reaction => (
            vec![
                median,
                mean,
                best,
                Metric::plain("omisiones", st.omissions.to_string(), 0),
                Metric::plain("anticip.", st.anticipations.to_string(), 1),
                score,
            ],
            vec![(format!("desc. {}", s.discarded), 2)],
        ),
        GameKind::GoNoGo => (
            vec![
                Metric { label: "mediana go", ..median },
                Metric { label: "precisión go", ..accuracy },
                Metric::cmp(
                    "comisiones",
                    pct(st.commission_rate),
                    st.commission_rate,
                    before(|x| x.commission_rate),
                    true,
                    100.0,
                ),
                Metric::plain("omisiones", st.omissions.to_string(), 1),
                score,
            ],
            vec![(
                format!(
                    "no-go: {} de {} sin responder · anticip. {}",
                    st.correct_rejections, st.catch_trials, st.anticipations
                ),
                1,
            )],
        ),
    }
}

/// Métrica de la gráfica de progreso.
pub struct Series {
    pub label: &'static str,
    pub get: fn(&Stats) -> Option<f64>,
    pub lower_is_better: bool,
    pub fmt: fn(Option<f64>) -> String,
}

pub fn series(kind: GameKind) -> [Series; 3] {
    let median = Series { label: "mediana", get: |s| s.median_ms, lower_is_better: true, fmt: ms };
    let accuracy = Series { label: "precisión", get: |s| s.accuracy, lower_is_better: false, fmt: pct };
    let score = Series { label: "puntaje", get: |s| s.ies_ms, lower_is_better: true, fmt: ms };
    match kind {
        GameKind::Tracking => [
            Series { label: "encima", get: |s| s.on_target, lower_is_better: false, fmt: pct },
            Series { label: "distancia", get: |s| s.mean_dist, lower_is_better: true, fmt: cells },
            Series { label: "seg. ≥50%", ..accuracy },
        ],
        GameKind::GoNoGo => [
            median,
            Series { label: "comisiones", get: |s| s.commission_rate, lower_is_better: true, fmt: pct },
            score,
        ],
        GameKind::Targets => [Series { label: "entre aciertos", ..median }, accuracy, score],
        _ => [median, accuracy, score],
    }
}

/// Valor principal de una sesión para las tablas (columna 2) y su encabezado.
pub fn main_column(kind: GameKind) -> (&'static str, fn(&Stats) -> String) {
    match kind {
        GameKind::Targets | GameKind::Flick => ("aciertos", |s| s.hits.to_string()),
        GameKind::Tracking => ("encima", |s| pct(s.on_target)),
        _ => ("mediana", |s| ms(s.median_ms)),
    }
}

/// Valor de la última columna (puntaje, o distancia en Seguimiento).
pub fn score_column(kind: GameKind) -> (&'static str, fn(&Stats) -> String) {
    match kind {
        GameKind::Tracking => ("distancia", |s| cells(s.mean_dist)),
        _ => ("puntaje", |s| ms(s.ies_ms)),
    }
}
