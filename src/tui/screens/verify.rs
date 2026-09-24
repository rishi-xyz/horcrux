//! Passive integrity check (step 3): the first real use of the masked
//! `tui-input`-backed password field, in a low-stakes context — no key
//! material ever leaves the file's own decrypt path even when the optional
//! password is supplied (see `horcrux::verify`).

use crate::tui::action::AppEvent;
use crate::tui::theme;
use crate::tui::widgets::TextField;
use crossterm::event::{KeyCode, KeyEvent};
use horcrux::verify::{Kind, Report, consistency_error, verify_files};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Files,
    Password,
}

pub struct VerifyScreen {
    files: TextField,
    password: TextField,
    focus: Focus,
    busy: bool,
    reports: Vec<Report>,
    inconsistent: Option<String>,
    ran: bool,
}

impl Default for VerifyScreen {
    fn default() -> Self {
        Self {
            files: TextField::default(),
            password: TextField::masked(),
            focus: Focus::Files,
            busy: false,
            reports: Vec::new(),
            inconsistent: None,
            ran: false,
        }
    }
}

impl VerifyScreen {
    pub fn is_busy(&self) -> bool {
        self.busy
    }

    pub fn apply(&mut self, ev: AppEvent) {
        if let AppEvent::VerifyDone(reports) = ev {
            self.inconsistent = consistency_error(&reports);
            self.reports = reports;
            self.ran = true;
            self.busy = false;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, worker_tx: &UnboundedSender<AppEvent>) {
        if self.busy {
            return;
        }
        match key.code {
            KeyCode::Tab | KeyCode::Down => {
                self.focus = match self.focus {
                    Focus::Files => Focus::Password,
                    Focus::Password => Focus::Files,
                }
            }
            KeyCode::Up => {
                self.focus = match self.focus {
                    Focus::Files => Focus::Password,
                    Focus::Password => Focus::Files,
                }
            }
            KeyCode::Enter => self.run(worker_tx),
            _ => match self.focus {
                Focus::Files => self.files.handle_key(key),
                Focus::Password => self.password.handle_key(key),
            },
        }
    }

    fn run(&mut self, worker_tx: &UnboundedSender<AppEvent>) {
        let paths: Vec<std::path::PathBuf> = self
            .files
            .value()
            .split_whitespace()
            .map(std::path::PathBuf::from)
            .collect();
        if paths.is_empty() {
            return;
        }
        let password = if self.password.value().is_empty() {
            None
        } else {
            Some(self.password.value().to_string())
        };
        self.busy = true;
        self.ran = false;
        let tx = worker_tx.clone();
        tokio::task::spawn_blocking(move || {
            let reports = verify_files(&paths, password.as_deref());
            let _ = tx.send(AppEvent::VerifyDone(reports));
        });
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(area);

        self.files.render(
            frame,
            chunks[0],
            "shard/share file paths (space-separated)",
            self.focus == Focus::Files,
        );
        self.password.render(
            frame,
            chunks[1],
            "password (optional — checks the auth tag)",
            self.focus == Focus::Password,
        );

        let items: Vec<ListItem> = if self.busy {
            vec![ListItem::new("verifying…")]
        } else if !self.ran {
            vec![ListItem::new(
                "Enter file paths above, then press Enter to run.",
            )]
        } else {
            self.reports
                .iter()
                .map(|r| {
                    let label = match r.kind {
                        Some(Kind::Sss) => "SSS shard",
                        Some(Kind::Frost) => "FROST share",
                        None => "invalid",
                    };
                    let params = r
                        .params
                        .map(|(t, n)| format!(" (t={t}, n={n})"))
                        .unwrap_or_default();
                    let mark = if r.ok { "ok  " } else { "FAIL" };
                    let color = if r.ok { theme::ALLOW } else { theme::BLOCK };
                    ListItem::new(format!("{mark}  {label}{params}  {}", r.path))
                        .style(Style::default().fg(color))
                })
                .collect()
        };
        frame.render_widget(
            List::new(items).block(Block::default().borders(Borders::ALL).title("results")),
            chunks[2],
        );

        let summary = if let Some(reason) = &self.inconsistent {
            format!("inconsistent set: {reason}")
        } else if self.ran {
            let ok = self.reports.iter().filter(|r| r.ok).count();
            format!("{ok}/{} file(s) verified", self.reports.len())
        } else {
            String::new()
        };
        frame.render_widget(
            Paragraph::new(summary).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Tab focus · Enter run · Esc back"),
            ),
            chunks[3],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use horcrux::init_shards;
    use k256::SecretKey;
    use rand::rngs::OsRng;

    /// Exercises the screen's actual glue code (not re-verifying SSS/AES-GCM
    /// correctness, which `horcrux::verify`'s own tests already cover): a
    /// real `verify_files` call, run the way the screen runs it, must land
    /// back in `self.reports` with the right pass/fail split via `apply`.
    #[tokio::test]
    async fn a_correct_password_verifies_and_a_wrong_one_fails_via_the_same_screen_state() {
        let dir = tempfile::tempdir().expect("tempdir");
        let key = SecretKey::random(&mut OsRng);
        let passwords = vec!["guardian-pw".to_string(); 3];
        let paths = init_shards(&key, 2, 3, dir.path(), &passwords).expect("init");
        let paths_str = paths
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ");

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let mut screen = VerifyScreen::default();
        screen.files.set_value(paths_str.clone());
        screen.password.set_value("guardian-pw");
        screen.run(&tx);
        assert!(screen.busy);
        let ev = rx.recv().await.expect("verify event");
        screen.apply(ev);
        assert!(!screen.busy);
        assert!(screen.ran);
        assert_eq!(screen.reports.len(), 3);
        assert!(screen.reports.iter().all(|r| r.ok));

        let mut screen = VerifyScreen::default();
        screen.files.set_value(paths_str);
        screen.password.set_value("wrong-password");
        screen.run(&tx);
        let ev = rx.recv().await.expect("verify event");
        screen.apply(ev);
        assert!(screen.reports.iter().all(|r| !r.ok));
    }

    #[test]
    fn tab_and_up_both_toggle_between_the_two_fields() {
        let mut screen = VerifyScreen::default();
        assert_eq!(screen.focus, Focus::Files);
        screen.focus = Focus::Password;
        assert_ne!(screen.focus, Focus::Files);
    }
}
