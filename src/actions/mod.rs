//! Everything dbx can do is an `Action`.
//!
//! Actions never draw anything themselves. They return a `Step` describing
//! what should happen next, and the app/UI layer carries it out. Optional
//! features live in `crate::extensions`; only the core actions are here.

mod config_menu;
mod quit;

use std::path::Path;
use std::process::Command;
use std::rc::Rc;

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

    /// Give a `Prompt` or `Pick` an Esc handler if it doesn't have one yet.
    pub fn or_back(mut self, on_back: impl FnMut(&Ctx) -> Result<Step> + 'static) -> Step {
        match &mut self {
            Step::Prompt(prompt) if prompt.on_back.is_none() => {
                prompt.on_back = Some(Box::new(on_back));
            }
            Step::Pick(pick) if pick.on_back.is_none() => pick.on_back = Some(Box::new(on_back)),
            _ => {}
        }
        self
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
    /// Optional shortcut key per item, shown before it; pressing one selects it.
    pub shortcuts: Vec<char>,
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
            shortcuts: Vec::new(),
            selected: 0,
            on_select: Box::new(on_select),
            on_back: None,
        }
    }

    /// Shortcut keys, one per item.
    pub fn shortcuts(mut self, keys: Vec<char>) -> Self {
        self.shortcuts = keys;
        self
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

/// The main menu: enabled extensions, then the core actions. An extension
/// with one action shows that action; with several, a submenu.
pub fn registry(config: &Config) -> Vec<Box<dyn Action>> {
    let mut actions: Vec<Box<dyn Action>> = Vec::new();
    for ext in extensions::all().into_iter().filter(|ext| ext.is_enabled(config)) {
        if ext.actions.len() == 1 {
            actions.extend(ext.actions);
        } else if !ext.actions.is_empty() {
            let actions_in_group = Rc::new(ext.actions);
            actions.push(Box::new(Group { key: ext.key, title: ext.name, actions: actions_in_group }));
        }
    }
    actions.push(Box::new(config_menu::ConfigMenu));
    actions.push(Box::new(quit::Quit));
    actions
}

/// Main-menu entry for an extension with several actions.
struct Group {
    key: char,
    title: String,
    actions: Rc<Vec<Box<dyn Action>>>,
}

impl Action for Group {
    fn key(&self) -> char {
        self.key
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn run(&self, _ctx: &Ctx) -> Result<Step> {
        Ok(submenu(self.title.clone(), self.actions.clone(), 0))
    }
}

fn submenu(title: String, actions: Rc<Vec<Box<dyn Action>>>, selected: usize) -> Step {
    let items = actions.iter().map(|a| a.title().to_string()).collect();
    let keys = actions.iter().map(|a| a.key()).collect();
    Pick::new(title.clone(), items, move |i, ctx| {
        let step = actions[i].run(ctx)?;
        // Esc from the action's first screen comes back here, not to the main menu.
        let (title, actions) = (title.clone(), actions.clone());
        Ok(step.or_back(move |_| Ok(submenu(title.clone(), actions.clone(), i))))
    })
    .shortcuts(keys)
    .selected(selected)
    .into()
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
    fn registry_groups_and_leaves_out_disabled_extensions() {
        let titles = |config: &Config| -> Vec<String> {
            registry(config).iter().map(|a| a.title().to_string()).collect()
        };
        let mut config = Config::default();
        assert_eq!(titles(&config), ["Coding projects", "Config", "Quit"]);

        config.raw = toml::from_str("[extensions.coding-projects]\nenabled = false").unwrap();
        assert_eq!(titles(&config), ["Config", "Quit"]);
    }

    #[test]
    fn submenu_actions_return_to_submenu_on_esc() {
        let config = Config::default();
        let ctx = Ctx { config: &config };
        let group = registry(&config).into_iter().next().unwrap();
        let Step::Pick(mut pick) = group.run(&ctx).unwrap() else { panic!("expected submenu") };
        assert_eq!(pick.shortcuts, ['n', 'r']);

        // New project opens a prompt; its Esc goes back to the submenu, same row.
        let Step::Prompt(mut prompt) = (pick.on_select)(0, &ctx).unwrap() else { panic!() };
        let back = prompt.on_back.as_mut().expect("back handler")(&ctx).unwrap();
        assert!(matches!(back, Step::Pick(p) if p.title == "Coding projects" && p.selected == 0));
    }
}
