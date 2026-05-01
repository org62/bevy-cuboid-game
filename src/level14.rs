use bevy::prelude::*;

use crate::player::{
    animate_player, escape_to_menu, player_movement, spawn_player, toggle_pause, GroundYOverride,
    MovementBounds, Player, PlayerMovementSet, PlayerPhysics, SquashState,
};
use crate::shared_ui;
use crate::{MeadowPhase, Screen, Scoreboard};

pub struct Level14Plugin;

impl Plugin for Level14Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::MeadowChallenge), setup_meadow)
            .add_systems(
                Update,
                (
                    shared_ui::update_camera_orbit.before(PlayerMovementSet),
                    player_movement.in_set(PlayerMovementSet),
                    terrain_collision,
                    (animate_player, shared_ui::follow_camera_system),
                    goal_check,
                    terrain_transition_system,
                )
                    .chain()
                    .run_if(in_state(MeadowPhase::Playing)),
            )
            .add_systems(
                Update,
                (escape_to_menu, toggle_pause).run_if(in_state(MeadowPhase::Playing)),
            )
            .add_systems(OnExit(Screen::MeadowChallenge), cleanup_meadow);
    }
}

// --- Components ---

#[derive(Component)]
struct MeadowEntity;

#[derive(Component)]
struct TerrainCell {
    cx: f32,
    cz: f32,
    current_h: f32,
    target_h: f32,
}

#[derive(Component)]
struct TerrainSurface {
    min: Vec2,
    max: Vec2,
    y: f32,
}

#[derive(Component)]
struct GoalZone {
    min: Vec2,
    max: Vec2,
    y: f32,
}

#[derive(Component)]
struct FlagEntity {
    y_above: f32,
    x_offset: f32,
}

#[derive(Component)]
struct RoundText;

#[derive(Component)]
struct CelebrationText;

// --- Resources ---

#[derive(Resource)]
struct TerrainMaterials {
    bands: [Handle<StandardMaterial>; 6],
}

#[derive(Resource)]
struct MeadowState {
    rng: u64,
    round: u32,
    transitioning: bool,
    needs_regen: bool,
    elapsed: f32,
    wave_origin: Vec3,
    goal_pos: Vec2,
    goal_peak: f32,
}

// --- Constants ---

const PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 0.0, 35.0);
const CAM_OFFSET: Vec3 = Vec3::new(0.0, 22.0, 18.0);
const AREA_HALF: f32 = 40.0;
const CELL: f32 = 2.0;
const GRID: i32 = 40;
const Y_BASE: f32 = -4.0;
const HEIGHT_STEP: f32 = 0.2;
const WAVE_SPREAD: f32 = 1.5;
const MORPH_TIME: f32 = 1.5;
const TOTAL_TRANSITION: f32 = WAVE_SPREAD + MORPH_TIME + 0.5;

// --- RNG ---

fn rng_next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn rng_f32(state: &mut u64) -> f32 {
    (rng_next(state) as u32 as f32) / (u32::MAX as f32)
}

fn rng_range(state: &mut u64, min: f32, max: f32) -> f32 {
    min + rng_f32(state) * (max - min)
}

fn rng_usize(state: &mut u64, min: usize, max: usize) -> usize {
    min + (rng_next(state) as usize % (max - min + 1))
}

// --- Terrain math ---

struct TerrainFeature {
    cx: f32,
    cz: f32,
    rx: f32,
    rz: f32,
    height: f32,
}

fn height_at(x: f32, z: f32, features: &[TerrainFeature]) -> f32 {
    let mut h = 0.0_f32;
    for f in features {
        let dx = (x - f.cx) / f.rx;
        let dz = (z - f.cz) / f.rz;
        let d2 = dx * dx + dz * dz;
        if d2 < 1.0 {
            let t = d2.sqrt();
            h += f.height * 0.5 * (1.0 + (t * std::f32::consts::PI).cos());
        }
    }
    h
}

fn height_band(h: f32) -> usize {
    if h < -1.0 { 0 }
    else if h < -0.2 { 1 }
    else if h < 0.4 { 2 }
    else if h < 1.2 { 3 }
    else if h < 2.2 { 4 }
    else { 5 }
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn random_features(rng: &mut u64) -> Vec<TerrainFeature> {
    let mut feats = Vec::new();
    // Main hill (tallest)
    let h = rng_range(rng, 3.0, 4.0);
    let r = h * 6.5 + rng_range(rng, 2.0, 6.0);
    feats.push(TerrainFeature {
        cx: rng_range(rng, -25.0, 25.0),
        cz: rng_range(rng, -25.0, 25.0),
        rx: r * rng_range(rng, 0.85, 1.15),
        rz: r * rng_range(rng, 0.85, 1.15),
        height: h,
    });
    // 4-6 smaller hills
    for _ in 0..rng_usize(rng, 4, 6) {
        let h = rng_range(rng, 0.6, 2.5);
        let r = h * 6.5 + rng_range(rng, 2.0, 10.0);
        feats.push(TerrainFeature {
            cx: rng_range(rng, -32.0, 32.0),
            cz: rng_range(rng, -32.0, 32.0),
            rx: r * rng_range(rng, 0.75, 1.3),
            rz: r * rng_range(rng, 0.75, 1.3),
            height: h,
        });
    }
    // 2-3 pits
    for _ in 0..rng_usize(rng, 2, 3) {
        let h = rng_range(rng, 0.8, 2.0);
        let r = h * 6.5 + rng_range(rng, 2.0, 8.0);
        feats.push(TerrainFeature {
            cx: rng_range(rng, -30.0, 30.0),
            cz: rng_range(rng, -30.0, 30.0),
            rx: r * rng_range(rng, 0.8, 1.2),
            rz: r * rng_range(rng, 0.8, 1.2),
            height: -h,
        });
    }
    feats
}

fn find_peak(feats: &[TerrainFeature]) -> (f32, f32, f32) {
    let mut best = (0.0_f32, 0.0_f32, f32::NEG_INFINITY);
    for gz in 0..GRID {
        for gx in 0..GRID {
            let cx = -AREA_HALF + CELL / 2.0 + gx as f32 * CELL;
            let cz = -AREA_HALF + CELL / 2.0 + gz as f32 * CELL;
            let h = height_at(cx, cz, feats);
            if h > best.2 {
                best = (cx, cz, h);
            }
        }
    }
    best.2 = (best.2 / HEIGHT_STEP).round() * HEIGHT_STEP;
    best
}

// --- Setup ---

fn setup_meadow(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(ClearColor(Color::srgb(0.5, 0.78, 0.95)));
    commands.insert_resource(GroundYOverride(-5.0));

    let bands: [Handle<StandardMaterial>; 6] = [
        materials.add(StandardMaterial { base_color: Color::srgb(0.4, 0.28, 0.15), ..default() }),
        materials.add(StandardMaterial { base_color: Color::srgb(0.55, 0.45, 0.28), ..default() }),
        materials.add(StandardMaterial { base_color: Color::srgb(0.35, 0.65, 0.2), ..default() }),
        materials.add(StandardMaterial { base_color: Color::srgb(0.4, 0.72, 0.22), ..default() }),
        materials.add(StandardMaterial { base_color: Color::srgb(0.32, 0.62, 0.18), ..default() }),
        materials.add(StandardMaterial { base_color: Color::srgb(0.48, 0.78, 0.3), ..default() }),
    ];
    commands.insert_resource(TerrainMaterials { bands: bands.clone() });

    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let mut rng = seed | 1;

    let feats = random_features(&mut rng);
    let (peak_x, peak_z, peak_h) = find_peak(&feats);

    commands.insert_resource(MeadowState {
        rng,
        round: 1,
        transitioning: false,
        needs_regen: false,
        elapsed: 0.0,
        wave_origin: PLAYER_SPAWN,
        goal_pos: Vec2::new(peak_x, peak_z),
        goal_peak: peak_h,
    });

    // Shared unit mesh for all terrain cells
    let unit_mesh = meshes.add(Cuboid::new(CELL, 1.0, CELL));

    // Generate grid
    for gz in 0..GRID {
        for gx in 0..GRID {
            let cx = -AREA_HALF + CELL / 2.0 + gx as f32 * CELL;
            let cz = -AREA_HALF + CELL / 2.0 + gz as f32 * CELL;
            let h = height_at(cx, cz, &feats);
            let hr = (h / HEIGHT_STEP).round() * HEIGHT_STEP;
            let col_h = (hr - Y_BASE).max(0.2);
            // Collision top must equal visual top. If hr is so low that col_h
            // got clamped, the cell visually sits at Y_BASE + col_h, not hr.
            let visual_top = Y_BASE + col_h;

            commands.spawn((
                Mesh3d(unit_mesh.clone()),
                MeshMaterial3d(bands[height_band(hr)].clone()),
                Transform::from_xyz(cx, Y_BASE + col_h / 2.0, cz)
                    .with_scale(Vec3::new(1.0, col_h, 1.0)),
                TerrainSurface {
                    min: Vec2::new(cx - CELL / 2.0, cz - CELL / 2.0),
                    max: Vec2::new(cx + CELL / 2.0, cz + CELL / 2.0),
                    y: visual_top,
                },
                TerrainCell { cx, cz, current_h: hr, target_h: hr },
                MeadowEntity,
            ));
        }
    }

    // Flag
    let brown = materials.add(StandardMaterial { base_color: Color::srgb(0.55, 0.35, 0.2), ..default() });
    let gold = materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.85, 0.0), ..default() });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.1, 3.5, 0.1))),
        MeshMaterial3d(brown),
        Transform::from_xyz(peak_x, peak_h + 1.75, peak_z),
        FlagEntity { y_above: 1.75, x_offset: 0.0 },
        MeadowEntity,
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.2, 0.7, 0.05))),
        MeshMaterial3d(gold),
        Transform::from_xyz(peak_x + 0.6, peak_h + 3.0, peak_z),
        FlagEntity { y_above: 3.0, x_offset: 0.6 },
        MeadowEntity,
    ));

    // Goal zone
    commands.spawn((
        GoalZone {
            min: Vec2::new(peak_x - 2.5, peak_z - 2.5),
            max: Vec2::new(peak_x + 2.5, peak_z + 2.5),
            y: peak_h - 0.5,
        },
        MeadowEntity,
    ));

    // Trees at edges (outside feature range)
    let dark_green = materials.add(StandardMaterial { base_color: Color::srgb(0.15, 0.4, 0.1), ..default() });
    let tree_brown = materials.add(StandardMaterial { base_color: Color::srgb(0.55, 0.35, 0.2), ..default() });
    let trunk = meshes.add(Cuboid::new(0.4, 2.5, 0.4));
    let canopy = meshes.add(Cuboid::new(2.5, 2.5, 2.5));
    for pos in [
        Vec2::new(-37.0, -37.0), Vec2::new(37.0, -37.0),
        Vec2::new(-37.0, 37.0),  Vec2::new(37.0, 37.0),
        Vec2::new(-37.0, -12.0), Vec2::new(-37.0, 12.0),
        Vec2::new(37.0, -12.0),  Vec2::new(37.0, 12.0),
        Vec2::new(-12.0, -37.0), Vec2::new(12.0, -37.0),
        Vec2::new(-12.0, 37.0),  Vec2::new(12.0, 37.0),
    ] {
        commands.spawn((
            Mesh3d(trunk.clone()), MeshMaterial3d(tree_brown.clone()),
            Transform::from_xyz(pos.x, 1.25, pos.y), MeadowEntity,
        ));
        commands.spawn((
            Mesh3d(canopy.clone()), MeshMaterial3d(dark_green.clone()),
            Transform::from_xyz(pos.x, 3.75, pos.y), MeadowEntity,
        ));
    }

    // Boundary fences
    let fence_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.4, 0.25, 0.15), ..default() });
    for (pos, size) in [
        (Vec3::new(0.0, 1.0, -AREA_HALF), Vec3::new(AREA_HALF * 2.0, 2.0, 0.3)),
        (Vec3::new(0.0, 1.0, AREA_HALF),  Vec3::new(AREA_HALF * 2.0, 2.0, 0.3)),
        (Vec3::new(AREA_HALF, 1.0, 0.0),  Vec3::new(0.3, 2.0, AREA_HALF * 2.0)),
        (Vec3::new(-AREA_HALF, 1.0, 0.0), Vec3::new(0.3, 2.0, AREA_HALF * 2.0)),
    ] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
            MeshMaterial3d(fence_mat.clone()),
            Transform::from_translation(pos), MeadowEntity,
        ));
    }

    // Lighting
    shared_ui::setup_level_lighting(&mut commands, 12000.0, (-0.8, 0.3, 0.0), Color::WHITE, 500.0, MeadowEntity);

    // Player
    let player = spawn_player(&mut commands, &mut meshes, &mut materials);
    commands.entity(player).insert((
        Transform::from_xyz(PLAYER_SPAWN.x, PLAYER_SPAWN.y, PLAYER_SPAWN.z)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        MovementBounds { rects: vec![(
            Vec2::new(-AREA_HALF + 0.5, -AREA_HALF + 0.5),
            Vec2::new(AREA_HALF - 0.5, AREA_HALF - 0.5),
        )] },
        MeadowEntity,
    ));

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(PLAYER_SPAWN.x + CAM_OFFSET.x, PLAYER_SPAWN.y + CAM_OFFSET.y, PLAYER_SPAWN.z + CAM_OFFSET.z)
            .looking_at(PLAYER_SPAWN, Vec3::Y),
        shared_ui::FollowCamera { offset: CAM_OFFSET, lerp_speed: 10.0, look_offset: Vec3::ZERO },
        MeadowEntity,
    ));

    // HUD
    commands.spawn((
        Node { position_type: PositionType::Absolute, left: Val::Px(12.0), top: Val::Px(12.0),
               flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() },
        MeadowEntity,
    )).with_children(|p| {
        p.spawn((
            Text::new("Level 14: The Rolling Meadow"),
            TextFont { font_size: 26.0, ..default() }, TextColor(Color::WHITE), MeadowEntity,
        ));
        p.spawn((
            Text::new("Round 1 — Reach the flag!"),
            TextFont { font_size: 18.0, ..default() },
            TextColor(Color::srgb(0.9, 0.9, 0.5)), RoundText, MeadowEntity,
        ));
    });
    shared_ui::spawn_controls_hint(&mut commands, "[ESC] Menu  |  [WASD] Move  |  [Space] Jump  |  [P] Pause", MeadowEntity);
}

// --- Terrain collision ---

fn terrain_collision(
    surfaces: Query<&TerrainSurface>,
    mut player_q: Query<(&mut Transform, &mut PlayerPhysics, &mut SquashState), With<Player>>,
    time: Res<Time>,
) {
    let Ok((mut transform, mut physics, mut squash)) = player_q.get_single_mut() else { return };
    let was_airborne = !physics.grounded;
    let px = transform.translation.x;
    let pz = transform.translation.z;
    let dt = time.delta_secs();

    // Previous-frame position. player_movement already advanced the transform
    // by `velocity * dt`, so subtracting it back gives where the player was
    // before this frame's motion. Used for the swept-path test below.
    let prev_x = px - physics.velocity.x * dt;
    let prev_y = transform.translation.y - physics.velocity.y * dt;
    let prev_z = pz - physics.velocity.z * dt;

    // Tolerance: how far above the player we look for surfaces to stand on.
    // When jumping (vy > 0) we use 0 so we don't snap to surfaces above us.
    let tolerance = if physics.velocity.y <= 0.0 {
        (physics.velocity.y.abs() * dt + 1.5).min(4.0)
    } else {
        0.0
    };

    // Surface search.
    //   best_y      — highest surface the player should land on this frame
    //   any_surface — highest surface at player XZ regardless of tolerance (fallback)
    // We consider any surface whose XZ overlaps EITHER the previous or the
    // current player position so that high-speed diagonal motion can't slip
    // between cells without being checked.
    let mut best_y = Y_BASE;
    let mut any_surface = Y_BASE;
    for surf in &surfaces {
        let in_now = px >= surf.min.x && px <= surf.max.x && pz >= surf.min.y && pz <= surf.max.y;
        let in_prev = prev_x >= surf.min.x && prev_x <= surf.max.x
            && prev_z >= surf.min.y && prev_z <= surf.max.y;
        if !(in_now || in_prev) { continue; }

        if in_now && surf.y > any_surface {
            any_surface = surf.y;
        }
        // Standing / phase-through-vertically check (only at current XZ).
        if in_now && surf.y <= transform.translation.y + tolerance && surf.y > best_y {
            best_y = surf.y;
        }
        // Swept check: if the player's y crossed this surface during the frame
        // while moving downward, this surface should catch them — even if their
        // XZ is now over a different cell whose surface is much lower. This is
        // what fixes "player keeps falling past visible surfaces" when running
        // off the edge of taller cells with high horizontal velocity.
        if physics.velocity.y <= 0.0
            && prev_y >= surf.y - 0.05
            && transform.translation.y < surf.y
            && surf.y > best_y
        {
            best_y = surf.y;
        }
    }

    // Fallback: if the normal search found nothing but there IS a surface here,
    // the player has phased below all surfaces. Push them up.
    if best_y <= Y_BASE + 0.1 && any_surface > Y_BASE + 0.1 && physics.velocity.y <= 0.0 {
        best_y = any_surface;
    }

    // Snap fires when:
    //   - player is at/below the surface (normal landing), OR
    //   - swept check shows they crossed the surface during the frame, OR
    //   - step-down: player was statically grounded last frame and is walking
    //     off a small ledge. Without this, fast horizontal motion lets them
    //     float over and tunnel past the next several cells before falling
    //     enough to be caught at the new XZ. Gated on `vy.abs < 0.01` so
    //     active jumps and falls aren't intercepted.
    let crossed = physics.velocity.y <= 0.0 && prev_y >= best_y && transform.translation.y < best_y;
    let static_grounded_last = !was_airborne && physics.velocity.y.abs() < 0.01;
    let step_down = static_grounded_last
        && transform.translation.y > best_y
        && transform.translation.y - best_y <= 1.5
        && best_y > Y_BASE + 0.1;
    if (transform.translation.y <= best_y + 0.1 || crossed || step_down) && physics.velocity.y <= 0.0 {
        transform.translation.y = best_y;
        physics.velocity.y = 0.0;
        if was_airborne {
            squash.timer = 0.2;
        }
        physics.grounded = true;
    } else if transform.translation.y > best_y + 1.5 {
        physics.grounded = false;
    }

    // Smooth slope descent: gently lower when walking downhill.
    // Player is ABOVE the surface here (safe, no clipping).
    if physics.grounded && transform.translation.y > best_y + 0.02 && best_y > Y_BASE + 0.1 {
        transform.translation.y = (transform.translation.y - 15.0 * dt).max(best_y);
    }
}

// --- Goal check ---

fn goal_check(
    mut state: ResMut<MeadowState>,
    player_q: Query<&Transform, With<Player>>,
    goals: Query<&GoalZone>,
    mut scoreboard: ResMut<Scoreboard>,
) {
    if state.transitioning || state.needs_regen { return; }
    let Ok(tf) = player_q.get_single() else { return };
    for goal in &goals {
        if tf.translation.x >= goal.min.x && tf.translation.x <= goal.max.x
            && tf.translation.z >= goal.min.y && tf.translation.z <= goal.max.y
            && tf.translation.y >= goal.y
        {
            scoreboard.set_solved(14);
            state.needs_regen = true;
            return;
        }
    }
}

// --- Terrain transition ---

fn terrain_transition_system(
    mut commands: Commands,
    time: Res<Time>,
    mut state: ResMut<MeadowState>,
    mats: Res<TerrainMaterials>,
    player_q: Query<&Transform, (With<Player>, Without<TerrainCell>, Without<FlagEntity>)>,
    mut cells: Query<
        (&mut Transform, &mut TerrainSurface, &mut TerrainCell, &mut MeshMaterial3d<StandardMaterial>),
        (Without<FlagEntity>, Without<Player>),
    >,
    mut flag_q: Query<(&mut Transform, &FlagEntity), (Without<TerrainCell>, Without<Player>)>,
    mut goal_q: Query<&mut GoalZone>,
    mut round_text: Query<&mut Text, With<RoundText>>,
    celeb_q: Query<Entity, With<CelebrationText>>,
) {
    // Handle regeneration request
    if state.needs_regen {
        state.needs_regen = false;
        state.transitioning = true;
        state.elapsed = 0.0;

        if let Ok(ptf) = player_q.get_single() {
            state.wave_origin = ptf.translation;
        }

        let feats = random_features(&mut state.rng);
        let (px, pz, ph) = find_peak(&feats);

        // Set new targets
        for (_, _, mut cell, _) in &mut cells {
            let new_h = height_at(cell.cx, cell.cz, &feats);
            cell.target_h = (new_h / HEIGHT_STEP).round() * HEIGHT_STEP;
        }

        // Update goal
        state.goal_pos = Vec2::new(px, pz);
        state.goal_peak = ph;
        if let Ok(mut goal) = goal_q.get_single_mut() {
            goal.min = Vec2::new(px - 2.5, pz - 2.5);
            goal.max = Vec2::new(px + 2.5, pz + 2.5);
            goal.y = ph - 0.5;
        }

        // Celebration text
        let completed_round = state.round;
        state.round += 1;
        commands.spawn((
            Node {
                width: Val::Percent(100.0), height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                align_items: AlignItems::Center, justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0),
                ..default()
            },
            CelebrationText, MeadowEntity,
        )).with_children(|p| {
            p.spawn((
                Text::new(format!("Round {} complete!", completed_round)),
                TextFont { font_size: 56.0, ..default() },
                TextColor(Color::srgb(1.0, 0.95, 0.3)),
            ));
            p.spawn((
                Text::new(format!("Round {} incoming...", state.round)),
                TextFont { font_size: 24.0, ..default() },
                TextColor(Color::srgb(0.8, 0.9, 0.6)),
            ));
        });

        // Update round text
        if let Ok(mut text) = round_text.get_single_mut() {
            *text = Text::new(format!("Round {} — Reach the flag!", state.round));
        }
    }

    if !state.transitioning { return; }

    let dt = time.delta_secs();
    state.elapsed += dt;
    let max_dist = (AREA_HALF * 2.0_f32).hypot(AREA_HALF * 2.0);

    // Track terrain height at goal for flag placement
    let mut goal_terrain_h = 0.0_f32;

    // Animate each cell
    for (mut tf, mut surface, cell, mut material) in &mut cells {
        let dist = Vec2::new(cell.cx - state.wave_origin.x, cell.cz - state.wave_origin.z).length();
        let cell_start = (dist / max_dist) * WAVE_SPREAD;
        let cell_t = smoothstep(((state.elapsed - cell_start) / MORPH_TIME).clamp(0.0, 1.0));

        let h = cell.current_h + (cell.target_h - cell.current_h) * cell_t;
        let hr = (h / HEIGHT_STEP).round() * HEIGHT_STEP;
        let col_h = (hr - Y_BASE).max(0.2);
        let visual_top = Y_BASE + col_h;

        tf.translation.y = Y_BASE + col_h / 2.0;
        tf.scale.y = col_h;
        surface.y = visual_top;
        material.0 = mats.bands[height_band(hr)].clone();

        // Track goal cell height
        if (cell.cx - state.goal_pos.x).abs() < CELL * 0.6
            && (cell.cz - state.goal_pos.y).abs() < CELL * 0.6
        {
            goal_terrain_h = visual_top;
        }
    }

    // Move flag to new peak (rides terrain during transition)
    for (mut tf, flag) in &mut flag_q {
        tf.translation.x = state.goal_pos.x + flag.x_offset;
        tf.translation.z = state.goal_pos.y;
        tf.translation.y = goal_terrain_h + flag.y_above;
    }

    // Despawn celebration text after 2.5s
    if state.elapsed > 2.5 {
        for entity in &celeb_q {
            commands.entity(entity).despawn_recursive();
        }
    }

    // Finalize transition
    if state.elapsed >= TOTAL_TRANSITION {
        state.transitioning = false;
        for (_, _, mut cell, _) in &mut cells {
            cell.current_h = cell.target_h;
        }
    }
}

// --- Cleanup ---

fn cleanup_meadow(mut commands: Commands, query: Query<Entity, With<MeadowEntity>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
    commands.remove_resource::<GroundYOverride>();
    commands.remove_resource::<MeadowState>();
    commands.remove_resource::<TerrainMaterials>();
}
