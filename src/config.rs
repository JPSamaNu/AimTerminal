//! Configuración: valores por defecto, archivo opcional y argumentos de línea de comandos.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeName {
    /// Verde fósforo con aspecto de salida de terminal.
    Matrix,
    /// Cian y magenta con aspecto de salida de terminal.
    Cyber,
    Dark,
    Mono,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fill {
    Filled,
    Hollow,
}

/// Fondo de las pantallas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Background {
    /// Degradado oscuro con una rejilla de puntos tenue.
    Grid,
    /// Solo el degradado.
    Plain,
    /// El fondo de la terminal, sin pintar nada.
    None,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GameKind {
    Decision,
    Targets,
    Flick,
    Tracking,
    Reaction,
    GoNoGo,
}

impl GameKind {
    pub const ALL: [GameKind; 6] = [
        GameKind::Decision,
        GameKind::Targets,
        GameKind::Flick,
        GameKind::Tracking,
        GameKind::Reaction,
        GameKind::GoNoGo,
    ];

    pub fn label(self) -> &'static str {
        match self {
            GameKind::Decision => "decisión",
            GameKind::Targets => "blancos",
            GameKind::Flick => "flick",
            GameKind::Tracking => "seguimiento",
            GameKind::Reaction => "reacción",
            GameKind::GoNoGo => "go/no-go",
        }
    }

    /// Identificador que se guarda en el historial.
    pub fn id(self) -> &'static str {
        match self {
            GameKind::Decision => "decision",
            GameKind::Targets => "targets",
            GameKind::Flick => "flick",
            GameKind::Tracking => "tracking",
            GameKind::Reaction => "reaction",
            GameKind::GoNoGo => "gonogo",
        }
    }

    pub fn next(self) -> GameKind {
        let i = GameKind::ALL.iter().position(|k| *k == self).unwrap_or(0);
        GameKind::ALL[(i + 1) % GameKind::ALL.len()]
    }

    pub fn prev(self) -> GameKind {
        let i = GameKind::ALL.iter().position(|k| *k == self).unwrap_or(0);
        GameKind::ALL[(i + GameKind::ALL.len() - 1) % GameKind::ALL.len()]
    }

    /// Modos con dificultad (fija o adaptativa).
    pub fn adaptive(self) -> bool {
        !matches!(self, GameKind::Reaction)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Modo de juego por defecto.
    pub mode: GameKind,
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
    pub background: Background,
    /// Efectos breves al acertar o fallar.
    pub effects: bool,
    pub target_fill: Fill,
    pub distractor_fill: Fill,
    pub target_color: String,
    pub distractor_color: String,
    /// Intentar el modo de mouse en píxeles (SGR-Pixels, 1016).
    pub pixel_mouse: bool,
    /// Modo Blancos: cuántos verdes a la vez, duración y tamaño.
    pub targets_count: usize,
    pub targets_secs: u64,
    pub targets_w: u16,
    pub targets_h: u16,
    /// Dificultad de Blancos, aparte de la de Decisión.
    pub targets_difficulty: Difficulty,
    /// Blancos con dificultad fija: nivel de 0 a 9; más alto, más lejos del centro.
    pub targets_level: usize,
    /// Blancos adaptativo: un acierto cuenta para subir de nivel si tarda como mucho esto.
    pub targets_adapt_ms: u64,
    /// Modo Flick: blancos por sesión y tiempo límite de cada uno.
    pub flick_trials: usize,
    pub flick_timeout_ms: u64,
    /// Dificultad de Flick: el nivel (0 a 9) achica el blanco y lo aleja del cursor.
    pub flick_difficulty: Difficulty,
    pub flick_level: usize,
    /// Flick adaptativo: un acierto cuenta para subir de nivel si tarda como mucho esto.
    pub flick_adapt_ms: u64,
    /// Modo Seguimiento: duración y dificultad. El nivel (0 a 9) sube la velocidad,
    /// achica el blanco y hace que cambie de rumbo más seguido.
    pub tracking_secs: u64,
    pub tracking_difficulty: Difficulty,
    pub tracking_level: usize,
    /// Reacción pura y Go/No-Go: intentos y tiempo para responder.
    pub reaction_trials: usize,
    pub gonogo_trials: usize,
    /// Proporción de intentos "no-go" (no responder).
    pub nogo_ratio: f64,
    /// Reacción pura: tiempo para responder.
    pub go_timeout_ms: u64,
    /// Go/No-Go: el nivel (0 a 9) acorta la ventana para responder.
    pub gonogo_difficulty: Difficulty,
    pub gonogo_level: usize,
    /// El texto de fondo del estilo de código también avanza mientras juegas.
    pub ambient_motion: bool,
    /// Intentos de calentamiento al empezar cada sesión: no cuentan para las estadísticas
    /// ni para terminar. En los modos por tiempo el reloj arranca al acabar.
    pub warmup_trials: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            mode: GameKind::Decision,
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
            theme: ThemeName::Matrix,
            background: Background::Grid,
            effects: true,
            target_fill: Fill::Filled,
            distractor_fill: Fill::Hollow,
            target_color: "green".into(),
            distractor_color: "red".into(),
            pixel_mouse: false,
            targets_count: 3,
            targets_secs: 30,
            targets_w: 6,
            targets_h: 3,
            targets_difficulty: Difficulty::Adaptive,
            targets_level: 3,
            targets_adapt_ms: 900,
            flick_trials: 30,
            flick_timeout_ms: 3000,
            flick_difficulty: Difficulty::Adaptive,
            flick_level: 3,
            flick_adapt_ms: 850,
            tracking_secs: 30,
            tracking_difficulty: Difficulty::Adaptive,
            tracking_level: 3,
            reaction_trials: 20,
            gonogo_trials: 30,
            nogo_ratio: 0.25,
            go_timeout_ms: 1000,
            gonogo_difficulty: Difficulty::Adaptive,
            gonogo_level: 3,
            ambient_motion: true,
            warmup_trials: 3,
        }
    }
}

impl Config {
    /// Configuración de la prueba base: siempre la misma, sin adaptación.
    /// Solo conserva la apariencia del usuario (tema, colores, relleno).
    ///
    /// Hay dos: la de Decisión (30 intentos, dificultad fija) y la de Flick (30 blancos en
    /// nivel 4 fijo). Con el modo en Flick se usa la de Flick; en los demás, la de Decisión.
    pub fn baseline(&self) -> Config {
        let d = Config::default();
        let mode = self.baseline_mode();
        Config {
            mode,
            trials: 30,
            difficulty: Difficulty::Fixed,
            flick_trials: 30,
            flick_difficulty: Difficulty::Fixed,
            flick_level: 4,
            theme: self.theme,
            background: self.background,
            effects: self.effects,
            target_fill: self.target_fill,
            distractor_fill: self.distractor_fill,
            target_color: self.target_color.clone(),
            distractor_color: self.distractor_color.clone(),
            pixel_mouse: self.pixel_mouse,
            ..d
        }
    }

    /// Modo de la prueba base que corresponde al modo elegido.
    pub fn baseline_mode(&self) -> GameKind {
        match self.mode {
            GameKind::Flick => GameKind::Flick,
            _ => GameKind::Decision,
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
        self.box_h = self.box_h.clamp(3, 10);
        self.targets_count = self.targets_count.clamp(1, 9);
        self.targets_secs = self.targets_secs.clamp(5, 600);
        self.targets_w = self.targets_w.clamp(3, 20);
        self.targets_h = self.targets_h.clamp(3, 10);
        self.targets_level = self.targets_level.min(9);
        self.targets_adapt_ms = self.targets_adapt_ms.clamp(200, 5000);
        self.flick_trials = self.flick_trials.clamp(1, 500);
        self.flick_level = self.flick_level.min(9);
        self.flick_adapt_ms = self.flick_adapt_ms.clamp(200, 5000);
        self.flick_timeout_ms = self.flick_timeout_ms.clamp(500, 20_000);
        self.tracking_secs = self.tracking_secs.clamp(5, 600);
        self.tracking_level = self.tracking_level.min(9);
        self.reaction_trials = self.reaction_trials.clamp(1, 500);
        self.gonogo_trials = self.gonogo_trials.clamp(1, 500);
        self.nogo_ratio = self.nogo_ratio.clamp(0.0, 0.6);
        self.go_timeout_ms = self.go_timeout_ms.clamp(300, 10_000);
        self.gonogo_level = self.gonogo_level.min(9);
        self.warmup_trials = self.warmup_trials.min(20);
    }

    /// Dificultad del modo elegido (Blancos tiene la suya).
    pub fn mode_difficulty(&self) -> Difficulty {
        self.levels().map(|(d, _)| d).unwrap_or(self.difficulty)
    }

    /// Dificultad y nivel fijo de los modos con niveles de 0 a 9 (todos menos Decisión,
    /// que tiene su propia escalera, y Reacción, que no tiene dificultad).
    pub fn levels(&self) -> Option<(Difficulty, usize)> {
        match self.mode {
            GameKind::Targets => Some((self.targets_difficulty, self.targets_level)),
            GameKind::Flick => Some((self.flick_difficulty, self.flick_level)),
            GameKind::Tracking => Some((self.tracking_difficulty, self.tracking_level)),
            GameKind::GoNoGo => Some((self.gonogo_difficulty, self.gonogo_level)),
            _ => None,
        }
    }

    fn levels_mut(&mut self) -> Option<(&mut Difficulty, &mut usize)> {
        match self.mode {
            GameKind::Targets => Some((&mut self.targets_difficulty, &mut self.targets_level)),
            GameKind::Flick => Some((&mut self.flick_difficulty, &mut self.flick_level)),
            GameKind::Tracking => Some((&mut self.tracking_difficulty, &mut self.tracking_level)),
            GameKind::GoNoGo => Some((&mut self.gonogo_difficulty, &mut self.gonogo_level)),
            _ => None,
        }
    }

    /// Nivel elegido para la dificultad fija.
    pub fn fixed_level(&self) -> Option<usize> {
        self.levels().map(|(_, l)| l)
    }

    /// Tiempo de un acierto "rápido" para la escalera de Blancos o Flick.
    pub fn adapt_ms(&self) -> Option<u64> {
        match self.mode {
            GameKind::Targets => Some(self.targets_adapt_ms),
            GameKind::Flick => Some(self.flick_adapt_ms),
            _ => None,
        }
    }

    /// Siguiente opción de dificultad del modo elegido. En Decisión alterna fija y
    /// adaptativa; en los modos con niveles recorre fácil, normal, difícil, experto y
    /// adaptativa.
    pub fn cycle_difficulty(&mut self, forward: bool) {
        if self.mode == GameKind::Decision {
            self.difficulty = match self.difficulty {
                Difficulty::Fixed => Difficulty::Adaptive,
                Difficulty::Adaptive => Difficulty::Fixed,
            };
            return;
        }
        let Some((difficulty, level)) = self.levels_mut() else { return };
        let n = TARGET_PRESETS.len() + 1;
        let current = match *difficulty {
            Difficulty::Adaptive => TARGET_PRESETS.len(),
            Difficulty::Fixed => {
                TARGET_PRESETS.iter().position(|(l, _)| *l >= *level).unwrap_or(TARGET_PRESETS.len() - 1)
            }
        };
        let next = if forward { (current + 1) % n } else { (current + n - 1) % n };
        if next == TARGET_PRESETS.len() {
            *difficulty = Difficulty::Adaptive;
        } else {
            *difficulty = Difficulty::Fixed;
            *level = TARGET_PRESETS[next].0;
        }
    }

    /// Nombre de la dificultad del modo elegido, si tiene.
    pub fn difficulty_label(&self) -> Option<String> {
        if self.mode == GameKind::Decision {
            return Some(self.difficulty.label().to_string());
        }
        self.levels().map(|(d, l)| match d {
            Difficulty::Adaptive => "adaptativa".to_string(),
            Difficulty::Fixed => targets_level_label(l),
        })
    }

    /// Intentos que cuentan para terminar una sesión (en los modos por cantidad).
    pub fn session_trials(&self) -> usize {
        match self.mode {
            GameKind::Decision => self.trials,
            GameKind::Flick => self.flick_trials,
            GameKind::Reaction => self.reaction_trials,
            GameKind::GoNoGo => self.gonogo_trials,
            GameKind::Targets | GameKind::Tracking => 0,
        }
    }

    /// Duración de la sesión en los modos por tiempo.
    pub fn session_secs(&self) -> Option<u64> {
        match self.mode {
            GameKind::Targets => Some(self.targets_secs),
            GameKind::Tracking => Some(self.tracking_secs),
            _ => None,
        }
    }
}

/// Dificultades fijas de Blancos: nivel y nombre.
pub const TARGET_PRESETS: [(usize, &str); 4] = [(1, "fácil"), (3, "normal"), (6, "difícil"), (9, "experto")];

/// Nombre de un nivel de Blancos: el del preajuste si coincide, si no "nivel N".
pub fn targets_level_label(level: usize) -> String {
    TARGET_PRESETS
        .iter()
        .find(|(l, _)| *l == level)
        .map(|(_, n)| n.to_string())
        .unwrap_or_else(|| format!("nivel {level}"))
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
  rt --baseline          prueba base de decisión (30 intentos)
  rt --mode blancos      modo Blancos: varios verdes, 30 s, solo mouse
  rt export [-o ARCHIVO] exporta los intentos a CSV (stdout por defecto)

opciones:
  --mode decision|blancos|flick|seguimiento|reaccion|gonogo  modo de juego
  --theme matrix|cyber|dark|mono  paleta (matrix y cyber imitan salida de terminal)
  --plain                sin fondo ni efectos (estilo sobrio)
  --trials N             intentos por sesión
  --timeout MS           tiempo límite por intento (modo fijo)
  --fixed | --adaptive   dificultad
  --pixel                intentar mouse en píxeles (SGR 1016)
  --warmup N             intentos de calentamiento (0 lo quita)
  -h, --help             esta ayuda
  -V, --version          versión

teclas: 1-9 o clic para responder, Esc pausa, q sale";

fn parse_mode(v: Option<&str>) -> Result<GameKind, String> {
    match v {
        Some("decision" | "decisión" | "d") => Ok(GameKind::Decision),
        Some("blancos" | "targets" | "b") => Ok(GameKind::Targets),
        Some("flick" | "f") => Ok(GameKind::Flick),
        Some("seguimiento" | "tracking" | "s") => Ok(GameKind::Tracking),
        Some("reaccion" | "reacción" | "reaction" | "r") => Ok(GameKind::Reaction),
        Some("gonogo" | "go" | "g") => Ok(GameKind::GoNoGo),
        other => Err(format!("modo desconocido: {}", other.unwrap_or(""))),
    }
}

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
            "--mode" | "-m" => cfg.mode = parse_mode(it.next().as_deref())?,
            "--theme" => {
                cfg.theme = match it.next().as_deref() {
                    Some("mono") => ThemeName::Mono,
                    Some("matrix") => ThemeName::Matrix,
                    Some("cyber") => ThemeName::Cyber,
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
            "--plain" => {
                cfg.background = Background::None;
                cfg.effects = false;
            }
            "--fixed" => cfg.difficulty = Difficulty::Fixed,
            "--adaptive" => cfg.difficulty = Difficulty::Adaptive,
            "--pixel" => cfg.pixel_mouse = true,
            "--warmup" => {
                cfg.warmup_trials = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or("--warmup necesita un número")?
            }
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
        let cli =
            parse_args(args(&["--quick", "--theme", "mono", "--trials", "10", "--fixed", "--mode", "blancos"]), &mut c)
                .unwrap();
        assert_eq!(c.mode, GameKind::Targets);
        assert!(cli.quick);
        assert_eq!(c.theme, ThemeName::Mono);
        assert_eq!(c.trials, 10);
        assert_eq!(c.difficulty, Difficulty::Fixed);
    }

    #[test]
    fn targets_difficulty_cycles_presets_then_adaptive() {
        let mut c = Config { mode: GameKind::Targets, targets_difficulty: Difficulty::Fixed, targets_level: 1, ..Config::default() };
        let mut seen = Vec::new();
        for _ in 0..5 {
            c.cycle_difficulty(true);
            seen.push(c.difficulty_label().unwrap());
        }
        assert_eq!(seen, ["normal", "difícil", "experto", "adaptativa", "fácil"]);
        c.cycle_difficulty(false);
        assert_eq!(c.difficulty_label().unwrap(), "adaptativa");
        // La dificultad de Decisión no cambia.
        assert_eq!(c.difficulty, Config::default().difficulty);

        // Flick tiene la suya, independiente de la de Blancos.
        c.mode = GameKind::Flick;
        c.flick_difficulty = Difficulty::Fixed;
        c.flick_level = 9;
        c.cycle_difficulty(true);
        assert_eq!(c.difficulty_label().unwrap(), "adaptativa");
        assert_eq!(c.targets_difficulty, Difficulty::Adaptive);
        c.cycle_difficulty(true);
        assert_eq!((c.flick_difficulty, c.flick_level), (Difficulty::Fixed, 1));
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
        assert_eq!(b.mode, GameKind::Decision);
    }

    #[test]
    fn flick_baseline_is_fixed_level() {
        let c = Config {
            mode: GameKind::Flick,
            flick_trials: 7,
            flick_difficulty: Difficulty::Adaptive,
            flick_level: 9,
            ..Config::default()
        };
        let b = c.baseline();
        assert_eq!(b.mode, GameKind::Flick);
        assert_eq!((b.flick_trials, b.flick_difficulty, b.flick_level), (30, Difficulty::Fixed, 4));
        assert_eq!(b.warmup_trials, Config::default().warmup_trials);
        // En un modo sin baseline propia se usa la de Decisión.
        let t = Config { mode: GameKind::Tracking, ..Config::default() };
        assert_eq!(t.baseline().mode, GameKind::Decision);
    }

    #[test]
    fn partial_file_uses_defaults() {
        let c: Config = serde_json::from_str(r#"{"trials": 12, "theme": "mono"}"#).unwrap();
        assert_eq!(c.trials, 12);
        assert_eq!(c.timeout_ms, 2000);
    }
}
