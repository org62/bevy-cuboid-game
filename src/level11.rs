use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;

use crate::player::{
    animate_player, escape_to_menu, player_movement, spawn_player, toggle_pause, MovementBounds,
    Player, PlayerMovementSet,
};
use crate::shared_ui;
use crate::{ClonePhase, GamePaused, Screen, Scoreboard};

pub struct Level11Plugin;

impl Plugin for Level11Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::CloneChallenge), setup_clone)
            .add_systems(
                Update,
                (player_movement.in_set(PlayerMovementSet), clone_playing_update)
                    .chain()
                    .run_if(in_state(ClonePhase::Playing)),
            )
            .add_systems(
                Update,
                (animate_player, shared_ui::dismiss_hint, shared_ui::follow_camera_system)
                    .after(PlayerMovementSet)
                    .run_if(in_state(Screen::CloneChallenge)),
            )
            .add_systems(
                Update,
                (escape_to_menu, toggle_pause).run_if(in_state(ClonePhase::Playing)),
            )
            .add_systems(
                Update,
                handle_victory.run_if(in_state(ClonePhase::Victory)),
            )
            .add_systems(OnExit(Screen::CloneChallenge), cleanup_clone);
    }
}

// --- Components ---

#[derive(Component)]
struct CloneEntity;

#[derive(Component)]
struct DarkClone;

#[repr(C)]
#[derive(Component)]
pub(crate) struct CloneData {
    mirror_axis: f32,
    pub(crate) invincible: bool,
    _decoy_flags: [bool; 7],
    trapped: bool,
}

#[derive(Component)]
struct TrapZoneVisual;

#[derive(Component)]
struct MirrorWall;


// --- Constants ---

const MIRROR_X: f32 = 0.0;
const TRAP_ZONE_MIN: Vec2 = Vec2::new(8.0, -3.0);
const TRAP_ZONE_MAX: Vec2 = Vec2::new(12.0, 3.0);
const ARENA_MIN: Vec2 = Vec2::new(-12.0, -6.0);
const ARENA_MAX: Vec2 = Vec2::new(-0.5, 6.0); // Player can only go on left side
const PLAYER_SPAWN: Vec3 = Vec3::new(-5.0, 0.0, 0.0);

// --- Debugger-target functions ---

#[inline(never)]
fn check_clone_trapped(clone_pos: Vec3, clone: &mut CloneData) -> bool {
    let in_zone = clone_pos.x >= TRAP_ZONE_MIN.x
        && clone_pos.x <= TRAP_ZONE_MAX.x
        && clone_pos.z >= TRAP_ZONE_MIN.y
        && clone_pos.z <= TRAP_ZONE_MAX.y;
    if in_zone && !clone.invincible {
        clone.trapped = true;
        return true;
    }
    false
}

#[inline(never)]
fn mirror_player_position(player_pos: Vec3, mirror_x: f32) -> Vec3 {
    Vec3::new(2.0 * mirror_x - player_pos.x, player_pos.y, player_pos.z)
}

// --- Setup ---

fn setup_clone(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    scoreboard: Res<Scoreboard>,
) {
    commands.insert_resource(ClearColor(Color::srgb(0.12, 0.12, 0.15)));

    // Left side floor (white tiles)
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(12.0, 12.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.85, 0.85, 0.9),
            ..default()
        })),
        Transform::from_xyz(-6.0, 0.0, 0.0),
        CloneEntity,
    ));

    // Right side floor (dark tiles)
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(12.0, 12.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.15, 0.15, 0.18),
            ..default()
        })),
        Transform::from_xyz(6.0, 0.0, 0.0),
        CloneEntity,
    ));

    // Mirror wall (translucent blue)
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.1, 3.0, 12.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.3, 0.5, 0.9, 0.3),
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::from_xyz(MIRROR_X, 1.5, 0.0),
        MirrorWall,
        CloneEntity,
    ));

    // Trap zone (glowing red floor patch on clone's side)
    let trap_center_x = (TRAP_ZONE_MIN.x + TRAP_ZONE_MAX.x) / 2.0;
    let trap_center_z = (TRAP_ZONE_MIN.y + TRAP_ZONE_MAX.y) / 2.0;
    let trap_w = TRAP_ZONE_MAX.x - TRAP_ZONE_MIN.x;
    let trap_h = TRAP_ZONE_MAX.y - TRAP_ZONE_MIN.y;
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(trap_w, trap_h))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.9, 0.15, 0.1, 0.5),
            emissive: LinearRgba::new(1.5, 0.2, 0.1, 1.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(trap_center_x, 0.01, trap_center_z),
        TrapZoneVisual,
        CloneEntity,
    ));

    // Dark clone (gray cube with red eyes)
    commands
        .spawn((
            Mesh3d(meshes.add(Cuboid::new(0.6, 1.4, 0.5))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.25, 0.25, 0.28),
                ..default()
            })),
            Transform::from_xyz(5.0, 0.7, 0.0),
            DarkClone,
            CloneData {
                mirror_axis: MIRROR_X,
                invincible: true,
                _decoy_flags: [false; 7],
                trapped: false,
            },
            CloneEntity,
        ))
        .with_children(|parent| {
            // Red eyes
            let eye_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.1, 0.05),
                emissive: LinearRgba::new(2.0, 0.1, 0.05, 1.0),
                ..default()
            });
            let eye_mesh = meshes.add(Sphere::new(0.06));
            parent.spawn((
                Mesh3d(eye_mesh.clone()),
                MeshMaterial3d(eye_mat.clone()),
                Transform::from_xyz(-0.12, 0.35, -0.26),
            ));
            parent.spawn((
                Mesh3d(eye_mesh),
                MeshMaterial3d(eye_mat),
                Transform::from_xyz(0.12, 0.35, -0.26),
            ));
        });

    // Player
    let player = spawn_player(&mut commands, &mut meshes, &mut materials);
    commands.entity(player).insert((
        CloneEntity,
        Transform::from_xyz(PLAYER_SPAWN.x, 0.0, PLAYER_SPAWN.z)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        MovementBounds {
            rects: vec![(ARENA_MIN, ARENA_MAX)],
        },
    ));

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 12.0, 12.0).looking_at(Vec3::ZERO, Vec3::Y),
        shared_ui::FollowCamera { offset: Vec3::new(0.0, 8.0, 8.0), lerp_speed: 8.0, look_offset: Vec3::Y },
        CloneEntity,
    ));

    // Lighting
    shared_ui::setup_level_lighting(
        &mut commands,
        6000.0,
        (-0.7, 0.3, 0.0),
        Color::srgb(0.6, 0.6, 0.7),
        200.0,
        CloneEntity,
    );

    // HUD
    shared_ui::spawn_controls_hint(&mut commands, "[Esc] Menu | WASD Move | Space Jump | [P] Pause", CloneEntity);

    // Hint
    if !scoreboard.is_solved(11) {
        shared_ui::spawn_hint_box(
            &mut commands,
            "Your shadow cannot be harmed... or can it? When the clone enters the red zone, something blocks the trap. Break on check_clone_trapped().",
            280.0,
            CloneEntity,
        );
    }
}

// --- Gameplay ---

fn clone_playing_update(
    mut next_phase: ResMut<NextState<ClonePhase>>,
    player_q: Query<&Transform, (With<Player>, Without<DarkClone>)>,
    mut clone_q: Query<(&mut Transform, &mut CloneData), (With<DarkClone>, Without<Player>)>,
    game_paused: Res<GamePaused>,
) {
    if game_paused.0 { return; }
    let Ok(pt) = player_q.get_single() else { return; };

    for (mut ct, mut clone_data) in &mut clone_q {
        // Mirror player position
        let mirrored = mirror_player_position(pt.translation, clone_data.mirror_axis);
        ct.translation.x = mirrored.x;
        ct.translation.z = mirrored.z;

        // Check trap
        if check_clone_trapped(ct.translation, &mut clone_data) {
            next_phase.set(ClonePhase::Victory);
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
        scoreboard.set_solved(11);
        shared_ui::spawn_victory_overlay(
            &mut commands,
            "CLONE TRAPPED!",
            None,
            24.0,
            "Press any key to continue",
            CloneEntity,
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

fn cleanup_clone(mut commands: Commands, query: Query<Entity, With<CloneEntity>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clone_mirrors_player_position() {
        let player_pos = Vec3::new(-5.0, 1.0, 3.0);
        let mirrored = mirror_player_position(player_pos, MIRROR_X);
        // mirror_x = 0.0, so mirrored.x = 2*0 - (-5) = 5.0
        assert!((mirrored.x - 5.0).abs() < 0.001);
        assert!((mirrored.y - 1.0).abs() < 0.001);
        assert!((mirrored.z - 3.0).abs() < 0.001);
    }

    #[test]
    fn mirror_preserves_z() {
        let pos = Vec3::new(-3.0, 0.0, 2.0);
        let mirrored = mirror_player_position(pos, 0.0);
        assert!((mirrored.z - pos.z).abs() < 0.001);
    }

    #[test]
    fn clone_invincible_by_default() {
        let clone = CloneData {
            mirror_axis: MIRROR_X,
            invincible: true,
            _decoy_flags: [false; 7],
            trapped: false,
        };
        assert!(clone.invincible);
        // Decoys are all false
        assert!(clone._decoy_flags.iter().all(|&f| !f));
    }

    #[test]
    fn invincible_clone_cannot_be_trapped() {
        let mut clone = CloneData {
            mirror_axis: MIRROR_X,
            invincible: true,
            _decoy_flags: [false; 7],
            trapped: false,
        };
        // Place clone in trap zone
        let clone_pos = Vec3::new(10.0, 0.0, 0.0);
        assert!(!check_clone_trapped(clone_pos, &mut clone));
        assert!(!clone.trapped);
    }

    #[test]
    fn vulnerable_clone_trapped_in_zone() {
        let mut clone = CloneData {
            mirror_axis: MIRROR_X,
            invincible: false, // debugger flipped this
            _decoy_flags: [false; 7],
            trapped: false,
        };
        // Place clone in trap zone
        let clone_pos = Vec3::new(10.0, 0.0, 0.0);
        assert!(check_clone_trapped(clone_pos, &mut clone));
        assert!(clone.trapped);
    }

    #[test]
    fn clone_outside_trap_zone_not_trapped() {
        let mut clone = CloneData {
            mirror_axis: MIRROR_X,
            invincible: false,
            _decoy_flags: [false; 7],
            trapped: false,
        };
        let clone_pos = Vec3::new(5.0, 0.0, 0.0); // outside trap zone
        assert!(!check_clone_trapped(clone_pos, &mut clone));
    }

    #[test]
    fn debugger_scenario_flip_invincible() {
        // Step 1: Find and flip the invincible flag (not the decoys!)
        let mut clone = CloneData {
            mirror_axis: MIRROR_X,
            invincible: true,
            _decoy_flags: [false; 7],
            trapped: false,
        };
        clone.invincible = false; // debugger flips this

        // Step 2: Maneuver clone into trap zone
        // Player at (-10, 0, 0) -> clone at (10, 0, 0) which is in trap zone
        let player_pos = Vec3::new(-10.0, 0.0, 0.0);
        let clone_pos = mirror_player_position(player_pos, MIRROR_X);
        assert!((clone_pos.x - 10.0).abs() < 0.001);

        assert!(check_clone_trapped(clone_pos, &mut clone));
        assert!(clone.trapped);
    }

    #[test]
    fn trap_zone_boundaries() {
        let mut clone = CloneData {
            mirror_axis: MIRROR_X,
            invincible: false,
            _decoy_flags: [false; 7],
            trapped: false,
        };

        // Inside trap zone
        assert!(check_clone_trapped(Vec3::new(10.0, 0.0, 0.0), &mut clone));
        clone.trapped = false; // reset

        // On boundary (min)
        assert!(check_clone_trapped(Vec3::new(8.0, 0.0, -3.0), &mut clone));
        clone.trapped = false;

        // Outside (too far left)
        assert!(!check_clone_trapped(Vec3::new(7.0, 0.0, 0.0), &mut clone));

        // Outside (too far right)
        assert!(!check_clone_trapped(Vec3::new(13.0, 0.0, 0.0), &mut clone));
    }

    #[test]
    fn decoy_flags_are_all_false() {
        // The one true bool is `invincible`, decoys are false
        let clone = CloneData {
            mirror_axis: MIRROR_X,
            invincible: true,
            _decoy_flags: [false; 7],
            trapped: false,
        };
        // Player must identify invincible (true) among 7 false decoys
        let all_bools: Vec<bool> = std::iter::once(clone.invincible)
            .chain(clone._decoy_flags.iter().copied())
            .collect();
        let true_count = all_bools.iter().filter(|&&b| b).count();
        assert_eq!(true_count, 1, "Only invincible should be true");
    }
}
