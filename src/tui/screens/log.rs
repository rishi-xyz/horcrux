//! Access log viewer (step 2): read-only, touches no key material and no
//! passwords — the first real feature screen, validating the
//! "call `lib.rs`, render a color-coded table" pattern before anything
//! password-related exists.

use crate::tui::theme;
use crossterm::event::{KeyCode, KeyEvent};
use horcrux::audit::{AccessLog, Entry, EntryKind};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

#[derive(Default)]
pub struct LogScreen {
    entries: Vec<Entry>,
    scroll: usize,
    error: Option<String>,
}

impl LogScreen {
    /// Re-read the access log at its default resolved path (`--log-file` has
    /// no TUI equivalent yet; `$HORCRUX_ACCESS_LOG` still applies).
    pub fn reload(&mut self) {
        let log = AccessLog::open(horcrux::audit::resolve_log_path(None));
        match log.read_all() {
            Ok(mut entries) => {
                entries.reverse(); // newest first
                self.entries = entries;
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        self.scroll = 0;
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                if self.scroll + 1 < self.entries.len() {
                    self.scroll += 1;
                }
            }
            KeyCode::Char('r') => self.reload(),
            _ => {}
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if let Some(err) = &self.error {
            frame.render_widget(
                Paragraph::new(format!("failed to read access log: {err}"))
                    .style(Style::default().fg(theme::BLOCK))
                    .block(Block::default().borders(Borders::ALL).title("access log")),
                area,
            );
            return;
        }

        let visible_height = area.height.saturating_sub(2) as usize;
        let items: Vec<ListItem> = self
            .entries
            .iter()
            .skip(self.scroll)
            .take(visible_height.max(1))
            .map(|e| {
                let kind = match e.kind {
                    EntryKind::DecryptOk => "ok",
                    EntryKind::DecryptFail => "fail",
                    EntryKind::Blocked => "blocked",
                    EntryKind::Signed => "signed",
                };
                let line = format!(
                    "{}  {kind:<7}  shard {:>2}",
                    horcrux::audit::format_utc(e.ts),
                    e.shard_id
                );
                ListItem::new(line).style(Style::default().fg(theme::entry_kind_color(e.kind)))
            })
            .collect();

        let title = if self.entries.is_empty() {
            "access log (empty) — Esc back".to_string()
        } else {
            format!(
                "access log — {} entries, newest first — ↑/↓ scroll, r reload, Esc back",
                self.entries.len()
            )
        };
        let block = Block::default().borders(Borders::ALL).title(title);
        frame.render_widget(List::new(items).block(block), area);
    }
}
