//! Init screen (step 4): split a key into encrypted shard files (Mode A).
//!
//! First screen touching real key material. A pasted key is entered through
//! a masked field (never rendered in the clear while typing). A freshly
//! **generated** disposable test key is the one deliberate, scoped exception
//! to the "never render key material" rule (see the plan's key-material
//! decision) — shown once, exactly mirroring the CLI's existing one-time
//! `println!("Generated test key: ...")`, then never redisplayed.

use crate::tui::action::{AppEvent, SplitOutcome};
use crate::tui::theme;
use crate::tui::widgets::TextField;
use crossterm::event::{KeyCode, KeyEvent};
use horcrux::error::Error;
use k256::SecretKey;
use rand::rngs::OsRng;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Generate,
    KeyHex,
    Threshold,
    Shares,
    OutDir,
    Password,
}
const FOCUS_ORDER: [Focus; 6] = [
    Focus::Generate,
    Focus::KeyHex,
    Focus::Threshold,
    Focus::Shares,
    Focus::OutDir,
    Focus::Password,
];

pub struct InitScreen {
    generate: bool,
    key_hex: TextField,
    threshold: TextField,
    shares: TextField,
    out_dir: TextField,
    password: TextField,
    focus: Focus,
    busy: bool,
    result: Option<Result<SplitOutcome, Error>>,
}

impl Default for InitScreen {
    fn default() -> Self {
        let mut threshold = TextField::default();
        threshold.set_value("2");
        let mut shares = TextField::default();
        shares.set_value("3");
        let mut out_dir = TextField::default();
        out_dir.set_value("shards");
        Self {
            generate: false,
            key_hex: TextField::masked(),
            threshold,
            shares,
            out_dir,
            password: TextField::masked(),
            focus: Focus::Generate,
            busy: false,
            result: None,
        }
    }
}

impl InitScreen {
    pub fn is_busy(&self) -> bool {
        self.busy
    }

    pub fn apply(&mut self, ev: AppEvent) {
        if let AppEvent::SplitDone(result) = ev {
            self.result = Some(result);
            self.busy = false;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, worker_tx: &UnboundedSender<AppEvent>) {
        if self.busy {
            return;
        }
        match key.code {
            KeyCode::Tab | KeyCode::Down => self.move_focus(1),
            KeyCode::Up => self.move_focus(-1),
            KeyCode::Char(' ') if self.focus == Focus::Generate => self.generate = !self.generate,
            KeyCode::Enter => {
                if self.focus == Focus::Generate {
                    self.generate = !self.generate;
                } else {
                    self.run(worker_tx);
                }
            }
            _ => match self.focus {
                Focus::Generate => {}
                Focus::KeyHex => self.key_hex.handle_key(key),
                Focus::Threshold => self.threshold.handle_key(key),
                Focus::Shares => self.shares.handle_key(key),
                Focus::OutDir => self.out_dir.handle_key(key),
                Focus::Password => self.password.handle_key(key),
            },
        }
    }

    fn move_focus(&mut self, dir: i32) {
        let idx = FOCUS_ORDER
            .iter()
            .position(|f| *f == self.focus)
            .unwrap_or(0) as i32;
        let n = FOCUS_ORDER.len() as i32;
        let next = ((idx + dir) % n + n) % n;
        self.focus = FOCUS_ORDER[next as usize];
    }

    fn run(&mut self, worker_tx: &UnboundedSender<AppEvent>) {
        let threshold: u8 = match self.threshold.value().trim().parse() {
            Ok(v) => v,
            Err(_) => return,
        };
        let shares: u8 = match self.shares.value().trim().parse() {
            Ok(v) => v,
            Err(_) => return,
        };
        let out_dir = std::path::PathBuf::from(if self.out_dir.value().is_empty() {
            "shards"
        } else {
            self.out_dir.value()
        });
        let password = self.password.value().to_string();
        let generate = self.generate;
        let key_hex = self.key_hex.value().to_string();

        self.busy = true;
        self.result = None;
        let tx = worker_tx.clone();
        tokio::task::spawn_blocking(move || {
            let (key, generated_key_hex) = if generate {
                let key = SecretKey::random(&mut OsRng);
                let hex = hex::encode(key.to_bytes());
                (key, Some(hex))
            } else {
                match parse_key(&key_hex) {
                    Ok(k) => (k, None),
                    Err(e) => {
                        let _ = tx.send(AppEvent::SplitDone(Err(e)));
                        return;
                    }
                }
            };
            let passwords = vec![password; shares as usize];
            let outcome =
                horcrux::init_shards(&key, threshold, shares, &out_dir, &passwords).map(|paths| {
                    SplitOutcome {
                        paths,
                        group_path: None,
                        generated_key_hex,
                    }
                });
            let _ = tx.send(AppEvent::SplitDone(outcome));
        });
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(area);

        let gen_label = format!(
            "[{}] generate a disposable test key (Space/Enter to toggle)",
            if self.generate { "x" } else { " " }
        );
        let gen_style = if self.focus == Focus::Generate {
            Style::default().fg(theme::ACCENT)
        } else {
            Style::default()
        };
        frame.render_widget(
            Paragraph::new(gen_label)
                .style(gen_style)
                .block(Block::default().borders(Borders::ALL)),
            chunks[0],
        );

        let key_label = if self.generate {
            "private key hex (disabled — generating instead)"
        } else {
            "private key hex (64 hex chars, optional 0x)"
        };
        self.key_hex
            .render(frame, chunks[1], key_label, self.focus == Focus::KeyHex);
        self.threshold.render(
            frame,
            chunks[2],
            "threshold",
            self.focus == Focus::Threshold,
        );
        self.shares
            .render(frame, chunks[3], "shares", self.focus == Focus::Shares);
        self.out_dir.render(
            frame,
            chunks[4],
            "output directory",
            self.focus == Focus::OutDir,
        );
        self.password.render(
            frame,
            chunks[5],
            "password (used for every shard)",
            self.focus == Focus::Password,
        );

        let items: Vec<ListItem> = if self.busy {
            vec![ListItem::new("splitting…")]
        } else {
            match &self.result {
                None => vec![ListItem::new(
                    "Tab to move between fields, Enter to run (or toggle generate), Esc back.",
                )],
                Some(Err(e)) => {
                    vec![
                        ListItem::new(format!("error: {e}"))
                            .style(Style::default().fg(theme::BLOCK)),
                    ]
                }
                Some(Ok(outcome)) => {
                    let mut lines = Vec::new();
                    if let Some(hex) = &outcome.generated_key_hex {
                        lines.push(
                            ListItem::new(format!("Generated test key (shown once): 0x{hex}"))
                                .style(Style::default().fg(theme::WARN)),
                        );
                    }
                    lines.push(
                        ListItem::new(format!("Wrote {} shard(s):", outcome.paths.len()))
                            .style(Style::default().fg(theme::ALLOW)),
                    );
                    for p in &outcome.paths {
                        lines.push(ListItem::new(format!("  {}", p.display())));
                    }
                    lines
                }
            }
        };
        frame.render_widget(
            List::new(items).block(Block::default().borders(Borders::ALL).title("result")),
            chunks[6],
        );
    }
}

fn parse_key(hex_key: &str) -> Result<SecretKey, Error> {
    let stripped = hex_key.strip_prefix("0x").unwrap_or(hex_key);
    let bytes =
        hex::decode(stripped).map_err(|e| Error::InvalidKey(format!("not valid hex: {e}")))?;
    if bytes.len() != 32 {
        return Err(Error::InvalidKey(format!(
            "expected 32 bytes, got {}",
            bytes.len()
        )));
    }
    SecretKey::from_slice(&bytes).map_err(|e| Error::InvalidKey(e.to_string()))
}
