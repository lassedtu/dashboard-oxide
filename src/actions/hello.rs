//! Placeholder action to prove the Action/Step plumbing works.
//! Removed once the real actions land in step 5.

use anyhow::Result;

use super::{Action, Ctx, Step};

pub struct Hello;

impl Action for Hello {
    fn key(&self) -> char {
        'h'
    }

    fn title(&self) -> &str {
        "Say hello"
    }

    fn run(&self, ctx: &Ctx) -> Result<Step> {
        Ok(Step::Done(format!(
            "hi! projects live in {}",
            ctx.config.projects_dir.display()
        )))
    }
}
