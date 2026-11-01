//! Lectura de eventos: filtra teclas Press, sigue el cursor del mouse y marca la hora de llegada.

use ratatui::crossterm::event::{self, Event, KeyEvent, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::crossterm::terminal;
use std::io;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Input {
    Key(KeyEvent),
    /// Clic izquierdo en coordenadas de celda (con fracción si hay píxeles).
    Click { x: f32, y: f32 },
    Move,
    Resize(u16, u16),
    FocusLost,
    FocusGained,
}

#[derive(Clone, Copy, Debug)]
pub struct Timed {
    pub input: Input,
    /// Momento en que el evento llegó a la aplicación.
    pub at: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelMode {
    Off,
    /// Se pidió 1016; se activa al ver una coordenada fuera de la cuadrícula.
    Probing,
    On,
}

/// Estado del mouse: última posición conocida y conversión píxel→celda.
#[derive(Debug)]
pub struct Mouse {
    /// Posición en unidades de celda (columna, fila); el centro de la celda (0,0) es (0.5,0.5).
    pub pos: Option<(f32, f32)>,
    pub pixel: PixelMode,
    grid: (u16, u16),
    cell_px: Option<(f32, f32)>,
}

impl Mouse {
    pub fn new(pixel_requested: bool, cols: u16, rows: u16) -> Mouse {
        let mut m = Mouse {
            pos: None,
            pixel: if pixel_requested { PixelMode::Probing } else { PixelMode::Off },
            grid: (cols, rows),
            cell_px: None,
        };
        m.set_grid(cols, rows);
        m
    }

    pub fn set_grid(&mut self, cols: u16, rows: u16) {
        self.grid = (cols, rows);
        if self.pixel == PixelMode::Off {
            return;
        }
        self.cell_px = cell_pixels(cols, rows);
        if self.cell_px.is_none() {
            // Sin tamaño en píxeles no hay forma de convertir: volvemos a celdas.
            self.pixel = PixelMode::Off;
        }
    }

    pub fn map(&mut self, col: u16, row: u16) -> (f32, f32) {
        if self.pixel == PixelMode::Probing && (col >= self.grid.0 || row >= self.grid.1) {
            self.pixel = PixelMode::On;
        }
        match (self.pixel, self.cell_px) {
            (PixelMode::On, Some((cw, ch))) => (col as f32 / cw, row as f32 / ch),
            _ => (col as f32 + 0.5, row as f32 + 0.5),
        }
    }
}

fn cell_pixels(cols: u16, rows: u16) -> Option<(f32, f32)> {
    let ws = terminal::window_size().ok()?;
    if ws.width == 0 || ws.height == 0 || cols == 0 || rows == 0 {
        return None;
    }
    Some((ws.width as f32 / cols as f32, ws.height as f32 / rows as f32))
}

/// Espera un evento hasta `timeout`. Devuelve `None` si no llegó nada útil.
pub fn read(timeout: Duration, mouse: &mut Mouse) -> io::Result<Option<Timed>> {
    if !event::poll(timeout)? {
        return Ok(None);
    }
    let ev = event::read()?;
    let at = Instant::now();
    let input = match ev {
        // En Windows llegan Press y Release: solo nos interesa Press.
        Event::Key(k) if k.kind == KeyEventKind::Press => Input::Key(k),
        Event::Mouse(m) => {
            let p = mouse.map(m.column, m.row);
            mouse.pos = Some(p);
            match m.kind {
                MouseEventKind::Down(MouseButton::Left) => Input::Click { x: p.0, y: p.1 },
                MouseEventKind::Moved | MouseEventKind::Drag(_) => Input::Move,
                _ => return Ok(None),
            }
        }
        Event::Resize(c, r) => {
            mouse.set_grid(c, r);
            Input::Resize(c, r)
        }
        Event::FocusLost => Input::FocusLost,
        Event::FocusGained => Input::FocusGained,
        _ => return Ok(None),
    };
    Ok(Some(Timed { input, at }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_coordinates_use_cell_center() {
        let mut m = Mouse::new(false, 80, 24);
        assert_eq!(m.map(3, 4), (3.5, 4.5));
    }

    #[test]
    fn pixel_probe_switches_on_out_of_grid() {
        let mut m = Mouse { pos: None, pixel: PixelMode::Probing, grid: (80, 24), cell_px: Some((10.0, 20.0)) };
        assert_eq!(m.map(5, 5), (5.5, 5.5));
        assert_eq!(m.map(400, 200), (40.0, 10.0));
        assert_eq!(m.pixel, PixelMode::On);
    }
}
