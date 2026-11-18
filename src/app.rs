use anyhow::Result;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};

use crate::actions::{self, Action, Ctx, Step};
use crate::config::Config;

pub struct Status {
    pub text: String,
    pub is_error: bool,
}

pub struct App {
    pub config: Config,
    pub actions: Vec<Box<dyn Action>>,
    pub selected: usize,
    pub status: Option<Status>,
    should_quit: bool,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            actions: actions::registry(),
            selected: 0,
            status: None,
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
        self.status = None;

        match key.code {
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = (self.selected + 1).min(self.actions.len() - 1);
            }
            KeyCode::Enter => self.activate(self.selected),
            KeyCode::Char(c) => {
                if let Some(i) = self.actions.iter().position(|a| a.key() == c) {
                    self.activate(i);
                }
            }
            _ => {}
        }
    }

    fn activate(&mut self, index: usize) {
        self.selected = index;
        let ctx = Ctx { config: &self.config };
        let result = self.actions[index].run(&ctx);
        self.apply(result);
    }

    /// Carry out whatever an action asked for.
    fn apply(&mut self, result: Result<Step>) {
        match result {
            Ok(Step::Done(text)) => self.status = Some(Status { text, is_error: false }),
            Ok(Step::Quit) => self.should_quit = true,
            Err(err) => {
                self.status = Some(Status { text: format!("{err:#}"), is_error: true });
            }
        }
    }
}
