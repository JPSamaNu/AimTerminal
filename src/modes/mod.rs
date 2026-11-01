//! Modos de juego. La aplicación maneja la sesión (cuenta regresiva, pausa, descarte,
//! guardado); cada modo maneja lo que pasa dentro de un intento.
//!
//! Para añadir un modo (Reacción pura, Seguimiento) basta con implementar
//! [`GameMode`], añadirlo a `GameKind` y crearlo en [`create`].

pub mod decision;
pub mod flick;
pub mod layout;
pub mod single;
pub mod targets;
pub mod tracking;

use crate::config::{Config, GameKind};
use crate::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Eligió el objetivo.
    Hit,
    /// Eligió un distractor.
    Error,
    /// Se acabó el tiempo con objetivo presente.
    Omission,
    /// Respondió en un intento sin objetivo.
    FalseAlarm,
    /// No respondió en un intento sin objetivo.
    CorrectRejection,
    /// Respondió antes del estímulo o más rápido que el umbral de anticipación.
    Anticipation,
    /// Clic en un lugar vacío (modo Blancos).
    Miss,
}

impl Outcome {
    pub fn label(self) -> &'static str {
        match self {
            Outcome::Hit => "acierto",
            Outcome::Error => "error",
            Outcome::Omission => "omisión",
            Outcome::FalseAlarm => "falsa alarma",
            Outcome::CorrectRejection => "rechazo correcto",
            Outcome::Anticipation => "anticipación",
            Outcome::Miss => "fallo",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    Mouse,
    Keyboard,
    None,
}

impl Method {
    pub fn label(self) -> &'static str {
        match self {
            Method::Mouse => "mouse",
            Method::Keyboard => "keyboard",
            Method::None => "none",
        }
    }
}

/// Un intento registrado. Los campos son generales para que sirvan a otros modos.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrialRecord {
    pub index: usize,
    /// Nivel de dificultad (None en dificultad fija).
    pub level: Option<usize>,
    /// Cantidad de recuadros en pantalla.
    pub n_boxes: usize,
    pub target_present: bool,
    pub target_w: u16,
    pub target_h: u16,
    /// Distancia del cursor al centro del objetivo al aparecer el estímulo, en anchos de
    /// celda (la vertical se multiplica por 2 para compensar la forma de la celda).
    pub cursor_dist: Option<f32>,
    pub rt_ms: Option<f64>,
    pub outcome: Outcome,
    pub method: Method,
    pub anticipation: bool,
    /// Cursor al aparecer el estímulo, en celdas (x, y).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<[f32; 2]>,
    /// Centro del objetivo; en un clic fallido, el del objetivo más cercano.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_pos: Option<[f32; 2]>,
    /// Dónde se hizo clic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub click: Option<[f32; 2]>,
    /// Métrica propia del modo entre 0 y 1 (Seguimiento: fracción del tiempo encima).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
    /// Blancos: distancia del objetivo al centro del área, de 0 (centro) a 1 (borde).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f32>,
    /// Intento de calentamiento: se guarda, pero no entra a las estadísticas.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub warmup: bool,
}

/// Punto como arreglo para guardarlo en JSON de forma compacta.
pub fn pt(p: (f32, f32)) -> [f32; 2] {
    [p.0, p.1]
}

/// Parámetros de dificultad de un intento.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    pub box_w: u16,
    pub box_h: u16,
    pub min_boxes: usize,
    pub max_boxes: usize,
    pub timeout: Duration,
    pub level: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Response {
    /// Tecla numérica, 1 = primer recuadro.
    Key(usize),
    /// Clic en coordenadas de celda.
    Click { x: f32, y: f32 },
    /// Barra espaciadora (modos de respuesta simple).
    Press,
}

/// Efecto visual que deja una respuesta; la aplicación decide si mostrarlo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlashKind {
    /// Acierto: anillo que se expande desde el recuadro.
    Hit,
    /// Eligió un distractor o respondió sin objetivo.
    Wrong,
    /// Dónde estaba el objetivo que no se eligió.
    Reveal,
    /// Clic en el vacío.
    Miss,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flash {
    pub kind: FlashKind,
    pub rect: Rect,
}

#[derive(Debug, PartialEq)]
pub enum Step {
    Idle,
    Redraw,
    Done(TrialRecord),
}

pub trait GameMode {
    fn id(&self) -> &'static str;
    /// Empieza un intento dentro de `area`; `cursor` es la última posición conocida.
    fn start_trial(&mut self, area: Rect, params: &Params, now: Instant, cursor: Option<(f32, f32)>);
    /// Próximo momento en que el modo necesita `tick`.
    fn deadline(&self) -> Option<Instant>;
    fn tick(&mut self, now: Instant) -> Step;
    /// Llamado justo después de dibujar y vaciar el cuadro; marca el inicio del estímulo.
    fn drawn(&mut self, now: Instant, cursor: Option<(f32, f32)>);
    fn respond(&mut self, r: Response, at: Instant) -> Step;
    /// Cancela el intento en curso sin registrarlo.
    fn abort(&mut self);
    /// Hay un intento en curso (fijación o estímulo).
    fn in_trial(&self) -> bool;
    fn render(&self, buf: &mut Buffer, area: Rect, theme: &Theme);
    /// Efectos del último intento terminado (se vacían al leerlos).
    fn take_flashes(&mut self) -> Vec<Flash> {
        Vec::new()
    }
    /// Movimiento del mouse (para los modos que siguen el cursor).
    fn on_move(&mut self, _pos: (f32, f32), _at: Instant) {}
    /// Si este intento cuenta para terminar una sesión por cantidad.
    fn counts(&self, _rec: &TrialRecord) -> bool {
        true
    }
}

pub fn create(cfg: &Config, rng: fastrand::Rng) -> Box<dyn GameMode> {
    match cfg.mode {
        GameKind::Decision => Box::new(decision::Decision::new(cfg, rng)),
        GameKind::Targets => Box::new(targets::Targets::new(cfg, rng)),
        GameKind::Flick => Box::new(flick::Flick::new(cfg, rng)),
        GameKind::Tracking => Box::new(tracking::Tracking::new(cfg, rng)),
        GameKind::Reaction => Box::new(single::Single::new(cfg, rng, 0.0)),
        GameKind::GoNoGo => Box::new(single::Single::new(cfg, rng, cfg.nogo_ratio)),
    }
}
