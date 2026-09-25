//! MPC screen (step 6): Mode B — dealer-split a key into FROST shares, then
//! sign with a threshold subset. Mirrors `agent/wayfinder/PLAN.md` Phase 4's
//! "key never assembled on any machine" framing, from the TUI. Solana only in
//! this MVP (matching the scope decision in the TUI plan).
//!
//! Shares the exact async pattern as the Sign screen (audit pre-flight ->
//! Warn/Block modal -> `spawn_blocking` for the FROST math -> optional
//! `tokio::spawn` broadcast); see `screens/sign.rs` for the rationale.

use crate::tui::action::{AppEvent, SplitOutcome};
use crate::tui::theme;
use crate::tui::widgets::{FileChecklist, TextField, render_audit_modal};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use horcrux::audit::{AccessLog, Entry, Scorer, Verdict};
use horcrux::device::{self, DriveInfo};
use horcrux::error::Error;
use k256::SecretKey;
use rand::rngs::OsRng;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Split,
    Sign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Mode,
    // split fields
    Generate,
    KeyHex,
    Threshold,
    Shares,
    SplitOutDir,
    SplitDest(usize),
    SplitCustomPath(usize),
    SplitPassword(usize),
    SplitSubmit,
    // sign fields
    GroupDir,
    ShareDir,
    Files,
    SignPassword(usize),
    To,
    Lamports,
    Blockhash,
    Broadcast,
    SignSubmit,
}

/// Where one guardian's FROST share should be written. See `screens::init`'s
/// equivalent for the rationale (this is a small, deliberate duplication —
/// this screen already duplicates plenty else against `init.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DestChoice {
    ThisDevice,
    Drive(usize),
    Custom,
}

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

/// A share rendered as a scannable terminal QR code, paged by frame and by
/// which written share is being shown. See `screens::init::QrOverlay`.
struct QrOverlay {
    share_index: usize,
    frame_index: usize,
    pages: Vec<String>,
}

struct Modal {
    blocking: bool,
    reasons: Vec<String>,
    attempt: u64,
    shares: Vec<PathBuf>,
    /// Passwords snapshotted at submit time, in the same order as `shares`.
    passwords: Vec<String>,
    /// Temp `.hx` files staged from any `.png` QR imports among `shares`,
    /// to be deleted however the modal is resolved. See `screens::sign`'s
    /// equivalent field.
    temp_files: Vec<PathBuf>,
}

pub struct MpcScreen {
    mode: Mode,
    focus: Focus,
    busy: bool,

    // split state
    generate: bool,
    key_hex: TextField,
    threshold: TextField,
    shares_count: TextField,
    split_out_dir: TextField,
    /// One password per share, kept in sync with `shares_count`. See
    /// `InitScreen`'s equivalent field for why a single shared password was
    /// wrong here.
    split_passwords: Vec<TextField>,
    /// One destination per share, kept in sync with `shares_count` alongside
    /// `split_passwords`. See `InitScreen`'s equivalent field.
    split_destinations: Vec<GuardianDest>,
    /// Removable drives detected when this screen was entered. Refresh with
    /// 'r' while a destination field is focused.
    drives: Vec<DriveInfo>,
    qr_view: Option<QrOverlay>,
    split_result: Option<Result<SplitOutcome, Error>>,

    // sign state
    group_dir: TextField,
    picker: FileChecklist,
    /// One password per currently-selected share file, keyed by path. See
    /// `SignScreen`'s equivalent field.
    password_by_path: HashMap<PathBuf, TextField>,
    to: TextField,
    lamports: TextField,
    blockhash: TextField,
    broadcast: bool,
    modal: Option<Modal>,
    signed: Option<Result<horcrux::SignedOutput, Error>>,
    broadcast_result: Option<Result<String, Error>>,
}

impl Default for MpcScreen {
    fn default() -> Self {
        let mut threshold = TextField::default();
        threshold.set_value("2");
        let mut shares_count = TextField::default();
        shares_count.set_value("3");
        let mut split_out_dir = TextField::default();
        split_out_dir.set_value("mpc");
        let mut group_dir = TextField::default();
        group_dir.set_value("mpc");
        let mut picker = FileChecklist::default();
        picker.dir.set_value("mpc");
        let mut screen = Self {
            mode: Mode::Split,
            focus: Focus::Mode,
            busy: false,
            generate: false,
            key_hex: TextField::masked(),
            threshold,
            shares_count,
            split_out_dir,
            split_passwords: Vec::new(),
            split_destinations: Vec::new(),
            drives: device::list_removable_drives(),
            qr_view: None,
            split_result: None,
            group_dir,
            picker,
            password_by_path: HashMap::new(),
            to: TextField::default(),
            lamports: TextField::default(),
            blockhash: TextField::default(),
            broadcast: false,
            modal: None,
            signed: None,
            broadcast_result: None,
        };
        screen.sync_split_guardians();
        screen
    }
}

impl MpcScreen {
    pub fn is_busy(&self) -> bool {
        self.busy
    }

    pub fn modal_open(&self) -> bool {
        self.modal.is_some()
    }

    pub fn apply(&mut self, ev: AppEvent) {
        match ev {
            AppEvent::SplitDone(result) => {
                self.split_result = Some(result);
                self.busy = false;
            }
            AppEvent::SignDone(result) => {
                self.signed = Some(result);
                if !self.broadcast {
                    self.busy = false;
                }
            }
            AppEvent::BroadcastDone(result) => {
                self.broadcast_result = Some(result);
                self.busy = false;
            }
            AppEvent::VerifyDone(_) => {}
        }
    }

    /// Keep `split_passwords` and `split_destinations` sized to
    /// `shares_count`, preserving already-typed values at surviving indices.
    fn sync_split_guardians(&mut self) {
        let parsed = self.shares_count.value().trim().parse::<usize>().unwrap_or(0);
        let n = if parsed == 0 {
            self.split_passwords.len()
        } else {
            parsed
        };
        self.split_passwords.resize_with(n, TextField::masked);
        self.split_destinations.resize_with(n, GuardianDest::default);
    }

    /// Cycle guardian `i`'s destination: this device -> each detected
    /// removable drive -> a custom path -> back to this device. See
    /// `InitScreen::cycle_dest`.
    fn cycle_split_dest(&mut self, i: usize, dir: i32) {
        let Some(dest) = self.split_destinations.get_mut(i) else {
            return;
        };
        let total = self.drives.len() as i32 + 2;
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

    fn split_dest_label(&self, i: usize) -> String {
        match self.split_destinations.get(i).map(|d| d.choice) {
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

    fn split_result_path_count(&self) -> usize {
        match &self.split_result {
            Some(Ok(outcome)) => outcome.paths.len(),
            _ => 0,
        }
    }

    fn open_qr_view(&mut self) {
        if self.split_result_path_count() == 0 {
            return;
        }
        self.qr_view = Some(QrOverlay {
            share_index: 0,
            frame_index: 0,
            pages: Vec::new(),
        });
        self.reload_qr_pages();
    }

    fn reload_qr_pages(&mut self) {
        let path = match (&self.split_result, &self.qr_view) {
            (Some(Ok(outcome)), Some(overlay)) => outcome.paths.get(overlay.share_index).cloned(),
            _ => None,
        };
        let pages = match path {
            Some(p) => {
                load_qr_pages(&p).unwrap_or_else(|e| vec![format!("failed to render QR: {e}")])
            }
            None => vec!["no share to display".to_string()],
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
        let mut reload_share = None;
        if let Some(overlay) = self.qr_view.as_mut() {
            match key.code {
                KeyCode::Left if !overlay.pages.is_empty() => {
                    overlay.frame_index =
                        (overlay.frame_index + overlay.pages.len() - 1) % overlay.pages.len();
                }
                KeyCode::Right if !overlay.pages.is_empty() => {
                    overlay.frame_index = (overlay.frame_index + 1) % overlay.pages.len();
                }
                KeyCode::Up if overlay.share_index > 0 => {
                    reload_share = Some(overlay.share_index - 1);
                }
                KeyCode::Down => {
                    reload_share = Some(overlay.share_index + 1);
                }
                _ => {}
            }
        }
        if let Some(idx) = reload_share
            && idx < self.split_result_path_count()
        {
            if let Some(overlay) = self.qr_view.as_mut() {
                overlay.share_index = idx;
            }
            self.reload_qr_pages();
        }
    }

    /// Ensure every currently-selected share has a password field. See
    /// `SignScreen::sync_passwords` for the equivalent and its rationale.
    fn sync_sign_passwords(&mut self) {
        for path in self.picker.selected() {
            self.password_by_path
                .entry(path)
                .or_insert_with(TextField::masked);
        }
    }

    fn order(&self) -> Vec<Focus> {
        match self.mode {
            Mode::Split => {
                let mut order = vec![
                    Focus::Generate,
                    Focus::KeyHex,
                    Focus::Threshold,
                    Focus::Shares,
                    Focus::SplitOutDir,
                ];
                for i in 0..self.split_passwords.len() {
                    order.push(Focus::SplitDest(i));
                    if self
                        .split_destinations
                        .get(i)
                        .is_some_and(|d| d.choice == DestChoice::Custom)
                    {
                        order.push(Focus::SplitCustomPath(i));
                    }
                    order.push(Focus::SplitPassword(i));
                }
                order.push(Focus::SplitSubmit);
                order
            }
            Mode::Sign => {
                let mut order = vec![Focus::GroupDir, Focus::ShareDir, Focus::Files];
                order.extend((0..self.picker.selected().len()).map(Focus::SignPassword));
                order.extend([
                    Focus::To,
                    Focus::Lamports,
                    Focus::Blockhash,
                    Focus::Broadcast,
                    Focus::SignSubmit,
                ]);
                order
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, worker_tx: &UnboundedSender<AppEvent>) {
        if self.busy {
            return;
        }
        if self.modal.is_some() {
            self.handle_modal_key(key, worker_tx);
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
        if self.focus == Focus::Mode {
            match key.code {
                KeyCode::Left | KeyCode::Right | KeyCode::Char(' ') | KeyCode::Enter => {
                    self.mode = match self.mode {
                        Mode::Split => Mode::Sign,
                        Mode::Sign => Mode::Split,
                    };
                }
                KeyCode::Tab => self.focus = self.order()[0],
                KeyCode::BackTab => self.focus = *self.order().last().unwrap(),
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Tab => self.move_focus(1),
            KeyCode::BackTab => self.move_focus(-1),
            _ => self.handle_field_key(key, worker_tx),
        }
    }

    fn move_focus(&mut self, dir: i32) {
        match self.mode {
            Mode::Split => self.sync_split_guardians(),
            Mode::Sign => self.sync_sign_passwords(),
        }
        let order = self.order();
        let idx = order.iter().position(|f| *f == self.focus).unwrap_or(0) as i32;
        let n = order.len() as i32;
        let next = idx + dir;
        if next < 0 || next >= n {
            // Tab past either end of a mode's field list wraps back to the
            // mode toggle, not around to the other end of the list.
            self.focus = Focus::Mode;
        } else {
            self.focus = order[next as usize];
        }
    }

    fn handle_field_key(&mut self, key: KeyEvent, worker_tx: &UnboundedSender<AppEvent>) {
        match self.focus {
            Focus::Mode => {}
            Focus::Generate => {
                if matches!(key.code, KeyCode::Char(' ') | KeyCode::Enter) {
                    self.generate = !self.generate;
                }
            }
            Focus::KeyHex => self.key_hex.handle_key(key),
            Focus::Threshold => self.threshold.handle_key(key),
            Focus::Shares => {
                self.shares_count.handle_key(key);
                self.sync_split_guardians();
            }
            Focus::SplitOutDir => self.split_out_dir.handle_key(key),
            Focus::SplitDest(i) => match key.code {
                KeyCode::Left => self.cycle_split_dest(i, -1),
                KeyCode::Right | KeyCode::Char(' ') => self.cycle_split_dest(i, 1),
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    self.drives = device::list_removable_drives();
                }
                _ => {}
            },
            Focus::SplitCustomPath(i) => {
                if let Some(dest) = self.split_destinations.get_mut(i) {
                    dest.custom_path.handle_key(key);
                }
            }
            Focus::SplitPassword(i) => {
                if let Some(field) = self.split_passwords.get_mut(i) {
                    field.handle_key(key);
                }
            }
            Focus::SplitSubmit => {
                if key.code == KeyCode::Enter {
                    self.run_split(worker_tx);
                }
            }
            Focus::GroupDir => self.group_dir.handle_key(key),
            Focus::ShareDir => {
                if key.code == KeyCode::Enter {
                    self.picker.rescan();
                    self.sync_sign_passwords();
                } else {
                    self.picker.dir.handle_key(key);
                }
            }
            Focus::Files => match key.code {
                KeyCode::Up => self.picker.move_up(),
                KeyCode::Down => self.picker.move_down(),
                KeyCode::Char(' ') | KeyCode::Enter => {
                    self.picker.toggle();
                    self.sync_sign_passwords();
                }
                _ => {}
            },
            Focus::SignPassword(i) => {
                let selected = self.picker.selected();
                if let Some(path) = selected.get(i) {
                    self.password_by_path
                        .entry(path.clone())
                        .or_insert_with(TextField::masked)
                        .handle_key(key);
                }
            }
            Focus::To => self.to.handle_key(key),
            Focus::Lamports => self.lamports.handle_key(key),
            Focus::Blockhash => self.blockhash.handle_key(key),
            Focus::Broadcast => {
                if matches!(key.code, KeyCode::Char(' ') | KeyCode::Enter) {
                    self.broadcast = !self.broadcast;
                }
            }
            Focus::SignSubmit => {
                if key.code == KeyCode::Enter {
                    self.try_sign(worker_tx);
                }
            }
        }
    }

    fn handle_modal_key(&mut self, key: KeyEvent, worker_tx: &UnboundedSender<AppEvent>) {
        let Some(modal) = &self.modal else { return };
        match key.code {
            KeyCode::Esc => {
                let modal = self.modal.take().expect("checked above");
                cleanup_temp_files(&modal.temp_files);
            }
            KeyCode::Char('f') | KeyCode::Char('F') if modal.blocking => {
                let modal = self.modal.take().expect("checked above");
                self.begin_sign(
                    modal.shares,
                    modal.passwords,
                    modal.attempt,
                    modal.temp_files,
                    worker_tx,
                );
            }
            KeyCode::Enter if !modal.blocking => {
                let modal = self.modal.take().expect("checked above");
                self.begin_sign(
                    modal.shares,
                    modal.passwords,
                    modal.attempt,
                    modal.temp_files,
                    worker_tx,
                );
            }
            _ => {}
        }
    }

    fn run_split(&mut self, worker_tx: &UnboundedSender<AppEvent>) {
        self.sync_split_guardians();
        let threshold: u8 = match self.threshold.value().trim().parse() {
            Ok(v) => v,
            Err(_) => return,
        };
        let shares: u8 = match self.shares_count.value().trim().parse() {
            Ok(v) => v,
            Err(_) => return,
        };
        let out_dir = std::path::PathBuf::from(if self.split_out_dir.value().is_empty() {
            "mpc"
        } else {
            self.split_out_dir.value()
        });
        let passwords: Vec<String> = self
            .split_passwords
            .iter()
            .map(|f| f.value().to_string())
            .collect();
        // See `InitScreen::run`'s equivalent resolution for the rationale:
        // "this device" keeps the previous single-`out_dir` behavior, a
        // detected drive gets a namespaced `horcrux/` subfolder, and the
        // filename's `{i+1}` is a positional human label, not the share's
        // real participant id (assigned during the split below).
        let all_this_device = self
            .split_destinations
            .iter()
            .all(|d| d.choice == DestChoice::ThisDevice);
        let destinations: Vec<PathBuf> = (0..self.split_passwords.len())
            .map(|i| {
                let name = format!("mpc-{}.hx", i + 1);
                match self.split_destinations.get(i).map(|d| d.choice) {
                    Some(DestChoice::Drive(idx)) => match self.drives.get(idx) {
                        Some(d) => d.mount_point.join("horcrux").join(&name),
                        None => out_dir.join(&name),
                    },
                    Some(DestChoice::Custom) => {
                        let custom = self
                            .split_destinations
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
        self.split_result = None;
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
                horcrux::mpc::mpc_split(&key, threshold, shares, &out_dir, &passwords)
            } else {
                horcrux::mpc::mpc_split_to(
                    &key,
                    threshold,
                    shares,
                    &passwords,
                    &destinations,
                    &out_dir,
                )
            }
            .map(|(paths, group_path)| SplitOutcome {
                paths,
                group_path: Some(group_path),
                generated_key_hex,
            });
            let _ = tx.send(AppEvent::SplitDone(outcome));
        });
    }

    fn try_sign(&mut self, worker_tx: &UnboundedSender<AppEvent>) {
        let selected = self.picker.selected();
        if selected.is_empty() {
            return;
        }
        if self.blockhash.value().trim().is_empty() && !self.broadcast {
            return;
        }
        self.sync_sign_passwords();
        let passwords: Vec<String> = selected
            .iter()
            .map(|p| {
                self.password_by_path
                    .get(p)
                    .map(|f| f.value().to_string())
                    .unwrap_or_default()
            })
            .collect();

        let mut temp_files = Vec::new();
        let shares: Vec<PathBuf> = match selected
            .iter()
            .map(|p| stage_shard_path(p, &mut temp_files))
            .collect::<Result<_, _>>()
        {
            Ok(v) => v,
            Err(e) => {
                cleanup_temp_files(&temp_files);
                self.signed = Some(Err(e));
                return;
            }
        };

        let ids = match horcrux::mpc::shard_ids(&shares) {
            Ok(ids) => ids,
            Err(e) => {
                cleanup_temp_files(&temp_files);
                self.signed = Some(Err(e));
                return;
            }
        };
        let log = AccessLog::open(horcrux::audit::resolve_log_path(None));
        let history = log.read_all().unwrap_or_default();
        let now = horcrux::audit::now_ms();
        let attempt: u64 = rand::random();

        match Scorer::new().assess(&history, &ids, now) {
            Verdict::Block(reasons) => {
                let _ = log.append(&Entry::blocked(now, attempt));
                self.modal = Some(Modal {
                    blocking: true,
                    reasons,
                    attempt,
                    shares,
                    passwords,
                    temp_files,
                });
            }
            Verdict::Warn(reasons) => {
                self.modal = Some(Modal {
                    blocking: false,
                    reasons,
                    attempt,
                    shares,
                    passwords,
                    temp_files,
                });
            }
            Verdict::Allow => self.begin_sign(shares, passwords, attempt, temp_files, worker_tx),
        }
    }

    fn begin_sign(
        &mut self,
        shares: Vec<PathBuf>,
        passwords: Vec<String>,
        attempt: u64,
        temp_files: Vec<PathBuf>,
        worker_tx: &UnboundedSender<AppEvent>,
    ) {
        let to: solana_pubkey::Pubkey = match self.to.value().trim().parse() {
            Ok(p) => p,
            Err(e) => {
                self.signed = Some(Err(Error::Tx(format!("invalid recipient address: {e}"))));
                return;
            }
        };
        let lamports: u64 = match self.lamports.value().trim().parse() {
            Ok(v) => v,
            Err(_) => {
                self.signed = Some(Err(Error::Tx("invalid lamports amount".into())));
                return;
            }
        };
        let group_dir = PathBuf::from(if self.group_dir.value().is_empty() {
            "mpc"
        } else {
            self.group_dir.value()
        });
        let blockhash_input = self.blockhash.value().trim().to_string();
        let broadcast = self.broadcast;
        if blockhash_input.is_empty() && !broadcast {
            return;
        }

        self.busy = true;
        self.signed = None;
        self.broadcast_result = None;
        self.modal = None;

        let tx = worker_tx.clone();
        let rpc_url = horcrux::chain::default_rpc_url();
        tokio::spawn(async move {
            let group_pub = group_dir.join(horcrux::mpc::GROUP_PUB_FILENAME);
            let verifying_key: [u8; 32] = match horcrux::mpc::group_verifying_key(&group_pub) {
                Ok(k) => k,
                Err(e) => {
                    let _ = tx.send(AppEvent::SignDone(Err(e)));
                    cleanup_temp_files(&temp_files);
                    return;
                }
            };
            let from = solana_pubkey::Pubkey::from(verifying_key);

            let blockhash: solana_hash::Hash = if !blockhash_input.is_empty() {
                match blockhash_input.parse() {
                    Ok(h) => h,
                    Err(e) => {
                        let _ = tx.send(AppEvent::SignDone(Err(Error::Tx(format!(
                            "invalid blockhash: {e}"
                        )))));
                        cleanup_temp_files(&temp_files);
                        return;
                    }
                }
            } else if broadcast {
                let chain = horcrux::chain::Chain::connect(&rpc_url);
                match chain.latest_blockhash().await {
                    Ok(h) => h,
                    Err(e) => {
                        let _ = tx.send(AppEvent::SignDone(Err(e)));
                        cleanup_temp_files(&temp_files);
                        return;
                    }
                }
            } else {
                let _ = tx.send(AppEvent::SignDone(Err(Error::Tx(
                    "offline signing requires a blockhash (or turn on Broadcast to fetch one)"
                        .into(),
                ))));
                cleanup_temp_files(&temp_files);
                return;
            };

            if broadcast {
                let chain = horcrux::chain::Chain::connect(&rpc_url);
                match chain.balance(&from).await {
                    Ok(balance) if balance > 0 => {}
                    Ok(_) => {
                        let _ = tx.send(AppEvent::SignDone(Err(Error::Tx(format!(
                            "sender {from} is unfunded; airdrop lamports first"
                        )))));
                        cleanup_temp_files(&temp_files);
                        return;
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::SignDone(Err(e)));
                        cleanup_temp_files(&temp_files);
                        return;
                    }
                }
            }

            let params = horcrux::tx::TxParams {
                from,
                to,
                lamports,
                blockhash,
            };
            let sign_result: Result<horcrux::tx::SignedTx, Error> =
                tokio::task::spawn_blocking(move || {
                    let log = AccessLog::open(horcrux::audit::resolve_log_path(None));
                    let message = horcrux::tx::transaction_message(&params);
                    let message_bytes = message.serialize();
                    let sig = horcrux::mpc::mpc_sign_with_audit(
                        &shares,
                        &passwords,
                        &group_pub,
                        &message_bytes,
                        &log,
                        attempt,
                    )?;
                    horcrux::tx::sign_transaction_with_signature(
                        params,
                        sig.signature,
                        sig.verifying_key,
                    )
                })
                .await
                .unwrap_or_else(|e| Err(Error::Tx(format!("worker task panicked: {e}"))));

            // Reconstruction/signing is done — any staged temp `.hx` file
            // from a `.png` QR import can be removed now.
            cleanup_temp_files(&temp_files);

            let signed = match sign_result {
                Ok(s) => s,
                Err(e) => {
                    let _ = tx.send(AppEvent::SignDone(Err(e)));
                    return;
                }
            };
            let _ = tx.send(AppEvent::SignDone(Ok(horcrux::SignedOutput::from(
                signed.clone(),
            ))));

            if broadcast {
                let chain = horcrux::chain::Chain::connect(&rpc_url);
                let result = horcrux::chain::broadcast(
                    chain.client(),
                    signed.tx(),
                    Duration::from_secs(1),
                    60,
                )
                .await
                .map(|sig| sig.to_string());
                let _ = tx.send(AppEvent::BroadcastDone(result));
            }
        });
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let mode_label = format!(
            "mode: < {} >  (Split/Sign — Left/Right/Space/Enter to change)",
            match self.mode {
                Mode::Split => "SPLIT",
                Mode::Sign => "SIGN",
            }
        );
        let mode_style = if self.focus == Focus::Mode {
            Style::default().fg(theme::ACCENT)
        } else {
            Style::default()
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(area);
        frame.render_widget(
            Paragraph::new(mode_label)
                .style(mode_style)
                .block(Block::default().borders(Borders::ALL)),
            chunks[0],
        );

        match self.mode {
            Mode::Split => self.render_split(frame, chunks[1]),
            Mode::Sign => self.render_sign(frame, chunks[1]),
        }

        if let Some(modal) = &self.modal {
            render_audit_modal(frame, area, modal.blocking, &modal.reasons);
        }
        if let Some(overlay) = &self.qr_view {
            render_qr_overlay(frame, area, overlay, self.split_result_path_count());
        }
    }

    fn render_split(&self, frame: &mut Frame, area: Rect) {
        let mut guardian_rows: Vec<Focus> = Vec::new();
        for i in 0..self.split_passwords.len() {
            guardian_rows.push(Focus::SplitDest(i));
            if self
                .split_destinations
                .get(i)
                .is_some_and(|d| d.choice == DestChoice::Custom)
            {
                guardian_rows.push(Focus::SplitCustomPath(i));
            }
            guardian_rows.push(Focus::SplitPassword(i));
        }
        let field_rows = 5 + guardian_rows.len();
        let mut constraints = vec![Constraint::Length(3); field_rows];
        constraints.push(Constraint::Min(0));
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        let gen_label = format!(
            "[{}] generate a disposable test key",
            if self.generate { "x" } else { " " }
        );
        frame.render_widget(
            Paragraph::new(gen_label)
                .style(focus_style(self.focus == Focus::Generate))
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
        self.shares_count
            .render(frame, chunks[3], "shares", self.focus == Focus::Shares);
        self.split_out_dir.render(
            frame,
            chunks[4],
            "output directory",
            self.focus == Focus::SplitOutDir,
        );
        let n = self.split_passwords.len();
        for (row_offset, row) in guardian_rows.iter().enumerate() {
            let row_area = chunks[5 + row_offset];
            match *row {
                Focus::SplitDest(i) => {
                    let label = format!(
                        "destination for share {} of {n} (Left/Right to change, r to rescan)",
                        i + 1
                    );
                    frame.render_widget(
                        Paragraph::new(self.split_dest_label(i))
                            .style(focus_style(self.focus == Focus::SplitDest(i)))
                            .block(Block::default().borders(Borders::ALL).title(label)),
                        row_area,
                    );
                }
                Focus::SplitCustomPath(i) => {
                    if let Some(dest) = self.split_destinations.get(i) {
                        dest.custom_path.render(
                            frame,
                            row_area,
                            &format!("custom destination directory for share {}", i + 1),
                            self.focus == Focus::SplitCustomPath(i),
                        );
                    }
                }
                Focus::SplitPassword(i) => {
                    if let Some(field) = self.split_passwords.get(i) {
                        field.render(
                            frame,
                            row_area,
                            &format!("password for share {} of {n} (distinct per guardian)", i + 1),
                            self.focus == Focus::SplitPassword(i),
                        );
                    }
                }
                _ => {}
            }
        }

        let mut items: Vec<ListItem> = vec![
            ListItem::new("[ Split ] — focus here and press Enter")
                .style(focus_style(self.focus == Focus::SplitSubmit)),
        ];
        if self.busy {
            items.push(ListItem::new("splitting…"));
        }
        match &self.split_result {
            None => {}
            Some(Err(e)) => items.push(
                ListItem::new(format!("error: {e}")).style(Style::default().fg(theme::BLOCK)),
            ),
            Some(Ok(outcome)) => {
                if let Some(hex) = &outcome.generated_key_hex {
                    items.push(
                        ListItem::new(format!("Generated test key (shown once): 0x{hex}"))
                            .style(Style::default().fg(theme::WARN)),
                    );
                }
                items.push(
                    ListItem::new(format!("Wrote {} share(s):", outcome.paths.len()))
                        .style(Style::default().fg(theme::ALLOW)),
                );
                for p in &outcome.paths {
                    items.push(ListItem::new(format!("  {}", p.display())));
                }
                if let Some(g) = &outcome.group_path {
                    items.push(ListItem::new(format!(
                        "Group public package: {}",
                        g.display()
                    )));
                }
                items.push(
                    ListItem::new("Ctrl+Q: show a share as a scannable QR code")
                        .style(Style::default().fg(theme::MUTED)),
                );
            }
        }
        frame.render_widget(
            List::new(items).block(Block::default().borders(Borders::ALL).title("result")),
            chunks[field_rows],
        );
    }

    fn render_sign(&self, frame: &mut Frame, area: Rect) {
        let selected = self.picker.selected();
        let n = selected.len();
        let mut constraints = vec![Constraint::Length(3), Constraint::Length(3), Constraint::Length(6)];
        constraints.extend(std::iter::repeat_n(Constraint::Length(3), n));
        constraints.extend([
            Constraint::Length(3), // To
            Constraint::Length(3), // Lamports
            Constraint::Length(3), // Blockhash
            Constraint::Length(1), // Blockhash hint
            Constraint::Length(3), // Broadcast
            Constraint::Min(0),    // Result
        ]);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        self.group_dir.render(
            frame,
            chunks[0],
            "group directory (contains group.pub)",
            self.focus == Focus::GroupDir,
        );
        self.picker.dir.render(
            frame,
            chunks[1],
            "share directory (Enter to scan)",
            self.focus == Focus::ShareDir,
        );
        self.picker
            .render(frame, chunks[2], self.focus == Focus::Files);
        for (i, path) in selected.iter().enumerate() {
            let label = format!(
                "password for {} (distinct per guardian)",
                path.file_name()
                    .map(|f| f.to_string_lossy().into_owned())
                    .unwrap_or_default()
            );
            if let Some(field) = self.password_by_path.get(path) {
                field.render(frame, chunks[3 + i], &label, self.focus == Focus::SignPassword(i));
            }
        }
        let to_idx = 3 + n;
        self.to.render(
            frame,
            chunks[to_idx],
            "recipient address (base58)",
            self.focus == Focus::To,
        );
        self.lamports.render(
            frame,
            chunks[to_idx + 1],
            "lamports",
            self.focus == Focus::Lamports,
        );
        self.blockhash.render(
            frame,
            chunks[to_idx + 2],
            "blockhash (base58 — leave empty to fetch when broadcasting)",
            self.focus == Focus::Blockhash,
        );
        let (hint_text, hint_style) = if self.blockhash.value().trim().is_empty()
            && !self.broadcast
        {
            (
                "REQUIRED: enter a blockhash, or turn Broadcast on (Space) to fetch one.",
                Style::default().fg(theme::WARN),
            )
        } else {
            (
                "optional — required only for offline signing",
                Style::default().fg(theme::MUTED),
            )
        };
        frame.render_widget(Paragraph::new(hint_text).style(hint_style), chunks[to_idx + 3]);

        let bc_label = format!(
            "[{}] broadcast to a Solana cluster",
            if self.broadcast { "x" } else { " " }
        );
        frame.render_widget(
            Paragraph::new(bc_label)
                .style(focus_style(self.focus == Focus::Broadcast))
                .block(Block::default().borders(Borders::ALL)),
            chunks[to_idx + 4],
        );

        let mut items: Vec<ListItem> = vec![
            ListItem::new("[ Sign ] — focus here and press Enter")
                .style(focus_style(self.focus == Focus::SignSubmit)),
        ];
        if self.busy {
            items.push(ListItem::new("working…"));
        }
        match &self.signed {
            None => {}
            Some(Err(e)) => items.push(
                ListItem::new(format!("error: {e}")).style(Style::default().fg(theme::BLOCK)),
            ),
            Some(Ok(out)) => {
                items.push(
                    ListItem::new(format!("From:      {}", out.from))
                        .style(Style::default().fg(theme::ALLOW)),
                );
                items.push(ListItem::new(format!("Signature: {}", out.signature)));
            }
        }
        match &self.broadcast_result {
            None => {}
            Some(Err(e)) => items.push(
                ListItem::new(format!("broadcast error: {e}"))
                    .style(Style::default().fg(theme::BLOCK)),
            ),
            Some(Ok(sig)) => items.push(
                ListItem::new(format!("Mined:     {sig} (confirmed)"))
                    .style(Style::default().fg(theme::ALLOW)),
            ),
        }
        frame.render_widget(
            List::new(items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Tab/Shift+Tab focus · Esc back"),
            ),
            chunks[to_idx + 5],
        );
    }
}

fn focus_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(theme::ACCENT)
    } else {
        Style::default()
    }
}

/// If `path` is a `.png` QR export of a share, decode it and stage the
/// payload to a fresh temp `.hx` file, pushing that temp path onto
/// `temp_files` for later cleanup. Otherwise returns `path` unchanged. See
/// `screens::sign`'s equivalent.
fn stage_shard_path(path: &std::path::Path, temp_files: &mut Vec<PathBuf>) -> Result<PathBuf, Error> {
    if path.extension().is_some_and(|e| e == "png") {
        let staged = horcrux::qr::stage_shard_from_qr_png(path)?;
        temp_files.push(staged.clone());
        Ok(staged)
    } else {
        Ok(path.to_path_buf())
    }
}

/// Best-effort removal of temp files staged by [`stage_shard_path`].
fn cleanup_temp_files(paths: &[PathBuf]) {
    for p in paths {
        let _ = std::fs::remove_file(p);
    }
}

/// Render a written share file as one or more scannable terminal QR frames.
/// See `screens::init::load_qr_pages`.
fn load_qr_pages(path: &std::path::Path) -> Result<Vec<String>, Error> {
    let bytes = std::fs::read(path).map_err(Error::Io)?;
    let session: [u8; horcrux::qr::SESSION_LEN] = rand::random();
    let frames = horcrux::qr::encode_message(horcrux::qr::MessageType::ShardFile, session, &bytes)?;
    Ok(frames.iter().map(horcrux::qr::terminal_qr).collect())
}

fn render_qr_overlay(frame: &mut Frame, area: Rect, overlay: &QrOverlay, total_shares: usize) {
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
        "QR — share {}/{total_shares} \u{b7} frame {}/{}",
        overlay.share_index + 1,
        overlay.frame_index + 1,
        overlay.pages.len().max(1),
    );
    let mut body = text;
    body.push_str("\nLeft/Right: frame   Up/Down: share   Esc: close");
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
    fn tab_from_the_last_split_field_wraps_to_mode_not_the_first_field() {
        let mut screen = MpcScreen::default();
        assert_eq!(screen.mode, Mode::Split);
        screen.focus = *screen.order().last().unwrap();
        screen.move_focus(1);
        assert_eq!(screen.focus, Focus::Mode);
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn shift_tab_from_the_first_split_field_wraps_to_mode_not_the_last_field() {
        let mut screen = MpcScreen::default();
        screen.focus = screen.order()[0];
        screen.move_focus(-1);
        assert_eq!(screen.focus, Focus::Mode);
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn tab_moves_through_the_split_fields_in_order() {
        let mut screen = MpcScreen::default();
        let order = screen.order();
        screen.focus = order[0];
        screen.move_focus(1);
        assert_eq!(screen.focus, order[1]);
    }

    #[test]
    fn switching_mode_switches_the_field_order_used_by_focus_navigation() {
        let mut screen = MpcScreen::default();
        let split_order = screen.order();
        assert!(split_order.iter().any(|f| matches!(f, Focus::SplitSubmit)));
        screen.mode = Mode::Sign;
        let sign_order = screen.order();
        assert!(sign_order.iter().any(|f| matches!(f, Focus::SignSubmit)));
        assert_ne!(split_order, sign_order);
    }
}
