use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders, List, ListItem, Padding, Paragraph},
};

use crate::app::App;
use crate::model::MenuAction;

// ═══════════════════════════════════════════════════════════════
// PALETA DE COLORES ELEGANTE Y COHERENTE
// ═══════════════════════════════════════════════════════════════
// Inspirada en interfaces modernas minimalistas con tonos sofisticados

/// Color de bordes principal: azul grisáceo sutil
const COLOR_BORDER_PRIMARY: Color = Color::Rgb(100, 130, 160);

/// Color de bordes secundario: más oscuro, para énfasis
const COLOR_BORDER_ACCENT: Color = Color::Rgb(80, 110, 150);

/// Color de fondo para selección: azul profundo muy suave
const COLOR_HIGHLIGHT_BG: Color = Color::Rgb(40, 60, 100);

/// Color de texto resaltado: blanco con ligerísimo azul
const COLOR_HIGHLIGHT_FG: Color = Color::Rgb(220, 230, 245);

/// Color para búsqueda exitosa: verde agua sutil
const COLOR_SEARCH_SUCCESS: Color = Color::Rgb(100, 170, 140);

/// Color para búsqueda fallida: rojo suave
const COLOR_SEARCH_FAIL: Color = Color::Rgb(200, 100, 110);

/// Color para comandos/ejecutables: cyan sutil
const COLOR_COMMAND: Color = Color::Rgb(120, 180, 200);

/// Color para texto secundario/gris: neutro elegante
const COLOR_SECONDARY: Color = Color::Rgb(140, 150, 170);

// /// Color para errores y alertas: rojo moderno
// const COLOR_ERROR: Color = Color::Rgb(210, 110, 120);

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
    let title = app.breadcrumb();

    // Calcular dimensiones a partir de items_to_render (lo que realmente se dibuja),
    // no de current_items. Usar chars().count() para ancho visual correcto con Unicode.
    let title_w = title.chars().count();
    // Dimensiones basadas en current_items para que el box no salte al filtrar
    let max_label_w = app.current_items
        .iter()
        .map(|item| item.label.chars().count())
        .max()
        .unwrap_or(0);
    let max_w = max_label_w.max(title_w);

    let box_width = (max_w + 14).max(24) as u16;
    // Altura fija al máximo del nivel actual (no al filtrado)
    let box_height = (app.current_items.len() + 7).max(8) as u16;

    let area = centered_rect(box_width, box_height, f.area());

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // Lista (crece)
            Constraint::Length(3), // Barra de busqueda (fija)
        ])
        .split(area);

    let menu_area = chunks[0];
    render_menu_list(f, app, &items_to_render, menu_area, &title);
    render_search_bar(f, app, chunks[1]);

    if app.wizard.is_some() {
        render_wizard(f, app);
    } else if app.show_help {
        render_help_modal(f);
    } else if app.confirmation.is_some() {
        render_confirmation_modal(f, app);
    } else {
        render_preview_popup(f, app, &items_to_render, menu_area);
    }
}

fn render_wizard(f: &mut Frame, app: &App) {
    use ratatui::widgets::Clear;

    let Some(wizard) = &app.wizard else { return };
    let param = wizard.current_param();

    let total = wizard.params.len();
    let current = wizard.current + 1; // 1-based para el usuario

    // Título con progreso: "Branch name (1/3)"
    let title = format!(" {} ({}/{}) ", param.label, current, total);

    let input_line = wizard.input.as_str();

    let popup_w: u16 = 54;
    let popup_h: u16 = 9; // título + cmd preview + separador + input + hints + bordes
    let area = centered_rect(popup_w, popup_h, f.area());

    f.render_widget(Clear, area);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // línea de contexto del comando
            Constraint::Length(1), // label del placeholder  ← NUEVO
            Constraint::Length(3), // campo de input
            Constraint::Length(1), // hints
        ])
        .margin(1)
        .split(area);


    // Bloque contenedor
    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .title_bottom(Line::from(" [Enter] Confirmar  [Esc] Cancelar ").centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_BORDER_ACCENT));
    f.render_widget(block, area);

    // Ancho disponible: ancho del popup menos márgenes y borde (popup_w - 4)
    let available_w = (popup_w as usize).saturating_sub(3);
    let cmd_str = format!("cmd: {}", wizard.original_cmd);

    let cmd_display = if cmd_str.chars().count() > available_w {
        // Truncar dejando espacio para "..."
        let truncated: String = cmd_str.chars().take(available_w.saturating_sub(3)).collect();
        format!("{}...", truncated)
    } else {
        cmd_str
    };

    let cmd_widget = Paragraph::new(cmd_display)
        .style(Style::default().fg(COLOR_SECONDARY));
    f.render_widget(cmd_widget, inner[0]);

    // Label del campo actual: "Ingrese un nombre:"
    let summary: String = wizard.params[..wizard.current]
        .iter()
        .zip(wizard.values[..wizard.current].iter())
        .map(|(p, v)| format!("{}:{} ", p.label, v))
        .collect();
    let label_text = if summary.is_empty() {
        format!("{}:", param.label)
    } else {
        format!("{} │ {}:", summary.trim_end(), param.label)
    };
    let label_widget = Paragraph::new(label_text)
        .style(Style::default().fg(COLOR_HIGHLIGHT_FG).add_modifier(Modifier::BOLD));
    f.render_widget(label_widget, inner[1]);

    // Campo de input
    let input_widget = Paragraph::new(input_line)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(COLOR_COMMAND)),
        )
        .style(Style::default().fg(COLOR_COMMAND));
    f.render_widget(input_widget, inner[2]);

    // Cursor dentro del campo de input
    let cursor_x = inner[2].x + wizard.input.chars().count() as u16 + 1;
    let cursor_y = inner[2].y + 1;
    f.set_cursor_position((cursor_x, cursor_y));
}
/// Renderiza la lista de items del menu.
fn render_menu_list(
    f: &mut Frame,
    app: &mut App,
    items_to_render: &[crate::model::MenuItem],
    area: Rect,
    title: &str,
) {
    let list_items: Vec<ListItem> = items_to_render
        .iter()
        .map(|item| {
            let symbol = match item.action {
                MenuAction::OpenSubmenu(_) => " \u{25b6}",
                MenuAction::Quit => " \u{2717}", // ✗ símbolo de salida
                _ => "",
            };
            ListItem::new(format!(" {}{}", item.label, symbol))
        })
        .collect();

    let depth_hint = if app.history.is_empty() {
        String::from(" [Ctrl+q] Salir ")
    } else {
        String::from(" [<-] Volver [Ctrl+q] Salir ")
    };

    let list = List::new(list_items)
        .block(
            Block::default()
                .title(format!(" {} ", title))
                .title_alignment(Alignment::Center)
                .title_bottom(Line::from(depth_hint).right_aligned())
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(COLOR_BORDER_PRIMARY))
                .padding(Padding::new(0, 0, 1, 1)),
        )
        .highlight_style(
            Style::default()
                .bg(COLOR_HIGHLIGHT_BG)
                .fg(COLOR_HIGHLIGHT_FG)
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

    // Contar resultados reales (sin el fallback)
    let result_count = crate::search::filter_recursive(&app.current_items, &app.search_text, 0).len();

    let (title, border_color, subtitle) = if result_count > 0 && !app.search_text.is_empty() {
        (
            format!(" 🔍 Búsqueda: {} resultados ", result_count),
            COLOR_SEARCH_SUCCESS,
            " [Tab] Cerrar  [Esc] Limpiar "
        )
    } else if app.search_text.is_empty() {
        (
            String::from(" 🔍 Búsqueda "),
            COLOR_COMMAND,
            " [Tab] Cerrar "
        )
    } else {
        (
            String::from(" 🔍 Sin resultados "),
            COLOR_SEARCH_FAIL,
            " [Esc] Limpiar  [Tab] Cerrar "
        )
    };

    let input_panel = Paragraph::new(app.search_text.as_str())
        .block(
            Block::default()
                .title(title)
                .title_alignment(Alignment::Center)
                .title_bottom(Line::from(subtitle).right_aligned())
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(border_color)),
        )
        .style(Style::default().fg(COLOR_COMMAND));

    let cursor_x = area
        .x
        .saturating_add(app.search_text.chars().count() as u16)
        .saturating_add(1);
    f.set_cursor_position((cursor_x, area.y + 1));
    f.render_widget(input_panel, area);
}

fn render_preview_popup(
    f: &mut Frame,
    app: &App,
    items: &[crate::model::MenuItem],
    menu_area: Rect,
) {
    use ratatui::widgets::Clear;

    if !app.show_preview {
        return;
    }

    // Solo mostrar preview si el item seleccionado es un comando ejecutable
    let is_executable_command = app
        .state
        .selected()
        .and_then(|i| items.get(i))
        .map(|item| matches!(&item.action, MenuAction::Execute(_)))
        .unwrap_or(false);

    if !is_executable_command {
        return;
    }

    let cmd_text = app
        .state
        .selected()
        .and_then(|i| items.get(i))
        .and_then(|item| match &item.action {
            MenuAction::Execute(cmd) => Some(format!("$ {}", cmd)),
            _ => None,
        })
        .unwrap_or_else(|| String::from("(sin selección)"));

    let screen = f.area();
    let popup_w = (cmd_text.chars().count() as u16 + 6)
        .max(24)
        .min(screen.width.saturating_sub(4));
    let popup_h: u16 = 3; // borde top + 1 línea de texto + borde bottom

    // Fila Y del ítem seleccionado en coordenadas de terminal:
    //   +1 borde superior, +1 padding top, +1 margen interno del bloque
    let index = app.state.selected().unwrap_or(0) as u16;
    let item_y = menu_area.y + 1 + 1 + 1 + index;

    // Intentar colocar debajo; si no entra, colocar encima
    let popup_y = if item_y + 1 + popup_h <= screen.height {
        item_y + 0
    } else {
        item_y.saturating_sub(popup_h)
    };

    // Alinear horizontalmente con el menú, sin salirse de pantalla
    let popup_x = menu_area.x.min(screen.width.saturating_sub(popup_w)) + 5;

    let popup_area = Rect::new(popup_x, popup_y, popup_w, popup_h);

    f.render_widget(Clear, popup_area);
    let popup = Paragraph::new(cmd_text)
        .block(
            Block::default()
                .title(" Comando a ejecutar ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(COLOR_COMMAND))
                .padding(Padding::new(1, 1, 0, 0)),
        )
        .style(Style::default().fg(COLOR_HIGHLIGHT_FG));

    f.render_widget(popup, popup_area);
}

/// Ventana de ayuda bloqueante con todos los atajos de teclado.
fn render_help_modal(f: &mut Frame) {
    use ratatui::{text::Span, widgets::{Clear, Table, Row, Cell}};

    let shortcuts: &[(&str, &str)] = &[
        ("↑ / ↓",       "Navegar ítems (↑↓ funciona en búsqueda)"),
        ("Enter / →",   "Seleccionar / entrar al submenú"),
        ("Esc / ←",     "Volver al menú anterior / limpiar búsqueda"),
        ("Inicio",      "Ir al menú raíz"),
        ("Tab",         "Activar / cerrar búsqueda"),
        ("Buscar",      "Escribe para filtrar en vivo"),
        ("Ctrl+Q",      "Salir de la aplicación"),
        ("F2",          "Mostrar / ocultar vista previa"),
        ("F1",          "Mostrar / cerrar esta ayuda"),
    ];

    let rows: Vec<Row> = shortcuts
        .iter()
        .map(|(key, desc)| {
            Row::new(vec![
                Cell::from(Span::styled(
                    format!(" › {} ", key),
                    Style::default().fg(COLOR_COMMAND).add_modifier(Modifier::BOLD),
                )),
                Cell::from(Span::raw(format!(" {} ", desc))),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [Constraint::Length(18), Constraint::Min(5q0)],
    )
    .block(
        Block::default()
            .title(" Ayuda — Atajos de teclado ")
            .title_alignment(Alignment::Center)
            .title_bottom(Line::from(" [Esc] [F1] Cerrar ").right_aligned())
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(COLOR_BORDER_PRIMARY)),
    )
    .column_spacing(1);

    let popup_w: u16 = 60;
    let popup_h: u16 = shortcuts.len() as u16 + 2; // filas + bordes + padding
    let area = centered_rect(popup_w, popup_h, f.area());

    f.render_widget(Clear, area);
    f.render_widget(table, area);

}

/// Modal de confirmación: muestra el comando y opciones Sí/No con navegación.
fn render_confirmation_modal(f: &mut Frame, app: &App) {
    use ratatui::widgets::Clear;
    use ratatui::text::Span;

    let Some(confirmation) = &app.confirmation else { return };

    // Truncar comando muy largo
    let cmd_text = &confirmation.cmd;
    let max_cmd_width = 50;
    let cmd_display = if cmd_text.chars().count() > max_cmd_width {
        let truncated: String = cmd_text.chars().take(max_cmd_width.saturating_sub(3)).collect();
        format!("{}...", truncated)
    } else {
        cmd_text.clone()
    };

    let popup_w: u16 = 64;
    let popup_h: u16 = 10; // título + cmd + separador + opciones + bordes + padding
    let area = centered_rect(popup_w, popup_h, f.area());

    f.render_widget(Clear, area);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // línea del comando
            Constraint::Length(1), // separador
            Constraint::Length(3), // opciones Sí/No
        ])
        .margin(1)
        .split(area);

    // Bloque contenedor
    let block = Block::default()
        .title(" ¿Ejecutar comando? ")
        .title_alignment(Alignment::Center)
        .title_bottom(Line::from(" [Enter] Confirmar  [Esc] Cancelar ").centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_BORDER_ACCENT));
    f.render_widget(block, area);

    // Mostrar el comando
    let cmd_widget = Paragraph::new(cmd_display)
        .style(Style::default().fg(COLOR_COMMAND));
    f.render_widget(cmd_widget, inner[0]);

    // Opciones: "[ Sí ]  [ No ]" con la selección destacada
    let si_style = if confirmation.selected == 0 {
        Style::default()
            .bg(COLOR_HIGHLIGHT_BG)
            .fg(COLOR_HIGHLIGHT_FG)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_SECONDARY)
    };

    let no_style = if confirmation.selected == 1 {
        Style::default()
            .bg(COLOR_HIGHLIGHT_BG)
            .fg(COLOR_HIGHLIGHT_FG)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_SECONDARY)
    };

    let options = vec![
        Span::styled("  [ Sí ]  ", si_style),
        Span::raw("     "),
        Span::styled("[ No ]  ", no_style),
    ];

    let options_widget = Paragraph::new(Line::from(options))
        .alignment(Alignment::Center);
    f.render_widget(options_widget, inner[2]);
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