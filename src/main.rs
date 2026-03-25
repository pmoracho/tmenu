/// Lector de menus interactivos TUI en Rust utilizando Ratatui y Clap.
mod app;
mod error;
mod model;
mod parser;
mod search;
mod ui;

use app::App;
use error::AppError;

use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    author = "Patricio Moracho",
    version = "1.0",
    about = "Lector de menus interactivos TUI",
    long_about = None
)]
struct Args {
    /// Ruta al archivo de menu (.toon)
    #[arg(value_name = "ARCHIVO", default_value = "tmenu.toon")]
    menu_file: PathBuf,

    /// Activa el modo depuracion
    #[arg(short, long)]
    debug: bool,
}

fn main() -> Result<(), AppError> {
    // registrar un hook de pánico:
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(info);
    }));    
    let args = Args::parse();

    let mut app = App::from_toon(&args.menu_file, args.debug)
        .map_err(|e| match e {
            AppError::IoError(ref io) if io.kind() == io::ErrorKind::NotFound =>
                AppError::MenuFileNotFound(args.menu_file.clone()),
            other => other,
        })?;

    enable_raw_mode().map_err(|e| AppError::TerminalError(e.to_string()))?;
    let mut stdout = io::stdout();
    execute!(stdout, crossterm::cursor::SetCursorStyle::SteadyUnderScore)
        .map_err(|e| AppError::TerminalError(e.to_string()))?;
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .map_err(|e| AppError::TerminalError(e.to_string()))?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal =
        Terminal::new(backend).map_err(|e| AppError::TerminalError(e.to_string()))?;

    let result = run_app(&mut terminal, &mut app);

    let _ = disable_raw_mode();
    let _ = execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    );
    let _ = terminal.show_cursor();

    result
}

/// Ciclo principal de eventos: dibuja la UI y procesa teclado.
fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<(), AppError> {
    loop {
        terminal
            .draw(|f| ui::ui(f, app))
            .map_err(|e| AppError::TerminalError(e.to_string()))?;

        // Un solo event::read() por iteracion — el KeyCode se pasa a los handlers
        let event = event::read().map_err(|e| AppError::EventError(e.to_string()))?;

        if let Event::Key(key) = event {
            if key.kind != event::KeyEventKind::Press {
                continue;
            }
            if key.code == KeyCode::F(1) {
                app.show_help = true;
                let quit = run_help_modal(terminal, app)?;  // ← ahora retorna bool
                if quit {
                    return Ok(());
                }
                continue;
            }
            // Ctrl+Q sale desde cualquier modo
            if key.modifiers.contains(event::KeyModifiers::CONTROL)
                && key.code == KeyCode::Char('q')
            {
                return Ok(());
            }

            let should_quit = if app.search_mode {
                // FIX: se pasa key.code; no se hace un segundo event::read()
                handle_search_mode(terminal, app, key.code)?
            } else {
                handle_navigation_mode(terminal, app, key.code)?
            };

            if should_quit {
                return Ok(());
            }
        }
    }
}

// run_help_modal retorna true si el usuario pidió salir
fn run_help_modal(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<bool, AppError> {
    loop {
        terminal
            .draw(|f| ui::ui(f, app))
            .map_err(|e| AppError::TerminalError(e.to_string()))?;

        if let Event::Key(key) = event::read()
            .map_err(|e| AppError::EventError(e.to_string()))?
        {
            if key.kind != event::KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Esc | KeyCode::F(1) | KeyCode::F(2) => {
                    app.show_help = false;
                    return Ok(false); // cerrar ayuda, continuar app
                }
                KeyCode::Char('q')
                    if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                {
                    return Ok(true); // salir de la app
                }
                _ => {}
            }
        }
    }
}

/// Maneja teclas en modo busqueda.
/// Recibe el KeyCode ya leido por el loop — sin segundo event::read().
fn handle_search_mode(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    key: KeyCode,
) -> Result<bool, AppError> {
    match key {
        KeyCode::Tab => {
            app.search_mode = false;
            app.search_text.clear();
        }
        KeyCode::Backspace => {
            app.search_text.pop();
        }
        KeyCode::F(2) => app.show_preview = !app.show_preview,
        KeyCode::Enter => {
            let filtered = app.filtered_items();
            if !filtered.is_empty() {
                app.state.select(Some(0));
                if app.activate_item(terminal, &filtered)? {
                    return Ok(true);
                }
            }
        }
        KeyCode::Char(c) => {
            app.search_text.push(c);
        }
        _ => {}
    }
    Ok(false)
}

/// Maneja teclas en modo navegacion normal.
fn handle_navigation_mode(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    key: KeyCode,
) -> Result<bool, AppError> {
    match key {
        KeyCode::Tab => app.search_mode = true,
        KeyCode::Down => app.next(),
        KeyCode::Up => app.previous(),
        KeyCode::Home => app.go_home(),
        KeyCode::F(2) => app.show_preview = !app.show_preview,
        KeyCode::Enter | KeyCode::Right => {
            let items = app.filtered_items();
            if app.activate_item(terminal, &items)? {
                return Ok(true);
            }
        }
        KeyCode::Left | KeyCode::Esc => app.back(),
        _ => {}
    }
    Ok(false)
}