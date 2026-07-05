use bevy::prelude::*;

use crate::player::{
    animate_player, escape_to_menu, player_movement, spawn_player, toggle_pause, GroundYOverride,
    MovementBounds, Player, PlayerMovementSet,
};
use crate::shared_ui;
use crate::terrain::{terrain_collision, ColumnPushout, TerrainConfig, TerrainSurface};
use crate::{MeadowPhase, Screen, Scoreboard};

pub struct Level102Plugin;

impl Plugin for Level102Plugin {
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

#[derive(Component, Clone, Copy)]
struct MeadowEntity;

#[derive(Component)]
struct TerrainCell {
    cx: f32,
    cz: f32,
    current_h: f32,
    target_h: f32,
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

#[derive(Component)]
struct DeepPitBeacon;

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
    deep_pit_pos: Vec2,
}

// --- Constants ---

const PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 0.0, 35.0);
const CAM_OFFSET: Vec3 = Vec3::new(0.0, 22.0, 18.0);
const AREA_HALF: f32 = 40.0;
const CELL: f32 = 2.0;
const GRID: i32 = 40;
const Y_BASE: f32 = -8.0;
const COLUMN_THICKNESS: f32 = 4.0;
const STEP_LIMIT: f32 = 1.5;
const VOID_HR: f32 = -50.0;
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
    base_r: f32,
    near_factor: f32, // along -dir (steep side)
    far_factor: f32,  // along +dir (gentle, climbable side)
    perp_factor: f32, // perpendicular radius factor
    dir: Vec2,        // unit vector toward the gentle side
    height: f32,
}

fn height_at(x: f32, z: f32, features: &[TerrainFeature]) -> f32 {
    let mut h = 0.0_f32;
    for f in features {
        let off = Vec2::new(x - f.cx, z - f.cz);
        let along = off.dot(f.dir);
        let perp_vec = off - f.dir * along;
        let r_along = if along >= 0.0 {
            f.base_r * f.far_factor
        } else {
            f.base_r * f.near_factor
        };
        let r_perp = f.base_r * f.perp_factor;
        let dx = along / r_along;
        let dy = perp_vec.length() / r_perp;
        let d2 = dx * dx + dy * dy;
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

fn random_unit_dir(rng: &mut u64) -> Vec2 {
    let angle = rng_f32(rng) * std::f32::consts::TAU;
    Vec2::new(angle.cos(), angle.sin())
}

fn rotate(v: Vec2, angle: f32) -> Vec2 {
    let (s, c) = angle.sin_cos();
    Vec2::new(v.x * c - v.y * s, v.x * s + v.y * c)
}

/// Returns (features, deep_pit_position). The deep pit is the last entry
/// in `features` and is the round's underground climb-out point.
fn random_features(rng: &mut u64) -> (Vec<TerrainFeature>, Vec2) {
    let mut feats = Vec::new();

    // Main hill: tallest, gentle side biased toward player spawn so the flag
    // is reachable from the natural starting approach.
    let main_h = rng_range(rng, 4.0, 6.0);
    let main_r = main_h * 2.5;
    let main_cx = rng_range(rng, -22.0, 22.0);
    let main_cz = rng_range(rng, -22.0, 22.0);
    let toward_spawn = (Vec2::new(PLAYER_SPAWN.x, PLAYER_SPAWN.z) - Vec2::new(main_cx, main_cz))
        .normalize_or_zero();
    let jitter = rng_range(rng, -0.52, 0.52); // ±30°
    let main_dir = rotate(toward_spawn, jitter);
    feats.push(TerrainFeature {
        cx: main_cx,
        cz: main_cz,
        base_r: main_r,
        near_factor: 0.5,
        far_factor: 2.5,
        perp_factor: 0.9,
        dir: if main_dir.length_squared() > 0.01 { main_dir.normalize() } else { Vec2::Y },
        height: main_h,
    });

    // 4-6 smaller asymmetric hills.
    for _ in 0..rng_usize(rng, 4, 6) {
        let h = rng_range(rng, 1.5, 3.5);
        feats.push(TerrainFeature {
            cx: rng_range(rng, -32.0, 32.0),
            cz: rng_range(rng, -32.0, 32.0),
            base_r: h * 2.5,
            near_factor: rng_range(rng, 0.4, 0.6),
            far_factor: rng_range(rng, 2.2, 2.8),
            perp_factor: rng_range(rng, 0.8, 1.0),
            dir: random_unit_dir(rng),
            height: h,
        });
    }

    // 2-3 surface pits (mud).
    for _ in 0..rng_usize(rng, 2, 3) {
        let h = rng_range(rng, 2.0, 3.5);
        feats.push(TerrainFeature {
            cx: rng_range(rng, -30.0, 30.0),
            cz: rng_range(rng, -30.0, 30.0),
            base_r: h * 2.5,
            near_factor: rng_range(rng, 0.4, 0.6),
            far_factor: rng_range(rng, 2.2, 2.8),
            perp_factor: rng_range(rng, 0.8, 1.0),
            dir: random_unit_dir(rng),
            height: -h,
        });
    }

    // Deep pit: bottom touches the underground floor; gentle slope is the
    // climb-out from underground. Place away from main hill so the climb-out
    // and the goal don't collide.
    let mut deep_cx;
    let mut deep_cz;
    loop {
        deep_cx = rng_range(rng, -28.0, 28.0);
        deep_cz = rng_range(rng, -28.0, 28.0);
        let dist = ((deep_cx - main_cx).powi(2) + (deep_cz - main_cz).powi(2)).sqrt();
        if dist > 25.0 { break; }
    }
    feats.push(TerrainFeature {
        cx: deep_cx,
        cz: deep_cz,
        base_r: 8.0,
        near_factor: 0.5,
        far_factor: 2.5,
        perp_factor: 1.0,
        dir: random_unit_dir(rng),
        height: -8.0,
    });

    (feats, Vec2::new(deep_cx, deep_cz))
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

/// Top-anchored slab geometry. Cells render at most COLUMN_THICKNESS thick,
/// anchored at their top. The tail clamp keeps a 0.2 visible thickness when
/// the column would otherwise collapse — matching CLAUDE.md's rule that
/// `TerrainSurface.y` and the visible mesh top must come from the same value.
/// Void cells (target_h <= VOID_HR + 1.0) render at their target_h with a
/// 0.2 thickness, deep below the underground floor where they're occluded.
fn cell_geometry(hr: f32) -> (f32 /* visual_top */, f32 /* col_h */, f32 /* center_y */) {
    if hr <= VOID_HR + 1.0 {
        // Void: keep visual_top wherever hr says, super-thin slab below floor.
        let col_h = 0.2;
        let visual_top = hr;
        let center_y = visual_top - col_h / 2.0;
        return (visual_top, col_h, center_y);
    }
    let visual_top = hr.max(Y_BASE + 0.2);
    let col_h = COLUMN_THICKNESS.min(visual_top - Y_BASE).max(0.2);
    let center_y = visual_top - col_h / 2.0;
    (visual_top, col_h, center_y)
}

fn cell_center(gx: i32, gz: i32) -> Vec2 {
    Vec2::new(
        -AREA_HALF + CELL / 2.0 + gx as f32 * CELL,
        -AREA_HALF + CELL / 2.0 + gz as f32 * CELL,
    )
}

/// Select 3-5 pairs of grid indices to be pit-hole top-left corners. Each
/// chosen position becomes a 2x2 hole. Filters keep holes on flat ground,
/// away from goal/spawn/deep pit, and not adjacent to each other.
fn select_pit_holes(
    heights: &[f32],
    rng: &mut u64,
    goal_pos: Vec2,
    deep_pit_pos: Vec2,
    spawn_xz: Vec2,
) -> Vec<(i32, i32)> {
    let grid = GRID as usize;
    let h_at = |gx: i32, gz: i32| -> f32 {
        if gx < 0 || gx >= GRID || gz < 0 || gz >= GRID {
            return f32::NAN;
        }
        heights[gz as usize * grid + gx as usize]
    };

    let mut candidates: Vec<(i32, i32)> = Vec::new();
    for gz in 1..GRID - 2 {
        for gx in 1..GRID - 2 {
            // 2x2 block + 1-cell flat buffer ring => 4x4 area must be near 0.
            let mut flat = true;
            'outer: for dz in -1..=2 {
                for dx in -1..=2 {
                    let h = h_at(gx + dx, gz + dz);
                    if !h.is_finite() || h.abs() > STEP_LIMIT {
                        flat = false;
                        break 'outer;
                    }
                }
            }
            if !flat { continue; }

            // 2x2 center is between (gx, gz) and (gx+1, gz+1)
            let center = (cell_center(gx, gz) + cell_center(gx + 1, gz + 1)) * 0.5;
            if (center - goal_pos).length() < 8.0 { continue; }
            if (center - deep_pit_pos).length() < 12.0 { continue; }
            if (center - spawn_xz).length() < 10.0 { continue; }

            candidates.push((gx, gz));
        }
    }

    let target = rng_usize(rng, 3, 5);
    let mut selected: Vec<(i32, i32)> = Vec::new();
    while selected.len() < target && !candidates.is_empty() {
        let i = (rng_next(rng) as usize) % candidates.len();
        let pick = candidates.swap_remove(i);
        // Keep holes apart so they don't merge into one big drop zone.
        let pick_center = (cell_center(pick.0, pick.1) + cell_center(pick.0 + 1, pick.1 + 1)) * 0.5;
        let too_close = selected.iter().any(|(sx, sz)| {
            let sc = (cell_center(*sx, *sz) + cell_center(*sx + 1, *sz + 1)) * 0.5;
            (sc - pick_center).length() < 8.0
        });
        if too_close { continue; }
        selected.push(pick);
    }
    selected
}

// --- Setup ---

fn setup_meadow(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(ClearColor(Color::srgb(0.5, 0.78, 0.95)));
    commands.insert_resource(GroundYOverride(Y_BASE));
    commands.insert_resource(TerrainConfig {
        step_up_limit: STEP_LIMIT,
        tolerance_max: 4.0,
        pushout_margin: 0.4,
        column_pushout: Some(ColumnPushout {
            thickness: COLUMN_THICKNESS,
            base_y: Y_BASE,
        }),
        ..TerrainConfig::standard(Y_BASE)
    });

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

    let (feats, deep_pit_pos) = random_features(&mut rng);
    let (peak_x, peak_z, peak_h) = find_peak(&feats);

    // Pre-compute heights per cell so we can run pit-hole filtering on a
    // realized terrain rather than re-evaluating height_at per filter step.
    let grid = GRID as usize;
    let mut heights = vec![0.0_f32; grid * grid];
    for gz in 0..GRID {
        for gx in 0..GRID {
            let pos = cell_center(gx, gz);
            let h = height_at(pos.x, pos.y, &feats);
            heights[gz as usize * grid + gx as usize] = (h / HEIGHT_STEP).round() * HEIGHT_STEP;
        }
    }

    let pit_holes = select_pit_holes(
        &heights,
        &mut rng,
        Vec2::new(peak_x, peak_z),
        deep_pit_pos,
        Vec2::new(PLAYER_SPAWN.x, PLAYER_SPAWN.z),
    );

    commands.insert_resource(MeadowState {
        rng,
        round: 1,
        transitioning: false,
        needs_regen: false,
        elapsed: 0.0,
        wave_origin: PLAYER_SPAWN,
        goal_pos: Vec2::new(peak_x, peak_z),
        goal_peak: peak_h,
        deep_pit_pos,
    });

    // Shared unit mesh for all terrain cells
    let unit_mesh = meshes.add(Cuboid::new(CELL, 1.0, CELL));

    // Stamp pit holes onto the height grid (the 2x2 anchored at each pick).
    let is_pit_hole = |gx: i32, gz: i32| -> bool {
        pit_holes.iter().any(|(px, pz)| {
            (gx == *px || gx == *px + 1) && (gz == *pz || gz == *pz + 1)
        })
    };

    for gz in 0..GRID {
        for gx in 0..GRID {
            let pos = cell_center(gx, gz);
            let hr = if is_pit_hole(gx, gz) {
                VOID_HR
            } else {
                heights[gz as usize * grid + gx as usize]
            };
            let (visual_top, col_h, center_y) = cell_geometry(hr);

            commands.spawn((
                Mesh3d(unit_mesh.clone()),
                MeshMaterial3d(bands[height_band(hr)].clone()),
                Transform::from_xyz(pos.x, center_y, pos.y)
                    .with_scale(Vec3::new(1.0, col_h, 1.0)),
                TerrainSurface {
                    min: Vec2::new(pos.x - CELL / 2.0, pos.y - CELL / 2.0),
                    max: Vec2::new(pos.x + CELL / 2.0, pos.y + CELL / 2.0),
                    y: visual_top,
                },
                TerrainCell { cx: pos.x, cz: pos.y, current_h: hr, target_h: hr },
                MeadowEntity,
            ));
        }
    }

    // Underground floor: a single dirt-colored plane at Y_BASE. Required by
    // CLAUDE.md whenever GroundYOverride sits below visible cells, and
    // serves as the navigable floor when the player drops through a pit hole.
    let underground_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.25, 0.18, 0.12),
        perceptual_roughness: 0.95,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(AREA_HALF * 2.0, 0.4, AREA_HALF * 2.0))),
        MeshMaterial3d(underground_mat),
        Transform::from_xyz(0.0, Y_BASE - 0.2, 0.0),
        MeadowEntity,
    ));

    // Cyan beacon over the deep pit — visible from underground so the player
    // knows where to head to climb out.
    let beacon_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.0, 0.9, 1.0),
        emissive: LinearRgba::new(0.0, 4.0, 5.0, 1.0),
        unlit: true,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.6, (Y_BASE.abs()) + 1.0, 0.6))),
        MeshMaterial3d(beacon_mat),
        Transform::from_xyz(deep_pit_pos.x, Y_BASE / 2.0 + 0.5, deep_pit_pos.y),
        DeepPitBeacon,
        MeadowEntity,
    ));

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
    shared_ui::spawn_controls_hint(&mut commands, "Reach the flag", MeadowEntity);
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
            scoreboard.set_solved(102);
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
    player_q: Query<&Transform, (With<Player>, Without<TerrainCell>, Without<FlagEntity>, Without<DeepPitBeacon>)>,
    mut cells: Query<
        (&mut Transform, &mut TerrainSurface, &mut TerrainCell, &mut MeshMaterial3d<StandardMaterial>),
        (Without<FlagEntity>, Without<Player>, Without<DeepPitBeacon>),
    >,
    mut flag_q: Query<(&mut Transform, &FlagEntity), (Without<TerrainCell>, Without<Player>, Without<DeepPitBeacon>)>,
    mut goal_q: Query<&mut GoalZone>,
    mut round_text: Query<&mut Text, With<RoundText>>,
    celeb_q: Query<Entity, With<CelebrationText>>,
    mut beacon_q: Query<&mut Transform, (With<DeepPitBeacon>, Without<TerrainCell>, Without<Player>, Without<FlagEntity>)>,
) {
    // Handle regeneration request
    if state.needs_regen {
        state.needs_regen = false;
        state.transitioning = true;
        state.elapsed = 0.0;

        if let Ok(ptf) = player_q.get_single() {
            state.wave_origin = ptf.translation;
        }

        let (feats, deep_pit_pos) = random_features(&mut state.rng);
        let (px, pz, ph) = find_peak(&feats);

        // Pre-compute heights for pit-hole filtering.
        let grid = GRID as usize;
        let mut heights = vec![0.0_f32; grid * grid];
        for gz in 0..GRID {
            for gx in 0..GRID {
                let pos = cell_center(gx, gz);
                let h = height_at(pos.x, pos.y, &feats);
                heights[gz as usize * grid + gx as usize] = (h / HEIGHT_STEP).round() * HEIGHT_STEP;
            }
        }

        let pit_holes = select_pit_holes(
            &heights,
            &mut state.rng,
            Vec2::new(px, pz),
            deep_pit_pos,
            Vec2::new(PLAYER_SPAWN.x, PLAYER_SPAWN.z),
        );
        let is_pit_hole = |gx: i32, gz: i32| -> bool {
            pit_holes.iter().any(|(p_gx, p_gz)| {
                (gx == *p_gx || gx == *p_gx + 1) && (gz == *p_gz || gz == *p_gz + 1)
            })
        };

        // Set new targets per cell — pit holes get the void sentinel so the
        // morph drops them below the underground floor where they vanish.
        for (_, _, mut cell, _) in &mut cells {
            let gx = ((cell.cx + AREA_HALF - CELL / 2.0) / CELL).round() as i32;
            let gz = ((cell.cz + AREA_HALF - CELL / 2.0) / CELL).round() as i32;
            let target = if is_pit_hole(gx, gz) {
                VOID_HR
            } else {
                heights[gz as usize * grid + gx as usize]
            };
            cell.target_h = target;
        }

        // Update goal & deep pit position
        state.goal_pos = Vec2::new(px, pz);
        state.goal_peak = ph;
        state.deep_pit_pos = deep_pit_pos;
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
        let (visual_top, col_h, center_y) = cell_geometry(hr);

        tf.translation.y = center_y;
        tf.scale.y = col_h;
        surface.y = visual_top;
        // Use the band of the *clamped* height so void cells use a dirt color.
        let band_h = if hr <= VOID_HR + 1.0 { Y_BASE } else { hr };
        material.0 = mats.bands[height_band(band_h)].clone();

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

    // Slide the cyan beacon to the new deep pit position (rides during transition)
    if let Ok(mut tf) = beacon_q.get_single_mut() {
        tf.translation.x = state.deep_pit_pos.x;
        tf.translation.z = state.deep_pit_pos.y;
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
    commands.remove_resource::<TerrainConfig>();
    commands.remove_resource::<MeadowState>();
    commands.remove_resource::<TerrainMaterials>();
}
