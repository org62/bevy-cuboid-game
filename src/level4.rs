use bevy::prelude::*;

use crate::level_kit::{self, GameplaySet, LevelPhase, VictoryText};
use crate::player::{spawn_player, MovementBounds, Player};
use crate::shared_ui;
use crate::terrain::{SolidBlock, TerrainConfig, TerrainSurface};
use crate::{GamePaused, Screen, Scoreboard};

pub const ID: u32 = 4;
const SCREEN: Screen = Screen::Level(ID);

pub fn register(app: &mut App) {
    app.add_systems(OnEnter(SCREEN), setup_maze)
        .add_systems(
            Update,
            check_goal
                .in_set(GameplaySet::Logic)
                .run_if(level_kit::in_phase(SCREEN, LevelPhase::Playing)),
        )
        .add_systems(
            Update,
            maze_visual_update
                .in_set(GameplaySet::Logic)
                .run_if(in_state(SCREEN)),
        )
        // Reveal the maze the player just bypassed behind the victory overlay.
        .add_systems(
            OnEnter(LevelPhase::Victory),
            reveal_walls.run_if(in_state(SCREEN)),
        )
        .add_systems(
            OnExit(SCREEN),
            (level_kit::despawn_level::<MazeEntity>, cleanup_maze),
        );
}

// --- Components ---

#[derive(Component, Clone, Copy)]
struct MazeEntity;

#[derive(Component)]
struct Trophy;

#[derive(Component)]
struct WallVisual;

#[derive(Resource, Default)]
struct WallsVisible(bool);

// --- Constants ---

const TROPHY_POS: Vec3 = Vec3::new(12.0, 0.5, -10.0);
const ARENA_MIN: Vec2 = Vec2::new(-2.0, -12.0);
const ARENA_MAX: Vec2 = Vec2::new(14.0, 2.0);
const PLAYER_SPAWN: Vec3 = Vec3::new(-1.0, 0.0, 1.0);
/// Height of the invisible walls, shared by the visual mesh and the collision
/// gate. A normal jump peaks at ~1.6 units, so it is intentionally taller than
/// the player can reach by jumping: the only way over is to lift the player's Y
/// coordinate in memory.
const WALL_HEIGHT: f32 = 2.5;
/// Player radius the wall pushout uses (matches the old bespoke collision).
const WALL_PUSHOUT_MARGIN: f32 = 0.4;

const MAZE_TUTORIAL: &str = "\
The maze walls are invisible and block you on the ground - but they are only so tall. If you lift the player above the walls, you can simply walk over them to the trophy. A normal jump is not high enough, so you must change the player's height directly in memory.

The player's height is its Y coordinate (Transform.translation.y). It reads 0.0 while standing on the ground and only rises while jumping. You don't need to know any exact number - find it by watching how it changes:

1) Stand still on the ground. In your memory scanner, scan for the 4-byte float 0.0 (or start with an 'Unknown initial value' scan).
2) Press Space to jump; while airborne, run a 'Increased value' scan. Land, then run a 'Decreased value' scan. Repeat jump/land a few times, alternating Increased/Decreased.
3) The Y coordinate is the address that rises when you jump and returns to 0.0 on the ground. Its two neighbours (+/-4 bytes) are X and Z, since the three floats are contiguous (x, y, z at offsets 0, 4, 8).
4) Set Y to a value above the walls (e.g. 5.0) and freeze it, then walk over the maze until you are above the trophy - the walls no longer stop you.
5) Once you are over the trophy, unfreeze Y (or set it back to 0.0) so the player drops down onto it. The trophy only counts as collected when you actually reach it, not while hovering above.";

// --- Debugger-target functions ---

#[inline(never)]
fn check_trophy_collected(player_pos: Vec3, trophy_pos: Vec3) -> bool {
    let dx = player_pos.x - trophy_pos.x;
    let dy = player_pos.y - trophy_pos.y;
    let dz = player_pos.z - trophy_pos.z;
    // Full 3D distance: the player must actually descend onto the trophy, not
    // just hover above its XZ. Flying over the walls at a high Y gets you to
    // the trophy's location, but you still have to come back down to collect it.
    (dx * dx + dy * dy + dz * dz) < 2.0
}

// --- Maze layout ---
//
// Collision is NOT bespoke: each rect becomes a shared-terrain `SolidBlock`
// spanning y 0..WALL_HEIGHT, so `terrain_collision` does the pushout and a
// player lifted above the walls passes straight over (the intended debugger
// solution).

struct WallRect {
    min: Vec2,
    max: Vec2,
}

fn wall_rects() -> Vec<WallRect> {
    vec![
        // === Full-width horizontal barriers with one gap each ===
        // Row A (Z = -1 to -0.5): gap at X = 5..7
        WallRect { min: Vec2::new(-2.0, -1.0), max: Vec2::new(5.0, -0.5) },
        WallRect { min: Vec2::new(7.0, -1.0), max: Vec2::new(14.0, -0.5) },
        // Row B (Z = -4 to -3.5): gap at X = 10..12
        WallRect { min: Vec2::new(-2.0, -4.0), max: Vec2::new(10.0, -3.5) },
        WallRect { min: Vec2::new(12.0, -4.0), max: Vec2::new(14.0, -3.5) },
        // Row C (Z = -7 to -6.5): gap at X = 0..2
        WallRect { min: Vec2::new(-2.0, -7.0), max: Vec2::new(0.0, -6.5) },
        WallRect { min: Vec2::new(2.0, -7.0), max: Vec2::new(14.0, -6.5) },
        // Row D (Z = -9 to -8.5): gap at X = 8..10
        WallRect { min: Vec2::new(-2.0, -9.0), max: Vec2::new(8.0, -8.5) },
        WallRect { min: Vec2::new(10.0, -9.0), max: Vec2::new(14.0, -8.5) },

        // === Vertical walls (block shortcuts between rows) ===
        // V1: after Row A gap, blocks going straight down
        WallRect { min: Vec2::new(7.0, -3.5), max: Vec2::new(7.5, -1.0) },
        // V2: blocks right-side bypass between Row B and Row C
        WallRect { min: Vec2::new(12.0, -6.5), max: Vec2::new(12.5, -4.0) },
        // V3: blocks left-side shortcut between Row C and Row D
        WallRect { min: Vec2::new(2.0, -8.5), max: Vec2::new(2.5, -7.0) },

        // === Dead-end traps (lure players into wrong corridors) ===
        // Trap near Row A: looks like a second passage
        WallRect { min: Vec2::new(9.0, -3.0), max: Vec2::new(9.5, -1.0) },
        // Trap near Row C: false corridor on the left
        WallRect { min: Vec2::new(5.0, -8.5), max: Vec2::new(5.5, -7.0) },
    ]
}

#[cfg(test)]
fn collides_with_walls(x: f32, z: f32, walls: &[WallRect]) -> bool {
    let radius = WALL_PUSHOUT_MARGIN;
    for w in walls {
        if x + radius > w.min.x && x - radius < w.max.x
            && z + radius > w.min.y && z - radius < w.max.y
        {
            return true;
        }
    }
    false
}

// --- Setup ---

fn setup_maze(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    scoreboard: Res<Scoreboard>,
) {
    commands.insert_resource(ClearColor(Color::srgb(0.08, 0.1, 0.15)));
    commands.insert_resource(WallsVisible::default());
    commands.insert_resource(VictoryText::new("TROPHY COLLECTED!"));
    commands.insert_resource(TerrainConfig {
        pushout_margin: WALL_PUSHOUT_MARGIN,
        ..TerrainConfig::standard(-1.0)
    });

    // Dark green ground, sized to exactly cover the walkable arena so the
    // colored floor and the reachable area agree.
    let ground_w = ARENA_MAX.x - ARENA_MIN.x;
    let ground_d = ARENA_MAX.y - ARENA_MIN.y;
    let ground_cx = (ARENA_MIN.x + ARENA_MAX.x) * 0.5;
    let ground_cz = (ARENA_MIN.y + ARENA_MAX.y) * 0.5;
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(ground_w, ground_d))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.12, 0.22, 0.1),
            ..default()
        })),
        Transform::from_xyz(ground_cx, 0.0, ground_cz),
        // Mesh top and walkable surface derive from the same y = 0.
        TerrainSurface { min: ARENA_MIN, max: ARENA_MAX, y: 0.0 },
        MazeEntity,
    ));

    // Trophy - golden glowing sphere on pedestal
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.5, 0.6))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.4, 0.38, 0.35),
            ..default()
        })),
        Transform::from_xyz(TROPHY_POS.x, 0.3, TROPHY_POS.z),
        MazeEntity,
    ));
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.4))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.85, 0.2),
            emissive: LinearRgba::new(2.0, 1.5, 0.3, 1.0),
            ..default()
        })),
        Transform::from_xyz(TROPHY_POS.x, 1.0, TROPHY_POS.z),
        Trophy,
        MazeEntity,
    ));
    // Trophy point light
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.9, 0.3),
            intensity: 8000.0,
            range: 6.0,
            ..default()
        },
        Transform::from_xyz(TROPHY_POS.x, 2.0, TROPHY_POS.z),
        MazeEntity,
    ));

    // Walls: one entity per rect carries both the collision (`SolidBlock`)
    // and the visual mesh (hidden by default, toggled with ?). NOT tagged
    // `CameraOccluder` — they're walk-over height and would twitch the camera.
    let wall_visual_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.4, 0.8, 1.0, 0.35),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    for wall in &wall_rects() {
        let w = wall.max.x - wall.min.x;
        let h = wall.max.y - wall.min.y;
        let cx = (wall.min.x + wall.max.x) * 0.5;
        let cz = (wall.min.y + wall.max.y) * 0.5;
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(w, WALL_HEIGHT, h))),
            MeshMaterial3d(wall_visual_mat.clone()),
            Transform::from_xyz(cx, WALL_HEIGHT * 0.5, cz),
            Visibility::Hidden,
            SolidBlock { min: wall.min, max: wall.max, y_min: 0.0, y_max: WALL_HEIGHT },
            WallVisual,
            MazeEntity,
        ));
    }

    // Moon light
    shared_ui::setup_level_lighting(
        &mut commands,
        6000.0,
        (-0.8, 0.3, 0.0),
        Color::srgb(0.4, 0.45, 0.6),
        300.0,
        MazeEntity,
    );

    // Player
    let player = spawn_player(&mut commands, &mut meshes, &mut materials);
    commands.entity(player).insert((
        MazeEntity,
        Transform::from_xyz(PLAYER_SPAWN.x, 0.0, PLAYER_SPAWN.z)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        MovementBounds {
            rects: vec![(ARENA_MIN, ARENA_MAX)],
        },
    ));

    // Camera with atmospheric fog
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(6.0, 12.0, 8.0).looking_at(Vec3::new(6.0, 0.0, -5.0), Vec3::Y),
        DistanceFog {
            color: Color::srgba(0.15, 0.18, 0.25, 1.0),
            falloff: FogFalloff::Exponential { density: 0.04 },
            ..default()
        },
        shared_ui::FollowCamera {
            offset: Vec3::new(0.0, 12.0, 12.0),

            look_offset: Vec3::Y,
        },
        MazeEntity,
    ));

    // HUD
    shared_ui::spawn_controls_hint(
        &mut commands,
        "Reach the trophy",
        MazeEntity,
    );

    // Hint box + tutorial modal (hidden; H reveals the hint, T the tutorial)
    if !scoreboard.is_solved(ID) {
        shared_ui::spawn_hint_box_with_tutorial(
            &mut commands,
            "Your Y (jump) coordinate is zero when the player is on the ground. The walls only reach so high...",
            300.0,
            MazeEntity,
        );
        shared_ui::spawn_hint_modal(
            &mut commands,
            "Invisible Maze - Full Solution",
            MAZE_TUTORIAL,
            MazeEntity,
        );
    }
}

// --- Gameplay ---

fn check_goal(
    mut next_phase: ResMut<NextState<LevelPhase>>,
    player_q: Query<&Transform, With<Player>>,
    game_paused: Res<GamePaused>,
) {
    if game_paused.0 { return; }
    let Ok(pt) = player_q.get_single() else {
        return;
    };
    if check_trophy_collected(pt.translation, TROPHY_POS) {
        next_phase.set(LevelPhase::Victory);
    }
}

// --- Visual ---

fn maze_visual_update(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut trophy_q: Query<&mut Transform, With<Trophy>>,
    mut walls_visible: ResMut<WallsVisible>,
    mut wall_q: Query<&mut Visibility, With<WallVisual>>,
) {
    let dt = time.delta_secs();
    let elapsed = time.elapsed_secs();

    // Trophy bob
    for mut t in &mut trophy_q {
        t.translation.y = 1.0 + (elapsed * 1.5).sin() * 0.15;
        t.rotate_y(1.0 * dt);
    }

    // Toggle wall visibility with ? (Shift + /)
    if keyboard.just_pressed(KeyCode::Slash) && keyboard.pressed(KeyCode::ShiftLeft)
        || keyboard.just_pressed(KeyCode::Slash) && keyboard.pressed(KeyCode::ShiftRight)
    {
        walls_visible.0 = !walls_visible.0;
        let vis = if walls_visible.0 { Visibility::Visible } else { Visibility::Hidden };
        for mut v in &mut wall_q {
            *v = vis;
        }
    }
}

// --- Victory hook (overlay and dismissal are the shared flow) ---

/// Reveal the maze the player just bypassed.
fn reveal_walls(
    mut walls_visible: ResMut<WallsVisible>,
    mut wall_q: Query<&mut Visibility, With<WallVisual>>,
) {
    walls_visible.0 = true;
    for mut v in &mut wall_q {
        *v = Visibility::Visible;
    }
}

// --- Cleanup ---

fn cleanup_maze(mut commands: Commands) {
    commands.remove_resource::<WallsVisible>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trophy_collected_when_close() {
        let player_pos = Vec3::new(12.0, 0.5, -10.0);
        assert!(check_trophy_collected(player_pos, TROPHY_POS));
    }

    #[test]
    fn trophy_not_collected_when_far() {
        let player_pos = Vec3::new(0.0, 0.0, 0.0);
        assert!(!check_trophy_collected(player_pos, TROPHY_POS));
    }

    #[test]
    fn trophy_collected_within_distance_2() {
        // Just inside radius
        let player_pos = Vec3::new(TROPHY_POS.x + 1.3, 0.5, TROPHY_POS.z);
        assert!(check_trophy_collected(player_pos, TROPHY_POS));

        // Just outside radius
        let player_pos2 = Vec3::new(TROPHY_POS.x + 1.5, 0.5, TROPHY_POS.z);
        assert!(!check_trophy_collected(player_pos2, TROPHY_POS));
    }

    #[test]
    fn invisible_walls_block_movement() {
        let walls = wall_rects();
        // Inside Row A left segment (x: -2 to 5, z: -1 to -0.5)
        assert!(collides_with_walls(3.0, -0.75, &walls));
        // Inside Row A right segment (x: 7 to 14, z: -1 to -0.5)
        assert!(collides_with_walls(10.0, -0.75, &walls));
        // Inside V1 vertical wall (x: 7 to 7.5, z: -3.5 to -1)
        assert!(collides_with_walls(7.25, -2.0, &walls));
        // Open area near start
        assert!(!collides_with_walls(-1.0, 0.0, &walls));
        // Row A gap is open (x: 5 to 7)
        assert!(!collides_with_walls(6.0, -0.75, &walls));
    }

    #[test]
    fn start_position_is_not_in_wall() {
        let walls = wall_rects();
        assert!(!collides_with_walls(PLAYER_SPAWN.x, PLAYER_SPAWN.z, &walls));
    }

    #[test]
    fn trophy_position_is_not_in_wall() {
        let walls = wall_rects();
        assert!(!collides_with_walls(TROPHY_POS.x, TROPHY_POS.z, &walls));
    }

    #[test]
    fn trophy_not_collected_while_hovering_above() {
        // Flying over the walls at a high Y and hovering above the trophy's XZ
        // must NOT collect it — the player has to descend onto it.
        let hovering = Vec3::new(TROPHY_POS.x, 5.0, TROPHY_POS.z);
        assert!(!check_trophy_collected(hovering, TROPHY_POS));

        // Dropping back down onto the trophy collects it.
        let landed = Vec3::new(TROPHY_POS.x, 0.0, TROPHY_POS.z);
        assert!(check_trophy_collected(landed, TROPHY_POS));
    }

    #[test]
    fn debugger_scenario_teleport_to_trophy() {
        // Simulates: player modifies Transform.translation to trophy position
        let teleported_pos = TROPHY_POS;
        assert!(check_trophy_collected(teleported_pos, TROPHY_POS));
    }

    #[test]
    fn maze_has_no_clear_path() {
        // Verify that walking in a straight line from start to trophy hits walls
        let walls = wall_rects();
        let steps = 100;
        let mut hit_wall = false;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let x = PLAYER_SPAWN.x + (TROPHY_POS.x - PLAYER_SPAWN.x) * t;
            let z = PLAYER_SPAWN.z + (TROPHY_POS.z - PLAYER_SPAWN.z) * t;
            if collides_with_walls(x, z, &walls) {
                hit_wall = true;
                break;
            }
        }
        assert!(hit_wall, "There should be walls blocking the direct path");
    }
}
