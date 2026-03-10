use super::signals::SessionSignals;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub enum Trend {
    Improving,
    Stable,
    Degrading,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReputationScore {
    /// Overall reputation: 0.0 (compromised) to 1.0 (clean).
    pub overall: f64,
    /// Captchas per 100 requests in the current window.
    pub captcha_rate: f64,
    /// Blocks per 100 requests in the current window.
    pub block_rate: f64,
    /// Challenge redirects per 100 requests.
    pub challenge_rate: f64,
    /// Score trend based on recent vs older requests.
    pub trend: Trend,
    /// Number of requests in the scoring window.
    pub window_size: usize,
    /// Total requests tracked.
    pub total_requests: usize,
}

/// Compute reputation score from a ring buffer of signals.
pub fn compute(signals: &[SessionSignals]) -> ReputationScore {
    let total = signals.len();
    if total == 0 {
        return ReputationScore {
            overall: 1.0,
            captcha_rate: 0.0,
            block_rate: 0.0,
            challenge_rate: 0.0,
            trend: Trend::Stable,
            window_size: 0,
            total_requests: 0,
        };
    }

    let captchas = signals.iter().filter(|s| s.captcha_encountered).count();
    let blocks = signals.iter().filter(|s| s.blocked).count();
    let challenges = signals.iter().filter(|s| s.redirect_to_challenge).count();

    let captcha_rate = (captchas as f64 / total as f64) * 100.0;
    let block_rate = (blocks as f64 / total as f64) * 100.0;
    let challenge_rate = (challenges as f64 / total as f64) * 100.0;

    let overall =
        (1.0 - (captcha_rate * 0.003 + block_rate * 0.006 + challenge_rate * 0.001)).clamp(0.0, 1.0);

    // Trend: compare recent half vs older half
    let trend = if total >= 10 {
        let mid = total / 2;
        let older = &signals[..mid];
        let recent = &signals[mid..];

        let older_score = half_score(older);
        let recent_score = half_score(recent);

        if recent_score > older_score + 0.05 {
            Trend::Improving
        } else if recent_score < older_score - 0.05 {
            Trend::Degrading
        } else {
            Trend::Stable
        }
    } else {
        Trend::Stable
    };

    ReputationScore {
        overall,
        captcha_rate,
        block_rate,
        challenge_rate,
        trend,
        window_size: total,
        total_requests: total,
    }
}

fn half_score(signals: &[SessionSignals]) -> f64 {
    let total = signals.len().max(1) as f64;
    let captchas = signals.iter().filter(|s| s.captcha_encountered).count() as f64;
    let blocks = signals.iter().filter(|s| s.blocked).count() as f64;
    1.0 - ((captchas / total) * 0.3 + (blocks / total) * 0.6)
}
