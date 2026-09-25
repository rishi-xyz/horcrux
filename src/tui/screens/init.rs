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
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use horcrux::device::{self, DriveInfo};
use horcrux::error::Error;
use k256::SecretKey;
use rand::rngs::OsRng;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};
use std::path::PathBuf;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Generate,
    KeyHex,
    Threshold,
    Shares,
    OutDir,
    Dest(usize),
    CustomPath(usize),
    Password(usize),
}

/// Where one guardian's shard should be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DestChoice {
    /// `out_dir/shard-{n}.hx` on this machine, same as before this feature.
    ThisDevice,
    /// A detected removable drive, indexed into `InitScreen::drives`.
    Drive(usize),
    /// A user-typed path (e.g. a network share, or a drive this OS didn't
    /// report as removable).
    Custom,
}

/// One guardian's destination choice plus its custom-path field (populated
/// only when `choice == Custom`).
struct GuardianDest {
    choice: DestChoice,
    custom_path: TextField,
}

impl Default for GuardianDest {
    fn default() -> Self {
        Self {
            choice: DestChoice::ThisDevice,
            custom_path: TextField::default(),
        }
    }
}

/// A shard rendered as a scannable terminal QR code, paged by frame (for
/// multi-frame payloads) and by which written shard is being shown.
struct QrOverlay {
    shard_index: usize,
    frame_index: usize,
    pages: Vec<String>,
}

pub struct InitScreen {
    generate: bool,
    key_hex: TextField,
    threshold: TextField,
    shares: TextField,
    out_dir: TextField,
    /// One password per shard, kept in sync with `shares` (see
    /// `sync_guardians`). Each guardian gets their own distinct password,
    /// matching the CLI's `collect_passwords` behavior — a single shared
    /// password here was the root cause of shards created with per-guardian
    /// passwords being unreconstructable from the TUI.
    passwords: Vec<TextField>,
    /// One destination per shard, kept in sync with `shares` alongside
    /// `passwords`: "this device" (a shared `out_dir`), a detected removable
    /// drive, or a custom path — so each guardian's shard can be written
    /// straight to their own USB key instead of one shared directory.
    destinations: Vec<GuardianDest>,
    /// Removable drives detected when this screen was entered. Refresh with
    /// 'r' while a destination field is focused (e.g. after plugging one in).
    drives: Vec<DriveInfo>,
    qr_view: Option<QrOverlay>,
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
        let mut screen = Self {
            generate: false,
            key_hex: TextField::masked(),
            threshold,
            shares,
            out_dir,
            passwords: Vec::new(),
            destinations: Vec::new(),
            drives: device::list_removable_drives(),
            qr_view: None,
            focus: Focus::Generate,
            busy: false,
            result: None,
        };
        screen.sync_guardians();
        screen
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

    /// Keep `passwords` and `destinations` sized to the current `shares`
    /// value, preserving already-typed passwords/destinations at surviving
    /// indices. Called on every edit to `shares` and before the split runs,
    /// so the field lists are always correct without a separate "apply"
    /// step.
    fn sync_guardians(&mut self) {
        let parsed = self.shares.value().trim().parse::<usize>().unwrap_or(0);
        let n = if parsed == 0 {
            self.passwords.len()
        } else {
            parsed
        };
        self.passwords.resize_with(n, TextField::masked);
        self.destinations.resize_with(n, GuardianDest::default);
    }

    fn order(&self) -> Vec<Focus> {
        let mut order = vec![
            Focus::Generate,
            Focus::KeyHex,
            Focus::Threshold,
            Focus::Shares,
            Focus::OutDir,
        ];
        for i in 0..self.passwords.len() {
            order.push(Focus::Dest(i));
            if self.destinations.get(i).is_some_and(|d| d.choice == DestChoice::Custom) {
                order.push(Focus::CustomPath(i));
            }
            order.push(Focus::Password(i));
        }
        order
    }

    /// Cycle guardian `i`'s destination: this device -> each detected
    /// removable drive (in `drives` order) -> a custom path -> back to this
    /// device.
    fn cycle_dest(&mut self, i: usize, dir: i32) {
        let Some(dest) = self.destinations.get_mut(i) else {
            return;
        };
        let total = self.drives.len() as i32 + 2; // ThisDevice + drives + Custom
        let current = match dest.choice {
            DestChoice::ThisDevice => 0,
            DestChoice::Drive(d) => 1 + d as i32,
            DestChoice::Custom => total - 1,
        };
        let next = (current + dir).rem_euclid(total);
        dest.choice = if next == 0 {
            DestChoice::ThisDevice
        } else if next == total - 1 {
            DestChoice::Custom
        } else {
            DestChoice::Drive((next - 1) as usize)
        };
    }

    fn dest_label(&self, i: usize) -> String {
        match self.destinations.get(i).map(|d| d.choice) {
            Some(DestChoice::ThisDevice) => "< this device >".to_string(),
            Some(DestChoice::Drive(idx)) => match self.drives.get(idx) {
                Some(d) => format!(
                    "< USB: {} ({:.1} GB free) >",
                    d.name,
                    d.available_bytes as f64 / 1e9
                ),
                None => "< drive unplugged — press r to rescan >".to_string(),
            },
            Some(DestChoice::Custom) => "< custom path >".to_string(),
            None => String::new(),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, worker_tx: &UnboundedSender<AppEvent>) {
        if self.busy {
            return;
        }
        if self.qr_view.is_some() {
            self.handle_qr_view_key(key);
            return;
        }
        if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.open_qr_view();
            return;
        }
        match key.code {
            KeyCode::Tab | KeyCode::Down if !matches!(self.focus, Focus::Dest(_)) => {
                self.move_focus(1)
            }
            KeyCode::Up if !matches!(self.focus, Focus::Dest(_)) => self.move_focus(-1),
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
                Focus::Shares => {
                    self.shares.handle_key(key);
                    self.sync_guardians();
                }
                Focus::OutDir => self.out_dir.handle_key(key),
                Focus::Dest(i) => match key.code {
                    KeyCode::Left => self.cycle_dest(i, -1),
                    KeyCode::Right => self.cycle_dest(i, 1),
                    KeyCode::Char(' ') => self.cycle_dest(i, 1),
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        self.drives = device::list_removable_drives();
                    }
                    KeyCode::Tab => self.move_focus(1),
                    KeyCode::Down => self.move_focus(1),
                    KeyCode::Up => self.move_focus(-1),
                    _ => {}
                },
                Focus::CustomPath(i) => {
                    if let Some(dest) = self.destinations.get_mut(i) {
                        dest.custom_path.handle_key(key);
                    }
                }
                Focus::Password(i) => {
                    if let Some(field) = self.passwords.get_mut(i) {
                        field.handle_key(key);
                    }
                }
            },
        }
    }

    fn move_focus(&mut self, dir: i32) {
        self.sync_guardians();
        let order = self.order();
        let idx = order.iter().position(|f| *f == self.focus).unwrap_or(0) as i32;
        let n = order.len() as i32;
        let next = ((idx + dir) % n + n) % n;
        self.focus = order[next as usize];
    }

    fn result_path_count(&self) -> usize {
        match &self.result {
            Some(Ok(outcome)) => outcome.paths.len(),
            _ => 0,
        }
    }

    fn open_qr_view(&mut self) {
        if self.result_path_count() == 0 {
            return;
        }
        self.qr_view = Some(QrOverlay {
            shard_index: 0,
            frame_index: 0,
            pages: Vec::new(),
        });
        self.reload_qr_pages();
    }

    fn reload_qr_pages(&mut self) {
        let path = match (&self.result, &self.qr_view) {
            (Some(Ok(outcome)), Some(overlay)) => outcome.paths.get(overlay.shard_index).cloned(),
            _ => None,
        };
        let pages = match path {
            Some(p) => {
                load_qr_pages(&p).unwrap_or_else(|e| vec![format!("failed to render QR: {e}")])
            }
            None => vec!["no shard to display".to_string()],
        };
        if let Some(overlay) = &mut self.qr_view {
            overlay.pages = pages;
            overlay.frame_index = 0;
        }
    }

    fn handle_qr_view_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Esc {
            self.qr_view = None;
            return;
        }
        let mut reload_shard = None;
        if let Some(overlay) = self.qr_view.as_mut() {
            match key.code {
                KeyCode::Left if !overlay.pages.is_empty() => {
                    overlay.frame_index =
                        (overlay.frame_index + overlay.pages.len() - 1) % overlay.pages.len();
                }
                KeyCode::Right if !overlay.pages.is_empty() => {
                    overlay.frame_index = (overlay.frame_index + 1) % overlay.pages.len();
                }
                KeyCode::Up if overlay.shard_index > 0 => {
                    reload_shard = Some(overlay.shard_index - 1);
                }
                KeyCode::Down => {
                    reload_shard = Some(overlay.shard_index + 1);
                }
                _ => {}
            }
        }
        if let Some(idx) = reload_shard
            && idx < self.result_path_count()
        {
            if let Some(overlay) = self.qr_view.as_mut() {
                overlay.shard_index = idx;
            }
            self.reload_qr_pages();
        }
    }

    fn run(&mut self, worker_tx: &UnboundedSender<AppEvent>) {
        self.sync_guardians();
        let threshold: u8 = match self.threshold.value().trim().parse() {
            Ok(v) => v,
            Err(_) => return,
        };
        let shares: u8 = match self.shares.value().trim().parse() {
            Ok(v) => v,
            Err(_) => return,
        };
        let out_dir = PathBuf::from(if self.out_dir.value().is_empty() {
            "shards"
        } else {
            self.out_dir.value()
        });
        let passwords: Vec<String> = self
            .passwords
            .iter()
            .map(|f| f.value().to_string())
            .collect();
        // Resolve each guardian's destination now, while `self` is still
        // available: "this device" shares `out_dir` (same as before this
        // feature); a detected drive gets a namespaced `horcrux/` subfolder
        // so the tool never litters the drive's root; a custom path is
        // treated as a directory. The filename uses the guardian's position
        // (1-based) as a human label — the share's real id, embedded in the
        // shard file itself, is assigned during the split below and is what
        // decryption/audit actually key off, not this filename.
        let all_this_device = self
            .destinations
            .iter()
            .all(|d| d.choice == DestChoice::ThisDevice);
        let destinations: Vec<PathBuf> = (0..self.passwords.len())
            .map(|i| {
                let name = format!("shard-{}.hx", i + 1);
                match self.destinations.get(i).map(|d| d.choice) {
                    Some(DestChoice::Drive(idx)) => match self.drives.get(idx) {
                        Some(d) => d.mount_point.join("horcrux").join(&name),
                        None => out_dir.join(&name),
                    },
                    Some(DestChoice::Custom) => {
                        let custom = self
                            .destinations
                            .get(i)
                            .map(|d| d.custom_path.value().to_string())
                            .unwrap_or_default();
                        if custom.trim().is_empty() {
                            out_dir.join(&name)
                        } else {
                            PathBuf::from(custom).join(&name)
                        }
                    }
                    _ => out_dir.join(&name),
                }
            })
            .collect();
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
            let outcome = if all_this_device {
                horcrux::init_shards(&key, threshold, shares, &out_dir, &passwords)
            } else {
                horcrux::init_shards_to(&key, threshold, shares, &passwords, &destinations)
            }
            .map(|paths| SplitOutcome {
                paths,
                group_path: None,
                generated_key_hex,
            });
            let _ = tx.send(AppEvent::SplitDone(outcome));
        });
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        // Each guardian contributes a Dest row, an optional CustomPath row
        // (only when Custom is selected), and a Password row.
        let mut guardian_rows: Vec<Focus> = Vec::new();
        for i in 0..self.passwords.len() {
            guardian_rows.push(Focus::Dest(i));
            if self.destinations.get(i).is_some_and(|d| d.choice == DestChoice::Custom) {
                guardian_rows.push(Focus::CustomPath(i));
            }
            guardian_rows.push(Focus::Password(i));
        }
        let field_rows = 5 + guardian_rows.len();
        let mut constraints = vec![Constraint::Length(3); field_rows];
        constraints.push(Constraint::Min(0));
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
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
        let n = self.passwords.len();
        for (row_offset, row) in guardian_rows.iter().enumerate() {
            let area = chunks[5 + row_offset];
            match *row {
                Focus::Dest(i) => {
                    let label = format!("destination for shard {} of {n} (Left/Right to change, r to rescan)", i + 1);
                    let style = if self.focus == Focus::Dest(i) {
                        Style::default().fg(theme::ACCENT)
                    } else {
                        Style::default()
                    };
                    frame.render_widget(
                        Paragraph::new(self.dest_label(i))
                            .style(style)
                            .block(Block::default().borders(Borders::ALL).title(label)),
                        area,
                    );
                }
                Focus::CustomPath(i) => {
                    if let Some(dest) = self.destinations.get(i) {
                        dest.custom_path.render(
                            frame,
                            area,
                            &format!("custom destination directory for shard {}", i + 1),
                            self.focus == Focus::CustomPath(i),
                        );
                    }
                }
                Focus::Password(i) => {
                    if let Some(field) = self.passwords.get(i) {
                        field.render(
                            frame,
                            area,
                            &format!("password for shard {} of {n} (distinct per guardian)", i + 1),
                            self.focus == Focus::Password(i),
                        );
                    }
                }
                _ => {}
            }
        }

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
                    lines.push(
                        ListItem::new("Ctrl+Q: show a shard as a scannable QR code")
                            .style(Style::default().fg(theme::MUTED)),
                    );
                    lines
                }
            }
        };
        frame.render_widget(
            List::new(items).block(Block::default().borders(Borders::ALL).title("result")),
            chunks[field_rows],
        );

        if let Some(overlay) = &self.qr_view {
            render_qr_overlay(frame, area, overlay, self.result_path_count());
        }
    }
}

/// Render a written shard file as one or more scannable terminal QR frames.
fn load_qr_pages(path: &std::path::Path) -> Result<Vec<String>, Error> {
    let bytes = std::fs::read(path).map_err(Error::Io)?;
    let session: [u8; horcrux::qr::SESSION_LEN] = rand::random();
    let frames = horcrux::qr::encode_message(horcrux::qr::MessageType::ShardFile, session, &bytes)?;
    Ok(frames.iter().map(horcrux::qr::terminal_qr).collect())
}

fn render_qr_overlay(frame: &mut Frame, area: Rect, overlay: &QrOverlay, total_shards: usize) {
    let text = overlay.pages.get(overlay.frame_index).cloned().unwrap_or_default();
    let content_w = text.lines().map(|l| l.chars().count()).max().unwrap_or(20) as u16;
    let content_h = text.lines().count() as u16;
    let width = (content_w + 4).clamp(24, area.width.saturating_sub(2));
    let height = (content_h + 5).clamp(12, area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup);
    let title = format!(
        "QR — shard {}/{total_shards} \u{b7} frame {}/{}",
        overlay.shard_index + 1,
        overlay.frame_index + 1,
        overlay.pages.len().max(1),
    );
    let mut body = text;
    body.push_str("\nLeft/Right: frame   Up/Down: shard   Esc: close");
    frame.render_widget(
        Paragraph::new(body).block(Block::default().borders(Borders::ALL).title(title)),
        popup,
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn cycle_dest_wraps_through_drives_and_custom_with_no_drives() {
        let mut screen = InitScreen::default();
        screen.drives = Vec::new();
        screen.sync_guardians();
        assert_eq!(screen.destinations[0].choice, DestChoice::ThisDevice);
        screen.cycle_dest(0, 1);
        assert_eq!(screen.destinations[0].choice, DestChoice::Custom);
        screen.cycle_dest(0, 1);
        assert_eq!(screen.destinations[0].choice, DestChoice::ThisDevice);
        screen.cycle_dest(0, -1);
        assert_eq!(screen.destinations[0].choice, DestChoice::Custom);
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn cycle_dest_visits_every_detected_drive_in_between() {
        let mut screen = InitScreen::default();
        screen.drives = vec![
            DriveInfo {
                name: "A".into(),
                mount_point: PathBuf::from("/mnt/a"),
                removable: true,
                total_bytes: 0,
                available_bytes: 0,
            },
            DriveInfo {
                name: "B".into(),
                mount_point: PathBuf::from("/mnt/b"),
                removable: true,
                total_bytes: 0,
                available_bytes: 0,
            },
        ];
        screen.sync_guardians();
        screen.cycle_dest(0, 1);
        assert_eq!(screen.destinations[0].choice, DestChoice::Drive(0));
        screen.cycle_dest(0, 1);
        assert_eq!(screen.destinations[0].choice, DestChoice::Drive(1));
        screen.cycle_dest(0, 1);
        assert_eq!(screen.destinations[0].choice, DestChoice::Custom);
        screen.cycle_dest(0, 1);
        assert_eq!(screen.destinations[0].choice, DestChoice::ThisDevice);
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn order_includes_custom_path_focus_only_when_custom_is_selected() {
        let mut screen = InitScreen::default();
        screen.drives = Vec::new();
        screen.sync_guardians();
        assert!(!screen.order().contains(&Focus::CustomPath(0)));
        screen.cycle_dest(0, -1); // wraps straight to Custom with no drives
        assert!(screen.order().contains(&Focus::CustomPath(0)));
    }
}
