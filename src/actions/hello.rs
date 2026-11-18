//! Placeholder action to prove the Action/Step plumbing works
//! (Prompt -> Pick -> Done). Removed once the real actions land in step 5.

use anyhow::{Result, bail};

use super::{Action, Ctx, Step};

pub struct Hello;

impl Action for Hello {
    fn key(&self) -> char {
        'h'
    }

    fn title(&self) -> &str {
        "Say hello"
    }

    fn run(&self, _ctx: &Ctx) -> Result<Step> {
        Ok(Step::prompt("Your name", |name, _ctx| {
            let name = name.trim().to_string();
            if name.is_empty() {
                bail!("name can't be empty");
            }
            let greetings = vec!["Hello".to_string(), "Hej".into(), "Yo".into()];
            Ok(Step::pick("Greeting", greetings.clone(), move |i, ctx| {
                Ok(Step::Done(format!(
                    "{}, {name}! projects live in {}",
                    greetings[i],
                    ctx.config.projects_dir.display()
                )))
            }))
        }))
    }
}
