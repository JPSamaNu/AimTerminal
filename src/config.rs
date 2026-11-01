//! Configuración: valores por defecto, archivo opcional y argumentos de línea de comandos.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeName {
    Dark,
    Mono,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fill {
    Filled,
    Hollow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Fixed,
    Adaptive,
}

impl Difficulty {
    pub fn label(self) -> &'static str {
        match self {
            Difficulty::Fixed => "fija",
            Difficulty::Adaptive => "adaptativa",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Intentos válidos por sesión.
    pub trials: usize,
    /// Tiempo límite por intento (modo fijo).
    pub timeout_ms: u64,
    /// Respuestas más rápidas que esto se marcan como anticipación.
    pub anticipation_ms: u64,
    /// Proporción de intentos sin objetivo (catch trials).
    pub catch_ratio: f64,
    pub fixation_min_ms: u64,
    pub fixation_max_ms: u64,
    pub min_boxes: usize,
    pub max_boxes: usize,
    /// Tamaño de los recuadros en celdas (modo fijo).
    pub box_w: u16,
    pub box_h: u16,
    pub difficulty: Difficulty,
    pub start_level: usize,
    pub theme: ThemeName,
    pub target_fill: Fill,
    pub distractor_fill: Fill,
    pub target_color: String,
    pub distractor_color: String,
    /// Intentar el modo de mouse en píxeles (SGR-Pixels, 1016).
    pub pixel_mouse: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            trials: 20,
            timeout_ms: 2000,
            anticipation_ms: 120,
            catch_ratio: 0.17,
            fixation_min_ms: 500,
            fixation_max_ms: 2000,
            min_boxes: 3,
            max_boxes: 5,
            box_w: 6,
            box_h: 3,
            difficulty: Difficulty::Adaptive,
            start_level: 0,
            theme: ThemeName::Dark,
            target_fill: Fill::Filled,
            distractor_fill: Fill::Hollow,
            target_color: "green".into(),
            distractor_color: "red".into(),
            pixel_mouse: false,
        }
    }
}

impl Config {
    /// Configuración de la prueba base: siempre la misma, sin adaptación.
    /// Solo conserva la apariencia del usuario (tema, colores, relleno).
    pub fn baseline(&self) -> Config {
        let d = Config::default();
        Config {
            trials: 30,
            difficulty: Difficulty::Fixed,
            theme: self.theme,
            target_fill: self.target_fill,
            distractor_fill: self.distractor_fill,
            target_color: self.target_color.clone(),
            distractor_color: self.distractor_color.clone(),
            pixel_mouse: self.pixel_mouse,
            ..d
        }
    }

    /// Corrige valores fuera de rango que podrían venir del archivo.
    pub fn sanitize(&mut self) {
        self.trials = self.trials.clamp(1, 500);
        self.timeout_ms = self.timeout_ms.clamp(300, 10_000);
        self.catch_ratio = self.catch_ratio.clamp(0.0, 0.5);
        self.min_boxes = self.min_boxes.clamp(2, 9);
        self.max_boxes = self.max_boxes.clamp(self.min_boxes, 9);
        self.fixation_max_ms = self.fixation_max_ms.max(self.fixation_min_ms);
        self.box_w = self.box_w.clamp(3, 20);
        self.box_h = self.box_h.clamp(2, 10);
    }
}

/// Carpeta de datos: %APPDATA%\term-aim (o ~/.config/term-aim fuera de Windows).
pub fn data_dir() -> PathBuf {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return PathBuf::from(appdata).join("term-aim");
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    home.join(".config").join("term-aim")
}

pub fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

/// Lee el archivo de configuración si existe. Devuelve también un aviso si no se pudo leer.
pub fn load() -> (Config, Option<String>) {
    let path = config_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return (Config::default(), None);
    };
    match serde_json::from_str::<Config>(&text) {
        Ok(mut c) => {
            c.sanitize();
            (c, None)
        }
        Err(e) => (
            Config::default(),
            Some(format!("aviso: {} no es válido ({e}); se usan valores por defecto", path.display())),
        ),
    }
}

#[derive(Debug, Default, PartialEq)]
pub enum Command {
    #[default]
    Run,
    Export { out: Option<PathBuf> },
    Help,
    Version,
}

#[derive(Debug, Default)]
pub struct Cli {
    pub command: Command,
    pub quick: bool,
    pub baseline: bool,
}

pub const USAGE: &str = "\
rt - práctica de decisión y reflejos

uso:
  rt [opciones]          abre el menú
  rt --quick             empieza una sesión de inmediato
  rt --baseline          prueba base fija (30 intentos)
  rt export [-o ARCHIVO] exporta los intentos a CSV (stdout por defecto)

opciones:
  --theme dark|mono      paleta
  --trials N             intentos por sesión
  --timeout MS           tiempo límite por intento (modo fijo)
  --fixed | --adaptive   dificultad
  --pixel                intentar mouse en píxeles (SGR 1016)
  -h, --help             esta ayuda
  -V, --version          versión

teclas: 1-9 o clic para responder, Esc pausa, q sale";

/// Aplica los argumentos sobre la configuración.
pub fn parse_args<I: IntoIterator<Item = String>>(args: I, cfg: &mut Config) -> Result<Cli, String> {
    let mut cli = Cli::default();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "export" => cli.command = Command::Export { out: None },
            "-o" | "--output" => {
                let p = it.next().ok_or("falta el archivo después de -o")?;
                cli.command = Command::Export { out: Some(PathBuf::from(p)) };
            }
            "--quick" | "-q" => cli.quick = true,
            "--baseline" => cli.baseline = true,
            "--theme" => {
                cfg.theme = match it.next().as_deref() {
                    Some("mono") => ThemeName::Mono,
                    Some("dark") => ThemeName::Dark,
                    other => return Err(format!("tema desconocido: {}", other.unwrap_or(""))),
                }
            }
            "--trials" => {
                cfg.trials = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or("--trials necesita un número")?
            }
            "--timeout" => {
                cfg.timeout_ms = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or("--timeout necesita milisegundos")?
            }
            "--fixed" => cfg.difficulty = Difficulty::Fixed,
            "--adaptive" => cfg.difficulty = Difficulty::Adaptive,
            "--pixel" => cfg.pixel_mouse = true,
            "-h" | "--help" => cli.command = Command::Help,
            "-V" | "--version" => cli.command = Command::Version,
            other => return Err(format!("argumento desconocido: {other}\n\n{USAGE}")),
        }
    }
    cfg.sanitize();
    Ok(cli)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_flags() {
        let mut c = Config::default();
        let cli = parse_args(args(&["--quick", "--theme", "mono", "--trials", "10", "--fixed"]), &mut c).unwrap();
        assert!(cli.quick);
        assert_eq!(c.theme, ThemeName::Mono);
        assert_eq!(c.trials, 10);
        assert_eq!(c.difficulty, Difficulty::Fixed);
    }

    #[test]
    fn parses_export() {
        let mut c = Config::default();
        let cli = parse_args(args(&["export", "-o", "x.csv"]), &mut c).unwrap();
        assert_eq!(cli.command, Command::Export { out: Some("x.csv".into()) });
    }

    #[test]
    fn baseline_is_constant() {
        let mut c = Config { trials: 7, timeout_ms: 900, box_w: 12, ..Config::default() };
        c.theme = ThemeName::Mono;
        let b = c.baseline();
        assert_eq!(b.trials, 30);
        assert_eq!(b.timeout_ms, 2000);
        assert_eq!(b.box_w, Config::default().box_w);
        assert_eq!(b.difficulty, Difficulty::Fixed);
        assert_eq!(b.theme, ThemeName::Mono);
    }

    #[test]
    fn partial_file_uses_defaults() {
        let c: Config = serde_json::from_str(r#"{"trials": 12, "theme": "mono"}"#).unwrap();
        assert_eq!(c.trials, 12);
        assert_eq!(c.timeout_ms, 2000);
    }
}
