/// Lector de menús interactivos TUI en Rust utilizando Ratatui y Clap
/// Este programa carga un menú desde un falso archivo .toon
/// El formato del archivo es simple, con títulos, comandos y submenús definidos por indentación.
/// El programa permite navegar por el menú, ejecutar comandos y volver atrás en submenús.
/// Ejemplo de formato .toon:
/// Menu Principal:
///     "Item 1": "echo 'Comando 1 ejecutado'"
///     "Item 2": "echo 'Comando 2 ejecutado'"
///     Submenú:
///         "Subitem 1": "echo 'Subcomando 1 ejecutado'"
///         "Subitem 2": "echo 'Subcomando 2 ejecutado'"
///     "Salir": "exit"
use ratatui::{
    Frame, Terminal,
    backend::{Backend, CrosstermBackend},
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState},
};

use clap::Parser; // Importamos el trait Parser
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use std::path::PathBuf;
#[cfg(not(target_os = "windows"))]
use std::process::Command;
use std::{error::Error, fs, io};

#[derive(Parser, Debug)]
#[command(author = "Patricio Moracho", version = "1.0", about = "Lector de menús interactivos TUI", long_about = None)]
struct Args {
    /// Ruta al archivo de menú (.toon)
    #[arg(value_name = "ARCHIVO", default_value = "tmenu.toon")]
    menu_file: PathBuf,

    /// Activa el modo depuración (ejemplo de flag opcional)
    #[arg(short, long)]
    debug: bool,
}

#[derive(Clone)]
enum MenuAction {
    Execute(String),            // Comando a ejecutar
    OpenSubmenu(Vec<MenuItem>), // Lista de items del submenú
}

#[derive(Clone)]
struct MenuItem {
    label: String,
    action: MenuAction,
}

struct App {
    history: Vec<(String, Vec<MenuItem>, ListState)>, // Para volver atrás (título, items, estado)
    current_title: String,
    current_items: Vec<MenuItem>,
    state: ListState,
    search_text: String,
    search_mode: bool,
}

/// Implementación de la lógica principal de la aplicación, incluyendo la carga del menú desde un archivo,
/// navegación entre items, ejecución de comandos y manejo de submenús. Esta implementación se encarga
/// de mantener el estado actual del menú, gestionar el historial para permitir volver atrás, y ejecutar
/// comandos externos de manera segura, restaurando la terminal antes de la ejecución y reconfigurándola
/// después de la ejecución para asegurar una experiencia de usuario fluida y sin interrupciones. Además
/// incluye la lógica para parsear el formato específico del archivo .toon, permitiendo una estructura de
/// menú jerárquica con submenús y comandos asociados a cada item.
impl App {
    // Este método devuelve el título actual y la referencia a los items
    fn current_data(&self) -> (&String, &Vec<MenuItem>) {
        (&self.current_title, &self.current_items)
    }

    fn from_toon(path: &str) -> Result<Self, Box<dyn Error>> {
        let content = fs::read_to_string(path)?;
        let mut main_title = String::from("Menu Principal");

        // Usaremos una pila para manejar los niveles de submenús
        // (Nombre, Items acumulados, Nivel de identación)
        let mut stack: Vec<(String, Vec<MenuItem>, usize)> = Vec::new();
        let mut root_items: Vec<MenuItem> = Vec::new();

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }

            let indent = line.len() - line.trim_start().len();
            let trimmed = line.trim();

            // 1. Caso Título Principal (Indent 0)
            if indent == 0 && trimmed.ends_with(':') {
                main_title = trimmed
                    .trim_end_matches(':')
                    .trim_matches('"')
                    .trim()
                    .to_string();
                continue;
            }

            // 2. Separar Clave y Valor
            if let Some(pos) = trimmed.find(':') {
                let key = trimmed[..pos].trim_matches('"').trim().to_string();
                let value = trimmed[pos + 1..].trim();

                if value.is_empty() {
                    // --- ES UN SUBMENÚ ---
                    // Si hay algo en la pila con igual o mayor identación, hay que cerrarlo
                    while !stack.is_empty() && stack.last().unwrap().2 >= indent {
                        Self::pop_and_insert(&mut stack, &mut root_items);
                    }
                    stack.push((key, Vec::new(), indent));
                } else {
                    // --- ES UN COMANDO ---
                    let item = MenuItem {
                        label: key,
                        action: MenuAction::Execute(value.trim_matches('"').to_string()),
                    };

                    // Si la pila no está vacía, este item pertenece al último submenú abierto
                    if let Some(last) = stack.last_mut() {
                        last.1.push(item);
                    } else {
                        root_items.push(item);
                    }
                }
            }
        }

        // Vaciar la pila al terminar el archivo
        while !stack.is_empty() {
            Self::pop_and_insert(&mut stack, &mut root_items);
        }

        let mut state = ListState::default();
        state.select(Some(0));

        Ok(App {
            history: Vec::new(),
            current_title: main_title,
            current_items: root_items,
            state,
            search_text: String::new(),
            search_mode: false,
        })
    }

    // Función auxiliar para mover items de la pila al nivel superior
    fn pop_and_insert(stack: &mut Vec<(String, Vec<MenuItem>, usize)>, root: &mut Vec<MenuItem>) {
        if let Some((name, items, _)) = stack.pop() {
            let submenu = MenuItem {
                label: name,
                action: MenuAction::OpenSubmenu(items),
            };
            if let Some(parent) = stack.last_mut() {
                parent.1.push(submenu);
            } else {
                root.push(submenu);
            }
        }
    }

    /// Función para volver al menú raíz, limpiando el historial y restaurando el estado inicial del menú.
    fn go_home(&mut self) {
        if !self.history.is_empty() {
            // El primer elemento del historial es el estado del menú raíz
            let (root_title, root_items, root_state) = self.history.remove(0);

            // Limpiamos el resto del historial
            self.history.clear();

            // Restauramos los valores raíz
            self.current_title = root_title;
            self.current_items = root_items;
            self.state = root_state;
        }
    }
    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.current_items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.current_items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn enter<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> bool {
        if let Some(index) = self.state.selected() {
            let item = &self.current_items[index];
            match &item.action {
                MenuAction::Execute(cmd_str) => {
                    let clean_cmd = cmd_str.trim().trim_matches('"');

                    if clean_cmd == "exit" {
                        return true;
                    }

                    // Ejecución del comando
                    self.execute_external_command(terminal, clean_cmd);
                }

                MenuAction::OpenSubmenu(sub_items) => {
                    let old_state = self.state.clone();
                    self.history.push((
                        self.current_title.clone(),
                        self.current_items.clone(),
                        old_state,
                    ));

                    self.current_title = item.label.clone();
                    self.current_items = sub_items.clone();
                    self.state = ListState::default();
                    self.state.select(Some(0));
                }
            }
        }
        false
    }

    /// Ejecuta un comando externo, restaurando la terminal antes de la ejecución y reconfigurándola
    /// después de la ejecución para asegurar una experiencia de usuario fluida y sin interrupciones.
    /// - `terminal`: Referencia mutable a la terminal de Ratatui, utilizada para redibujar la
    ///   interfaz después de ejecutar el comando.
    /// - `cmd`: Comando a ejecutar, que se espera sea una cadena de texto
    ///   devuelve un booleano indicando si el comando ejecutado fue "exit", lo que indicaría que
    ///   la aplicación debe cerrar.
    fn execute_external_command<B: Backend>(&self, terminal: &mut Terminal<B>, cmd: &str) {
        // Restaurar terminal
        let _ = disable_raw_mode();
        execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture).unwrap();

        // Ejecutar comando
        #[cfg(target_os = "windows")]
        let mut child = Command::new("cmd")
            .args(["/C", cmd])
            .spawn()
            .expect("Fallo");
        #[cfg(not(target_os = "windows"))]
        let mut child = Command::new("sh").args(["-c", cmd]).spawn().expect("Fallo");

        let _ = child.wait();

        println!("\nPresiona Enter para volver...");
        let _ = io::stdin().read_line(&mut std::string::String::new());

        // 2. REGRESO A RATATUI
        let _ = enable_raw_mode();
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture).unwrap();

        // 3. LA CLAVE: Forzar limpieza total y redibujado
        terminal.clear().unwrap();
    }

    fn back(&mut self) {
        if let Some((title, items, state)) = self.history.pop() {
            self.current_title = title;
            self.current_items = items;
            self.state = state;
        }
    }

    fn filter_recursive(items: &[MenuItem], query: &str) -> Vec<MenuItem> {
        let mut results = Vec::new();
        for item in items {
            match &item.action {
                MenuAction::Execute(_) => {
                    if is_fuzzy_match(&item.label, query) {
                        results.push(item.clone());
                    }
                }
                MenuAction::OpenSubmenu(sub_items) => {
                    results.append(&mut Self::filter_recursive(sub_items, query));
                }
            }
        }
        results
    }

    // Busca el primer comando disponible (para el caso de "lista vacía")
    fn find_first_command(items: &[MenuItem]) -> Option<MenuItem> {
        for item in items {
            match &item.action {
                MenuAction::Execute(_) => return Some(item.clone()),
                MenuAction::OpenSubmenu(sub_items) => {
                    if let Some(found) = Self::find_first_command(sub_items) {
                        return Some(found);
                    }
                }
            }
        }
        None
    }

    fn filtered_items(&self) -> Vec<MenuItem> {
        if !self.search_mode || self.search_text.is_empty() {
            return self.current_items.clone();
        }

        let mut results = Self::filter_recursive(&self.current_items, &self.search_text);

        if results.is_empty() {
            // Si no hay match, buscamos el primer comando que exista en el menú actual
            if let Some(fallback) = Self::find_first_command(&self.current_items) {
                results.push(fallback);
            }
        }
        results
    }

    fn enter_with_list<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
        list: &[MenuItem],
    ) -> bool {
        if let Some(index) = self.state.selected() {
            if let Some(item) = list.get(index) {
                match &item.action {
                    MenuAction::Execute(cmd_str) => {
                        let clean_cmd = cmd_str.trim().trim_matches('"');
                        if clean_cmd == "exit" {
                            return true;
                        }
                        self.execute_external_command(terminal, clean_cmd);
                    }
                    MenuAction::OpenSubmenu(sub_items) => {
                        // Si el filtro devolvió un submenú, entramos en él
                        self.search_text.clear();
                        self.search_mode = false;
                        self.history.push((
                            self.current_title.clone(),
                            self.current_items.clone(),
                            self.state.clone(),
                        ));
                        self.current_title = item.label.clone();
                        self.current_items = sub_items.clone();
                        self.state = ListState::default();
                        self.state.select(Some(0));
                    }
                }
            }
        }
        false
    }
}

/// Función principal que inicializa la aplicación, configura la terminal y maneja el ciclo de eventos.
/// - Se encarga de parsear los argumentos de línea de comandos utilizando Clap, cargar el
/// archivo de menú especificado, configurar la terminal en modo raw y alternativo, y luego iniciar el ciclo de eventos que maneja la interacción del usuario. Al finalizar, restaura la terminal a su estado original. Devuelve un Result para manejar posibles errores durante la inicialización o ejecución de la aplicación.
/// Nota: Es importante manejar los errores de manera adecuada, especialmente al cargar el archivo de menú, para proporcionar una experiencia de usuario clara y evitar que la aplicación falle sin explicación. Además, la configuración y restauración de la terminal es crucial para asegurar que el entorno del usuario no quede en un estado inconsistente después de usar la aplicación.
/// Importante: Esta función es el punto de entrada de la aplicación y coordina la configuración inicial, la carga de datos y el ciclo principal de eventos, por lo que su correcta implementación es esencial para el funcionamiento general de la aplicación.
fn main() -> Result<(), Box<dyn Error>> {
    // 1. Parsear argumentos con Clap
    let args = Args::parse();

    // 2. Convertir PathBuf a string para pasarlo a from_toon
    let filename = args.menu_file.to_str().unwrap_or("menu.toon");

    // Si no existe el archivo, dar un error claro y salir
    if !std::path::Path::new(filename).exists() {
        eprintln!("Error: El archivo de menú '{}' no existe.", filename);
        std::process::exit(1);
    }

    // Intentar cargar el archivo antes de entrar en modo terminal
    let mut app = App::from_toon(filename)?;

    // Configuración de la terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, crossterm::cursor::SetCursorStyle::SteadyUnderScore)?;
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let res = run_app(&mut terminal, &mut app);

    // Restaurar terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error: {:?}", err)
    }
    Ok(())
}

/// Función principal del ciclo de eventos que maneja la interacción del usuario y el renderizado de la interfaz.
/// - `terminal`: Referencia mutable a la terminal de Ratatui, que se utiliza para dibujar la interfaz de usuario en cada iteración del ciclo.
/// - `app`: Referencia mutable a la instancia de App que contiene el estado actual del menú, incluyendo el título, los items
/// y el historial de navegación. Esta función se encarga de:
///   1. Dibujar la interfaz de usuario llamando a la función `ui` en cada iteración del ciclo.
///   2. Leer eventos de teclado utilizando Crossterm y responder a las teclas presionadas para navegar por el menú, entrar en submenús,
/// volver atrás o salir de la aplicación. El ciclo continúa hasta que el usuario decide salir (presionando 'q') o ejecuta un comando que indica salir.
///   3. Manejar errores de eventos y renderizado, devolviendo un error si ocurre algún problema durante la ejecución.
/// Nota: El tipo de error específico para eventos de teclado se maneja con un bound en la firma de la función, asegurando que cualquier error relacionado con eventos sea compatible con el tipo de error esperado por Ratatui.
/// Importante: Esta función es el núcleo de la aplicación, ya que coordina la interacción del usuario y el renderizado dinámico de la interfaz basada en el estado actual del menú.
fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<(), Box<dyn Error>>
where
    B::Error: Error + 'static,
{
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == event::KeyEventKind::Press {
                if key.modifiers.contains(event::KeyModifiers::CONTROL)
                    && key.code == KeyCode::Char('q')
                {
                    return Ok(()); // Salir de la aplicación
                }
                if app.search_mode {
                    // --- MODO EDICIÓN ACTIVO ---
                    match key.code {
                        KeyCode::Tab => {
                            app.search_mode = false; // Salir del modo edición
                            app.search_text.clear();
                        }
                        KeyCode::Backspace => {
                            app.search_text.pop();
                        }
                        KeyCode::Enter => {
                            let filtered = app.filtered_items();
                            if !filtered.is_empty() {
                                // Seleccionamos el primero y ejecutamos
                                app.state.select(Some(0));
                                if app.enter_with_list(terminal, &filtered) {
                                    return Ok(());
                                }
                            }
                        }
                        KeyCode::Char(c) => {
                            app.search_text.push(c);
                        }
                        _ => {}
                    }
                } else {
                    // --- MODO NAVEGACIÓN (Normal) ---
                    match key.code {
                        KeyCode::Tab => {
                            app.search_mode = true; // Activar con tecla '/' o 's'
                        }
                        KeyCode::Down => app.next(),
                        KeyCode::Up => app.previous(),
                        KeyCode::Home => app.go_home(),
                        KeyCode::Enter | KeyCode::Right => {
                            if app.enter(terminal) {
                                return Ok(());
                            }
                        }
                        KeyCode::Left | KeyCode::Esc => app.back(),
                        _ => {}
                    }
                }
            }
        }
    }
}

/// Función de renderizado principal que dibuja la interfaz de usuario en cada ciclo de renderizado.
/// - `f`: Referencia mutable al Frame proporcionado por Ratatui para dibujar los widgets.
/// - `app`: Referencia mutable a la instancia de App que contiene el estado actual del
/// menú, incluyendo el título, los items y el estado de selección. Esta función se encarga de:
///   1. Dibujar un fondo opcional para mejorar la estética.
///   2. Obtener el título y los items actuales del menú desde la instancia de App.
///   3. Calcular el ancho máximo necesario para mostrar los items y el título sin recortar texto.
///   4. Crear un área centrada para el menú, ajustando su tamaño según el contenido y el tamaño de la terminal.
///   5. Construir los widgets de lista
///   6. Renderizar el widget de lista con estilos personalizados, incluyendo colores y símbolos para indicar submenús.
fn ui(f: &mut Frame, app: &mut App) {
    let items_to_render = app.filtered_items();
    let current_selected = app.state.selected().unwrap_or(0);
    if !items_to_render.is_empty() && current_selected >= items_to_render.len() {
        app.state.select(Some(0));
    }

    // Definir color basado en el modo
    let input_color = if app.search_mode {
        Color::Yellow
    } else {
        Color::DarkGray
    };
    let input_title = if app.search_mode { " Buscar ... " } else { "" };

    // Dibujar un fondo tenue (opcional)
    let background_block = Block::default().style(Style::default().bg(Color::Reset));
    f.render_widget(background_block, f.area());

    // 1. Obtenemos los datos actuales
    let (title, items_to_show) = app.current_data();

    // 2. Calculamos el ancho máximo (especificando el tipo para evitar el error E0282)
    let max_w = items_to_show
        .iter()
        .map(|item: &MenuItem| item.label.len()) // Especificamos &MenuItem
        .max()
        .unwrap_or(0)
        .max(title.len());

    // 3. Área centrada con espacio extra para el padding interno
    let area = auto_size_rect(
        (max_w + 14) as u16,
        (items_to_show.len() + 6) as u16,
        f.area(),
    );

    let chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Min(3),    // Lista (crece)
            ratatui::layout::Constraint::Length(3), // Input (fijo)
        ])
        .split(area);

    // 4. Creamos los ListItems con el nuevo estilo
    let items: Vec<ListItem> = items_to_render
        .iter()
        .map(|i| {
            let symbol = match i.action {
                MenuAction::OpenSubmenu(_) => " ",
                _ => "",
            };
            ListItem::new(format!(" {}{}", i.label, symbol))
        })
        .collect();

    // 5. Definimos el color del borde según el nivel (opcional pero muy cool)
    let border_color = if app.history.is_empty() {
        Color::Cyan // Menú Principal
    } else {
        Color::Magenta // Submenú
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(format!(" {} ", title))
                .title_alignment(ratatui::layout::Alignment::Center)
                .title_bottom(Line::from("[Ctrl+q] Salir").right_aligned())
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .border_style(Style::default().fg(border_color))
                .padding(ratatui::widgets::Padding::new(0, 0, 1, 1)), // Padding interno
        )
        .highlight_style(
            Style::default()
                .bg(Color::Indexed(24)) // Azul profundo
                .fg(Color::Yellow) // Texto resaltado
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" ➔ ");

    // Renderizado final
    f.render_stateful_widget(list, chunks[0], &mut app.state);

    // 2. RENDERIZAR INPUT (en chunks[1])
    if app.search_mode {
        let input_panel = ratatui::widgets::Paragraph::new(app.search_text.as_str()).block(
            Block::default()
                .title(input_title)
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .border_style(Style::default().fg(input_color)), // Color dinámico
        );
        f.set_cursor_position((
            chunks[1].x + app.search_text.len() as u16 + 1,
            chunks[1].y + 1,
        ));
        f.render_widget(input_panel, chunks[1]);
    }
}

/// Calcula un Rect centrado con un tamaño máximo dado por width y height,
/// pero sin exceder el tamaño del rectángulo original (r).
/// Si el tamaño calculado es menor que el del rectángulo original, se
/// centra dentro de él.
///
/// - `width`: Ancho máximo deseado para el nuevo rectángulo.
/// - `height`: Alto máximo deseado para el nuevo rectángulo.
/// - `r`: Rectángulo original que define el área total disponible.
///
/// Devuelve un nuevo Rect que es el resultado de aplicar las restricciones de tamaño y centrado.
fn auto_size_rect(width: u16, height: u16, r: Rect) -> Rect {
    let w = width.min(r.width);
    let h = height.min(r.height);
    Rect::new((r.width - w) / 2, (r.height - h) / 2, w, h)
}

/// Función de comparación difusa (fuzzy match) que verifica si el texto contiene los caracteres del query
/// en el mismo orden, pero no necesariamente de forma contigua. La comparación es insensible a mayúsculas y minúsculas.
/// - `text`: El texto completo que se va a comparar.
/// - `query`: La cadena de búsqueda que se desea encontrar dentro del texto.
///
/// Devuelve `true` si el texto contiene los caracteres del query en el mismo orden
///
fn is_fuzzy_match(text: &str, query: &str) -> bool {
    let mut it = text.chars();
    for c in query.chars() {
        match it.find(|&x| x.to_lowercase().next() == c.to_lowercase().next()) {
            Some(_) => (),
            None => return false,
        }
    }
    true
}
