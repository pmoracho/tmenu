use ratatui::{
    backend::{CrosstermBackend, Backend},
    widgets::{Block, Borders, List, ListItem, ListState}, // Añadimos Clear
    layout::{Rect},
    style::{Color, Modifier, Style},
    Terminal, Frame,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{error::Error, io};

enum Screen {
    Main,
    SubMenu,
}

struct App {
    screen: Screen,
    menu_state: ListState,
    items: Vec<String>,
    submenu_items: Vec<String>,
}

impl App {
    fn new() -> App {
        let mut state = ListState::default();
        state.select(Some(0));
        App {
            screen: Screen::Main,
            menu_state: state,
            items: vec![
                "1. Abrir Submenú (Flecha Der o Enter)".to_string(),
                "2. Opción Inerte".to_string(),
                "3. Salir".to_string(),
            ],
            submenu_items: vec![
                "<- Volver (Flecha Izq o Enter)".to_string(),
                "Opción Secreta".to_string()
            ],
        }
    }

    // Obtenemos la longitud de la lista actual para evitar errores de índice
    fn current_list_len(&self) -> usize {
        match self.screen {
            Screen::Main => self.items.len(),
            Screen::SubMenu => self.submenu_items.len(),
        }
    }

    pub fn next(&mut self) {
        let i = match self.menu_state.selected() {
            Some(i) => if i >= self.current_list_len() - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.menu_state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let i = match self.menu_state.selected() {
            Some(i) => if i == 0 { self.current_list_len() - 1 } else { i - 1 },
            None => 0,
        };
        self.menu_state.select(Some(i));
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    if let Err(err) = res { println!("{:?}", err) }
    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<(), Box<dyn Error>>
where
    B::Error: Error + 'static,
{
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == event::KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Down => app.next(),
                    KeyCode::Up => app.previous(),
                    // Flecha Derecha entra al submenú si estamos en la opción 0
                    KeyCode::Right | KeyCode::Enter if matches!(app.screen, Screen::Main) => {
                        match app.menu_state.selected() {
                            Some(0) => {
                                app.screen = Screen::SubMenu;
                                app.menu_state.select(Some(0));
                            }
                            Some(2) if key.code == KeyCode::Enter => return Ok(()),
                            _ => {}
                        }
                    }
                    // Flecha Izquierda vuelve al principal si estamos en el submenú
                    KeyCode::Left | KeyCode::Enter if matches!(app.screen, Screen::SubMenu) => {
                        if app.menu_state.selected() == Some(0) || key.code == KeyCode::Left {
                            app.screen = Screen::Main;
                            app.menu_state.select(Some(0));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let (title, items_to_show) = match app.screen {
        Screen::Main => (" Menú Principal ", &app.items),
        Screen::SubMenu => (" Submenú ", &app.submenu_items),
    };

    // 1. Calculamos las dimensiones dinámicas
    // El ancho es el largo del texto más largo + margen para bordes y flecha
    let max_item_width = items_to_show
        .iter()
        .map(|s| s.len())
        .max()
        .unwrap_or(0)
        .max(title.len()); // También consideramos el ancho del título

    let target_width = (max_item_width + 6) as u16; // +6 para bordes y padding
    let target_height = (items_to_show.len() + 2) as u16; // +2 para bordes superior/inferior

    // 2. Centramos el Rect basándonos en estos valores exactos
    let area = auto_size_rect(target_width, target_height, f.area());

    let items: Vec<ListItem> = items_to_show
        .iter()
        .map(|i| ListItem::new(i.as_str()))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
        )
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .highlight_symbol("→ ");

    // Renderizamos
    f.render_stateful_widget(list, area, &mut app.menu_state);
}

/// Crea un rectángulo centrado con dimensiones fijas en píxeles (celdas)
fn auto_size_rect(width: u16, height: u16, r: Rect) -> Rect {
    // Nos aseguramos de no exceder el tamaño de la terminal
    let w = width.min(r.width);
    let h = height.min(r.height);
    
    let x = (r.width - w) / 2;
    let y = (r.height - h) / 2;
    
    Rect::new(x, y, w, h)
}

