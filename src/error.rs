use std::fmt;
use std::path::PathBuf;
use rust_i18n::t;

/// Errores tipados de la aplicación.
#[derive(Debug)]
pub enum AppError {
    MenuFileNotFound(PathBuf),
    IoError(std::io::Error),
    TerminalError(String),
    ForbiddenCommand(String),
    EventError(String),
    HistoryError(String),
}

/// Implementación de `Display` para `AppError` para mostrar mensajes amigables al usuario.
impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {

        match self {
            AppError::MenuFileNotFound(path) => {
                // Pasamos la ruta formateada como variable al archivo de traducción
                write!(
                    f,
                    "{}\n",
                    t!("errors.menu_file_not_found", path = path.display().to_string())
                )
            }
            AppError::IoError(e) => {
                write!(f, "{}\n", t!("errors.io_error", error = e.to_string()))
            }
            AppError::TerminalError(msg) => {
                write!(f, "{}\n", t!("errors.terminal_error", msg = msg))
            }
            AppError::ForbiddenCommand(c) => {
                write!(f, "{}\n", t!("errors.forbidden_command", command = c))
            }
            AppError::EventError(msg) => {
                write!(f, "{}\n", t!("errors.event_error", msg = msg))
            }
            AppError::HistoryError(msg) => {
                write!(f, "{}\n", t!("errors.history_error", msg = msg))
            }
        }
    }
}

/// Implementación de `std::error::Error` para `AppError` para permitir encadenamiento de errores.
impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AppError::IoError(e) => Some(e),
            _ => None,
        }
    }
}

/// Implementación de conversión de `std::io::Error` a `AppError` para facilitar el manejo de errores.
impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::IoError(e)
    }
}
