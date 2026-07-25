//! Frame-time pacing: advance the simulation by one display refresh interval
//! per presented frame so vsync jitter doesn't judder motion.
//!
//! With vsync (Fifo) the display scans out exactly one frame per refresh, but
//! the CPU-side frame loop returns at wildly uneven times — on real hardware a
//! trivial scene measures `Time::delta` bursting in a 3-frame cycle like
//! 1ms / 15ms / 33ms (swapchain queue jitter) while the screen still updates
//! every 16.7ms. Integrating movement with those raw deltas — or rounding each
//! one to the nearest whole refresh, which reproduces the same 0 / 1 / 2-step
//! cadence — bakes judder into all motion.
//!
//! This module rewrites the default `Time` at the top of each frame to a
//! steady one-interval step (`pace_locked`), carrying a bounded debt so
//! genuine frame drops are still repaid and simulation time tracks real time
//! long-term.

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
    /// Pacing debt: real elapsed time minus simulation elapsed time, in
    /// seconds. Positive means sim has fallen behind real (a frame drop is
    /// being repaid); `pace_locked` keeps it bounded within about ±one
    /// interval so the sim never drifts free of wall clock.
    leftover: f32,
    /// Elapsed time of the rewritten `Time`, for cross-frame continuity.
    smoothed_elapsed: Option<Duration>,
}

impl FramePacing {
    /// Current simulation-vs-wall-clock offset, in ms. Structurally bounded
    /// by roughly one refresh interval.
    pub fn drift_ms(&self) -> f32 {
        self.leftover * 1000.0
    }
}

/// Rolling window length (frames). Two seconds at 60Hz — long enough for a
/// stable estimate, short enough to re-adapt quickly after a refresh change.
const HISTORY: usize = 120;
/// Frames spanning more than this many refresh intervals are genuine hitches:
/// pass them through unsmoothed rather than pacing them one interval at a time.
const HITCH_PERIODS: f32 = 4.0;

/// Frame-locked pacing: advance the simulation by exactly ONE refresh interval
/// per presented frame, tracking a bounded `debt` (real elapsed minus sim
/// elapsed) to self-correct genuine frame drops without ever slow-motioning.
///
/// The premise `snap_to_grid` gets wrong: under Fifo vsync the display scans
/// out exactly one frame per refresh, so the correct per-frame sim step is one
/// interval — *always* — no matter what the CPU-measured `raw` delta says. On
/// real hardware `raw` bursts (e.g. a 1ms / 15ms / 33ms 3-frame cycle from
/// swapchain queue jitter) while the screen updates evenly; rounding each raw
/// delta to the grid turns that CPU noise into a 0 / 1 / 2-interval sim cadence
/// that shows as judder. Locking to the interval emits a dead-flat step stream.
///
/// `debt` keeps sim time honest long-term: it accumulates `raw - used`, and
/// once it exceeds a full interval in either direction (a real drop, or the
/// app briefly outrunning vsync) the emitted step gains or loses one interval
/// to catch up. Returns `(used_delta, new_debt)`.
fn pace_locked(debt: f32, raw: f32, interval: f32) -> (f32, f32) {
    // Genuine hitch (alt-tab, shader compile): pass through and resync so the
    // debt term doesn't have to unwind a huge spike one interval at a time.
    if raw > HITCH_PERIODS * interval {
        return (raw, 0.0);
    }
    let mut used = interval;
    if debt > interval {
        // Sim has fallen a whole frame behind real time — a frame was
        // genuinely dropped; advance an extra interval to catch up.
        used += interval;
    } else if debt < -interval {
        // Sim ran ahead of real time (a short burst frame) — skip this
        // frame's advance so it can't drift permanently early.
        used -= interval;
    }
    (used, debt + raw - used)
}

/// Estimate the display refresh interval, preferring the OS-reported monitor
/// refresh rate and only solving it from frame deltas when the OS gives none.
///
/// The OS-reported rate is authoritative — accurate to well under a
/// microsecond — so when it is present, use it verbatim. Do NOT refine it
/// against the measured frame deltas: on real hardware those deltas burst
/// (a 1 / 15 / 33ms 3-frame cycle at a true 60Hz cadence), the sub-interval
/// 1ms frames get excluded from the fit, and dropping their *time* while the
/// long frames keep theirs biases the fitted interval a few percent LOW.
/// Feeding `pace_locked` a low interval makes the sim run slow, accumulate
/// debt, and inject a catch-up step ~twice a second — a periodic world hitch
/// (measured: est 16.16ms vs true 16.667ms, catch-up every ~27 frames).
///
/// The self-consistent fit below is the fallback for the no-OS-rate case
/// (headless, exotic drivers). Every frame spans a whole number of refresh
/// periods, so the interval is `total_time / total_periods`; two guards:
/// - Frames shorter than half the current estimate can't be a whole period
///   (the unthrottled startup burst) — skip them or they drag the estimate
///   down.
/// - The halved interval is always self-consistent (every frame counts
///   double), so a bad seed would lock in; the result is banded around the
///   median seed to stay above that half-interval attractor.
fn estimate_refresh_interval(history: &[f32], monitor_interval: Option<f32>) -> f32 {
    if let Some(m) = monitor_interval {
        return m.clamp(1.0 / 500.0, 1.0 / 20.0);
    }
    let mut sorted = history.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let seed = sorted[sorted.len() / 2].clamp(1.0 / 500.0, 1.0 / 20.0);
    // A frame-time median lands on whichever jitter cluster is larger — up to
    // ~30% off the true interval either way — so the band is wide, but its
    // floor still sits above the half-interval attractor (~0.5x).
    let (lo, hi) = (seed * 0.7, seed * 1.6);
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

    // Advance the sim by one refresh interval per presented frame (the vsync
    // scanout cadence), letting the bounded debt self-correct genuine drops.
    // See `pace_locked` for why snapping the noisy raw delta was wrong.
    let (used, debt) = pace_locked(pacing.leftover, raw, interval);
    pacing.leftover = debt;

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

    /// When the OS reports a monitor rate it is used verbatim — never refined
    /// against the (bursty, biased-low) frame deltas. Regression: refining it
    /// pulled the interval ~3% low (est 16.16ms at a true 16.667ms), making
    /// pace_locked run slow and hitch ~twice a second catching up.
    #[test]
    fn monitor_anchor_is_used_verbatim() {
        // A pathological frame-time distribution that would drag any fit low.
        let history: Vec<f32> = std::iter::repeat(0.00873).take(120).collect();
        let est = estimate_refresh_interval(&history, Some(HZ60));
        assert!(
            (est - HZ60).abs() < 1e-6,
            "estimated {:.6}, expected exactly {:.6}",
            est,
            HZ60
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

    /// The core fix: under the real-hardware burst pattern (a 1 / 15 / 33ms
    /// 3-frame cycle at a true 60Hz cadence) the emitted sim step must stay a
    /// flat one interval per frame, NOT the 0 / 1 / 2-interval cadence that
    /// rounding the raw delta produces — that cadence is the judder.
    #[test]
    fn locked_pacing_emits_flat_steps_under_burst() {
        // 1 / 16 / 33ms repeating, summing to exactly 3 periods (50ms) — the
        // shape observed on real hardware: 60fps average, brutal per-frame.
        let pattern = [0.001, 0.016, 0.033];
        let mut debt = 0.0;
        let mut cum_raw = 0.0;
        let mut cum_used = 0.0;
        let mut off_grid = 0;
        for i in 0..600 {
            let raw = pattern[i % pattern.len()];
            let (used, next) = pace_locked(debt, raw, HZ60);
            debt = next;
            cum_raw += raw;
            cum_used += used;
            // Skip the initial transient; after it, every step is one interval.
            if i >= 6 && (used - HZ60).abs() > 1e-4 {
                off_grid += 1;
            }
            // Sim time never drifts free of real time.
            assert!(
                (cum_raw - cum_used).abs() <= 2.0 * HZ60,
                "debt {:.4} unbounded after frame {}",
                cum_raw - cum_used,
                i
            );
        }
        // At most a handful of correction steps across 600 frames.
        assert!(off_grid <= 4, "{off_grid} non-flat steps — cadence not locked");
        // And no sustained slow/fast motion.
        assert!(
            (cum_used / 600.0 - cum_raw / 600.0).abs() < 0.0005,
            "sim avg diverged from raw avg — sustained speed drift"
        );
    }

    /// A genuine sustained drop (frames actually 2 intervals long) must be
    /// repaid: the sim advances two intervals to keep pace, not run in slow
    /// motion.
    #[test]
    fn locked_pacing_repays_genuine_drops() {
        let mut debt = 0.0;
        let mut cum_raw = 0.0;
        let mut cum_used = 0.0;
        for _ in 0..300 {
            let raw = 2.0 * HZ60; // steady 30fps
            let (used, next) = pace_locked(debt, raw, HZ60);
            debt = next;
            cum_raw += raw;
            cum_used += used;
        }
        assert!(
            (cum_used - cum_raw).abs() < 3.0 * HZ60,
            "sim {:.3}s vs real {:.3}s — slow motion under sustained drop",
            cum_used,
            cum_raw
        );
    }

    /// Hitches (alt-tab, shader compile) pass through raw and reset the debt.
    #[test]
    fn locked_pacing_passes_hitches_through() {
        let (used, debt) = pace_locked(0.004, 0.5, HZ60);
        assert_eq!(used, 0.5);
        assert_eq!(debt, 0.0);
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
