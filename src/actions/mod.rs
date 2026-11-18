//! Everything dbx can do is an `Action`.
//!
//! Actions never draw anything themselves. They return a `Step` describing
//! what should happen next, and the app/UI layer carries it out. Optional
//! features live in `crate::extensions`; only the core actions are here.

mod config_menu;
mod quit;

use std::path::Path;
use std::process::Command;

use anyhow::Result;

use crate::config::Config;
use crate::{extensions, sys};

/// What an action gets access to while running. `config` is re-read from
/// disk before every action and callback, so it always reflects the file.
pub struct Ctx<'a> {
    pub config: &'a Config,
}

/// Continuations are `FnMut` so they can be retried: if one returns an error,
/// the user stays on the same prompt/picker (input intact) and can try again.
pub type OnSubmit = Box<dyn FnMut(String, &Ctx) -> Result<Step>>;
pub type OnSelect = Box<dyn FnMut(usize, &Ctx) -> Result<Step>>;
pub type OnBack = Box<dyn FnMut(&Ctx) -> Result<Step>>;

/// What the app should do after an action (or one stage of it) runs.
/// `Prompt` and `Pick` carry a continuation, so multi-stage actions are
/// just chains of closures, each returning the next `Step`.
pub enum Step {
    /// Finished; show this message and return to the menu.
    Done(String),
    /// Exit dbx, optionally printing a message to the shell afterwards.
    Quit(Option<String>),
    /// Ask for a line of text.
    Prompt(Prompt),
    /// Let the user choose one item from a list.
    Pick(Pick),
    /// Hand the terminal to `command` (e.g. nvim) until it exits, then continue
    /// with `then`. If `then` is `Quit`, dbx exits first and runs the command
    /// in its place, so there's no flicker back into the TUI.
    RunInTerminal { command: Command, then: Box<Step> },
}

impl Step {
    /// Shorthand for a plain `Prompt`.
    pub fn prompt(
        label: impl Into<String>,
        on_submit: impl FnMut(String, &Ctx) -> Result<Step> + 'static,
    ) -> Step {
        Prompt::new(label, on_submit).into()
    }

    /// Shorthand for a plain `Pick`.
    pub fn pick(
        title: impl Into<String>,
        items: Vec<String>,
        on_select: impl FnMut(usize, &Ctx) -> Result<Step> + 'static,
    ) -> Step {
        Pick::new(title, items, on_select).into()
    }
}

/// A text prompt. Esc runs `on_back` if set, otherwise returns to the main menu.
pub struct Prompt {
    pub label: String,
    pub initial: String,
    pub on_submit: OnSubmit,
    pub on_back: Option<OnBack>,
}

impl Prompt {
    pub fn new(
        label: impl Into<String>,
        on_submit: impl FnMut(String, &Ctx) -> Result<Step> + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            initial: String::new(),
            on_submit: Box::new(on_submit),
            on_back: None,
        }
    }

    /// Pre-fill the input, e.g. with the current value of a setting.
    pub fn initial(mut self, text: impl Into<String>) -> Self {
        self.initial = text.into();
        self
    }

    /// What Esc does, e.g. go back to the previous list.
    pub fn back(mut self, on_back: impl FnMut(&Ctx) -> Result<Step> + 'static) -> Self {
        self.on_back = Some(Box::new(on_back));
        self
    }
}

/// A list to choose from. Esc runs `on_back` if set, otherwise returns to the main menu.
pub struct Pick {
    pub title: String,
    pub items: Vec<String>,
    pub selected: usize,
    pub on_select: OnSelect,
    pub on_back: Option<OnBack>,
}

impl Pick {
    pub fn new(
        title: impl Into<String>,
        items: Vec<String>,
        on_select: impl FnMut(usize, &Ctx) -> Result<Step> + 'static,
    ) -> Self {
        Self {
            title: title.into(),
            items,
            selected: 0,
            on_select: Box::new(on_select),
            on_back: None,
        }
    }

    /// Start with this row highlighted.
    pub fn selected(mut self, index: usize) -> Self {
        self.selected = index;
        self
    }

    /// What Esc does, e.g. go back to the previous list.
    pub fn back(mut self, on_back: impl FnMut(&Ctx) -> Result<Step> + 'static) -> Self {
        self.on_back = Some(Box::new(on_back));
        self
    }
}

impl From<Prompt> for Step {
    fn from(prompt: Prompt) -> Self {
        Step::Prompt(prompt)
    }
}

impl From<Pick> for Step {
    fn from(pick: Pick) -> Self {
        Step::Pick(pick)
    }
}

pub trait Action {
    /// Shortcut key shown in the menu.
    fn key(&self) -> char;
    /// Label shown in the menu.
    fn title(&self) -> &str;
    /// Called when the user picks this action. Errors are shown in the status line.
    fn run(&self, ctx: &Ctx) -> Result<Step>;
}

/// The main menu: actions from enabled extensions, then the core actions.
pub fn registry(config: &Config) -> Vec<Box<dyn Action>> {
    let mut actions: Vec<Box<dyn Action>> = extensions::all()
        .into_iter()
        .filter(|ext| ext.is_enabled(config))
        .flat_map(|ext| ext.actions)
        .collect();
    actions.push(Box::new(config_menu::ConfigMenu));
    actions.push(Box::new(quit::Quit));
    actions
}

/// Open `path` in the configured editor, then continue with `then`.
/// GUI editors are launched in the background; terminal editors get the terminal.
pub(crate) fn open_in_editor(ctx: &Ctx, path: &Path, then: Step) -> Result<Step> {
    let editor = sys::editor_command(&ctx.config.editor, path)?;
    if editor.in_terminal {
        Ok(Step::RunInTerminal { command: editor.command, then: Box::new(then) })
    } else {
        sys::spawn_detached(editor.command)?;
        Ok(then)
    }
}

/// Shared ending for actions that open a project: open the editor, then
/// quit or report back depending on `close_on_open`.
pub(crate) fn open_project(ctx: &Ctx, path: &Path, message: &str) -> Result<Step> {
    let message = message.to_string();
    let then = if ctx.config.close_on_open {
        Step::Quit(Some(message))
    } else {
        Step::Done(message)
    };
    open_in_editor(ctx, path, then)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_leaves_out_disabled_extensions() {
        let titles = |config: &Config| -> Vec<String> {
            registry(config).iter().map(|a| a.title().to_string()).collect()
        };
        let mut config = Config::default();
        assert_eq!(titles(&config), ["New project", "Recent projects", "Config", "Quit"]);

        config.extensions.disabled = vec!["recent-projects".into(), "not-installed".into()];
        assert_eq!(titles(&config), ["New project", "Config", "Quit"]);
    }
}
