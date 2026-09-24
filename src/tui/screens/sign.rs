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
use crate::tui::widgets::{FileChecklist, TextField};
use crossterm::event::{KeyCode, KeyEvent};
use horcrux::audit::{AccessLog, Entry, Scorer, Verdict};
use horcrux::error::Error;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Dir,
    Files,
    Password,
    To,
    Lamports,
    Blockhash,
    Broadcast,
    Submit,
}
const FOCUS_ORDER: [Focus; 8] = [
    Focus::Dir,
    Focus::Files,
    Focus::Password,
    Focus::To,
    Focus::Lamports,
    Focus::Blockhash,
    Focus::Broadcast,
    Focus::Submit,
];

/// A pending audit verdict the user must resolve before signing proceeds.
struct Modal {
    /// `true` for a `Block` verdict (needs an explicit force); `false` for
    /// `Warn` (needs an explicit continue).
    blocking: bool,
    reasons: Vec<String>,
    attempt: u64,
    shards: Vec<PathBuf>,
}

pub struct SignScreen {
    picker: FileChecklist,
    password: TextField,
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
            password: TextField::masked(),
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
                    } else {
                        self.picker.dir.handle_key(key);
                    }
                }
                Focus::Files => match key.code {
                    KeyCode::Up => self.picker.move_up(),
                    KeyCode::Down => self.picker.move_down(),
                    KeyCode::Char(' ') | KeyCode::Enter => self.picker.toggle(),
                    _ => {}
                },
                Focus::Password => self.password.handle_key(key),
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
            KeyCode::Esc => self.modal = None,
            KeyCode::Char('f') | KeyCode::Char('F') if modal.blocking => {
                let modal = self.modal.take().expect("checked above");
                self.begin_sign(modal.shards, modal.attempt, worker_tx);
            }
            KeyCode::Enter if !modal.blocking => {
                let modal = self.modal.take().expect("checked above");
                self.begin_sign(modal.shards, modal.attempt, worker_tx);
            }
            _ => {}
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

    /// Score the proposed attempt against the access log before any key
    /// material is touched, exactly mirroring the CLI's `audit_preflight`.
    fn try_submit(&mut self, worker_tx: &UnboundedSender<AppEvent>) {
        let shards = self.picker.selected();
        if shards.is_empty() {
            return;
        }
        let ids = match horcrux::audit::shard_ids(&shards) {
            Ok(ids) => ids,
            Err(e) => {
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
                });
            }
            Verdict::Warn(reasons) => {
                self.modal = Some(Modal {
                    blocking: false,
                    reasons,
                    attempt,
                    shards,
                });
            }
            Verdict::Allow => self.begin_sign(shards, attempt, worker_tx),
        }
    }

    fn begin_sign(
        &mut self,
        shards: Vec<PathBuf>,
        attempt: u64,
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
        let password = self.password.value().to_string();
        let broadcast = self.broadcast;

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
                        return;
                    }
                }
            } else if broadcast {
                let chain = horcrux::chain::Chain::connect(&rpc_url);
                match chain.latest_blockhash().await {
                    Ok(h) => h,
                    Err(e) => {
                        let _ = tx.send(AppEvent::SignDone(Err(e)));
                        return;
                    }
                }
            } else {
                let _ = tx.send(AppEvent::SignDone(Err(Error::Tx(
                    "offline signing requires a blockhash (or turn on Broadcast to fetch one)"
                        .into(),
                ))));
                return;
            };

            let passwords = vec![password; shards.len()];
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
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(6),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(area);

        self.picker.dir.render(
            frame,
            chunks[0],
            "shard directory (Enter to scan)",
            self.focus == Focus::Dir,
        );
        self.picker
            .render(frame, chunks[1], self.focus == Focus::Files);
        self.password.render(
            frame,
            chunks[2],
            "password (used for every selected shard)",
            self.focus == Focus::Password,
        );
        self.to.render(
            frame,
            chunks[3],
            "recipient address (base58)",
            self.focus == Focus::To,
        );
        self.lamports
            .render(frame, chunks[4], "lamports", self.focus == Focus::Lamports);
        self.blockhash.render(
            frame,
            chunks[5],
            "blockhash (base58 — leave empty to fetch when broadcasting)",
            self.focus == Focus::Blockhash,
        );

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
            chunks[6],
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
            chunks[7],
        );

        if let Some(modal) = &self.modal {
            render_modal(frame, area, modal);
        }
    }
}

fn render_modal(frame: &mut Frame, area: Rect, modal: &Modal) {
    let width = area.width.saturating_sub(8).clamp(20, 70);
    let height = (modal.reasons.len() as u16 + 5).min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup);
    let (title, color, hint) = if modal.blocking {
        (
            "AUDIT: BLOCKED",
            theme::BLOCK,
            "f = force through (logged)   Esc = cancel",
        )
    } else {
        (
            "AUDIT: WARNING",
            theme::WARN,
            "Enter = continue   Esc = cancel",
        )
    };
    let mut lines: Vec<ListItem> = modal
        .reasons
        .iter()
        .map(|r| ListItem::new(format!("• {r}")))
        .collect();
    lines.push(ListItem::new(""));
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
