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

/// What the app should do after an action (or one stage of it) runs.
pub enum Step {
    /// Finished; show this message and return to the menu.
    Done(String),
    /// Exit dbx.
    Quit,
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
