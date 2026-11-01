mod app;
mod config;
mod difficulty;
mod history;
mod input;
mod modes;
mod render;
mod stats;
mod term;
mod theme;

use config::Command;
use std::process::ExitCode;

fn main() -> ExitCode {
    let (mut cfg, warning) = config::load();
    if let Some(w) = warning {
        eprintln!("{w}");
    }
    let cli = match config::parse_args(std::env::args().skip(1), &mut cfg) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    match cli.command {
        Command::Help => println!("{}", config::USAGE),
        Command::Version => println!("rt {}", env!("CARGO_PKG_VERSION")),
        Command::Export { out } => {
            if let Err(e) = export(out) {
                eprintln!("error: {e}");
                return ExitCode::FAILURE;
            }
        }
        Command::Run => {
            let start = if cli.baseline {
                Some(true)
            } else if cli.quick {
                Some(false)
            } else {
                None
            };
            if let Err(e) = run_tui(cfg, start) {
                eprintln!("error: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}

fn run_tui(cfg: config::Config, start: Option<bool>) -> std::io::Result<()> {
    let mut term = term::init()?;
    let _guard = term::Guard;
    let size = term.size()?;
    let mut app = app::App::new(cfg, ratatui::layout::Rect::new(0, 0, size.width, size.height));
    app::run(&mut term, &mut app, start)
}

fn export(out: Option<std::path::PathBuf>) -> std::io::Result<()> {
    let path = history::history_path();
    let (sessions, bad) = history::load(&path);
    if bad > 0 {
        eprintln!("aviso: {bad} líneas dañadas ignoradas en {}", path.display());
    }
    let n = match &out {
        Some(p) => history::write_csv(&sessions, std::io::BufWriter::new(std::fs::File::create(p)?))?,
        None => history::write_csv(&sessions, std::io::stdout().lock())?,
    };
    if let Some(p) = out {
        eprintln!("{n} intentos de {} sesiones en {}", sessions.len(), p.display());
    }
    Ok(())
}
