//! Central normalization of raw mouse motion.
//!
//! Bevy's `MouseMotion` is winit's raw device motion, which everything in this
//! game treats as a relative delta in mouse counts. That assumption is false on
//! **absolute** pointing devices — an RDP session's virtual mouse, VM guest
//! pointers, some streaming clients. Those report `MOUSE_MOVE_ABSOLUTE` with
//! the pointer's *position* normalized over `0..=65535`, and winit 0.30 does
//! not filter them: `MOUSE_MOVE_RELATIVE` is `0`, so its
//! `has_flag(usFlags, MOUSE_MOVE_RELATIVE)` test is `x & 0 == 0` — always true
//! — and the position is forwarded as if it were a delta.
//!
//! Measured in a real RDP session (2560x1440, `usFlags = 0x0003`):
//!
//! ```text
//!   usFlags=0x0003 lLastX=  32503 lLastY=  31903   cursor=(1280,720)
//!   usFlags=0x0003 lLastX=  32554 lLastY=  31690   cursor=(1280,720)
//! ```
//!
//! Every event carries ~32000 in both axes, so the camera receives a constant
//! enormous same-signed "delta" and whips around. Scaling cannot fix a constant
//! offset — the stream has to be *differentiated* back into motion. The same
//! measurement with the cursor locked to screen center (what the game does
//! during play) showed the positions still tracking the physical mouse, with
//! zero warp-sized jumps, so differencing is valid while grabbed.
//!
//! [`RawMouse::delta`] is the single normalized per-frame motion every consumer
//! should read; nothing else should read `MouseMotion` directly.

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::Monitor;

/// Absolute reports are normalized over this range (`MOUSE_MOVE_ABSOLUTE`).
const ABS_RANGE: f32 = 65535.0;
/// A relative report is a per-event mouse-count delta; even a hard flick on a
/// high-DPI mouse stays in the hundreds. A same-signed report this large is a
/// normalized position, not motion.
const ABS_MIN_MAGNITUDE: f32 = 4096.0;
/// Consecutive corroborating reports before switching interpretation. Guards
/// against a single freak value flipping a working local mouse.
const MODE_STREAK: u8 = 3;
/// Largest position change accepted as real hand motion (~12.5% of the screen
/// in one report; the measured worst case was 1826). Anything larger is a warp
/// — reconnect, monitor hop, focus return — and must not become camera motion.
const MAX_ABS_STEP: f32 = 8192.0;
/// Fallback virtual-desktop size if no monitors are enumerated yet.
const FALLBACK_DESKTOP: Vec2 = Vec2::new(1920.0, 1080.0);

/// Interprets the raw stream, recovering relative motion from absolute-mode
/// devices. Pure and unit-tested; [`accumulate_raw_mouse`] is the thin wrapper.
#[derive(Default)]
pub struct RawMouseFilter {
    /// Whether the stream is currently read as positions rather than deltas.
    pub absolute: bool,
    /// Previous position sample, in normalized units (absolute mode only).
    last: Option<Vec2>,
    abs_streak: u8,
    rel_streak: u8,
}

impl RawMouseFilter {
    /// Feeds one raw event and returns its contribution to this frame's motion,
    /// in mouse-count equivalents. `units_to_counts` converts a normalized step
    /// into screen pixels per axis (absolute mode only; ignored otherwise).
    pub fn push(&mut self, ev: Vec2, units_to_counts: Vec2) -> Vec2 {
        // Absolute positions are never negative; a negative component is proof
        // of a relative device. A large same-signed component is proof of an
        // absolute one. Small positive values (pointer near the desktop origin)
        // prove nothing and leave the current interpretation alone.
        let looks_absolute = ev.x >= 0.0
            && ev.y >= 0.0
            && (ev.x > ABS_MIN_MAGNITUDE || ev.y > ABS_MIN_MAGNITUDE);
        let looks_relative = ev.x < 0.0 || ev.y < 0.0;

        if looks_absolute {
            self.rel_streak = 0;
            self.abs_streak = self.abs_streak.saturating_add(1);
            if !self.absolute {
                if self.abs_streak >= MODE_STREAK {
                    self.absolute = true;
                    self.last = Some(ev);
                }
                // Never forward a suspected position as motion, latched or not:
                // dropping a few reports is invisible, leaking one is a whip
                // across the whole screen.
                return Vec2::ZERO;
            }
        } else if looks_relative {
            self.abs_streak = 0;
            self.rel_streak = self.rel_streak.saturating_add(1);
            if self.absolute && self.rel_streak >= MODE_STREAK {
                self.absolute = false;
                self.last = None;
            }
        }

        if !self.absolute {
            return ev;
        }

        match self.last.replace(ev) {
            // First sample only establishes an origin to difference against.
            None => Vec2::ZERO,
            Some(prev) => {
                let step = ev - prev;
                if step.x.abs() > MAX_ABS_STEP || step.y.abs() > MAX_ABS_STEP {
                    Vec2::ZERO
                } else {
                    step * units_to_counts
                }
            }
        }
    }
}

/// This frame's mouse motion, normalized across pointer types. Read this
/// instead of `MouseMotion`.
#[derive(Resource, Default)]
pub struct RawMouse {
    /// Relative motion accumulated this frame, in mouse-count equivalents.
    pub delta: Vec2,
    /// True while the stream is being read as absolute positions (RDP / VM).
    /// Diagnostics only — consumers just use `delta`.
    pub absolute: bool,
    filter: RawMouseFilter,
}

/// Bounding box of all monitors: absolute reports carrying
/// `MOUSE_VIRTUAL_DESKTOP` are normalized over the whole virtual desktop, not
/// one screen.
fn virtual_desktop_size(monitors: &Query<&Monitor>) -> Vec2 {
    let mut min = Vec2::splat(f32::MAX);
    let mut max = Vec2::splat(f32::MIN);
    let mut any = false;
    for m in monitors.iter() {
        let pos = m.physical_position.as_vec2();
        let size = m.physical_size().as_vec2();
        min = min.min(pos);
        max = max.max(pos + size);
        any = true;
    }
    if any {
        (max - min).max(Vec2::ONE)
    } else {
        FALLBACK_DESKTOP
    }
}

pub fn accumulate_raw_mouse(
    mut motion: EventReader<MouseMotion>,
    monitors: Query<&Monitor>,
    mut raw: ResMut<RawMouse>,
) {
    let units_to_counts = virtual_desktop_size(&monitors) / ABS_RANGE;
    let mut total = Vec2::ZERO;
    for ev in motion.read() {
        total += raw.filter.push(ev.delta, units_to_counts);
    }
    raw.delta = total;
    raw.absolute = raw.filter.absolute;
}

pub struct RawMousePlugin;

impl Plugin for RawMousePlugin {
    fn build(&self, app: &mut App) {
        // PreUpdate, after input collection: every Update consumer then sees a
        // complete, already-normalized delta for the frame.
        app.init_resource::<RawMouse>()
            .add_systems(PreUpdate, accumulate_raw_mouse.after(bevy::input::InputSystem));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2560x1440, the session this was measured on.
    fn units() -> Vec2 {
        Vec2::new(2560.0, 1440.0) / ABS_RANGE
    }

    #[test]
    fn relative_device_passes_through_untouched() {
        let mut f = RawMouseFilter::default();
        for d in [Vec2::new(3.0, -2.0), Vec2::new(-40.0, 12.0), Vec2::new(0.0, 5.0)] {
            assert_eq!(f.push(d, units()), d);
        }
        assert!(!f.absolute);
    }

    #[test]
    fn a_single_freak_value_does_not_flip_a_local_mouse() {
        let mut f = RawMouseFilter::default();
        f.push(Vec2::new(9000.0, 3.0), units());
        assert!(!f.absolute, "one report must not latch absolute mode");
        // ...and a following negative delta clears the streak entirely.
        assert_eq!(f.push(Vec2::new(-5.0, -5.0), units()), Vec2::new(-5.0, -5.0));
        f.push(Vec2::new(9000.0, 3.0), units());
        f.push(Vec2::new(9000.0, 3.0), units());
        assert!(!f.absolute);
    }

    #[test]
    fn absolute_positions_never_leak_as_motion() {
        let mut f = RawMouseFilter::default();
        // The real captured stream: ~32000 in both axes, every event.
        for _ in 0..3 {
            assert_eq!(
                f.push(Vec2::new(32503.0, 31903.0), units()),
                Vec2::ZERO,
                "a position must never be forwarded as a delta"
            );
        }
        assert!(f.absolute);
    }

    #[test]
    fn absolute_stream_is_differenced_into_pixel_motion() {
        let mut f = RawMouseFilter::default();
        for _ in 0..3 {
            f.push(Vec2::new(32503.0, 31903.0), units());
        }
        assert!(f.absolute);
        // +256 units on x is 256 * 2560/65535 = 10 px of real pointer travel.
        let d = f.push(Vec2::new(32759.0, 31903.0), units());
        assert!((d.x - 10.0).abs() < 0.01, "got {d:?}");
        assert_eq!(d.y, 0.0);
        // Motion is signed even though the underlying values never are.
        let d = f.push(Vec2::new(32503.0, 31903.0), units());
        assert!((d.x + 10.0).abs() < 0.01, "got {d:?}");
    }

    #[test]
    fn near_origin_positions_still_difference_correctly() {
        let mut f = RawMouseFilter::default();
        for _ in 0..3 {
            f.push(Vec2::new(32503.0, 31903.0), units());
        }
        // Pointer walks to the top-left corner: small positive values that look
        // neither absolute nor relative must not be mistaken for deltas.
        f.push(Vec2::new(600.0, 400.0), units()); // big step, rejected as a warp
        let d = f.push(Vec2::new(856.0, 400.0), units());
        assert!((d.x - 10.0).abs() < 0.01, "got {d:?}");
        assert!(f.absolute);
    }

    #[test]
    fn warp_sized_jumps_are_dropped() {
        let mut f = RawMouseFilter::default();
        for _ in 0..3 {
            f.push(Vec2::new(32503.0, 31903.0), units());
        }
        // Reconnect / monitor hop: a whole-screen jump is not hand motion.
        assert_eq!(f.push(Vec2::new(1000.0, 31903.0), units()), Vec2::ZERO);
        // ...but the new position becomes the origin, so motion resumes.
        let d = f.push(Vec2::new(1256.0, 31903.0), units());
        assert!((d.x - 10.0).abs() < 0.01, "got {d:?}");
    }

    #[test]
    fn a_relative_device_taking_over_unlatches() {
        let mut f = RawMouseFilter::default();
        for _ in 0..3 {
            f.push(Vec2::new(32503.0, 31903.0), units());
        }
        assert!(f.absolute);
        for _ in 0..3 {
            f.push(Vec2::new(-4.0, -3.0), units());
        }
        assert!(!f.absolute);
        assert_eq!(f.push(Vec2::new(7.0, -2.0), units()), Vec2::new(7.0, -2.0));
    }

    #[test]
    fn eight_seconds_of_measured_rdp_circles_stays_sane() {
        // Reproduces the probe's summary: 538 events, positions confined to a
        // ~3500-unit box, differences summing to ~3500 px of hand motion. The
        // camera must see that, not 34 million units.
        let mut f = RawMouseFilter::default();
        let mut travel = 0.0;
        for i in 0..538 {
            let t = i as f32 * 0.1;
            let p = Vec2::new(32800.0 + 1780.0 * t.cos(), 32600.0 + 1780.0 * t.sin());
            travel += f.push(p, units()).length();
        }
        assert!(f.absolute);
        // Circles of ~70 px radius over 538 samples: thousands of px, not
        // millions of units.
        assert!(travel > 100.0 && travel < 20_000.0, "travel was {travel}");
    }
}
