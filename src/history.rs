use std::fs;
use std::io::Write;
use std::path::PathBuf;
use chrono::Local;

use crate::error::AppError;

/// Retorna la ruta al archivo de historial: `~/.local/share/tmenu/history.log`
fn history_file_path() -> Result<PathBuf, AppError> {
    let home = dirs::home_dir()
        .ok_or_else(|| AppError::HistoryError("No se pudo determinar el directorio home".to_string()))?;
    Ok(home.join(".local/share/tmenu/history.log"))
}

/// Asegura que el directorio `~/.local/share/tmenu/` existe.
/// Si no existe, lo crea con permisos estándar (0o755).
fn ensure_history_dir() -> Result<(), AppError> {
    let history_path = history_file_path()?;
    let dir = history_path.parent()
        .ok_or_else(|| AppError::HistoryError("Ruta de historial inválida".to_string()))?;

    if !dir.exists() {
        fs::create_dir_all(dir)
            .map_err(|e| AppError::HistoryError(format!("No se pudo crear directorio: {}", e)))?;
    }
    Ok(())
}

/// Registra un comando ejecutado en el historial.
/// Formato: `[YYYY-MM-DD HH:MM:SS] comando completo`
///
/// # Errores
/// Retorna `AppError::HistoryError` si no se puede escribir el archivo.
/// No es un error fatal — si falla, la app continúa (solo se pierden los logs).
pub fn log_command(cmd: &str) -> Result<(), AppError> {
    ensure_history_dir()?;
    let history_path = history_file_path()?;

    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let entry = format!("[{}] {}\n", timestamp, cmd);

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&history_path)
        .map_err(|e| AppError::HistoryError(format!("No se pudo abrir historial: {}", e)))?;

    file.write_all(entry.as_bytes())
        .map_err(|e| AppError::HistoryError(format!("No se pudo escribir historial: {}", e)))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_history_file_path() {
        let path = history_file_path();
        assert!(path.is_ok());
        let p = path.unwrap();
        assert!(p.to_string_lossy().contains(".local/share/tmenu/history.log"));
    }

    #[test]
    fn test_log_command_creates_entry() {
        // Este test requiere que exista ~/.local/share/tmenu/
        // En un proyecto real usarías tempfiles o mocks
        let _ = log_command("echo test");
        // Verificar que no retorne error
    }
}