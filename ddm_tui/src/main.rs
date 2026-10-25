//! DDM analysis TUI
use std::io::{self, BufReader};

use crossterm::{
    event::{self, DisableMouseCapture, Event, KeyCode},
    terminal::{LeaveAlternateScreen, disable_raw_mode},
};
use ddm_app::load_l0b;
use ddm_tui::{App, ui};
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
    crossterm::{
        event::EnableMouseCapture,
        execute,
        terminal::{EnterAlternateScreen, enable_raw_mode},
    },
};

fn main() -> eyre::Result<()> {
    enable_raw_mode()?;
    let mut stderr = io::stderr();
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;

    let reader = BufReader::new(std::fs::File::open("../data/HG1_TEST_512.l0b.ddm")?);
    let l0b_data = load_l0b(reader, &mut prodash::progress::Discard)?;
    // create app and run it
    let mut app = App::new(l0b_data)?;
    run_app(&mut terminal, &mut app)?;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> eyre::Result<()> {
    loop {
        terminal
            .draw(|f| ui(f, app))
            .map_err(|e| eyre::eyre!("{e}"))?;
        if let Event::Key(key) = event::read()? {
            if key.kind == event::KeyEventKind::Release {
                // Skip events that are not KeyEventKind::Press
                continue;
            }
            match key.code {
                KeyCode::Left => {
                    app.change_timestamp(1);
                }
                KeyCode::Right => {
                    app.change_timestamp(-1);
                }
                _ => (),
            }
        }
    }
}
