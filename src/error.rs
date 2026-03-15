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
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::MenuFileNotFound(path) => {
                write!(f, "El archivo de menú no fue encontrado: {}", path.display())
            }
            AppError::IoError(e) => write!(f, "Error de I/O: {}", e),
            AppError::TerminalError(msg) => write!(f, "Error de terminal: {}", msg),
            AppError::ForbiddenCommand(c) => {
                write!(f, "El comando contiene caracteres no permitidos: '{}'", c)
            }
            AppError::EventError(msg) => write!(f, "Error de evento de terminal: {}", msg),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AppError::IoError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::IoError(e)
    }
}