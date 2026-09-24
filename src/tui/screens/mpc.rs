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
use crate::tui::widgets::{FileChecklist, TextField};
use crossterm::event::{KeyCode, KeyEvent};
use horcrux::audit::{AccessLog, Entry, Scorer, Verdict};
use horcrux::error::Error;
use k256::SecretKey;
use rand::rngs::OsRng;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};
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
    SplitPassword,
    SplitSubmit,
    // sign fields
    GroupDir,
    ShareDir,
    Files,
    SignPassword,
    To,
    Lamports,
    Blockhash,
    Broadcast,
    SignSubmit,
}

const SPLIT_ORDER: [Focus; 7] = [
    Focus::Generate,
    Focus::KeyHex,
    Focus::Threshold,
    Focus::Shares,
    Focus::SplitOutDir,
    Focus::SplitPassword,
    Focus::SplitSubmit,
];
const SIGN_ORDER: [Focus; 9] = [
    Focus::GroupDir,
    Focus::ShareDir,
    Focus::Files,
    Focus::SignPassword,
    Focus::To,
    Focus::Lamports,
    Focus::Blockhash,
    Focus::Broadcast,
    Focus::SignSubmit,
];

struct Modal {
    blocking: bool,
    reasons: Vec<String>,
    attempt: u64,
    shares: Vec<PathBuf>,
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
    split_password: TextField,
    split_result: Option<Result<SplitOutcome, Error>>,

    // sign state
    group_dir: TextField,
    picker: FileChecklist,
    sign_password: TextField,
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
        Self {
            mode: Mode::Split,
            focus: Focus::Mode,
            busy: false,
            generate: false,
            key_hex: TextField::masked(),
            threshold,
            shares_count,
            split_out_dir,
            split_password: TextField::masked(),
            split_result: None,
            group_dir,
            picker,
            sign_password: TextField::masked(),
            to: TextField::default(),
            lamports: TextField::default(),
            blockhash: TextField::default(),
            broadcast: false,
            modal: None,
            signed: None,
            broadcast_result: None,
        }
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

    fn order(&self) -> &'static [Focus] {
        match self.mode {
            Mode::Split => &SPLIT_ORDER,
            Mode::Sign => &SIGN_ORDER,
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
            Focus::Shares => self.shares_count.handle_key(key),
            Focus::SplitOutDir => self.split_out_dir.handle_key(key),
            Focus::SplitPassword => self.split_password.handle_key(key),
            Focus::SplitSubmit => {
                if key.code == KeyCode::Enter {
                    self.run_split(worker_tx);
                }
            }
            Focus::GroupDir => self.group_dir.handle_key(key),
            Focus::ShareDir => {
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
            Focus::SignPassword => self.sign_password.handle_key(key),
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
            KeyCode::Esc => self.modal = None,
            KeyCode::Char('f') | KeyCode::Char('F') if modal.blocking => {
                let modal = self.modal.take().expect("checked above");
                self.begin_sign(modal.shares, modal.attempt, worker_tx);
            }
            KeyCode::Enter if !modal.blocking => {
                let modal = self.modal.take().expect("checked above");
                self.begin_sign(modal.shares, modal.attempt, worker_tx);
            }
            _ => {}
        }
    }

    fn run_split(&mut self, worker_tx: &UnboundedSender<AppEvent>) {
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
        let password = self.split_password.value().to_string();
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
            let passwords = vec![password; shares as usize];
            let outcome = horcrux::mpc::mpc_split(&key, threshold, shares, &out_dir, &passwords)
                .map(|(paths, group_path)| SplitOutcome {
                    paths,
                    group_path: Some(group_path),
                    generated_key_hex,
                });
            let _ = tx.send(AppEvent::SplitDone(outcome));
        });
    }

    fn try_sign(&mut self, worker_tx: &UnboundedSender<AppEvent>) {
        let shares = self.picker.selected();
        if shares.is_empty() {
            return;
        }
        let ids = match horcrux::mpc::shard_ids(&shares) {
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
                    shares,
                });
            }
            Verdict::Warn(reasons) => {
                self.modal = Some(Modal {
                    blocking: false,
                    reasons,
                    attempt,
                    shares,
                });
            }
            Verdict::Allow => self.begin_sign(shares, attempt, worker_tx),
        }
    }

    fn begin_sign(
        &mut self,
        shares: Vec<PathBuf>,
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
        let group_dir = PathBuf::from(if self.group_dir.value().is_empty() {
            "mpc"
        } else {
            self.group_dir.value()
        });
        let blockhash_input = self.blockhash.value().trim().to_string();
        let password = self.sign_password.value().to_string();
        let broadcast = self.broadcast;

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

            if broadcast {
                let chain = horcrux::chain::Chain::connect(&rpc_url);
                match chain.balance(&from).await {
                    Ok(balance) if balance > 0 => {}
                    Ok(_) => {
                        let _ = tx.send(AppEvent::SignDone(Err(Error::Tx(format!(
                            "sender {from} is unfunded; airdrop lamports first"
                        )))));
                        return;
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::SignDone(Err(e)));
                        return;
                    }
                }
            }

            let passwords = vec![password; shares.len()];
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
            render_modal(frame, area, modal);
        }
    }

    fn render_split(&self, frame: &mut Frame, area: Rect) {
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
        self.split_password.render(
            frame,
            chunks[5],
            "password (used for every share)",
            self.focus == Focus::SplitPassword,
        );

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
            }
        }
        frame.render_widget(
            List::new(items).block(Block::default().borders(Borders::ALL).title("result")),
            chunks[6],
        );
    }

    fn render_sign(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
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
        self.sign_password.render(
            frame,
            chunks[3],
            "password (used for every selected share)",
            self.focus == Focus::SignPassword,
        );
        self.to.render(
            frame,
            chunks[4],
            "recipient address (base58)",
            self.focus == Focus::To,
        );
        self.lamports
            .render(frame, chunks[5], "lamports", self.focus == Focus::Lamports);
        self.blockhash.render(
            frame,
            chunks[6],
            "blockhash (base58 — leave empty to fetch when broadcasting)",
            self.focus == Focus::Blockhash,
        );

        let bc_label = format!(
            "[{}] broadcast to a Solana cluster",
            if self.broadcast { "x" } else { " " }
        );
        frame.render_widget(
            Paragraph::new(bc_label)
                .style(focus_style(self.focus == Focus::Broadcast))
                .block(Block::default().borders(Borders::ALL)),
            chunks[7],
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
            chunks[8],
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
        screen.focus = *SPLIT_ORDER.last().unwrap();
        screen.move_focus(1);
        assert_eq!(screen.focus, Focus::Mode);
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn shift_tab_from_the_first_split_field_wraps_to_mode_not_the_last_field() {
        let mut screen = MpcScreen::default();
        screen.focus = SPLIT_ORDER[0];
        screen.move_focus(-1);
        assert_eq!(screen.focus, Focus::Mode);
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn tab_moves_through_the_split_fields_in_order() {
        let mut screen = MpcScreen::default();
        screen.focus = SPLIT_ORDER[0];
        screen.move_focus(1);
        assert_eq!(screen.focus, SPLIT_ORDER[1]);
    }

    #[test]
    fn switching_mode_switches_the_field_order_used_by_focus_navigation() {
        let mut screen = MpcScreen::default();
        assert_eq!(screen.order(), &SPLIT_ORDER[..]);
        screen.mode = Mode::Sign;
        assert_eq!(screen.order(), &SIGN_ORDER[..]);
    }
}
