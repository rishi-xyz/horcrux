//! Input event plumbing.
//!
//! `crossterm::event::poll`/`read` are blocking calls. They run on a
//! dedicated OS thread (not a tokio task) so they never occupy a tokio worker
//! thread that the render loop also needs — see `src/tui/mod.rs`'s module
//! docs for the full async-integration rationale.

use crossterm::event::{self, Event, KeyEvent};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// An event delivered to the TUI's main loop.
#[derive(Debug, Clone, Copy)]
pub enum TuiEvent {
    Key(KeyEvent),
    /// A terminal resize. Carries no data: `terminal.draw` recomputes layout
    /// against the current size every frame regardless, so this variant only
    /// needs to exist to wake the select loop.
    Resize,
    Tick,
}

/// Spawns the blocking input thread and exposes its events over a channel.
pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<TuiEvent>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        std::thread::spawn(move || {
            let mut last_tick = Instant::now();
            loop {
                let timeout = tick_rate
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or(Duration::from_millis(0));
                if event::poll(timeout).unwrap_or(false) {
                    let sent = match event::read() {
                        Ok(Event::Key(key)) => tx.send(TuiEvent::Key(key)).is_ok(),
                        Ok(Event::Resize(..)) => tx.send(TuiEvent::Resize).is_ok(),
                        Ok(_) => true,
                        Err(_) => false,
                    };
                    if !sent {
                        return;
                    }
                }
                if last_tick.elapsed() >= tick_rate {
                    if tx.send(TuiEvent::Tick).is_err() {
                        return;
                    }
                    last_tick = Instant::now();
                }
            }
        });
        Self { rx }
    }

    pub async fn next(&mut self) -> Option<TuiEvent> {
        self.rx.recv().await
    }
}
