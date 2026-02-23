mod app;
mod event;
mod ui;

pub use app::App;
pub use typub_core::PostInfo;

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, prelude::CrosstermBackend};
use std::io::{self, Stdout};
use std::path::Path;
use typub_config::Config;

pub async fn run_with_root(config: &Config, project_root: &Path) -> Result<()> {
    let mut terminal = setup_terminal()?;

    let mut app = App::new(config, project_root)?;
    let result = event::run_event_loop(&mut terminal, &mut app).await;

    restore_terminal(&mut terminal)?;

    result
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}
