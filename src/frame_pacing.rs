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
    /// Raw-minus-used drift still to be paid back (seconds).
    accumulated_error: f32,
    /// Elapsed time of the rewritten `Time`, for cross-frame continuity.
    smoothed_elapsed: Option<Duration>,
}

impl FramePacing {
    /// Current simulation-vs-wall-clock drift awaiting repayment, in ms.
    pub fn drift_ms(&self) -> f32 {
        self.accumulated_error * 1000.0
    }
}

/// Rolling window length (frames). Two seconds at 60Hz — long enough for a
/// stable estimate, short enough to re-adapt quickly after a refresh change.
const HISTORY: usize = 120;
/// Frames spanning more than this many refresh intervals are genuine hitches:
/// pass them through unsmoothed instead of snapping.
const HITCH_PERIODS: f32 = 4.0;
/// Cap on drift repayment per frame, as a fraction of the interval. Keeps
/// the correction itself invisible.
const ERROR_CORRECTION: f32 = 0.10;

/// Estimate the display refresh interval from a window of raw frame deltas.
///
/// A median or plain mean both fail under real pacing jitter (e.g. many
/// ~14ms frames and fewer ~20-35ms ones at a true 16.7ms cadence): the
/// median lands on the short cluster and the mean is skewed by dropped
/// frames. Instead solve for the self-consistent interval: every frame
/// spans a whole number of refresh periods, so the interval is
/// `total_time / total_periods` where each frame's period count is its
/// delta rounded to multiples of the current estimate. A few fixed-point
/// iterations from the mean converge for any jitter distribution.
fn estimate_refresh_interval(history: &[f32]) -> f32 {
    let mut est = history.iter().sum::<f32>() / history.len() as f32;
    for _ in 0..3 {
        est = est.clamp(1.0 / 500.0, 1.0 / 20.0);
        let mut total_time = 0.0;
        let mut total_periods = 0.0;
        for &dt in history {
            let k = (dt / est).round().max(1.0);
            // Hitches (alt-tab, shader compiles…) span an unknowable number
            // of periods — keep them out of the estimate entirely.
            if k <= HITCH_PERIODS {
                total_time += dt;
                total_periods += k;
            }
        }
        if total_periods > 0.0 {
            est = total_time / total_periods;
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
    let interval = estimate_refresh_interval(&pacing.raw_history);
    pacing.interval = interval;

    // Snap every non-hitch frame to a whole number of refresh intervals —
    // that is when the frame was actually shown, so integrating with exactly
    // that step is what makes displayed motion even. Only genuine hitches
    // pass through raw.
    let k = (raw / interval).round().max(1.0);
    let mut used = if k <= HITCH_PERIODS { k * interval } else { raw };

    // Pay back the raw-vs-used drift a little at a time so long-run
    // simulation time matches wall-clock time.
    pacing.accumulated_error = (pacing.accumulated_error + raw - used).clamp(-0.25, 0.25);
    let correction = pacing
        .accumulated_error
        .clamp(-ERROR_CORRECTION * interval, ERROR_CORRECTION * interval);
    used += correction;
    pacing.accumulated_error -= correction;

    // Respect the virtual clock's hitch clamp (default 250ms max step).
    used = used.min(virt.max_delta().as_secs_f32());
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
        let est = estimate_refresh_interval(&history);
        assert!(
            (est - HZ60).abs() < 0.0005,
            "estimated {:.5}, expected ~{:.5}",
            est,
            HZ60
        );
    }

    #[test]
    fn dropped_frames_count_as_two_periods() {
        let mut history = vec![HZ60; 110];
        history.extend(std::iter::repeat(2.0 * HZ60).take(10));
        let est = estimate_refresh_interval(&history);
        assert!((est - HZ60).abs() < 0.0005, "estimated {:.5}", est);
    }

    #[test]
    fn hitches_are_excluded_from_the_estimate() {
        let mut history = vec![HZ60; 116];
        history.extend([0.2, 0.5, 1.0, 2.0]); // alt-tab class outliers
        let est = estimate_refresh_interval(&history);
        assert!((est - HZ60).abs() < 0.0005, "estimated {:.5}", est);
    }

    #[test]
    fn high_refresh_rates_estimate_correctly() {
        let hz144 = 1.0 / 144.0;
        let mut history = Vec::new();
        for i in 0..120 {
            // ±30% jitter around the 144Hz interval.
            history.push(if i % 2 == 0 { hz144 * 0.72 } else { hz144 * 1.28 });
        }
        let est = estimate_refresh_interval(&history);
        assert!((est - hz144).abs() < 0.0005, "estimated {:.5}", est);
    }
}
