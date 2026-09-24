//! The main menu: an arrow-navigable list of the available modes.

use crate::tui::app::Screen;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

const ITEMS: &[(&str, Screen)] = &[
    ("Access log", Screen::Log),
    ("Verify shard/share files", Screen::Verify),
    ("Init — split a key (Mode A)", Screen::Init),
    ("Sign (Mode A)", Screen::Sign),
    ("MPC split/sign (Mode B)", Screen::Mpc),
];

#[derive(Default)]
pub struct MenuScreen {
    pub cursor: usize,
}

impl MenuScreen {
    /// Returns the screen to switch to, if the user selected one.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Screen> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.cursor = self.cursor.saturating_sub(1);
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.cursor + 1 < ITEMS.len() {
                    self.cursor += 1;
                }
                None
            }
            KeyCode::Enter => Some(ITEMS[self.cursor].1),
            _ => None,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        use ratatui::layout::{Constraint, Direction, Layout};
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(area);

        let items: Vec<ListItem> = ITEMS
            .iter()
            .enumerate()
            .map(|(i, (label, _))| {
                let style = if i == self.cursor {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                ListItem::new(*label).style(style)
            })
            .collect();
        let block = Block::default()
            .borders(Borders::ALL)
            .title("HORCRUX — select a mode");
        frame.render_widget(List::new(items).block(block), chunks[0]);

        let help = Paragraph::new("↑/↓ move   Enter select   q quit")
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(help, chunks[1]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn cursor_does_not_move_above_the_first_item() {
        let mut menu = MenuScreen::default();
        assert!(menu.handle_key(key(KeyCode::Up)).is_none());
        assert_eq!(menu.cursor, 0);
    }

    #[test]
    fn cursor_does_not_move_below_the_last_item() {
        let mut menu = MenuScreen::default();
        for _ in 0..ITEMS.len() + 2 {
            menu.handle_key(key(KeyCode::Down));
        }
        assert_eq!(menu.cursor, ITEMS.len() - 1);
    }

    #[test]
    fn enter_selects_the_highlighted_item() {
        let mut menu = MenuScreen::default();
        menu.handle_key(key(KeyCode::Down));
        menu.handle_key(key(KeyCode::Down));
        let selected = menu.handle_key(key(KeyCode::Enter));
        assert_eq!(selected, Some(ITEMS[2].1));
    }

    #[test]
    fn vim_style_jk_keys_also_navigate() {
        let mut menu = MenuScreen::default();
        menu.handle_key(key(KeyCode::Char('j')));
        assert_eq!(menu.cursor, 1);
        menu.handle_key(key(KeyCode::Char('k')));
        assert_eq!(menu.cursor, 0);
    }
}
