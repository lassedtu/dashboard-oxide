use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Position, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};

use crate::app::{App, Screen};

const WIDTH: u16 = 50;
/// Max visible rows in a Pick list before it scrolls.
const MAX_PICK_ROWS: u16 = 15;

pub fn draw(frame: &mut Frame, app: &App) {
    let body_height = match &app.screen {
        Screen::Menu => app.actions.len() as u16 + 2,
        Screen::Prompt { .. } => 3,
        Screen::Pick { items, .. } => (items.len() as u16).clamp(1, MAX_PICK_ROWS) + 2,
    };

    let [area] = Layout::horizontal([Constraint::Length(WIDTH)])
        .flex(Flex::Center)
        .areas(frame.area());
    let [body, status_area] =
        Layout::vertical([Constraint::Length(body_height), Constraint::Length(1)])
            .flex(Flex::Center)
            .areas(area);

    match &app.screen {
        Screen::Menu => draw_menu(frame, app, body),
        Screen::Prompt { label, input, .. } => draw_prompt(frame, label, input, body),
        Screen::Pick { title, items, selected, .. } => {
            draw_pick(frame, title, items, *selected, body)
        }
    }

    if let Some(status) = &app.status {
        let line = Line::from(status.text.as_str()).centered();
        let line = if status.is_error { line.red() } else { line.green() };
        frame.render_widget(line, status_area);
    }
}

fn draw_menu(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .actions
        .iter()
        .map(|a| {
            ListItem::new(Line::from(vec![
                Span::from(format!(" {} ", a.key())).bold().cyan(),
                Span::from(a.title()),
            ]))
        })
        .collect();

    let footer = Line::from(format!(" {} ", app.config.projects_dir.display()))
        .dark_gray()
        .right_aligned();

    let list = List::new(items)
        .block(Block::bordered().title(" dbx ").title_bottom(footer))
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED));

    let mut state = ListState::default().with_selected(Some(app.selected));
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_prompt(frame: &mut Frame, label: &str, input: &str, area: Rect) {
    let hint = Line::from(" enter: ok · esc: cancel ").dark_gray().right_aligned();
    let block = Block::bordered().title(format!(" {label} ")).title_bottom(hint);
    frame.render_widget(Paragraph::new(input).block(block), area);

    let cursor_x = area.x + 1 + Line::from(input).width() as u16;
    frame.set_cursor_position(Position::new(cursor_x.min(area.right() - 2), area.y + 1));
}

fn draw_pick(frame: &mut Frame, title: &str, items: &[String], selected: usize, area: Rect) {
    let hint = Line::from(" enter: select · esc: back ").dark_gray().right_aligned();
    let block = Block::bordered().title(format!(" {title} ")).title_bottom(hint);

    if items.is_empty() {
        let empty = Paragraph::new(" nothing here").dark_gray().block(block);
        frame.render_widget(empty, area);
        return;
    }

    let list = List::new(items.iter().map(|s| ListItem::new(format!(" {s}"))))
        .block(block)
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED));
    let mut state = ListState::default().with_selected(Some(selected));
    frame.render_stateful_widget(list, area, &mut state);
}
