use std::fs;
use std::path::Path;

use crate::error::AppError;
use crate::model::{MenuAction, MenuItem, ExecutionMode};
use crate::model::CommandParam;

/// Configuración global del menú extraída del archivo `.toon`.
#[derive(Clone, Debug)]
pub struct GlobalConfig {
    /// Modo de ejecución: limpiar pantalla o usar la actual
    pub execution_mode: ExecutionMode,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        GlobalConfig {
            execution_mode: ExecutionMode::Inherit,
        }
    }
}

/// Carga y parsea un archivo `.toon`, retornando la configuración global,
/// el titulo principal y la lista de items del menu raiz.
pub fn parse_toon_file(path: &Path) -> Result<(GlobalConfig, String, Vec<MenuItem>), AppError> {
    let content = fs::read_to_string(path)?;

    let mut config = GlobalConfig::default();
    let mut main_title = String::from("Menu Principal");
    let mut stack: Vec<(String, Vec<MenuItem>, usize)> = Vec::new();
    let mut root_items: Vec<MenuItem> = Vec::new();
    let mut indent_levels: Vec<usize> = Vec::new();

    let mut config_base_indent: Option<usize> = None;

    for line in content.lines() {
        let normalized = line.replace('\t', "    ");
        if normalized.trim().is_empty() {
            continue;
        }

        let raw_indent = normalized.len() - normalized.trim_start().len();
        let trimmed = normalized.trim();

        // ========== PARSING DE CONFIGURACIÓN ==========
        if trimmed.starts_with("config:") {
            config_base_indent = Some(raw_indent);
            continue;
        }

        // Si estamos en la sección de configuración (hay config_base_indent)
        if let Some(config_indent) = config_base_indent {
            // Si encontramos una línea indentada bajo config: con ":", es una opción
            if raw_indent > config_indent && trimmed.contains(':') && !trimmed.ends_with(':') {
                // Parsear: "execution_mode: clean"
                if let Some(pos) = trimmed.find(':') {
                    let key = trimmed[..pos].trim();
                    let value = trimmed[pos + 1..].trim();

                    match key {
                        "execution_mode" => {
                            config.execution_mode = ExecutionMode::from_str(value);
                        }
                        _ => {}
                    }
                }
                continue;
            }

            // Si encontramos algo al mismo nivel o menor que config:, salimos de la sección
            if raw_indent <= config_indent && !trimmed.starts_with("config") {
                config_base_indent = None;
            }
        }

        // ========== PARSING DEL MENÚ ==========

        // Registrar el nivel de indentacion si es nuevo
        if !indent_levels.contains(&raw_indent) {
            indent_levels.push(raw_indent);
            indent_levels.sort_unstable();
        }
        let level = indent_levels.iter().position(|&x| x == raw_indent).unwrap_or(0);

        // Titulo principal (nivel 0, termina en ':' fuera de comillas)
        if level == 0 && ends_with_separator_colon(trimmed) {
            let pos = find_separator_colon(trimmed).unwrap();
            main_title = trimmed[..pos]
                .trim_matches('"')
                .trim()
                .to_string();
            continue;
        }

        // Buscar ':' separador fuera de comillas
        if let Some(pos) = find_separator_colon(trimmed) {
            let key = trimmed[..pos].trim_matches('"').trim().to_string();
            let value_with_flag = trimmed[pos + 1..].trim();

            // Extraer flag [confirm=...] si existe
            let (value, require_confirmation) = extract_confirm_flag(value_with_flag);
            let value = value.trim();

            if value.is_empty() {
                // Es un submenu: cerrar los niveles iguales o mayores
                while stack.last().map_or(false, |e| e.2 >= level) {
                    pop_and_insert(&mut stack, &mut root_items);
                }
                stack.push((key, Vec::new(), level));
            } else {
                while stack.last().map_or(false, |e| e.2 >= level) {
                    pop_and_insert(&mut stack, &mut root_items);
                }
                let raw_value = value.trim_matches('"').to_string();
                let action = if raw_value == "exit" {
                    MenuAction::Quit
                } else {
                    MenuAction::Execute(raw_value)
                };
                let item = MenuItem {
                    label: key,
                    action,
                    require_confirmation,
                };
                if let Some(parent) = stack.last_mut() {
                    parent.1.push(item);
                } else {
                    root_items.push(item);
                }
            }
        }
    }

    // Vaciar la pila al terminar el archivo
    while !stack.is_empty() {
        pop_and_insert(&mut stack, &mut root_items);
    }

    Ok((config, main_title, root_items))
}

/// Extrae la flag [confirm=true/false] de una línea si existe.
/// Retorna (línea sin flag, require_confirmation).
/// Default: false (no confirmación por defecto, pero puede ser true).
fn extract_confirm_flag(s: &str) -> (&str, bool) {
    // Buscar [confirm=...] al final
    if let Some(bracket_pos) = s.rfind('[') {
        let rest = &s[bracket_pos..];
        if rest.starts_with("[confirm=") {
            let inner = rest.strip_prefix("[confirm=").and_then(|s| s.strip_suffix("]"));
            if let Some(flag_str) = inner {
                let flag_str = flag_str.trim();
                let line_without_flag = s[..bracket_pos].trim();

                let require_conf = match flag_str {
                    "false" | "False" | "FALSE" | "no" | "No" | "NO" => false,
                    _ => true, // default: true (incluye "true", typos, etc)
                };

                return (line_without_flag, require_conf);
            }
        }
    }
    // No hay flag, default a false
    (s, false)
}

/// Busca el ':' separador fuera de comillas en una cadena.
///
/// Ejemplos:
///   `"Nivel 1":` -> Some(9)
///   `"Item": "echo algo"` -> Some(6)
///   `Submenu:` -> Some(7)
fn find_separator_colon(s: &str) -> Option<usize> {
    let mut in_quotes = false;

    for (i, c) in s.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            ':' if !in_quotes => return Some(i),
            _ => {}
        }
    }
    None
}

/// Verifica si el ':' de la cadena es el ultimo caracter y esta fuera de comillas.
fn ends_with_separator_colon(s: &str) -> bool {
    find_separator_colon(s).map_or(false, |pos| {
        // El ':' debe ser el ultimo caracter (o solo seguido de espacios)
        s[pos + 1..].trim().is_empty()
    })
}

/// Saca el tope de la pila y lo inserta como submenu en el nivel superior
/// o en los items raiz si la pila quedo vacia.
fn pop_and_insert(stack: &mut Vec<(String, Vec<MenuItem>, usize)>, root: &mut Vec<MenuItem>) {
    if let Some((name, items, _)) = stack.pop() {
        let submenu = MenuItem {
            label: name,
            action: MenuAction::OpenSubmenu(items),
            require_confirmation: false,
        };

        if let Some(parent) = stack.last_mut() {
            parent.1.push(submenu);
        } else {
            root.push(submenu);
        }
    }
}

/// Extrae todos los parámetros únicos `{{text: Etiqueta}}` de un comando.
/// Si el mismo placeholder aparece más de una vez, se retorna una sola entrada.
pub fn extract_params(cmd: &str) -> Vec<CommandParam> {
    let mut seen: Vec<String> = Vec::new();
    let mut params = Vec::new();
    let mut rest = cmd;

    while let Some(start) = rest.find("{{") {
        rest = &rest[start + 2..];
        if let Some(end) = rest.find("}}") {
            let inner = rest[..end].trim();
            rest = &rest[end + 2..];

            // Solo manejar el tipo "text:" por ahora; extensible a otros tipos
            if let Some(label) = inner.strip_prefix("text:") {
                let label = label.trim().to_string();
                let placeholder = format!("{{{{text: {}}}}}", label);

                // Deduplicar: si ya vimos este placeholder, no agregarlo de nuevo
                if !seen.contains(&placeholder) {
                    seen.push(placeholder.clone());
                    params.push(CommandParam { label, placeholder });
                }
            }
        } else {
            break; // '}}' no encontrado, malformado — ignorar el resto
        }
    }
    params
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_confirm_flag_true() {
        let (line, flag) = extract_confirm_flag("cmd [confirm=true]");
        assert_eq!(line, "cmd");
        assert!(flag);
    }

    #[test]
    fn test_extract_confirm_flag_false() {
        let (line, flag) = extract_confirm_flag("cmd [confirm=false]");
        assert_eq!(line, "cmd");
        assert!(!flag);
    }

    #[test]
    fn test_extract_confirm_flag_no() {
        let (line, flag) = extract_confirm_flag("cmd [confirm=no]");
        assert_eq!(line, "cmd");
        assert!(!flag);
    }

    #[test]
    fn test_extract_confirm_flag_missing() {
        let (line, flag) = extract_confirm_flag("cmd");
        assert_eq!(line, "cmd");
        assert!(!flag); // default false
    }

    #[test]
    fn test_extract_confirm_flag_typo() {
        let (line, flag) = extract_confirm_flag("cmd [confirm=maybe]");
        assert_eq!(line, "cmd");
        assert!(flag); // default true on typo
    }

    #[test]
    fn test_extract_confirm_flag_with_quotes() {
        let (line, flag) = extract_confirm_flag("\"echo hola\" [confirm=false]");
        assert_eq!(line, "\"echo hola\"");
        assert!(!flag);
    }

    #[test]
    fn test_execution_mode_from_str_clean() {
        assert_eq!(ExecutionMode::from_str("clean"), ExecutionMode::Clean);
    }

    #[test]
    fn test_execution_mode_from_str_inherit() {
        assert_eq!(ExecutionMode::from_str("inherit"), ExecutionMode::Inherit);
    }

    #[test]
    fn test_execution_mode_from_str_default() {
        assert_eq!(ExecutionMode::from_str("unknown"), ExecutionMode::Inherit);
    }
}