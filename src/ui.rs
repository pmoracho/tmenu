use std::borrow::Cow;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders, List, ListItem, Padding, Paragraph},
};

use crate::app::App;
use crate::model::MenuAction;

// Importamos la macro de traducción de rust-i18n
use rust_i18n::t;

// ═══════════════════════════════════════════════════════════════
// PALETA DE COLORES ELEGANTE Y COHERENTE
// ═══════════════════════════════════════════════════════════════

const COLOR_BORDER_PRIMARY: Color = Color::Rgb(100, 130, 160);
const COLOR_BORDER_ACCENT: Color = Color::Rgb(80, 110, 150);
const COLOR_HIGHLIGHT_BG: Color = Color::Rgb(40, 60, 100);
const COLOR_HIGHLIGHT_FG: Color = Color::Rgb(220, 230, 245);
const COLOR_SEARCH_SUCCESS: Color = Color::Rgb(100, 170, 140);
const COLOR_SEARCH_FAIL: Color = Color::Rgb(200, 100, 110);
const COLOR_COMMAND: Color = Color::Rgb(120, 180, 200);
const COLOR_SECONDARY: Color = Color::Rgb(140, 150, 170);
const COLOR_CONFIRMATION: Color = Color::Rgb(209, 85, 85);

/// Renderiza la interfaz completa en cada ciclo de dibujado.
pub fn ui(f: &mut Frame, app: &mut App) {
    let items_to_render = app.filtered_items();

    if !items_to_render.is_empty()
        && app
            .state
            .selected()
            .map_or(false, |i| i >= items_to_render.len())
    {
        app.state.select(Some(0));
    }

    let title = app.breadcrumb();
    let title_w = title.chars().count();

    let max_label_w = app
        .current_items
        .iter()
        .map(|item| item.label.chars().count())
        .max()
        .unwrap_or(0);
    let max_w = max_label_w.max(title_w);

    let box_width = (max_w + 14).max(24) as u16;
    let box_height = (app.current_items.len() + 7).max(8) as u16;

    let area = centered_rect(box_width, box_height, f.area());

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(3),
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

/// Renderiza el modal del wizard para comandos con parámetros.
fn render_wizard(f: &mut Frame, app: &App) {
    use ratatui::widgets::Clear;

    let Some(wizard) = &app.wizard else { return };
    let param = wizard.current_param();

    let total = wizard.params.len();
    let current = wizard.current + 1;

    // Título dinámico traducido
    let title = format!(" {} ({}/{}) ", param.label, current, total);
    let input_line = wizard.input.as_str();

    let popup_w: u16 = 54;
    let popup_h: u16 = 9;
    let area = centered_rect(popup_w, popup_h, f.area());

    f.render_widget(Clear, area);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .margin(1)
        .split(area);

    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .title_bottom(Line::from(t!("ui.wizard.bottom_hints")).centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_BORDER_ACCENT));
    f.render_widget(block, area);

    let available_w = (popup_w as usize).saturating_sub(3);
    let cmd_str = format!("cmd: {}", wizard.original_cmd);

    let cmd_display = if cmd_str.chars().count() > available_w {
        let truncated: String = cmd_str
            .chars()
            .take(available_w.saturating_sub(3))
            .collect();
        format!("{}...", truncated)
    } else {
        cmd_str
    };

    let cmd_widget = Paragraph::new(cmd_display).style(Style::default().fg(COLOR_SECONDARY));
    f.render_widget(cmd_widget, inner[0]);

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
    let label_widget = Paragraph::new(label_text).style(
        Style::default()
            .fg(COLOR_HIGHLIGHT_FG)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(label_widget, inner[1]);

    let input_widget = Paragraph::new(input_line)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(COLOR_COMMAND)),
        )
        .style(Style::default().fg(COLOR_COMMAND));
    f.render_widget(input_widget, inner[2]);

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
                MenuAction::Quit => " \u{2717}",
                _ => "",
            };
            ListItem::new(format!(" {}{}", item.label, symbol))
        })
        .collect();

    // El hint inferior cambia según si podemos volver atrás en el historial
    let depth_hint = if app.history.is_empty() {
        t!("ui.menu.hint_root")
    } else {
        t!("ui.menu.hint_submenu")
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
        .highlight_symbol(" \u{27a4} ");

    f.render_stateful_widget(list, area, &mut app.state);
}

/// Renderiza la barra de busqueda.
fn render_search_bar(f: &mut Frame, app: &App, area: Rect) {
    if !app.search_mode {
        return;
    }

    let result_count =
        crate::search::filter_recursive(&app.current_items, &app.search_text, 0).len();

    let (title, border_color, subtitle) = if result_count > 0 && !app.search_text.is_empty() {
        (
            t!("ui.search.results", count = result_count),
            COLOR_SEARCH_SUCCESS,
            t!("ui.search.hints_results"),
        )
    } else if app.search_text.is_empty() {
        (
            t!("ui.search.title"),
            COLOR_COMMAND,
            t!("ui.search.hints_empty"),
        )
    } else {
        (
            t!("ui.search.no_results"),
            COLOR_SEARCH_FAIL,
            t!("ui.search.hints_no_results"),
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

/// Renderiza un popup de vista previa del comando seleccionado.
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
        .unwrap_or_else(|| t!("ui.preview.no_selection").to_string());

    let screen = f.area();
    let popup_w = (cmd_text.chars().count() as u16 + 6)
        .max(24)
        .min(screen.width.saturating_sub(4));
    let popup_h: u16 = 3;

    let index = app.state.selected().unwrap_or(0) as u16;
    let item_y = menu_area.y + 1 + 1 + 1 + index;

    let popup_y = if item_y + 1 + popup_h <= screen.height {
        item_y
    } else {
        item_y.saturating_sub(popup_h)
    };

    let popup_x = menu_area.x.min(screen.width.saturating_sub(popup_w)) + 5;
    let popup_area = Rect::new(popup_x, popup_y, popup_w, popup_h);

    f.render_widget(Clear, popup_area);
    let popup = Paragraph::new(cmd_text)
        .block(
            Block::default()
                .title(t!("ui.preview.title"))
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
    use ratatui::{
        text::Span,
        widgets::{Cell, Clear, Row, Table},
    };

    // Estructuramos las filas llamando a t! para la descripción de cada atajo
    let shortcuts: &[(&str, Cow<'_, str>)] = &[
        ("↑ / ↓", t!("ui.help.keys.navigate")),
        ("Enter / →", t!("ui.help.keys.select")),
        ("Esc / ←", t!("ui.help.keys.back")),
        ("Inicio", t!("ui.help.keys.root")),
        ("Tab", t!("ui.help.keys.search")),
        ("Buscar", t!("ui.help.keys.filter")),
        ("Ctrl+Q", t!("ui.help.keys.quit")),
        ("F2", t!("ui.help.keys.preview")),
        ("F1", t!("ui.help.keys.help")),
    ];

    let rows: Vec<Row> = shortcuts
        .iter()
        .map(|(key, desc)| {
            Row::new(vec![
                Cell::from(Span::styled(
                    format!(" › {} ", key),
                    Style::default()
                        .fg(COLOR_COMMAND)
                        .add_modifier(Modifier::BOLD),
                )),
                Cell::from(Span::raw(format!(" {} ", desc))),
            ])
        })
        .collect();

    let table = Table::new(rows, [Constraint::Length(18), Constraint::Min(50)])
        .block(
            Block::default()
                .title(t!("ui.help.title"))
                .title_alignment(Alignment::Center)
                .title_bottom(Line::from(t!("ui.help.bottom_hints")).right_aligned())
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(COLOR_BORDER_PRIMARY)),
        )
        .column_spacing(1);

    let popup_w: u16 = 60;
    let popup_h: u16 = shortcuts.len() as u16 + 2;
    let area = centered_rect(popup_w, popup_h, f.area());

    f.render_widget(Clear, area);
    f.render_widget(table, area);
}

/// Modal de confirmación: muestra el comando y opciones Sí/No con navegación.
fn render_confirmation_modal(f: &mut Frame, app: &App) {
    use ratatui::text::Span;
    use ratatui::widgets::Clear;

    let Some(confirmation) = &app.confirmation else {
        return;
    };

    let cmd_text = &confirmation.cmd;
    let max_cmd_width = 50;
    let cmd_display = if cmd_text.chars().count() > max_cmd_width {
        let truncated: String = cmd_text
            .chars()
            .take(max_cmd_width.saturating_sub(3))
            .collect();
        format!("{}...", truncated)
    } else {
        cmd_text.clone()
    };

    let popup_w: u16 = 64;
    let popup_h: u16 = 8;
    let area = centered_rect(popup_w, popup_h, f.area());

    f.render_widget(Clear, area);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .margin(1)
        .split(area);

    let block = Block::default()
        .title(t!("ui.confirm.title"))
        .title_alignment(Alignment::Center)
        .title_bottom(Line::from(t!("ui.confirm.bottom_hints")).centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_CONFIRMATION));
    f.render_widget(block, area);

    let cmd_widget = Paragraph::new(cmd_display).style(Style::default().fg(COLOR_COMMAND));
    f.render_widget(cmd_widget, inner[0]);

    let si_style = if confirmation.selected == 0 {
        Style::default()
            .bg(COLOR_HIGHLIGHT_BG)
            .fg(COLOR_CONFIRMATION)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_CONFIRMATION)
    };

    let no_style = if confirmation.selected == 1 {
        Style::default()
            .bg(COLOR_HIGHLIGHT_BG)
            .fg(COLOR_CONFIRMATION)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_SECONDARY)
    };

    let options = vec![
        Span::styled(t!("ui.confirm.yes"), si_style),
        Span::raw("     "),
        Span::styled(t!("ui.confirm.no"), no_style),
    ];

    let options_widget = Paragraph::new(Line::from(options)).alignment(Alignment::Center);
    f.render_widget(options_widget, inner[2]);
}

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