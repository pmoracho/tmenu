use ratatui::{
    Frame, Terminal, backend::{Backend, CrosstermBackend}, layout::Rect, style::{Color, Modifier, Style}, text::Line, widgets::{Block, Borders, List, ListItem, ListState}
};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
#[cfg(not(target_os = "windows"))]
use std::process::Command;
use std::{error::Error, fs, io};
use clap::Parser; // Importamos el trait Parser
use std::path::PathBuf;

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
    Execute(String),      // Comando a ejecutar
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
}

impl App {
    // Este método devuelve el título actual y la referencia a los items
    fn current_data(&self) -> (&String, &Vec<MenuItem>) {
        (&self.current_title, &self.current_items)
    }
    fn from_toon(path: &str) -> Result<Self, Box<dyn Error>> {
        let content = fs::read_to_string(path)?;
        let mut root_items: Vec<MenuItem> = Vec::new();
        let mut current_submenu: Option<(String, Vec<MenuItem>)> = None;
        let mut main_title = String::from("Menu Principal"); // Valor por defecto
        let mut first_key_found = false;

        for line in content.lines() {
            if line.trim().is_empty() { continue; }
            
            let indent = line.len() - line.trim_start().len();
            let trimmed = line.trim();

            if trimmed.ends_with(':') && indent == 0 {
                // Capturamos la primera clave global como título del menú
                if !first_key_found {
                    main_title = trimmed.trim_matches(':').trim_matches('"').to_string();
                    first_key_found = true;
                }
            } else if trimmed.ends_with(':') && indent > 0 {
                // Inicio de un submenú
                let name = trimmed.trim_matches(':').trim_matches('"').to_string();
                current_submenu = Some((name, Vec::new()));
            } else if trimmed.contains('[') {
                // Es un item: "Nombre"[2]: comando...
                let parts: Vec<&str> = trimmed.split("]:").collect();
                let label = parts[0].split('[').next().unwrap().trim_matches('"').to_string();
                let action_str = parts.get(1).unwrap_or(&"").trim().to_string();
                
                let item = MenuItem {
                    label,
                    action: MenuAction::Execute(action_str),
                };

                if indent > 2 { 
                    if let Some(ref mut sub) = current_submenu {
                        sub.1.push(item);
                    }
                } else {
                    root_items.push(item);
                }
            }
        }

        // Agregar el último submenú procesado a los items raíz
        if let Some((name, sub_items)) = current_submenu {
            root_items.push(MenuItem {
                label: name,
                action: MenuAction::OpenSubmenu(sub_items),
            });
        }

        let mut state = ListState::default();
        state.select(Some(0));

        Ok(App {
            history: Vec::new(),
            current_title: main_title, // Usamos el título capturado
            current_items: root_items,
            state,
        })
    }

    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => if i >= self.current_items.len() - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => if i == 0 { self.current_items.len() - 1 } else { i - 1 },
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
                    self.history.push((self.current_title.clone(), self.current_items.clone(), old_state));
                    
                    self.current_title = item.label.clone();
                    self.current_items = sub_items.clone();
                    self.state = ListState::default();
                    self.state.select(Some(0));
                }
            }
        }
        false
    }

    fn execute_external_command<B: Backend>(&self, terminal: &mut Terminal<B>, cmd: &str) {
        // Restaurar terminal
        let _ = disable_raw_mode();
        execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture).unwrap();

        // Ejecutar comando
        #[cfg(target_os = "windows")]
        let mut child = Command::new("cmd").args(["/C", cmd]).spawn().expect("Fallo");
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
}


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
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal, &mut app);

    // Restaurar terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    if let Err(err) = res { println!("Error: {:?}", err) }
    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<(), Box<dyn Error>>
where B::Error: Error + 'static {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == event::KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Down => app.next(),
                    KeyCode::Up => app.previous(),
                    KeyCode::Enter | KeyCode::Right => {
                        if app.enter(terminal) { return Ok(()); }
                    }
                    KeyCode::Left | KeyCode::Esc => app.back(),
                    _ => {}
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {

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
        (items_to_show.len() + 4) as u16, 
        f.area()
    );

    // 4. Creamos los ListItems con el nuevo estilo
    let items: Vec<ListItem> = items_to_show
        .iter()
        .map(|i| {
            let symbol = match i.action {
                MenuAction::OpenSubmenu(_) => " ", // O ">" si no tienes NerdFonts
                _ => "",
            };
            // Agregamos un poco de espacio a la izquierda del texto
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
                .title_bottom(Line::from("[q] Salir | [←] Volver").right_aligned())                
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .border_style(Style::default().fg(border_color))
                .padding(ratatui::widgets::Padding::new(0, 0, 1, 1)) // Padding interno
        )
        .highlight_style(
            Style::default()
                .bg(Color::Indexed(24)) // Azul profundo
                .fg(Color::Yellow)      // Texto resaltado
                .add_modifier(Modifier::BOLD)
        )
        .highlight_symbol(" ➔ ");

    // Renderizado final
    f.render_stateful_widget(list, area, &mut app.state);
}

fn auto_size_rect(width: u16, height: u16, r: Rect) -> Rect {
    let w = width.min(r.width);
    let h = height.min(r.height);
    Rect::new((r.width - w) / 2, (r.height - h) / 2, w, h)
}


