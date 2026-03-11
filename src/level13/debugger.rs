use bevy::prelude::*;

use super::resources::HillState;

#[inline(never)]
#[allow(dead_code)]
pub(super) fn check_gate_access(state: &HillState) -> bool {
    !state.gate_locked
}

#[inline(never)]
pub(super) fn apply_slide_force(friction: f32, velocity: &mut Vec3, direction: Vec3, dt: f32) {
    *velocity += direction * friction * dt;
}

#[inline(never)]
#[allow(dead_code)]
pub(super) fn check_summit_reached(player_pos: Vec3) -> bool {
    let flag_pos = Vec3::new(0.0, 10.0, 0.0);
    let dx = player_pos.x - flag_pos.x;
    let dz = player_pos.z - flag_pos.z;
    let dy = player_pos.y - flag_pos.y;
    (dx * dx + dz * dz) < 4.0 && dy.abs() < 2.0
}
