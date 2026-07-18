//! Frame-time smoothing: quantize the simulation delta to the display's
//! refresh interval so vsync pacing jitter doesn't judder motion.
//!
//! With vsync, frames are *displayed* at exact multiples of the refresh
//! interval, but the CPU-side frame loop can start early or late, so the
//! measured `Time::delta` oscillates (e.g. 5ms / 33ms around a 60Hz cadence).
//! Integrating movement with those uneven deltas while the display shows
//! frames at even intervals bakes judder into all motion. This module
//! rewrites the default `Time` at the top of each frame: when the raw delta
//! is close to a whole number of refresh intervals it snaps to exactly that,
//! and the residual error is paid back gradually so simulation time still
//! tracks real time long-term.

use bevy::prelude::*;
use bevy::time::TimeSystem;
use bevy::window::{Monitor, PrimaryMonitor};
use std::time::Duration;

pub struct FramePacingPlugin;

impl Plugin for FramePacingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FramePacing>()
            .add_systems(First, smooth_frame_time.after(TimeSystem));
    }
}

/// Rolling pacing state, also read by the F3 overlay to show raw-vs-used
/// frame time statistics.
#[derive(Resource, Default)]
pub struct FramePacing {
    /// Raw `Time<Real>` deltas (seconds), most recent last.
    pub raw_history: Vec<f32>,
    /// Deltas actually fed to the simulation, most recent last.
    pub used_history: Vec<f32>,
    /// Estimated display refresh interval (seconds); 0 until warmed up.
    pub interval: f32,
    /// How far the measured timeline sits ahead (+) of the snapped
    /// simulation timeline, in seconds. Bounded by ±half an interval.
    leftover: f32,
    /// Elapsed time of the rewritten `Time`, for cross-frame continuity.
    smoothed_elapsed: Option<Duration>,
}

impl FramePacing {
    /// Current simulation-vs-wall-clock offset, in ms. Structurally bounded
    /// by roughly half a refresh interval.
    pub fn drift_ms(&self) -> f32 {
        self.leftover * 1000.0
    }
}

/// Rolling window length (frames). Two seconds at 60Hz — long enough for a
/// stable estimate, short enough to re-adapt quickly after a refresh change.
const HISTORY: usize = 120;
/// Frames spanning more than this many refresh intervals are genuine hitches:
/// pass them through unsmoothed instead of snapping.
const HITCH_PERIODS: f32 = 4.0;

/// Snap one frame of the cumulative timeline to the vsync grid.
///
/// Rounding each delta independently misclassifies extreme-but-single frames
/// (a 25ms measurement at a true 60Hz cadence rounds to two periods), and
/// those errors add up into sustained slow motion. Carrying the `leftover`
/// between the measured and snapped timelines instead means a mistimed
/// measurement is paid back by the very next frames, and the simulation
/// clock can never drift more than about half an interval from wall clock.
/// Returns `(used_delta, new_leftover)`.
fn snap_to_grid(leftover: f32, raw: f32, interval: f32) -> (f32, f32) {
    let acc = leftover + raw;
    let k = (acc / interval).round().max(0.0);
    if k > HITCH_PERIODS {
        // Genuine hitch: pass it through and resync the grid phase.
        (raw, 0.0)
    } else {
        let used = k * interval;
        (used, (acc - used).clamp(-interval, interval))
    }
}

/// Estimate the display refresh interval from a window of raw frame deltas,
/// optionally anchored to the OS-reported monitor refresh rate.
///
/// A median or plain mean both fail under real pacing jitter (e.g. many
/// ~14ms frames and fewer ~20-35ms ones at a true 16.7ms cadence): the
/// median lands on the short cluster and the mean is skewed by dropped
/// frames. Instead solve for the self-consistent interval: every frame
/// spans a whole number of refresh periods, so the interval is
/// `total_time / total_periods` where each frame's period count is its
/// delta rounded to multiples of the current estimate.
///
/// Two degeneracy guards, both learned the hard way:
/// - Frames shorter than half the current estimate can't be a whole period
///   (they're the unthrottled startup burst) — skip them, or they drag the
///   estimate down.
/// - The halved interval is *always* self-consistent (every frame just
///   counts double), so a bad seed locks in permanently. The result is
///   therefore constrained to a band around the seed, and the seed comes
///   from the monitor's reported rate when the OS provides it.
fn estimate_refresh_interval(history: &[f32], monitor_interval: Option<f32>) -> f32 {
    let seed = monitor_interval.unwrap_or_else(|| {
        let mut sorted = history.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        sorted[sorted.len() / 2]
    });
    let seed = seed.clamp(1.0 / 500.0, 1.0 / 20.0);
    // The monitor rate is trustworthy (tight band, refinement only tracks
    // clock skew). A frame-time median lands on whichever jitter cluster is
    // larger — up to ~30% off the true interval in either direction — so the
    // unanchored band is wide, but its floor still sits above the
    // half-interval attractor (~0.5x) that a bad seed would lock into.
    let (lo, hi) = if monitor_interval.is_some() {
        (seed * 0.9, seed * 1.1)
    } else {
        (seed * 0.7, seed * 1.6)
    };
    let mut est = seed;
    for _ in 0..3 {
        let mut total_time = 0.0;
        let mut total_periods = 0.0;
        for &dt in history {
            // Sub-half-period frames (startup burst) can't span a refresh.
            if dt < 0.5 * est {
                continue;
            }
            let k = (dt / est).round().max(1.0);
            // Hitches (alt-tab, shader compiles…) span an unknowable number
            // of periods — keep them out of the estimate entirely.
            if k <= HITCH_PERIODS {
                total_time += dt;
                total_periods += k;
            }
        }
        if total_periods > 0.0 {
            est = (total_time / total_periods).clamp(lo, hi);
        }
    }
    est.clamp(1.0 / 500.0, 1.0 / 20.0)
}

fn push(history: &mut Vec<f32>, value: f32) {
    history.push(value);
    if history.len() > HISTORY {
        let excess = history.len() - HISTORY;
        history.drain(..excess);
    }
}

fn smooth_frame_time(
    real: Res<Time<Real>>,
    virt: Res<Time<Virtual>>,
    monitors: Query<(&Monitor, Option<&PrimaryMonitor>)>,
    mut time: ResMut<Time>,
    mut pacing: ResMut<FramePacing>,
) {
    let raw = real.delta_secs();
    if raw <= 0.0 {
        return;
    }
    push(&mut pacing.raw_history, raw);

    // Don't fight deliberate time scaling (e.g. the test bot's speedup) or a
    // paused virtual clock — leave the stock `Time` untouched there.
    if (virt.relative_speed() - 1.0).abs() > 1e-3 || virt.is_paused() {
        let used = time.delta_secs();
        push(&mut pacing.used_history, used);
        return;
    }

    let n = pacing.raw_history.len();
    if n < 20 {
        let used = time.delta_secs();
        push(&mut pacing.used_history, used);
        return;
    }
    // Prefer the primary monitor's OS-reported refresh rate as the anchor.
    let monitor_interval = monitors
        .iter()
        .filter_map(|(m, primary)| {
            m.refresh_rate_millihertz
                .map(|mhz| (primary.is_some(), 1000.0 / mhz as f32))
        })
        .max_by_key(|&(is_primary, _)| is_primary)
        .map(|(_, interval)| interval);
    let interval = estimate_refresh_interval(&pacing.raw_history, monitor_interval);
    pacing.interval = interval;

    // Snap the cumulative timeline to the vsync grid: frames are *shown* at
    // whole refresh intervals, so integrating with exactly those steps is
    // what makes displayed motion even. Only genuine hitches pass through.
    let (used, leftover) = snap_to_grid(pacing.leftover, raw, interval);
    pacing.leftover = leftover;

    // Respect the virtual clock's hitch clamp (default 250ms max step).
    let used = used.min(virt.max_delta().as_secs_f32());
    push(&mut pacing.used_history, used);

    // Rewrite the default `Time` the game's systems read, keeping elapsed
    // time continuous across frames.
    let prev = *pacing.smoothed_elapsed.get_or_insert_with(|| {
        time.elapsed().saturating_sub(time.delta())
    });
    let mut smoothed = Time::default();
    smoothed.advance_to(prev);
    smoothed.advance_by(Duration::from_secs_f32(used.max(0.0)));
    pacing.smoothed_elapsed = Some(smoothed.elapsed());
    *time = smoothed;
}

#[cfg(test)]
mod tests {
    use super::*;

    const HZ60: f32 = 1.0 / 60.0;

    /// The pattern observed on the user's machine: raw deltas oscillating
    /// short/long (median ~14.2ms) while presenting at a true 60Hz cadence
    /// (avg 16.66ms). A median estimator returns 14.2 here; the
    /// self-consistent estimator must return the real interval.
    #[test]
    fn jittered_60hz_estimates_true_interval() {
        let mut history = Vec::new();
        for i in 0..120 {
            history.push(if i % 2 == 0 { 0.01424 } else { 0.01909 });
        }
        let est = estimate_refresh_interval(&history, None);
        assert!(
            (est - HZ60).abs() < 0.0005,
            "estimated {:.5}, expected ~{:.5}",
            est,
            HZ60
        );
    }

    /// Regression: an unthrottled startup burst (frames far shorter than a
    /// refresh period) must not seed the estimator into the half-interval
    /// attractor (observed in the field as `refresh est 8.73ms` at 60Hz,
    /// with the game then running ~17% slow).
    #[test]
    fn startup_burst_does_not_halve_the_estimate() {
        let mut history: Vec<f32> = std::iter::repeat(0.006).take(20).collect();
        for i in 0..100 {
            history.push(if i % 2 == 0 { 0.01424 } else { 0.01909 });
        }
        let est = estimate_refresh_interval(&history, None);
        assert!(
            (est - HZ60).abs() < 0.001,
            "estimated {:.5}, expected ~{:.5}",
            est,
            HZ60
        );
    }

    /// With the OS-reported monitor rate as anchor, the estimate stays in a
    /// tight band around it no matter how odd the frame-time distribution is.
    #[test]
    fn monitor_anchor_pins_the_estimate() {
        let history: Vec<f32> = std::iter::repeat(0.00873).take(120).collect();
        let est = estimate_refresh_interval(&history, Some(HZ60));
        assert!(
            est >= HZ60 * 0.9 && est <= HZ60 * 1.1,
            "estimated {:.5} outside the anchored band",
            est
        );
    }

    #[test]
    fn dropped_frames_count_as_two_periods() {
        let mut history = vec![HZ60; 110];
        history.extend(std::iter::repeat(2.0 * HZ60).take(10));
        let est = estimate_refresh_interval(&history, None);
        assert!((est - HZ60).abs() < 0.0005, "estimated {:.5}", est);
    }

    #[test]
    fn hitches_are_excluded_from_the_estimate() {
        let mut history = vec![HZ60; 116];
        history.extend([0.2, 0.5, 1.0, 2.0]); // alt-tab class outliers
        let est = estimate_refresh_interval(&history, None);
        assert!((est - HZ60).abs() < 0.0005, "estimated {:.5}", est);
    }

    /// Regression: independently rounding each delta classified extreme
    /// single frames (24-35ms measurements at a true 60Hz cadence) as
    /// doubles, accumulating into ~16% sustained slow motion (observed as
    /// sim avg 19.9ms vs raw 16.7ms). Grid snapping must keep the snapped
    /// timeline within half an interval of the measured one, always.
    #[test]
    fn grid_snapping_bounds_drift_under_extreme_jitter() {
        // 60Hz cadence with severe measurement jitter: repeating pattern of
        // frames summing to 4 true periods (26 + 8 + 19 + 13.6 ≈ 66.6ms).
        let pattern = [0.026, 0.008, 0.019, 0.0136];
        let mut leftover = 0.0;
        let mut cum_raw = 0.0;
        let mut cum_used = 0.0;
        for i in 0..400 {
            let raw = pattern[i % pattern.len()];
            let (used, next) = snap_to_grid(leftover, raw, HZ60);
            leftover = next;
            cum_raw += raw;
            cum_used += used;
            // Every used delta is a whole number of periods.
            let k = used / HZ60;
            assert!((k - k.round()).abs() < 1e-4, "non-grid delta {used}");
            // The snapped timeline never departs the measured one by more
            // than an interval.
            assert!(
                (cum_raw - cum_used).abs() <= HZ60 + 1e-4,
                "drift {:.4} after frame {}",
                cum_raw - cum_used,
                i
            );
        }
        let avg_used = cum_used / 400.0;
        let avg_raw = cum_raw / 400.0;
        assert!(
            (avg_used - avg_raw).abs() < 0.0005,
            "sim avg {:.5} diverged from raw avg {:.5} — sustained slow motion",
            avg_used,
            avg_raw
        );
    }

    /// Hitches pass through raw and resync the grid phase.
    #[test]
    fn grid_snapping_passes_hitches_through() {
        let (used, leftover) = snap_to_grid(0.004, 0.5, HZ60);
        assert_eq!(used, 0.5);
        assert_eq!(leftover, 0.0);
    }

    /// A fast loop iteration between two grid lines yields a zero-length
    /// simulation step rather than inventing time.
    #[test]
    fn grid_snapping_can_emit_zero_steps() {
        // Previous frame overshot (leftover well negative), next raw frame
        // is short: no grid line was crossed.
        let (used, leftover) = snap_to_grid(-0.008, 0.010, HZ60);
        assert_eq!(used, 0.0);
        assert!((leftover - 0.002).abs() < 1e-6);
    }

    #[test]
    fn high_refresh_rates_estimate_correctly() {
        let hz144 = 1.0 / 144.0;
        let mut history = Vec::new();
        for i in 0..120 {
            // ±30% jitter around the 144Hz interval.
            history.push(if i % 2 == 0 { hz144 * 0.72 } else { hz144 * 1.28 });
        }
        let est = estimate_refresh_interval(&history, None);
        assert!((est - hz144).abs() < 0.0005, "estimated {:.5}", est);
    }
}
