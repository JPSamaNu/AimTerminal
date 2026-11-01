//! Plan de entrenamiento: una rutina corta de 3 bloques elegidos según tus puntos débiles.
//!
//! Cada modo recibe una prioridad: sube si la última sesión muestra una debilidad clara
//! (precisión baja, comisiones, poco tiempo encima, fallos hacia una dirección), si nunca
//! lo jugaste o si hace una semana que no. Se toman los 3 de mayor prioridad. Si fallas
//! más hacia alguna dirección, el bloque de Flick hace salir los blancos el doble de veces
//! hacia allá.

use crate::aim::{self, AimReport};
use crate::config::{Config, GameKind};
use crate::history::{self, SessionRecord};

pub const BLOCKS: usize = 3;

#[derive(Clone, Debug)]
pub struct Block {
    pub cfg: Config,
    pub reason: String,
}

impl Block {
    pub fn kind(&self) -> GameKind {
        self.cfg.mode
    }

    /// Duración aproximada en segundos (cuenta regresiva y calentamiento incluidos).
    pub fn est_secs(&self) -> u64 {
        let per_trial = match self.kind() {
            GameKind::Decision | GameKind::Reaction | GameKind::GoNoGo => 2.5,
            _ => 1.2,
        };
        let warm = self.cfg.warmup_trials as f64 * per_trial;
        let main = match self.cfg.session_secs() {
            Some(s) => s as f64,
            None => self.cfg.session_trials() as f64 * per_trial,
        };
        (3.0 + warm + main).round() as u64
    }

    /// "20 blancos", "20 s"…
    pub fn size_label(&self) -> String {
        match self.cfg.session_secs() {
            Some(s) => format!("{s} s"),
            None => format!("{} intentos", self.cfg.session_trials()),
        }
    }
}

/// Días entre dos fechas `AAAA-MM-DD…`.
fn days_between(from: &str, to: &str) -> Option<i64> {
    let day = |d: &str| -> Option<i64> {
        Some(history::days_from_civil(d.get(0..4)?.parse().ok()?, d.get(5..7)?.parse().ok()?, d.get(8..10)?.parse().ok()?))
    };
    Some(day(to)? - day(from)?)
}

/// Direcciones con menos acierto (hasta 2), con datos suficientes y por debajo del 90%.
pub fn weak_directions(r: &AimReport) -> Vec<usize> {
    let mut dirs: Vec<(usize, f64)> =
        (0..8).filter(|d| r.dirs[*d].n >= 3).filter_map(|d| r.dirs[d].hit_rate.map(|h| (d, h))).collect();
    dirs.sort_by(|a, b| a.1.total_cmp(&b.1));
    dirs.into_iter().filter(|(_, h)| *h < 0.9).take(2).map(|(d, _)| d).collect()
}

/// Prioridad y motivo de un modo.
fn score(kind: GameKind, history: &[SessionRecord], aim: &AimReport, today: &str) -> (u32, String, Vec<usize>) {
    let last = history.iter().rev().find(|s| !s.incomplete && !s.baseline && s.mode == kind.id());
    let Some(last) = last else {
        return (2, "sin datos todavía".to_string(), Vec::new());
    };
    let st = &last.stats;
    let pct = |v: f64| format!("{:.0}%", v * 100.0);
    let (mut p, mut reason, mut focus) = (1, "mantener".to_string(), Vec::new());
    match kind {
        GameKind::Flick => {
            let weak = weak_directions(aim);
            if let Some(&d) = weak.first() {
                p = 3;
                reason = format!(
                    "fallas más hacia {} ({})",
                    aim::ARROWS[d],
                    pct(aim.dirs[d].hit_rate.unwrap_or(0.0))
                );
                focus = weak;
            } else if aim.along.is_some_and(|a| a.abs() >= 0.3) {
                p = 2;
                reason = aim::insights(aim).into_iter().find(|t| t.contains("pasas") || t.contains("corto")).unwrap_or(reason);
            }
        }
        GameKind::Decision if st.accuracy.is_some_and(|a| a < 0.8) => {
            p = 3;
            reason = format!("precisión baja ({})", pct(st.accuracy.unwrap_or(0.0)));
        }
        GameKind::GoNoGo if st.commission_rate.is_some_and(|c| c > 0.2) => {
            p = 3;
            reason = format!("muchas comisiones ({})", pct(st.commission_rate.unwrap_or(0.0)));
        }
        GameKind::Tracking if st.on_target.is_some_and(|o| o < 0.6) => {
            p = 3;
            reason = format!("poco tiempo encima ({})", pct(st.on_target.unwrap_or(0.0)));
        }
        GameKind::Targets if st.accuracy.is_some_and(|a| a < 0.85) => {
            p = 3;
            reason = format!("muchos clics fallidos ({} de acierto)", pct(st.accuracy.unwrap_or(0.0)));
        }
        _ => {}
    }
    if let Some(days) = days_between(&last.date, today).filter(|d| *d >= 7) {
        p += 1;
        if reason == "mantener" {
            reason = format!("hace {days} días que no lo juegas");
        }
    }
    (p, reason, focus)
}

/// Configuración corta de un bloque: menos intentos o segundos que una sesión normal.
fn block_cfg(base: &Config, kind: GameKind, first: bool, focus: Vec<usize>) -> Config {
    let mut c = Config { mode: kind, plan_block: true, flick_focus: focus, ..base.clone() };
    c.trials = 15;
    c.flick_trials = 20;
    c.targets_secs = 20;
    c.tracking_secs = 20;
    c.reaction_trials = 15;
    c.gonogo_trials = 20;
    // Solo el primer bloque calienta de verdad; después ya vienes en ritmo.
    if !first {
        c.warmup_trials = c.warmup_trials.min(1);
    }
    c
}

/// Arma el plan de hoy.
pub fn build(base: &Config, history: &[SessionRecord], aim: &AimReport, today: &str) -> Vec<Block> {
    let order = [
        GameKind::Flick,
        GameKind::Decision,
        GameKind::GoNoGo,
        GameKind::Tracking,
        GameKind::Targets,
        GameKind::Reaction,
    ];
    let mut scored: Vec<(u32, usize, GameKind, String, Vec<usize>)> = order
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let (p, reason, focus) = score(*k, history, aim, today);
            (p, i, *k, reason, focus)
        })
        .collect();
    // Mayor prioridad primero; a igual prioridad, el orden fijo de arriba.
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored
        .into_iter()
        .take(BLOCKS)
        .enumerate()
        .map(|(i, (_, _, kind, reason, focus))| Block { cfg: block_cfg(base, kind, i == 0, focus), reason })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aim::DirStats;
    use crate::config::Difficulty;
    use crate::stats::Stats;

    fn rec(mode: GameKind, date: &str, stats: Stats) -> SessionRecord {
        SessionRecord {
            version: 1,
            date: date.into(),
            mode: mode.id().into(),
            difficulty: Difficulty::Adaptive,
            baseline: false,
            incomplete: false,
            discarded: 0,
            final_level: None,
            duration_s: None,
            pixel_mouse: false,
            config: Config::default(),
            stats,
            trials: vec![],
        }
    }

    fn played_all_ok(date: &str) -> Vec<SessionRecord> {
        let ok = Stats { accuracy: Some(0.95), commission_rate: Some(0.05), on_target: Some(0.8), ..Stats::default() };
        GameKind::ALL.iter().map(|k| rec(*k, date, ok.clone())).collect()
    }

    #[test]
    fn weak_spots_come_first_with_reasons() {
        let mut h = played_all_ok("2026-11-01T10:00:00Z");
        h.push(rec(
            GameKind::GoNoGo,
            "2026-11-01T11:00:00Z",
            Stats { commission_rate: Some(0.4), accuracy: Some(0.9), ..Stats::default() },
        ));
        let mut aim = AimReport { total: 40, ..AimReport::default() };
        for d in 0..8 {
            aim.dirs[d] = DirStats { n: 5, hits: 5, hit_rate: Some(1.0), ..DirStats::default() };
        }
        aim.dirs[5] = DirStats { n: 5, hits: 3, hit_rate: Some(0.6), ..DirStats::default() };
        let plan = build(&Config::default(), &h, &aim, "2026-11-02");
        assert_eq!(plan.len(), 3);
        assert_eq!(plan[0].kind(), GameKind::Flick);
        assert!(plan[0].reason.contains("↙"), "{}", plan[0].reason);
        assert_eq!(plan[0].cfg.flick_focus, vec![5]);
        assert_eq!(plan[1].kind(), GameKind::GoNoGo);
        assert!(plan[1].reason.contains("comisiones"));
        assert!(plan.iter().all(|b| b.cfg.plan_block));
        // Bloques cortos; solo el primero calienta completo.
        assert_eq!(plan[0].cfg.flick_trials, 20);
        assert_eq!(plan[0].cfg.warmup_trials, Config::default().warmup_trials);
        assert_eq!(plan[1].cfg.warmup_trials, 1);
    }

    #[test]
    fn unplayed_and_stale_modes_get_priority() {
        let h = vec![rec(GameKind::Flick, "2026-10-01T10:00:00Z", Stats { accuracy: Some(0.95), ..Stats::default() })];
        let plan = build(&Config::default(), &h, &AimReport::default(), "2026-11-02");
        // Flick: viejo (1+1=2) empata con los nunca jugados (2) y va primero por el orden.
        assert_eq!(plan[0].kind(), GameKind::Flick);
        assert!(plan[0].reason.contains("días"), "{}", plan[0].reason);
        assert!(plan[1].reason.contains("sin datos"));
    }

    #[test]
    fn weak_directions_need_data() {
        let mut aim = AimReport::default();
        aim.dirs[2] = DirStats { n: 2, hits: 0, hit_rate: Some(0.0), ..DirStats::default() };
        aim.dirs[4] = DirStats { n: 6, hits: 3, hit_rate: Some(0.5), ..DirStats::default() };
        aim.dirs[6] = DirStats { n: 6, hits: 5, hit_rate: Some(0.83), ..DirStats::default() };
        assert_eq!(weak_directions(&aim), vec![4, 6]);
    }
}
