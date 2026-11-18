use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState};

use crate::app::App;

pub fn draw(frame: &mut Frame, app: &App) {
    let [area] = Layout::horizontal([Constraint::Length(40)])
        .flex(Flex::Center)
        .areas(frame.area());
    let [area] = Layout::vertical([Constraint::Length(app.menu.len() as u16 + 2)])
        .flex(Flex::Center)
        .areas(area);

    let items: Vec<ListItem> = app
        .menu
        .iter()
        .map(|m| {
            ListItem::new(Line::from(vec![
                Span::from(format!(" {} ", m.key)).bold().cyan(),
                Span::from(m.title),
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
