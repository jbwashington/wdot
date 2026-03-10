pub mod adapter;
pub mod scorer;
pub mod signals;

use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

use adapter::{AdaptiveConfig, AdaptiveState};
use scorer::ReputationScore;
use signals::SessionSignals;

/// Lightweight reputation monitor backed by an in-memory ring buffer.
/// Total memory: ~25KB at default window size (200 signals).
pub struct ReputationMonitor {
    signals: Arc<RwLock<VecDeque<SessionSignals>>>,
    window_size: usize,
    config: Arc<RwLock<AdaptiveConfig>>,
    requests_since_rotation: Arc<RwLock<usize>>,
}

impl ReputationMonitor {
    pub fn new(window_size: usize) -> Self {
        Self {
            signals: Arc::new(RwLock::new(VecDeque::with_capacity(window_size))),
            window_size,
            config: Arc::new(RwLock::new(AdaptiveConfig::default())),
            requests_since_rotation: Arc::new(RwLock::new(0)),
        }
    }

    /// Record signals from a completed page load.
    pub async fn record(&self, signal: SessionSignals) {
        let mut signals = self.signals.write().await;
        if signals.len() >= self.window_size {
            signals.pop_front();
        }
        signals.push_back(signal);
        drop(signals);

        let mut count = self.requests_since_rotation.write().await;
        *count += 1;
    }

    /// Get current reputation score.
    pub async fn score(&self) -> ReputationScore {
        let signals = self.signals.read().await;
        let slice: Vec<SessionSignals> = signals.iter().cloned().collect();
        scorer::compute(&slice)
    }

    /// Get current adaptive state (delay, rotation, alert level).
    pub async fn adaptive_state(&self) -> AdaptiveState {
        let score = self.score().await;
        let config = self.config.read().await;
        let count = *self.requests_since_rotation.read().await;
        adapter::adapt(&score, &config, count)
    }

    /// Get the delay to apply before the next request.
    pub async fn current_delay_ms(&self) -> u64 {
        self.adaptive_state().await.current_delay_ms
    }

    /// Check if requests should be paused.
    pub async fn is_paused(&self) -> bool {
        self.adaptive_state().await.paused
    }

    /// Reset rotation counter after fingerprint rotation.
    pub async fn mark_rotation(&self) {
        let mut count = self.requests_since_rotation.write().await;
        *count = 0;
    }

    /// Get recent signal history.
    pub async fn history(&self, limit: usize) -> Vec<SessionSignals> {
        let signals = self.signals.read().await;
        signals.iter().rev().take(limit).cloned().collect()
    }

    /// Reset all tracking data.
    pub async fn reset(&self) {
        let mut signals = self.signals.write().await;
        signals.clear();
        let mut count = self.requests_since_rotation.write().await;
        *count = 0;
    }

    /// Get adaptive config.
    pub async fn get_config(&self) -> AdaptiveConfig {
        self.config.read().await.clone()
    }

    /// Update adaptive config.
    pub async fn update_config(&self, new_config: AdaptiveConfig) {
        let mut config = self.config.write().await;
        *config = new_config;
    }
}
