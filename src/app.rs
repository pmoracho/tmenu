use ratatui::{Terminal, backend::Backend, widgets::ListState};

use crossterm::{
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
    },
    event::DisableMouseCapture,
    event::EnableMouseCapture,
};
use std::io;
use std::process::Command;

use crate::{error::AppError, parser};
use crate::model::{HistoryEntry, MenuAction, MenuItem, CommandParam};
use crate::parser::parse_toon_file;
use crate::search::{filter_recursive, find_first_command};

/// Estado principal de la aplicación TUI.
pub struct App {
    pub history: Vec<HistoryEntry>,
    pub current_title: String,
    pub current_items: Vec<MenuItem>,
    pub state: ListState,
    pub search_text: String,
    pub search_mode: bool,
    pub show_preview: bool, 
    pub show_help: bool,
    pub debug: bool,
    pub wizard: Option<WizardState>,
}

impl App {
    /// Crea una instancia de `App` cargando el menú desde un archivo `.toon`.
    pub fn from_toon(path: &std::path::Path, debug: bool) -> Result<Self, AppError> {
        let (main_title, root_items) = parse_toon_file(path)?;

        let mut state = ListState::default();
        state.select(Some(0));

        Ok(App {
            history: Vec::new(),
            current_title: main_title,
            current_items: root_items,
            state,
            search_text: String::new(),
            search_mode: false,
            show_preview: false,
            show_help: false,
            debug,
            wizard: None,
        })
    }

    /// Devuelve los ítems filtrados según el texto de búsqueda actual.
    /// Si no hay búsqueda activa, retorna todos los ítems del nivel actual.
    pub fn filtered_items(&self) -> Vec<MenuItem> {
        if !self.search_mode || self.search_text.is_empty() {
            return self.current_items.clone();
        }

        let mut results = filter_recursive(&self.current_items, &self.search_text);

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

    /// Vuelve directamente al menú raíz, limpiando todo el historial.
    pub fn go_home(&mut self) {
        if self.history.is_empty() {
            return;
        }
        // El root es el primer elemento guardado
        let root = self.history.drain(..).next().unwrap();
        self.current_title = root.title;
        self.current_items = root.items;
        self.state = root.state;
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
    pub fn activate_item<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
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
                    // Sin interpolación: ejecutar directo como antes
                    self.execute_external_command(terminal, cmd)?;
                } else {
                    // Con interpolación: iniciar wizard (no ejecutar todavía)
                    self.wizard = Some(WizardState::new(params, cmd.to_string()));
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

    /// Ejecuta un comando externo en el shell del sistema operativo.
    ///
    /// Antes de ejecutar, restaura la terminal a modo normal y la reconfigura
    /// en modo TUI al finalizar.
    ///
    /// # Seguridad
    /// Se rechazan comandos que contengan caracteres de shell peligrosos para
    /// evitar inyección de comandos desde el archivo `.toon`.
    pub fn execute_external_command<B: Backend>(
        &self,
        terminal: &mut Terminal<B>,
        cmd: &str,
    ) -> Result<(), AppError> {
        // Validar caracteres peligrosos
        if !Self::is_safe_command(cmd) {
            return Err(AppError::ForbiddenCommand(cmd.to_string()));
        }

        if self.debug {
            eprintln!("[debug] ejecutando: {:?}", cmd);
        }

        // Restaurar terminal a modo normal
        let _ = disable_raw_mode();
        if let Err(e) =
            execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)
        {
            eprintln!("[warn] no se pudo restaurar la terminal: {}", e);
        }

        let mut command = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", cmd]);
            c
        } else {
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            let (bin, args) = parts.split_first().unwrap_or((&cmd, &[]));
            let mut c = Command::new(bin);
            c.args(args);
            c
        };

        match command.spawn() {
            Ok(mut child) => { let _ = child.wait(); }
            Err(e) => eprintln!("[error] no se pudo ejecutar el comando: {}", e),
        }

        println!("\nPresioná Enter para volver...");
        let _ = io::stdin().read_line(&mut String::new());

        // Volver a modo TUI
        if let Err(e) = enable_raw_mode() {
            eprintln!("[warn] no se pudo activar raw mode: {}", e);
        }
        if let Err(e) = execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture) {
            eprintln!("[warn] no se pudo restaurar pantalla alternativa: {}", e);
        }
        terminal
            .clear()
            .map_err(|e| AppError::TerminalError(e.to_string()))?;

        Ok(())
    }
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
    /// Ejecuta el comando resuelto y limpia el wizard.
    pub fn finish_wizard<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<bool, AppError> {
        if let Some(ref wizard) = self.wizard {
            let cmd = wizard.resolve();
            self.wizard = None;
            self.execute_external_command(terminal, &cmd)?;
        }
        Ok(false)
    }    

    fn is_safe_command(cmd: &str) -> bool {
    cmd.chars().all(|c| matches!(c,
        'a'..='z' | 'A'..='Z' | '0'..='9'
        | ' ' | '.' | '/' | '_' | '-' | '='
    ))
}
}

/// Estado del wizard de interpolación de parámetros.
pub struct WizardState {
    /// Parámetros a completar, en orden.
    pub params: Vec<CommandParam>,
    /// Índice del parámetro actual.
    pub current: usize,
    /// Valores ingresados hasta ahora (mismo orden que `params`).
    pub values: Vec<String>,
    /// Texto que el usuario está escribiendo ahora mismo.
    pub input: String,
    /// Comando original con placeholders sin reemplazar.
    pub original_cmd: String,
}

impl WizardState {
    pub fn new(params: Vec<CommandParam>, cmd: String) -> Self {
        let len = params.len();
        WizardState {
            params,
            current: 0,
            values: vec![String::new(); len],
            input: String::new(),
            original_cmd: cmd,
        }
    }

    /// Parámetro que se está pidiendo ahora.
    pub fn current_param(&self) -> &CommandParam {
        &self.params[self.current]
    }

    /// Confirma el campo actual y avanza. Retorna `true` si era el último.
    pub fn confirm_current(&mut self) -> bool {
        self.values[self.current] = self.input.clone();
        self.input.clear();
        if self.current + 1 >= self.params.len() {
            true // wizard completo
        } else {
            self.current += 1;
            false
        }
    }

    /// Construye el comando final reemplazando todos los placeholders.
    pub fn resolve(&self) -> String {
        let mut cmd = self.original_cmd.clone();
        for (param, value) in self.params.iter().zip(self.values.iter()) {
            // replace() reemplaza TODAS las ocurrencias del placeholder
            cmd = cmd.replace(&param.placeholder, value);
        }
        cmd
    }

}