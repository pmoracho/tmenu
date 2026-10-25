use std::fs;
use std::path::Path;

use crate::error::AppError;
use crate::model::CommandParam;
use crate::model::{ExecutionMode, MenuAction, MenuItem};
use crate::search::unquote;

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
pub fn parse_toon_file(
    path: &Path,
) -> Result<(GlobalConfig, String, Vec<MenuItem>, Vec<String>), AppError> {
    let content = fs::read_to_string(path)?;
    let cleaned_content = clean_invisible_chars(&content);

    let mut config = GlobalConfig::default();
    let mut main_title = String::from("Menu Principal");
    let mut stack: Vec<(String, Vec<MenuItem>, usize)> = Vec::new();
    let mut root_items: Vec<MenuItem> = Vec::new();
    let mut indent_levels: Vec<usize> = Vec::new();

    let mut config_base_indent: Option<usize> = None;
    let mut warnings: Vec<String> = Vec::new();

    for (idx, line) in cleaned_content.lines().enumerate() {
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
        let level = indent_levels
            .iter()
            .position(|&x| x == raw_indent)
            .unwrap_or(0);

        // Titulo principal (nivel 0, termina en ':' fuera de comillas)
        if level == 0 && ends_with_separator_colon(trimmed) {
            let pos = find_separator_colon(trimmed).unwrap();
            main_title = trimmed[..pos].trim_matches('"').trim().to_string();
            continue;
        }

        // Buscar ':' separador fuera de comillas
        if let Some(pos) = find_separator_colon(trimmed) {
            let key = trimmed[..pos].trim_matches('"').trim().to_string();
            let value_with_flag = trimmed[pos + 1..].trim();

            // Extraer flag [confirm=...] si existe
            let (value, flags) = extract_flags(value_with_flag);
            let value = value.trim();
            let require_confirmation = flag_bool(&flags, "confirm", false);
            let require_shell = flag_bool(&flags, "shell", false);

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
                let raw_value = unquote(value).to_string();
                for p in unknown_placeholders(&raw_value) {
                    warnings.push(format!(
                        "línea {}: '{}' contiene un placeholder desconocido: {}",
                        idx + 1,
                        key,
                        p
                    ));
                }

                let action = if raw_value == "exit" {
                    MenuAction::Quit
                } else {
                    MenuAction::Execute(raw_value)
                };
                let item = MenuItem {
                    label: key,
                    action,
                    require_confirmation,
                    require_shell
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

    Ok((config, main_title, root_items, warnings))
}

/// Extrae todos los flags [key=value] al final de la línea.
/// Retorna (línea sin flags, mapa de flags encontrados).
fn extract_flags(s: &str) -> (&str, Vec<(String, String)>) {
    let mut flags = Vec::new();
    let mut rest = s.trim_end();

    loop {
        let trimmed = rest.trim_end();
        if !trimmed.ends_with(']') {
            break;
        }
        let Some(bracket_pos) = trimmed.rfind('[') else { break };
        let inner = &trimmed[bracket_pos + 1..trimmed.len() - 1];
        let Some(eq_pos) = inner.find('=') else { break };

        let key = inner[..eq_pos].trim().to_string();
        let value = inner[eq_pos + 1..].trim().to_string();
        flags.push((key, value));
        rest = trimmed[..bracket_pos].trim_end();
    }

    flags.reverse(); // mantener orden original
    (rest, flags)
}

fn flag_bool(flags: &[(String, String)], key: &str, default: bool) -> bool {
    flags.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| !matches!(v.as_str(), "false" | "False" | "FALSE" | "no" | "No" | "NO"))
        .unwrap_or(default)
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
            require_shell: false,
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

/// Devuelve los `{{...}}` de un comando que no son un parámetro válido
/// (`{{text: Etiqueta}}`). Incluye llaves sin cerrar.
pub fn unknown_placeholders(cmd: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = cmd;

    while let Some(start) = rest.find("{{") {
        rest = &rest[start + 2..];
        match rest.find("}}") {
            Some(end) => {
                let inner = rest[..end].trim();
                if !inner.starts_with("text:") {
                    out.push(format!("{{{{{}}}}}", inner));
                }
                rest = &rest[end + 2..];
            }
            None => {
                out.push(format!("{{{{{}", rest.trim()));
                break;
            }
        }
    }
    out
}

fn clean_invisible_chars(s: &str) -> String {
    let original_len = s.len();
    let result: String = s
        .chars()
        .filter(|c| {
            !matches!(c,
                '\u{FE0F}' |    // Emoji Variation Selector
                '\u{200B}' |    // Zero Width Space
                '\u{FEFF}' |    // Byte Order Mark
                '\u{200C}'      // Zero Width Non-Joiner
            )
        })
        .collect();
    let cleaned_len = result.len();

    // Log de debug si algo fue removido
    if original_len != cleaned_len {
        eprintln!(
            "[parser] Caracteres invisibles removidos: {} → {} bytes",
            original_len, cleaned_len
        );
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_flags_shell_and_confirm() {
        let (line, flags) = extract_flags("cmd [confirm=true] [shell=true]");
        assert_eq!(line, "cmd");
        assert!(flag_bool(&flags, "confirm", false));
        assert!(flag_bool(&flags, "shell", false));
    }

    #[test]
    fn test_extract_flags_none() {
        let (line, flags) = extract_flags("cmd");
        assert_eq!(line, "cmd");
        assert!(flags.is_empty());
    }

    #[test]
    fn test_execution_mode_from_str_inherit() {
        assert_eq!(ExecutionMode::from_str("inherit"), ExecutionMode::Inherit);
    }

    #[test]
    fn test_execution_mode_from_str_default() {
        assert_eq!(ExecutionMode::from_str("unknown"), ExecutionMode::Inherit);
    }
    #[test]
    fn test_clean_emoji_variation_selector() {
        let dirty = "🛠\u{FE0F}";
        let clean = clean_invisible_chars(dirty);
        assert_eq!(clean, "🛠");
    }

    #[test]
    fn test_clean_zero_width_space() {
        let dirty = "test\u{200B}word";
        let clean = clean_invisible_chars(dirty);
        assert_eq!(clean, "testword");
    }

    #[test]
    fn test_clean_byte_order_mark() {
        let dirty = "\u{FEFF}test";
        let clean = clean_invisible_chars(dirty);
        assert_eq!(clean, "test");
    }

    #[test]
    fn test_clean_zero_width_non_joiner() {
        let dirty = "test\u{200C}word";
        let clean = clean_invisible_chars(dirty);
        assert_eq!(clean, "testword");
    }

    #[test]
    fn test_clean_multiple_invisible_chars() {
        let dirty = "\u{FEFF}test\u{200B}word\u{FE0F}";
        let clean = clean_invisible_chars(dirty);
        assert_eq!(clean, "testword");
    }

    #[test]
    fn test_clean_preserves_accents() {
        let text = "café";
        let clean = clean_invisible_chars(text);
        assert_eq!(clean, "café");  // ✅ Se mantiene el acento
    }

    #[test]
    fn test_clean_preserves_zwj_emoji() {
        let text = "👨\u{200D}💻";
        let clean = clean_invisible_chars(text);
        assert_eq!(clean, "👨\u{200D}💻");  // ✅ Emoji compuesto intacto
    }

    #[test]
    fn test_clean_empty_string() {
        assert_eq!(clean_invisible_chars(""), "");
    }

    #[test]
    fn test_clean_normal_text() {
        let text = "Hola mundo";
        let clean = clean_invisible_chars(text);
        assert_eq!(clean, "Hola mundo");  // ✅ Texto normal sin cambios
    }
    #[test]
    fn test_unquote_envolvente() {
        assert_eq!(unquote("\"git pull\""), "git pull");
    }

    #[test]
    fn test_unquote_conserva_comilla_final_del_argumento() {
        assert_eq!(
            unquote("git commit -m \"WIP\""),
            "git commit -m \"WIP\""
        );
    }

    #[test]
    fn test_unquote_sin_comillas() {
        assert_eq!(unquote("git status"), "git status");
    }

    #[test]
    fn test_unquote_comilla_suelta() {
        assert_eq!(unquote("\""), "\"");
    }

    #[test]
    fn test_placeholder_valido_no_advierte() {
        assert!(unknown_placeholders("git tag {{text: Etiqueta}}").is_empty());
    }

    #[test]
    fn test_placeholder_typo() {
        assert_eq!(
            unknown_placeholders("git tag {{test:Etiquete}}"),
            vec!["{{test:Etiquete}}".to_string()]
        );
    }

    #[test]
    fn test_placeholder_sin_cerrar() {
        assert_eq!(
            unknown_placeholders("echo {{text: x"),
            vec!["{{text: x".to_string()]
        );
    }

    #[test]
    fn test_sin_placeholders() {
        assert!(unknown_placeholders("git status").is_empty());
    }

}
