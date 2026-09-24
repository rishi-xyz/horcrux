//! Results flowing back from spawned (network or CPU-bound) work into the
//! render loop.
//!
//! Kept separate from `app.rs` so the complete set of things that can happen
//! asynchronously — including anything derived from key material — is easy
//! to review in one place: this is the only channel through which spawned
//! work reports back, and nothing here ever carries a raw key or seed (see
//! the key-material decision in `agent/wayfinder/PLAN.md`'s TUI plan).

use horcrux::SignedOutput;
use horcrux::error::Error;
use horcrux::verify::Report;
use std::path::PathBuf;

/// Result of an `init`/`mpc-split` key-splitting run.
#[derive(Debug)]
pub struct SplitOutcome {
    pub paths: Vec<PathBuf>,
    pub group_path: Option<PathBuf>,
    pub generated_key_hex: Option<String>,
}

// Every variant deliberately ends in "Done" — it names what just finished,
// consistently, across a small enum with no glob-imported variants to
// disambiguate; clippy's heuristic for accidental repetition doesn't apply.
#[allow(clippy::enum_variant_names)]
#[derive(Debug)]
pub enum AppEvent {
    /// A passive `verify` pass finished.
    VerifyDone(Vec<Report>),
    /// An `init`/`mpc-split` finished.
    SplitDone(Result<SplitOutcome, Error>),
    /// A Mode A or Mode B sign finished (key material was reconstructed and
    /// zeroized entirely inside the `spawn_blocking` closure that produced
    /// this — only the signed output ever crosses back into UI state).
    SignDone(Result<SignedOutput, Error>),
    /// A broadcast finished; the payload is the confirmed signature/txid.
    BroadcastDone(Result<String, Error>),
}
