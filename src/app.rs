use anyhow::Result;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};

/// A menu entry. Static for now; replaced by the Action registry in step 3.
pub struct MenuItem {
    pub key: char,
    pub title: &'static str,
}

pub struct App {
    pub menu: Vec<MenuItem>,
    pub selected: usize,
    should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            menu: vec![
                MenuItem { key: 'n', title: "New project" },
                MenuItem { key: 'r', title: "Recent projects" },
                MenuItem { key: 'q', title: "Quit" },
            ],
            selected: 0,
            should_quit: false,
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| crate::ui::draw(frame, self))?;
            self.handle_event(event::read()?);
        }
        Ok(())
    }

    fn handle_event(&mut self, event: Event) {
        let Event::Key(key) = event else { return };
        // Ignore key releases (relevant on Windows / kitty keyboard protocol).
        if key.kind != KeyEventKind::Press {
            return;
        }
        match key.code {
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = (self.selected + 1).min(self.menu.len() - 1);
            }
            KeyCode::Enter => self.activate(self.selected),
            KeyCode::Char(c) => {
                if let Some(i) = self.menu.iter().position(|m| m.key == c) {
                    self.activate(i);
                }
            }
            _ => {}
        }
    }

    fn activate(&mut self, index: usize) {
        self.selected = index;
        if self.menu[index].key == 'q' {
            self.should_quit = true;
        }
        // Other entries do nothing yet.
    }
}
