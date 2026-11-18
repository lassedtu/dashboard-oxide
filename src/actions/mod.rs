//! Everything dbx can do is an `Action`.
//!
//! Actions never draw anything themselves. They return a `Step` describing
//! what should happen next, and the app/UI layer carries it out. Adding a
//! feature = writing a new file here and listing it in `registry()`.

mod hello;
mod quit;

use anyhow::Result;

use crate::config::Config;

/// What an action gets access to while running.
pub struct Ctx<'a> {
    pub config: &'a Config,
}

pub type OnSubmit = Box<dyn FnOnce(String, &Ctx) -> Result<Step>>;
pub type OnSelect = Box<dyn FnOnce(usize, &Ctx) -> Result<Step>>;

/// What the app should do after an action (or one stage of it) runs.
/// `Prompt` and `Pick` carry a continuation, so multi-stage actions are
/// just chains of closures, each returning the next `Step`.
pub enum Step {
    /// Finished; show this message and return to the menu.
    Done(String),
    /// Exit dbx.
    Quit,
    /// Ask for a line of text.
    Prompt { label: String, on_submit: OnSubmit },
    /// Let the user choose one of `items`; the callback gets its index.
    Pick { title: String, items: Vec<String>, on_select: OnSelect },
}

impl Step {
    pub fn prompt(
        label: impl Into<String>,
        on_submit: impl FnOnce(String, &Ctx) -> Result<Step> + 'static,
    ) -> Step {
        Step::Prompt { label: label.into(), on_submit: Box::new(on_submit) }
    }

    pub fn pick(
        title: impl Into<String>,
        items: Vec<String>,
        on_select: impl FnOnce(usize, &Ctx) -> Result<Step> + 'static,
    ) -> Step {
        Step::Pick { title: title.into(), items, on_select: Box::new(on_select) }
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

/// All actions, in menu order.
pub fn registry() -> Vec<Box<dyn Action>> {
    vec![Box::new(hello::Hello), Box::new(quit::Quit)]
}
