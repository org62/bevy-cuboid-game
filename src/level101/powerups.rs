use bevy::prelude::*;

use crate::player::{Player, PowerUpState};

use super::components::*;
use super::constants::PICKUP_RADIUS;
use super::resources::*;

/// Pick a random position on reachable ground, avoiding the hill center and pool.
pub(super) fn random_apple_pos(rng: &mut AppleRng) -> Vec3 {
    // (x_min, x_max, z_min, z_max) — all at ground level y=0
    let zones: &[(f32, f32, f32, f32)] = &[
        (10.0, 28.0, -28.0, 28.0),   // East
        (-13.0, 9.0, 10.0, 28.0),    // South
        (-13.0, 9.0, -28.0, -10.0),  // North
        (-28.0, -23.0, -28.0, 28.0), // Far west
        (-22.0, -15.0, -28.0, -4.0), // West, north of pool
        (-22.0, -15.0, 4.0, 28.0),   // West, south of pool
    ];

    // Weight zone selection by area
    let areas: [f32; 6] = std::array::from_fn(|i| {
        (zones[i].1 - zones[i].0) * (zones[i].3 - zones[i].2)
    });
    let total: f32 = areas.iter().sum();
    let mut pick = rng.next_f32() * total;

    let mut zone = zones[0];
    for (i, &area) in areas.iter().enumerate() {
        pick -= area;
        if pick <= 0.0 {
            zone = zones[i];
            break;
        }
    }

    let x = rng.range(zone.0, zone.1);
    let z = rng.range(zone.2, zone.3);
    Vec3::new(x, 0.5, z)
}

pub(super) fn apple_bob_system(
    time: Res<Time>,
    mut query: Query<&mut Transform, With<PowerUpApple>>,
) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    for mut transform in &mut query {
        transform.translation.y = 0.5 + (t * 2.0).sin() * 0.15;
        transform.rotate_y(dt * 1.0);
    }
}

pub(super) fn apple_collection_system(
    mut commands: Commands,
    player_q: Query<&Transform, With<Player>>,
    apple_q: Query<(Entity, &Transform, &PowerUpApple), Without<Player>>,
    mut power_ups: ResMut<ActivePowerUps>,
    power_up_state: Option<ResMut<PowerUpState>>,
) {
    let Ok(player_tf) = player_q.get_single() else { return };
    let pp = player_tf.translation;

    let mut any_collected = false;
    for (entity, apple_tf, apple) in &apple_q {
        let dist = pp.distance(apple_tf.translation);
        if dist < PICKUP_RADIUS {
            commands.entity(entity).despawn_recursive();
            match apple.kind {
                AppleKind::Speed => power_ups.speed_timer = 60.0,
                AppleKind::Jump => power_ups.jump_timer = 60.0,
                AppleKind::Backwards => power_ups.backwards_timer = 60.0,
            }
            power_ups.respawn_timers.push((apple.kind, 60.0));
            any_collected = true;
        }
    }
    if any_collected {
        if let Some(mut state) = power_up_state {
            state.speed_multiplier = if power_ups.speed_timer > 0.0 { 2.0 } else { 0.0 };
            state.jump_multiplier = if power_ups.jump_timer > 0.0 { 2.0 } else { 0.0 };
            state.reverse_facing = power_ups.backwards_timer > 0.0;
        }
    }
}

pub(super) fn power_up_timer_system(
    mut commands: Commands,
    time: Res<Time>,
    mut power_ups: ResMut<ActivePowerUps>,
    apple_assets: Option<Res<AppleAssets>>,
    mut apple_rng: ResMut<AppleRng>,
    power_up_state: Option<ResMut<PowerUpState>>,
) {
    let dt = time.delta_secs();
    let mut changed = false;

    if power_ups.speed_timer > 0.0 {
        power_ups.speed_timer = (power_ups.speed_timer - dt).max(0.0);
        if power_ups.speed_timer == 0.0 { changed = true; }
    }
    if power_ups.jump_timer > 0.0 {
        power_ups.jump_timer = (power_ups.jump_timer - dt).max(0.0);
        if power_ups.jump_timer == 0.0 { changed = true; }
    }
    if power_ups.backwards_timer > 0.0 {
        power_ups.backwards_timer = (power_ups.backwards_timer - dt).max(0.0);
        if power_ups.backwards_timer == 0.0 { changed = true; }
    }
    if changed {
        if let Some(mut state) = power_up_state {
            state.speed_multiplier = if power_ups.speed_timer > 0.0 { 2.0 } else { 0.0 };
            state.jump_multiplier = if power_ups.jump_timer > 0.0 { 2.0 } else { 0.0 };
            state.reverse_facing = power_ups.backwards_timer > 0.0;
        }
    }

    // Tick respawn timers and spawn apples when ready
    let Some(assets) = apple_assets else { return };
    let mut i = 0;
    while i < power_ups.respawn_timers.len() {
        power_ups.respawn_timers[i].1 -= dt;
        if power_ups.respawn_timers[i].1 <= 0.0 {
            let (kind, _) = power_ups.respawn_timers.swap_remove(i);
            let pos = random_apple_pos(&mut apple_rng);
            let mat = match kind {
                AppleKind::Speed => assets.green.clone(),
                AppleKind::Jump => assets.red.clone(),
                AppleKind::Backwards => assets.purple.clone(),
            };
            commands
                .spawn((
                    Transform::from_translation(pos),
                    Visibility::default(),
                    PowerUpApple { kind },
                    HillEntity,
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Mesh3d(assets.sphere.clone()),
                        MeshMaterial3d(mat),
                        Transform::IDENTITY,
                    ));
                    parent.spawn((
                        Mesh3d(assets.stem.clone()),
                        MeshMaterial3d(assets.stem_mat.clone()),
                        Transform::from_xyz(0.0, 0.45, 0.0),
                    ));
                });
        } else {
            i += 1;
        }
    }
}

fn timer_for_kind(power_ups: &ActivePowerUps, kind: AppleKind) -> f32 {
    match kind {
        AppleKind::Speed => power_ups.speed_timer,
        AppleKind::Jump => power_ups.jump_timer,
        AppleKind::Backwards => power_ups.backwards_timer,
    }
}

pub(super) fn power_up_bar_ui_system(
    power_ups: Res<ActivePowerUps>,
    mut bar_q: Query<(&PowerUpBar, &mut Node)>,
    mut bg_q: Query<(&PowerUpBarBg, &mut Node), Without<PowerUpBar>>,
) {
    const MAX_WIDTH: f32 = 200.0;

    for (bar, mut node) in &mut bar_q {
        let remaining = timer_for_kind(&power_ups, bar.kind);
        node.width = Val::Px(remaining / 60.0 * MAX_WIDTH);
    }

    for (bg, mut node) in &mut bg_q {
        let remaining = timer_for_kind(&power_ups, bg.kind);
        node.display = if remaining > 0.0 { Display::Flex } else { Display::None };
    }
}
