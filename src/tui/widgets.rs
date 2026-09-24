//! Shared field widgets used across TUI screens.
//!
//! `TextField` is the one and only place password masking happens: it never
//! hands the typed value to a `Paragraph` when `masked` is set, only a
//! `*`-run of the same length. Every password prompt in the TUI goes through
//! this widget, so the masking rule can't be forgotten on one screen and kept
//! on another.

use crossterm::event::{Event, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use tui_input::Input;
use tui_input::backend::crossterm::to_input_request;

use crate::tui::theme;

/// A single-line text field, optionally masked (for passwords).
#[derive(Default)]
pub struct TextField {
    pub input: Input,
    pub masked: bool,
}

impl TextField {
    pub fn masked() -> Self {
        Self {
            input: Input::default(),
            masked: true,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if let Some(req) = to_input_request(&Event::Key(key)) {
            self.input.handle(req);
        }
    }

    pub fn value(&self) -> &str {
        self.input.value()
    }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.input = Input::new(value.into());
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, label: &str, focused: bool) {
        let display = if self.masked {
            "*".repeat(self.input.value().chars().count())
        } else {
            self.input.value().to_string()
        };
        let border_style = if focused {
            Style::default().fg(theme::ACCENT)
        } else {
            Style::default()
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(label)
            .border_style(border_style);
        frame.render_widget(Paragraph::new(display).block(block), area);
        if focused {
            frame.set_cursor_position((area.x + 1 + self.input.visual_cursor() as u16, area.y + 1));
        }
    }
}

/// A directory path plus a checkbox list of `.hx` shard/share files found in
/// it, for selecting which files to combine for a reconstruction/signing
/// attempt.
#[derive(Default)]
pub struct FileChecklist {
    pub dir: TextField,
    pub files: Vec<std::path::PathBuf>,
    pub checked: Vec<bool>,
    pub cursor: usize,
}

impl FileChecklist {
    /// Re-scan `dir` for `.hx` files, resetting all checkboxes.
    pub fn rescan(&mut self) {
        let dir = std::path::PathBuf::from(self.dir.value());
        let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "hx"))
            .collect();
        entries.sort();
        self.checked = vec![false; entries.len()];
        self.files = entries;
        self.cursor = 0;
    }

    pub fn selected(&self) -> Vec<std::path::PathBuf> {
        self.files
            .iter()
            .zip(&self.checked)
            .filter(|(_, c)| **c)
            .map(|(p, _)| p.clone())
            .collect()
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        if self.cursor + 1 < self.files.len() {
            self.cursor += 1;
        }
    }

    pub fn toggle(&mut self) {
        if let Some(c) = self.checked.get_mut(self.cursor) {
            *c = !*c;
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, focused: bool) {
        let items: Vec<ListItem> = self
            .files
            .iter()
            .zip(&self.checked)
            .enumerate()
            .map(|(i, (p, checked))| {
                let mark = if *checked { "[x]" } else { "[ ]" };
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let style = if focused && i == self.cursor {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                ListItem::new(format!("{mark} {name}")).style(style)
            })
            .collect();
        let selected_count = self.checked.iter().filter(|c| **c).count();
        let title = if self.files.is_empty() {
            "shard files (none found — Enter to (re)scan the directory above)".to_string()
        } else {
            format!("shard files — Space to toggle ({selected_count} selected)")
        };
        let border_style = if focused {
            Style::default().fg(theme::ACCENT)
        } else {
            Style::default()
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style);
        frame.render_widget(List::new(items).block(block), area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    /// A masked field must never render the typed characters — only a
    /// `*`-run of the same length. This is the one property the whole
    /// key-material design leans on for password entry.
    #[test]
    fn masked_field_never_renders_typed_text() {
        let mut field = TextField::masked();
        for c in "hunter2".chars() {
            field.handle_key(key(c));
        }
        assert_eq!(field.value(), "hunter2");

        let backend = TestBackend::new(30, 3);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| field.render(frame, frame.area(), "password", true))
            .expect("draw");

        let rendered = buffer_text(terminal.backend().buffer());
        assert!(!rendered.contains("hunter2"));
        assert!(rendered.contains("*******"));
    }

    /// An unmasked field renders the value as typed.
    #[test]
    fn unmasked_field_renders_typed_text() {
        let mut field = TextField::default();
        for c in "hello".chars() {
            field.handle_key(key(c));
        }
        let backend = TestBackend::new(30, 3);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| field.render(frame, frame.area(), "field", false))
            .expect("draw");
        assert!(buffer_text(terminal.backend().buffer()).contains("hello"));
    }

    #[test]
    fn file_checklist_rescan_finds_hx_files_and_resets_checks() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("shard-1.hx"), b"x").expect("write");
        std::fs::write(dir.path().join("shard-2.hx"), b"x").expect("write");
        std::fs::write(dir.path().join("notes.txt"), b"x").expect("write");

        let mut list = FileChecklist::default();
        list.dir
            .set_value(dir.path().to_string_lossy().into_owned());
        list.rescan();

        assert_eq!(list.files.len(), 2);
        assert!(list.files.iter().all(|p| p.extension().unwrap() == "hx"));
        assert_eq!(list.selected().len(), 0);

        list.toggle();
        assert_eq!(list.selected().len(), 1);
        list.move_down();
        list.toggle();
        assert_eq!(list.selected().len(), 2);

        // Re-scanning resets selection state.
        list.rescan();
        assert_eq!(list.selected().len(), 0);
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn file_checklist_cursor_does_not_move_past_bounds() {
        let mut list = FileChecklist::default();
        list.files = vec![std::path::PathBuf::from("a.hx")];
        list.checked = vec![false];
        list.move_up(); // already at 0, must not underflow
        assert_eq!(list.cursor, 0);
        list.move_down(); // only one file, must not move
        assert_eq!(list.cursor, 0);
    }

    fn buffer_text(buffer: &ratatui::buffer::Buffer) -> String {
        let area = buffer.area();
        let mut out = String::new();
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                out.push_str(buffer[(x, y)].symbol());
            }
        }
        out
    }
}
