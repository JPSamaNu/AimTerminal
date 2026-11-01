//! Análisis de puntería por dirección, al estilo de Aim Lab: con la posición del cursor
//! al aparecer cada blanco, el centro del blanco y el punto del clic se calcula hacia
//! dónde ibas (8 direcciones), cuánto acertaste y tardaste en cada una, y si te pasas
//! del blanco o te quedas corto (a lo largo del movimiento) o te desvías a un lado.
//!
//! Las distancias van en anchos de celda; la vertical cuenta doble porque una celda
//! mide más o menos el doble de alto que de ancho.

use crate::modes::{Method, Outcome, TrialRecord};
use crate::stats::{mean, median};

pub const ARROWS: [&str; 8] = ["→", "↗", "↑", "↖", "←", "↙", "↓", "↘"];
pub const NAMES: [&str; 8] =
    ["derecha", "arriba-der.", "arriba", "arriba-izq.", "izquierda", "abajo-izq.", "abajo", "abajo-der."];

/// Movimientos más cortos que esto no tienen una dirección fiable.
const MIN_MOVE: f32 = 2.0;
/// Datos mínimos por dirección para sacar conclusiones.
const MIN_N: usize = 3;
/// Sesgos menores que esto (en celdas) se consideran ruido.
const BIAS: f64 = 0.3;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DirStats {
    pub n: usize,
    pub hits: usize,
    pub hit_rate: Option<f64>,
    pub median_ms: Option<f64>,
    /// Positivo: te pasas del blanco; negativo: te quedas corto.
    pub along: Option<f64>,
    /// Positivo: a la derecha de la trayectoria; negativo: a la izquierda.
    pub across: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AimReport {
    pub dirs: [DirStats; 8],
    pub total: usize,
    pub sessions: usize,
    pub median_ms: Option<f64>,
    pub along: Option<f64>,
    pub across: Option<f64>,
}

/// Dirección (0 = derecha, en sentido antihorario de 45 en 45 grados) y, si hubo clic,
/// el error a lo largo y a lo ancho del movimiento.
pub fn geometry(r: &TrialRecord) -> Option<(usize, Option<(f64, f64)>)> {
    let ([sx, sy], [tx, ty]) = (r.start?, r.target_pos?);
    // Coordenadas "matemáticas": y hacia arriba y escala vertical corregida.
    let (vx, vy) = (tx - sx, -(ty - sy) * 2.0);
    let len = vx.hypot(vy);
    if len < MIN_MOVE {
        return None;
    }
    let angle = vy.atan2(vx).to_degrees().rem_euclid(360.0);
    let sector = ((angle + 22.5) / 45.0) as usize % 8;
    let err = r.click.map(|[cx, cy]| {
        let (ox, oy) = (cx - tx, -(cy - ty) * 2.0);
        let (ux, uy) = (vx / len, vy / len);
        let along = ox * ux + oy * uy;
        // Perpendicular hacia la derecha de quien se mueve en la dirección (ux, uy).
        let across = ox * uy - oy * ux;
        (along as f64, across as f64)
    });
    Some((sector, err))
}

/// Junta los intentos con mouse que tienen geometría.
pub fn analyze<'a>(records: impl IntoIterator<Item = &'a TrialRecord>, sessions: usize) -> AimReport {
    let mut rts: [Vec<f64>; 8] = Default::default();
    let mut along: [Vec<f64>; 8] = Default::default();
    let mut across: [Vec<f64>; 8] = Default::default();
    let mut report = AimReport { sessions, ..AimReport::default() };
    let mut all_rts = Vec::new();
    for r in records {
        if r.warmup {
            continue;
        }
        if r.method != Method::Mouse && r.outcome != Outcome::Omission {
            continue;
        }
        if !matches!(r.outcome, Outcome::Hit | Outcome::Miss | Outcome::Omission | Outcome::Error) {
            continue;
        }
        let Some((d, err)) = geometry(r) else { continue };
        report.total += 1;
        let ds = &mut report.dirs[d];
        ds.n += 1;
        if r.outcome == Outcome::Hit {
            ds.hits += 1;
            if let Some(ms) = r.rt_ms {
                rts[d].push(ms);
                all_rts.push(ms);
            }
        }
        if let (Some((a, c)), Outcome::Hit | Outcome::Miss) = (err, r.outcome) {
            along[d].push(a);
            across[d].push(c);
        }
    }
    for d in 0..8 {
        let ds = &mut report.dirs[d];
        ds.hit_rate = (ds.n > 0).then(|| ds.hits as f64 / ds.n as f64);
        ds.median_ms = median(&rts[d]);
        ds.along = mean(&along[d]);
        ds.across = mean(&across[d]);
    }
    report.median_ms = median(&all_rts);
    report.along = mean(&along.concat());
    report.across = mean(&across.concat());
    report
}

/// Conclusiones en palabras, de la más útil a la menos.
pub fn insights(r: &AimReport) -> Vec<String> {
    let mut out = Vec::new();
    if r.total < 8 {
        out.push(format!("pocos datos todavía ({} movimientos); juega flick o blancos", r.total));
        return out;
    }
    let enough = |d: &&(usize, &DirStats)| d.1.n >= MIN_N;
    let dirs: Vec<(usize, &DirStats)> = r.dirs.iter().enumerate().collect();
    if let Some((i, d)) = dirs
        .iter()
        .filter(enough)
        .filter(|(_, d)| d.hit_rate.is_some())
        .min_by(|a, b| a.1.hit_rate.unwrap_or(1.0).total_cmp(&b.1.hit_rate.unwrap_or(1.0)))
        && d.hit_rate.unwrap_or(1.0) < 0.95
    {
        out.push(format!("más fallos hacia {} {} ({:.0}%)", ARROWS[*i], NAMES[*i], d.hit_rate.unwrap_or(0.0) * 100.0));
    }
    if let (Some((i, d)), Some(all)) = (
        dirs.iter()
            .filter(|(_, d)| d.hits >= MIN_N)
            .max_by(|a, b| a.1.median_ms.unwrap_or(0.0).total_cmp(&b.1.median_ms.unwrap_or(0.0))),
        r.median_ms,
    ) {
        let m = d.median_ms.unwrap_or(all);
        if m - all >= 20.0 {
            out.push(format!("más lento hacia {} {} ({m:.0} ms, +{:.0})", ARROWS[*i], NAMES[*i], m - all));
        }
    }
    if let Some(a) = r.along.filter(|a| a.abs() >= BIAS) {
        let what = if a > 0.0 { "te pasas del blanco" } else { "te quedas corto" };
        let worst = dirs
            .iter()
            .filter(enough)
            .filter(|(_, d)| d.along.is_some_and(|x| x.signum() == a.signum()))
            .max_by(|x, y| x.1.along.unwrap_or(0.0).abs().total_cmp(&y.1.along.unwrap_or(0.0).abs()));
        match worst {
            Some((i, _)) => out.push(format!("{what} ({a:+.1} celdas), sobre todo hacia {}", ARROWS[*i])),
            None => out.push(format!("{what} ({a:+.1} celdas)")),
        }
    }
    if let Some(c) = r.across.filter(|c| c.abs() >= BIAS) {
        let side = if c > 0.0 { "derecha" } else { "izquierda" };
        out.push(format!("te desvías a la {side} de la trayectoria ({:.1} celdas)", c.abs()));
    }
    if out.is_empty() {
        out.push("sin sesgos claros: parejo en todas las direcciones".to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(start: [f32; 2], target: [f32; 2], click: Option<[f32; 2]>, outcome: Outcome, rt: f64) -> TrialRecord {
        TrialRecord {
            index: 0,
            level: None,
            n_boxes: 1,
            target_present: true,
            target_w: 4,
            target_h: 2,
            cursor_dist: None,
            rt_ms: Some(rt),
            outcome,
            method: Method::Mouse,
            anticipation: false,
            start: Some(start),
            target_pos: Some(target),
            click,
            score: None,
            radius: None,
            warmup: false,
        }
    }

    #[test]
    fn sectors_follow_screen_directions() {
        let s = [50.0, 20.0];
        let cases = [
            ([70.0, 20.0], 0),
            ([65.0, 12.5], 1),
            ([50.0, 10.0], 2),
            ([35.0, 12.5], 3),
            ([30.0, 20.0], 4),
            ([35.0, 27.5], 5),
            ([50.0, 30.0], 6),
            ([65.0, 27.5], 7),
        ];
        for (t, want) in cases {
            assert_eq!(geometry(&rec(s, t, None, Outcome::Hit, 300.0)).unwrap().0, want, "{t:?}");
        }
        assert!(geometry(&rec(s, [50.5, 20.0], None, Outcome::Hit, 300.0)).is_none());
    }

    #[test]
    fn overshoot_and_side_error() {
        // Moviéndose a la derecha, el clic 1 celda más allá y 1 fila más abajo:
        // se pasa (+1) y se desvía a la derecha de la trayectoria (+2, por la escala vertical).
        let r = rec([10.0, 10.0], [30.0, 10.0], Some([31.0, 11.0]), Outcome::Hit, 300.0);
        let (_, err) = geometry(&r).unwrap();
        let (along, across) = err.unwrap();
        assert!((along - 1.0).abs() < 1e-6 && (across - 2.0).abs() < 1e-6, "{along} {across}");
        // Moviéndose hacia arriba, clic por debajo del centro: se queda corto.
        let r = rec([10.0, 20.0], [10.0, 5.0], Some([10.0, 6.0]), Outcome::Miss, 300.0);
        let (along, _) = geometry(&r).unwrap().1.unwrap();
        assert!(along < 0.0);
    }

    #[test]
    fn report_finds_weak_direction_and_bias() {
        let mut rs = Vec::new();
        for i in 0..6 {
            // Derecha: aciertos rápidos pero pasándose.
            rs.push(rec([10.0, 10.0], [40.0, 10.0], Some([41.5, 10.0]), Outcome::Hit, 300.0 + i as f64));
            // Izquierda: la mitad son fallos y más lentos.
            let o = if i % 2 == 0 { Outcome::Miss } else { Outcome::Hit };
            rs.push(rec([40.0, 10.0], [10.0, 10.0], Some([9.0, 10.0]), o, 450.0));
        }
        let r = analyze(&rs, 1);
        assert_eq!(r.total, 12);
        assert_eq!(r.dirs[0].hit_rate, Some(1.0));
        assert_eq!(r.dirs[4].hit_rate, Some(0.5));
        let text = insights(&r).join("\n");
        assert!(text.contains("más fallos hacia ←"), "{text}");
        assert!(text.contains("más lento hacia ←"), "{text}");
        assert!(text.contains("te pasas del blanco"), "{text}");
    }

    #[test]
    fn little_data_says_so() {
        let r = analyze(&[rec([0.0, 0.0], [20.0, 0.0], None, Outcome::Hit, 300.0)], 1);
        assert!(insights(&r)[0].contains("pocos datos"));
    }
}
