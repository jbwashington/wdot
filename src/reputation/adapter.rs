use super::scorer::ReputationScore;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveConfig {
    pub min_delay_ms: u64,
    pub max_delay_ms: u64,
    pub fingerprint_rotation_interval: usize,
    pub cooldown_duration_secs: u64,
}

impl Default for AdaptiveConfig {
    fn default() -> Self {
        Self {
            min_delay_ms: 100,
            max_delay_ms: 10_000,
            fingerprint_rotation_interval: 50,
            cooldown_duration_secs: 60,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AdaptiveState {
    pub current_delay_ms: u64,
    pub requests_since_rotation: usize,
    pub should_rotate_fingerprint: bool,
    pub paused: bool,
    pub alert_level: AlertLevel,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum AlertLevel {
    /// Score > 0.8 — all clear.
    Green,
    /// Score 0.6–0.8 — increased delays.
    Yellow,
    /// Score 0.4–0.6 — rotating fingerprints.
    Orange,
    /// Score 0.2–0.4 — aggressive adaptation.
    Red,
    /// Score < 0.2 — paused, cooldown active.
    Critical,
}

/// Compute the adaptive state based on current reputation score.
pub fn adapt(score: &ReputationScore, config: &AdaptiveConfig, requests_since_rotation: usize) -> AdaptiveState {
    let (delay, alert, should_rotate, paused) = match score.overall {
        s if s > 0.8 => (config.min_delay_ms, AlertLevel::Green, false, false),
        s if s > 0.6 => {
            let delay = (config.min_delay_ms as f64 * 1.5) as u64;
            (delay.min(config.max_delay_ms), AlertLevel::Yellow, false, false)
        }
        s if s > 0.4 => {
            let delay = config.min_delay_ms * 2;
            (delay.min(config.max_delay_ms), AlertLevel::Orange, true, false)
        }
        s if s > 0.2 => {
            let delay = config.min_delay_ms * 3;
            (delay.min(config.max_delay_ms), AlertLevel::Red, true, false)
        }
        _ => (config.max_delay_ms, AlertLevel::Critical, true, true),
    };

    let should_rotate = should_rotate
        || requests_since_rotation >= config.fingerprint_rotation_interval;

    AdaptiveState {
        current_delay_ms: delay,
        requests_since_rotation,
        should_rotate_fingerprint: should_rotate,
        paused,
        alert_level: alert,
    }
}
