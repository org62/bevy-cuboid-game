use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;

use crate::player::{
    animate_player, escape_to_menu, player_movement, spawn_player, toggle_pause, MovementBounds,
    Player, PlayerPhysics,
};
use crate::{GamePaused, MazePhase, Screen, Scoreboard};

pub struct Level4Plugin;

impl Plugin for Level4Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::MazeChallenge), setup_maze)
            .add_systems(
                FixedUpdate,
                (player_movement, maze_playing_update)
                    .chain()
                    .run_if(in_state(MazePhase::Playing)),
            )
            .add_systems(
                Update,
                (animate_player, maze_visual_update).run_if(in_state(Screen::MazeChallenge)),
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
struct MazeFollowCam;

#[derive(Component)]
struct Trophy;

#[derive(Component)]
struct FogCube {
    drift: Vec3,
}

#[derive(Component)]
struct BreadcrumbOrb;

#[derive(Component)]
struct MazeHintBox;

#[derive(Component)]
struct MazeHintCloseButton;

#[derive(Component)]
struct OverlayScreen;

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

    // Invisible wall colliders (no visual geometry)
    // We don't spawn meshes for them - they exist only in the collision logic

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

    // Fog cubes
    let fog_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.8, 0.8, 0.9, 0.12),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let fog_mesh = meshes.add(Cuboid::new(1.0, 0.8, 1.0));
    for i in 0..20 {
        let x = ((i * 7 + 3) % 16) as f32 - 2.0;
        let z = ((i * 11 + 5) % 14) as f32 - 12.0;
        let drift = Vec3::new(
            ((i % 3) as f32 - 1.0) * 0.3,
            0.0,
            ((i % 5) as f32 - 2.0) * 0.2,
        );
        commands.spawn((
            Mesh3d(fog_mesh.clone()),
            MeshMaterial3d(fog_mat.clone()),
            Transform::from_xyz(x, 0.6, z),
            FogCube { drift },
            MazeEntity,
        ));
    }

    // Moon light
    commands.spawn((
        DirectionalLight {
            illuminance: 6000.0,
            color: Color::srgb(0.7, 0.75, 0.9),
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -0.8,
            0.3,
            0.0,
        )),
        MazeEntity,
    ));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.4, 0.45, 0.6),
        brightness: 300.0,
    });

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

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(6.0, 12.0, 8.0).looking_at(Vec3::new(6.0, 0.0, -5.0), Vec3::Y),
        MazeFollowCam,
        MazeEntity,
    ));

    // HUD
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(16.0),
                left: Val::Px(16.0),
                ..default()
            },
            MazeEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("[Esc] Menu | WASD Move | Space Jump | [P] Pause"),
                TextFont { font_size: 20.0, ..default() },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
            ));
        });

    // Hint
    if !scoreboard.maze_solved {
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(16.0),
                    right: Val::Px(16.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(14.0)),
                    row_gap: Val::Px(8.0),
                    max_width: Val::Px(280.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.1, 0.08, 0.15, 0.9)),
                BorderRadius::all(Val::Px(10.0)),
                MazeHintBox,
                MazeEntity,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new("Hint"),
                    TextFont { font_size: 20.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.85, 0.3)),
                ));
                parent.spawn((
                    Node { max_width: Val::Px(250.0), ..default() },
                    Text::new("The path is hidden, but your position is not. What if you could simply... be somewhere else? Look for Transform.translation."),
                    TextFont { font_size: 16.0, ..default() },
                    TextColor(Color::srgb(0.85, 0.85, 0.85)),
                ));
                parent
                    .spawn((
                        Node {
                            align_self: AlignSelf::FlexEnd,
                            padding: UiRect::axes(Val::Px(12.0), Val::Px(4.0)),
                            ..default()
                        },
                        Button,
                        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.15)),
                        BorderRadius::all(Val::Px(6.0)),
                        MazeHintCloseButton,
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("[X] Close"),
                            TextFont { font_size: 14.0, ..default() },
                            TextColor(Color::srgb(0.7, 0.7, 0.7)),
                        ));
                    });
            });
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

    // Apply invisible wall collision
    let walls = wall_rects();
    if collides_with_walls(pt.translation.x, pt.translation.z, &walls) {
        // Push player back out of walls
        // Simple: find the nearest non-colliding position by nudging
        let step = 0.1;
        for &(dx, dz) in &[(0.0, step), (0.0, -step), (step, 0.0), (-step, 0.0),
                           (step, step), (step, -step), (-step, step), (-step, -step)] {
            let nx = pt.translation.x + dx;
            let nz = pt.translation.z + dz;
            if !collides_with_walls(nx, nz, &walls) {
                pt.translation.x = nx;
                pt.translation.z = nz;
                break;
            }
        }
    }

    // Check trophy
    if check_trophy_collected(pt.translation, TROPHY_POS) {
        next_phase.set(MazePhase::Victory);
    }
}

// --- Visual ---

#[allow(clippy::too_many_arguments)]
fn maze_visual_update(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    player_q: Query<&Transform, (With<Player>, Without<MazeFollowCam>, Without<FogCube>, Without<Trophy>)>,
    mut camera_q: Query<&mut Transform, (With<MazeFollowCam>, Without<Player>, Without<FogCube>, Without<Trophy>)>,
    mut fog_q: Query<(&mut Transform, &FogCube), (Without<Player>, Without<MazeFollowCam>, Without<Trophy>)>,
    mut trophy_q: Query<&mut Transform, (With<Trophy>, Without<Player>, Without<MazeFollowCam>, Without<FogCube>)>,
    hint_q: Query<Entity, With<MazeHintBox>>,
    btn_q: Query<&Interaction, (Changed<Interaction>, With<MazeHintCloseButton>)>,
) {
    let dt = time.delta_secs();
    let elapsed = time.elapsed_secs();

    // Camera follow
    if let (Ok(pt), Ok(mut ct)) = (player_q.get_single(), camera_q.get_single_mut()) {
        let target = pt.translation + Vec3::new(0.0, 12.0, 8.0);
        let t = (6.0 * dt).min(1.0);
        ct.translation = ct.translation.lerp(target, t);
        ct.look_at(pt.translation + Vec3::Y, Vec3::Y);
    }

    // Fog drift
    for (mut t, fog) in &mut fog_q {
        t.translation += fog.drift * dt;
        t.translation.y = 0.6 + (elapsed * 0.5 + t.translation.x).sin() * 0.2;
    }

    // Trophy bob
    for mut t in &mut trophy_q {
        t.translation.y = 1.0 + (elapsed * 1.5).sin() * 0.15;
        t.rotate_y(1.0 * dt);
    }

    // Hint dismiss
    let should_close = keyboard.just_pressed(KeyCode::KeyX)
        || btn_q.iter().any(|i| *i == Interaction::Pressed);
    if should_close {
        for entity in &hint_q {
            commands.entity(entity).despawn_recursive();
        }
    }
}

// --- Victory ---

fn handle_victory(
    mut commands: Commands,
    mut events: EventReader<KeyboardInput>,
    mut next_phase: ResMut<NextState<MazePhase>>,
    mut scoreboard: ResMut<Scoreboard>,
    mut player_q: Query<(&mut Transform, &mut PlayerPhysics), With<Player>>,
    overlay_q: Query<Entity, With<OverlayScreen>>,
) {
    if overlay_q.is_empty() {
        scoreboard.maze_solved = true;
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
                BackgroundColor(Color::srgba(0.0, 0.15, 0.0, 0.8)),
                GlobalZIndex(10),
                OverlayScreen,
                MazeEntity,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new("TROPHY COLLECTED!"),
                    TextFont { font_size: 52.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.85, 0.2)),
                ));
                parent.spawn((
                    Text::new("Press any key to continue"),
                    TextFont { font_size: 22.0, ..default() },
                    TextColor(Color::srgb(0.6, 0.8, 0.6)),
                ));
            });
    }

    for event in events.read() {
        if !event.state.is_pressed() { continue; }
        for entity in &overlay_q {
            commands.entity(entity).despawn_recursive();
        }
        if let Ok((mut t, mut p)) = player_q.get_single_mut() {
            t.translation = PLAYER_SPAWN;
            p.velocity = Vec3::ZERO;
            p.grounded = true;
        }
        next_phase.set(MazePhase::Playing);
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
