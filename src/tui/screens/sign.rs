//! Sign screen (step 5): Mode A signing, offline or broadcast — the MVP's
//! real milestone, mirroring `agent/wayfinder/PLAN.md` Phase 2's "complete,
//! defensible project" framing, now reachable without memorizing CLI flags.
//!
//! Exercises the full async design: the audit pre-flight runs synchronously
//! (cheap — log read + arithmetic) and blocks on a Warn/Block modal exactly
//! like the CLI's pre-flight bails/warns; the CPU-bound Argon2id decrypt +
//! sign runs in `spawn_blocking`; an optional broadcast runs as a plain
//! `tokio::spawn` network task. Neither ever runs on the render thread.
//!
//! The reconstructed key never becomes UI state: it lives only inside the
//! `spawn_blocking` closure and is zeroized on drop before this screen ever
//! sees anything back (only the signed output crosses the channel).

use crate::tui::action::AppEvent;
use crate::tui::theme;
use crate::tui::widgets::{FileChecklist, TextField, render_audit_modal};
use crossterm::event::{KeyCode, KeyEvent};
use horcrux::audit::{AccessLog, Entry, Scorer, Verdict};
use horcrux::error::Error;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Dir,
    Files,
    Password(usize),
    To,
    Lamports,
    Blockhash,
    Broadcast,
    Submit,
}

/// A pending audit verdict the user must resolve before signing proceeds.
struct Modal {
    /// `true` for a `Block` verdict (needs an explicit force); `false` for
    /// `Warn` (needs an explicit continue).
    blocking: bool,
    reasons: Vec<String>,
    attempt: u64,
    shards: Vec<PathBuf>,
    /// Passwords snapshotted at submit time, in the same order as `shards`,
    /// so resolving the modal later reuses exactly what was entered then.
    passwords: Vec<String>,
    /// Temp `.hx` files staged from any `.png` QR imports among `shards`,
    /// to be deleted however the modal is resolved (forced through,
    /// continued past, or cancelled).
    temp_files: Vec<PathBuf>,
}

pub struct SignScreen {
    picker: FileChecklist,
    /// One password per currently-selected shard file, keyed by path (the
    /// selected set is dynamic, unlike Init's fixed shard count). Entries
    /// are added lazily and never pruned on uncheck, so re-checking a box
    /// doesn't force retyping. A single shared password here was the root
    /// cause of shards created with per-guardian passwords being unable to
    /// reconstruct from the TUI.
    password_by_path: HashMap<PathBuf, TextField>,
    to: TextField,
    lamports: TextField,
    blockhash: TextField,
    broadcast: bool,
    focus: Focus,
    busy: bool,
    modal: Option<Modal>,
    signed: Option<Result<horcrux::SignedOutput, Error>>,
    broadcast_result: Option<Result<String, Error>>,
}

impl Default for SignScreen {
    fn default() -> Self {
        let mut picker = FileChecklist::default();
        picker.dir.set_value("shards");
        Self {
            picker,
            password_by_path: HashMap::new(),
            to: TextField::default(),
            lamports: TextField::default(),
            blockhash: TextField::default(),
            broadcast: false,
            focus: Focus::Dir,
            busy: false,
            modal: None,
            signed: None,
            broadcast_result: None,
        }
    }
}

impl SignScreen {
    pub fn is_busy(&self) -> bool {
        self.busy
    }

    pub fn modal_open(&self) -> bool {
        self.modal.is_some()
    }

    pub fn apply(&mut self, ev: AppEvent) {
        match ev {
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
            _ => {}
        }
    }

    /// Ensure every currently-selected shard has a password field, adding
    /// one lazily for newly-checked paths. Never removes entries for
    /// unchecked paths, so toggling a box off and back on keeps what was
    /// typed.
    fn sync_passwords(&mut self) {
        for path in self.picker.selected() {
            self.password_by_path
                .entry(path)
                .or_insert_with(TextField::masked);
        }
    }

    fn order(&self) -> Vec<Focus> {
        let mut order = vec![Focus::Dir, Focus::Files];
        order.extend((0..self.picker.selected().len()).map(Focus::Password));
        order.extend([
            Focus::To,
            Focus::Lamports,
            Focus::Blockhash,
            Focus::Broadcast,
            Focus::Submit,
        ]);
        order
    }

    pub fn handle_key(&mut self, key: KeyEvent, worker_tx: &UnboundedSender<AppEvent>) {
        if self.busy {
            return;
        }
        if self.modal.is_some() {
            self.handle_modal_key(key, worker_tx);
            return;
        }
        match key.code {
            KeyCode::Tab => self.move_focus(1),
            KeyCode::BackTab => self.move_focus(-1),
            _ => match self.focus {
                Focus::Dir => {
                    if key.code == KeyCode::Enter {
                        self.picker.rescan();
                        self.sync_passwords();
                    } else {
                        self.picker.dir.handle_key(key);
                    }
                }
                Focus::Files => match key.code {
                    KeyCode::Up => self.picker.move_up(),
                    KeyCode::Down => self.picker.move_down(),
                    KeyCode::Char(' ') | KeyCode::Enter => {
                        self.picker.toggle();
                        self.sync_passwords();
                    }
                    _ => {}
                },
                Focus::Password(i) => {
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
                Focus::Submit => {
                    if key.code == KeyCode::Enter {
                        self.try_submit(worker_tx);
                    }
                }
            },
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
                    modal.shards,
                    modal.passwords,
                    modal.attempt,
                    modal.temp_files,
                    worker_tx,
                );
            }
            KeyCode::Enter if !modal.blocking => {
                let modal = self.modal.take().expect("checked above");
                self.begin_sign(
                    modal.shards,
                    modal.passwords,
                    modal.attempt,
                    modal.temp_files,
                    worker_tx,
                );
            }
            _ => {}
        }
    }

    fn move_focus(&mut self, dir: i32) {
        self.sync_passwords();
        let order = self.order();
        let idx = order.iter().position(|f| *f == self.focus).unwrap_or(0) as i32;
        let n = order.len() as i32;
        let next = ((idx + dir) % n + n) % n;
        self.focus = order[next as usize];
    }

    /// Score the proposed attempt against the access log before any key
    /// material is touched, exactly mirroring the CLI's `audit_preflight`.
    fn try_submit(&mut self, worker_tx: &UnboundedSender<AppEvent>) {
        let selected = self.picker.selected();
        if selected.is_empty() {
            return;
        }
        if self.blockhash.value().trim().is_empty() && !self.broadcast {
            return;
        }
        self.sync_passwords();
        // Passwords are keyed by the originally selected path (what the
        // guardian actually typed a password against), before any `.png` QR
        // import is staged to a temp `.hx` file below.
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
        let shards: Vec<PathBuf> = match selected
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

        let ids = match horcrux::audit::shard_ids(&shards) {
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
                    shards,
                    passwords,
                    temp_files,
                });
            }
            Verdict::Warn(reasons) => {
                self.modal = Some(Modal {
                    blocking: false,
                    reasons,
                    attempt,
                    shards,
                    passwords,
                    temp_files,
                });
            }
            Verdict::Allow => self.begin_sign(shards, passwords, attempt, temp_files, worker_tx),
        }
    }

    fn begin_sign(
        &mut self,
        shards: Vec<PathBuf>,
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

            let sign_result: Result<horcrux::tx::SignedTx, Error> =
                tokio::task::spawn_blocking(move || {
                    let log = AccessLog::open(horcrux::audit::resolve_log_path(None));
                    let key = horcrux::reconstruct_with_audit(&shards, &passwords, &log, attempt)?;
                    let seed = horcrux::key_seed(&key);
                    let from = horcrux::tx::derive_address(&seed);
                    let params = horcrux::tx::TxParams {
                        from,
                        to,
                        lamports,
                        blockhash,
                    };
                    let seed_bytes: [u8; 32] = *seed;
                    horcrux::tx::sign_transaction(seed_bytes, params)
                })
                .await
                .unwrap_or_else(|e| Err(Error::Tx(format!("worker task panicked: {e}"))));

            // Reconstruction is done (successfully or not) — any staged
            // temp `.hx` file from a `.png` QR import has served its
            // purpose and can be removed now.
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
                match chain.balance(&signed.from()).await {
                    Ok(balance) if balance > 0 => {
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
                    Ok(_) => {
                        let _ = tx.send(AppEvent::BroadcastDone(Err(Error::Tx(format!(
                            "sender {} is unfunded; airdrop lamports first",
                            signed.from()
                        )))));
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::BroadcastDone(Err(e)));
                    }
                }
            }
        });
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let selected = self.picker.selected();
        let n = selected.len();
        let mut constraints = vec![Constraint::Length(3), Constraint::Length(6)];
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

        self.picker.dir.render(
            frame,
            chunks[0],
            "shard directory (Enter to scan)",
            self.focus == Focus::Dir,
        );
        self.picker
            .render(frame, chunks[1], self.focus == Focus::Files);
        for (i, path) in selected.iter().enumerate() {
            let label = format!(
                "password for {} (distinct per guardian)",
                path.file_name()
                    .map(|f| f.to_string_lossy().into_owned())
                    .unwrap_or_default()
            );
            if let Some(field) = self.password_by_path.get(path) {
                field.render(frame, chunks[2 + i], &label, self.focus == Focus::Password(i));
            }
        }
        let to_idx = 2 + n;
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
            "[{}] broadcast to a Solana cluster (Space/Enter to toggle)",
            if self.broadcast { "x" } else { " " }
        );
        let bc_style = if self.focus == Focus::Broadcast {
            Style::default().fg(theme::ACCENT)
        } else {
            Style::default()
        };
        frame.render_widget(
            Paragraph::new(bc_label)
                .style(bc_style)
                .block(Block::default().borders(Borders::ALL)),
            chunks[to_idx + 4],
        );

        let submit_style = if self.focus == Focus::Submit {
            Style::default().fg(theme::ACCENT)
        } else {
            Style::default()
        };
        let mut result_lines: Vec<ListItem> =
            vec![ListItem::new("[ Sign ] — focus here and press Enter").style(submit_style)];
        if self.busy {
            result_lines.push(ListItem::new("working…"));
        }
        match &self.signed {
            None => {}
            Some(Err(e)) => result_lines.push(
                ListItem::new(format!("error: {e}")).style(Style::default().fg(theme::BLOCK)),
            ),
            Some(Ok(out)) => {
                result_lines.push(
                    ListItem::new(format!("From:      {}", out.from))
                        .style(Style::default().fg(theme::ALLOW)),
                );
                result_lines.push(ListItem::new(format!("Signature: {}", out.signature)));
            }
        }
        match &self.broadcast_result {
            None => {}
            Some(Err(e)) => result_lines.push(
                ListItem::new(format!("broadcast error: {e}"))
                    .style(Style::default().fg(theme::BLOCK)),
            ),
            Some(Ok(sig)) => result_lines.push(
                ListItem::new(format!("Mined:     {sig} (confirmed)"))
                    .style(Style::default().fg(theme::ALLOW)),
            ),
        }
        frame.render_widget(
            List::new(result_lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Tab/Shift+Tab focus · Esc back"),
            ),
            chunks[to_idx + 5],
        );

        if let Some(modal) = &self.modal {
            render_audit_modal(frame, area, modal.blocking, &modal.reasons);
        }
    }
}

/// If `path` is a `.png` QR export of a shard, decode it and stage the
/// payload to a fresh temp `.hx` file, pushing that temp path onto
/// `temp_files` for later cleanup. Otherwise returns `path` unchanged.
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
