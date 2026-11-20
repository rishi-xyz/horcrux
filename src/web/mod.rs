//! A local, loopback-only HTTP console covering Horcrux flows (access log,
//! verify, split/reconstruct/sign for Mode A, MPC split/sign for Mode B,
//! QR air-gap, wallets/on-chain views, and security/AI events) for guardians
//! who would rather use a browser than a terminal. The UI is a statically
//! exported Next.js app (`web/`, output in `web/out`) embedded into this
//! binary at compile time via `include_dir!`, so the shipped artifact remains
//! a single self-contained executable — no Node process, no separately
//! deployed static host.
//!
//! Security model, deliberately narrower than a general-purpose web app:
//!
//! - By default the listener binds `127.0.0.1` (offline-first local posture).
//!   `HORCRUX_WEB_HOST` can override the bind address for containerized local
//!   use (e.g. Docker port publishing); keep access limited to a trusted local
//!   environment and do not expose it publicly.
//! - Every `/api/*` request must carry a random per-run bearer token (printed
//!   once at startup and pre-filled into the page's URL) in the
//!   `X-Horcrux-Token` header. A custom header forces the browser to run a
//!   CORS preflight, which this server does not answer for other origins, so
//!   a malicious page cannot forge these requests even by guessing the port.
//! - The static SPA bundle (`/`, `/assets/*`) carries no secrets and is
//!   served without the token so the page itself can load before it knows
//!   the token from its own URL (`?token=...`, read client-side).
//! - Exactly like the CLI and TUI, an audit `Block` verdict refuses the
//!   operation unless the request explicitly sets `force`; a `Warn` verdict
//!   proceeds but the warnings are returned alongside the result.
//! - No response ever includes a reconstructed private key or seed, with the
//!   same one exception as the CLI/TUI: a freshly **generated** disposable
//!   test key on Init/MPC-split, returned once and never persisted.

mod handlers;

use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use include_dir::{Dir, include_dir};
use rand::RngCore;
use std::sync::Arc;

pub struct AppState {
    pub token: String,
}

/// The Next.js static export, embedded at compile time. Run `npm install &&
/// npm run build` inside `web/` before `cargo build`/`cargo run` — this
/// directory must exist and be populated first, same as any other compile-time
/// asset embed.
static DIST: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/web/out");

/// Serve a file from the embedded static export by request path, with
/// sensible fallbacks for Next.js `trailingSlash` routes (`/x/` -> `/x/index.html`)
/// and SPA-style deep links (fall back to `index.html`).
async fn static_asset(uri: Uri) -> Response {
    let raw = uri.path().trim_start_matches('/');
    let candidates: Vec<String> = if raw.is_empty() {
        vec!["index.html".to_string()]
    } else if raw.ends_with('/') {
        vec![
            format!("{raw}index.html"),
            format!("{}index.html", raw.trim_end_matches('/')),
            raw.trim_end_matches('/').to_string(),
        ]
    } else {
        vec![
            raw.to_string(),
            format!("{raw}.html"),
            format!("{raw}/index.html"),
        ]
    };

    let file = candidates
        .iter()
        .find_map(|p| DIST.get_file(p))
        .or_else(|| DIST.get_file("index.html"));

    match file {
        Some(file) => {
            let mime = mime_guess::from_path(file.path())
                .first_or_octet_stream()
                .to_string();
            ([(header::CONTENT_TYPE, mime)], Body::from(file.contents())).into_response()
        }
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

/// Generate a random 32-byte token, hex-encoded.
fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Constant-time string comparison (the token is short-lived and local-only,
/// but there is no reason not to avoid a timing oracle for free).
fn tokens_match(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

async fn require_token(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let supplied = headers
        .get("x-horcrux-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if tokens_match(supplied, &state.token) {
        next.run(request).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            "missing or invalid X-Horcrux-Token",
        )
            .into_response()
    }
}

/// Start the local web UI and block until the process is interrupted.
pub async fn run(port: u16) -> anyhow::Result<()> {
    let token = generate_token();
    let state = Arc::new(AppState {
        token: token.clone(),
    });

    let api = Router::new()
        .route("/status", get(handlers::status))
        .route("/log", get(handlers::log))
        .route("/verify", post(handlers::verify))
        .route("/init", post(handlers::init))
        .route("/reconstruct", post(handlers::reconstruct))
        .route("/sign", post(handlers::sign))
        .route("/mpc-split", post(handlers::mpc_split))
        .route("/mpc-sign", post(handlers::mpc_sign))
        .route("/shard-qr", post(handlers::shard_qr))
        .route("/shard-qr-import", post(handlers::shard_qr_import))
        .route("/drives", get(handlers::drives))
        .route("/qr/request", post(handlers::qr_request))
        .route("/qr/commit", post(handlers::qr_commit))
        .route("/qr/package", post(handlers::qr_package))
        .route("/qr/share", post(handlers::qr_share))
        .route("/qr/finalize", post(handlers::qr_finalize))
        .route("/ai/status", get(handlers::ai_status))
        .route("/ai/configure", post(handlers::ai_configure))
        .route("/ai/check", post(handlers::ai_check))
        .route("/ai/events", get(handlers::ai_events))
        .route(
            "/wallets",
            get(handlers::wallets_list).post(handlers::wallets_create),
        )
        .route("/wallets/import", post(handlers::wallets_import))
        .route("/wallets/connect", post(handlers::wallets_connect))
        .route("/wallets/watch", post(handlers::wallets_watch))
        .route(
            "/wallets/{id}",
            axum::routing::delete(handlers::wallets_delete),
        )
        .route("/onchain/balance", get(handlers::onchain_balance))
        .route("/onchain/transactions", get(handlers::onchain_transactions))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_token))
        .with_state(state.clone());

    let app = Router::new().nest("/api", api).fallback(static_asset);

    // Default remains loopback-only for local security. For containerized use
    // (e.g. docker port publishing), HORCRUX_WEB_HOST can be set to 0.0.0.0.
    let host = std::env::var("HORCRUX_WEB_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let host_ip: std::net::IpAddr = host
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid HORCRUX_WEB_HOST {host:?}: {e}"))?;
    let addr = std::net::SocketAddr::from((host_ip, port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let bound = listener.local_addr()?;
    let display_host = match host_ip {
        std::net::IpAddr::V4(ip) if ip.is_unspecified() => "127.0.0.1".to_string(),
        std::net::IpAddr::V6(ip) if ip.is_unspecified() => "127.0.0.1".to_string(),
        _ => bound.ip().to_string(),
    };

    println!("HORCRUX web UI (bound to {bound}; token-gated /api, intended for local use)");
    println!();
    println!("  http://{display_host}:{}/?token={token}", bound.port());
    println!();
    println!(
        "Open that URL in a browser. The token above is required for every action\n\
         (/api/* returns 401 without X-Horcrux-Token). Keep the URL/token local and\n\
         do not expose this service beyond a trusted local environment. Press Ctrl+C to stop."
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    println!("\nShutting down the web UI.");
}
