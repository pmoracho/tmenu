use ratatui::widgets::ListState;

/// Acción asociada a cada ítem del menú.
#[derive(Clone)]
pub enum MenuAction {
    Execute(String),
    Quit,
    OpenSubmenu(Vec<MenuItem>),
}

/// Un ítem del menú con su etiqueta y acción asociada.
#[derive(Clone)]
pub struct MenuItem {
    pub label: String,
    pub action: MenuAction,
    /// Si true, pedir confirmación antes de ejecutar este comando.
    /// Default: true (seguro por defecto).
    pub require_confirmation: bool,
}

/// Entrada del historial de navegación para poder volver atrás.
pub struct HistoryEntry {
    pub title: String,
    pub items: Vec<MenuItem>,
    pub state: ListState,
}

/// Un parámetro interpolable extraído de un comando.
/// Corresponde a una ocurrencia de `{{text: Etiqueta}}` en el string del comando.
#[derive(Clone, Debug)]
pub struct CommandParam {
    /// Texto que se muestra al usuario como prompt ("Branch name").
    pub label: String,
    /// Placeholder original completo para hacer el reemplazo ("{{text: Branch name}}").
    pub placeholder: String,
}

/// Estado de una confirmación de ejecución de comando.
/// El usuario ve el comando y elige Sí / No con las flechas.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ConfirmationState {
    /// El comando que se quiere ejecutar
    pub cmd: String,
    /// Índice de selección: 0 = "Sí", 1 = "No" (se navega con Up/Down)
    pub selected: usize,
}

#[allow(dead_code)]
impl ConfirmationState {
    pub fn new(cmd: String) -> Self {
        ConfirmationState {
            cmd,
            selected: 0, // por defecto "Sí" está seleccionado (es más seguro que "No")
        }
    }

    /// Retorna true si el usuario selecciona "Sí"
    pub fn is_confirmed(&self) -> bool {
        self.selected == 0
    }

    /// Navega entre las opciones (Up/Down)
    pub fn toggle(&mut self) {
        self.selected = if self.selected == 0 { 1 } else { 0 };
    }
}