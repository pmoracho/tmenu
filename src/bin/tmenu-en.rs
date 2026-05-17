use std::io;

use crossterm::{event::DisableMouseCapture, execute, terminal::{LeaveAlternateScreen, disable_raw_mode}};
use tmenu::run;

fn main() {
    let idioma_compilacion = option_env!("RUST_I18N_DEFAULT_LOCALE").unwrap_or("en");
    rust_i18n::set_locale(idioma_compilacion);

    // registrar un hook de pánico:
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(info);
    }));

    if let Err(err) = run() {
        // Aquí es donde ocurre la magia: {} usa tu impl fmt::Display
        eprintln!("\n❌ Error: {}", err);
        std::process::exit(1);
    }
}