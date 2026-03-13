use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;

use crate::player::{
    animate_player, escape_to_menu, player_movement, spawn_player, toggle_pause, MovementBounds,
    Player, PlayerMovementSet,
};
use crate::shared_ui;
use crate::{GamePaused, MazePhase, Screen, Scoreboard};

pub struct Level4Plugin;

impl Plugin for Level4Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::MazeChallenge), setup_maze)
            .add_systems(
                Update,
                (player_movement.in_set(PlayerMovementSet), maze_playing_update)
                    .chain()
                    .run_if(in_state(MazePhase::Playing)),
            )
            .add_systems(
                Update,
                (animate_player, maze_visual_update, shared_ui::dismiss_hint, shared_ui::follow_camera_system)
                    .after(PlayerMovementSet)
                    .run_if(in_state(Screen::MazeChallenge)),
            )
            .add_systems(
                Update,
                (escape_to_menu, toggle_pause).run_if(in_state(MazePhase::Playing)),
            )
            .add_systems(
                Update,
                handle_victory.run_if(in_state(MazePhase::Victory)),
            )
            .add_systems(OnExit(Screen::MazeChallenge), cleanup_maze);
    }
}

// --- Components ---

#[derive(Component)]
struct MazeEntity;

#[derive(Component)]
struct Trophy;

#[derive(Component)]
struct BreadcrumbOrb;

#[derive(Component)]
struct WallVisual;

#[derive(Resource, Default)]
struct WallsVisible(bool);

// --- Constants ---

const TROPHY_POS: Vec3 = Vec3::new(12.0, 0.5, -10.0);
const ARENA_MIN: Vec2 = Vec2::new(-2.0, -12.0);
const ARENA_MAX: Vec2 = Vec2::new(14.0, 2.0);
const PLAYER_SPAWN: Vec3 = Vec3::new(-1.0, 0.0, 1.0);

// --- Debugger-target functions ---

#[inline(never)]
fn check_trophy_collected(player_pos: Vec3, trophy_pos: Vec3) -> bool {
    let dx = player_pos.x - trophy_pos.x;
    let dz = player_pos.z - trophy_pos.z;
    (dx * dx + dz * dz) < 2.0
}

// --- Invisible wall collision ---

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
    let radius = 0.4;
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

    // Dark green ground
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(20.0, 16.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.12, 0.22, 0.1),
            ..default()
        })),
        Transform::from_xyz(6.0, 0.0, -5.0),
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

    // Wall visuals (hidden by default, toggled with ?)
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
            Mesh3d(meshes.add(Cuboid::new(w, 1.5, h))),
            MeshMaterial3d(wall_visual_mat.clone()),
            Transform::from_xyz(cx, 0.75, cz),
            Visibility::Hidden,
            WallVisual,
            MazeEntity,
        ));
    }

    // Breadcrumb orbs at dead ends
    let dead_end_positions = [
        Vec3::new(8.5, 0.4, -2.0),   // dead end near trap wall after Row A
        Vec3::new(13.0, 0.4, -5.5),  // dead end in right corridor between B and C
        Vec3::new(-1.0, 0.4, -8.0),  // dead end left of Row C
        Vec3::new(4.0, 0.4, -8.0),   // dead end in false corridor near trap
    ];
    let red_orb_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.15, 0.1),
        emissive: LinearRgba::new(0.5, 0.1, 0.05, 1.0),
        ..default()
    });
    let orb_mesh = meshes.add(Sphere::new(0.15));
    for pos in &dead_end_positions {
        commands.spawn((
            Mesh3d(orb_mesh.clone()),
            MeshMaterial3d(red_orb_mat.clone()),
            Transform::from_translation(*pos),
            BreadcrumbOrb,
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
            lerp_speed: 6.0,
            look_offset: Vec3::Y,
        },
        MazeEntity,
    ));

    // HUD
    shared_ui::spawn_controls_hint(
        &mut commands,
        "[Esc] Menu | WASD Move | Space Jump | [P] Pause",
        MazeEntity,
    );

    // Hint
    if !scoreboard.is_solved(4) {
        shared_ui::spawn_hint_box(
            &mut commands,
            "The path is hidden, but your position is not. What if you could simply... be somewhere else? Look for Transform.translation.",
            280.0,
            MazeEntity,
        );
    }
}

// --- Gameplay ---

fn maze_playing_update(
    mut next_phase: ResMut<NextState<MazePhase>>,
    mut player_q: Query<&mut Transform, With<Player>>,
    game_paused: Res<GamePaused>,
) {
    if game_paused.0 { return; }
    let Ok(mut pt) = player_q.get_single_mut() else {
        return;
    };

    // Apply invisible wall collision — push player out by minimum penetration
    let walls = wall_rects();
    let radius = 0.4;
    for w in &walls {
        let px = pt.translation.x;
        let pz = pt.translation.z;
        if px + radius > w.min.x && px - radius < w.max.x
            && pz + radius > w.min.y && pz - radius < w.max.y
        {
            let pen_left = px + radius - w.min.x;
            let pen_right = w.max.x - (px - radius);
            let pen_top = pz + radius - w.min.y;
            let pen_bottom = w.max.y - (pz - radius);
            let min_pen = pen_left.min(pen_right).min(pen_top).min(pen_bottom);

            if min_pen == pen_left {
                pt.translation.x = w.min.x - radius;
            } else if min_pen == pen_right {
                pt.translation.x = w.max.x + radius;
            } else if min_pen == pen_top {
                pt.translation.z = w.min.y - radius;
            } else {
                pt.translation.z = w.max.y + radius;
            }
        }
    }

    // Check trophy
    if check_trophy_collected(pt.translation, TROPHY_POS) {
        next_phase.set(MazePhase::Victory);
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

// --- Victory ---

fn handle_victory(
    mut commands: Commands,
    mut events: EventReader<KeyboardInput>,
    mut next_screen: ResMut<NextState<Screen>>,
    mut scoreboard: ResMut<Scoreboard>,
    overlay_q: Query<Entity, With<shared_ui::OverlayScreen>>,
) {
    if overlay_q.is_empty() {
        scoreboard.set_solved(4);
        shared_ui::spawn_victory_overlay(
            &mut commands,
            "TROPHY COLLECTED!",
            None,
            0.0,
            "Press any key to continue",
            MazeEntity,
        );
    }

    for event in events.read() {
        if !event.state.is_pressed() { continue; }
        for entity in &overlay_q {
            commands.entity(entity).despawn_recursive();
        }
        next_screen.set(Screen::Menu);
        return;
    }
}

// --- Cleanup ---

fn cleanup_maze(mut commands: Commands, query: Query<Entity, With<MazeEntity>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
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
