use bevy::prelude::*;

use crate::player::{Player, PlayerPhysics, SquashState};
use crate::GamePaused;

use super::components::*;
use super::constants::*;
use super::debugger;
use super::resources::*;

pub(super) fn terrain_collision(
    surfaces: Query<&TerrainSurface>,
    solids: Query<&SolidBlock>,
    mut player_q: Query<(&mut Transform, &mut PlayerPhysics, &mut SquashState), (With<Player>, Without<SuckUpAnimation>, Without<ZipLineRide>)>,
    time: Res<Time>,
) {
    let Ok((mut transform, mut physics, mut squash)) = player_q.get_single_mut() else {
        return;
    };

    let was_airborne = !physics.grounded;

    // Push player out of solid blocks horizontally (multi-iteration to handle cascading)
    // (only applies when player is below the top surface of the block)
    for _iter in 0..3 {
        let mut pushed = false;
        for solid in &solids {
            let px = transform.translation.x;
            let pz = transform.translation.z;
            let py = transform.translation.y;
            if py >= solid.y_max {
                continue; // player is on top, no horizontal blocking needed
            }
            let body_top = py + 1.6;
            let overlap = body_top.min(solid.y_max) - py.max(solid.y_min);
            if overlap < 0.3 {
                continue; // head barely grazing a block shouldn't cause XZ pushout
            }
            // Check if player is inside this solid block in XZ
            let margin = 0.3; // player radius
            if px + margin > solid.min.x && px - margin < solid.max.x
                && pz + margin > solid.min.y && pz - margin < solid.max.y
            {
                // Find shortest push-out direction
                let push_left = (px + margin) - solid.min.x;
                let push_right = solid.max.x - (px - margin);
                let push_front = (pz + margin) - solid.min.y;
                let push_back = solid.max.y - (pz - margin);

                let min_push = push_left.min(push_right).min(push_front).min(push_back);

                if min_push == push_left {
                    transform.translation.x = solid.min.x - margin;
                    physics.velocity.x = physics.velocity.x.min(0.0);
                } else if min_push == push_right {
                    transform.translation.x = solid.max.x + margin;
                    physics.velocity.x = physics.velocity.x.max(0.0);
                } else if min_push == push_front {
                    transform.translation.z = solid.min.y - margin;
                    physics.velocity.z = physics.velocity.z.min(0.0);
                } else {
                    transform.translation.z = solid.max.y + margin;
                    physics.velocity.z = physics.velocity.z.max(0.0);
                }
                pushed = true;
            }
        }
        if !pushed { break; }
    }

    // Ceiling collision: prevent jumping through bottom of solid blocks
    let player_height = 1.8;
    for solid in &solids {
        let px = transform.translation.x;
        let pz = transform.translation.z;
        let py = transform.translation.y;
        let margin = 0.3;
        if px + margin > solid.min.x && px - margin < solid.max.x
            && pz + margin > solid.min.y && pz - margin < solid.max.y
        {
            if physics.velocity.y > 0.0 && py + player_height > solid.y_min && py < solid.y_min {
                transform.translation.y = solid.y_min - player_height;
                physics.velocity.y = 0.0;
            }
        }
    }

    // Find the highest surface under the player.
    // When falling, use generous tolerance (0.5) to prevent phasing through platforms.
    // When jumping upward, use tight tolerance so we don't snap to surfaces above.
    let px = transform.translation.x;
    let pz = transform.translation.z;
    let dt = time.delta_secs();
    let tolerance = if physics.velocity.y <= 0.0 {
        (physics.velocity.y.abs() * dt + 0.5).min(2.0)
    } else {
        0.0
    };
    let mut best_y = -2.0_f32; // below any surface (pool is at -1.2)
    for surf in &surfaces {
        if px >= surf.min.x && px <= surf.max.x && pz >= surf.min.y && pz <= surf.max.y {
            if surf.y <= transform.translation.y + tolerance && surf.y > best_y {
                best_y = surf.y;
            }
        }
    }

    // Snap player to surface if on or below it (but not while jumping upward)
    if transform.translation.y <= best_y + 0.1 && physics.velocity.y <= 0.0 {
        transform.translation.y = best_y;
        physics.velocity.y = 0.0;
        if was_airborne {
            squash.timer = 0.3;
        }
        physics.grounded = true;
    } else if transform.translation.y > best_y + 0.2 {
        // Player is above the ground — they should be falling
        physics.grounded = false;
    }
}

pub(super) fn slide_force_system(
    time: Res<Time>,
    hill_state: Res<HillState>,
    slides: Query<&SlideSegment>,
    mut player_q: Query<(&Transform, &mut PlayerPhysics), (With<Player>, Without<SuckUpAnimation>, Without<ZipLineRide>)>,
) {
    let Ok((transform, mut physics)) = player_q.get_single_mut() else {
        return;
    };

    let px = transform.translation.x;
    let pz = transform.translation.z;
    let py = transform.translation.y;

    for seg in &slides {
        if px >= seg.min.x && px <= seg.max.x && pz >= seg.min.y && pz <= seg.max.y {
            if (py - seg.y).abs() < 1.0 {
                // Player is on this slide segment - apply force pushing them downhill (positive X = away from hill)
                let direction = Vec3::new(1.0, 0.0, 0.0);
                debugger::apply_slide_force(hill_state.slide_friction, &mut physics.velocity, direction, time.delta_secs());
                break;
            }
        }
    }
}

pub(super) fn water_slide_system(
    slides: Query<&WaterSlideSegment>,
    mut player_q: Query<(&Transform, &mut PlayerPhysics), (With<Player>, Without<SuckUpAnimation>, Without<ZipLineRide>)>,
) {
    let Ok((transform, mut physics)) = player_q.get_single_mut() else {
        return;
    };

    let px = transform.translation.x;
    let pz = transform.translation.z;
    let py = transform.translation.y;

    for seg in &slides {
        if px >= seg.min.x && px <= seg.max.x && pz >= seg.min.y && pz <= seg.max.y {
            if (py - seg.y).abs() < 1.0 {
                // Force player velocity toward the pool (-x direction)
                physics.velocity.x = -6.0;
                break;
            }
        }
    }
}

pub(super) fn follow_camera(
    time: Res<Time>,
    player_q: Query<&Transform, (With<Player>, Without<HillFollowCam>)>,
    mut cam_q: Query<&mut Transform, (With<HillFollowCam>, Without<Player>)>,
) {
    let Ok(player_tf) = player_q.get_single() else { return };
    let Ok(mut cam_tf) = cam_q.get_single_mut() else { return };

    let target_pos = player_tf.translation + CAM_OFFSET;
    let lerp_factor = (12.0 * time.delta_secs()).min(1.0);
    cam_tf.translation = cam_tf.translation.lerp(target_pos, lerp_factor);
    cam_tf.look_at(player_tf.translation, Vec3::Y);
}

pub(super) fn hill_playing_update(
    game_paused: Res<GamePaused>,
) {
    if game_paused.0 {
        return;
    }
}
