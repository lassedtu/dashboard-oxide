use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState};

use crate::app::App;

pub fn draw(frame: &mut Frame, app: &App) {
    let [area] = Layout::horizontal([Constraint::Length(50)])
        .flex(Flex::Center)
        .areas(frame.area());
    let [menu_area, status_area] = Layout::vertical([
        Constraint::Length(app.actions.len() as u16 + 2),
        Constraint::Length(1),
    ])
    .flex(Flex::Center)
    .areas(area);

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
    frame.render_stateful_widget(list, menu_area, &mut state);

    if let Some(status) = &app.status {
        let line = Line::from(status.text.as_str()).centered();
        let line = if status.is_error { line.red() } else { line.green() };
        frame.render_widget(line, status_area);
    }
}
