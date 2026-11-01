//! Historial en JSON Lines (una sesión por línea) y exportación a CSV.

use crate::aim;
use crate::config::{self, Config, Difficulty};
use crate::modes::TrialRecord;
use crate::stats::Stats;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionRecord {
    pub version: u32,
    /// Fecha y hora UTC, ISO 8601.
    pub date: String,
    pub mode: String,
    pub difficulty: Difficulty,
    pub baseline: bool,
    pub incomplete: bool,
    #[serde(default)]
    pub discarded: usize,
    #[serde(default)]
    pub final_level: Option<usize>,
    /// Duración activa en segundos (sesiones por tiempo).
    #[serde(default)]
    pub duration_s: Option<f64>,
    #[serde(default)]
    pub pixel_mouse: bool,
    pub config: Config,
    pub stats: Stats,
    pub trials: Vec<TrialRecord>,
}

pub fn history_path() -> PathBuf {
    config::data_dir().join("history.jsonl")
}

pub fn append(path: &Path, rec: &SessionRecord) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut line = serde_json::to_string(rec).map_err(io::Error::other)?;
    line.push('\n');
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    f.write_all(line.as_bytes())
}

/// Lee todas las sesiones. Las líneas dañadas se saltan y se cuentan.
pub fn load(path: &Path) -> (Vec<SessionRecord>, usize) {
    let Ok(f) = fs::File::open(path) else { return (Vec::new(), 0) };
    let mut out = Vec::new();
    let mut bad = 0;
    for line in BufReader::new(f).lines() {
        let Ok(line) = line else {
            bad += 1;
            continue;
        };
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str(&line) {
            Ok(r) => out.push(r),
            Err(_) => bad += 1,
        }
    }
    (out, bad)
}

pub const CSV_HEADER: &str = "session_date,mode,baseline,incomplete,difficulty,trial,level,n_boxes,\
target_present,target_w,target_h,cursor_dist,rt_ms,outcome,method,anticipation,start_x,start_y,target_x,target_y,click_x,click_y,direction,overshoot,lateral,score,radius,warmup";

fn opt<T: std::fmt::Display>(v: Option<T>) -> String {
    v.map(|x| x.to_string()).unwrap_or_default()
}

/// Escribe un intento por fila. Devuelve cuántas filas escribió.
pub fn write_csv<W: Write>(sessions: &[SessionRecord], mut w: W) -> io::Result<usize> {
    writeln!(w, "{CSV_HEADER}")?;
    let mut n = 0;
    for s in sessions {
        let diff = match s.difficulty {
            Difficulty::Fixed => "fixed",
            Difficulty::Adaptive => "adaptive",
        };
        for t in &s.trials {
            let outcome = serde_json::to_value(t.outcome).ok().and_then(|v| v.as_str().map(String::from));
            writeln!(
                w,
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                s.date,
                s.mode,
                s.baseline,
                s.incomplete,
                diff,
                t.index,
                opt(t.level),
                t.n_boxes,
                t.target_present,
                t.target_w,
                t.target_h,
                opt(t.cursor_dist.map(|d| format!("{d:.2}"))),
                opt(t.rt_ms.map(|r| format!("{r:.1}"))),
                outcome.unwrap_or_default(),
                t.method.label(),
                t.anticipation,
                geometry_columns(t),
            )?;
            n += 1;
        }
    }
    w.flush()?;
    Ok(n)
}

/// Columnas de posición y del análisis por dirección (vacías si no hay datos).
fn geometry_columns(t: &TrialRecord) -> String {
    let p = |v: Option<[f32; 2]>| match v {
        Some([x, y]) => format!("{x:.2},{y:.2}"),
        None => ",".to_string(),
    };
    let (dir, along, across) = match aim::geometry(t) {
        Some((d, err)) => (
            aim::NAMES[d].to_string(),
            opt(err.map(|e| format!("{:.2}", e.0))),
            opt(err.map(|e| format!("{:.2}", e.1))),
        ),
        None => (String::new(), String::new(), String::new()),
    };
    format!(
        "{},{},{},{dir},{along},{across},{},{},{}",
        p(t.start),
        p(t.target_pos),
        p(t.click),
        opt(t.score.map(|s| format!("{s:.3}"))),
        opt(t.radius.map(|r| format!("{r:.3}"))),
        t.warmup
    )
}

pub fn now_iso() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    iso_utc(secs)
}

/// Segundos Unix a `AAAA-MM-DDTHH:MM:SSZ` (algoritmo de días civiles de H. Hinnant).
pub fn iso_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::{Method, Outcome};

    #[test]
    fn iso_dates() {
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(iso_utc(1_793_532_245), "2026-11-01T11:24:05Z");
    }

    fn sample(incomplete: bool) -> SessionRecord {
        SessionRecord {
            version: 1,
            date: "2026-11-01T10:00:00Z".into(),
            mode: "decision".into(),
            difficulty: Difficulty::Adaptive,
            baseline: false,
            incomplete,
            discarded: 1,
            final_level: Some(3),
            duration_s: None,
            pixel_mouse: false,
            config: Config::default(),
            stats: Stats::default(),
            trials: vec![TrialRecord {
                index: 0,
                level: Some(2),
                n_boxes: 4,
                target_present: true,
                target_w: 6,
                target_h: 3,
                cursor_dist: Some(12.345),
                rt_ms: Some(401.26),
                outcome: Outcome::Hit,
                method: Method::Mouse,
                anticipation: false,
                start: None,
                target_pos: None,
                click: None,
                score: None,
                radius: None,
                warmup: false,
            }],
        }
    }

    #[test]
    fn roundtrip_jsonl_and_skip_bad_lines() {
        let dir = std::env::temp_dir().join(format!("rt-test-{}", std::process::id()));
        let path = dir.join("history.jsonl");
        let _ = fs::remove_file(&path);
        append(&path, &sample(false)).unwrap();
        fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"{roto\n").unwrap();
        append(&path, &sample(true)).unwrap();
        let (v, bad) = load(&path);
        assert_eq!(v.len(), 2);
        assert_eq!(bad, 1);
        assert_eq!(v[0], sample(false));
        assert!(v[1].incomplete);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn csv_rows() {
        let mut out = Vec::new();
        let n = write_csv(&[sample(false)], &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(n, 1);
        assert_eq!(lines[0], CSV_HEADER);
        assert_eq!(
            lines[1],
            "2026-11-01T10:00:00Z,decision,false,false,adaptive,0,2,4,true,6,3,12.35,401.3,hit,mouse,false,,,,,,,,,,,,false"
        );
        assert_eq!(lines[1].split(',').count(), CSV_HEADER.split(',').count());

        let mut rec = sample(false);
        rec.trials[0].start = Some([10.0, 10.0]);
        rec.trials[0].target_pos = Some([30.0, 10.0]);
        rec.trials[0].click = Some([31.0, 10.0]);
        let mut out = Vec::new();
        write_csv(&[rec], &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        let row = text.lines().nth(1).unwrap();
        assert!(row.ends_with("10.00,10.00,30.00,10.00,31.00,10.00,derecha,1.00,0.00,,,false"), "{row}");
        assert_eq!(row.split(',').count(), CSV_HEADER.split(',').count());
    }
}
