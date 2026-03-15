use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders, List, ListItem, Padding, Paragraph},
};

use crate::app::App;
use crate::model::MenuAction;

/// Renderiza la interfaz completa en cada ciclo de dibujado.
pub fn ui(f: &mut Frame, app: &mut App) {
    let items_to_render = app.filtered_items();

    // Ajustar seleccion si esta fuera de rango (puede pasar al filtrar)
    if !items_to_render.is_empty()
        && app
            .state
            .selected()
            .map_or(false, |i| i >= items_to_render.len())
    {
        app.state.select(Some(0));
    }

    // Clonar titulo para liberar el borrow inmutable antes de pasar app como mutable.
    let title = app.current_title.clone();

    // Calcular dimensiones a partir de items_to_render (lo que realmente se dibuja),
    // no de current_items. Usar chars().count() para ancho visual correcto con Unicode.
    let title_w = title.chars().count();
    let max_label_w = items_to_render
        .iter()
        .map(|item| item.label.chars().count())
        .max()
        .unwrap_or(0);
    let max_w = max_label_w.max(title_w);

    // Minimo de ancho razonable para que el menu siempre sea visible
    let box_width = (max_w + 14).max(24) as u16;
    // +6: bordes (2) + padding top/bottom (2) + hint inferior (1) + margen (1)
    // minimo de alto para que siempre haya espacio para al menos 1 item
    let box_height = (items_to_render.len() + 6).max(8) as u16;

    let area = centered_rect(box_width, box_height, f.area());

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // Lista (crece)
            Constraint::Length(3), // Barra de busqueda (fija)
        ])
        .split(area);

    render_menu_list(f, app, &items_to_render, chunks[0], &title);
    render_search_bar(f, app, chunks[1]);
}

/// Renderiza la lista de items del menu.
fn render_menu_list(
    f: &mut Frame,
    app: &mut App,
    items_to_render: &[crate::model::MenuItem],
    area: Rect,
    title: &str,
) {
    let border_color = if app.history.is_empty() {
        Color::Cyan    // Menu raiz
    } else {
        Color::Magenta // Submenu
    };

    let list_items: Vec<ListItem> = items_to_render
        .iter()
        .map(|item| {
            let symbol = match item.action {
                MenuAction::OpenSubmenu(_) => " \u{25b6}", // triangulo indicando submenu
                _ => "",
            };
            ListItem::new(format!(" {}{}", item.label, symbol))
        })
        .collect();

    let depth_hint = if app.history.is_empty() {
        String::from("[Ctrl+q] Salir")
    } else {
        format!("[Esc] Volver  [Ctrl+q] Salir  (nivel {})", app.history.len())
    };

    let list = List::new(list_items)
        .block(
            Block::default()
                .title(format!(" {} ", title))
                .title_alignment(Alignment::Center)
                .title_bottom(Line::from(depth_hint).right_aligned())
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(border_color))
                .padding(Padding::new(0, 0, 1, 1)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Indexed(24))
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" \u{27a4} "); // flecha

    f.render_stateful_widget(list, area, &mut app.state);
}

/// Renderiza la barra de busqueda (solo en modo busqueda).
fn render_search_bar(f: &mut Frame, app: &App, area: Rect) {
    if !app.search_mode {
        return;
    }

    let input_panel = Paragraph::new(app.search_text.as_str()).block(
        Block::default()
            .title(" Buscar... ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Yellow)),
    );

    // Posicion del cursor: x = borde izq + chars escritos + 1 (por el borde)
    let cursor_x = area
        .x
        .saturating_add(app.search_text.chars().count() as u16)
        .saturating_add(1);
    f.set_cursor_position((cursor_x, area.y + 1));
    f.render_widget(input_panel, area);
}

/// Calcula un Rect centrado dentro de `r` con el tamano indicado,
/// sin exceder los limites del contenedor.
fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
    let w = width.min(r.width);
    let h = height.min(r.height);
    Rect::new(
        r.x + (r.width.saturating_sub(w)) / 2,
        r.y + (r.height.saturating_sub(h)) / 2,
        w,
        h,
    )
}