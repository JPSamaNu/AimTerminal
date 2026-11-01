//! Dónde viven el historial y la configuración, para poder llevarlos a otra computadora.
//!
//! Orden de prioridad:
//! 1. `--data CARPETA` en la línea de comandos.
//! 2. La variable de entorno `RT_DATA`.
//! 3. La carpeta guardada con `rt data set CARPETA` (archivo `location.txt` en la carpeta
//!    por defecto).
//! 4. Una carpeta `rt-data` junto al ejecutable (modo portátil).
//! 5. `%APPDATA%\term-aim` (o `~/.config/term-aim` fuera de Windows).

use std::collections::HashSet;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Flag,
    Env,
    Saved,
    Portable,
    Default,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::Flag => "--data",
            Source::Env => "variable RT_DATA",
            Source::Saved => "guardada con rt data set",
            Source::Portable => "rt-data junto al ejecutable",
            Source::Default => "por defecto",
        }
    }
}

/// Usa esta carpeta en esta ejecución (`--data`). Solo la primera llamada cuenta.
pub fn set_override(dir: PathBuf) {
    let _ = OVERRIDE.set(dir);
}

/// Carpeta por defecto; también guarda `location.txt`.
pub fn default_dir() -> PathBuf {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return PathBuf::from(appdata).join("term-aim");
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    home.join(".config").join("term-aim")
}

fn pointer_path() -> PathBuf {
    default_dir().join("location.txt")
}

/// Elige la carpeta según la prioridad, con las fuentes ya leídas (para poder probarla).
pub fn choose(
    flag: Option<&Path>,
    env: Option<&str>,
    saved: Option<&str>,
    portable: Option<&Path>,
    default: &Path,
) -> (PathBuf, Source) {
    if let Some(p) = flag {
        return (p.to_path_buf(), Source::Flag);
    }
    if let Some(e) = env.map(str::trim).filter(|e| !e.is_empty()) {
        return (PathBuf::from(e), Source::Env);
    }
    if let Some(s) = saved.map(str::trim).filter(|s| !s.is_empty()) {
        return (PathBuf::from(s), Source::Saved);
    }
    if let Some(p) = portable {
        return (p.to_path_buf(), Source::Portable);
    }
    (default.to_path_buf(), Source::Default)
}

pub fn resolve() -> (PathBuf, Source) {
    let env = std::env::var("RT_DATA").ok();
    let saved = fs::read_to_string(pointer_path()).ok();
    let portable = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join("rt-data")))
        .filter(|d| d.is_dir());
    choose(OVERRIDE.get().map(PathBuf::as_path), env.as_deref(), saved.as_deref(), portable.as_deref(), &default_dir())
}

pub fn data_dir() -> PathBuf {
    resolve().0
}

/// Clave de una sesión para no duplicarla al fusionar: su fecha y su modo.
fn key(line: &str) -> Option<(String, String)> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    Some((v.get("date")?.as_str()?.to_string(), v.get("mode")?.as_str()?.to_string()))
}

fn read_lines(path: &Path) -> io::Result<Vec<String>> {
    match fs::File::open(path) {
        Ok(f) => BufReader::new(f).lines().filter(|l| l.as_ref().map_or(true, |l| !l.trim().is_empty())).collect(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

/// Junta dos historiales JSON Lines en `dest`, sin repetir sesiones y ordenados por fecha.
/// Las líneas se copian tal cual (no se pierde ningún campo). Devuelve (añadidas, total).
pub fn merge_history(src: &Path, dest: &Path) -> io::Result<(usize, usize)> {
    let mut lines = read_lines(dest)?;
    let mut seen: HashSet<(String, String)> = lines.iter().filter_map(|l| key(l)).collect();
    let mut added = 0;
    for l in read_lines(src)? {
        match key(&l) {
            Some(k) if seen.contains(&k) => {}
            Some(k) => {
                seen.insert(k);
                lines.push(l);
                added += 1;
            }
            // Una línea dañada se conserva al final: mejor no perder nada.
            None if !lines.contains(&l) => {
                lines.push(l);
                added += 1;
            }
            None => {}
        }
    }
    lines.sort_by_key(|l| key(l).map(|k| k.0).unwrap_or_default());
    if let Some(dir) = dest.parent() {
        fs::create_dir_all(dir)?;
    }
    // Se escribe aparte y luego se reemplaza, para no dejar el historial a medias.
    let tmp = dest.with_extension("jsonl.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        for l in &lines {
            writeln!(f, "{l}")?;
        }
        f.flush()?;
    }
    fs::rename(&tmp, dest)?;
    Ok((added, lines.len()))
}

pub struct Moved {
    pub from: PathBuf,
    pub to: PathBuf,
    pub added: usize,
    pub total: usize,
    pub config_copied: bool,
}

/// Pasa a usar `dir`: fusiona el historial actual en el de `dir`, copia la configuración
/// si allá no hay una y guarda la carpeta para las próximas veces. No borra nada.
pub fn set_location(dir: &Path) -> io::Result<Moved> {
    let to = std::path::absolute(dir)?;
    let from = data_dir();
    fs::create_dir_all(&to)?;
    let (added, total) = if from == to {
        (0, read_lines(&to.join("history.jsonl"))?.len())
    } else {
        merge_history(&from.join("history.jsonl"), &to.join("history.jsonl"))?
    };
    let (src_cfg, dst_cfg) = (from.join("config.json"), to.join("config.json"));
    let config_copied = src_cfg.is_file() && !dst_cfg.exists() && fs::copy(&src_cfg, &dst_cfg).is_ok();
    fs::create_dir_all(default_dir())?;
    fs::write(pointer_path(), to.display().to_string())?;
    Ok(Moved { from, to, added, total, config_copied })
}

/// Olvida la carpeta guardada; vuelve a la de por defecto. No borra datos.
pub fn reset_location() -> io::Result<bool> {
    match fs::remove_file(pointer_path()) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_order() {
        let d = Path::new("D");
        let p = Path::new("P");
        let f = Path::new("F");
        assert_eq!(choose(Some(f), Some("E"), Some("S"), Some(p), d), (f.into(), Source::Flag));
        assert_eq!(choose(None, Some("E"), Some("S"), Some(p), d), ("E".into(), Source::Env));
        assert_eq!(choose(None, Some("  "), Some("S\n"), Some(p), d), ("S".into(), Source::Saved));
        assert_eq!(choose(None, None, None, Some(p), d), (p.into(), Source::Portable));
        assert_eq!(choose(None, None, Some(""), None, d), (d.into(), Source::Default));
    }

    #[test]
    fn merge_dedupes_sorts_and_keeps_fields() {
        let dir = std::env::temp_dir().join(format!("rt-merge-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a.jsonl"), dir.join("sub").join("b.jsonl"));
        let line = |date: &str, extra: &str| format!(r#"{{"date":"{date}","mode":"flick","x":"{extra}"}}"#);
        fs::write(&a, format!("{}\n{}\n", line("2026-11-03", "nuevo"), line("2026-11-01", "a"))).unwrap();
        fs::create_dir_all(b.parent().unwrap()).unwrap();
        fs::write(&b, format!("{}\n{}\n", line("2026-11-02", "b"), line("2026-11-01", "a"))).unwrap();
        let (added, total) = merge_history(&a, &b).unwrap();
        assert_eq!((added, total), (1, 3));
        let text = fs::read_to_string(&b).unwrap();
        let dates: Vec<&str> = text.lines().map(|l| &l[9..19]).collect();
        assert_eq!(dates, ["2026-11-01", "2026-11-02", "2026-11-03"]);
        assert!(text.contains(r#""x":"nuevo""#));
        // Fusionar otra vez no duplica nada.
        assert_eq!(merge_history(&a, &b).unwrap(), (0, 3));
        // Origen inexistente: no pasa nada.
        assert_eq!(merge_history(&dir.join("no.jsonl"), &b).unwrap(), (0, 3));
        let _ = fs::remove_dir_all(&dir);
    }
}
