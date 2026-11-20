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
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
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

/// A directory path plus a checkbox list of `.hx` shard/share files — or a
/// `.png` QR export of one (see `crate::qr::write_shard_qr`) — found in it,
/// for selecting which files to combine for a reconstruction/signing
/// attempt.
#[derive(Default)]
pub struct FileChecklist {
    pub dir: TextField,
    pub files: Vec<std::path::PathBuf>,
    pub checked: Vec<bool>,
    pub cursor: usize,
}

impl FileChecklist {
    /// Scan `dir` and add any files not already in the list, preserving
    /// existing entries and their checkbox/password state. Call this once
    /// per guardian's directory (typing a new path into `dir` and pressing
    /// Enter each time) to build a combined selection across several USB
    /// drives instead of being limited to one directory at a time.
    pub fn scan_add(&mut self) {
        let mut new_entries: Vec<std::path::PathBuf> = scan_dir(self.dir.value())
            .into_iter()
            .filter(|p| !self.files.contains(p))
            .collect();
        new_entries.sort();
        self.checked
            .extend(std::iter::repeat_n(false, new_entries.len()));
        self.files.extend(new_entries);
    }

    /// Empty the list entirely (e.g. to start a fresh multi-drive selection
    /// from scratch).
    pub fn clear(&mut self) {
        self.files.clear();
        self.checked.clear();
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

    /// Render the checklist. Uses a stateful `List` (rather than a plain
    /// one) so ratatui automatically scrolls the viewport to keep the
    /// cursor visible when there are more files than fit the given `area`
    /// — a directory (or a multi-drive selection built with `scan_add`)
    /// with many files no longer gets silently cut off with no way to reach
    /// the entries past the bottom edge.
    pub fn render(&self, frame: &mut Frame, area: Rect, focused: bool) {
        let items: Vec<ListItem> = self
            .files
            .iter()
            .zip(&self.checked)
            .map(|(p, checked)| {
                let mark = if *checked { "[x]" } else { "[ ]" };
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let name = if p.extension().is_some_and(|e| e == "png") {
                    format!("[QR] {name}")
                } else {
                    name
                };
                ListItem::new(format!("{mark} {name}"))
            })
            .collect();
        let selected_count = self.checked.iter().filter(|c| **c).count();
        let title = if self.files.is_empty() {
            "shard files (none found — Enter adds this directory's files)".to_string()
        } else {
            format!(
                "shard files — Space toggle, Enter (on dir) adds more, c clears ({selected_count} selected, {} listed)",
                self.files.len()
            )
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
        // Only mark a row selected (reversed + auto-scrolled to) while this
        // widget actually has focus, matching the pre-scrolling behavior of
        // only highlighting the cursor when it's reachable by the keyboard.
        let mut state = ListState::default();
        if focused && !self.files.is_empty() {
            state.select(Some(self.cursor));
        }
        frame.render_stateful_widget(
            List::new(items)
                .block(block)
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
            area,
            &mut state,
        );
    }
}

/// Scan a directory for `.hx` shard/share files and `.png` QR exports of
/// one. Shared by [`FileChecklist::rescan`] and [`FileChecklist::scan_add`].
fn scan_dir(dir: &str) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(std::path::PathBuf::from(dir))
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "hx" || e == "png"))
        .collect()
}

/// Compute which contiguous slice `[start, end)` of `total` same-height rows
/// should be rendered so that `focused` stays inside a `capacity`-row
/// viewport — clamped so the window never runs past either end of the list.
/// Purely a function of the current focus and capacity (no separate scroll
/// state to keep in sync): tabbing through a long list of per-guardian rows
/// naturally scrolls the visible window along with it.
///
/// A computed `capacity` of 0 (a terminal too short to fit even the fixed
/// fields around the scrollable section) is floored to 1 rather than
/// showing nothing: a cramped single row you can still Tab to and edit
/// beats a screen that silently hides every guardian field.
pub fn scroll_window(total: usize, focused: usize, capacity: usize) -> (usize, usize) {
    if total == 0 {
        return (0, 0);
    }
    let capacity = capacity.max(1);
    if total <= capacity {
        return (0, total);
    }
    let focused = focused.min(total - 1);
    let max_start = total - capacity;
    let start = focused.saturating_sub(capacity - 1).min(max_start);
    (start, start + capacity)
}

/// Render the shared audit Block/Warn modal, used identically by the Sign
/// and MPC (sign side) screens. Restates the consequence of each choice and
/// repeats the key hint above and below the reasons list, so it survives a
/// small terminal or a distracted skim — the modal is the one place a user
/// unfamiliar with the audit gate is most likely to get stuck.
pub fn render_audit_modal(frame: &mut Frame, area: Rect, blocking: bool, reasons: &[String]) {
    let width = area.width.saturating_sub(8).clamp(20, 70);
    let height = (reasons.len() as u16 + 9).min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup);
    let (title, color, banner, consequence, hint) = if blocking {
        (
            "AUDIT: BLOCKED",
            theme::BLOCK,
            "\u{26a0} AUDIT BLOCKED \u{2014} signing refused",
            "This attempt is already logged; forcing through logs it again as forced.",
            "f = force through (logged)   Esc = cancel",
        )
    } else {
        (
            "AUDIT: WARNING",
            theme::WARN,
            "\u{26a0} AUDIT WARNING \u{2014} review before continuing",
            "Continuing proceeds immediately; nothing is blocked.",
            "Enter = continue   Esc = cancel",
        )
    };

    let mut lines: Vec<ListItem> = vec![
        ListItem::new(banner).style(Style::default().fg(color).add_modifier(Modifier::BOLD)),
        ListItem::new(hint).style(Style::default().fg(theme::MUTED)),
        ListItem::new(""),
    ];
    lines.extend(
        reasons
            .iter()
            .map(|r| ListItem::new(format!("\u{2022} {r}"))),
    );
    lines.push(ListItem::new(""));
    lines.push(ListItem::new(consequence).style(Style::default().fg(theme::MUTED)));
    lines.push(ListItem::new(hint).style(Style::default().fg(theme::MUTED)));

    frame.render_widget(
        List::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(color)),
        ),
        popup,
    );
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
    fn file_checklist_scan_add_finds_hx_and_png_but_not_other_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("shard-1.hx"), b"x").expect("write");
        std::fs::write(dir.path().join("shard-2.hx"), b"x").expect("write");
        std::fs::write(dir.path().join("qr-export.png"), b"x").expect("write");
        std::fs::write(dir.path().join("notes.txt"), b"x").expect("write");

        let mut list = FileChecklist::default();
        list.dir
            .set_value(dir.path().to_string_lossy().into_owned());
        list.scan_add();

        assert_eq!(list.files.len(), 3);
        assert!(
            list.files
                .iter()
                .all(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("hx" | "png")))
        );
        assert_eq!(list.selected().len(), 0);

        list.toggle();
        assert_eq!(list.selected().len(), 1);
        list.move_down();
        list.toggle();
        assert_eq!(list.selected().len(), 2);
    }

    #[test]
    fn file_checklist_scan_add_accumulates_across_directories_without_losing_selection() {
        let dir_a = tempfile::tempdir().expect("tempdir a");
        let dir_b = tempfile::tempdir().expect("tempdir b");
        std::fs::write(dir_a.path().join("shard-1.hx"), b"x").expect("write");
        std::fs::write(dir_b.path().join("shard-2.hx"), b"x").expect("write");

        let mut list = FileChecklist::default();
        list.dir
            .set_value(dir_a.path().to_string_lossy().into_owned());
        list.scan_add();
        assert_eq!(list.files.len(), 1);
        list.toggle(); // check shard-1 from drive A

        // Scanning a second directory (a different guardian's drive) must
        // add its files without clearing what drive A already contributed.
        list.dir
            .set_value(dir_b.path().to_string_lossy().into_owned());
        list.scan_add();
        assert_eq!(list.files.len(), 2);
        assert_eq!(list.selected().len(), 1, "drive A's checkbox must survive");

        // Re-scanning the same directory again must not duplicate entries.
        list.scan_add();
        assert_eq!(list.files.len(), 2);
    }

    #[test]
    fn file_checklist_clear_empties_the_list() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("shard-1.hx"), b"x").expect("write");
        let mut list = FileChecklist::default();
        list.dir
            .set_value(dir.path().to_string_lossy().into_owned());
        list.scan_add();
        list.toggle();
        list.clear();
        assert!(list.files.is_empty());
        assert!(list.checked.is_empty());
        assert_eq!(list.cursor, 0);
    }

    #[test]
    fn scroll_window_keeps_focused_row_inside_a_small_viewport() {
        assert_eq!(scroll_window(10, 0, 3), (0, 3));
        assert_eq!(scroll_window(10, 2, 3), (0, 3));
        assert_eq!(scroll_window(10, 3, 3), (1, 4));
        assert_eq!(scroll_window(10, 9, 3), (7, 10));
        assert_eq!(
            scroll_window(5, 4, 10),
            (0, 5),
            "fits entirely, no windowing"
        );
        assert_eq!(scroll_window(0, 0, 3), (0, 0));
        assert_eq!(
            scroll_window(10, 5, 0),
            (5, 6),
            "a zero capacity (terminal too short for the fixed fields around it) still shows one row"
        );
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
