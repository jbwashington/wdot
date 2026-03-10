use rand::Rng;
use super::profile::MouseProfile;

/// A point on the screen.
#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Generate a human-like mouse path from `start` to `end` using cubic Bezier curves.
/// Returns a series of (point, delay_ms) pairs for dispatching via CDP.
pub fn generate_path(
    start: Point,
    end: Point,
    profile: &MouseProfile,
    rng: &mut impl Rng,
) -> Vec<(Point, u64)> {
    let distance = ((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt();
    if distance < 2.0 {
        return vec![(end, sample_normal(rng, profile.click_delay_mean, profile.click_delay_std) as u64)];
    }

    // Number of interpolation steps proportional to distance
    let steps = ((distance / 10.0) as usize).clamp(5, 100);

    // Generate control points with curvature
    let curvature = sample_normal(rng, profile.curvature_mean, profile.curvature_std).abs();
    let mid_x = (start.x + end.x) / 2.0;
    let mid_y = (start.y + end.y) / 2.0;
    let perpendicular_x = -(end.y - start.y);
    let perpendicular_y = end.x - start.x;
    let perp_len = (perpendicular_x.powi(2) + perpendicular_y.powi(2)).sqrt().max(1.0);

    let cp1 = Point {
        x: mid_x + perpendicular_x / perp_len * distance * curvature * rng.gen_range(-1.0..1.0f64),
        y: mid_y + perpendicular_y / perp_len * distance * curvature * rng.gen_range(-1.0..1.0f64),
    };
    let cp2 = Point {
        x: mid_x + perpendicular_x / perp_len * distance * curvature * rng.gen_range(-0.5..0.5f64),
        y: mid_y + perpendicular_y / perp_len * distance * curvature * rng.gen_range(-0.5..0.5f64),
    };

    // Calculate total time based on speed
    let speed = sample_normal(rng, profile.speed_mean, profile.speed_std).max(0.1);
    let total_time_ms = (distance / speed) as u64;
    let step_delay = (total_time_ms / steps as u64).max(1);

    let mut path = Vec::with_capacity(steps + 2);

    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let point = cubic_bezier(start, cp1, cp2, end, t);

        // Add micro-jitter
        let jitter_x = rng.gen_range(-1.5..1.5f64);
        let jitter_y = rng.gen_range(-1.5..1.5f64);

        path.push((
            Point {
                x: point.x + jitter_x,
                y: point.y + jitter_y,
            },
            step_delay + rng.gen_range(0..3),
        ));
    }

    // Overshoot simulation
    if rng.gen_bool(profile.overshoot_probability.clamp(0.0, 1.0)) {
        let overshoot = profile.overshoot_magnitude_mean;
        let angle = rng.gen_range(0.0..std::f64::consts::TAU);
        let overshoot_point = Point {
            x: end.x + angle.cos() * overshoot,
            y: end.y + angle.sin() * overshoot,
        };
        path.push((overshoot_point, step_delay));
        // Correct back
        path.push((end, (step_delay as f64 * 1.5) as u64));
    }

    path
}

/// Cubic Bezier interpolation.
fn cubic_bezier(p0: Point, p1: Point, p2: Point, p3: Point, t: f64) -> Point {
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    let mt3 = mt2 * mt;
    let t2 = t * t;
    let t3 = t2 * t;

    Point {
        x: mt3 * p0.x + 3.0 * mt2 * t * p1.x + 3.0 * mt * t2 * p2.x + t3 * p3.x,
        y: mt3 * p0.y + 3.0 * mt2 * t * p1.y + 3.0 * mt * t2 * p2.y + t3 * p3.y,
    }
}

/// Sample from a normal distribution (Box-Muller transform).
pub fn sample_normal(rng: &mut impl Rng, mean: f64, std: f64) -> f64 {
    let u1: f64 = rng.gen_range(0.001..1.0);
    let u2: f64 = rng.gen_range(0.0..1.0);
    let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
    mean + z * std
}
