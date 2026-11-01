//! Preparación y restauración de la terminal.

use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{DisableFocusChange, DisableMouseCapture, EnableFocusChange, EnableMouseCapture};
use ratatui::crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::crossterm::{cursor, execute};
use std::io::{self, Stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static PIXEL_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Entra a la pantalla alterna en modo raw, con mouse (incluido movimiento) y eventos de foco.
pub fn init() -> io::Result<Tui> {
    install_panic_hook();
    terminal::enable_raw_mode()?;
    ACTIVE.store(true, Ordering::SeqCst);
    let mut out = io::stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture, cursor::Hide)?;
    // No todas las terminales lo soportan; si falla, simplemente no hay eventos de foco.
    let _ = execute!(out, EnableFocusChange);
    let mut t = Terminal::new(CrosstermBackend::new(out))?;
    t.clear()?;
    Ok(t)
}

/// Pide coordenadas de mouse en píxeles (SGR-Pixels, modo 1016). Si la terminal no lo
/// soporta, ignora la secuencia y todo sigue en celdas.
pub fn request_pixel_mouse() {
    let mut out = io::stdout();
    if out.write_all(b"\x1b[?1016h").and_then(|_| out.flush()).is_ok() {
        PIXEL_REQUESTED.store(true, Ordering::SeqCst);
    }
}

pub fn disable_pixel_mouse() {
    if PIXEL_REQUESTED.swap(false, Ordering::SeqCst) {
        let mut out = io::stdout();
        let _ = out.write_all(b"\x1b[?1016l").and_then(|_| out.flush());
    }
}

/// Deja la terminal como estaba. Es seguro llamarla más de una vez.
pub fn restore() {
    if !ACTIVE.swap(false, Ordering::SeqCst) {
        return;
    }
    disable_pixel_mouse();
    let mut out = io::stdout();
    let _ = execute!(out, DisableFocusChange, DisableMouseCapture, LeaveAlternateScreen, cursor::Show);
    let _ = terminal::disable_raw_mode();
}

fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        prev(info);
    }));
}

/// Restaura la terminal al salir del ámbito, también si hay un error con `?`.
pub struct Guard;

impl Drop for Guard {
    fn drop(&mut self) {
        restore();
    }
}
