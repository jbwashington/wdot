use serde::{Deserialize, Serialize};
use std::time::Instant;

use super::profile::*;

/// A raw interaction event captured during a recording session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BehaviorEvent {
    MouseMove { x: f64, y: f64, timestamp_ms: u64 },
    MouseClick { x: f64, y: f64, timestamp_ms: u64 },
    Scroll { delta_y: f64, timestamp_ms: u64 },
    KeyPress { delay_since_last_ms: u64 },
    PageDwell { duration_ms: u64 },
}

/// Records user behavior and compiles it into a statistical profile.
pub struct BehaviorRecorder {
    events: Vec<BehaviorEvent>,
    start_time: Option<Instant>,
    name: String,
}

impl BehaviorRecorder {
    pub fn new(name: String) -> Self {
        Self {
            events: Vec::new(),
            start_time: None,
            name,
        }
    }

    pub fn start(&mut self) {
        self.start_time = Some(Instant::now());
        self.events.clear();
    }

    pub fn record_event(&mut self, event: BehaviorEvent) {
        self.events.push(event);
    }

    /// Compile raw events into a statistical behavior profile.
    pub fn compile_profile(&self) -> BehaviorProfile {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        BehaviorProfile {
            name: self.name.clone(),
            created_at: now,
            mouse: self.compile_mouse(),
            scroll: self.compile_scroll(),
            keyboard: self.compile_keyboard(),
            timing: self.compile_timing(),
        }
    }

    fn compile_mouse(&self) -> MouseProfile {
        let mut speeds = Vec::new();
        let mut click_delays = Vec::new();
        let mut prev_move: Option<(f64, f64, u64)> = None;

        for event in &self.events {
            match event {
                BehaviorEvent::MouseMove { x, y, timestamp_ms } => {
                    if let Some((px, py, pt)) = prev_move {
                        let dist = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
                        let dt = timestamp_ms.saturating_sub(pt) as f64;
                        if dt > 0.0 {
                            speeds.push(dist / dt);
                        }
                    }
                    prev_move = Some((*x, *y, *timestamp_ms));
                }
                BehaviorEvent::MouseClick { timestamp_ms, .. } => {
                    if let Some((_, _, pt)) = prev_move {
                        click_delays.push(timestamp_ms.saturating_sub(pt) as f64);
                    }
                }
                _ => {}
            }
        }

        let (speed_mean, speed_std) = mean_std(&speeds);
        let (click_delay_mean, click_delay_std) = mean_std(&click_delays);

        MouseProfile {
            speed_mean: if speed_mean > 0.0 { speed_mean } else { 0.5 },
            speed_std: if speed_std > 0.0 { speed_std } else { 0.15 },
            curvature_mean: 0.3,
            curvature_std: 0.1,
            click_delay_mean: if click_delay_mean > 0.0 { click_delay_mean } else { 120.0 },
            click_delay_std: if click_delay_std > 0.0 { click_delay_std } else { 40.0 },
            overshoot_probability: 0.12,
            overshoot_magnitude_mean: 8.0,
        }
    }

    fn compile_scroll(&self) -> ScrollProfile {
        let mut deltas = Vec::new();
        let mut pauses = Vec::new();
        let mut prev_ts: Option<u64> = None;

        for event in &self.events {
            if let BehaviorEvent::Scroll { delta_y, timestamp_ms } = event {
                deltas.push(delta_y.abs());
                if let Some(pt) = prev_ts {
                    let gap = timestamp_ms.saturating_sub(pt);
                    if gap > 100 {
                        pauses.push(gap as f64);
                    }
                }
                prev_ts = Some(*timestamp_ms);
            }
        }

        let (speed_mean, speed_std) = mean_std(&deltas);
        let (pause_mean, pause_std) = mean_std(&pauses);

        ScrollProfile {
            speed_mean: if speed_mean > 0.0 { speed_mean } else { 80.0 },
            speed_std: if speed_std > 0.0 { speed_std } else { 30.0 },
            pause_mean: if pause_mean > 0.0 { pause_mean } else { 800.0 },
            pause_std: if pause_std > 0.0 { pause_std } else { 400.0 },
            burst_length_mean: 4.0,
            burst_length_std: 2.0,
        }
    }

    fn compile_keyboard(&self) -> KeyboardProfile {
        let mut delays = Vec::new();
        for event in &self.events {
            if let BehaviorEvent::KeyPress { delay_since_last_ms } = event {
                delays.push(*delay_since_last_ms as f64);
            }
        }

        let (delay_mean, delay_std) = mean_std(&delays);

        KeyboardProfile {
            delay_mean: if delay_mean > 0.0 { delay_mean } else { 90.0 },
            delay_std: if delay_std > 0.0 { delay_std } else { 35.0 },
            error_rate: 0.03,
            initial_pause_mean: 300.0,
            initial_pause_std: 100.0,
        }
    }

    fn compile_timing(&self) -> TimingProfile {
        let mut dwells = Vec::new();
        for event in &self.events {
            if let BehaviorEvent::PageDwell { duration_ms } = event {
                dwells.push(*duration_ms as f64);
            }
        }

        let (dwell_mean, dwell_std) = mean_std(&dwells);

        TimingProfile {
            dwell_time_mean: if dwell_mean > 0.0 { dwell_mean } else { 3000.0 },
            dwell_time_std: if dwell_std > 0.0 { dwell_std } else { 1500.0 },
            first_action_delay_mean: 1500.0,
            first_action_delay_std: 700.0,
            navigation_delay_mean: 2000.0,
            navigation_delay_std: 1000.0,
        }
    }
}

fn mean_std(values: &[f64]) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0);
    }
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    (mean, variance.sqrt())
}
