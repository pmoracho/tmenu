use std::fmt;
use std::path::PathBuf;

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
                write!(
                    f,
                    "El archivo de menú no fue encontrado: {}\n",
                    path.display()
                )
            }
            AppError::IoError(e) => write!(f, "Error de I/O: {}\n", e),
            AppError::TerminalError(msg) => write!(f, "Error de terminal: {}\n", msg),
            AppError::ForbiddenCommand(c) => {
                write!(f, "El comando contiene caracteres no permitidos: '{}'\n", c)
            }
            AppError::EventError(msg) => write!(f, "Error de evento de terminal: {}\n", msg),
            AppError::HistoryError(msg) => write!(f, "Error al guardar historial: {}\n", msg),
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
