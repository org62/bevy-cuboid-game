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

/// Rolling window length (frames). Two seconds at 60Hz — long enough for a
/// stable median, short enough to re-adapt quickly after a refresh change.
const HISTORY: usize = 120;
/// Snap when the raw delta is within this fraction of the interval of a
/// whole multiple. Beyond it the frame is a genuine hitch — pass it through.
const SNAP_TOLERANCE: f32 = 0.30;
/// Cap on drift repayment per frame, as a fraction of the interval. Keeps
/// the correction itself invisible.
const ERROR_CORRECTION: f32 = 0.10;

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

    // Estimate the display interval as the median recent raw frame time.
    // The median is robust against both hitches and the short frames that
    // pair with them.
    let n = pacing.raw_history.len();
    if n < 20 {
        let used = time.delta_secs();
        push(&mut pacing.used_history, used);
        return;
    }
    let mut sorted = pacing.raw_history.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let interval = sorted[n / 2].clamp(1.0 / 500.0, 1.0 / 20.0);
    pacing.interval = interval;

    // Snap to the nearest whole number of intervals when close enough.
    let k = (raw / interval).round().max(1.0);
    let snapped = k * interval;
    let mut used = if (raw - snapped).abs() <= SNAP_TOLERANCE * interval {
        snapped
    } else {
        raw
    };

    // Pay back the raw-vs-used drift a little at a time so long-run
    // simulation time matches wall-clock time.
    pacing.accumulated_error += raw - used;
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
