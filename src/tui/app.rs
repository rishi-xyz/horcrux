//! The TUI's top-level state machine.

use crate::tui::action::AppEvent;
use crate::tui::event::TuiEvent;
use crate::tui::screens::init::InitScreen;
use crate::tui::screens::log::LogScreen;
use crate::tui::screens::menu::MenuScreen;
use crate::tui::screens::mpc::MpcScreen;
use crate::tui::screens::sign::SignScreen;
use crate::tui::screens::verify::VerifyScreen;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

/// Which screen is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Menu,
    Log,
    Verify,
    Init,
    Sign,
    Mpc,
}

pub struct App {
    pub screen: Screen,
    pub should_quit: bool,
    pub worker_tx: mpsc::UnboundedSender<AppEvent>,
    pub worker_rx: mpsc::UnboundedReceiver<AppEvent>,
    pub menu: MenuScreen,
    pub log: LogScreen,
    pub verify: VerifyScreen,
    pub init: InitScreen,
    pub sign: SignScreen,
    pub mpc: MpcScreen,
}

impl App {
    pub fn new() -> Self {
        let (worker_tx, worker_rx) = mpsc::unbounded_channel();
        Self {
            screen: Screen::Menu,
            should_quit: false,
            worker_tx,
            worker_rx,
            menu: MenuScreen::default(),
            log: LogScreen::default(),
            verify: VerifyScreen::default(),
            init: InitScreen::default(),
            sign: SignScreen::default(),
            mpc: MpcScreen::default(),
        }
    }

    pub fn handle_event(&mut self, ev: TuiEvent) {
        if let TuiEvent::Key(key) = ev {
            self.handle_key(key);
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return;
        }

        match self.screen {
            Screen::Menu => {
                if key.code == KeyCode::Char('q') {
                    self.should_quit = true;
                    return;
                }
                if let Some(next) = self.menu.handle_key(key) {
                    self.enter(next);
                }
            }
            Screen::Log => {
                if key.code == KeyCode::Esc {
                    self.screen = Screen::Menu;
                } else {
                    self.log.handle_key(key);
                }
            }
            Screen::Verify => {
                if key.code == KeyCode::Esc && !self.verify.is_busy() {
                    self.screen = Screen::Menu;
                } else {
                    self.verify.handle_key(key, &self.worker_tx);
                }
            }
            Screen::Init => {
                if key.code == KeyCode::Esc && !self.init.is_busy() {
                    self.screen = Screen::Menu;
                } else {
                    self.init.handle_key(key, &self.worker_tx);
                }
            }
            Screen::Sign => {
                if key.code == KeyCode::Esc && !self.sign.is_busy() && !self.sign.modal_open() {
                    self.screen = Screen::Menu;
                } else {
                    self.sign.handle_key(key, &self.worker_tx);
                }
            }
            Screen::Mpc => {
                if key.code == KeyCode::Esc && !self.mpc.is_busy() && !self.mpc.modal_open() {
                    self.screen = Screen::Menu;
                } else {
                    self.mpc.handle_key(key, &self.worker_tx);
                }
            }
        }
    }

    fn enter(&mut self, screen: Screen) {
        match screen {
            Screen::Log => self.log.reload(),
            Screen::Verify => self.verify = VerifyScreen::default(),
            Screen::Init => self.init = InitScreen::default(),
            Screen::Sign => self.sign = SignScreen::default(),
            Screen::Mpc => self.mpc = MpcScreen::default(),
            Screen::Menu => {}
        }
        self.screen = screen;
    }

    pub fn apply(&mut self, ev: AppEvent) {
        match self.screen {
            Screen::Verify => self.verify.apply(ev),
            Screen::Init => self.init.apply(ev),
            Screen::Sign => self.sign.apply(ev),
            Screen::Mpc => self.mpc.apply(ev),
            Screen::Menu | Screen::Log => {}
        }
    }
}
