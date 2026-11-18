mod actions;
mod app;
mod config;
mod extensions;
mod sys;
mod ui;

use app::App;
use config::Config;

const HELP: &str = "\
dbx - dashboard-oxide

Usage: dbx [--help | --version]

Run without arguments to open the dashboard.
Config: ~/.dbx/config.toml";

fn main() -> anyhow::Result<()> {
    match std::env::args().nth(1).as_deref() {
        None => {}
        Some("-V" | "--version") => {
            println!("dbx {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some("-h" | "--help") => {
            println!("{HELP}");
            return Ok(());
        }
        Some(other) => {
            eprintln!("dbx: unknown argument `{other}`\n\n{HELP}");
            std::process::exit(2);
        }
    }

    // Load config before touching the terminal so errors print normally.
    let config = Config::load()?;

    // ratatui::init enters raw mode + alternate screen and installs a panic hook
    // that restores the terminal, so a crash never leaves your shell broken.
    let mut terminal = ratatui::init();
    let mut app = App::new(config);
    let result = app.run(&mut terminal);
    ratatui::restore();
    result?;

    // A terminal editor takes over the shell where dbx was.
    if let Some(mut command) = app.exit_command {
        sys::run_foreground(&mut command)?;
    }
    if let Some(message) = app.exit_message {
        println!("{message}");
    }
    Ok(())
}
