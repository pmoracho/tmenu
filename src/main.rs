rust_i18n::i18n!("locales");
use rust_i18n::t;

/// Lector de menus interactivos TUI en Rust utilizando Ratatui y Clap.
/// El programa carga un menú desde un archivo `.toon`, muestra una interfaz de texto
/// para navegar y ejecutar comandos, con soporte para búsqueda fuzzy, historial,
/// y un sistema de wizards para comandos con parámetros.
mod app;
mod error;
mod history;
mod model;
mod parser;
mod search;
mod ui;

use app::App;
use error::AppError;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
struct Args {
    menu_file: PathBuf,
    debug: bool,
    lang: Option<String>,
}

/// Detecta el locale del sistema y devuelve "es" o "en" según corresponda.
/// Si no se puede detectar, devuelve "en" por defecto.
/// Esto se usa como fallback si no se especifica --lang y LANG no es claro.
fn detect_locale() -> String {
    let locale = sys_locale::get_locale().unwrap_or("en".into());

    if locale.starts_with("es") {
        "es".into()
    } else {
        "en".into()
    }
}


/// Función principal: configura el terminal, maneja errores y ejecuta la app.
fn main() {

    // registrar un hook de pánico:
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(info);
    }));

    if let Err(err) = run() {
        eprintln!("\n❌ Error: {}", err);
        std::process::exit(1);
    }
}

/// Configura el terminal en modo raw, ejecuta el ciclo principal de eventos, y
/// asegura restaurar el estado del terminal al salir, incluso si ocurre un error.
fn run() -> Result<(), AppError> {

    // PASO 1: Parsear manualmente --lang de std::env::args si existe
    let args_vec: Vec<String> = std::env::args().collect();
    let cli_lang = args_vec
        .windows(2)
        .find(|w| w[0] == "--lang")
        .map(|w| w[1].clone());

    // PASO 2: Decidir qué locale usar
    let locale = if let Some(lang) = cli_lang {
        // Si pasó --lang, usar eso
        lang
    } else {
        // Si no, detectar locale del sistema
        std::env::var("LANG")
            .ok()
            .and_then(|l| {
                if l.starts_with("es") {
                    Some("es".to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| detect_locale())
    };

    // PASO 3: Establecer locale ANTES de crear help
    rust_i18n::set_locale(&locale);

    // PASO 4: Crear ayuda localizada dinámicamente
    let about: &'static str = Box::leak(t!("args.about").to_string().into_boxed_str());
    let author: &'static str = Box::leak(t!("args.author").to_string().into_boxed_str());
    let menu_file_help: &'static str = Box::leak(t!("args.menu_file").to_string().into_boxed_str());
    let debug_help: &'static str = Box::leak(t!("args.debug").to_string().into_boxed_str());
    let lang_help: &'static str = Box::leak(t!("args.lang").to_string().into_boxed_str());
    let file_name: &'static str = Box::leak(t!("args.file_name").to_string().into_boxed_str());
    let usage_text: &'static str = Box::leak(t!("clap.usage").to_string().into_boxed_str());

    let help_template = format!(
        "{{about}}\n{usage_text}: {{usage}}\n\n{{all-args}}",
        usage_text = usage_text
    );

    let cmd = clap::Command::new("tmenu")
        .version(env!("CARGO_PKG_VERSION"))
        .about(about)
        .author(author)
        .help_template(&help_template)
        .arg(
            clap::Arg::new("menu_file")
                .value_name(file_name)
                .default_value("tmenu.toon")
                .help(menu_file_help)
                .index(1)
        )
        .arg(
            clap::Arg::new("debug")
                .short('d')
                .long("debug")
                .help(debug_help)
                .action(clap::ArgAction::SetTrue)
        )
        .arg(
            clap::Arg::new("lang")
                .long("lang")
                .help(lang_help)
                .value_name("LANG")
        );

    // PASO 5: Parsear los argumentos
    let matches = cmd.try_get_matches()
        .unwrap_or_else(|e: clap::error::Error| e.exit());

    // PASO 6: Extraer valores manualmente
    let menu_file: PathBuf = matches.get_one::<String>("menu_file")
        .map(|s| s.as_str())
        .unwrap_or("tmenu.toon")
        .into();

    // Si no existe el menu en la carpeta actual, buscar en el home del usuario
    let menu_file = if !menu_file.exists() {
        let home_menu = dirs::home_dir().map(|h| h.join("tmenu.toon"))
            .filter(|p| p.exists())
            .unwrap_or(menu_file);
        home_menu
    } else {
        menu_file
    };

    let debug = matches.get_flag("debug");
    let lang = matches.get_one::<String>("lang")
        .cloned();

    let args = Args {
        menu_file,
        debug,
        lang,
    };

    // PASO 7: Si se especificó --lang, cambiar locale nuevamente (por si acaso)
    if let Some(lang) = args.lang {
        rust_i18n::set_locale(&lang);
    }

    let mut app = App::from_toon(&args.menu_file, args.debug).map_err(|e| match e {
        AppError::IoError(ref io) if io.kind() == io::ErrorKind::NotFound => {
            AppError::MenuFileNotFound(args.menu_file.clone())
        }
        other => other,
    })?;

    enable_raw_mode().map_err(|e| AppError::TerminalError(e.to_string()))?;
    let mut stdout = io::stdout();
    execute!(stdout, crossterm::cursor::SetCursorStyle::SteadyUnderScore)
        .map_err(|e| AppError::TerminalError(e.to_string()))?;

    // Si el terminal es "linux", no usar pantalla alterna ni captura de mouse, porque ese terminal no
    // las soporta bien. En cambio, en otros terminales (xterm, Windows Terminal, etc) sí usar pantalla
    // alterna para evitar problemas de redibujado al salir. Esto se detecta por la variable de entorno TERM.
    let is_tty_native = std::env::var("TERM").unwrap_or_default() == "linux";
    if !is_tty_native {
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .map_err(|e| AppError::TerminalError(e.to_string()))?;
    }

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
                let quit = run_help_modal(terminal, app)?;
                if quit {
                    return Ok(());
                }
                continue;
            }
            // Ctrl+q sale desde cualquier modo y menu, incluso dentro de modales o wizards
            if key.modifiers.contains(event::KeyModifiers::CONTROL)
                && key.code == KeyCode::Char('q')
            {
                return Ok(());
            }

            let should_quit = if app.search_mode {
                handle_search_mode(terminal, app, key.code)?
            } else {
                handle_navigation_mode(terminal, app, key.code)?
            };
            if app.wizard.is_some() {
                let quit = run_wizard(terminal, app)?;
                if quit {
                    return Ok(());
                }
            }

            if should_quit {
                return Ok(());
            }
        }
    }
}

/// Loop bloqueante del modal de ayuda.
/// Retorna Ok(true) si el usuario eligió salir de la app, Ok(false) si cerró la ayuda para volver al menú.
fn run_help_modal(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<bool, AppError> {
    loop {
        terminal
            .draw(|f| ui::ui(f, app))
            .map_err(|e| AppError::TerminalError(e.to_string()))?;

        if let Event::Key(key) = event::read().map_err(|e| AppError::EventError(e.to_string()))? {
            if key.kind != event::KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Esc | KeyCode::F(1) | KeyCode::F(2) => {
                    app.show_help = false;
                    return Ok(false); // cerrar ayuda, continuar app
                }
                KeyCode::Char('q') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    return Ok(true); // salir de la app
                }
                _ => {}
            }
        }
    }
}

/// Loop bloqueante del wizard para interpolación de variables.
fn run_wizard(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<bool, AppError> {
    loop {
        terminal
            .draw(|f| ui::ui(f, app))
            .map_err(|e| AppError::TerminalError(e.to_string()))?;

        if let Event::Key(key) = event::read().map_err(|e| AppError::EventError(e.to_string()))? {
            if key.kind != event::KeyEventKind::Press {
                continue;
            }

            // Ctrl+Q cancela y sale de la app
            if key.modifiers.contains(event::KeyModifiers::CONTROL)
                && key.code == KeyCode::Char('q')
            {
                app.wizard = None;
                return Ok(true); // señal de quit
            }

            match key.code {
                KeyCode::Esc => {
                    // Cancelar: descartar wizard, volver al menú
                    app.wizard = None;
                    return Ok(false);
                }
                KeyCode::Backspace => {
                    if let Some(ref mut w) = app.wizard {
                        w.input.pop();
                    }
                }
                KeyCode::Char(c) => {
                    if let Some(ref mut w) = app.wizard {
                        w.input.push(c);
                    }
                }
                KeyCode::Enter => {
                    let done = app
                        .wizard
                        .as_mut()
                        .map(|w| w.confirm_current())
                        .unwrap_or(true);

                    if done {
                        // Último campo confirmado: ejecutar
                        app.finish_wizard(terminal)?;
                        return Ok(false);
                    }
                    // Si no es el último, el loop redibuja con el siguiente campo
                }
                _ => {}
            }
        }
    }
}

/// Loop bloqueante del modal de confirmación.
pub fn run_confirmation_modal(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<bool, AppError> {
    loop {
        terminal
            .draw(|f| ui::ui(f, app))
            .map_err(|e| AppError::TerminalError(e.to_string()))?;

        if let Event::Key(key) = event::read().map_err(|e| AppError::EventError(e.to_string()))? {
            if key.kind != event::KeyEventKind::Press {
                continue;
            }

            match key.code {
                // Up/Down navega entre "Sí" y "No"
                KeyCode::Up | KeyCode::Left => {
                    if let Some(ref mut conf) = app.confirmation {
                        conf.toggle();
                    }
                }
                KeyCode::Down | KeyCode::Right => {
                    if let Some(ref mut conf) = app.confirmation {
                        conf.toggle();
                    }
                }
                // Enter confirma la selección actual
                KeyCode::Enter => {
                    let confirmed = app
                        .confirmation
                        .as_ref()
                        .map(|c| c.is_confirmed())
                        .unwrap_or(false);
                    app.confirmation = None;
                    return Ok(confirmed);
                }
                // Esc o N = cancelar (ir a "No" implícitamente)
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                    app.confirmation = None;
                    return Ok(false);
                }
                // Y = confirmar (ir a "Sí" implícitamente)
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    app.confirmation = None;
                    return Ok(true);
                }
                // Ctrl+Q cancela y sale de la app
                _ if key.modifiers.contains(event::KeyModifiers::CONTROL)
                    && key.code == KeyCode::Char('q') =>
                {
                    app.confirmation = None;
                    return Err(AppError::EventError("Cancelado por Ctrl+Q".to_string()));
                }
                _ => {}
            }
        }
    }
}

/// Maneja teclas en modo búsqueda.
/// Ahora las teclas de navegación (↑↓) funcionan sobre el menú filtrado en vivo.
/// Recibe el KeyCode ya leido por el loop — sin segundo event::read().
fn handle_search_mode(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    key: KeyCode,
) -> Result<bool, AppError> {
    match key {
        // Tab cierra la búsqueda y mantiene el menú actual
        KeyCode::Tab => {
            app.search_mode = false;
        }
        // Esc cancela la búsqueda: limpia el texto y vuelve al menú completo
        KeyCode::Esc => {
            app.search_mode = false;
            app.search_text.clear();
            app.state.select(Some(0));
        }
        // Backspace: elimina carácter del filtro y reselecciona primer item
        KeyCode::Backspace => {
            app.search_text.pop();
            app.state.select(Some(0));
        }
        // ↑ ↓: navegación en el menú filtrado
        KeyCode::Up => {
            let filtered = app.filtered_items();
            if !filtered.is_empty() {
                let current = app.state.selected().unwrap_or(0);
                let next = if current == 0 {
                    filtered.len() - 1
                } else {
                    current - 1
                };
                app.state.select(Some(next));
            }
        }
        KeyCode::Down => {
            let filtered = app.filtered_items();
            if !filtered.is_empty() {
                let current = app.state.selected().unwrap_or(0);
                let next = (current + 1) % filtered.len();
                app.state.select(Some(next));
            }
        }
        // F2: toggle preview (funciona durante búsqueda)
        KeyCode::F(2) => app.show_preview = !app.show_preview,
        // Enter: ejecuta el item filtrado seleccionado
        KeyCode::Enter => {
            let filtered = app.filtered_items();
            if !filtered.is_empty() {
                if app.activate_item(terminal, &filtered)? {
                    return Ok(true);
                }
            }
        }
        // Cualquier otro carácter: agregar al filtro y resetear a primer item
        KeyCode::Char(c) => {
            app.search_text.push(c);
            app.state.select(Some(0));
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
        KeyCode::Char(c) => {
            app.search_mode = true;
            app.search_text.push(c);
            app.state.select(Some(0));
        }
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
        KeyCode::Left | KeyCode::Esc => {
            if !app.back() {
                return Ok(true); // estamos en root, salir
            }
        }
        _ => {}
    }
    Ok(false)
}