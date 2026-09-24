//! The interactive terminal UI (`horcrux tui`).
//!
//! Owned by the binary, not `lib.rs`: the TUI is a consumer of horcrux's
//! crypto/protocol API at the same layer as `main.rs`'s CLI handling, not
//! part of that API itself (see `agent/wayfinder/PLAN.md`'s TUI plan for the
//! full rationale — a second binary would also mean a second checksummed
//! release artifact, weakening the "what you audited is what you ran" story).
//!
//! # Async design
//!
//! The binary is already `#[tokio::main]`. Two kinds of work must never run
//! on the thread driving the render loop:
//! - CPU-bound Argon2id shard/share decryption (deliberately slow, as an
//!   anti-brute-force measure) runs via `tokio::task::spawn_blocking`.
//! - Network I/O (blockhash fetch, broadcast) runs via plain `tokio::spawn`.
//!
//! Both report results back over one channel (`AppEvent`, see `action.rs`);
//! the render loop itself never `.await`s either directly. Input is read on
//! a dedicated OS thread (`event.rs`), not a tokio task, since
//! `crossterm::event::poll`/`read` block.
//!
//! # Key material
//!
//! No screen ever renders a reconstructed private key or seed. See the
//! key-material decision in `agent/wayfinder/PLAN.md`'s TUI plan for why, and
//! `screens/sign.rs`/`screens/mpc.rs` for how the reconstructed key stays
//! confined to a `spawn_blocking` closure and is zeroized before anything
//! crosses back into UI state.

mod action;
mod app;
mod event;
mod screens;
mod theme;
mod ui;
mod widgets;

use app::App;
use event::{EventHandler, TuiEvent};
use std::time::Duration;

/// Run the interactive terminal UI until the user quits.
pub async fn run() -> anyhow::Result<()> {
    let mut terminal = ratatui::try_init()?;
    let mut app = App::new();
    let mut events = EventHandler::new(Duration::from_millis(100));

    let result = run_loop(&mut terminal, &mut app, &mut events).await;

    ratatui::try_restore()?;
    result
}

async fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    events: &mut EventHandler,
) -> anyhow::Result<()> {
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;

        tokio::select! {
            ev = events.next() => {
                match ev {
                    Some(TuiEvent::Tick) => {}
                    Some(other) => app.handle_event(other),
                    None => break,
                }
            }
            Some(msg) = app.worker_rx.recv() => {
                app.apply(msg);
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}
