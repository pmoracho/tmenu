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
use crate::search::{filter_recursive};
use crate::{error::AppError, history, parser};

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
    pub error_message: Option<String>,
    pub quit_requested: bool,
}

/// Implementación de métodos para `App`.
impl App {
    /// Crea una instancia de `App` cargando el menú desde un archivo `.toon`.
    pub fn from_toon(path: &std::path::Path, debug: bool) -> Result<Self, AppError> {

        let (config, main_title, root_items, warnings) = parse_toon_file(path)?;

        // Se muestra antes de entrar al modo raw / pantalla alternativa.
        if !warnings.is_empty() {
            eprintln!("{}:", path.display());
            for w in &warnings {
                eprintln!("  [warn] {}", w);
            }
            eprintln!("\n{}", t!("prompts.press_enter_to_continue"));
            let mut _buf = String::new();
            let _ = io::stdin().read_line(&mut _buf);
        }

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
            error_message: None,
            quit_requested: false,
        })
    }

    /// Convierte `ForbiddenCommand` en un mensaje de UI en vez de abortar la app.
    /// Cualquier otro error se propaga igual.
    fn absorb_forbidden<T: Default>(&mut self, r: Result<T, AppError>) -> Result<T, AppError> {
        match r {
            Err(AppError::ForbiddenCommand(c)) => {
                self.error_message =
                    Some(t!("errors.forbidden_command", command = c).to_string());
                Ok(T::default())
            }
            other => other,
        }
    }

    /// Devuelve los ítems filtrados según el texto de búsqueda actual.
    /// Si no hay búsqueda activa, retorna todos los ítems del nivel actual.
    pub fn filtered_items(&self) -> Vec<MenuItem> {
        if !self.search_mode || self.search_text.is_empty() {
            return self.current_items.clone();
        }

        filter_recursive(&self.current_items, &self.search_text, 0)
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
                let cmd = cmd_str.trim();
                if cmd == "exit" {
                    return Ok(true);
                }

                let params = parser::extract_params(cmd);
                if params.is_empty() {
                    // Sin interpolación: pedir confirmación solo si el ítem lo requiere
                    if item.require_confirmation {
                        let r = self.request_command_confirmation(terminal, cmd, item.require_shell);
                        return self.absorb_forbidden(r);
                    } else {
                        // Ejecutar directo sin confirmación
                        let r = self.execute_external_command(terminal, cmd, item.require_shell);
                        self.absorb_forbidden(r)?;
                    }
                } else {
                    // Con interpolación: iniciar wizard (no ejecutar todavía)
                    self.wizard = Some(WizardState::new(
                        params,
                        cmd.to_string(),
                        item.require_confirmation,
                        item.require_shell
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
        use_shell: bool,
    ) -> Result<bool, AppError> {
        // Validar seguridad ANTES de mostrar cualquier modal
        if !Self::is_safe_command(cmd, use_shell) {
            return Err(AppError::ForbiddenCommand(cmd.to_string()));
        }

        // Mostrar modal de confirmación
        self.confirmation = Some(ConfirmationState::new(cmd.to_string()));

        // Ejecutar el modal bloqueante — devuelve true si se ejecutó, false si se canceló
        let should_execute = crate::run_confirmation_modal(terminal, self)?;

        if should_execute {
            self.execute_external_command(terminal, cmd, use_shell)?;
        }

        Ok(self.quit_requested)
    }

    /// Ejecuta un comando externo en el shell del sistema operativo.
    ///
    /// Si `execution_mode` es `Clean`, ejecuta `clear` antes del comando.
    /// Antes de ejecutar, restaura la terminal a modo normal y la reconfigura
    /// en modo TUI al finalizar.
    ///
    /// # Seguridad
    /// - Rechaza comandos con path traversal (`..`).
    /// - Si `use_shell` es `false` (default), usa `shlex::split` para respetar
    ///   quoting correctamente y spawnea el binario directo, sin intérprete de
    ///   shell de por medio — `&&`, `|`, `;`, etc. se tratan como argumentos
    ///   literales, no como sintaxis de shell.
    /// - Si `use_shell` es `true` (flag `[shell=true]` en el `.toon`), el
    ///   comando se pasa completo a `sh -c` (Unix) o `cmd /C` (Windows),
    ///   habilitando `&&`, `|`, redirecciones, etc. Usar con cuidado: el
    ///   comando se interpreta con reglas de shell.
    /// - Los valores interpolados por el wizard se validan aquí también,
    ///   ya que `finish_wizard` llama a este método con el comando resuelto.
    pub fn execute_external_command(
        &self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
        cmd: &str,
        use_shell: bool,
    ) -> Result<(), AppError> {
        if !Self::is_safe_command(cmd, use_shell) {
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

        // Construir el comando según el modo: shell interpretado o spawn directo.
        let command = if use_shell {
            #[cfg(windows)]
            let built = {
                let mut c = Command::new("cmd");
                c.args(["/C", cmd]);
                c
            };

            #[cfg(not(windows))]
            let built = {
                let mut c = Command::new("sh");
                c.args(["-c", cmd]);
                c
            };

            Some(built)
        } else {
            // Parsear respetando quoting ("arg con espacios" se trata como un solo arg).
            let parts: Vec<String> = shlex::split(cmd)
                .unwrap_or_else(|| cmd.split_whitespace().map(str::to_string).collect());

            parts.split_first().map(|(bin, args)| {
                let mut c = Command::new(bin);
                c.args(args);
                c
            })
        };

        if let Some(mut command) = command {
            match command.spawn() {
                Ok(mut child) => {
                    let ret = child.wait();
                    // Registrar en historial solo si la ejecución fue exitosa
                    if let Err(e) = history::log_command(cmd) && ret.is_ok() {
                        eprintln!("{} {}", t!("errors.history_save"), e);
                    }
                }
                Err(e) => eprintln!("{}: {}", t!("errors.execution_failed"), e),
            }
        }

        println!("\n{}", t!("prompts.press_any_key"));

        // Habilitar raw mode para detectar cualquier tecla sin buffering
        if let Err(e) = enable_raw_mode() {
            eprintln!("{} {}", t!("errors.raw_mode_enable"), e);
        }
        let _ = wait_for_any_key();
        // Deshabilitar raw mode antes de restaurar la pantalla alternativa
        let _ = disable_raw_mode();

        // Volver a modo TUI
        if let Err(e) = enable_raw_mode() {
            eprintln!("{} {}", t!("errors.raw_mode_enable"), e);
        }

        let is_tty_native = std::env::var("TERM").unwrap_or_default() == "linux";
        if !is_tty_native {
            if let Err(e) = execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture) {
                eprintln!("{} {}", t!("errors.alt_screen_restore"), e);
            }
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
            let cmd = wizard.resolve(wizard.require_shell);
            let require_confirmation = wizard.require_confirmation;
            let require_shell = wizard.require_shell;
            self.wizard = None;

            if require_confirmation {
                // Pedir confirmación antes de ejecutar el comando resuelto
                let r = self.request_command_confirmation(terminal, &cmd, require_shell);
                return self.absorb_forbidden(r);
            } else {
                // Ejecutar directo sin confirmación
                let r = self.execute_external_command(terminal, &cmd, require_shell);
                self.absorb_forbidden(r)?;
            }
        }
        Ok(false)
    }

    /// Valida que el comando no contenga path traversal ni caracteres de shell peligrosos.
    ///
    /// Nota: no se usan pipes/shell, así que `|`, `&`, `;` no son vectores de inyección
    /// en este contexto — pero `..` sí puede usarse para path traversal en argumentos.
    fn is_safe_command(cmd: &str, use_shell: bool) -> bool {

        // Rechazar path traversal explícito
        if cmd.split_whitespace().any(|part| part.contains("..")) {
            return false;
        }
        if use_shell {
            return true; // Si se permite shell, no filtramos metacaracteres
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
/// En la línea de comandos, los placeholders se reemplazan por los valores ingresados por el usuario.
pub struct WizardState {
    pub params: Vec<CommandParam>,
    pub current: usize,
    pub values: Vec<String>,
    pub input: String,
    pub original_cmd: String,
    pub require_confirmation: bool,
    pub require_shell: bool,
}

/// Implementación de métodos para `WizardState`.
impl WizardState {
    pub fn new(params: Vec<CommandParam>, cmd: String, require_confirmation: bool, require_shell: bool) -> Self {
        let len = params.len();
        WizardState {
            params,
            current: 0,
            values: vec![String::new(); len],
            input: String::new(),
            original_cmd: cmd,
            require_confirmation,
            require_shell,
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

    /// Construye el comando final reemplazando los placeholders por los valores ingresados.
    /// Si `use_shell` es true, los valores se escapan con `shlex::quote` para que puedan contener espacios y caracteres especiales.
    /// Si `use_shell` es false, los valores se insertan tal cual, respetando el quoting original del comando.
    pub fn resolve(&self, use_shell: bool) -> String {
        let mut cmd = self.original_cmd.clone();
        for (param, value) in self.params.iter().zip(self.values.iter()) {
            let substituted = if use_shell {
                shlex::try_quote(value)
                    .map(|quoted| quoted.into_owned())
                    .unwrap_or_else(|_| value.clone())
            } else {
                value.clone()
            };
            cmd = cmd.replace(&param.placeholder, &substituted);
        }
        cmd
    }
}

/// Espera a que el usuario presione cualquier tecla usando crossterm.
/// Ignora eventos que no sean de teclado y retorna cuando detecta una tecla.
fn wait_for_any_key() -> Result<(), std::io::Error> {
    use crossterm::event::{self, KeyEventKind};

    loop {
        if let Ok(event) = event::read() {
            if let crossterm::event::Event::Key(key_event) = event {
                // Solo procesar eventos de tecla presionada (ignorar repetición)
                if key_event.kind == KeyEventKind::Press {
                    return Ok(());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::CommandParam;

    /// Crea un WizardState con un solo parámetro ya "confirmado",
    /// simulando que el usuario ya pasó por el wizard y tipeó `value`.
    fn wizard_with_value(cmd: &str, value: &str, require_shell: bool) -> WizardState {
        let params = vec![CommandParam {
            label: "patrón".to_string(),
            placeholder: "{{text: patrón}}".to_string(),
        }];
        let mut wizard = WizardState::new(params, cmd.to_string(), false, require_shell);
        wizard.values[0] = value.to_string();
        wizard
    }

    #[test]
    fn test_resolve_no_shell_plain_value() {
        let wizard = wizard_with_value("grep {{text: patrón}} file.log", "error", false);
        assert_eq!(wizard.resolve(false), "grep error file.log");
    }

    #[test]
    fn test_resolve_no_shell_does_not_quote() {
        // En modo no-shell no se escapa: el comando entero pasa luego por
        // shlex::split en execute_external_command, así que acá el valor
        // se sustituye literal.
        let wizard = wizard_with_value("grep {{text: patrón}} file.log", "foo bar", false);
        assert_eq!(wizard.resolve(false), "grep foo bar file.log");
    }

    #[test]
    fn test_resolve_shell_quotes_simple_value() {
        let wizard = wizard_with_value("grep {{text: patrón}} file.log", "error", true);
        let resolved = wizard.resolve(true);
        let tokens = shlex::split(&resolved).expect("comando debe ser válido para shlex");
        assert_eq!(tokens, vec!["grep", "error", "file.log"]);
    }

    #[test]
    fn test_resolve_shell_quotes_value_with_spaces() {
        let wizard = wizard_with_value("grep {{text: patrón}} file.log", "foo bar", true);
        let resolved = wizard.resolve(true);
        let tokens = shlex::split(&resolved).expect("comando debe ser válido para shlex");
        // "foo bar" debe quedar como UN solo token, no dos
        assert_eq!(tokens, vec!["grep", "foo bar", "file.log"]);
    }

    #[test]
    fn test_resolve_shell_neutralizes_semicolon_injection() {
        let wizard = wizard_with_value(
            "grep {{text: patrón}} logs/*.log",
            "foo; rm -rf ~",
            true,
        );
        let resolved = wizard.resolve(true);
        let tokens = shlex::split(&resolved).expect("comando debe ser válido para shlex");
        // El ';' debe quedar DENTRO del token, no actuar como separador de shell
        assert_eq!(tokens, vec!["grep", "foo; rm -rf ~", "logs/*.log"]);
    }

    #[test]
    fn test_resolve_shell_neutralizes_command_substitution() {
        let wizard = wizard_with_value("echo {{text: patrón}}", "$(whoami)", true);
        let resolved = wizard.resolve(true);
        let tokens = shlex::split(&resolved).expect("comando debe ser válido para shlex");
        // El $(...) debe quedar como texto literal en un único token
        assert_eq!(tokens, vec!["echo", "$(whoami)"]);
    }

    #[test]
    fn test_resolve_shell_value_with_single_quote_is_safely_escaped() {
        let wizard = wizard_with_value("echo {{text: patrón}}", "it's here", true);
        let resolved = wizard.resolve(true);
        let tokens = shlex::split(&resolved).expect("comando debe ser válido para shlex");
        // El valor con apóstrofe debe sobrevivir intacto al ida-y-vuelta de escape
        assert_eq!(tokens, vec!["echo", "it's here"]);
    }

    #[test]
    fn test_resolve_multiple_placeholders_same_param() {
        // Si el mismo placeholder aparece dos veces en el comando original,
        // ambas ocurrencias deben resolverse (comportamiento de String::replace).
        let params = vec![CommandParam {
            label: "Etiqueta".to_string(),
            placeholder: "{{text: Etiqueta}}".to_string(),
        }];
        let mut wizard = WizardState::new(
            params,
            "git tag -a {{text: Etiqueta}} -m \"Release {{text: Etiqueta}}\"".to_string(),
            false,
            false,
        );
        wizard.values[0] = "v1.0.0".to_string();

        assert_eq!(
            wizard.resolve(false),
            "git tag -a v1.0.0 -m \"Release v1.0.0\""
        );
    }

    fn app_with_search(text: &str) -> App {
        let items = vec![MenuItem {
            label: "Ver estado".into(),
            action: MenuAction::Execute("git status".into()),
            require_confirmation: false,
            require_shell: false,
        }];
        let mut state = ListState::default();
        state.select(Some(0));
        App {
            history: Vec::new(),
            current_title: "t".into(),
            current_items: items.clone(),
            root_title: "t".into(),
            root_items: items,
            state,
            search_text: text.into(),
            search_mode: true,
            show_preview: false,
            show_help: false,
            debug: false,
            wizard: None,
            confirmation: None,
            execution_mode: ExecutionMode::Inherit,
            error_message: None,
            quit_requested: false,
        }
    }

    #[test]
    fn test_search_sin_resultados_devuelve_lista_vacia() {
        let app = app_with_search("zzz");
        assert!(app.filtered_items().is_empty());
    }

    #[test]
    fn test_search_con_resultado() {
        let app = app_with_search("est");
        assert_eq!(app.filtered_items().len(), 1);
    }

    #[test]
    fn test_forbidden_command_no_propaga_error() {
        let mut app = app_with_search("");
        let r: Result<(), AppError> = Err(AppError::ForbiddenCommand("a;b".into()));
        assert!(app.absorb_forbidden(r).is_ok());
        assert!(app.error_message.is_some());
    }

}