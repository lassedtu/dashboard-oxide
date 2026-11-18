mod app;
mod ui;

use app::App;

fn main() -> anyhow::Result<()> {
    // ratatui::init enters raw mode + alternate screen and installs a panic hook
    // that restores the terminal, so a crash never leaves your shell broken.
    let mut terminal = ratatui::init();
    let result = App::new().run(&mut terminal);
    ratatui::restore();
    result
}
