use chromiumoxide::page::Page;
use rand::SeedableRng;
use rand::rngs::SmallRng;
use std::time::Duration;

use super::mouse::{self, Point, sample_normal};
use super::profile::BehaviorProfile;

/// Replays a behavior profile during browser automation.
pub struct BehaviorReplayer {
    profile: BehaviorProfile,
    rng: SmallRng,
}

impl BehaviorReplayer {
    pub fn new(profile: BehaviorProfile) -> Self {
        Self {
            profile,
            rng: SmallRng::from_entropy(),
        }
    }

    /// Simulate a delay before navigating, as a human would pause before clicking a link.
    pub async fn pre_navigate_delay(&mut self) {
        let delay = sample_normal(
            &mut self.rng,
            self.profile.timing.navigation_delay_mean,
            self.profile.timing.navigation_delay_std,
        )
        .max(50.0) as u64;
        tokio::time::sleep(Duration::from_millis(delay)).await;
    }

    /// Simulate a human reading the page: scroll, pause, move mouse randomly.
    pub async fn simulate_reading(&mut self, page: &Page) {
        let dwell = sample_normal(
            &mut self.rng,
            self.profile.timing.dwell_time_mean,
            self.profile.timing.dwell_time_std,
        )
        .max(200.0) as u64;

        // Initial pause before first action
        let first_action = sample_normal(
            &mut self.rng,
            self.profile.timing.first_action_delay_mean,
            self.profile.timing.first_action_delay_std,
        )
        .max(100.0) as u64;
        tokio::time::sleep(Duration::from_millis(first_action)).await;

        let scroll_time = dwell / 3;
        let remaining = dwell - first_action;

        // Simulate scrolling
        self.simulate_scroll(page, scroll_time).await;

        // Simulate random mouse movements for the remaining time
        if remaining > scroll_time {
            self.simulate_mouse_movement(page, remaining - scroll_time).await;
        }
    }

    /// Simulate scrolling behavior.
    async fn simulate_scroll(&mut self, page: &Page, duration_ms: u64) {
        let mut elapsed = 0u64;

        while elapsed < duration_ms {
            let burst_len = sample_normal(
                &mut self.rng,
                self.profile.scroll.burst_length_mean,
                self.profile.scroll.burst_length_std,
            )
            .max(1.0) as usize;

            for _ in 0..burst_len {
                let delta = sample_normal(
                    &mut self.rng,
                    self.profile.scroll.speed_mean,
                    self.profile.scroll.speed_std,
                );
                let js = format!("window.scrollBy(0, {})", delta as i64);
                let _ = page.evaluate(js).await;
                tokio::time::sleep(Duration::from_millis(16)).await; // ~60fps
                elapsed += 16;
            }

            let pause = sample_normal(
                &mut self.rng,
                self.profile.scroll.pause_mean,
                self.profile.scroll.pause_std,
            )
            .max(50.0) as u64;
            tokio::time::sleep(Duration::from_millis(pause)).await;
            elapsed += pause;
        }
    }

    /// Simulate random mouse movements across the page.
    async fn simulate_mouse_movement(&mut self, page: &Page, duration_ms: u64) {
        let mut current = Point {
            x: 400.0 + self.rng.gen_range(-200.0..200.0f64),
            y: 300.0 + self.rng.gen_range(-150.0..150.0f64),
        };
        let mut elapsed = 0u64;

        while elapsed < duration_ms {
            let target = Point {
                x: self.rng.gen_range(100.0..1800.0f64),
                y: self.rng.gen_range(100.0..900.0f64),
            };

            let path = mouse::generate_path(current, target, &self.profile.mouse, &mut self.rng);
            for (point, delay) in &path {
                let js = format!(
                    "document.dispatchEvent(new MouseEvent('mousemove', {{clientX: {}, clientY: {}}}))",
                    point.x as i32,
                    point.y as i32
                );
                let _ = page.evaluate(js).await;
                tokio::time::sleep(Duration::from_millis(*delay)).await;
                elapsed += delay;
                if elapsed >= duration_ms {
                    break;
                }
            }
            current = target;
        }
    }

    /// Simulate human-like typing into a field.
    pub async fn type_text(&mut self, page: &Page, selector: &str, text: &str) {
        // Focus the element
        let focus_js = format!("document.querySelector('{}')?.focus()", selector);
        let _ = page.evaluate(focus_js).await;

        let initial_pause = sample_normal(
            &mut self.rng,
            self.profile.keyboard.initial_pause_mean,
            self.profile.keyboard.initial_pause_std,
        )
        .max(50.0) as u64;
        tokio::time::sleep(Duration::from_millis(initial_pause)).await;

        for ch in text.chars() {
            // Simulate typo
            if self.rng.gen_bool(self.profile.keyboard.error_rate.clamp(0.0, 0.2)) {
                // Type a wrong key
                let wrong: char = (b'a' + self.rng.gen_range(0..26u8)) as char;
                let _ = page.evaluate(format!(
                    "document.execCommand('insertText', false, '{}')",
                    wrong
                )).await;
                let delay = sample_normal(
                    &mut self.rng,
                    self.profile.keyboard.delay_mean * 0.5,
                    self.profile.keyboard.delay_std,
                )
                .max(20.0) as u64;
                tokio::time::sleep(Duration::from_millis(delay)).await;

                // Backspace to correct
                let _ = page.evaluate(
                    "document.execCommand('delete', false)".to_string()
                ).await;
                tokio::time::sleep(Duration::from_millis(80)).await;
            }

            // Type the correct character
            let _ = page.evaluate(format!(
                "document.execCommand('insertText', false, '{}')",
                ch
            )).await;

            let delay = sample_normal(
                &mut self.rng,
                self.profile.keyboard.delay_mean,
                self.profile.keyboard.delay_std,
            )
            .max(20.0) as u64;
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
    }

    /// Move mouse to element and click it, with human-like behavior.
    pub async fn click_element(&mut self, page: &Page, selector: &str) {
        // Get element position
        let js = format!(
            "(() => {{ const el = document.querySelector('{}'); if (!el) return null; const r = el.getBoundingClientRect(); return JSON.stringify({{x: r.x + r.width/2, y: r.y + r.height/2}}); }})()",
            selector
        );
        let result = page.evaluate(js).await;
        let pos_str = match result {
            Ok(v) => v.into_value::<Option<String>>().unwrap_or(None),
            Err(_) => None,
        };

        if let Some(pos_json) = pos_str {
            if let Ok(pos) = serde_json::from_str::<serde_json::Value>(&pos_json) {
                let target = Point {
                    x: pos["x"].as_f64().unwrap_or(500.0),
                    y: pos["y"].as_f64().unwrap_or(300.0),
                };
                let start = Point {
                    x: target.x + self.rng.gen_range(-300.0..300.0f64),
                    y: target.y + self.rng.gen_range(-200.0..200.0f64),
                };

                // Move mouse along a natural path
                let path = mouse::generate_path(start, target, &self.profile.mouse, &mut self.rng);
                for (point, delay) in &path {
                    let js = format!(
                        "document.elementFromPoint({}, {})?.dispatchEvent(new MouseEvent('mousemove', {{clientX: {}, clientY: {}, bubbles: true}}))",
                        point.x as i32, point.y as i32, point.x as i32, point.y as i32
                    );
                    let _ = page.evaluate(js).await;
                    tokio::time::sleep(Duration::from_millis(*delay)).await;
                }

                // Click delay
                let click_delay = sample_normal(
                    &mut self.rng,
                    self.profile.mouse.click_delay_mean,
                    self.profile.mouse.click_delay_std,
                )
                .max(30.0) as u64;
                tokio::time::sleep(Duration::from_millis(click_delay)).await;

                // Click
                let click_js = format!("document.querySelector('{}')?.click()", selector);
                let _ = page.evaluate(click_js).await;
            }
        }
    }
}

use rand::Rng;
