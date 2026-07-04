use bevy::prelude::*;

use crate::player::Player;

use super::components::*;
use super::constants::*;
use super::resources::*;

/// Returns the world position of checkpoint i (pulled 2 units back from the turn).
fn checkpoint_position(i: usize) -> Vec3 {
    let cp_idx = RACE_CHECKPOINT_INDICES[i];
    let wp = RACE_WAYPOINTS[cp_idx];
    let prev = if cp_idx == 0 { RACE_WAYPOINTS[NUM_WAYPOINTS - 1] } else { RACE_WAYPOINTS[cp_idx - 1] };
    let incoming_dir = (wp - prev).normalize();
    wp - incoming_dir * 2.0
}

/// Cached checkpoint positions (computed once since they derive from constants).
fn cached_checkpoint_positions() -> &'static [Vec3; NUM_CHECKPOINTS] {
    use std::sync::OnceLock;
    static POSITIONS: OnceLock<[Vec3; NUM_CHECKPOINTS]> = OnceLock::new();
    POSITIONS.get_or_init(|| std::array::from_fn(checkpoint_position))
}

fn reset_bot(tf: &mut Transform, bot: &mut RaceBot) {
    bot.progress = 0.0;
    tf.translation = RACE_WAYPOINTS[0];
    // First segment goes west (-x), so face -x = rotation of +PI/2 around Y
    tf.rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
}

fn race_track_total_length() -> f32 {
    use std::sync::OnceLock;
    static LEN: OnceLock<f32> = OnceLock::new();
    *LEN.get_or_init(|| {
        let mut total = 0.0_f32;
        for i in 0..NUM_WAYPOINTS {
            let a = RACE_WAYPOINTS[i];
            let b = RACE_WAYPOINTS[(i + 1) % NUM_WAYPOINTS];
            total += a.distance(b);
        }
        total
    })
}

pub(super) fn spawn_race_bot(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) -> Entity {
    let bot_green = materials.add(StandardMaterial {
        base_color: Color::srgb(0.1, 0.8, 0.2),
        ..default()
    });
    let white = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        ..default()
    });
    let black = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        ..default()
    });

    let max_speed = 7.0_f32;
    commands
        .spawn((
            Transform::from_translation(RACE_WAYPOINTS[0])
                .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
            Visibility::default(),
            RaceBot {
                progress: 0.0,
                speed: max_speed * RACE_BOT_SPEED_FACTOR,
            },
            HillEntity,
        ))
        .with_children(|parent| {
            // Body
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.6, 0.8, 0.5))),
                MeshMaterial3d(bot_green.clone()),
                Transform::from_xyz(0.0, 0.4, 0.0),
            ));
            // Head with eyes
            parent
                .spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.65, 0.6, 0.55))),
                    MeshMaterial3d(bot_green),
                    Transform::from_xyz(0.0, 1.1, 0.0),
                ))
                .with_children(|head| {
                    // Left eye
                    head.spawn((
                        Mesh3d(meshes.add(Sphere::new(0.09))),
                        MeshMaterial3d(white.clone()),
                        Transform::from_xyz(-0.15, 0.05, -0.28),
                    ))
                    .with_children(|eye| {
                        eye.spawn((
                            Mesh3d(meshes.add(Sphere::new(0.045))),
                            MeshMaterial3d(black.clone()),
                            Transform::from_xyz(0.0, 0.0, -0.05),
                        ));
                    });
                    // Right eye
                    head.spawn((
                        Mesh3d(meshes.add(Sphere::new(0.09))),
                        MeshMaterial3d(white),
                        Transform::from_xyz(0.15, 0.05, -0.28),
                    ))
                    .with_children(|eye| {
                        eye.spawn((
                            Mesh3d(meshes.add(Sphere::new(0.045))),
                            MeshMaterial3d(black),
                            Transform::from_xyz(0.0, 0.0, -0.05),
                        ));
                    });
                });
        })
        .id()
}

pub(super) fn race_trigger_system(
    player_q: Query<&Transform, With<Player>>,
    mut race_state: ResMut<HillRaceState>,
    mut bot_q: Query<(&mut Transform, &mut RaceBot), Without<Player>>,
    mut countdown_text_q: Query<&mut Text, (With<RaceCountdownText>, Without<RaceStatusText>)>,
) {
    if race_state.phase != HillRacePhase::Idle {
        return;
    }
    let Ok(player_tf) = player_q.get_single() else { return };
    let pp = player_tf.translation;
    let start = RACE_WAYPOINTS[0];
    let dx = pp.x - start.x;
    let dz = pp.z - start.z;
    if dx * dx + dz * dz < START_ZONE_RADIUS * START_ZONE_RADIUS && (pp.y - start.y).abs() < 2.0 {
        race_state.phase = HillRacePhase::Countdown;
        race_state.countdown_timer = 3.0;
        race_state.player_checkpoints = [false; NUM_CHECKPOINTS];
        race_state.bot_checkpoints = [false; NUM_CHECKPOINTS];
        race_state.result_timer = 0.0;

        // Reset bot to start
        for (mut tf, mut bot) in &mut bot_q {
            reset_bot(&mut tf, &mut bot);
        }

        // Show countdown
        for mut text in &mut countdown_text_q {
            **text = "3".to_string();
        }
    }
}

pub(super) fn race_countdown_system(
    time: Res<Time>,
    mut race_state: ResMut<HillRaceState>,
    mut countdown_text_q: Query<&mut Text, (With<RaceCountdownText>, Without<RaceStatusText>)>,
    mut status_text_q: Query<&mut Text, (With<RaceStatusText>, Without<RaceCountdownText>)>,
) {
    if race_state.phase != HillRacePhase::Countdown {
        return;
    }
    race_state.countdown_timer -= time.delta_secs();

    let display = if race_state.countdown_timer > 2.0 {
        "3"
    } else if race_state.countdown_timer > 1.0 {
        "2"
    } else if race_state.countdown_timer > 0.0 {
        "1"
    } else {
        "GO!"
    };

    // Only update text when it actually changes to avoid triggering change detection every frame
    for mut text in &mut countdown_text_q {
        if text.as_str() != display {
            **text = display.to_string();
        }
    }

    if race_state.countdown_timer <= -0.5 {
        race_state.phase = HillRacePhase::Racing;
        race_state.countdown_timer = 0.0;
        for mut text in &mut countdown_text_q {
            if !text.is_empty() {
                **text = String::new();
            }
        }
        for mut text in &mut status_text_q {
            **text = format!("Checkpoints: 0/{}", NUM_CHECKPOINTS);
        }
    }
}

pub(super) fn race_bot_movement_system(
    time: Res<Time>,
    mut race_state: ResMut<HillRaceState>,
    mut bot_q: Query<(&mut Transform, &mut RaceBot), Without<Player>>,
) {
    if race_state.phase != HillRacePhase::Racing {
        return;
    }
    let dt = time.delta_secs();
    let total_len = race_track_total_length();

    for (mut tf, mut bot) in &mut bot_q {
        let advance = bot.speed * dt / total_len;
        bot.progress += advance;

        if bot.progress >= 1.0 {
            bot.progress = 1.0;
            // Bot finished — loss handled in result system
        }

        // Interpolate position along waypoints
        let total_progress = bot.progress * total_len;
        let mut accumulated = 0.0_f32;
        let mut pos = RACE_WAYPOINTS[0];
        let mut dir = Vec3::Z;
        for i in 0..NUM_WAYPOINTS {
            let a = RACE_WAYPOINTS[i];
            let b = RACE_WAYPOINTS[(i + 1) % NUM_WAYPOINTS];
            let seg_len = a.distance(b);
            if accumulated + seg_len >= total_progress {
                let t = (total_progress - accumulated) / seg_len;
                pos = a.lerp(b, t);
                dir = (b - a).normalize();
                break;
            }
            accumulated += seg_len;
        }

        tf.translation = pos;
        // Face movement direction
        let look_target = pos + dir;
        let look_tf = Transform::from_translation(pos).looking_at(look_target, Vec3::Y);
        tf.rotation = look_tf.rotation;

        // Detect bot checkpoint crossings using same radius as player
        let cp_positions = cached_checkpoint_positions();
        for i in 0..NUM_CHECKPOINTS {
            if race_state.bot_checkpoints[i] {
                continue;
            }
            let cp_pos = cp_positions[i];
            let dx = pos.x - cp_pos.x;
            let dz = pos.z - cp_pos.z;
            if dx * dx + dz * dz < CHECKPOINT_RADIUS * CHECKPOINT_RADIUS {
                race_state.bot_checkpoints[i] = true;
            }
        }
    }
}

pub(super) fn race_player_tracking_system(
    mut race_state: ResMut<HillRaceState>,
    player_q: Query<&Transform, With<Player>>,
    bot_q: Query<&RaceBot>,
    mut status_text_q: Query<&mut Text, (With<RaceStatusText>, Without<RaceCountdownText>)>,
    mut prev_reached: Local<usize>,
) {
    if race_state.phase != HillRacePhase::Racing {
        return;
    }
    let Ok(player_tf) = player_q.get_single() else { return };
    let pp = player_tf.translation;

    // Check every checkpoint — player can reach them in any order
    let cp_positions = cached_checkpoint_positions();
    for i in 0..NUM_CHECKPOINTS {
        if race_state.player_checkpoints[i] {
            continue;
        }
        let cp_pos = cp_positions[i];
        let dx = pp.x - cp_pos.x;
        let dz = pp.z - cp_pos.z;
        if dx * dx + dz * dz < CHECKPOINT_RADIUS * CHECKPOINT_RADIUS && (pp.y - cp_pos.y).abs() < 2.0 {
            race_state.player_checkpoints[i] = true;
        }
    }

    let reached = race_state.player_checkpoints.iter().filter(|&&v| v).count();
    let all_reached = reached == NUM_CHECKPOINTS;

    // Only update status text when checkpoint count changes to avoid per-frame allocation
    if reached != *prev_reached {
        *prev_reached = reached;
        let msg = if all_reached {
            "All checkpoints! Return to start!".to_string()
        } else {
            format!("Checkpoints: {}/{}", reached, NUM_CHECKPOINTS)
        };
        for mut text in &mut status_text_q {
            **text = msg.clone();
        }
    }

    // Check if all checkpoints cleared and player is back at start
    if all_reached {
        let start = RACE_WAYPOINTS[0];
        let dx = pp.x - start.x;
        let dz = pp.z - start.z;
        if dx * dx + dz * dz < START_ZONE_RADIUS * START_ZONE_RADIUS && (pp.y - start.y).abs() < 2.0 {
            race_state.phase = HillRacePhase::Won;
            race_state.result_timer = 4.0;
            return;
        }
    }

    // Check if bot finished
    for bot in &bot_q {
        if bot.progress >= 1.0 {
            race_state.phase = HillRacePhase::Lost;
            race_state.result_timer = 4.0;
            return;
        }
    }
}

pub(super) fn race_result_system(
    time: Res<Time>,
    mut race_state: ResMut<HillRaceState>,
    mut status_text_q: Query<(&mut Text, &mut TextColor), (With<RaceStatusText>, Without<RaceCountdownText>)>,
    mut bot_q: Query<(&mut Transform, &mut RaceBot), Without<Player>>,
) {
    let (msg, col) = match race_state.phase {
        HillRacePhase::Won => ("YOU WON THE RACE!", Color::srgb(0.1, 1.0, 0.2)),
        HillRacePhase::Lost => ("Bot wins! Try again.", Color::srgb(1.0, 0.2, 0.1)),
        _ => return,
    };

    // Only update text once on phase entry (when result_timer is still near max)
    let just_entered = race_state.result_timer > 3.9;
    if just_entered {
        for (mut text, mut color) in &mut status_text_q {
            **text = msg.to_string();
            color.0 = col;
        }
    }

    race_state.result_timer -= time.delta_secs();
    if race_state.result_timer <= 0.0 {
        race_state.phase = HillRacePhase::Idle;
        race_state.player_checkpoints = [false; NUM_CHECKPOINTS];
        race_state.bot_checkpoints = [false; NUM_CHECKPOINTS];

        // Clear status text
        for (mut text, mut color) in &mut status_text_q {
            **text = String::new();
            color.0 = Color::WHITE;
        }

        // Reset bot to start
        for (mut tf, mut bot) in &mut bot_q {
            reset_bot(&mut tf, &mut bot);
        }
    }
}

/// Swaps checkpoint pillar materials based on bitmask state.
/// Player pillars light blue, bot pillars light green. Resets on Idle.
pub(super) fn race_checkpoint_light_system(
    race_state: Res<HillRaceState>,
    cp_mats: Option<Res<CheckpointMaterials>>,
    mut player_marker_q: Query<
        (&RaceCheckpointPlayer, &mut MeshMaterial3d<StandardMaterial>),
        Without<RaceCheckpointBot>,
    >,
    mut bot_marker_q: Query<
        (&RaceCheckpointBot, &mut MeshMaterial3d<StandardMaterial>),
        Without<RaceCheckpointPlayer>,
    >,
) {
    let Some(cp_mats) = cp_mats else { return };

    if !race_state.is_changed() {
        return;
    }

    for (marker, mut mat) in &mut player_marker_q {
        let target = if race_state.player_checkpoints[marker.index] {
            &cp_mats.player_lit
        } else {
            &cp_mats.unlit
        };
        if mat.0 != *target {
            mat.0 = target.clone();
        }
    }

    for (marker, mut mat) in &mut bot_marker_q {
        let target = if race_state.bot_checkpoints[marker.index] {
            &cp_mats.bot_lit
        } else {
            &cp_mats.unlit
        };
        if mat.0 != *target {
            mat.0 = target.clone();
        }
    }
}
