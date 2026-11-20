//! JSON API handlers backing the web console. Each endpoint wraps the same
//! library functions the CLI and TUI call where possible — no crypto logic is
//! duplicated here, only request parsing, audit preflight, JSON shaping, and
//! local (non-secret) persistence for wallets and advisory AI/security events.

use axum::Json;
use axum::extract::{Path as AxumPath, Query};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use horcrux::audit::{AccessLog, Entry, EntryKind, Scorer, Verdict};
use horcrux::error::Error;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

pub struct ApiError {
    status: StatusCode,
    body: Value,
}

impl ApiError {
    fn message(status: StatusCode, msg: impl Into<String>) -> Self {
        Self {
            status,
            body: json!({ "error": msg.into() }),
        }
    }
    fn bad(msg: impl Into<String>) -> Self {
        Self::message(StatusCode::BAD_REQUEST, msg)
    }
    fn blocked(reasons: Vec<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            body: json!({ "error": "blocked", "reasons": reasons }),
        }
    }
    fn internal(msg: impl Into<String>) -> Self {
        Self::message(StatusCode::INTERNAL_SERVER_ERROR, msg)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}

impl From<Error> for ApiError {
    fn from(e: Error) -> Self {
        match e {
            Error::Io(ref io) if io.kind() == std::io::ErrorKind::NotFound => {
                ApiError::message(StatusCode::NOT_FOUND, e.to_string())
            }
            _ => ApiError::bad(e.to_string()),
        }
    }
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        ApiError::bad(e.to_string())
    }
}

fn join_err(e: tokio::task::JoinError) -> ApiError {
    ApiError::internal(format!("worker task panicked: {e}"))
}

fn parse_key(hex_key: &str) -> Result<k256::SecretKey, Error> {
    let stripped = hex_key.strip_prefix("0x").unwrap_or(hex_key);
    let bytes =
        hex::decode(stripped).map_err(|e| Error::InvalidKey(format!("not valid hex: {e}")))?;
    if bytes.len() != 32 {
        return Err(Error::InvalidKey(format!(
            "expected 32 bytes, got {}",
            bytes.len()
        )));
    }
    k256::SecretKey::from_slice(&bytes).map_err(|e| Error::InvalidKey(e.to_string()))
}

/// Parse a hex-encoded private key, or generate a disposable test key.
fn resolve_key(
    key_hex: Option<&str>,
    generate: bool,
) -> Result<(k256::SecretKey, Option<String>), ApiError> {
    match (key_hex.filter(|s| !s.is_empty()), generate) {
        (Some(hex_key), _) => Ok((parse_key(hex_key)?, None)),
        (None, true) => {
            let key = k256::SecretKey::random(&mut rand::rngs::OsRng);
            let hex = hex::encode(key.to_bytes());
            Ok((key, Some(hex)))
        }
        (None, false) => Err(ApiError::bad("provide either key_hex or generate")),
    }
}

/// Audit preflight: score proposed shard IDs against the access log.
/// A `Block` verdict is logged and refuses the operation unless `force` is set;
/// a `Warn` proceeds with reasons surfaced.
fn preflight(ids: Vec<u8>, force: bool) -> Result<(AccessLog, u64, Vec<String>), ApiError> {
    let log = AccessLog::open(horcrux::audit::resolve_log_path(None));
    let history = log.read_all()?;
    let now = horcrux::audit::now_ms();
    let attempt: u64 = rand::random();
    let warnings = match Scorer::new().assess(&history, &ids, now) {
        Verdict::Block(reasons) => {
            log.append(&Entry::blocked(now, attempt))?;
            if !force {
                return Err(ApiError::blocked(reasons));
            }
            reasons
                .into_iter()
                .map(|r| format!("BLOCKED, overridden by force: {r}"))
                .collect()
        }
        Verdict::Warn(reasons) => reasons,
        Verdict::Allow => Vec::new(),
    };
    Ok((log, attempt, warnings))
}

fn entry_kind_str(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::DecryptOk => "decrypt_ok",
        EntryKind::DecryptFail => "decrypt_fail",
        EntryKind::Blocked => "blocked",
        EntryKind::Signed => "signed",
    }
}
fn now_ms() -> u64 {
    horcrux::audit::now_ms()
}

// ----------------------------- persistence --------------------------------

static FILE_LOCK: Mutex<()> = Mutex::new(());

fn data_dir() -> PathBuf {
    std::env::var("HORCRUX_WEB_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}
fn wallets_path() -> PathBuf {
    data_dir().join("horcrux-wallets.json")
}
fn ai_events_path() -> PathBuf {
    data_dir().join("horcrux-ai-events.jsonl")
}
fn web_state_path() -> PathBuf {
    data_dir().join("horcrux-web-state.json")
}
fn dotenv_path() -> PathBuf {
    data_dir().join(".env")
}

fn read_json_file<T: for<'de> Deserialize<'de> + Default>(path: &Path) -> T {
    let _g = FILE_LOCK.lock().ok();
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}
fn write_json_file<T: Serialize>(path: &Path, value: &T) -> Result<(), ApiError> {
    let _g = FILE_LOCK.lock().ok();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(ApiError::from)?;
    }
    let s = serde_json::to_string_pretty(value).map_err(|e| ApiError::internal(e.to_string()))?;
    fs::write(path, s).map_err(ApiError::from)
}
fn append_jsonl(path: &Path, value: &impl Serialize) -> Result<(), ApiError> {
    let _g = FILE_LOCK.lock().ok();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(ApiError::from)?;
    }
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(ApiError::from)?;
    let line = serde_json::to_string(value).map_err(|e| ApiError::internal(e.to_string()))?;
    writeln!(f, "{line}").map_err(ApiError::from)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct WalletStore {
    wallets: Vec<WalletProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WalletProfile {
    id: String,
    label: String,
    #[serde(rename = "type")]
    kind: String,
    chain: String,
    network: String,
    address: String,
    protected: bool,
    created_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shard_refs: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    group_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct WebState {
    #[serde(default)]
    ai_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AiEventRecord {
    id: String,
    ts: u64,
    ts_utc: String,
    action: String,
    derived: String,
    status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    severity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    deterministic: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    details: Option<Value>,
}

fn new_id(prefix: &str) -> String {
    format!("{prefix}-{:016x}", rand::random::<u64>())
}

fn read_ai_events(limit: Option<usize>) -> Vec<AiEventRecord> {
    let _g = FILE_LOCK.lock().ok();
    let Ok(s) = fs::read_to_string(ai_events_path()) else {
        return Vec::new();
    };
    let mut out: Vec<AiEventRecord> = s
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    if let Some(n) = limit {
        if out.len() > n {
            out = out.split_off(out.len() - n);
        }
    }
    out
}

fn record_ai_event(mut ev: AiEventRecord) -> Result<AiEventRecord, ApiError> {
    if ev.ts == 0 {
        ev.ts = now_ms();
    }
    if ev.ts_utc.is_empty() {
        ev.ts_utc = horcrux::audit::format_utc(ev.ts);
    }
    append_jsonl(&ai_events_path(), &ev)?;
    Ok(ev)
}

// ----------------------------- AI helpers ---------------------------------

fn ai_status_info() -> Value {
    let from_env = std::env::var("OPENROUTER_API_KEY").ok();
    let from_dotenv = if from_env.is_none() {
        dotenvy::from_path_iter(dotenv_path()).ok().and_then(|it| {
            it.filter_map(|r| r.ok())
                .find(|(k, _)| k == "OPENROUTER_API_KEY")
                .map(|(_, v)| v)
        })
    } else {
        None
    };
    let key = from_env.clone().or(from_dotenv.clone());
    let model = std::env::var("HORCRUX_AI_MODEL")
        .ok()
        .or_else(|| read_json_file::<WebState>(&web_state_path()).ai_model)
        .unwrap_or_else(|| horcrux::ai_anomaly::DEFAULT_MODEL.to_string());
    let source = if from_env.is_some() {
        "env"
    } else if from_dotenv.is_some() {
        "dotenv"
    } else {
        "none"
    };
    let redacted = key.as_ref().map(|k| {
        if k.len() <= 8 {
            "****".to_string()
        } else {
            format!("{}…{}", &k[..4], &k[k.len() - 4..])
        }
    });
    json!({ "configured": key.is_some(), "model": model, "source": source, "key_redacted": redacted, "background_enabled": key.is_some() })
}

async fn run_ai_check(entries: Vec<Entry>, model_override: Option<String>) -> AiEventRecord {
    let id = new_id("ai");
    let ts = now_ms();
    let model = model_override
        .or_else(|| std::env::var("HORCRUX_AI_MODEL").ok())
        .or_else(|| read_json_file::<WebState>(&web_state_path()).ai_model)
        .unwrap_or_else(|| horcrux::ai_anomaly::DEFAULT_MODEL.to_string());
    let api_key = std::env::var("OPENROUTER_API_KEY").ok().or_else(|| {
        dotenvy::from_path_iter(dotenv_path()).ok().and_then(|it| {
            it.filter_map(|r| r.ok())
                .find(|(k, _)| k == "OPENROUTER_API_KEY")
                .map(|(_, v)| v)
        })
    });
    let Some(api_key) = api_key else {
        return AiEventRecord {
            id,
            ts,
            ts_utc: horcrux::audit::format_utc(ts),
            action: "ai_check".into(),
            derived: "ai_unavailable".into(),
            status: "unavailable".into(),
            summary: Some("OPENROUTER_API_KEY not configured; advisory AI check skipped".into()),
            model: Some(model),
            severity: Some("info".into()),
            deterministic: None,
            details: None,
        };
    };
    let tail = if entries.len() > 50 {
        entries[entries.len() - 50..].to_vec()
    } else {
        entries
    };
    match horcrux::ai_anomaly::check(&tail, &api_key, Some(&model)).await {
        Ok(v) => AiEventRecord {
            id,
            ts,
            ts_utc: horcrux::audit::format_utc(ts),
            action: "ai_check".into(),
            derived: if v.anomaly { "ai_anomaly" } else { "ai_clear" }.into(),
            status: if v.anomaly { "anomaly" } else { "ok" }.into(),
            summary: Some(v.summary),
            model: Some(v.model),
            severity: Some(if v.anomaly { "warn" } else { "info" }.into()),
            deterministic: None,
            details: None,
        },
        Err(e) => AiEventRecord {
            id,
            ts,
            ts_utc: horcrux::audit::format_utc(ts),
            action: "ai_check".into(),
            derived: "ai_unavailable".into(),
            status: "unavailable".into(),
            summary: Some(format!("advisory AI check unavailable: {e}")),
            model: Some(model),
            severity: Some("info".into()),
            deterministic: None,
            details: None,
        },
    }
}

fn spawn_action_ai_event(
    action: &str,
    derived: &str,
    details: Value,
    deterministic: Option<Value>,
) {
    let action = action.to_string();
    let derived = derived.to_string();
    tokio::spawn(async move {
        let ts = now_ms();
        let pending = AiEventRecord {
            id: new_id("evt"),
            ts,
            ts_utc: horcrux::audit::format_utc(ts),
            action: action.clone(),
            derived: derived.clone(),
            status: "pending".into(),
            summary: Some("Advisory AI assessment running…".into()),
            model: Some(
                ai_status_info()["model"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            ),
            severity: Some("info".into()),
            deterministic: deterministic.clone(),
            details: Some(details.clone()),
        };
        let _ = record_ai_event(pending.clone());
        let log = AccessLog::open(horcrux::audit::resolve_log_path(None));
        let entries = log.tail(50).unwrap_or_default();
        let model = ai_status_info()["model"].as_str().map(|s| s.to_string());
        let ai = run_ai_check(entries, model).await;
        let done = AiEventRecord {
            id: new_id("evt"),
            ts: now_ms(),
            ts_utc: horcrux::audit::format_utc(now_ms()),
            action,
            derived,
            status: ai.status,
            summary: ai.summary,
            model: ai.model,
            severity: ai.severity,
            deterministic,
            details: Some(json!({"source_event": pending.id, "context": details })),
        };
        let _ = record_ai_event(done);
    });
}

// ----------------------------- status/log ---------------------------------

#[derive(Deserialize)]
pub struct LogQuery {
    tail: Option<usize>,
}

pub async fn status() -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true, "version": env!("CARGO_PKG_VERSION"), "bind": "127.0.0.1",
        "ai": ai_status_info(),
        "defaults": {
            "solana_rpc": horcrux::chain::default_rpc_url(),
            "cosmos_rpc": horcrux::cosmos::default_rpc_url(),
            "bitcoin_rpc": horcrux::bitcoin::default_rpc_url(),
        }
    })))
}

pub async fn log(Query(q): Query<LogQuery>) -> Result<Json<Value>, ApiError> {
    let log = AccessLog::open(horcrux::audit::resolve_log_path(None));
    let entries = match q.tail {
        Some(n) => log.tail(n)?,
        None => log.read_all()?,
    };
    let items: Vec<Value> = entries.iter().map(|e| json!({
        "ts": e.ts, "ts_utc": horcrux::audit::format_utc(e.ts), "attempt": e.attempt, "shard_id": e.shard_id, "kind": entry_kind_str(e.kind),
    })).collect();
    Ok(Json(json!({ "entries": items })))
}

// ----------------------------- verify -------------------------------------

#[derive(Deserialize)]
pub struct VerifyReq {
    files: Vec<String>,
    password: Option<String>,
}

pub async fn verify(Json(req): Json<VerifyReq>) -> Result<Json<Value>, ApiError> {
    let paths: Vec<PathBuf> = req.files.iter().map(PathBuf::from).collect();
    let reports = horcrux::verify::verify_files(&paths, req.password.as_deref());
    let consistency_error = horcrux::verify::consistency_error(&reports);
    let all_ok = consistency_error.is_none() && reports.iter().all(|r| r.ok);
    let items: Vec<Value> = reports.iter().map(|r| json!({
        "path": r.path, "ok": r.ok,
        "kind": r.kind.map(|k| match k { horcrux::verify::Kind::Sss => "sss", horcrux::verify::Kind::Frost => "frost" }),
        "threshold": r.params.map(|(t, _)| t), "shares": r.params.map(|(_, n)| n),
    })).collect();
    let derived = if all_ok {
        "shard_configured"
    } else {
        "shard_rejected"
    };
    spawn_action_ai_event(
        "verify",
        derived,
        json!({"files": req.files.len(), "all_ok": all_ok}),
        None,
    );
    Ok(Json(
        json!({ "reports": items, "consistency_error": consistency_error, "all_ok": all_ok }),
    ))
}

// ----------------------------- destinations/init/split --------------------

fn resolve_destinations(
    destinations: &[String],
    out_dir: &Path,
    prefix: &str,
) -> Option<Vec<PathBuf>> {
    if destinations.is_empty() || destinations.iter().all(|d| d.trim().is_empty()) {
        return None;
    }
    Some(
        destinations
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let dir = if d.trim().is_empty() {
                    out_dir.to_path_buf()
                } else {
                    PathBuf::from(d.trim())
                };
                dir.join(format!("{prefix}-{}.hx", i + 1))
            })
            .collect(),
    )
}

#[derive(Deserialize)]
pub struct InitReq {
    threshold: u8,
    shares: u8,
    key_hex: Option<String>,
    generate: bool,
    out_dir: String,
    passwords: Vec<String>,
    #[serde(default)]
    destinations: Vec<String>,
}

pub async fn init(Json(req): Json<InitReq>) -> Result<Json<Value>, ApiError> {
    let (key, generated_key_hex) = resolve_key(req.key_hex.as_deref(), req.generate)?;
    let out_dir = PathBuf::from(if req.out_dir.trim().is_empty() {
        "shards"
    } else {
        req.out_dir.trim()
    });
    let shares = req.shares;
    let threshold = req.threshold;
    let passwords = req.passwords;
    if passwords.len() != shares as usize {
        return Err(ApiError::bad(format!(
            "expected {shares} passwords, got {}",
            passwords.len()
        )));
    }
    if !req.destinations.is_empty() && req.destinations.len() != shares as usize {
        return Err(ApiError::bad(format!(
            "expected {shares} destinations, got {}",
            req.destinations.len()
        )));
    }
    let dest_paths = resolve_destinations(&req.destinations, &out_dir, "shard");
    let (out_dir_s, paths) = tokio::task::spawn_blocking(move || {
        let result = match dest_paths {
            Some(dests) => horcrux::init_shards_to(&key, threshold, shares, &passwords, &dests),
            None => horcrux::init_shards(&key, threshold, shares, &out_dir, &passwords),
        };
        result.map(|paths| {
            (
                out_dir.display().to_string(),
                paths
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>(),
            )
        })
    })
    .await
    .map_err(join_err)??;
    spawn_action_ai_event(
        "init",
        "shard_configured",
        json!({"mode":"sss","threshold":threshold,"shares":shares}),
        None,
    );
    Ok(Json(
        json!({ "generated_key_hex": generated_key_hex, "out_dir": out_dir_s, "paths": paths }),
    ))
}

#[derive(Deserialize, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MpcChain {
    #[default]
    Solana,
    Bitcoin,
}

#[derive(Deserialize)]
pub struct MpcSplitReq {
    threshold: u8,
    shares: u8,
    key_hex: Option<String>,
    generate: bool,
    out_dir: String,
    passwords: Vec<String>,
    #[serde(default)]
    destinations: Vec<String>,
    #[serde(default)]
    chain: MpcChain,
}

pub async fn mpc_split(Json(req): Json<MpcSplitReq>) -> Result<Json<Value>, ApiError> {
    let (key, generated_key_hex) = resolve_key(req.key_hex.as_deref(), req.generate)?;
    let out_dir = PathBuf::from(if req.out_dir.trim().is_empty() {
        "mpc"
    } else {
        req.out_dir.trim()
    });
    let shares = req.shares;
    let threshold = req.threshold;
    let passwords = req.passwords;
    let chain = req.chain;
    if passwords.len() != shares as usize {
        return Err(ApiError::bad(format!(
            "expected {shares} passwords, got {}",
            passwords.len()
        )));
    }
    if !req.destinations.is_empty() && req.destinations.len() != shares as usize {
        return Err(ApiError::bad(format!(
            "expected {shares} destinations, got {}",
            req.destinations.len()
        )));
    }
    let dest_paths = resolve_destinations(&req.destinations, &out_dir, "mpc");
    let (paths, group_path) =
        tokio::task::spawn_blocking(move || -> Result<(Vec<String>, String), Error> {
            match chain {
                MpcChain::Solana => {
                    let (p, g) = match dest_paths {
                        Some(dests) => horcrux::mpc::mpc_split_to(
                            &key, threshold, shares, &passwords, &dests, &out_dir,
                        ),
                        None => {
                            horcrux::mpc::mpc_split(&key, threshold, shares, &out_dir, &passwords)
                        }
                    }?;
                    Ok((
                        p.iter().map(|x| x.display().to_string()).collect(),
                        g.display().to_string(),
                    ))
                }
                MpcChain::Bitcoin => {
                    let (p, g) = horcrux::btc_mpc::btc_mpc_split(
                        &key, threshold, shares, &out_dir, &passwords,
                    )?;
                    Ok((
                        p.iter().map(|x| x.display().to_string()).collect(),
                        g.display().to_string(),
                    ))
                }
            }
        })
        .await
        .map_err(join_err)??;
    spawn_action_ai_event(
        "mpc_split",
        "shard_configured",
        json!({"mode":"frost","chain": match chain { MpcChain::Solana=>"solana", MpcChain::Bitcoin=>"bitcoin"}, "threshold":threshold,"shares":shares}),
        None,
    );
    Ok(Json(
        json!({ "generated_key_hex": generated_key_hex, "paths": paths, "group_path": group_path }),
    ))
}

pub async fn drives() -> Result<Json<Value>, ApiError> {
    // Return all mounted drives (removable first) so users can browse/select
    // inserted pendrives and other mount points for shard/share destinations.
    let list = tokio::task::spawn_blocking(horcrux::device::list_drives)
        .await
        .map_err(join_err)?;
    let items: Vec<Value> = list.iter().map(|d| json!({ "name": d.name, "mount_point": d.mount_point.display().to_string(), "total_bytes": d.total_bytes, "available_bytes": d.available_bytes, "removable": d.removable })).collect();
    Ok(Json(json!({ "drives": items })))
}

// ----------------------------- reconstruct (safe) -------------------------

#[derive(Deserialize)]
pub struct ReconstructReq {
    shards: Vec<String>,
    passwords: Vec<String>,
}

pub async fn reconstruct(Json(req): Json<ReconstructReq>) -> Result<Json<Value>, ApiError> {
    let shard_paths: Vec<PathBuf> = req.shards.iter().map(PathBuf::from).collect();
    if shard_paths.is_empty() {
        return Err(ApiError::bad("select at least one shard"));
    }
    if req.passwords.len() != shard_paths.len() {
        return Err(ApiError::bad(format!(
            "expected {} passwords, got {}",
            shard_paths.len(),
            req.passwords.len()
        )));
    }
    let ids = horcrux::audit::shard_ids(&shard_paths)?;
    let (log, attempt, warnings) = preflight(ids, false)?;
    let passwords = req.passwords;
    let result = tokio::task::spawn_blocking(move || -> Result<Value, Error> {
        let key = horcrux::reconstruct_with_audit(&shard_paths, &passwords, &log, attempt)?;
        let seed = horcrux::key_seed(&key);
        Ok(json!({
            "ok": true,
            "derived": {
                "solana": horcrux::tx::derive_address(&seed).to_string(),
                "bitcoin_p2tr_mainnet": horcrux::bitcoin::derive_address(&seed).ok().map(|a| a.to_string()),
                "cosmos_cosmos": horcrux::cosmos::derive_address(&seed, "cosmos").ok().map(|a| a.to_string()),
                "cosmos_osmo": horcrux::cosmos::derive_address(&seed, "osmo").ok().map(|a| a.to_string()),
            },
            "note": "Reconstruction succeeded in memory and was discarded. Raw private keys/seeds are never returned by the web console."
        }))
    }).await.map_err(join_err)?;
    match result {
        Ok(mut v) => {
            v["warnings"] = json!(warnings);
            spawn_action_ai_event("reconstruct", "shard_inserted", json!({"ok":true}), None);
            Ok(Json(v))
        }
        Err(e) => {
            spawn_action_ai_event("reconstruct", "shard_rejected", json!({"ok":false}), None);
            Err(ApiError::from(e))
        }
    }
}

// ----------------------------- sign (Mode A) ------------------------------

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ChainKind {
    #[default]
    Solana,
    Bitcoin,
    Cosmos,
}

#[derive(Deserialize)]
pub struct SignReq {
    shards: Vec<String>,
    passwords: Vec<String>,
    #[serde(default)]
    chain: ChainKind,
    to: String,
    broadcast: bool,
    rpc_url: Option<String>,
    #[serde(default)]
    force: bool,
    lamports: Option<u64>,
    blockhash: Option<String>,
    rpc_user: Option<String>,
    rpc_password: Option<String>,
    #[serde(default)]
    utxo: Vec<String>,
    amount_sat: Option<u64>,
    change_address: Option<String>,
    fee_sat: Option<u64>,
    chain_id: Option<String>,
    account_number: Option<u64>,
    sequence: Option<u64>,
    amount: Option<u128>,
    denom: Option<String>,
    gas: Option<u64>,
    fee: Option<u128>,
    fee_denom: Option<String>,
    #[serde(default)]
    memo: String,
    timeout_height: Option<u64>,
}

fn parse_utxo(s: &str) -> Result<horcrux::bitcoin::Utxo, ApiError> {
    let bad = || {
        ApiError::bad(format!(
            "invalid utxo {s:?} (expected txid:vout:amount_sat)"
        ))
    };
    let mut parts = s.split(':');
    let txid = parts.next().ok_or_else(bad)?;
    let vout = parts.next().ok_or_else(bad)?;
    let value = parts.next().ok_or_else(bad)?;
    if parts.next().is_some() {
        return Err(bad());
    }
    let outpoint: bitcoin::OutPoint = format!("{txid}:{vout}")
        .parse()
        .map_err(|e| ApiError::bad(format!("invalid utxo {s:?}: {e}")))?;
    let value_sat: u64 = value
        .parse()
        .map_err(|e| ApiError::bad(format!("invalid utxo amount in {s:?}: {e}")))?;
    Ok(horcrux::bitcoin::Utxo {
        outpoint,
        value_sat,
    })
}

pub async fn sign(Json(req): Json<SignReq>) -> Result<Json<Value>, ApiError> {
    let shard_paths: Vec<PathBuf> = req.shards.iter().map(PathBuf::from).collect();
    if shard_paths.is_empty() {
        return Err(ApiError::bad("select at least one shard"));
    }
    if req.passwords.len() != shard_paths.len() {
        return Err(ApiError::bad(format!(
            "expected {} passwords, got {}",
            shard_paths.len(),
            req.passwords.len()
        )));
    }
    let ids = horcrux::audit::shard_ids(&shard_paths)?;
    let (log, attempt, warnings) = preflight(ids, req.force)?;
    let chain = req.chain;
    let res = match chain {
        ChainKind::Solana => sign_solana(req, shard_paths, log, attempt, warnings).await,
        ChainKind::Bitcoin => sign_bitcoin(req, shard_paths, log, attempt, warnings).await,
        ChainKind::Cosmos => sign_cosmos(req, shard_paths, log, attempt, warnings).await,
    };
    let (derived, _ok) = match &res {
        Ok(_) => ("transaction_signed", true),
        Err(_) => ("shard_rejected", false),
    };
    let chain_s = match chain {
        ChainKind::Solana => "solana",
        ChainKind::Bitcoin => "bitcoin",
        ChainKind::Cosmos => "cosmos",
    };
    spawn_action_ai_event("sign", derived, json!({"chain":chain_s}), None);
    res
}

async fn sign_solana(
    req: SignReq,
    shard_paths: Vec<PathBuf>,
    log: AccessLog,
    attempt: u64,
    warnings: Vec<String>,
) -> Result<Json<Value>, ApiError> {
    let passwords = req.passwords;
    let to: solana_pubkey::Pubkey = req
        .to
        .trim()
        .parse()
        .map_err(|e| ApiError::bad(format!("invalid recipient address: {e}")))?;
    let lamports = req
        .lamports
        .ok_or_else(|| ApiError::bad("lamports is required for solana"))?;
    let rpc_url = req
        .rpc_url
        .clone()
        .unwrap_or_else(horcrux::chain::default_rpc_url);
    let chain_client = if req.broadcast {
        Some(horcrux::chain::Chain::connect(&rpc_url))
    } else {
        None
    };
    let blockhash: solana_hash::Hash = match req.blockhash.as_deref().filter(|s| !s.is_empty()) {
        Some(bh) => bh
            .parse()
            .map_err(|e| ApiError::bad(format!("invalid blockhash: {e}")))?,
        None => {
            let c = chain_client.as_ref().ok_or_else(|| {
                ApiError::bad("offline signing requires a blockhash (or enable broadcast)")
            })?;
            c.latest_blockhash().await?
        }
    };
    let signed: horcrux::tx::SignedTx = tokio::task::spawn_blocking(move || {
        let key = horcrux::reconstruct_with_audit(&shard_paths, &passwords, &log, attempt)?;
        let seed = horcrux::key_seed(&key);
        let from = horcrux::tx::derive_address(&seed);
        horcrux::tx::sign_transaction(
            *seed,
            horcrux::tx::TxParams {
                from,
                to,
                lamports,
                blockhash,
            },
        )
    })
    .await
    .map_err(join_err)??;
    let mut result = json!({ "chain":"solana", "from": signed.from().to_string(), "signature": signed.signature().to_string(), "raw_base58": signed.raw_base58(), "warnings": warnings });
    if req.broadcast {
        let c = chain_client.expect("broadcast implies chain client");
        match c.balance(&signed.from()).await {
            Ok(balance) if balance > 0 => {
                match horcrux::chain::broadcast(c.client(), signed.tx(), Duration::from_secs(1), 60)
                    .await
                {
                    Ok(sig) => result["mined_signature"] = json!(sig.to_string()),
                    Err(e) => result["broadcast_error"] = json!(e.to_string()),
                }
            }
            Ok(_) => {
                result["broadcast_error"] = json!(format!("sender {} is unfunded", signed.from()))
            }
            Err(e) => result["broadcast_error"] = json!(e.to_string()),
        }
    }
    Ok(Json(result))
}

async fn sign_bitcoin(
    req: SignReq,
    shard_paths: Vec<PathBuf>,
    log: AccessLog,
    attempt: u64,
    warnings: Vec<String>,
) -> Result<Json<Value>, ApiError> {
    let passwords = req.passwords;
    let recipient = horcrux::bitcoin::parse_address(&req.to)?;
    let utxos = req
        .utxo
        .iter()
        .map(|s| parse_utxo(s))
        .collect::<Result<Vec<_>, _>>()?;
    let amount_sat = req
        .amount_sat
        .ok_or_else(|| ApiError::bad("amount_sat is required for bitcoin"))?;
    let fee_sat = req
        .fee_sat
        .ok_or_else(|| ApiError::bad("fee_sat is required for bitcoin"))?;
    let change_address = req
        .change_address
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(horcrux::bitcoin::parse_address)
        .transpose()?;
    let signed: horcrux::bitcoin::SignedBitcoinTx = tokio::task::spawn_blocking(move || {
        let key = horcrux::reconstruct_with_audit(&shard_paths, &passwords, &log, attempt)?;
        let seed = horcrux::key_seed(&key);
        horcrux::bitcoin::sign_transaction(
            *seed,
            horcrux::bitcoin::BitcoinParams {
                utxos,
                recipient,
                amount_sat,
                change_address,
                fee_sat,
            },
        )
    })
    .await
    .map_err(join_err)??;
    let mut result = json!({ "chain":"bitcoin", "from": signed.sender().to_string(), "signature": hex::encode(signed.signature()), "raw_hex": signed.raw_hex(), "txid": signed.txid().to_string(), "fee_sat": signed.fee_sat(), "change_sat": signed.change_sat(), "warnings": warnings });
    if req.broadcast {
        let rpc_url = req
            .rpc_url
            .unwrap_or_else(horcrux::bitcoin::default_rpc_url);
        let (rpc_user, rpc_password, tx) = (req.rpc_user, req.rpc_password, signed.tx().clone());
        let b = tokio::task::spawn_blocking(move || {
            horcrux::bitcoin::broadcast(&rpc_url, rpc_user.as_deref(), rpc_password.as_deref(), &tx)
        })
        .await
        .map_err(join_err)?;
        match b {
            Ok(txid) => result["mined_signature"] = json!(txid.to_string()),
            Err(e) => result["broadcast_error"] = json!(e.to_string()),
        }
    }
    Ok(Json(result))
}

async fn sign_cosmos(
    req: SignReq,
    shard_paths: Vec<PathBuf>,
    log: AccessLog,
    attempt: u64,
    warnings: Vec<String>,
) -> Result<Json<Value>, ApiError> {
    let passwords = req.passwords;
    let to = horcrux::cosmos::parse_address(&req.to)?;
    let missing = |f: &str| ApiError::bad(format!("{f} is required for cosmos"));
    let chain_id = req.chain_id.ok_or_else(|| missing("chain_id"))?;
    let account_number = req
        .account_number
        .ok_or_else(|| missing("account_number"))?;
    let sequence = req.sequence.ok_or_else(|| missing("sequence"))?;
    let amount = req.amount.ok_or_else(|| missing("amount"))?;
    let denom = req.denom.ok_or_else(|| missing("denom"))?;
    let gas = req.gas.ok_or_else(|| missing("gas"))?;
    let fee_amount = req.fee.ok_or_else(|| missing("fee"))?;
    let (memo, timeout_height, fee_denom) = (
        req.memo,
        req.timeout_height.unwrap_or(0),
        req.fee_denom.unwrap_or_default(),
    );
    let rpc_url = req
        .rpc_url
        .clone()
        .unwrap_or_else(horcrux::cosmos::default_rpc_url);
    let signed: horcrux::cosmos::SignedCosmosTx = tokio::task::spawn_blocking(move || {
        let key = horcrux::reconstruct_with_audit(&shard_paths, &passwords, &log, attempt)?;
        let seed = horcrux::key_seed(&key);
        horcrux::cosmos::sign_transaction(
            *seed,
            horcrux::cosmos::CosmosParams {
                chain_id,
                account_number,
                sequence,
                to,
                amount,
                denom,
                memo,
                timeout_height,
                gas,
                fee_amount,
                fee_denom,
            },
        )
    })
    .await
    .map_err(join_err)??;
    let mut result = json!({ "chain":"cosmos", "from": signed.from().to_string(), "raw_hex": signed.raw_hex(), "warnings": warnings });
    if req.broadcast {
        match horcrux::cosmos::broadcast(&rpc_url, &signed).await {
            Ok(tx_hash) => result["mined_signature"] = json!(tx_hash),
            Err(e) => result["broadcast_error"] = json!(e.to_string()),
        }
    }
    Ok(Json(result))
}

// ----------------------------- mpc sign -----------------------------------

#[derive(Deserialize)]
pub struct MpcSignReq {
    #[serde(default)]
    chain: MpcChain,
    shares: Vec<String>,
    group_dir: String,
    passwords: Vec<String>,
    to: String,
    #[serde(default)]
    force: bool,
    broadcast: bool,
    rpc_url: Option<String>,
    lamports: Option<u64>,
    blockhash: Option<String>,
    #[serde(default)]
    utxo: Vec<String>,
    amount_sat: Option<u64>,
    change_address: Option<String>,
    fee_sat: Option<u64>,
    rpc_user: Option<String>,
    rpc_password: Option<String>,
}

pub async fn mpc_sign(Json(req): Json<MpcSignReq>) -> Result<Json<Value>, ApiError> {
    let res = match req.chain {
        MpcChain::Solana => mpc_sign_solana(req).await,
        MpcChain::Bitcoin => mpc_sign_bitcoin(req).await,
    };
    let derived = match &res {
        Ok(_) => "transaction_signed",
        Err(_) => "shard_rejected",
    };
    spawn_action_ai_event("mpc_sign", derived, json!({}), None);
    res
}

async fn mpc_sign_solana(req: MpcSignReq) -> Result<Json<Value>, ApiError> {
    let share_paths: Vec<PathBuf> = req.shares.iter().map(PathBuf::from).collect();
    if share_paths.is_empty() {
        return Err(ApiError::bad("select at least one key share"));
    }
    if req.passwords.len() != share_paths.len() {
        return Err(ApiError::bad(format!(
            "expected {} passwords, got {}",
            share_paths.len(),
            req.passwords.len()
        )));
    }
    let group_dir = PathBuf::from(if req.group_dir.trim().is_empty() {
        "mpc"
    } else {
        req.group_dir.trim()
    });
    let group_pub = group_dir.join(horcrux::mpc::GROUP_PUB_FILENAME);
    let to: solana_pubkey::Pubkey = req
        .to
        .trim()
        .parse()
        .map_err(|e| ApiError::bad(format!("invalid recipient address: {e}")))?;
    let lamports = req
        .lamports
        .ok_or_else(|| ApiError::bad("lamports is required for solana MPC sign"))?;
    let ids = horcrux::mpc::shard_ids(&share_paths)?;
    let (log, attempt, warnings) = preflight(ids, req.force)?;
    let verifying_key: [u8; 32] = horcrux::mpc::group_verifying_key(&group_pub)?;
    let from = solana_pubkey::Pubkey::from(verifying_key);
    let rpc_url = req
        .rpc_url
        .clone()
        .unwrap_or_else(horcrux::chain::default_rpc_url);
    let chain_client = if req.broadcast {
        Some(horcrux::chain::Chain::connect(&rpc_url))
    } else {
        None
    };
    let blockhash: solana_hash::Hash = match req.blockhash.as_deref().filter(|s| !s.is_empty()) {
        Some(bh) => bh
            .parse()
            .map_err(|e| ApiError::bad(format!("invalid blockhash: {e}")))?,
        None => {
            let c = chain_client.as_ref().ok_or_else(|| {
                ApiError::bad("offline signing requires a blockhash (or enable broadcast)")
            })?;
            c.latest_blockhash().await?
        }
    };
    if let Some(c) = &chain_client {
        if c.balance(&from).await? == 0 {
            return Err(ApiError::bad(format!("sender {from} is unfunded")));
        }
    }
    let params = horcrux::tx::TxParams {
        from,
        to,
        lamports,
        blockhash,
    };
    let passwords = req.passwords;
    let signed: horcrux::tx::SignedTx = tokio::task::spawn_blocking(move || {
        let message = horcrux::tx::transaction_message(&params);
        let sig = horcrux::mpc::mpc_sign_with_audit(
            &share_paths,
            &passwords,
            &group_pub,
            &message.serialize(),
            &log,
            attempt,
        )?;
        horcrux::tx::sign_transaction_with_signature(params, sig.signature, sig.verifying_key)
    })
    .await
    .map_err(join_err)??;
    let mut result = json!({ "chain":"solana", "from": signed.from().to_string(), "signature": signed.signature().to_string(), "raw_base58": signed.raw_base58(), "warnings": warnings });
    if req.broadcast {
        let c = chain_client.expect("broadcast implies chain client");
        match horcrux::chain::broadcast(c.client(), signed.tx(), Duration::from_secs(1), 60).await {
            Ok(sig) => result["mined_signature"] = json!(sig.to_string()),
            Err(e) => result["broadcast_error"] = json!(e.to_string()),
        }
    }
    Ok(Json(result))
}

async fn mpc_sign_bitcoin(req: MpcSignReq) -> Result<Json<Value>, ApiError> {
    let share_paths: Vec<PathBuf> = req.shares.iter().map(PathBuf::from).collect();
    if share_paths.is_empty() {
        return Err(ApiError::bad("select at least one key share"));
    }
    if req.passwords.len() != share_paths.len() {
        return Err(ApiError::bad(format!(
            "expected {} passwords, got {}",
            share_paths.len(),
            req.passwords.len()
        )));
    }
    let group_dir = PathBuf::from(if req.group_dir.trim().is_empty() {
        "mpc"
    } else {
        req.group_dir.trim()
    });
    let group_pub = group_dir.join(horcrux::btc_mpc::BTC_GROUP_PUB_FILENAME);
    let recipient = horcrux::bitcoin::parse_address(&req.to)?;
    let utxos = req
        .utxo
        .iter()
        .map(|s| parse_utxo(s))
        .collect::<Result<Vec<_>, _>>()?;
    let amount_sat = req
        .amount_sat
        .ok_or_else(|| ApiError::bad("amount_sat is required for bitcoin MPC sign"))?;
    let fee_sat = req
        .fee_sat
        .ok_or_else(|| ApiError::bad("fee_sat is required for bitcoin MPC sign"))?;
    let change_address = req
        .change_address
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(horcrux::bitcoin::parse_address)
        .transpose()?;
    let ids = horcrux::btc_mpc::shard_ids(&share_paths)?;
    let (log, attempt, warnings) = preflight(ids, req.force)?;
    let output_xonly = horcrux::btc_mpc::group_output_xonly(&group_pub)?;
    let params = horcrux::bitcoin::BitcoinParams {
        utxos,
        recipient,
        amount_sat,
        change_address,
        fee_sat,
    };
    let passwords = req.passwords;
    let signed: horcrux::bitcoin::SignedBitcoinTx = tokio::task::spawn_blocking(
        move || -> Result<horcrux::bitcoin::SignedBitcoinTx, Error> {
            let sighash = horcrux::bitcoin::unsigned_sighash(&output_xonly, &params)?;
            let sig = horcrux::btc_mpc::btc_mpc_sign_with_audit(
                &share_paths,
                &passwords,
                &group_pub,
                &sighash,
                &log,
                attempt,
            )?;
            horcrux::bitcoin::assemble_signed_transaction(sig.output_xonly, params, sig.signature)
        },
    )
    .await
    .map_err(join_err)??;
    let mut result = json!({ "chain":"bitcoin", "from": signed.sender().to_string(), "signature": hex::encode(signed.signature()), "raw_hex": signed.raw_hex(), "txid": signed.txid().to_string(), "fee_sat": signed.fee_sat(), "change_sat": signed.change_sat(), "warnings": warnings });
    if req.broadcast {
        let rpc_url = req
            .rpc_url
            .unwrap_or_else(horcrux::bitcoin::default_rpc_url);
        let (rpc_user, rpc_password, tx) = (req.rpc_user, req.rpc_password, signed.tx().clone());
        let b = tokio::task::spawn_blocking(move || {
            horcrux::bitcoin::broadcast(&rpc_url, rpc_user.as_deref(), rpc_password.as_deref(), &tx)
        })
        .await
        .map_err(join_err)?;
        match b {
            Ok(txid) => result["mined_signature"] = json!(txid.to_string()),
            Err(e) => result["broadcast_error"] = json!(e.to_string()),
        }
    }
    Ok(Json(result))
}

// ----------------------------- shard QR -----------------------------------

#[derive(Deserialize)]
pub struct ShardQrReq {
    path: String,
}

pub async fn shard_qr(Json(req): Json<ShardQrReq>) -> Result<Json<Value>, ApiError> {
    let path = PathBuf::from(req.path);
    let frames_b64 = tokio::task::spawn_blocking(move || -> Result<Vec<String>, Error> {
        let bytes = std::fs::read(&path).map_err(Error::Io)?;
        let session: [u8; horcrux::qr::SESSION_LEN] = rand::random();
        let frames =
            horcrux::qr::encode_message(horcrux::qr::MessageType::ShardFile, session, &bytes)?;
        frames
            .iter()
            .map(|f| Ok(BASE64.encode(horcrux::qr::encode_qr_png_bytes(f, 6)?)))
            .collect()
    })
    .await
    .map_err(join_err)??;
    Ok(Json(json!({ "frames": frames_b64 })))
}

#[derive(Deserialize)]
pub struct ShardQrImportReq {
    png_base64: String,
}

pub async fn shard_qr_import(Json(req): Json<ShardQrImportReq>) -> Result<Json<Value>, ApiError> {
    let bytes = BASE64
        .decode(req.png_base64.trim())
        .map_err(|e| ApiError::bad(format!("invalid base64: {e}")))?;
    let path = tokio::task::spawn_blocking(move || -> Result<PathBuf, Error> {
        let frame = horcrux::qr::decode_qr_png_bytes(&bytes)?;
        if frame.message_type != horcrux::qr::MessageType::ShardFile || frame.total != 1 {
            return Err(Error::Qr(
                "QR image is not a single-frame shard/share code".into(),
            ));
        }
        let tmp = std::env::temp_dir().join(format!(
            "horcrux-qr-import-{}.hx",
            hex::encode(rand::random::<[u8; 8]>())
        ));
        std::fs::write(&tmp, &frame.payload).map_err(Error::Io)?;
        Ok(tmp)
    })
    .await
    .map_err(join_err)??;
    spawn_action_ai_event(
        "qr_import",
        "shard_inserted",
        json!({"source":"qr_png"}),
        None,
    );
    Ok(Json(json!({ "path": path.display().to_string() })))
}

// ----------------------------- QR 5-step air-gap flow ---------------------

fn qr_frames_png_b64(frames: &[horcrux::qr::Frame]) -> Result<Vec<String>, Error> {
    frames
        .iter()
        .map(|f| Ok(BASE64.encode(horcrux::qr::encode_qr_png_bytes(f, 6)?)))
        .collect()
}

#[derive(Deserialize)]
pub struct QrRequestReq {
    group_dir: String,
    message_hex: String,
    #[serde(default)]
    out_dir: Option<String>,
}

pub async fn qr_request(Json(req): Json<QrRequestReq>) -> Result<Json<Value>, ApiError> {
    let group_dir = PathBuf::from(if req.group_dir.trim().is_empty() {
        "mpc"
    } else {
        req.group_dir.trim()
    });
    let group_pub = group_dir.join(horcrux::mpc::GROUP_PUB_FILENAME);
    let msg = hex::decode(req.message_hex.trim().trim_start_matches("0x"))
        .map_err(|e| ApiError::bad(format!("invalid message_hex: {e}")))?;
    let out_dir = PathBuf::from(req.out_dir.unwrap_or_else(|| "qr".into()));
    let (frames, session_hex) =
        tokio::task::spawn_blocking(move || -> Result<(Vec<String>, String), Error> {
            fs::create_dir_all(&out_dir).map_err(Error::Io)?;
            let group_bytes = fs::read(&group_pub).map_err(Error::Io)?;
            let rd = horcrux::qr_mpc::mpc_sign_request(&group_bytes, &msg)?;
            let payload = rd.to_payload();
            let frames = horcrux::qr::encode_message(
                horcrux::qr::MessageType::Request,
                rd.session,
                &payload,
            )?;
            horcrux::qr::write_message_transport(
                &out_dir,
                "request",
                horcrux::qr::Transport::Hx3,
                horcrux::qr::MessageType::Request,
                rd.session,
                &payload,
            )?;
            Ok((qr_frames_png_b64(&frames)?, hex::encode(rd.session)))
        })
        .await
        .map_err(join_err)??;
    spawn_action_ai_event(
        "qr_request",
        "transaction_initiated",
        json!({"session": session_hex}),
        None,
    );
    Ok(Json(json!({ "session": session_hex, "frames": frames })))
}

#[derive(Deserialize)]
pub struct QrCommitReq {
    dir: String,
    share_path: String,
    password: String,
    #[serde(default)]
    nonce_path: Option<String>,
    #[serde(default)]
    out_dir: Option<String>,
}

pub async fn qr_commit(Json(req): Json<QrCommitReq>) -> Result<Json<Value>, ApiError> {
    let dir = PathBuf::from(req.dir.clone());
    let share_path = PathBuf::from(req.share_path.clone());
    let password = req.password;
    let out_dir = PathBuf::from(req.out_dir.unwrap_or(req.dir));
    let nonce_path_req = req.nonce_path.map(PathBuf::from);
    let result = tokio::task::spawn_blocking(move || -> Result<Value, Error> {
        let (msg_type, session, payload) = horcrux::qr::read_message_transport(&dir, "request")?;
        if msg_type != horcrux::qr::MessageType::Request { return Err(Error::Qr("expected a signing request".into())); }
        let request = horcrux::qr_mpc::RequestData::from_payload(session, &payload)?;
        let participant = horcrux::qr_mpc::load_participant(&request, &share_path, &password, |_, _| {})?;
        let mut rng = rand::rngs::OsRng;
        let (commitment, nonces) = horcrux::qr_mpc::produce_commitment(&participant.key_package, &mut rng)?;
        let participant_id = commitment.participant_id;
        let nonce_path = nonce_path_req.unwrap_or_else(|| out_dir.join(format!("qr-nonce-{participant_id}-{}.bin", hex::encode(&session[..4]))));
        horcrux::qr_mpc::save_nonces(&nonce_path, &password, &session, &nonces)?;
        let c_payload = commitment.to_payload();
        let frames = horcrux::qr::encode_message(horcrux::qr::MessageType::Commitment, session, &c_payload)?;
        horcrux::qr::write_message_transport(&out_dir, &format!("commit-{participant_id}"), horcrux::qr::Transport::Hx3, horcrux::qr::MessageType::Commitment, session, &c_payload)?;
        Ok(json!({
            "session": hex::encode(session),
            "participant_id": participant_id,
            "nonce_path": nonce_path.display().to_string(),
            "frames": qr_frames_png_b64(&frames)?,
            "note": "Nonces kept locally until qr/share; do not copy the nonce file across the air gap."
        }))
    }).await.map_err(join_err)??;
    spawn_action_ai_event(
        "qr_commit",
        "shard_inserted",
        json!({"step":"commit"}),
        None,
    );
    Ok(Json(result))
}

#[derive(Deserialize)]
pub struct QrPackageReq {
    dir: String,
    #[serde(default)]
    out_dir: Option<String>,
}

pub async fn qr_package(Json(req): Json<QrPackageReq>) -> Result<Json<Value>, ApiError> {
    let dir = PathBuf::from(req.dir.clone());
    let out_dir = PathBuf::from(req.out_dir.unwrap_or(req.dir));
    let result = tokio::task::spawn_blocking(move || -> Result<Value, Error> {
        let (msg_type, session, payload) = horcrux::qr::read_message_transport(&dir, "request")?;
        if msg_type != horcrux::qr::MessageType::Request { return Err(Error::Qr("expected a signing request".into())); }
        let request = horcrux::qr_mpc::RequestData::from_payload(session, &payload)?;
        let ids = horcrux::qr::list_participant_ids(&dir, "commit")?;
        if ids.is_empty() { return Err(Error::Qr("no commitments found in dir (expected commit-<id>.hx3 or .png)".into())); }
        let mut commitments = Vec::with_capacity(ids.len());
        for id in &ids {
            let (t, s, p) = horcrux::qr::read_message_transport(&dir, &format!("commit-{id}"))?;
            if t != horcrux::qr::MessageType::Commitment || s != session { return Err(Error::Qr(format!("commit-{id} session mismatch"))); }
            commitments.push(horcrux::qr_mpc::CommitmentData::from_payload(&p)?);
        }
        let pkg_bytes = horcrux::qr_mpc::mpc_sign_package(&request, &commitments)?;
        let frames = horcrux::qr::encode_message(horcrux::qr::MessageType::SigningPackage, session, &pkg_bytes)?;
        horcrux::qr::write_message_transport(&out_dir, "package", horcrux::qr::Transport::Hx3, horcrux::qr::MessageType::SigningPackage, session, &pkg_bytes)?;
        Ok(json!({ "session": hex::encode(session), "participants": ids, "frames": qr_frames_png_b64(&frames)? }))
    }).await.map_err(join_err)??;
    spawn_action_ai_event(
        "qr_package",
        "shard_configured",
        json!({"step":"package"}),
        None,
    );
    Ok(Json(result))
}

#[derive(Deserialize)]
pub struct QrShareReq {
    dir: String,
    share_path: String,
    password: String,
    nonce_path: String,
    #[serde(default)]
    out_dir: Option<String>,
}

pub async fn qr_share(Json(req): Json<QrShareReq>) -> Result<Json<Value>, ApiError> {
    let dir = PathBuf::from(req.dir.clone());
    let share_path = PathBuf::from(req.share_path);
    let nonce_path = PathBuf::from(req.nonce_path);
    let password = req.password;
    let out_dir = PathBuf::from(req.out_dir.unwrap_or(req.dir));
    let result = tokio::task::spawn_blocking(move || -> Result<Value, Error> {
        let (rt, session, rp) = horcrux::qr::read_message_transport(&dir, "request")?;
        if rt != horcrux::qr::MessageType::Request { return Err(Error::Qr("expected request".into())); }
        let request = horcrux::qr_mpc::RequestData::from_payload(session, &rp)?;
        let (pt, ps, pp) = horcrux::qr::read_message_transport(&dir, "package")?;
        if pt != horcrux::qr::MessageType::SigningPackage || ps != session { return Err(Error::Qr("expected matching signing package".into())); }
        let package_bytes = pp;
        let participant = horcrux::qr_mpc::load_participant(&request, &share_path, &password, |_, _| {})?;
        let nonces = horcrux::qr_mpc::load_nonces(&nonce_path, &password, &session)?;
        let share = horcrux::qr_mpc::produce_signature_share(&request, &package_bytes, &participant.key_package, nonces)?;
        let _ = fs::remove_file(&nonce_path);
        let participant_id = share.participant_id;
        let s_payload = share.to_payload();
        let frames = horcrux::qr::encode_message(horcrux::qr::MessageType::SignatureShare, session, &s_payload)?;
        horcrux::qr::write_message_transport(&out_dir, &format!("share-{participant_id}"), horcrux::qr::Transport::Hx3, horcrux::qr::MessageType::SignatureShare, session, &s_payload)?;
        Ok(json!({ "session": hex::encode(session), "participant_id": participant_id, "frames": qr_frames_png_b64(&frames)? }))
    }).await.map_err(join_err)??;
    spawn_action_ai_event("qr_share", "shard_inserted", json!({"step":"share"}), None);
    Ok(Json(result))
}

#[derive(Deserialize)]
pub struct QrFinalizeReq {
    dir: String,
}

pub async fn qr_finalize(Json(req): Json<QrFinalizeReq>) -> Result<Json<Value>, ApiError> {
    let dir = PathBuf::from(req.dir);
    let result = tokio::task::spawn_blocking(move || -> Result<Value, Error> {
        let (rt, session, rp) = horcrux::qr::read_message_transport(&dir, "request")?;
        if rt != horcrux::qr::MessageType::Request {
            return Err(Error::Qr("expected request".into()));
        }
        let request = horcrux::qr_mpc::RequestData::from_payload(session, &rp)?;
        let (pt, ps, pp) = horcrux::qr::read_message_transport(&dir, "package")?;
        if pt != horcrux::qr::MessageType::SigningPackage || ps != session {
            return Err(Error::Qr("expected matching signing package".into()));
        }
        let package_bytes = pp;
        let ids = horcrux::qr::list_participant_ids(&dir, "share")?;
        if ids.is_empty() {
            return Err(Error::Qr(
                "no signature shares found (expected share-<id>.hx3 or .png)".into(),
            ));
        }
        let mut shares = Vec::with_capacity(ids.len());
        for id in &ids {
            let (t, s, p) = horcrux::qr::read_message_transport(&dir, &format!("share-{id}"))?;
            if t != horcrux::qr::MessageType::SignatureShare || s != session {
                return Err(Error::Qr("signature share session mismatch".into()));
            }
            shares.push(horcrux::qr_mpc::SignatureShareData::from_payload(&p)?);
        }
        let sig = horcrux::qr_mpc::finalize_signature(&package_bytes, &shares, &request.group_pub)?;
        let raw_base58 = bincode::deserialize::<solana_message::Message>(&request.message)
            .ok()
            .and_then(|msg| {
                horcrux::tx::assemble_signed_transaction(msg, sig.signature, sig.verifying_key)
                    .ok()
                    .map(|tx| tx.raw_base58())
            });
        Ok(json!({
            "session": hex::encode(session),
            "signature_hex": hex::encode(sig.signature),
            "verifying_key_hex": hex::encode(sig.verifying_key),
            "raw_base58": raw_base58,
            "participants": ids
        }))
    })
    .await
    .map_err(join_err)??;
    spawn_action_ai_event(
        "qr_finalize",
        "transaction_signed",
        json!({"step":"finalize"}),
        None,
    );
    Ok(Json(result))
}

// ----------------------------- AI endpoints --------------------------------

pub async fn ai_status() -> Result<Json<Value>, ApiError> {
    Ok(Json(ai_status_info()))
}

#[derive(Deserialize)]
pub struct AiConfigureReq {
    api_key: Option<String>,
    model: Option<String>,
}

fn upsert_env_var(path: &Path, key: &str, value: &str) -> Result<(), ApiError> {
    let _g = FILE_LOCK.lock().ok();
    let mut lines: Vec<String> = if path.exists() {
        fs::read_to_string(path)
            .map_err(ApiError::from)?
            .lines()
            .map(|s| s.to_string())
            .collect()
    } else {
        Vec::new()
    };
    let prefix = format!("{key}=");
    let mut replaced = false;
    for line in &mut lines {
        if line.starts_with(&prefix) {
            *line = format!("{key}={value}");
            replaced = true;
        }
    }
    if !replaced {
        lines.push(format!("{key}={value}"));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(ApiError::from)?;
    }
    fs::write(path, lines.join("\n") + "\n").map_err(ApiError::from)
}

pub async fn ai_configure(Json(req): Json<AiConfigureReq>) -> Result<Json<Value>, ApiError> {
    if let Some(k) = req.api_key.as_deref().filter(|s| !s.trim().is_empty()) {
        upsert_env_var(&dotenv_path(), "OPENROUTER_API_KEY", k.trim())?;
        // SAFETY: local single-user console process; best-effort runtime config update.
        unsafe {
            std::env::set_var("OPENROUTER_API_KEY", k.trim());
        }
    }
    if let Some(m) = req.model.as_deref().filter(|s| !s.trim().is_empty()) {
        let mut st: WebState = read_json_file(&web_state_path());
        st.ai_model = Some(m.trim().to_string());
        write_json_file(&web_state_path(), &st)?;
        // SAFETY: local single-user console process; best-effort runtime config update.
        unsafe {
            std::env::set_var("HORCRUX_AI_MODEL", m.trim());
        }
    }
    spawn_action_ai_event(
        "ai_configure",
        "ai_configured",
        json!({"configured": ai_status_info()["configured"]}),
        None,
    );
    Ok(Json(ai_status_info()))
}

#[derive(Deserialize)]
pub struct AiCheckReq {
    tail: Option<usize>,
    model: Option<String>,
}

pub async fn ai_check(Json(req): Json<AiCheckReq>) -> Result<Json<Value>, ApiError> {
    let n = req.tail.unwrap_or(50).min(500);
    let log = AccessLog::open(horcrux::audit::resolve_log_path(None));
    let entries = log.tail(n)?;
    let ev = run_ai_check(entries, req.model).await;
    let saved = record_ai_event(ev)?;
    Ok(Json(
        serde_json::to_value(saved).map_err(|e| ApiError::internal(e.to_string()))?,
    ))
}

#[derive(Deserialize)]
pub struct AiEventsQuery {
    tail: Option<usize>,
}

pub async fn ai_events(Query(q): Query<AiEventsQuery>) -> Result<Json<Value>, ApiError> {
    let n = q.tail.unwrap_or(100).min(1000);
    Ok(Json(json!({ "events": read_ai_events(Some(n)) })))
}

// ----------------------------- wallets ------------------------------------

#[derive(Deserialize)]
pub struct WalletCreateReq {
    label: String,
    chain: String,
    network: String,
    mode: String,
    threshold: u8,
    shares: u8,
    passwords: Vec<String>,
    out_dir: Option<String>,
    #[serde(default)]
    destinations: Vec<String>,
    #[serde(default)]
    cosmos_prefix: Option<String>,
}

pub async fn wallets_list() -> Result<Json<Value>, ApiError> {
    let store: WalletStore = read_json_file(&wallets_path());
    Ok(Json(json!({ "wallets": store.wallets })))
}

pub async fn wallets_create(Json(req): Json<WalletCreateReq>) -> Result<Json<Value>, ApiError> {
    let label = req.label.trim().to_string();
    if label.is_empty() {
        return Err(ApiError::bad("label is required"));
    }
    let chain = req.chain.to_lowercase();
    if chain != "solana" && chain != "cosmos" {
        return Err(ApiError::bad("chain must be solana or cosmos"));
    }
    let mode = req.mode.to_lowercase();
    if mode != "sss" && mode != "frost" {
        return Err(ApiError::bad("mode must be sss or frost"));
    }
    if req.passwords.len() != req.shares as usize {
        return Err(ApiError::bad("passwords length must equal shares"));
    }
    if mode == "frost" && chain == "cosmos" {
        return Err(ApiError::bad(
            "FROST (MPC) wallets are not supported for Cosmos; use SSS (Mode A)",
        ));
    }
    let key = k256::SecretKey::random(&mut rand::rngs::OsRng);
    let generated_key_hex = hex::encode(key.to_bytes());
    let seed = *horcrux::key_seed(&key);
    let address = if chain == "solana" {
        horcrux::tx::derive_address(&seed).to_string()
    } else {
        horcrux::cosmos::derive_address(&seed, req.cosmos_prefix.as_deref().unwrap_or("cosmos"))
            .map_err(ApiError::from)?
            .to_string()
    };
    let out_dir = PathBuf::from(req.out_dir.unwrap_or_else(|| {
        if mode == "sss" {
            "shards".into()
        } else {
            "mpc".into()
        }
    }));
    let (shares, threshold, passwords) = (req.shares, req.threshold, req.passwords.clone());
    let dest_paths = resolve_destinations(
        &req.destinations,
        &out_dir,
        if mode == "sss" { "shard" } else { "mpc" },
    );
    let mode_for_split = mode.clone();
    let (paths, group_path): (Vec<String>, Option<String>) =
        tokio::task::spawn_blocking(move || -> Result<(Vec<String>, Option<String>), Error> {
            if mode_for_split == "sss" {
                let p = match dest_paths {
                    Some(d) => horcrux::init_shards_to(&key, threshold, shares, &passwords, &d),
                    None => horcrux::init_shards(&key, threshold, shares, &out_dir, &passwords),
                }?;
                Ok((p.iter().map(|x| x.display().to_string()).collect(), None))
            } else {
                let (p, g) = match dest_paths {
                    Some(d) => horcrux::mpc::mpc_split_to(
                        &key, threshold, shares, &passwords, &d, &out_dir,
                    ),
                    None => horcrux::mpc::mpc_split(&key, threshold, shares, &out_dir, &passwords),
                }?;
                Ok((
                    p.iter().map(|x| x.display().to_string()).collect(),
                    Some(g.display().to_string()),
                ))
            }
        })
        .await
        .map_err(join_err)??;
    let profile = WalletProfile {
        id: new_id("wal"),
        label,
        kind: "shard_backed".into(),
        chain: chain.clone(),
        network: req.network,
        address,
        protected: true,
        created_at: now_ms(),
        source: Some(format!("{mode}/generated")),
        shard_refs: Some(paths.clone()),
        group_ref: group_path.clone(),
        notes: Some(
            "Horcrux-protected (signing requires shards/MPC). Private key material not stored."
                .into(),
        ),
    };
    let mut store: WalletStore = read_json_file(&wallets_path());
    store.wallets.push(profile.clone());
    write_json_file(&wallets_path(), &store)?;
    spawn_action_ai_event(
        "wallet_create",
        "wallet_configured",
        json!({"chain": chain, "mode": mode, "address": profile.address}),
        None,
    );
    Ok(Json(
        json!({ "wallet": profile, "generated_key_hex": generated_key_hex, "paths": paths, "group_path": group_path }),
    ))
}

#[derive(Deserialize)]
pub struct WalletImportReq {
    label: String,
    chain: String,
    network: String,
    mode: String,
    threshold: u8,
    shares: u8,
    passwords: Vec<String>,
    out_dir: Option<String>,
    #[serde(default)]
    destinations: Vec<String>,
    key_hex: String,
    #[serde(default)]
    cosmos_prefix: Option<String>,
}

pub async fn wallets_import(Json(req): Json<WalletImportReq>) -> Result<Json<Value>, ApiError> {
    let label = req.label.trim().to_string();
    if label.is_empty() {
        return Err(ApiError::bad("label is required"));
    }
    let chain = req.chain.to_lowercase();
    if chain != "solana" && chain != "cosmos" {
        return Err(ApiError::bad("chain must be solana or cosmos"));
    }
    let mode = req.mode.to_lowercase();
    if mode != "sss" && mode != "frost" {
        return Err(ApiError::bad("mode must be sss or frost"));
    }
    if req.passwords.len() != req.shares as usize {
        return Err(ApiError::bad("passwords length must equal shares"));
    }
    if mode == "frost" && chain == "cosmos" {
        return Err(ApiError::bad(
            "FROST (MPC) wallets are not supported for Cosmos",
        ));
    }
    let key = parse_key(&req.key_hex)?;
    let seed = *horcrux::key_seed(&key);
    let address = if chain == "solana" {
        horcrux::tx::derive_address(&seed).to_string()
    } else {
        horcrux::cosmos::derive_address(&seed, req.cosmos_prefix.as_deref().unwrap_or("cosmos"))
            .map_err(ApiError::from)?
            .to_string()
    };
    let out_dir = PathBuf::from(req.out_dir.unwrap_or_else(|| {
        if mode == "sss" {
            "shards".into()
        } else {
            "mpc".into()
        }
    }));
    let (shares, threshold, passwords) = (req.shares, req.threshold, req.passwords.clone());
    let dest_paths = resolve_destinations(
        &req.destinations,
        &out_dir,
        if mode == "sss" { "shard" } else { "mpc" },
    );
    let mode_for_split = mode.clone();
    let (paths, group_path): (Vec<String>, Option<String>) =
        tokio::task::spawn_blocking(move || -> Result<(Vec<String>, Option<String>), Error> {
            if mode_for_split == "sss" {
                let p = match dest_paths {
                    Some(d) => horcrux::init_shards_to(&key, threshold, shares, &passwords, &d),
                    None => horcrux::init_shards(&key, threshold, shares, &out_dir, &passwords),
                }?;
                Ok((p.iter().map(|x| x.display().to_string()).collect(), None))
            } else {
                let (p, g) = match dest_paths {
                    Some(d) => horcrux::mpc::mpc_split_to(
                        &key, threshold, shares, &passwords, &d, &out_dir,
                    ),
                    None => horcrux::mpc::mpc_split(&key, threshold, shares, &out_dir, &passwords),
                }?;
                Ok((
                    p.iter().map(|x| x.display().to_string()).collect(),
                    Some(g.display().to_string()),
                ))
            }
        })
        .await
        .map_err(join_err)??;
    let profile = WalletProfile { id: new_id("wal"), label, kind: "shard_backed".into(), chain: chain.clone(), network: req.network, address, protected: true, created_at: now_ms(), source: Some(format!("{mode}/imported")), shard_refs: Some(paths.clone()), group_ref: group_path.clone(), notes: Some("Horcrux-protected. Imported key material was used transiently to split and was not stored.".into()) };
    let mut store: WalletStore = read_json_file(&wallets_path());
    store.wallets.push(profile.clone());
    write_json_file(&wallets_path(), &store)?;
    spawn_action_ai_event(
        "wallet_import",
        "wallet_configured",
        json!({"chain": chain, "mode": mode, "address": profile.address}),
        None,
    );
    Ok(Json(
        json!({ "wallet": profile, "paths": paths, "group_path": group_path }),
    ))
}

#[derive(Deserialize)]
pub struct WalletConnectReq {
    label: String,
    chain: String,
    network: String,
    address: String,
    #[serde(default)]
    source: Option<String>,
}

pub async fn wallets_connect(Json(req): Json<WalletConnectReq>) -> Result<Json<Value>, ApiError> {
    let label = req.label.trim().to_string();
    if label.is_empty() {
        return Err(ApiError::bad("label is required"));
    }
    let chain = req.chain.to_lowercase();
    if chain != "solana" && chain != "cosmos" {
        return Err(ApiError::bad("chain must be solana or cosmos"));
    }
    let address = req.address.trim().to_string();
    if chain == "solana" {
        let _: solana_pubkey::Pubkey = address
            .parse()
            .map_err(|e| ApiError::bad(format!("invalid solana address: {e}")))?;
    } else {
        horcrux::cosmos::parse_address(&address)?;
    }
    let profile = WalletProfile { id: new_id("wal"), label, kind: "browser_connected".into(), chain: chain.clone(), network: req.network, address: address.clone(), protected: false, created_at: now_ms(), source: req.source.or(Some("browser_wallet".into())), shard_refs: None, group_ref: None, notes: Some("Read-only browser-connected wallet. Not Horcrux-protected and has no Horcrux signing authority.".into()) };
    let mut store: WalletStore = read_json_file(&wallets_path());
    store.wallets.push(profile.clone());
    write_json_file(&wallets_path(), &store)?;
    spawn_action_ai_event(
        "wallet_connect",
        "wallet_connected",
        json!({"chain": chain, "address": address, "protected": false}),
        None,
    );
    Ok(Json(json!({ "wallet": profile })))
}

#[derive(Deserialize)]
pub struct WalletWatchReq {
    label: String,
    chain: String,
    network: String,
    address: String,
}

pub async fn wallets_watch(Json(req): Json<WalletWatchReq>) -> Result<Json<Value>, ApiError> {
    let label = req.label.trim().to_string();
    if label.is_empty() {
        return Err(ApiError::bad("label is required"));
    }
    let chain = req.chain.to_lowercase();
    if chain != "solana" && chain != "cosmos" {
        return Err(ApiError::bad("chain must be solana or cosmos"));
    }
    let address = req.address.trim().to_string();
    if chain == "solana" {
        let _: solana_pubkey::Pubkey = address
            .parse()
            .map_err(|e| ApiError::bad(format!("invalid solana address: {e}")))?;
    } else {
        horcrux::cosmos::parse_address(&address)?;
    }
    let profile = WalletProfile {
        id: new_id("wal"),
        label,
        kind: "watch_only".into(),
        chain: chain.clone(),
        network: req.network,
        address: address.clone(),
        protected: false,
        created_at: now_ms(),
        source: Some("watch_only".into()),
        shard_refs: None,
        group_ref: None,
        notes: Some("Watch-only address. Not Horcrux-protected.".into()),
    };
    let mut store: WalletStore = read_json_file(&wallets_path());
    store.wallets.push(profile.clone());
    write_json_file(&wallets_path(), &store)?;
    spawn_action_ai_event(
        "wallet_watch",
        "wallet_added",
        json!({"chain": chain, "address": address}),
        None,
    );
    Ok(Json(json!({ "wallet": profile })))
}

pub async fn wallets_delete(AxumPath(id): AxumPath<String>) -> Result<Json<Value>, ApiError> {
    let mut store: WalletStore = read_json_file(&wallets_path());
    let before = store.wallets.len();
    store.wallets.retain(|w| w.id != id);
    if store.wallets.len() == before {
        return Err(ApiError::message(StatusCode::NOT_FOUND, "wallet not found"));
    }
    write_json_file(&wallets_path(), &store)?;
    spawn_action_ai_event("wallet_delete", "wallet_removed", json!({"id": id}), None);
    Ok(Json(json!({ "ok": true })))
}

// ----------------------------- on-chain -----------------------------------

#[derive(Deserialize)]
pub struct OnchainQuery {
    chain: String,
    network: String,
    address: String,
    rpc_url: Option<String>,
    limit: Option<usize>,
}

fn solana_rpc_for(network: &str, override_url: Option<String>) -> Result<String, ApiError> {
    if let Some(u) = override_url.filter(|s| !s.trim().is_empty()) {
        return Ok(u);
    }
    Ok(match network {
        "localnet" => "http://127.0.0.1:8899".into(),
        "devnet" => "https://api.devnet.solana.com".into(),
        _ => {
            return Err(ApiError::bad(
                "unsupported solana network; use localnet, devnet, or provide rpc_url",
            ));
        }
    })
}
fn cosmos_rpc_for(network: &str, override_url: Option<String>) -> Result<String, ApiError> {
    if let Some(u) = override_url.filter(|s| !s.trim().is_empty()) {
        return Ok(u);
    }
    Ok(match network {
        "local" => "http://127.0.0.1:26657".into(),
        _ => {
            return Err(ApiError::bad(
                "unsupported cosmos network; use local or provide rpc_url",
            ));
        }
    })
}

pub async fn onchain_balance(Query(q): Query<OnchainQuery>) -> Result<Json<Value>, ApiError> {
    match q.chain.as_str() {
        "solana" => {
            let rpc_url = solana_rpc_for(&q.network, q.rpc_url)?;
            let addr: solana_pubkey::Pubkey = q
                .address
                .parse()
                .map_err(|e| ApiError::bad(format!("invalid solana address: {e}")))?;
            let client = solana_rpc_client::nonblocking::rpc_client::RpcClient::new_with_commitment(
                rpc_url,
                solana_commitment_config::CommitmentConfig::confirmed(),
            );
            let lamports = client
                .get_balance(&addr)
                .await
                .map_err(|e| Error::Tx(format!("RPC error: {e}")))?;
            Ok(Json(
                json!({ "chain":"solana", "network": q.network, "address": addr.to_string(), "balance": lamports.to_string(), "unit":"lamports", "sol": (lamports as f64)/1_000_000_000.0 }),
            ))
        }
        "cosmos" => {
            let rpc_url = cosmos_rpc_for(&q.network, q.rpc_url)?;
            let addr = horcrux::cosmos::parse_address(&q.address)?;
            Ok(Json(
                json!({ "chain":"cosmos", "network": q.network, "address": addr.to_string(), "balance": null, "unit": "unknown", "note": "Cosmos local balance via raw Tendermint RPC is not universally available without LCD/gRPC bank query endpoints. Use a local endpoint with bank query/indexing support for full balance/history.", "rpc_url": rpc_url }),
            ))
        }
        _ => Err(ApiError::bad("chain must be solana or cosmos")),
    }
}

pub async fn onchain_transactions(Query(q): Query<OnchainQuery>) -> Result<Json<Value>, ApiError> {
    let limit = q.limit.unwrap_or(20).min(100);
    match q.chain.as_str() {
        "solana" => {
            let rpc_url = solana_rpc_for(&q.network, q.rpc_url)?;
            let addr: solana_pubkey::Pubkey = q
                .address
                .parse()
                .map_err(|e| ApiError::bad(format!("invalid solana address: {e}")))?;
            let client = solana_rpc_client::nonblocking::rpc_client::RpcClient::new_with_commitment(
                rpc_url,
                solana_commitment_config::CommitmentConfig::confirmed(),
            );
            let sigs = client
                .get_signatures_for_address(&addr)
                .await
                .map_err(|e| Error::Tx(format!("RPC error: {e}")))?;
            let txs: Vec<Value> = sigs.into_iter().take(limit).map(|s| json!({ "signature": s.signature, "slot": s.slot, "timestamp": s.block_time, "status": if s.err.is_none() { "success" } else { "failed" }, "err": s.err.map(|e| e.to_string()), "memo": s.memo })).collect();
            Ok(Json(
                json!({ "chain":"solana", "network": q.network, "address": addr.to_string(), "transactions": txs, "count": txs.len() }),
            ))
        }
        "cosmos" => {
            let rpc_url = cosmos_rpc_for(&q.network, q.rpc_url)?;
            let addr = horcrux::cosmos::parse_address(&q.address)?;
            Ok(Json(
                json!({ "chain":"cosmos", "network": q.network, "address": addr.to_string(), "transactions": [], "note": "Cosmos transaction history via raw Tendermint RPC depends on tx indexing (tx_index=index). If unavailable, use a local LCD/gRPC indexer or node query tools.", "rpc_url": rpc_url }),
            ))
        }
        _ => Err(ApiError::bad("chain must be solana or cosmos")),
    }
}
