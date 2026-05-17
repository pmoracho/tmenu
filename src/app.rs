use ratatui::{Terminal, backend::CrosstermBackend, widgets::ListState};
use rust_i18n::t;
use crossterm::{
    event::DisableMouseCapture,
    event::EnableMouseCapture,
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use std::io::{self, Stdout};
use std::process::Command;

use crate::model::{
    CommandParam, ConfirmationState, ExecutionMode, HistoryEntry, MenuAction, MenuItem,
};

use crate::parser::parse_toon_file;
use crate::search::{filter_recursive, find_first_command};
use crate::{error::AppError, history, parser};
// Asumiendo que tu macro t! se expone o importa desde aquí

/// Estado principal de la aplicación TUI.
pub struct App {
    pub history: Vec<HistoryEntry>,
    pub current_title: String,
    pub current_items: Vec<MenuItem>,
    pub root_title: String,
    pub root_items: Vec<MenuItem>,
    pub state: ListState,
    pub search_text: String,
    pub search_mode: bool,
    pub show_preview: bool,
    pub show_help: bool,
    pub debug: bool,
    pub wizard: Option<WizardState>,
    pub confirmation: Option<ConfirmationState>,
    pub execution_mode: ExecutionMode,
}

/// Implementación de métodos para `App`.
impl App {
    /// Crea una instancia de `App` cargando el menú desde un archivo `.toon`.
    pub fn from_toon(path: &std::path::Path, debug: bool) -> Result<Self, AppError> {

        let (config, main_title, root_items) = parse_toon_file(path)?;
        let mut state = ListState::default();
        state.select(Some(0));

        Ok(App {
            history: Vec::new(),
            current_title: main_title.clone(),
            current_items: root_items.clone(),
            root_title: main_title,
            root_items,
            state,
            search_text: String::new(),
            search_mode: false,
            show_preview: false,
            show_help: false,
            debug,
            wizard: None,
            confirmation: None,
            execution_mode: config.execution_mode,
        })
    }

    /// Devuelve los ítems filtrados según el texto de búsqueda actual.
    /// Si no hay búsqueda activa, retorna todos los ítems del nivel actual.
    pub fn filtered_items(&self) -> Vec<MenuItem> {
        if !self.search_mode || self.search_text.is_empty() {
            return self.current_items.clone();
        }

        let mut results = filter_recursive(&self.current_items, &self.search_text, 0);

        if results.is_empty() {
            if let Some(fallback) = find_first_command(&self.current_items) {
                results.push(fallback);
            }
        }
        results
    }

    /// Avanza la selección al siguiente ítem (con wrap-around).
    pub fn next(&mut self) {
        let len = self.current_items.len();
        if len == 0 {
            return;
        }
        let i = self.state.selected().map_or(0, |i| (i + 1) % len);
        self.state.select(Some(i));
    }

    /// Retrocede la selección al ítem anterior (con wrap-around).
    pub fn previous(&mut self) {
        let len = self.current_items.len();
        if len == 0 {
            return;
        }
        let i = self
            .state
            .selected()
            .map_or(0, |i| if i == 0 { len - 1 } else { i - 1 });
        self.state.select(Some(i));
    }

    /// Vuelve al menú anterior en el historial.
    pub fn back(&mut self) -> bool {
        if let Some(entry) = self.history.pop() {
            self.current_title = entry.title;
            self.current_items = entry.items;
            self.state = entry.state;
            true
        } else {
            false
        }
    }

    /// Vuelve directamente al menú raíz usando los ítems guardados al inicio.
    pub fn go_home(&mut self) {
        if self.history.is_empty() {
            return;
        }
        self.history.clear();
        self.current_title = self.root_title.clone();
        self.current_items = self.root_items.clone();
        self.state = ListState::default();
        self.state.select(Some(0));
    }

    /// Guarda el estado actual en el historial antes de navegar a un submenú.
    fn push_history(&mut self) {
        self.history.push(HistoryEntry {
            title: self.current_title.clone(),
            items: self.current_items.clone(),
            state: self.state.clone(),
        });
    }

    /// Activa el ítem en el índice seleccionado de `list`.
    /// Retorna `true` si la aplicación debe cerrarse (comando "exit").
    pub fn activate_item(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
        list: &[MenuItem],
    ) -> Result<bool, AppError> {
        let Some(index) = self.state.selected() else {
            return Ok(false);
        };
        let Some(item) = list.get(index) else {
            return Ok(false);
        };

        match &item.action.clone() {
            MenuAction::Quit => return Ok(true),
            MenuAction::Execute(cmd_str) => {
                let cmd = cmd_str.trim().trim_matches('"');
                if cmd == "exit" {
                    return Ok(true);
                }

                let params = parser::extract_params(cmd);
                if params.is_empty() {
                    // Sin interpolación: pedir confirmación solo si el ítem lo requiere
                    if item.require_confirmation {
                        return self.request_command_confirmation(terminal, cmd);
                    } else {
                        // Ejecutar directo sin confirmación
                        self.execute_external_command(terminal, cmd)?;
                    }
                } else {
                    // Con interpolación: iniciar wizard (no ejecutar todavía)
                    self.wizard = Some(WizardState::new(
                        params,
                        cmd.to_string(),
                        item.require_confirmation,
                    ));
                }
            }
            MenuAction::OpenSubmenu(sub_items) => {
                self.search_text.clear();
                self.search_mode = false;
                self.push_history();
                self.current_title = item.label.clone();
                self.current_items = sub_items.clone();
                self.state = ListState::default();
                self.state.select(Some(0));
            }
        }
        Ok(false)
    }

    /// Intenta ejecutar un comando, mostrando primero un modal de confirmación.
    /// Si el usuario confirma (Sí), se ejecuta y se registra en el historial.
    /// Retorna true si la app debe cerrarse.
    pub fn request_command_confirmation(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
        cmd: &str,
    ) -> Result<bool, AppError> {
        // Validar seguridad ANTES de mostrar cualquier modal
        if !Self::is_safe_command(cmd) {
            return Err(AppError::ForbiddenCommand(cmd.to_string()));
        }

        // Mostrar modal de confirmación
        self.confirmation = Some(ConfirmationState::new(cmd.to_string()));

        // Ejecutar el modal bloqueante — devuelve true si se ejecutó, false si se canceló
        let should_execute = crate::run_confirmation_modal(terminal, self)?;

        if should_execute {
            self.execute_external_command(terminal, cmd)?;
        }

        Ok(false)
    }

    /// Ejecuta un comando externo en el shell del sistema operativo.
    ///
    /// Si `execution_mode` es `Clean`, ejecuta `clear` antes del comando.
    /// Antes de ejecutar, restaura la terminal a modo normal y la reconfigura
    /// en modo TUI al finalizar.
    ///
    /// # Seguridad
    /// - Rechaza comandos con path traversal (`..`).
    /// - Usa `shlex::split` para respetar quoting correctamente en lugar de
    ///   `split_whitespace`, que parte argumentos con espacios.
    /// - Los valores interpolados por el wizard se validan aquí también,
    ///   ya que `finish_wizard` llama a este método con el comando resuelto.
    pub fn execute_external_command(
        &self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
        cmd: &str,
    ) -> Result<(), AppError> {
        if !Self::is_safe_command(cmd) {
            return Err(AppError::ForbiddenCommand(cmd.to_string()));
        }

        if self.debug {
            eprintln!("{} {:?}", t!("logs.debugging"), cmd);
        }

        // Restaurar terminal a modo normal
        let _ = disable_raw_mode();
        if let Err(e) = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture) {
            eprintln!("{} {}", t!("errors.terminal_restore"), e);
        }

        // Si el modo es Clean, limpiar pantalla antes de ejecutar
        if self.execution_mode == crate::model::ExecutionMode::Clean {
            let _ = std::process::Command::new("clear").spawn();
        }

        // Parsear respetando quoting ("arg con espacios" se trata como un solo arg).
        let parts: Vec<String> = shlex::split(cmd)
            .unwrap_or_else(|| cmd.split_whitespace().map(str::to_string).collect());

        if let Some((bin, args)) = parts.split_first() {
            let mut command = Command::new(bin);
            command.args(args);
            match command.spawn() {
                Ok(mut child) => {
                    let _ = child.wait();
                    // Registrar en historial solo si la ejecución fue exitosa
                    if let Err(e) = history::log_command(cmd) {
                        eprintln!("{} {}", t!("errors.history_save"), e);
                    }
                }
                Err(e) => eprintln!("{} '{}': {}", t!("errors.execution_failed"), bin, e),
            }
        }

        println!("\n{}", t!("prompts.press_enter"));
        let _ = io::stdin().read_line(&mut String::new());

        // Volver a modo TUI
        if let Err(e) = enable_raw_mode() {
            eprintln!("{} {}", t!("errors.raw_mode_enable"), e);
        }
        if let Err(e) = execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture) {
            eprintln!("{} {}", t!("errors.alt_screen_restore"), e);
        }
        terminal
            .clear()
            .map_err(|e| AppError::TerminalError(e.to_string()))?;

        Ok(())
    }

    /// Construye la cadena de breadcrumb para el título, truncando si es muy larga.
    pub fn breadcrumb(&self) -> String {
        const MAX_WIDTH: usize = 40;

        if self.history.is_empty() {
            return self.current_title.clone();
        }

        let root = self.history[0].title.as_str();
        let current = self.current_title.as_str();

        // Construir la cadena completa y ver si entra
        let mut parts: Vec<&str> = self.history.iter().map(|e| e.title.as_str()).collect();
        parts.push(current);
        let full = parts.join(" › ");

        if full.chars().count() <= MAX_WIDTH {
            return full;
        }

        // Truncar: Raíz › .. › Actual
        let candidate = format!("{} › .. › {}", root, current);
        if candidate.chars().count() <= MAX_WIDTH {
            return candidate;
        }

        // Caso extremo: solo el nivel actual (root o current son muy largos)
        if current.chars().count() <= MAX_WIDTH {
            return current.to_string();
        }
        current.chars().take(MAX_WIDTH).collect()
    }

    /// Finaliza el wizard: si requiere confirmación, muestra modal; sino, ejecuta directo.
    pub fn finish_wizard(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<bool, AppError> {
        if let Some(ref wizard) = self.wizard {
            let cmd = wizard.resolve();
            let require_confirmation = wizard.require_confirmation;
            self.wizard = None;

            if require_confirmation {
                // Pedir confirmación antes de ejecutar el comando resuelto
                return self.request_command_confirmation(terminal, &cmd);
            } else {
                // Ejecutar directo sin confirmación
                self.execute_external_command(terminal, &cmd)?;
            }
        }
        Ok(false)
    }

    /// Valida que el comando no contenga path traversal ni caracteres de shell peligrosos.
    ///
    /// Nota: no se usan pipes/shell, así que `|`, `&`, `;` no son vectores de inyección
    /// en este contexto — pero `..` sí puede usarse para path traversal en argumentos.
    fn is_safe_command(cmd: &str) -> bool {
        // Rechazar path traversal explícito
        if cmd.split_whitespace().any(|part| part.contains("..")) {
            return false;
        }

        // Rechazar shell metacaracteres peligrosos (aunque no ejecutemos sh -c)
        // - `;` puede concatenar comandos si pasa a exec sin cuidado
        // - `$()` o backticks podrían expandirse
        // - `|`, `&`, `>`, `<` son redirecciones
        // Nota: `shlex::split` respeta quoting, así que "arg;with;semicolon" es seguro

        // Allowlist de caracteres válidos (más restrictivo)
        cmd.chars().all(|c| {
            matches!(c,
                'a'..='z' | 'A'..='Z' | '0'..='9'
                | ' ' | '.' | '/' | '_' | '-' | '='
                | ':' | '@' | '+' | '%' | '~' | ','
                | '\'' | '"'
            )
        })
    }
}

/// Estado del wizard de interpolación de parámetros.
pub struct WizardState {
    pub params: Vec<CommandParam>,
    pub current: usize,
    pub values: Vec<String>,
    pub input: String,
    pub original_cmd: String,
    pub require_confirmation: bool,
}

/// Implementación de métodos para `WizardState`.
impl WizardState {
    pub fn new(params: Vec<CommandParam>, cmd: String, require_confirmation: bool) -> Self {
        let len = params.len();
        WizardState {
            params,
            current: 0,
            values: vec![String::new(); len],
            input: String::new(),
            original_cmd: cmd,
            require_confirmation,
        }
    }

    pub fn current_param(&self) -> &CommandParam {
        &self.params[self.current]
    }

    pub fn confirm_current(&mut self) -> bool {
        self.values[self.current] = self.input.clone();
        self.input.clear();
        if self.current + 1 >= self.params.len() {
            true
        } else {
            self.current += 1;
            false
        }
    }

    pub fn resolve(&self) -> String {
        let mut cmd = self.original_cmd.clone();
        for (param, value) in self.params.iter().zip(self.values.iter()) {
            cmd = cmd.replace(&param.placeholder, value);
        }
        cmd
    }
}