use serde::{Deserialize, Serialize};

/// Statistical model of a human's browsing behavior.
/// Stored as distributions rather than raw events (~1KB per profile).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorProfile {
    pub name: String,
    pub created_at: u64,
    pub mouse: MouseProfile,
    pub scroll: ScrollProfile,
    pub keyboard: KeyboardProfile,
    pub timing: TimingProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseProfile {
    /// Average speed in px/ms.
    pub speed_mean: f64,
    pub speed_std: f64,
    /// Bezier control point variance for natural curved movement.
    pub curvature_mean: f64,
    pub curvature_std: f64,
    /// Delay (ms) between arriving at target and clicking.
    pub click_delay_mean: f64,
    pub click_delay_std: f64,
    /// Probability of overshooting the target.
    pub overshoot_probability: f64,
    pub overshoot_magnitude_mean: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrollProfile {
    /// Scroll speed in px/event.
    pub speed_mean: f64,
    pub speed_std: f64,
    /// Pause duration between scroll bursts (ms).
    pub pause_mean: f64,
    pub pause_std: f64,
    /// Number of scroll events per burst.
    pub burst_length_mean: f64,
    pub burst_length_std: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardProfile {
    /// Inter-key delay (ms).
    pub delay_mean: f64,
    pub delay_std: f64,
    /// Probability of a typo + correction per character.
    pub error_rate: f64,
    /// Pause before starting to type (ms).
    pub initial_pause_mean: f64,
    pub initial_pause_std: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingProfile {
    /// Time spent reading a page before interacting (ms).
    pub dwell_time_mean: f64,
    pub dwell_time_std: f64,
    /// Delay between page load and first action (ms).
    pub first_action_delay_mean: f64,
    pub first_action_delay_std: f64,
    /// Delay between sequential page navigations (ms).
    pub navigation_delay_mean: f64,
    pub navigation_delay_std: f64,
}

impl BehaviorProfile {
    /// A reasonable default profile modeled on average human behavior.
    pub fn default_human() -> Self {
        Self {
            name: "default".into(),
            created_at: 0,
            mouse: MouseProfile {
                speed_mean: 0.5,
                speed_std: 0.15,
                curvature_mean: 0.3,
                curvature_std: 0.1,
                click_delay_mean: 120.0,
                click_delay_std: 40.0,
                overshoot_probability: 0.12,
                overshoot_magnitude_mean: 8.0,
            },
            scroll: ScrollProfile {
                speed_mean: 80.0,
                speed_std: 30.0,
                pause_mean: 800.0,
                pause_std: 400.0,
                burst_length_mean: 4.0,
                burst_length_std: 2.0,
            },
            keyboard: KeyboardProfile {
                delay_mean: 90.0,
                delay_std: 35.0,
                error_rate: 0.03,
                initial_pause_mean: 300.0,
                initial_pause_std: 100.0,
            },
            timing: TimingProfile {
                dwell_time_mean: 3000.0,
                dwell_time_std: 1500.0,
                first_action_delay_mean: 1500.0,
                first_action_delay_std: 700.0,
                navigation_delay_mean: 2000.0,
                navigation_delay_std: 1000.0,
            },
        }
    }
}
