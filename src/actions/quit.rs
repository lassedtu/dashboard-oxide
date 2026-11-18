use anyhow::Result;

use super::{Action, Ctx, Step};

pub struct Quit;

impl Action for Quit {
    fn key(&self) -> char {
        'q'
    }

    fn title(&self) -> &str {
        "Quit"
    }

    fn run(&self, _ctx: &Ctx) -> Result<Step> {
        Ok(Step::Quit(None))
    }
}
