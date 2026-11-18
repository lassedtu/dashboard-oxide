use std::io;
use std::process::Command;

use anyhow::Result;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{EnterAlternateScreen, enable_raw_mode};

use crate::actions::{self, Action, Ctx, OnBack, Pick, Prompt, Step};
use crate::config::Config;
use crate::sys;

pub struct Status {
    pub text: String,
    pub is_error: bool,
}

/// What's currently on screen.
pub enum Screen {
    Menu,
    Prompt { prompt: Prompt, input: String },
    Pick(Pick),
}

pub struct App {
    pub config: Config,
    pub actions: Vec<Box<dyn Action>>,
    pub selected: usize,
    pub screen: Screen,
    pub status: Option<Status>,
    /// Printed to the shell after the TUI closes (e.g. "opened foo").
    pub exit_message: Option<String>,
    /// Run in the foreground after the TUI closes (e.g. nvim on a project).
    pub exit_command: Option<Command>,
    /// A terminal program to run with the TUI suspended, then the step to resume with.
    foreground: Option<(Command, Step)>,
    should_quit: bool,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            actions: actions::registry(&config),
            config,
            selected: 0,
            screen: Screen::Menu,
            status: None,
            exit_message: None,
            exit_command: None,
            foreground: None,
            should_quit: false,
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| crate::ui::draw(frame, self))?;
            self.handle_event(event::read()?);

            if let Some((mut command, then)) = self.foreground.take() {
                let result = suspend_and_run(terminal, &mut command);
                // The program may have been an editor on our own config.
                self.reload_config();
                self.apply(result.map(|()| then));
            }
        }
        Ok(())
    }

    /// Pick up edits to the config file. On error the previous config stays in use.
    fn reload_config(&mut self) {
        if self.config.path.as_os_str().is_empty() {
            return; // not loaded from a file (tests)
        }
        match self.config.reload() {
            Ok(config) => self.config = config,
            Err(err) => {
                self.status = Some(Status {
                    text: format!("config not reloaded: {err:#}"),
                    is_error: true,
                });
            }
        }
    }

    /// Return to the main menu, rebuilding it so extensions toggled since
    /// (in dbx or in the file) appear or disappear. The cursor stays on the
    /// same action if it's still there.
    fn show_menu(&mut self) {
        self.screen = Screen::Menu;
        self.reload_config();
        let current = self.actions.get(self.selected).map(|a| a.title().to_string());
        self.actions = actions::registry(&self.config);
        self.selected = current
            .and_then(|title| self.actions.iter().position(|a| a.title() == title))
            .unwrap_or(0);
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
        match key.code {
            KeyCode::Enter => {
                self.reload_config();
                let Screen::Prompt { prompt, input } = &mut self.screen else { return };
                let result = (prompt.on_submit)(input.clone(), &Ctx { config: &self.config });
                self.apply(result);
            }
            KeyCode::Esc => {
                let Screen::Prompt { prompt, .. } = &mut self.screen else { return };
                let on_back = prompt.on_back.take();
                self.go_back(on_back);
            }
            _ => {
                let Screen::Prompt { input, .. } = &mut self.screen else { return };
                match key.code {
                    KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        input.push(c);
                    }
                    KeyCode::Backspace => {
                        input.pop();
                    }
                    _ => {}
                }
            }
        }
    }

    fn handle_pick_key(&mut self, key: KeyEvent) {
        let Screen::Pick(pick) = &mut self.screen else { return };
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => pick.selected = pick.selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                pick.selected = (pick.selected + 1).min(pick.items.len().saturating_sub(1));
            }
            KeyCode::Enter if !pick.items.is_empty() => {
                self.reload_config();
                let Screen::Pick(pick) = &mut self.screen else { return };
                let result = (pick.on_select)(pick.selected, &Ctx { config: &self.config });
                self.apply(result);
            }
            KeyCode::Esc => {
                let on_back = pick.on_back.take();
                self.go_back(on_back);
            }
            _ => {}
        }
    }

    /// Esc: run the screen's `on_back` if it has one, otherwise return to the menu.
    fn go_back(&mut self, on_back: Option<OnBack>) {
        let Some(mut on_back) = on_back else {
            self.show_menu();
            return;
        };
        self.reload_config();
        match on_back(&Ctx { config: &self.config }) {
            Ok(step) => self.apply(Ok(step)),
            Err(err) => {
                // Couldn't go back; stay here, and keep Esc working next time.
                self.put_back(on_back);
                self.apply(Err(err));
            }
        }
    }

    fn put_back(&mut self, on_back: OnBack) {
        match &mut self.screen {
            Screen::Prompt { prompt, .. } => prompt.on_back = Some(on_back),
            Screen::Pick(pick) => pick.on_back = Some(on_back),
            Screen::Menu => {}
        }
    }

    fn activate(&mut self, index: usize) {
        self.selected = index;
        // Re-read the config before every action and callback, so edits made
        // outside dbx (or by a previous step) are always picked up.
        self.reload_config();
        let result = self.actions[index].run(&Ctx { config: &self.config });
        self.apply(result);
    }

    /// Carry out whatever an action asked for. On error the current screen is
    /// left as-is, so the user can fix their input or pick something else.
    fn apply(&mut self, result: Result<Step>) {
        match result {
            Ok(Step::Done(text)) => {
                self.show_menu();
                self.status = Some(Status { text, is_error: false });
            }
            Ok(Step::Quit(message)) => {
                self.exit_message = message;
                self.should_quit = true;
            }
            Ok(Step::Prompt(prompt)) => {
                let input = prompt.initial.clone();
                self.screen = Screen::Prompt { prompt, input };
            }
            Ok(Step::Pick(mut pick)) => {
                pick.selected = pick.selected.min(pick.items.len().saturating_sub(1));
                self.screen = Screen::Pick(pick);
            }
            Ok(Step::RunInTerminal { command, then }) => {
                if matches!(*then, Step::Quit(_)) {
                    self.exit_command = Some(command);
                    self.apply(Ok(*then));
                } else {
                    self.foreground = Some((command, *then));
                }
            }
            Err(err) => {
                self.status = Some(Status { text: format!("{err:#}"), is_error: true });
            }
        }
    }
}

/// Leave the TUI, run `command` attached to the terminal, then come back.
/// The TUI is restored even if the command fails.
fn suspend_and_run(terminal: &mut DefaultTerminal, command: &mut Command) -> Result<()> {
    ratatui::restore();
    let result = sys::run_foreground(command);
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    terminal.clear()?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn esc_runs_back_handler_else_returns_to_menu() {
        let mut app = App::new(Config::default());
        let list = || Pick::new("List", vec!["a".into(), "b".into()], |_, _| Ok(Step::Quit(None)));
        let prompt = Prompt::new("Sub", |_, _| Ok(Step::Quit(None)))
            .initial("prefilled")
            .back(move |_| Ok(list().selected(1).into()));
        app.apply(Ok(prompt.into()));
        assert!(matches!(&app.screen, Screen::Prompt { input, .. } if input == "prefilled"));

        press(&mut app, KeyCode::Esc);
        assert!(matches!(&app.screen, Screen::Pick(pick) if pick.selected == 1));

        press(&mut app, KeyCode::Esc); // no back handler
        assert!(matches!(app.screen, Screen::Menu));
    }

    #[test]
    fn returning_to_menu_picks_up_toggled_extensions() {
        let dir = std::env::temp_dir().join(format!("dbx-app-ext-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("config.toml");
        let mut app = App::new(Config::load_from(&path).unwrap());
        app.selected = 2; // Config
        assert_eq!(app.actions.len(), 4);

        // Disable an extension while a sub-screen is open, then Esc back.
        app.apply(Ok(Step::pick("Sub", vec!["x".into()], |_, _| Ok(Step::Quit(None)))));
        let disabled: toml_edit::Array = ["new-project"].into_iter().collect();
        crate::config::set(&path, "extensions.disabled", disabled).unwrap();
        press(&mut app, KeyCode::Esc);

        let titles: Vec<_> = app.actions.iter().map(|a| a.title()).collect();
        assert_eq!(titles, ["Recent projects", "Config", "Quit"]);
        assert_eq!(app.actions[app.selected].title(), "Config");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn terminal_command_before_quit_runs_after_exit() {
        let mut app = App::new(Config::default());
        app.apply(Ok(Step::RunInTerminal {
            command: Command::new("nvim"),
            then: Box::new(Step::Quit(Some("opened foo".into()))),
        }));
        assert!(app.should_quit);
        assert!(app.exit_command.is_some());
        assert!(app.foreground.is_none());
        assert_eq!(app.exit_message.as_deref(), Some("opened foo"));
    }

    #[test]
    fn terminal_command_before_done_suspends_tui() {
        let mut app = App::new(Config::default());
        app.apply(Ok(Step::RunInTerminal {
            command: Command::new("nvim"),
            then: Box::new(Step::Done("saved".into())),
        }));
        assert!(!app.should_quit);
        assert!(app.foreground.is_some());
        assert!(app.exit_command.is_none());
    }

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