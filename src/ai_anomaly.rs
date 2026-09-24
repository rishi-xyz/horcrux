//! AI-assisted anomaly detection over the access log (advisory only).
//!
//! This sends a plain-text summary of recent [`audit::Entry`] history to a
//! free model on OpenRouter and asks it whether anything looks unusual. It is
//! a second, fuzzier opinion alongside `audit::Scorer`'s rule-based checks —
//! it never blocks a signing attempt, only reports a warning. A network
//! failure, a missing API key, or an unparseable model reply are all
//! non-fatal: the caller decides what to do (the CLI just prints why the
//! check could not run).

use crate::audit::{Entry, EntryKind, format_utc};
use crate::error::Error;
use serde::Deserialize;
use serde_json::json;

/// A capable free-tier model on OpenRouter. Overridable via `--model` or the
/// `HORCRUX_AI_MODEL` environment variable, since free model availability
/// changes over time.
pub const DEFAULT_MODEL: &str = "meta-llama/llama-3.3-70b-instruct:free";

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

const SYSTEM_PROMPT: &str = "You are a terse security log auditor for an offline crypto key-signing tool. \
You will be given a chronological list of access-log entries (decrypt_ok, decrypt_fail, blocked, signed), \
one per line, each with a timestamp, an attempt id (entries sharing one id belong to the same signing \
attempt), and a shard id. Look for patterns a human analyst would find suspicious: bursts of failures, \
repeated blocked attempts, activity at odd hours, unusually rapid or repeated attempts, or anything else out \
of place. If the log looks like normal, routine use, say so. Respond with ONLY a single JSON object and \
nothing else, in exactly this shape: {\"anomaly\": true|false, \"summary\": \"one short sentence\"}.";

/// The AI scorer's verdict on a slice of access-log history.
#[derive(Debug, Clone)]
pub struct AiVerdict {
    /// Whether the model flagged the history as unusual.
    pub anomaly: bool,
    /// The model's one-sentence explanation.
    pub summary: String,
    /// The model id that produced this verdict.
    pub model: String,
}

#[derive(Deserialize)]
struct ModelJson {
    anomaly: bool,
    summary: String,
}

#[derive(Deserialize)]
struct OpenRouterResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    content: String,
}

/// Format access-log entries as a compact, human/model-readable log.
pub fn format_entries(entries: &[Entry]) -> String {
    entries
        .iter()
        .map(|e| {
            let kind = match e.kind {
                EntryKind::DecryptOk => "decrypt_ok",
                EntryKind::DecryptFail => "decrypt_fail",
                EntryKind::Blocked => "blocked",
                EntryKind::Signed => "signed",
            };
            format!(
                "{}  attempt={}  shard={}  {kind}",
                format_utc(e.ts),
                e.attempt,
                e.shard_id
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Ask a free OpenRouter model whether `entries` look anomalous.
///
/// `api_key` is the OpenRouter API key (get one at https://openrouter.ai/keys
/// and pick a `:free`-suffixed model). `model` overrides [`DEFAULT_MODEL`].
///
/// Errors are returned as [`Error::Ai`] and should be treated as advisory —
/// never as a reason to block signing.
pub async fn check(
    entries: &[Entry],
    api_key: &str,
    model: Option<&str>,
) -> Result<AiVerdict, Error> {
    let model = model.unwrap_or(DEFAULT_MODEL).to_string();

    if entries.is_empty() {
        return Ok(AiVerdict {
            anomaly: false,
            summary: "no log history yet".to_string(),
            model,
        });
    }

    let body = json!({
        "model": model,
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": format_entries(entries)},
        ],
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(OPENROUTER_URL)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::Ai(format!("request to OpenRouter failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(Error::Ai(format!("OpenRouter returned {status}: {text}")));
    }

    let parsed: OpenRouterResponse = resp
        .json()
        .await
        .map_err(|e| Error::Ai(format!("failed to parse OpenRouter response: {e}")))?;

    let content = parsed
        .choices
        .first()
        .map(|c| c.message.content.trim())
        .ok_or_else(|| Error::Ai("OpenRouter returned no choices".to_string()))?;

    let json_str = extract_json_object(content)
        .ok_or_else(|| Error::Ai(format!("model reply had no JSON object: {content}")))?;

    let verdict: ModelJson = serde_json::from_str(json_str)
        .map_err(|e| Error::Ai(format!("could not parse model JSON ({e}): {json_str}")))?;

    Ok(AiVerdict {
        anomaly: verdict.anomaly,
        summary: verdict.summary,
        model,
    })
}

/// Extract the first top-level `{...}` object from `s`, tolerating stray
/// text or markdown fences around it — models don't always follow
/// instructions to emit bare JSON exactly.
fn extract_json_object(s: &str) -> Option<&str> {
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    (end >= start).then(|| &s[start..=end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::Entry;

    #[test]
    fn extracts_bare_json() {
        let s = r#"{"anomaly": true, "summary": "burst of failures"}"#;
        assert_eq!(extract_json_object(s), Some(s));
    }

    #[test]
    fn extracts_json_wrapped_in_fences() {
        let s = "```json\n{\"anomaly\": false, \"summary\": \"looks routine\"}\n```";
        assert_eq!(
            extract_json_object(s),
            Some(r#"{"anomaly": false, "summary": "looks routine"}"#)
        );
    }

    #[test]
    fn no_json_object_returns_none() {
        assert_eq!(extract_json_object("no json here"), None);
    }

    #[test]
    fn formats_entries_readably() {
        let entries = vec![
            Entry::ok(1_767_225_600_000, 9, 2),
            Entry::fail(1_767_225_600_000, 9, 3),
        ];
        let out = format_entries(&entries);
        assert!(out.contains("decrypt_ok"));
        assert!(out.contains("decrypt_fail"));
        assert!(out.contains("attempt=9"));
    }
}
