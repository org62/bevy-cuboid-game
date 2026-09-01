//! The shared follow camera and its orbit input.
//!
//! The camera is rigidly anchored to the player by design — see CLAUDE.md's
//! frame-pacing notes before adding any smoothing layer here.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};

use crate::player::Player;

#[derive(Component)]
pub struct FollowCamera {
    pub offset: Vec3,
    pub look_offset: Vec3,
}

#[derive(Resource)]
pub struct CameraOrbit {
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
}

impl Default for CameraOrbit {
    fn default() -> Self {
        Self { yaw: 0.0, pitch: 0.0, zoom: 1.0 }
    }
}

/// User-adjustable mouse-look sensitivity (a multiplier on the base rate).
#[derive(Resource)]
pub struct MouseSettings {
    pub sensitivity: f32,
}

impl Default for MouseSettings {
    fn default() -> Self {
        Self { sensitivity: 0.4 }
    }
}

// --- Camera follow ---

/// Inflation of occluder boxes toward the camera — acts as the camera's
/// "radius" so the near plane (0.1) never pokes through a wall face.
const OCCLUSION_PAD_XZ: f32 = 0.35;
const OCCLUSION_PAD_Y: f32 = 0.2;
/// The occlusion ray starts at the player's torso, not the feet. Load-bearing:
/// the feet rest exactly on surface tops, so an unlifted origin would sit ON
/// the top face of any walk-on occluder and false-hit at t=0 every frame.
const OCCLUSION_ORIGIN_LIFT: f32 = 0.6;
/// Never pull the camera closer than this to the player.
const MIN_CAMERA_DISTANCE: f32 = 0.6;
/// Easing rate (per second) for the camera moving back OUT when an occlusion
/// clears. Pulling IN is instant — the camera must never spend a frame
/// behind a wall.
const OCCLUSION_RECOVER_RATE: f32 = 4.0;

pub fn follow_camera_system(
    player_q: Query<&Transform, (With<Player>, Without<FollowCamera>)>,
    mut cam_q: Query<(&mut Transform, &FollowCamera), Without<Player>>,
    occluders: Query<
        &crate::terrain::SolidBlock,
        With<crate::terrain::CameraOccluder>,
    >,
    orbit: Res<CameraOrbit>,
    mut prev_p: Local<Option<Vec3>>,
    mut occ_frac: Local<Option<f32>>,
    time: Res<Time>,
) {
    let Ok(player_tf) = player_q.get_single() else { return };
    let Ok((mut cam_tf, follow)) = cam_q.get_single_mut() else { return };
    let dt = time.delta_secs();
    let p = player_tf.translation;
    // The camera is rigidly anchored to the player — no follow smoothing.
    // Positional smoothing layers re-integrate frame-time jitter and read as
    // judder; the frame_pacing module is the one place time is smoothed.
    if prev_p.map_or(false, |s| s.distance_squared(p) > 100.0) {
        *occ_frac = None; // teleport: don't carry over a pulled-in distance
    }
    *prev_p = Some(p);

    // Apply the orbit rotation directly (no position lerp) so mouse-look is
    // responsive.
    let rot = Quat::from_rotation_y(orbit.yaw) * Quat::from_rotation_x(orbit.pitch);
    let target = p + rot * (follow.offset * orbit.zoom);

    // Occlusion: cast from the player's torso toward the desired camera spot
    // and clamp to the nearest tagged wall, so the camera dollies in front of
    // geometry instead of clipping inside it. Levels without CameraOccluder
    // entities skip the loop entirely.
    let ray_origin = p + follow.look_offset + Vec3::Y * OCCLUSION_ORIGIN_LIFT;
    let to_cam = target - ray_origin;
    let full = to_cam.length();
    let desired = if full > 1e-4 {
        let dir = to_cam / full;
        let mut allowed = full;
        for block in &occluders {
            if let Some(t) =
                block.ray_entry(ray_origin, dir, full, OCCLUSION_PAD_XZ, OCCLUSION_PAD_Y)
            {
                allowed = allowed.min(t);
            }
        }
        allowed = allowed.max(MIN_CAMERA_DISTANCE);
        // Track occlusion as a FRACTION of the desired distance, not an
        // absolute distance: zoom/pitch changes of the unoccluded camera then
        // apply instantly instead of lagging behind the recovery ease (an
        // absolute-distance ease made every zoom-out crawl at 4 u/s, which
        // read as a choppy/sticky camera on every level).
        let target_frac = (allowed / full).min(1.0);
        let f = occ_frac.get_or_insert(1.0);
        if target_frac < *f {
            *f = target_frac; // pull in instantly — never spend a frame in a wall
        } else {
            *f += (target_frac - *f) * (OCCLUSION_RECOVER_RATE * dt).min(1.0);
            if target_frac - *f < 0.01 {
                *f = target_frac; // settle so unoccluded frames don't keep easing
            }
        }
        ray_origin + dir * (*f * full)
    } else {
        target
    };
    cam_tf.translation = desired;
    cam_tf.look_at(p + follow.look_offset, Vec3::Y);
}

#[allow(clippy::too_many_arguments)]
pub fn update_camera_orbit(
    gamepads: Query<&Gamepad>,
    time: Res<Time>,
    game_paused: Res<crate::GamePaused>,
    raw_mouse: Res<crate::raw_mouse::RawMouse>,
    mut mouse_wheel: EventReader<MouseWheel>,
    windows: Query<&Window, With<PrimaryWindow>>,
    settings: Res<MouseSettings>,
    mut orbit: ResMut<CameraOrbit>,
) {
    if game_paused.0 {
        return;
    }
    const DEADZONE: f32 = 0.2;
    const YAW_RATE: f32 = 2.5;
    const PITCH_RATE: f32 = 1.5;
    const PITCH_CLAMP: f32 = 0.7;
    const TRIGGER_DEADZONE: f32 = 0.05;
    const ZOOM_RATE: f32 = 1.5;
    const ZOOM_MIN: f32 = 0.5;
    const ZOOM_MAX: f32 = 2.5;
    // Mouse sensitivities
    const MOUSE_YAW_SENS: f32 = 0.006;
    const MOUSE_PITCH_SENS: f32 = 0.005;
    const WHEEL_ZOOM_STEP: f32 = 0.12;
    let dt = time.delta_secs();
    for gp in &gamepads {
        let stick = gp.right_stick();
        if stick.x.abs() > DEADZONE {
            orbit.yaw -= stick.x * YAW_RATE * dt;
        }
        if stick.y.abs() > DEADZONE {
            orbit.pitch =
                (orbit.pitch + stick.y * PITCH_RATE * dt).clamp(-PITCH_CLAMP, PITCH_CLAMP);
        }
        // Only count a trigger when it's actually pressed past the deadzone.
        // Some controllers rest their trigger axes at a nonzero value (often
        // -1), which would otherwise drift the zoom every frame.
        let rt = gp.get(GamepadButton::RightTrigger2).unwrap_or(0.0);
        let lt = gp.get(GamepadButton::LeftTrigger2).unwrap_or(0.0);
        let rt = if rt > TRIGGER_DEADZONE { rt } else { 0.0 };
        let lt = if lt > TRIGGER_DEADZONE { lt } else { 0.0 };
        let zoom_delta = rt - lt;
        if zoom_delta != 0.0 {
            orbit.zoom = (orbit.zoom - zoom_delta * ZOOM_RATE * dt).clamp(ZOOM_MIN, ZOOM_MAX);
        }
    }

    // Mouse free-look: when the cursor is grabbed (see `manage_cursor_grab`)
    // raw motion turns the camera directly, no button needed. When the cursor
    // is released (menu / pause / modal), motion is ignored so it doesn't fight
    // the UI. Motion comes from `RawMouse`, never from `MouseMotion` directly —
    // see `src/raw_mouse.rs`.
    let cursor_grabbed = windows
        .get_single()
        .map(|w| w.cursor_options.grab_mode != CursorGrabMode::None)
        .unwrap_or(false);
    let look = raw_mouse.delta;
    if cursor_grabbed && look != Vec2::ZERO {
        let sens = settings.sensitivity;
        orbit.yaw -= look.x * MOUSE_YAW_SENS * sens;
        orbit.pitch =
            (orbit.pitch - look.y * MOUSE_PITCH_SENS * sens).clamp(-PITCH_CLAMP, PITCH_CLAMP);
    }

    // Mouse wheel zooms.
    let mut scroll = 0.0;
    for ev in mouse_wheel.read() {
        scroll += ev.y;
    }
    if scroll != 0.0 {
        orbit.zoom = (orbit.zoom - scroll * WHEEL_ZOOM_STEP).clamp(ZOOM_MIN, ZOOM_MAX);
    }
}
