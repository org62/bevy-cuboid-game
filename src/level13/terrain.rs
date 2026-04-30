use bevy::prelude::*;

use crate::player::{Player, PlayerPhysics, SquashState};

use super::components::*;
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

    // Previous-frame position. player_movement already advanced the transform
    // by `velocity * dt`, so subtracting it back gives where the player was
    // before this frame's motion. Used for the swept-path test below — without
    // it, fast diagonal motion (e.g. sliding down stair-stepped surfaces, or
    // running off the edge of a tier) lets the player tunnel between cells
    // without ever satisfying `player.y <= surface.y + 0.1`.
    let prev_x = px - physics.velocity.x * dt;
    let prev_y = transform.translation.y - physics.velocity.y * dt;
    let prev_z = pz - physics.velocity.z * dt;

    let mut best_y = -2.0_f32; // below any surface (pool is at -1.2)
    for surf in &surfaces {
        let in_now = px >= surf.min.x && px <= surf.max.x && pz >= surf.min.y && pz <= surf.max.y;
        let in_prev = prev_x >= surf.min.x && prev_x <= surf.max.x
            && prev_z >= surf.min.y && prev_z <= surf.max.y;
        if !(in_now || in_prev) { continue; }

        // Standing / vertical phase-through check (only at current XZ).
        if in_now && surf.y <= transform.translation.y + tolerance && surf.y > best_y {
            best_y = surf.y;
        }
        // Swept check: if the player crossed this surface vertically while
        // moving downward, snap to it even if the current XZ is over a cell
        // whose surface is lower.
        if physics.velocity.y <= 0.0
            && prev_y >= surf.y - 0.05
            && transform.translation.y < surf.y
            && surf.y > best_y
        {
            best_y = surf.y;
        }
    }

    // Snap player to surface if on or below it, OR if the swept test showed
    // they crossed it during the frame, OR step-down: player was statically
    // grounded last frame and walked off a small ledge. Step-down keeps fast
    // slides and tier walk-offs in contact with each successive surface
    // instead of letting horizontal velocity carry the player past several
    // cells before gravity catches them. Gated on `vy.abs < 0.01` so jumps
    // and active falls aren't intercepted. `best_y > -1.5` keeps the snap
    // out of the void (terrain surfaces start at the pool top -1.2).
    let crossed = physics.velocity.y <= 0.0 && prev_y >= best_y && transform.translation.y < best_y;
    let static_grounded_last = !was_airborne && physics.velocity.y.abs() < 0.01;
    let step_down = static_grounded_last
        && transform.translation.y > best_y
        && transform.translation.y - best_y <= 1.5
        && best_y > -1.5;
    if (transform.translation.y <= best_y + 0.1 || crossed || step_down) && physics.velocity.y <= 0.0 {
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



