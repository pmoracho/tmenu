use ratatui::widgets::ListState;

/// Acción asociada a cada ítem del menú.
#[derive(Clone)]
pub enum MenuAction {
    /// Comando de shell a ejecutar.
    Execute(String),
    /// Submenú con su lista de ítems.
    OpenSubmenu(Vec<MenuItem>),
}

/// Un ítem del menú con su etiqueta y acción asociada.
#[derive(Clone)]
pub struct MenuItem {
    pub label: String,
    pub action: MenuAction,
}

/// Entrada del historial de navegación para poder volver atrás.
pub struct HistoryEntry {
    pub title: String,
    pub items: Vec<MenuItem>,
    pub state: ListState,
}