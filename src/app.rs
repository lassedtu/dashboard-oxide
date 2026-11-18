use anyhow::Result;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::actions::{self, Action, Ctx, OnSelect, OnSubmit, Step};
use crate::config::Config;

pub struct Status {
    pub text: String,
    pub is_error: bool,
}

/// What's currently on screen.
pub enum Screen {
    Menu,
    Prompt { label: String, input: String, on_submit: OnSubmit },
    Pick { title: String, items: Vec<String>, selected: usize, on_select: OnSelect },
}

pub struct App {
    pub config: Config,
    pub actions: Vec<Box<dyn Action>>,
    pub selected: usize,
    pub screen: Screen,
    pub status: Option<Status>,
    /// Printed to the shell after the TUI closes (e.g. "opened foo").
    pub exit_message: Option<String>,
    should_quit: bool,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            actions: actions::registry(),
            selected: 0,
            screen: Screen::Menu,
            status: None,
            exit_message: None,
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
        // Raw mode swallows SIGINT, so handle Ctrl+C ourselves, everywhere.
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }
        self.status = None;

        match self.screen {
            Screen::Menu => self.handle_menu_key(key),
            Screen::Prompt { .. } => self.handle_prompt_key(key),
            Screen::Pick { .. } => self.handle_pick_key(key),
        }
    }

    fn handle_menu_key(&mut self, key: KeyEvent) {
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

    fn handle_prompt_key(&mut self, key: KeyEvent) {
        let Screen::Prompt { input, on_submit, .. } = &mut self.screen else { return };
        match key.code {
            KeyCode::Esc => self.screen = Screen::Menu,
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => input.push(c),
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Enter => {
                let result = on_submit(input.clone(), &Ctx { config: &self.config });
                self.apply(result);
            }
            _ => {}
        }
    }

    fn handle_pick_key(&mut self, key: KeyEvent) {
        let Screen::Pick { items, selected, on_select, .. } = &mut self.screen else { return };
        match key.code {
            KeyCode::Esc => self.screen = Screen::Menu,
            KeyCode::Up | KeyCode::Char('k') => *selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                *selected = (*selected + 1).min(items.len().saturating_sub(1));
            }
            KeyCode::Enter if !items.is_empty() => {
                let result = on_select(*selected, &Ctx { config: &self.config });
                self.apply(result);
            }
            _ => {}
        }
    }

    fn activate(&mut self, index: usize) {
        self.selected = index;
        let result = self.actions[index].run(&Ctx { config: &self.config });
        self.apply(result);
    }

    /// Carry out whatever an action asked for. On error the current screen is
    /// left as-is, so the user can fix their input or pick something else.
    fn apply(&mut self, result: Result<Step>) {
        match result {
            Ok(Step::Done(text)) => {
                self.screen = Screen::Menu;
                self.status = Some(Status { text, is_error: false });
            }
            Ok(Step::Quit(message)) => {
                self.exit_message = message;
                self.should_quit = true;
            }
            Ok(Step::Prompt { label, on_submit }) => {
                self.screen = Screen::Prompt { label, input: String::new(), on_submit };
            }
            Ok(Step::Pick { title, items, on_select }) => {
                self.screen = Screen::Pick { title, items, selected: 0, on_select };
            }
            Err(err) => {
                self.status = Some(Status { text: format!("{err:#}"), is_error: true });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(app: &mut App, code: KeyCode) {
        app.handle_event(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
    }

    #[test]
    fn prompt_error_keeps_input_and_allows_retry() {
        let mut app = App::new(Config::default());
        app.apply(Ok(Step::prompt("Name", |name, _| {
            if name == "ok" {
                Ok(Step::Done("accepted".into()))
            } else {
                anyhow::bail!("bad name")
            }
        })));

        press(&mut app, KeyCode::Char('x'));
        press(&mut app, KeyCode::Enter);
        assert!(matches!(&app.screen, Screen::Prompt { input, .. } if input == "x"));
        assert!(app.status.as_ref().is_some_and(|s| s.is_error));

        // Fix the input and retry with the same closure.
        press(&mut app, KeyCode::Backspace);
        press(&mut app, KeyCode::Char('o'));
        press(&mut app, KeyCode::Char('k'));
        press(&mut app, KeyCode::Enter);
        assert!(matches!(app.screen, Screen::Menu));
        assert!(app.status.as_ref().is_some_and(|s| s.text == "accepted"));
    }

    #[test]
    fn ctrl_c_quits_from_prompt() {
        let mut app = App::new(Config::default());
        app.apply(Ok(Step::prompt("Name", |_, _| Ok(Step::Quit(None)))));
        app.handle_event(Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)));
        assert!(app.should_quit);
        assert!(matches!(&app.screen, Screen::Prompt { input, .. } if input.is_empty()));
    }
}
