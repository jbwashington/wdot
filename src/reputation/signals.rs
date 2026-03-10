use serde::{Deserialize, Serialize};

/// Raw signals collected after each page load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSignals {
    pub timestamp: u64,
    pub domain: String,
    pub captcha_encountered: bool,
    pub captcha_type: Option<String>,
    pub http_status: u16,
    pub blocked: bool,
    pub redirect_to_challenge: bool,
    pub response_time_ms: u64,
    pub tls_fingerprint_hash: String,
    pub rate_limited: bool,
}

/// Known block page signatures.
const BLOCK_SIGNATURES: &[&str] = &[
    "access denied",
    "please verify you are human",
    "rate limit exceeded",
    "too many requests",
    "blocked by",
    "captcha required",
    "unusual traffic",
    "automated access",
    "bot detected",
    "enable javascript and cookies",
];

/// Known challenge page signatures.
const CHALLENGE_SIGNATURES: &[&str] = &[
    "challenges.cloudflare.com",
    "just a moment",
    "checking your browser",
    "verify you are human",
    "security check",
    "ray id",
];

/// Analyze page content for block/challenge signals.
pub fn detect_block_signals(html: &str, status: u16) -> (bool, bool, bool) {
    let lower = html.to_lowercase();

    let blocked = status == 403
        || status == 429
        || BLOCK_SIGNATURES.iter().any(|sig| lower.contains(sig));

    let challenge = CHALLENGE_SIGNATURES.iter().any(|sig| lower.contains(sig));

    let rate_limited =
        status == 429 || lower.contains("rate limit") || lower.contains("too many requests");

    (blocked, challenge, rate_limited)
}
