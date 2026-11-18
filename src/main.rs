mod actions;
mod app;
mod config;
mod sys;
mod ui;

use app::App;
use config::Config;

fn main() -> anyhow::Result<()> {
    // Load config before touching the terminal so errors print normally.
    let config = Config::load()?;

    // ratatui::init enters raw mode + alternate screen and installs a panic hook
    // that restores the terminal, so a crash never leaves your shell broken.
    let mut terminal = ratatui::init();
    let mut app = App::new(config);
    let result = app.run(&mut terminal);
    ratatui::restore();
    result?;

    if let Some(message) = app.exit_message {
        println!("✓ {message}");
    }
    Ok(())
}
