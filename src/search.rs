use crate::model::{MenuAction, MenuItem};

/// Verifica si `text` contiene los caracteres de `query` en el mismo orden,
/// sin necesidad de que sean contiguos. La comparación es insensible a mayúsculas.
pub fn is_fuzzy_match(text: &str, query: &str) -> bool {
    let mut chars = text.chars();
    query.chars().all(|q| {
        chars
            .by_ref()
            .any(|c| c.to_lowercase().eq(q.to_lowercase()))
    })
}

/// Filtra recursivamente los ítems del menú usando coincidencia fuzzy sobre
/// los ítems ejecutables (comandos). Los submenús se recorren pero no se incluyen
/// directamente en los resultados.
pub fn filter_recursive(items: &[MenuItem], query: &str, depth: usize) -> Vec<MenuItem> {
    if depth > 32 {
        return Vec::new(); // prevenir recursión excesiva
    }
    let mut results = Vec::new();
    for item in items {
        match &item.action {
            MenuAction::Execute(_) | MenuAction::Quit => {
                if is_fuzzy_match(&item.label, query) {
                    results.push(item.clone());
                }
            }
            MenuAction::OpenSubmenu(sub_items) => {
                results.extend(filter_recursive(sub_items, query, depth + 1));
            }
        }
    }
    results
}

/// Retorna el primer ítem ejecutable encontrado en la lista, recorriendo
/// submenús recursivamente. Usado como fallback cuando la búsqueda no encuentra resultados.
pub fn find_first_command(items: &[MenuItem]) -> Option<MenuItem> {
    for item in items {
        match &item.action {
            MenuAction::Execute(_) => return Some(item.clone()),
            MenuAction::Quit => {} // no usar como fallback de búsqueda
            MenuAction::OpenSubmenu(sub_items) => {
                if let Some(found) = find_first_command(sub_items) {
                    return Some(found);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_match_exact() {
        assert!(is_fuzzy_match("hola mundo", "hola"));
    }

    #[test]
    fn test_fuzzy_match_sparse() {
        assert!(is_fuzzy_match("Configuración del sistema", "conf"));
    }

    #[test]
    fn test_fuzzy_match_case_insensitive() {
        assert!(is_fuzzy_match("Salir", "sal"));
        assert!(is_fuzzy_match("salir", "SAL"));
    }

    #[test]
    fn test_fuzzy_no_match() {
        assert!(!is_fuzzy_match("hola", "xyz"));
    }
}
