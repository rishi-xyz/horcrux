//! JSON API handlers backing the web UI. Each endpoint wraps the same
//! library functions the CLI and TUI call — no crypto logic is duplicated
//! here, only request parsing, the audit pre-flight, and JSON shaping.

use axum::Json;
use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use horcrux::audit::{AccessLog, Entry, EntryKind, Scorer, Verdict};
use horcrux::error::Error;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;
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

    fn blocked(reasons: Vec<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            body: json!({ "error": "blocked", "reasons": reasons }),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}

impl From<Error> for ApiError {
    fn from(e: Error) -> Self {
        ApiError::message(StatusCode::BAD_REQUEST, e.to_string())
    }
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        ApiError::message(StatusCode::BAD_REQUEST, e.to_string())
    }
}

fn join_err(e: tokio::task::JoinError) -> ApiError {
    ApiError::message(
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("worker task panicked: {e}"),
    )
}

/// Parse a hex-encoded private key, or generate a disposable test key.
/// Mirrors the CLI's `parse_key` / TUI's `Init`/`Mpc` screens (each keeps
/// their own copy of this rather than sharing one across the CLI, TUI and
/// web front ends).
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
        (None, false) => Err(ApiError::message(
            StatusCode::BAD_REQUEST,
            "provide either key_hex or generate",
        )),
    }
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

/// Score a proposed attempt against the access log, exactly like the CLI's
/// `audit_preflight` / the TUI's `try_submit`: a `Block` verdict is always
/// logged and refuses the operation unless `force` is set (still logged when
/// overridden); a `Warn` proceeds immediately with the reasons surfaced back
/// to the caller.
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

#[derive(Deserialize)]
pub struct LogQuery {
    tail: Option<usize>,
}

pub async fn log(Query(q): Query<LogQuery>) -> Result<Json<Value>, ApiError> {
    let log = AccessLog::open(horcrux::audit::resolve_log_path(None));
    let entries = match q.tail {
        Some(n) => log.tail(n)?,
        None => log.read_all()?,
    };
    let items: Vec<Value> = entries
        .iter()
        .map(|e| {
            json!({
                "ts": e.ts,
                "ts_utc": horcrux::audit::format_utc(e.ts),
                "attempt": e.attempt,
                "shard_id": e.shard_id,
                "kind": entry_kind_str(e.kind),
            })
        })
        .collect();
    Ok(Json(json!({ "entries": items })))
}

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
    let items: Vec<Value> = reports
        .iter()
        .map(|r| {
            json!({
                "path": r.path,
                "ok": r.ok,
                "kind": r.kind.map(|k| match k {
                    horcrux::verify::Kind::Sss => "sss",
                    horcrux::verify::Kind::Frost => "frost",
                }),
                "threshold": r.params.map(|(t, _)| t),
                "shares": r.params.map(|(_, n)| n),
            })
        })
        .collect();
    Ok(Json(json!({
        "reports": items,
        "consistency_error": consistency_error,
        "all_ok": all_ok,
    })))
}

#[derive(Deserialize)]
pub struct InitReq {
    threshold: u8,
    shares: u8,
    key_hex: Option<String>,
    generate: bool,
    out_dir: String,
    password: String,
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
    let password = req.password;

    let paths = tokio::task::spawn_blocking(move || {
        let passwords = vec![password; shares as usize];
        horcrux::init_shards(&key, threshold, shares, &out_dir, &passwords).map(|paths| {
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

    let (out_dir, paths) = paths;
    Ok(Json(json!({
        "generated_key_hex": generated_key_hex,
        "out_dir": out_dir,
        "paths": paths,
    })))
}

#[derive(Deserialize)]
pub struct MpcSplitReq {
    threshold: u8,
    shares: u8,
    key_hex: Option<String>,
    generate: bool,
    out_dir: String,
    password: String,
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
    let password = req.password;

    let (paths, group_path) = tokio::task::spawn_blocking(move || {
        let passwords = vec![password; shares as usize];
        horcrux::mpc::mpc_split(&key, threshold, shares, &out_dir, &passwords)
    })
    .await
    .map_err(join_err)??;

    Ok(Json(json!({
        "generated_key_hex": generated_key_hex,
        "paths": paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
        "group_path": group_path.display().to_string(),
    })))
}

#[derive(Deserialize)]
pub struct SignReq {
    shards: Vec<String>,
    password: String,
    to: String,
    lamports: u64,
    blockhash: Option<String>,
    broadcast: bool,
    rpc_url: Option<String>,
    #[serde(default)]
    force: bool,
}

pub async fn sign(Json(req): Json<SignReq>) -> Result<Json<Value>, ApiError> {
    let SignReq {
        shards,
        password,
        to,
        lamports,
        blockhash,
        broadcast,
        rpc_url,
        force,
    } = req;

    let shard_paths: Vec<PathBuf> = shards.iter().map(PathBuf::from).collect();
    if shard_paths.is_empty() {
        return Err(ApiError::message(
            StatusCode::BAD_REQUEST,
            "select at least one shard",
        ));
    }
    let to: solana_pubkey::Pubkey = to.trim().parse().map_err(|e| {
        ApiError::message(
            StatusCode::BAD_REQUEST,
            format!("invalid recipient address: {e}"),
        )
    })?;

    let ids = horcrux::audit::shard_ids(&shard_paths)?;
    let (log, attempt, warnings) = preflight(ids, force)?;

    let rpc_url = rpc_url.unwrap_or_else(horcrux::chain::default_rpc_url);
    let chain = if broadcast {
        Some(horcrux::chain::Chain::connect(&rpc_url))
    } else {
        None
    };

    let blockhash: solana_hash::Hash = match blockhash.as_deref().filter(|s| !s.is_empty()) {
        Some(bh) => bh.parse().map_err(|e| {
            ApiError::message(StatusCode::BAD_REQUEST, format!("invalid blockhash: {e}"))
        })?,
        None => {
            let chain = chain.as_ref().ok_or_else(|| {
                ApiError::message(
                    StatusCode::BAD_REQUEST,
                    "offline signing requires a blockhash (or turn on broadcast to fetch one)",
                )
            })?;
            chain.latest_blockhash().await?
        }
    };

    let passwords = vec![password; shard_paths.len()];
    let signed: horcrux::tx::SignedTx = tokio::task::spawn_blocking(move || {
        let key = horcrux::reconstruct_with_audit(&shard_paths, &passwords, &log, attempt)?;
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
    .map_err(join_err)??;

    let mut result = json!({
        "from": signed.from().to_string(),
        "signature": signed.signature().to_string(),
        "raw_base58": signed.raw_base58(),
        "warnings": warnings,
    });

    if broadcast {
        let chain = chain.expect("broadcast implies chain was connected above");
        match chain.balance(&signed.from()).await {
            Ok(balance) if balance > 0 => {
                match horcrux::chain::broadcast(
                    chain.client(),
                    signed.tx(),
                    Duration::from_secs(1),
                    60,
                )
                .await
                {
                    Ok(sig) => result["mined_signature"] = json!(sig.to_string()),
                    Err(e) => result["broadcast_error"] = json!(e.to_string()),
                }
            }
            Ok(_) => {
                result["broadcast_error"] = json!(format!(
                    "sender {} is unfunded; airdrop lamports first",
                    signed.from()
                ))
            }
            Err(e) => result["broadcast_error"] = json!(e.to_string()),
        }
    }

    Ok(Json(result))
}

#[derive(Deserialize)]
pub struct MpcSignReq {
    shares: Vec<String>,
    group_dir: String,
    password: String,
    to: String,
    lamports: u64,
    blockhash: Option<String>,
    broadcast: bool,
    rpc_url: Option<String>,
    #[serde(default)]
    force: bool,
}

pub async fn mpc_sign(Json(req): Json<MpcSignReq>) -> Result<Json<Value>, ApiError> {
    let MpcSignReq {
        shares,
        group_dir,
        password,
        to,
        lamports,
        blockhash,
        broadcast,
        rpc_url,
        force,
    } = req;

    let share_paths: Vec<PathBuf> = shares.iter().map(PathBuf::from).collect();
    if share_paths.is_empty() {
        return Err(ApiError::message(
            StatusCode::BAD_REQUEST,
            "select at least one key share",
        ));
    }
    let group_dir = PathBuf::from(if group_dir.trim().is_empty() {
        "mpc"
    } else {
        group_dir.trim()
    });
    let group_pub = group_dir.join(horcrux::mpc::GROUP_PUB_FILENAME);
    let to: solana_pubkey::Pubkey = to.trim().parse().map_err(|e| {
        ApiError::message(
            StatusCode::BAD_REQUEST,
            format!("invalid recipient address: {e}"),
        )
    })?;

    let ids = horcrux::mpc::shard_ids(&share_paths)?;
    let (log, attempt, warnings) = preflight(ids, force)?;

    let verifying_key: [u8; 32] = horcrux::mpc::group_verifying_key(&group_pub)?;
    let from = solana_pubkey::Pubkey::from(verifying_key);

    let rpc_url = rpc_url.unwrap_or_else(horcrux::chain::default_rpc_url);
    let chain = if broadcast {
        Some(horcrux::chain::Chain::connect(&rpc_url))
    } else {
        None
    };

    let blockhash: solana_hash::Hash = match blockhash.as_deref().filter(|s| !s.is_empty()) {
        Some(bh) => bh.parse().map_err(|e| {
            ApiError::message(StatusCode::BAD_REQUEST, format!("invalid blockhash: {e}"))
        })?,
        None => {
            let chain = chain.as_ref().ok_or_else(|| {
                ApiError::message(
                    StatusCode::BAD_REQUEST,
                    "offline signing requires a blockhash (or turn on broadcast to fetch one)",
                )
            })?;
            chain.latest_blockhash().await?
        }
    };

    if let Some(chain) = &chain {
        let balance = chain.balance(&from).await?;
        if balance == 0 {
            return Err(ApiError::message(
                StatusCode::BAD_REQUEST,
                format!("sender {from} is unfunded; airdrop lamports first"),
            ));
        }
    }

    let passwords = vec![password; share_paths.len()];
    let params = horcrux::tx::TxParams {
        from,
        to,
        lamports,
        blockhash,
    };
    let signed: horcrux::tx::SignedTx = tokio::task::spawn_blocking(move || {
        let message = horcrux::tx::transaction_message(&params);
        let message_bytes = message.serialize();
        let sig = horcrux::mpc::mpc_sign_with_audit(
            &share_paths,
            &passwords,
            &group_pub,
            &message_bytes,
            &log,
            attempt,
        )?;
        horcrux::tx::sign_transaction_with_signature(params, sig.signature, sig.verifying_key)
    })
    .await
    .map_err(join_err)??;

    let mut result = json!({
        "from": signed.from().to_string(),
        "signature": signed.signature().to_string(),
        "raw_base58": signed.raw_base58(),
        "warnings": warnings,
    });

    if broadcast {
        let chain = chain.expect("broadcast implies chain was connected above");
        match horcrux::chain::broadcast(chain.client(), signed.tx(), Duration::from_secs(1), 60)
            .await
        {
            Ok(sig) => result["mined_signature"] = json!(sig.to_string()),
            Err(e) => result["broadcast_error"] = json!(e.to_string()),
        }
    }

    Ok(Json(result))
}

/// Render a shard/share file on disk as one or more base64-encoded QR PNG
/// frames, for a guardian to scan onto their own device instead of copying
/// the file. Reuses the same `qr::encode_message`/`encode_qr_png_bytes`
/// machinery as the TUI's Ctrl+Q overlay and the CLI's air-gapped `qr-*`
/// subcommands — no new QR logic, just a byte-level, non-file-writing path.
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

/// Decode a single-frame shard/share QR PNG (uploaded as base64 from the
/// browser — a scanned photo or a saved export) and stage its payload to a
/// fresh temp `.hx` file on the server, returning that path so the browser
/// can paste it straight into the existing `shards`/`shares` textarea. The
/// caller is responsible for eventually removing the temp file; `sign`/
/// `mpc_sign` read it like any other shard/share path and don't delete it
/// themselves, matching how a manually-typed path is treated.
#[derive(Deserialize)]
pub struct ShardQrImportReq {
    png_base64: String,
}

pub async fn shard_qr_import(Json(req): Json<ShardQrImportReq>) -> Result<Json<Value>, ApiError> {
    let bytes = BASE64
        .decode(req.png_base64.trim())
        .map_err(|e| ApiError::message(StatusCode::BAD_REQUEST, format!("invalid base64: {e}")))?;

    let path = tokio::task::spawn_blocking(move || -> Result<PathBuf, Error> {
        let frame = horcrux::qr::decode_qr_png_bytes(&bytes)?;
        if frame.message_type != horcrux::qr::MessageType::ShardFile || frame.total != 1 {
            return Err(Error::Qr(
                "QR image is not a single-frame shard/share code".into(),
            ));
        }
        let unique: [u8; 8] = rand::random();
        let tmp =
            std::env::temp_dir().join(format!("horcrux-qr-import-{}.hx", hex::encode(unique)));
        std::fs::write(&tmp, &frame.payload).map_err(Error::Io)?;
        Ok(tmp)
    })
    .await
    .map_err(join_err)??;

    Ok(Json(json!({ "path": path.display().to_string() })))
}
