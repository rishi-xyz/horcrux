//! Top-level render dispatcher.

use crate::tui::app::{App, Screen};
use ratatui::Frame;

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    match app.screen {
        Screen::Menu => app.menu.render(frame, area),
        Screen::Log => app.log.render(frame, area),
        Screen::Verify => app.verify.render(frame, area),
        Screen::Init => app.init.render(frame, area),
        Screen::Sign => app.sign.render(frame, area),
        Screen::Mpc => app.mpc.render(frame, area),
    }
}
