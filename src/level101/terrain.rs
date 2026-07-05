use bevy::prelude::*;

use crate::player::{Player, PlayerPhysics};
use crate::terrain::{TerrainPhysicsExempt, WaterSlideSegment, SLIDE_CARRY_SPEED};

use super::components::*;
use super::debugger;
use super::resources::*;

pub(super) fn slide_force_system(
    time: Res<Time>,
    hill_state: Res<HillState>,
    slides: Query<&SlideSegment>,
    mut player_q: Query<(&Transform, &mut PlayerPhysics), (With<Player>, Without<TerrainPhysicsExempt>)>,
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
    mut player_q: Query<(&Transform, &mut PlayerPhysics), (With<Player>, Without<TerrainPhysicsExempt>)>,
) {
    let Ok((transform, mut physics)) = player_q.get_single_mut() else {
        return;
    };

    for seg in &slides {
        if seg.carries(transform.translation) {
            // Force player velocity along the slide (toward the pool)
            physics.velocity.x = seg.direction.x * SLIDE_CARRY_SPEED;
            physics.velocity.z = seg.direction.z * SLIDE_CARRY_SPEED;
            break;
        }
    }
}



