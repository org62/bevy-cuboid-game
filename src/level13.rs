use bevy::prelude::*;

use crate::player::{
    animate_player, escape_to_menu, player_movement, spawn_player, toggle_pause, GroundYOverride,
    JumpBoostMultiplier, MovementBounds, Player, PlayerMovementSet, PlayerPhysics, SquashState,
    ReversePlayerFacing, SpeedBoostMultiplier,
};
use crate::walls::spawn_maze_grid_walls;
use crate::{GamePaused, HillPhase, Screen};

pub struct Level13Plugin;

impl Plugin for Level13Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::HillChallenge), setup_hill)
            .add_systems(
                Update,
                (player_movement.in_set(PlayerMovementSet), terrain_collision, hill_playing_update)
                    .chain()
                    .run_if(in_state(HillPhase::Playing)),
            )
            .add_systems(
                Update,
                (
                    animate_player,
                    slide_force_system,
                    water_slide_system,
                    apple_collection_system,
                    power_up_timer_system,
                    power_up_bar_ui_system,
                    apple_bob_system,
                    maze_exit_check_system,
                    suck_up_animation_system,
                    teleporter_system,
                    maze_rebuild_system,
                )
                    .after(PlayerMovementSet)
                    .run_if(in_state(Screen::HillChallenge)),
            )
            .add_systems(
                Update,
                (escape_to_menu, toggle_pause).run_if(in_state(HillPhase::Playing)),
            )
            .add_systems(
                Update,
                follow_camera
                    .after(PlayerMovementSet)
                    .after(suck_up_animation_system)
                    .run_if(in_state(Screen::HillChallenge)),
            )
            .add_systems(
                Update,
                handle_victory.run_if(in_state(HillPhase::Victory)),
            )
            .add_systems(OnExit(Screen::HillChallenge), cleanup_hill);
    }
}

// --- Components ---

#[derive(Component)]
struct HillEntity;

#[derive(Component)]
struct HillFollowCam;

#[derive(Component)]
struct OverlayScreen;

#[derive(Component)]
struct HudText;

#[derive(Clone, Copy, PartialEq, Eq)]
enum AppleKind {
    Speed,
    Jump,
    Backwards,
}

#[derive(Component)]
struct PowerUpApple {
    kind: AppleKind,
}

#[derive(Component)]
struct PowerUpBarContainer;

#[derive(Component)]
struct PowerUpBar {
    kind: AppleKind,
}

#[derive(Component)]
struct PowerUpBarBg {
    kind: AppleKind,
}

#[derive(Resource, Default)]
struct ActivePowerUps {
    speed_timer: f32,
    jump_timer: f32,
    backwards_timer: f32,
    /// Respawn cooldowns per kind: (kind, remaining_secs)
    respawn_timers: Vec<(AppleKind, f32)>,
}

/// Simple pseudo-random number generator (xorshift64).
#[derive(Resource)]
struct AppleRng {
    state: u64,
}

impl AppleRng {
    fn new() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        Self { state: seed | 1 }
    }

    fn next_f32(&mut self) -> f32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        (self.state as u32 as f32) / (u32::MAX as f32)
    }

    fn range(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }
}

/// Pick a random position on reachable ground, avoiding the hill center and pool.
fn random_apple_pos(rng: &mut AppleRng) -> Vec3 {
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

/// Recursive backtracker maze generation on a 7x7 grid.
/// Returns (h_walls, v_walls) where:
///   h_walls[r][c] = horizontal wall between row r-1 and row r at column c (r=0..8, c=0..7)
///   v_walls[r][c] = vertical wall between col c-1 and col c at row r (r=0..7, c=0..8)
fn generate_maze_grid(rng: &mut AppleRng) -> ([[bool; 7]; 8], [[bool; 8]; 7]) {
    let mut h_walls = [[true; 7]; 8];
    let mut v_walls = [[true; 8]; 7];
    let mut visited = [[false; 7]; 7];

    // Start from cell (6, 3) — south-center, near entrance
    let mut stack: Vec<(usize, usize)> = Vec::new();
    visited[6][3] = true;
    stack.push((6, 3));

    while let Some(&(r, c)) = stack.last() {
        // Collect unvisited neighbors
        let mut neighbors = Vec::new();
        if r > 0 && !visited[r - 1][c] {
            neighbors.push((r - 1, c, 0)); // north
        }
        if r < 6 && !visited[r + 1][c] {
            neighbors.push((r + 1, c, 1)); // south
        }
        if c > 0 && !visited[r][c - 1] {
            neighbors.push((r, c - 1, 2)); // west
        }
        if c < 6 && !visited[r][c + 1] {
            neighbors.push((r, c + 1, 3)); // east
        }

        if neighbors.is_empty() {
            stack.pop();
        } else {
            let idx = (rng.next_f32() * neighbors.len() as f32) as usize;
            let idx = idx.min(neighbors.len() - 1);
            let (nr, nc, dir) = neighbors[idx];
            // Carve wall between current and neighbor
            match dir {
                0 => h_walls[r][c] = false,     // north wall of current cell
                1 => h_walls[r + 1][c] = false,  // south wall of current cell
                2 => v_walls[r][c] = false,       // west wall of current cell
                3 => v_walls[r][c + 1] = false,   // east wall of current cell
                _ => unreachable!(),
            }
            visited[nr][nc] = true;
            stack.push((nr, nc));
        }
    }

    (h_walls, v_walls)
}

fn spawn_maze_interior(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    rng: &mut AppleRng,
) {
    let stone = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.5, 0.5),
        ..default()
    });
    let wall_h = 2.5;

    let (mut h_walls, v_walls) = generate_maze_grid(rng);

    // Carve entrance gap in south boundary (row 7) at column 3 (x=20..22)
    h_walls[7][3] = false;

    // Spawn ALL walls: boundary + interior + corner posts (zero overlap, zero gaps)
    spawn_maze_grid_walls(
        commands,
        meshes,
        &stone,
        Vec2::new(14.0, -28.0),
        2.0,
        0.4,
        wall_h,
        &h_walls,
        &v_walls,
        |cmds, e, min, max| {
            cmds.entity(e).insert((
                SolidBlock {
                    min,
                    max,
                    y_min: 0.0,
                    y_max: wall_h,
                },
                HillEntity,
                MazeInteriorWall,
            ));
        },
    );

    // Maze exit trigger zone at cell (0,6): NE corner
    commands.spawn((
        MazeExitZone {
            min: Vec2::new(26.0, -28.0),
            max: Vec2::new(28.0, -26.0),
        },
        HillEntity,
        MazeInteriorWall,
    ));

    // Exit glow indicator: green emissive floor tile
    let exit_glow = materials.add(StandardMaterial {
        base_color: Color::srgb(0.0, 0.8, 0.0),
        emissive: LinearRgba::new(0.0, 2.0, 0.0, 1.0),
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0, 0.05, 2.0))),
        MeshMaterial3d(exit_glow),
        Transform::from_xyz(27.0, 0.025, -27.0),
        HillEntity,
        MazeInteriorWall,
    ));
}

#[derive(Resource)]
struct AppleAssets {
    sphere: Handle<Mesh>,
    stem: Handle<Mesh>,
    green: Handle<StandardMaterial>,
    red: Handle<StandardMaterial>,
    purple: Handle<StandardMaterial>,
    stem_mat: Handle<StandardMaterial>,
}

/// Marks a terrain surface for collision.
/// min/max define the XZ bounds, y is the surface height.
#[derive(Component)]
struct TerrainSurface {
    min: Vec2,
    max: Vec2,
    y: f32,
}

/// Solid wall collision box - blocks horizontal movement.
/// min/max are XZ bounds, y_min/y_max are vertical bounds.
#[derive(Component)]
struct SolidBlock {
    min: Vec2,
    max: Vec2,
    y_min: f32,
    y_max: f32,
}

/// Marks a slide segment for the slide force system.
#[derive(Component)]
struct SlideSegment {
    min: Vec2,
    max: Vec2,
    y: f32,
}

/// Marks a water slide segment that auto-carries the player toward the pool (-x direction).
#[derive(Component)]
struct WaterSlideSegment {
    min: Vec2,
    max: Vec2,
    y: f32,
}

#[derive(Component)]
struct MazeExitZone {
    min: Vec2,
    max: Vec2,
}

#[derive(Component)]
struct SuckUpAnimation {
    start_pos: Vec3,
    end_pos: Vec3,
    elapsed: f32,
    duration: f32,
}

#[derive(Resource)]
struct MazeCompleted;

#[derive(Component)]
struct MazeInteriorWall;

#[derive(Resource)]
struct MazeNeedsRebuild;

#[derive(Component)]
struct TeleporterPad {
    destination: Vec3,
}

// --- Resources ---

#[repr(C)]
#[derive(Resource)]
pub struct HillState {
    pub gate_locked: bool,
    pub slide_friction: f32,
    pub _padding: [u8; 4],
}

impl Default for HillState {
    fn default() -> Self {
        Self {
            gate_locked: true,
            slide_friction: -15.0,
            _padding: [0; 4],
        }
    }
}

// --- Debugger-target functions ---

#[inline(never)]
#[allow(dead_code)]
fn check_gate_access(state: &HillState) -> bool {
    !state.gate_locked
}

#[inline(never)]
fn apply_slide_force(friction: f32, velocity: &mut Vec3, direction: Vec3, dt: f32) {
    *velocity += direction * friction * dt;
}

#[inline(never)]
#[allow(dead_code)]
fn check_summit_reached(player_pos: Vec3) -> bool {
    let flag_pos = Vec3::new(0.0, 10.0, 0.0);
    let dx = player_pos.x - flag_pos.x;
    let dz = player_pos.z - flag_pos.z;
    let dy = player_pos.y - flag_pos.y;
    (dx * dx + dz * dz) < 4.0 && dy.abs() < 2.0
}

// --- Constants ---

const PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 0.0, 25.0);
const CAM_OFFSET: Vec3 = Vec3::new(0.0, 15.0, 15.0);

// --- Setup ---

fn setup_hill(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(ClearColor(Color::srgb(0.45, 0.7, 0.9)));
    commands.insert_resource(HillState::default());
    commands.insert_resource(GroundYOverride(-3.0));

    let gray = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.5, 0.5),
        ..default()
    });
    let dark_gray = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.35, 0.35),
        ..default()
    });
    let green = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.7, 0.2),
        ..default()
    });
    let brown = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.35, 0.2),
        ..default()
    });
    let ice_blue = materials.add(StandardMaterial {
        base_color: Color::srgba(0.5, 0.8, 1.0, 0.85),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let _red = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.15, 0.1),
        ..default()
    });
    let gold = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.85, 0.0),
        ..default()
    });
    let water_blue = materials.add(StandardMaterial {
        base_color: Color::srgba(0.2, 0.4, 0.8, 0.5),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let dark_green = materials.add(StandardMaterial {
        base_color: Color::srgb(0.15, 0.4, 0.1),
        ..default()
    });
    let stone = materials.add(StandardMaterial {
        base_color: Color::srgb(0.6, 0.58, 0.55),
        ..default()
    });
    let fence_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.4, 0.25, 0.15),
        ..default()
    });
    let _white = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        ..default()
    });

    // Ground plane 60x60 — split to leave hole for pool at (-18, 0) size 8x6
    // Pool spans x=-22..-14, z=-3..3
    // Right section: x=-14 to 30 (width 44)
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(44.0, 0.2, 60.0))),
        MeshMaterial3d(green.clone()),
        Transform::from_xyz(8.0, -0.1, 0.0),
        HillEntity,
    ));
    commands.spawn((
        TerrainSurface { min: Vec2::new(-14.5, -30.0), max: Vec2::new(30.0, 30.0), y: 0.0 },
        HillEntity,
    ));
    // Left section: x=-30 to -22 (width 8)
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(8.0, 0.2, 60.0))),
        MeshMaterial3d(green.clone()),
        Transform::from_xyz(-26.0, -0.1, 0.0),
        HillEntity,
    ));
    commands.spawn((
        TerrainSurface { min: Vec2::new(-30.0, -30.0), max: Vec2::new(-21.5, 30.0), y: 0.0 },
        HillEntity,
    ));
    // Top strip (behind pool): x=-22..-14, z=-30..-3 (width 8, depth 27)
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(8.0, 0.2, 27.0))),
        MeshMaterial3d(green.clone()),
        Transform::from_xyz(-18.0, -0.1, -16.5),
        HillEntity,
    ));
    commands.spawn((
        TerrainSurface { min: Vec2::new(-22.5, -30.0), max: Vec2::new(-13.5, -2.5), y: 0.0 },
        HillEntity,
    ));
    // Bottom strip (in front of pool): x=-22..-14, z=3..30 (width 8, depth 27)
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(8.0, 0.2, 27.0))),
        MeshMaterial3d(green.clone()),
        Transform::from_xyz(-18.0, -0.1, 16.5),
        HillEntity,
    ));
    commands.spawn((
        TerrainSurface { min: Vec2::new(-22.5, 2.5), max: Vec2::new(-13.5, 30.0), y: 0.0 },
        HillEntity,
    ));

    // --- Hill tiers (stacked cuboids with tunnel gap in base) ---

    // --- Hill tiers: 10 steps of 1-unit height, each 0.8 units narrower per side ---
    // Each tier is jumpable (max jump ~1.6 units).
    // Tiers 0-2 (y=0..3) have a 3-unit wide tunnel gap along x=0 for a walk-through passage.
    // Tier i: height 1.0, y_center = 0.5 + i, half_size = 9.0 - i * 0.8
    let tunnel_height = 3; // first 3 tiers have tunnel gap
    let tunnel_half_w = 1.5_f32; // tunnel is 3 units wide
    for i in 0..10u32 {
        let h = 1.0_f32;
        let y_center = 0.5 + i as f32;
        let y_top = y_center + h / 2.0;
        let y_bot = y_center - h / 2.0;
        let half = 9.0 - i as f32 * 0.8; // from 9.0 down to 1.8

        if (i as i32) < tunnel_height && half > tunnel_half_w {
            // Split tier into two halves with tunnel gap
            let side_w = half - tunnel_half_w;
            // Left half
            let left_cx = -(tunnel_half_w + side_w / 2.0);
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(side_w, h, half * 2.0))),
                MeshMaterial3d(gray.clone()),
                Transform::from_xyz(left_cx, y_center, 0.0),
                HillEntity,
            ));
            commands.spawn((
                TerrainSurface { min: Vec2::new(-half, -half), max: Vec2::new(-tunnel_half_w, half), y: y_top },
                SolidBlock { min: Vec2::new(-half, -half), max: Vec2::new(-tunnel_half_w, half), y_min: y_bot, y_max: y_top },
                HillEntity,
            ));
            // Right half
            let right_cx = tunnel_half_w + side_w / 2.0;
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(side_w, h, half * 2.0))),
                MeshMaterial3d(gray.clone()),
                Transform::from_xyz(right_cx, y_center, 0.0),
                HillEntity,
            ));
            commands.spawn((
                TerrainSurface { min: Vec2::new(tunnel_half_w, -half), max: Vec2::new(half, half), y: y_top },
                SolidBlock { min: Vec2::new(tunnel_half_w, -half), max: Vec2::new(half, half), y_min: y_bot, y_max: y_top },
                HillEntity,
            ));
        } else {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(half * 2.0, h, half * 2.0))),
                MeshMaterial3d(gray.clone()),
                Transform::from_xyz(0.0, y_center, 0.0),
                HillEntity,
            ));
            commands.spawn((
                TerrainSurface { min: Vec2::new(-half, -half), max: Vec2::new(half, half), y: y_top },
                SolidBlock { min: Vec2::new(-half, -half), max: Vec2::new(half, half), y_min: y_bot, y_max: y_top },
                HillEntity,
            ));
        }
    }

    // Tunnel ceiling (at y=3, spanning the tunnel length)
    let base_half = 9.0_f32;
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(3.0, 0.3, base_half * 2.0))),
        MeshMaterial3d(dark_gray.clone()),
        Transform::from_xyz(0.0, 3.15, 0.0),
        HillEntity,
    ));

    // Tunnel floor terrain (ground level through tunnel)
    commands.spawn((
        TerrainSurface { min: Vec2::new(-tunnel_half_w, -base_half), max: Vec2::new(tunnel_half_w, base_half), y: 0.0 },
        HillEntity,
    ));

    // --- Slide on east side (10 stepped platforms from summit y=9 down to y=0) ---
    for i in 0..10 {
        let slide_y = 9.0 - i as f32 * 0.9;
        let slide_x = 5.0 + i as f32 * 2.0; // from x=5 to x=23
        let step_h = 1.0_f32; // solid height for collision
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(2.0, step_h, 3.0))),
            MeshMaterial3d(ice_blue.clone()),
            Transform::from_xyz(slide_x, slide_y, 0.0),
            HillEntity,
        ));
        let y_top = slide_y + step_h / 2.0;
        let y_bot = slide_y - step_h / 2.0;
        commands.spawn((
            TerrainSurface {
                min: Vec2::new(slide_x - 1.0, -1.5),
                max: Vec2::new(slide_x + 1.0, 1.5),
                y: y_top,
            },
            SolidBlock {
                min: Vec2::new(slide_x - 1.0, -1.5),
                max: Vec2::new(slide_x + 1.0, 1.5),
                y_min: y_bot,
                y_max: y_top,
            },
            HillEntity,
        ));
        commands.spawn((
            SlideSegment {
                min: Vec2::new(slide_x - 1.0, -1.5),
                max: Vec2::new(slide_x + 1.0, 1.5),
                y: y_top,
            },
            HillEntity,
        ));
    }

    // --- Upper ice slide (sky level, 6 segments from y=17 down to y=12.5) ---
    for i in 0..6 {
        let slide_y = 17.0 - i as f32 * 0.9;
        let slide_x = i as f32 * 1.0;
        let step_h = 1.0_f32;
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(2.0, step_h, 3.0))),
            MeshMaterial3d(ice_blue.clone()),
            Transform::from_xyz(slide_x, slide_y, 0.0),
            HillEntity,
        ));
        let y_top = slide_y + step_h / 2.0;
        let y_bot = slide_y - step_h / 2.0;
        commands.spawn((
            TerrainSurface {
                min: Vec2::new(slide_x - 1.0, -1.5),
                max: Vec2::new(slide_x + 1.0, 1.5),
                y: y_top,
            },
            SolidBlock {
                min: Vec2::new(slide_x - 1.0, -1.5),
                max: Vec2::new(slide_x + 1.0, 1.5),
                y_min: y_bot,
                y_max: y_top,
            },
            HillEntity,
        ));
        commands.spawn((
            SlideSegment {
                min: Vec2::new(slide_x - 1.0, -1.5),
                max: Vec2::new(slide_x + 1.0, 1.5),
                y: y_top,
            },
            HillEntity,
        ));
    }

    // --- Water slide on west side (12 stepped platforms from hilltop down to pool) ---
    for i in 0..12 {
        let slide_x = -2.0 - i as f32 * 1.1;
        let slide_y = 9.5 - i as f32 * 0.9;
        let step_h = 0.5_f32;
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(1.2, step_h, 3.0))),
            MeshMaterial3d(water_blue.clone()),
            Transform::from_xyz(slide_x, slide_y, 0.0),
            HillEntity,
        ));
        let y_top = slide_y + step_h / 2.0;
        let y_bot = slide_y - step_h / 2.0;
        commands.spawn((
            TerrainSurface {
                min: Vec2::new(slide_x - 0.6, -1.5),
                max: Vec2::new(slide_x + 0.6, 1.5),
                y: y_top,
            },
            SolidBlock {
                min: Vec2::new(slide_x - 0.6, -1.5),
                max: Vec2::new(slide_x + 0.6, 1.5),
                y_min: y_bot,
                y_max: y_top,
            },
            WaterSlideSegment {
                min: Vec2::new(slide_x - 0.6, -1.5),
                max: Vec2::new(slide_x + 0.6, 1.5),
                y: y_top,
            },
            HillEntity,
        ));
    }

    // --- Victory flag at hilltop ---
    // Pole
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.1, 3.0, 0.1))),
        MeshMaterial3d(brown.clone()),
        Transform::from_xyz(0.0, 11.5, 0.0),
        HillEntity,
    ));
    // Flag
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 0.6, 0.05))),
        MeshMaterial3d(gold.clone()),
        Transform::from_xyz(0.5, 12.5, 0.0),
        HillEntity,
    ));

    // --- Exploration elements ---

    // Trees (trunk + canopy)
    let tree_positions = [
        Vec3::new(-15.0, 0.0, -10.0),
        Vec3::new(-20.0, 0.0, 5.0),
        Vec3::new(20.0, 0.0, 10.0),
        Vec3::new(-10.0, 0.0, 20.0),
        Vec3::new(25.0, 0.0, -5.0),
        Vec3::new(-25.0, 0.0, -20.0),
        Vec3::new(10.0, 0.0, 22.0),
    ];
    for pos in &tree_positions {
        // Trunk
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.4, 2.0, 0.4))),
            MeshMaterial3d(brown.clone()),
            Transform::from_xyz(pos.x, 1.0, pos.z),
            HillEntity,
        ));
        // Canopy
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(2.0, 2.0, 2.0))),
            MeshMaterial3d(dark_green.clone()),
            Transform::from_xyz(pos.x, 3.0, pos.z),
            HillEntity,
        ));
    }

    // Rocks
    let rock_positions = [
        Vec3::new(-12.0, 0.3, 8.0),
        Vec3::new(14.0, 0.3, 12.0),
        Vec3::new(-18.0, 0.3, -5.0),
        Vec3::new(-8.0, 0.3, -18.0),
        Vec3::new(8.0, 0.3, 18.0),
        Vec3::new(-22.0, 0.3, 15.0),
        Vec3::new(18.0, 0.3, 20.0),
        Vec3::new(-5.0, 0.3, -22.0),
        Vec3::new(12.0, 0.3, -20.0),
    ];
    for pos in &rock_positions {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(1.2, 0.6, 0.9))),
            MeshMaterial3d(stone.clone()),
            Transform::from_xyz(pos.x, pos.y, pos.z),
            HillEntity,
        ));
    }

    // --- Maze (east side, ground level) ---
    let mut maze_rng = AppleRng::new();

    // All maze walls (boundary + interior + corner posts) — spawned as one grid system
    spawn_maze_interior(&mut commands, &mut meshes, &mut materials, &mut maze_rng);

    // --- Teleporter pad (gold glowing platform) ---
    let teleporter_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.85, 0.0),
        emissive: LinearRgba::new(1.0, 0.7, 0.0, 1.0),
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.5, 0.3, 2.5))),
        MeshMaterial3d(teleporter_mat),
        Transform::from_xyz(21.0, 0.15, -10.0),
        TeleporterPad { destination: Vec3::new(0.0, 17.5, 0.0) },
        HillEntity,
    ));

    // --- Chipmunk statue (east side) ---
    let tan = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.65, 0.4),
        ..default()
    });
    let dark_brown = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.2, 0.1),
        ..default()
    });
    commands
        .spawn((
            Transform::from_xyz(18.0, 0.0, -8.0),
            Visibility::default(),
            HillEntity,
        ))
        .with_children(|parent| {
            // Pedestal
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(2.0, 0.4, 2.0))),
                MeshMaterial3d(stone.clone()),
                Transform::from_xyz(0.0, 0.2, 0.0),
            ));
            // Body
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.7, 1.0, 0.6))),
                MeshMaterial3d(brown.clone()),
                Transform::from_xyz(0.0, 1.0, 0.0),
            ));
            // Belly
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.5, 0.6, 0.1))),
                MeshMaterial3d(tan.clone()),
                Transform::from_xyz(0.0, 0.9, -0.3),
            ));
            // Head
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.6, 0.55, 0.55))),
                MeshMaterial3d(brown.clone()),
                Transform::from_xyz(0.0, 1.8, 0.0),
            ));
            // Left cheek
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.15, 0.2, 0.1))),
                MeshMaterial3d(tan.clone()),
                Transform::from_xyz(-0.2, 1.75, -0.3),
            ));
            // Right cheek
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.15, 0.2, 0.1))),
                MeshMaterial3d(tan.clone()),
                Transform::from_xyz(0.2, 1.75, -0.3),
            ));
            // Left ear
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.12, 0.15, 0.08))),
                MeshMaterial3d(brown.clone()),
                Transform::from_xyz(-0.2, 2.15, 0.0),
            ));
            // Right ear
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.12, 0.15, 0.08))),
                MeshMaterial3d(brown.clone()),
                Transform::from_xyz(0.2, 2.15, 0.0),
            ));
            // Nose
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.08, 0.08, 0.08))),
                MeshMaterial3d(dark_brown.clone()),
                Transform::from_xyz(0.0, 1.78, -0.3),
            ));
            // Left eye
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.08, 0.08, 0.08))),
                MeshMaterial3d(dark_brown.clone()),
                Transform::from_xyz(-0.12, 1.85, -0.28),
            ));
            // Right eye
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.08, 0.08, 0.08))),
                MeshMaterial3d(dark_brown.clone()),
                Transform::from_xyz(0.12, 1.85, -0.28),
            ));
            // Tail
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.2, 0.8, 0.2))),
                MeshMaterial3d(brown.clone()),
                Transform::from_xyz(0.0, 1.2, 0.4),
            ));
            // Back stripe
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.1, 0.6, 0.1))),
                MeshMaterial3d(dark_brown),
                Transform::from_xyz(0.0, 1.0, 0.31),
            ));
        });

    // Sunken pool (left of hill) — player falls in and must jump out
    let pool_depth = 1.0_f32;
    let pool_x = -18.0_f32;
    let pool_z = 0.0_f32;
    let pool_w = 8.0_f32;
    let pool_l = 6.0_f32;
    let pool_x_min = pool_x - pool_w / 2.0; // -22
    let pool_x_max = pool_x + pool_w / 2.0; // -14
    let pool_z_min = pool_z - pool_l / 2.0; // -3
    let pool_z_max = pool_z + pool_l / 2.0; //  3
    // Pool floor
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(pool_w, 0.1, pool_l))),
        MeshMaterial3d(water_blue.clone()),
        Transform::from_xyz(pool_x, -pool_depth, pool_z),
        HillEntity,
    ));
    commands.spawn((
        TerrainSurface {
            min: Vec2::new(pool_x_min, pool_z_min),
            max: Vec2::new(pool_x_max, pool_z_max),
            y: -pool_depth + 0.05,
        },
        HillEntity,
    ));
    // Pool shore walls — solid blocks so player bumps into edges and must jump out
    // These are the ground edges around the pool, acting as walls from the pool side.
    let wall_thick = 0.5;
    // East shore (x = pool_x_max)
    commands.spawn((
        SolidBlock {
            min: Vec2::new(pool_x_max, pool_z_min),
            max: Vec2::new(pool_x_max + wall_thick, pool_z_max),
            y_min: -pool_depth,
            y_max: 0.0,
        },
        HillEntity,
    ));
    // West shore (x = pool_x_min)
    commands.spawn((
        SolidBlock {
            min: Vec2::new(pool_x_min - wall_thick, pool_z_min),
            max: Vec2::new(pool_x_min, pool_z_max),
            y_min: -pool_depth,
            y_max: 0.0,
        },
        HillEntity,
    ));
    // North shore (z = pool_z_min)
    commands.spawn((
        SolidBlock {
            min: Vec2::new(pool_x_min, pool_z_min - wall_thick),
            max: Vec2::new(pool_x_max, pool_z_min),
            y_min: -pool_depth,
            y_max: 0.0,
        },
        HillEntity,
    ));
    // South shore (z = pool_z_max)
    commands.spawn((
        SolidBlock {
            min: Vec2::new(pool_x_min, pool_z_max),
            max: Vec2::new(pool_x_max, pool_z_max + wall_thick),
            y_min: -pool_depth,
            y_max: 0.0,
        },
        HillEntity,
    ));
    // Water surface (translucent)
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(pool_w, 0.05, pool_l))),
        MeshMaterial3d(water_blue),
        Transform::from_xyz(pool_x, -0.3, pool_z),
        HillEntity,
    ));

    // Path stones from spawn to hill
    for i in 0..6 {
        let z = 24.0 - i as f32 * 3.0;
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(1.5, 0.1, 1.0))),
            MeshMaterial3d(stone.clone()),
            Transform::from_xyz(0.0, 0.01, z),
            HillEntity,
        ));
    }

    // Boundary fences (4 walls around the 60x60 area)
    let fence_thickness = 0.3;
    let fence_height = 1.5;
    // North
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(60.0, fence_height, fence_thickness))),
        MeshMaterial3d(fence_mat.clone()),
        Transform::from_xyz(0.0, fence_height / 2.0, -30.0),
        HillEntity,
    ));
    // South
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(60.0, fence_height, fence_thickness))),
        MeshMaterial3d(fence_mat.clone()),
        Transform::from_xyz(0.0, fence_height / 2.0, 30.0),
        HillEntity,
    ));
    // East
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(fence_thickness, fence_height, 60.0))),
        MeshMaterial3d(fence_mat.clone()),
        Transform::from_xyz(30.0, fence_height / 2.0, 0.0),
        HillEntity,
    ));
    // West
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(fence_thickness, fence_height, 60.0))),
        MeshMaterial3d(fence_mat),
        Transform::from_xyz(-30.0, fence_height / 2.0, 0.0),
        HillEntity,
    ));

    // --- Lighting ---
    commands.spawn((
        DirectionalLight {
            illuminance: 12000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, 0.3, 0.0)),
        HillEntity,
    ));

    // Ambient
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 400.0,
    });

    // --- Player ---
    let player = spawn_player(&mut commands, &mut meshes, &mut materials);
    commands.entity(player).insert((
        Transform::from_xyz(PLAYER_SPAWN.x, PLAYER_SPAWN.y, PLAYER_SPAWN.z)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        MovementBounds {
            rects: vec![(Vec2::new(-29.5, -29.5), Vec2::new(29.5, 29.5))],
        },
        HillEntity,
    ));

    // --- Camera ---
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(
            PLAYER_SPAWN.x + CAM_OFFSET.x,
            PLAYER_SPAWN.y + CAM_OFFSET.y,
            PLAYER_SPAWN.z + CAM_OFFSET.z,
        )
        .looking_at(PLAYER_SPAWN, Vec3::Y),
        HillFollowCam,
        HillEntity,
    ));

    // --- HUD ---
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(12.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
            HillEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Level 13: The Hill Fortress"),
                TextFont { font_size: 26.0, ..default() },
                TextColor(Color::WHITE),
                HillEntity,
            ));
            parent.spawn((
                Text::new("Reach the flag at the summit!"),
                TextFont { font_size: 18.0, ..default() },
                TextColor(Color::srgb(0.9, 0.9, 0.5)),
                HudText,
                HillEntity,
            ));
        });

    // Hint text at bottom
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(12.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        HillEntity,
    )).with_children(|parent| {
        parent.spawn((
            Text::new("[ESC] Menu  |  [WASD] Move  |  [Space] Jump  |  [P] Pause"),
            TextFont { font_size: 14.0, ..default() },
            TextColor(Color::srgb(0.7, 0.7, 0.7)),
            HillEntity,
        ));
    });

    // --- Power-up apples ---
    let apple_green = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.9, 0.2),
        ..default()
    });
    let apple_red = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.15, 0.15),
        ..default()
    });
    let apple_purple = materials.add(StandardMaterial {
        base_color: Color::srgb(0.7, 0.2, 0.9),
        ..default()
    });
    let stem_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.3, 0.15),
        ..default()
    });

    let apple_sphere = meshes.add(Cuboid::new(0.6, 0.6, 0.6));
    let stem_mesh = meshes.add(Cuboid::new(0.08, 0.25, 0.08));

    let apple_assets = AppleAssets {
        sphere: apple_sphere.clone(),
        stem: stem_mesh.clone(),
        green: apple_green.clone(),
        red: apple_red.clone(),
        purple: apple_purple.clone(),
        stem_mat: stem_mat.clone(),
    };

    let mut apple_rng = AppleRng::new();
    let apples = [
        (random_apple_pos(&mut apple_rng), AppleKind::Speed, apple_green),
        (random_apple_pos(&mut apple_rng), AppleKind::Jump, apple_red),
        (random_apple_pos(&mut apple_rng), AppleKind::Backwards, apple_purple),
    ];
    for (pos, kind, mat) in apples {
        commands
            .spawn((
                Transform::from_translation(pos),
                Visibility::default(),
                PowerUpApple { kind },
                HillEntity,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Mesh3d(apple_sphere.clone()),
                    MeshMaterial3d(mat),
                    Transform::IDENTITY,
                ));
                parent.spawn((
                    Mesh3d(stem_mesh.clone()),
                    MeshMaterial3d(stem_mat.clone()),
                    Transform::from_xyz(0.0, 0.45, 0.0),
                ));
            });
    }

    commands.insert_resource(ActivePowerUps::default());
    commands.insert_resource(apple_assets);
    commands.insert_resource(apple_rng);

    // --- Power-up progress bar UI (top-right) ---
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(12.0),
                top: Val::Px(12.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
            PowerUpBarContainer,
            HillEntity,
        ))
        .with_children(|parent| {
            let bar_defs: [(AppleKind, Color, &str); 3] = [
                (AppleKind::Speed, Color::srgb(0.2, 0.9, 0.2), "Speed"),
                (AppleKind::Jump, Color::srgb(0.9, 0.15, 0.15), "Jump"),
                (AppleKind::Backwards, Color::srgb(0.7, 0.2, 0.9), "Reverse"),
            ];
            for (kind, color, label) in bar_defs {
                parent
                    .spawn((
                        Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(6.0),
                            display: Display::None,
                            ..default()
                        },
                        PowerUpBarBg { kind },
                        HillEntity,
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Node {
                                min_width: Val::Px(55.0),
                                ..default()
                            },
                            Text::new(label),
                            TextFont { font_size: 13.0, ..default() },
                            TextColor(color),
                        ));
                        row.spawn((
                            Node {
                                width: Val::Px(200.0),
                                height: Val::Px(12.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
                        ))
                        .with_children(|bg| {
                            bg.spawn((
                                Node {
                                    width: Val::Px(200.0),
                                    height: Val::Px(12.0),
                                    ..default()
                                },
                                BackgroundColor(color),
                                PowerUpBar { kind },
                            ));
                        });
                    });
            }
        });

    // Gate hint sign
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(40.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        HillEntity,
    )).with_children(|parent| {
        parent.spawn((
            Text::new("Hint: HillState.gate_locked / HillState.slide_friction"),
            TextFont { font_size: 13.0, ..default() },
            TextColor(Color::srgb(0.6, 0.6, 0.6)),
            HillEntity,
        ));
    });
}

// --- Terrain collision ---

fn terrain_collision(
    surfaces: Query<&TerrainSurface>,
    solids: Query<&SolidBlock>,
    mut player_q: Query<(&mut Transform, &mut PlayerPhysics, &mut SquashState), (With<Player>, Without<SuckUpAnimation>)>,
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

// --- Slide force system ---

fn slide_force_system(
    time: Res<Time>,
    hill_state: Res<HillState>,
    slides: Query<&SlideSegment>,
    mut player_q: Query<(&Transform, &mut PlayerPhysics), (With<Player>, Without<SuckUpAnimation>)>,
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
                apply_slide_force(hill_state.slide_friction, &mut physics.velocity, direction, time.delta_secs());
                break;
            }
        }
    }
}

// --- Water slide system ---

fn water_slide_system(
    slides: Query<&WaterSlideSegment>,
    mut player_q: Query<(&Transform, &mut PlayerPhysics), (With<Player>, Without<SuckUpAnimation>)>,
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

// --- Camera follow ---

fn follow_camera(
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

// --- Playing update (placeholder for future logic) ---

fn hill_playing_update(
    game_paused: Res<GamePaused>,
) {
    if game_paused.0 {
        return;
    }
}

// --- Victory overlay ---

fn handle_victory(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_screen: ResMut<NextState<Screen>>,
    overlay_q: Query<Entity, With<OverlayScreen>>,
) {
    if overlay_q.is_empty() {
        commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    position_type: PositionType::Absolute,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(16.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
                GlobalZIndex(10),
                OverlayScreen,
                HillEntity,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new("SUMMIT REACHED!"),
                    TextFont { font_size: 48.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.85, 0.0)),
                ));
                parent.spawn((
                    Text::new("You conquered the Hill Fortress!"),
                    TextFont { font_size: 22.0, ..default() },
                    TextColor(Color::WHITE),
                ));
                parent.spawn((
                    Text::new("Press ENTER to return to menu"),
                    TextFont { font_size: 18.0, ..default() },
                    TextColor(Color::srgb(0.7, 0.7, 0.7)),
                ));
            });
    }

    if keyboard.just_pressed(KeyCode::Enter) {
        next_screen.set(Screen::Menu);
    }
}

// --- Apple bob animation ---

fn apple_bob_system(
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

// --- Apple collection ---

fn apple_collection_system(
    mut commands: Commands,
    player_q: Query<&Transform, With<Player>>,
    apple_q: Query<(Entity, &Transform, &PowerUpApple), Without<Player>>,
    mut power_ups: ResMut<ActivePowerUps>,
) {
    let Ok(player_tf) = player_q.get_single() else { return };
    let pp = player_tf.translation;

    for (entity, apple_tf, apple) in &apple_q {
        let dist = pp.distance(apple_tf.translation);
        if dist < 1.5 {
            commands.entity(entity).despawn_recursive();
            match apple.kind {
                AppleKind::Speed => {
                    commands.insert_resource(SpeedBoostMultiplier(2.0));
                    power_ups.speed_timer = 60.0;
                }
                AppleKind::Jump => {
                    commands.insert_resource(JumpBoostMultiplier(2.0));
                    power_ups.jump_timer = 60.0;
                }
                AppleKind::Backwards => {
                    commands.insert_resource(ReversePlayerFacing);
                    power_ups.backwards_timer = 60.0;
                }
            }
            // Queue respawn after 60 seconds at a new random position
            power_ups.respawn_timers.push((apple.kind, 60.0));
        }
    }
}

// --- Power-up timer ---

fn power_up_timer_system(
    mut commands: Commands,
    time: Res<Time>,
    mut power_ups: ResMut<ActivePowerUps>,
    apple_assets: Option<Res<AppleAssets>>,
    mut apple_rng: ResMut<AppleRng>,
) {
    let dt = time.delta_secs();

    if power_ups.speed_timer > 0.0 {
        power_ups.speed_timer = (power_ups.speed_timer - dt).max(0.0);
        if power_ups.speed_timer == 0.0 {
            commands.remove_resource::<SpeedBoostMultiplier>();
        }
    }
    if power_ups.jump_timer > 0.0 {
        power_ups.jump_timer = (power_ups.jump_timer - dt).max(0.0);
        if power_ups.jump_timer == 0.0 {
            commands.remove_resource::<JumpBoostMultiplier>();
        }
    }
    if power_ups.backwards_timer > 0.0 {
        power_ups.backwards_timer = (power_ups.backwards_timer - dt).max(0.0);
        if power_ups.backwards_timer == 0.0 {
            commands.remove_resource::<ReversePlayerFacing>();
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

// --- Power-up bar UI ---

fn power_up_bar_ui_system(
    power_ups: Res<ActivePowerUps>,
    mut bar_q: Query<(&PowerUpBar, &mut Node)>,
    mut bg_q: Query<(&PowerUpBarBg, &mut Node), Without<PowerUpBar>>,
) {
    const MAX_WIDTH: f32 = 200.0;

    for (bar, mut node) in &mut bar_q {
        let remaining = match bar.kind {
            AppleKind::Speed => power_ups.speed_timer,
            AppleKind::Jump => power_ups.jump_timer,
            AppleKind::Backwards => power_ups.backwards_timer,
        };
        node.width = Val::Px(remaining / 60.0 * MAX_WIDTH);
    }

    for (bg, mut node) in &mut bg_q {
        let remaining = match bg.kind {
            AppleKind::Speed => power_ups.speed_timer,
            AppleKind::Jump => power_ups.jump_timer,
            AppleKind::Backwards => power_ups.backwards_timer,
        };
        node.display = if remaining > 0.0 { Display::Flex } else { Display::None };
    }
}

// --- Maze exit check ---

fn maze_exit_check_system(
    mut commands: Commands,
    player_q: Query<(Entity, &Transform), With<Player>>,
    exit_zones: Query<&MazeExitZone>,
    maze_completed: Option<Res<MazeCompleted>>,
) {
    if maze_completed.is_some() {
        return;
    }
    let Ok((player_entity, player_tf)) = player_q.get_single() else { return };
    let px = player_tf.translation.x;
    let pz = player_tf.translation.z;

    for zone in &exit_zones {
        if px >= zone.min.x && px <= zone.max.x && pz >= zone.min.y && pz <= zone.max.y {
            commands.insert_resource(MazeCompleted);
            commands.entity(player_entity).insert(SuckUpAnimation {
                start_pos: player_tf.translation,
                end_pos: Vec3::new(21.0, 0.5, -10.0),
                elapsed: 0.0,
                duration: 2.0,
            });
            break;
        }
    }
}

// --- Teleporter system ---

fn teleporter_system(
    mut player_q: Query<(&mut Transform, &mut PlayerPhysics), (With<Player>, Without<SuckUpAnimation>)>,
    pads: Query<(&Transform, &TeleporterPad), Without<Player>>,
) {
    let Ok((mut player_tf, mut physics)) = player_q.get_single_mut() else { return };
    let pp = player_tf.translation;

    for (pad_tf, pad) in &pads {
        let center = pad_tf.translation;
        let dx = pp.x - center.x;
        let dz = pp.z - center.z;
        let dy = (pp.y - center.y).abs();
        if dx * dx + dz * dz < 1.5 * 1.5 && dy < 1.0 {
            player_tf.translation = pad.destination;
            physics.velocity = Vec3::ZERO;
            physics.grounded = false;
            return;
        }
    }
}

// --- Suck-up animation ---

fn suck_up_animation_system(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Transform, &mut PlayerPhysics, &mut SuckUpAnimation), With<Player>>,
) {
    for (entity, mut transform, mut physics, mut anim) in &mut query {
        anim.elapsed += time.delta_secs();
        let t = (anim.elapsed / anim.duration).clamp(0.0, 1.0);

        // smoothstep
        let t_smooth = t * t * (3.0 - 2.0 * t);

        let start = anim.start_pos;
        let end = anim.end_pos;
        let x = start.x + (end.x - start.x) * t_smooth;
        let z = start.z + (end.z - start.z) * t_smooth;
        let y = start.y + (end.y - start.y) * t_smooth + 4.0 * t * (1.0 - t) * 3.0;

        transform.translation = Vec3::new(x, y, z);
        physics.velocity = Vec3::ZERO;
        physics.grounded = false;

        if t >= 1.0 {
            transform.translation = end;
            physics.grounded = true;
            commands.entity(entity).remove::<SuckUpAnimation>();
            commands.remove_resource::<MazeCompleted>();
            commands.insert_resource(MazeNeedsRebuild);
        }
    }
}

// --- Maze rebuild ---

fn maze_rebuild_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    rebuild: Option<Res<MazeNeedsRebuild>>,
    maze_walls: Query<Entity, With<MazeInteriorWall>>,
    mut rng: ResMut<AppleRng>,
) {
    if rebuild.is_none() {
        return;
    }
    commands.remove_resource::<MazeNeedsRebuild>();
    for entity in &maze_walls {
        commands.entity(entity).despawn_recursive();
    }
    spawn_maze_interior(&mut commands, &mut meshes, &mut materials, &mut rng);
}

// --- Cleanup ---

fn cleanup_hill(
    mut commands: Commands,
    query: Query<Entity, With<HillEntity>>,
) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
    commands.remove_resource::<HillState>();
    commands.remove_resource::<GroundYOverride>();
    commands.remove_resource::<ActivePowerUps>();
    commands.remove_resource::<SpeedBoostMultiplier>();
    commands.remove_resource::<JumpBoostMultiplier>();
    commands.remove_resource::<ReversePlayerFacing>();
    commands.remove_resource::<AppleAssets>();
    commands.remove_resource::<AppleRng>();
    commands.remove_resource::<MazeCompleted>();
    commands.remove_resource::<MazeNeedsRebuild>();
}
