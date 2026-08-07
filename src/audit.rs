//! Append-only access logging and rule-based anomaly scoring (Phase 3).
//!
//! Every shard decryption attempt is recorded as a JSON-lines entry in an
//! append-only log. Before a signing attempt proceeds, a rule-based scorer
//! evaluates the attempt against the historical log and returns a verdict:
//! allow, warn, or block. Blocked attempts are themselves recorded so a
//! refused attempt cannot hide.

use crate::error::Error;
use crate::shard::Shard;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The outcome recorded for a shard access attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// The shard decrypted successfully.
    DecryptOk,
    /// Decryption failed (wrong password or tampered file).
    DecryptFail,
    /// The attempt was blocked by the audit layer before any decryption.
    Blocked,
}

/// A single JSON-lines record in the access log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Unix epoch milliseconds.
    pub ts: u64,
    /// Identifier of the signing/reconstruction attempt this entry belongs to.
    /// Shard entries from one attempt share the same id, which lets the scorer
    /// group them back into combinations.
    pub attempt: u64,
    /// Shard id involved. Only meaningful for decrypt entries; `blocked`
    /// entries carry `0`.
    pub shard_id: u8,
    /// What happened.
    pub kind: EntryKind,
}

impl Entry {
    /// A successful decryption of `shard_id`.
    pub fn ok(ts: u64, attempt: u64, shard_id: u8) -> Self {
        Self {
            ts,
            attempt,
            shard_id,
            kind: EntryKind::DecryptOk,
        }
    }

    /// A failed decryption of `shard_id`.
    pub fn fail(ts: u64, attempt: u64, shard_id: u8) -> Self {
        Self {
            ts,
            attempt,
            shard_id,
            kind: EntryKind::DecryptFail,
        }
    }

    /// An attempt that was refused before any decryption.
    pub fn blocked(ts: u64, attempt: u64) -> Self {
        Self {
            ts,
            attempt,
            shard_id: 0,
            kind: EntryKind::Blocked,
        }
    }
}

/// An append-only, JSON-lines access log.
///
/// Entries are only ever appended (never rewritten), so history is preserved
/// even if an attempt is refused.
#[derive(Debug, Clone)]
pub struct AccessLog {
    path: PathBuf,
}

impl AccessLog {
    /// Open (or lazily create) the log at `path`.
    pub fn open(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// The resolved log file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one entry, creating the file and parent directories on demand.
    pub fn append(&self, entry: &Entry) -> Result<(), Error> {
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let line = serde_json::to_string(entry).map_err(|e| Error::Audit(e.to_string()))?;
        writeln!(file, "{line}")?;
        Ok(())
    }

    /// Read every entry in chronological order. A missing file yields an empty
    /// history.
    pub fn read_all(&self) -> Result<Vec<Entry>, Error> {
        let file = match fs::File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut entries = Vec::new();
        for line in BufReader::new(file).lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            entries.push(serde_json::from_str(line).map_err(|e| Error::Audit(e.to_string()))?);
        }
        Ok(entries)
    }

    /// Return the last `n` entries (all of them when fewer exist).
    pub fn tail(&self, n: usize) -> Result<Vec<Entry>, Error> {
        let all = self.read_all()?;
        let start = all.len().saturating_sub(n);
        Ok(all[start..].to_vec())
    }
}

/// Tunable rules and thresholds for the anomaly scorer.
#[derive(Debug, Clone)]
pub struct RuleConfig {
    /// Trailing decrypt failures within `fail_lookback_secs` that block an
    /// attempt.
    pub failed_window: usize,
    /// Failures older than this many seconds do not count toward the window.
    pub fail_lookback_secs: u64,
    /// UTC hours (inclusive start, exclusive end) treated as unusual.
    pub odd_hours: (u8, u8),
    /// Inter-attempt gap z-score above which the gap is flagged as unusual.
    pub z_threshold: f64,
}

impl Default for RuleConfig {
    fn default() -> Self {
        Self {
            failed_window: 3,
            fail_lookback_secs: 3600,
            odd_hours: (0, 6),
            z_threshold: 3.0,
        }
    }
}

/// The outcome of scoring a signing attempt against the access log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// No signals fired; signing may proceed.
    Allow,
    /// Unusual but not clearly malicious; signing proceeds with a warning.
    Warn(Vec<String>),
    /// A hard rule fired; signing must be refused unless forced.
    Block(Vec<String>),
}

/// Rule-based anomaly scorer evaluated before any key material is handled.
#[derive(Debug, Clone)]
pub struct Scorer {
    config: RuleConfig,
}

impl Default for Scorer {
    fn default() -> Self {
        Self::new()
    }
}

impl Scorer {
    /// A scorer with the default [`RuleConfig`].
    pub fn new() -> Self {
        Self {
            config: RuleConfig::default(),
        }
    }

    /// A scorer with custom rules.
    pub fn with_config(config: RuleConfig) -> Self {
        Self { config }
    }

    /// The active rules.
    pub fn config(&self) -> &RuleConfig {
        &self.config
    }

    /// Score a proposed attempt (using `attempt_ids` shards, starting at
    /// `now_ms`) against the historical log.
    pub fn assess(&self, history: &[Entry], attempt_ids: &[u8], now_ms: u64) -> Verdict {
        let mut blocks = Vec::new();
        let mut warns = Vec::new();

        let failures = self.trailing_failures(history, now_ms);
        if failures >= self.config.failed_window {
            blocks.push(format!(
                "{failures} failed decrypt attempts within the last {}s",
                self.config.fail_lookback_secs
            ));
        }

        let hour = utc_hour(now_ms);
        if hour >= self.config.odd_hours.0 && hour < self.config.odd_hours.1 {
            warns.push(format!("attempt at unusual hour {hour:02}:00 UTC"));
        }

        if !self.combo_seen(history, attempt_ids) {
            warns.push(format!("unfamiliar shard combination {:?}", attempt_ids));
        }

        if let Some(z) = self.gap_z_score(history, now_ms)
            && z > self.config.z_threshold
        {
            warns.push(format!("unusual gap since last access (z = {z:.1})"));
        }

        if blocks.is_empty() && warns.is_empty() {
            Verdict::Allow
        } else if blocks.is_empty() {
            Verdict::Warn(warns)
        } else {
            Verdict::Block(blocks)
        }
    }

    /// Number of trailing `decrypt_fail` entries within the lookback window.
    /// A success resets the run; blocked entries are neutral.
    fn trailing_failures(&self, history: &[Entry], now_ms: u64) -> usize {
        let window_start = now_ms.saturating_sub(self.config.fail_lookback_secs * 1000);
        let mut count = 0;
        for e in history.iter().rev() {
            if e.ts < window_start {
                break;
            }
            match e.kind {
                EntryKind::DecryptFail => count += 1,
                EntryKind::DecryptOk => break,
                EntryKind::Blocked => {}
            }
        }
        count
    }

    /// Whether this exact set of shard ids has been used by a previous
    /// completed attempt. With no prior completed attempts (fresh log) the
    /// combination is treated as known so first-time use does not warn.
    fn combo_seen(&self, history: &[Entry], attempt_ids: &[u8]) -> bool {
        let mut per_attempt: HashMap<u64, HashSet<u8>> = HashMap::new();
        for e in history {
            match e.kind {
                EntryKind::DecryptOk | EntryKind::DecryptFail => {
                    per_attempt.entry(e.attempt).or_default().insert(e.shard_id);
                }
                EntryKind::Blocked => {}
            }
        }
        if per_attempt.values().all(|ids| ids.is_empty()) {
            return true;
        }
        let mut current: Vec<u8> = attempt_ids.to_vec();
        current.sort_unstable();
        per_attempt
            .into_values()
            .map(|ids| {
                let mut seen: Vec<u8> = ids.into_iter().collect();
                seen.sort_unstable();
                seen
            })
            .any(|seen| seen == current)
    }

    /// z-score of the gap since the last logged attempt against the historical
    /// distribution of inter-attempt gaps. Returns `None` when there is not
    /// enough history to estimate the distribution.
    fn gap_z_score(&self, history: &[Entry], now_ms: u64) -> Option<f64> {
        let mut last_ts: HashMap<u64, u64> = HashMap::new();
        for e in history {
            if e.kind == EntryKind::Blocked {
                continue;
            }
            let slot = last_ts.entry(e.attempt).or_insert(0);
            *slot = (*slot).max(e.ts);
        }
        let mut times: Vec<u64> = last_ts.into_values().collect();
        if times.len() < 2 {
            return None;
        }
        times.sort_unstable();
        let gaps: Vec<u64> = times.windows(2).map(|w| w[1] - w[0]).collect();
        let n = gaps.len() as f64;
        let mean = gaps.iter().map(|&g| g as f64).sum::<f64>() / n;
        let var = gaps.iter().map(|&g| (g as f64 - mean).powi(2)).sum::<f64>() / n;
        let sd = var.sqrt();
        if sd < 1e-9 {
            return None;
        }
        let gap = now_ms.saturating_sub(times[times.len() - 1]);
        Some((gap as f64 - mean) / sd)
    }
}

/// UTC hour (0–23) for a Unix epoch-millis timestamp.
pub fn utc_hour(epoch_ms: u64) -> u8 {
    ((epoch_ms / 1000) % 86_400 / 3600) as u8
}

/// Current Unix epoch milliseconds.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after the Unix epoch")
        .as_millis() as u64
}

/// Read the shard ids from shard files without decrypting anything.
pub fn shard_ids(paths: &[PathBuf]) -> Result<Vec<u8>, Error> {
    paths.iter().map(|p| Ok(Shard::read(p)?.id)).collect()
}

/// Format a Unix epoch-millis timestamp as `YYYY-MM-DD HH:MM:SS UTC`.
pub fn format_utc(epoch_ms: u64) -> String {
    let days = (epoch_ms / 86_400_000) as i64;
    let rem = epoch_ms % 86_400_000;
    let (y, mo, d) = civil_from_days(days);
    let hh = rem / 3_600_000;
    let mm = rem % 3_600_000 / 60_000;
    let ss = rem % 60_000 / 1_000;
    format!("{y:04}-{mo:02}-{d:02} {hh:02}:{mm:02}:{ss:02} UTC")
}

/// Days since 1970-01-01 to a (year, month, day) calendar date, using Howard
/// Hinnant's civil-from-days algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn history(entries: &[Entry]) -> Vec<Entry> {
        entries.to_vec()
    }

    #[test]
    fn empty_history_allows() {
        assert_eq!(
            Scorer::new().assess(&[], &[1, 2], 40_000_000),
            Verdict::Allow
        );
    }

    #[test]
    fn first_use_with_clean_history_allows() {
        let h = history(&[Entry::ok(40_000_000, 1, 1), Entry::ok(40_000_000, 1, 2)]);
        assert_eq!(
            Scorer::new().assess(&h, &[1, 2], 40_000_100),
            Verdict::Allow
        );
    }

    #[test]
    fn three_trailing_failures_block() {
        let h = history(&[
            Entry::fail(30_000_000, 1, 1),
            Entry::fail(30_000_000, 1, 2),
            Entry::fail(30_000_000, 1, 3),
        ]);
        assert!(matches!(
            Scorer::new().assess(&h, &[1, 2], 30_000_100),
            Verdict::Block(_)
        ));
    }

    #[test]
    fn two_failures_then_success_does_not_block() {
        let h = history(&[
            Entry::fail(30_000_000, 1, 1),
            Entry::fail(30_000_000, 1, 2),
            Entry::ok(30_000_000, 1, 3),
        ]);
        let v = Scorer::new().assess(&h, &[1, 2], 30_000_100);
        assert!(matches!(v, Verdict::Allow | Verdict::Warn(_)));
    }

    #[test]
    fn stale_failures_do_not_block() {
        let h = history(&[
            Entry::fail(20_000_000, 1, 1),
            Entry::fail(20_000_000, 1, 2),
            Entry::fail(20_000_000, 1, 3),
        ]);
        let now = 23_600_001; // failures now fall just outside the 1h window
        assert!(matches!(
            Scorer::new().assess(&h, &[1, 2], now),
            Verdict::Allow | Verdict::Warn(_)
        ));
    }

    #[test]
    fn odd_hour_warns() {
        let now = 3 * 3_600_000; // 03:00 UTC
        let v = Scorer::new().assess(&[], &[1, 2], now);
        assert!(matches!(v, Verdict::Warn(_)));
    }

    #[test]
    fn unfamiliar_combination_warns() {
        let h = history(&[Entry::ok(40_000_000, 1, 1), Entry::ok(40_000_000, 1, 2)]);
        let v = Scorer::new().assess(&h, &[1, 3], 40_000_100);
        assert!(matches!(v, Verdict::Warn(_)));
    }

    #[test]
    fn outlier_gap_warns() {
        let h = history(&[
            Entry::ok(39_900_000, 1, 1),
            Entry::ok(39_901_000, 2, 1),
            Entry::ok(39_903_000, 3, 1),
        ]);
        let v = Scorer::new().assess(&h, &[1], 40_000_000);
        assert!(matches!(v, Verdict::Warn(reasons) if reasons.iter().any(|r| r.contains("gap"))));
    }

    #[test]
    fn constant_gaps_produce_no_gap_signal() {
        let h = history(&[
            Entry::ok(40_010_000, 1, 1),
            Entry::ok(40_020_000, 2, 1),
            Entry::ok(40_030_000, 3, 1),
        ]);
        assert_eq!(Scorer::new().assess(&h, &[1], 40_040_000), Verdict::Allow);
    }

    #[test]
    fn append_and_read_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = AccessLog::open(dir.path().join("access.log"));
        log.append(&Entry::ok(1, 9, 2)).expect("append");
        log.append(&Entry::fail(2, 9, 3)).expect("append");
        log.append(&Entry::blocked(3, 10)).expect("append");

        let all = log.read_all().expect("read");
        assert_eq!(
            all,
            vec![
                Entry::ok(1, 9, 2),
                Entry::fail(2, 9, 3),
                Entry::blocked(3, 10)
            ]
        );
        assert_eq!(
            log.tail(2).expect("tail"),
            vec![Entry::fail(2, 9, 3), Entry::blocked(3, 10)]
        );
    }

    #[test]
    fn missing_log_reads_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = AccessLog::open(dir.path().join("nope.log"));
        assert!(log.read_all().expect("read").is_empty());
    }

    #[test]
    fn skips_blank_lines() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("access.log");
        std::fs::write(
            &path,
            "{\"ts\":1,\"attempt\":1,\"shard_id\":1,\"kind\":\"decrypt_ok\"}\n\n",
        )
        .expect("write");
        let log = AccessLog::open(path);
        assert_eq!(log.read_all().expect("read").len(), 1);
    }

    #[test]
    fn utc_hour_is_clock_consistent() {
        assert_eq!(utc_hour(0), 0);
        assert_eq!(utc_hour(3 * 3_600_000), 3);
        assert_eq!(utc_hour(23 * 3_600_000), 23);
        assert_eq!(utc_hour(24 * 3_600_000), 0);
    }

    #[test]
    fn format_utc_matches_known_date() {
        // 2026-01-01 00:00:00 UTC = 1767225600000 ms
        assert_eq!(format_utc(1_767_225_600_000), "2026-01-01 00:00:00 UTC");
    }
}
