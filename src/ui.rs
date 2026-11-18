use std::sync::LazyLock;

use ansi_to_tui::IntoText;
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph, Wrap};

use crate::actions::Pick;
use crate::app::{App, Screen, Status};

const WIDTH: u16 = 50;
/// Status messages (often long error chains) get more room than the menu.
const STATUS_WIDTH: u16 = 80;
const STATUS_HEIGHT: u16 = 2;
/// Max visible rows in a Pick list before it scrolls.
const MAX_PICK_ROWS: u16 = 15;
/// Baked into the binary at compile time, so the installed `dbx` doesn't need the file.
/// It contains ANSI color codes, which are parsed into ratatui styles once.
static LOGO: LazyLock<Text<'static>> = LazyLock::new(|| {
    let mut text = include_str!("../assets/logo.ans").into_text().unwrap_or_default();
    while text.lines.last().is_some_and(|l| l.width() == 0) {
        text.lines.pop();
    }
    text
});

pub fn draw(frame: &mut Frame, app: &App) {
    let body_height = match &app.screen {
        Screen::Menu => app.actions.len() as u16 + 2,
        Screen::Prompt { .. } => 3,
        Screen::Pick(pick) => (pick.items.len() as u16).clamp(1, MAX_PICK_ROWS) + 2,
    };

    let logo = &*LOGO;
    let logo_width = logo.width() as u16;
    let mut logo_height = logo.height() as u16;
    let spacer = 1;
    // Hide the logo rather than squash the menu when the terminal is too small.
    let needed = logo_height + spacer + body_height + STATUS_HEIGHT;
    if logo_height == 0 || needed > frame.area().height || logo_width > frame.area().width {
        logo_height = 0;
    }

    let [logo_area, _, body, status_area] = Layout::vertical([
        Constraint::Length(logo_height),
        Constraint::Length(if logo_height > 0 { spacer } else { 0 }),
        Constraint::Length(body_height),
        Constraint::Length(STATUS_HEIGHT),
    ])
    .flex(Flex::Center)
    .areas(frame.area());

    if logo_height > 0 {
        // Center the logo as a block but keep its lines left-aligned,
        // so the ASCII art doesn't get skewed.
        let [logo_area] = Layout::horizontal([Constraint::Length(logo_width)])
            .flex(Flex::Center)
            .areas(logo_area);
        frame.render_widget(Paragraph::new(logo.clone()), logo_area);
    }

    let center = |area: Rect, width: u16| {
        let [area] = Layout::horizontal([Constraint::Length(width)])
            .flex(Flex::Center)
            .areas(area);
        area
    };
    let body = center(body, WIDTH);
    let status_area = center(status_area, STATUS_WIDTH);

    match &app.screen {
        Screen::Menu => draw_menu(frame, app, body),
        Screen::Prompt { prompt, input } => draw_prompt(frame, &prompt.label, input, body),
        Screen::Pick(pick) => draw_pick(frame, pick, body),
    }

    if let Some(status) = &app.status {
        draw_status(frame, status, status_area);
    }
}

fn draw_status(frame: &mut Frame, status: &Status, area: Rect) {
    // No ✓/✗ icons: some terminals and fonts draw them two cells wide,
    // covering the first letter of the message. Color carries the meaning.
    let (text, color) = if status.is_error {
        (format!("Error: {}", status.text), Color::Red)
    } else {
        (status.text.clone(), Color::Green)
    };
    let paragraph = Paragraph::new(text)
        .fg(color)
        .centered()
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
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

    let footer = Line::from(concat!(" v", env!("CARGO_PKG_VERSION"), " "))
        .dark_gray()
        .right_aligned();

    let list = List::new(items)
        .block(Block::bordered().title(" dbx ").title_bottom(footer))
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED));

    let mut state = ListState::default().with_selected(Some(app.selected));
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_prompt(frame: &mut Frame, label: &str, input: &str, area: Rect) {
    let hint = Line::from(" enter: ok · esc: cancel ")
        .dark_gray()
        .right_aligned();
    let block = Block::bordered()
        .title(format!(" {label} "))
        .title_bottom(hint);
    frame.render_widget(Paragraph::new(input).block(block), area);

    let cursor_x = area.x + 1 + Line::from(input).width() as u16;
    frame.set_cursor_position(Position::new(cursor_x.min(area.right() - 2), area.y + 1));
}

fn draw_pick(frame: &mut Frame, pick: &Pick, area: Rect) {
    let hint = Line::from(" enter: select · esc: back ")
        .dark_gray()
        .right_aligned();
    let block = Block::bordered()
        .title(format!(" {} ", pick.title))
        .title_bottom(hint);

    if pick.items.is_empty() {
        let empty = Paragraph::new(" nothing here").dark_gray().block(block);
        frame.render_widget(empty, area);
        return;
    }

    let rows = pick.items.iter().enumerate().map(|(i, item)| match pick.shortcuts.get(i) {
        // Same look as the main menu.
        Some(key) => ListItem::new(Line::from(vec![
            Span::from(format!(" {key} ")).bold().cyan(),
            Span::from(item.as_str()),
        ])),
        None => ListItem::new(format!(" {item}")),
    });
    let list = List::new(rows)
        .block(block)
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED));
    let mut state = ListState::default().with_selected(Some(pick.selected));
    frame.render_stateful_widget(list, area, &mut state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn logo_parses_without_raw_escape_codes() {
        assert!(LOGO.height() > 0);
        let raw: String = LOGO
            .lines
            .iter()
            .flat_map(|l| &l.spans)
            .map(|s| s.content.as_ref())
            .collect();
        assert!(!raw.contains('\x1b'), "unparsed escape codes in logo");
    }

    #[test]
    fn renders_menu_with_logo() {
        let app = App::new(Config::default());
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| draw(f, &app)).unwrap();
        let buffer = terminal.backend().buffer();
        let rows: Vec<String> = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect()
            })
            .collect();
        println!("{}", rows.join("\n"));
        assert!(rows.iter().any(|r| r.contains("Coding projects")));
    }
}