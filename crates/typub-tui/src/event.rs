use super::app::{App, PublishState, View};
use super::ui;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{Terminal, prelude::CrosstermBackend};
use std::io::Stdout;
use std::time::Duration;

pub async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App<'_>,
) -> Result<()> {
    loop {
        if app.pending_action.is_some()
            && let Err(e) = app.execute_pending().await
        {
            app.status_message = Some(format!("Error: {e}"));
        }

        terminal.draw(|f| ui::draw(f, app))?;

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            if matches!(app.publish_state, PublishState::Completed { .. }) {
                app.publish_state = PublishState::Idle;
                continue;
            }

            app.clear_status();

            match (key.modifiers, key.code) {
                (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                    app.should_quit = true;
                }
                (KeyModifiers::NONE, KeyCode::Char('q')) => {
                    app.back();
                }
                (KeyModifiers::NONE, KeyCode::Char('r')) => {
                    if let Err(e) = app.reload() {
                        app.status_message = Some(format!("Reload failed: {e}"));
                    }
                }
                _ => handle_view_keys(app, key.code),
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

fn handle_view_keys(app: &mut App<'_>, code: KeyCode) {
    match code {
        KeyCode::Up | KeyCode::Char('k') => app.select_up(),
        KeyCode::Down | KeyCode::Char('j') => app.select_down(),
        KeyCode::PageUp => app.page_up(),
        KeyCode::PageDown => app.page_down(),
        KeyCode::Home => {
            if app.view == View::Preview {
                app.preview_scroll = 0;
            }
        }
        KeyCode::End => {
            if app.view == View::Preview {
                app.preview_scroll = u16::MAX;
            }
        }
        KeyCode::Enter => app.enter(),
        KeyCode::Esc => app.back(),
        KeyCode::Char('p') => app.request_preview(),
        KeyCode::Char('o') => app.request_browser(),
        KeyCode::Char('P') => {
            if app.view == View::PostDetail {
                app.request_publish_platform();
            }
        }
        KeyCode::Char('A') => {
            app.request_publish_all();
        }
        KeyCode::Char('s') => {
            if app.view == View::PostList {
                app.cycle_sort_field();
            }
        }
        KeyCode::Char('S') => {
            if app.view == View::PostList {
                app.toggle_sort_order();
            }
        }
        _ => {}
    }
}
