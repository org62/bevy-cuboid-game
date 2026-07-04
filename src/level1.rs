use bevy::prelude::*;

use crate::player::{
    animate_player, escape_to_menu, player_movement, spawn_player, toggle_pause, MovementBounds,
    Player, PlayerMovementSet,
};
use crate::shared_ui;
use crate::{ChallengePhase, GamePaused, Screen, Scoreboard};

const ALLOWED_MIN: Vec2 = Vec2::new(-6.0, -5.0);
const ALLOWED_MAX: Vec2 = Vec2::new(6.0, 5.0);
const PASSWORD_MIN: Vec2 = Vec2::new(6.0, -4.0);
const PASSWORD_MAX: Vec2 = Vec2::new(13.0, 4.0);

/// Long-form walkthrough revealed by the expandable hint box. Two routes,
/// search-and-watchpoint first because it needs no symbols.
const PASSWORD_SOLUTION: &str = "\
Approach 1 - memory search + watchpoint (robust, needs no symbols):
1) Type a 6-character string like \"aaaaaa\". The length must be 6, or check_password's compare loop is skipped entirely.
2) Search memory for those bytes - they live in PasswordInput.text (a heap-allocated String).
3) Set a hardware READ watchpoint on that address (\"find what accesses this address\").
4) Press Enter. check_password reads your bytes one at a time and the watchpoint fires inside the loop.
5) Single-step and read what each byte is compared against: s, e, s, a, m, e. Enter \"sesame\".

Approach 2 - breakpoint on the symbol (quick, but fragile):
1) Set a breakpoint on check_password.
2) Inspect the local \"correct\" - it is b\"sesame\".
3) Type \"sesame\". Note: a stripped or optimized build may not keep the symbol, which is why Approach 1 is preferred.";

pub struct Level1Plugin;

impl Plugin for Level1Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::PasswordChallenge), setup_world)
            .add_systems(
                Update,
                (
                    shared_ui::update_camera_orbit.before(PlayerMovementSet),
                    player_movement.in_set(PlayerMovementSet),
                    detect_zone,
                )
                    .run_if(in_state(ChallengePhase::Exploring)),
            )
            .add_systems(
                Update,
                (escape_to_menu, toggle_pause).run_if(in_state(ChallengePhase::Exploring)),
            )
            .add_systems(
                Update,
                (
                    shared_ui::follow_camera_system,
                    animate_player,
                    animate_barriers,
                    shared_ui::hint_tutorial_controls,
                )
                    .after(PlayerMovementSet)
                    .run_if(in_state(Screen::PasswordChallenge)),
            )
            .add_systems(OnExit(Screen::PasswordChallenge), cleanup_world);
    }
}

// --- Components ---

#[derive(Component)]
pub struct WorldEntity;

#[derive(Component)]
struct Barrier;

// --- Resources ---

#[derive(Resource)]
struct BarrierState {
    lowered: bool,
    offset_y: f32,
}

#[derive(Resource)]
struct ZoneGroundMaterial(Handle<StandardMaterial>);

// --- Setup ---

fn setup_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    scoreboard: Res<Scoreboard>,
) {
    commands.insert_resource(ClearColor(Color::srgb(0.55, 0.75, 0.95)));
    commands.insert_resource(BarrierState {
        lowered: scoreboard.is_solved(1),
        offset_y: if scoreboard.is_solved(1) { -1.5 } else { 0.0 },
    });

    // Main ground (pastel green)
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(12.0, 10.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.4, 0.75, 0.35),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
        WorldEntity,
    ));

    // Restricted zone ground
    let zone_color = if scoreboard.is_solved(1) {
        Color::srgb(0.35, 0.7, 0.35)
    } else {
        Color::srgb(0.7, 0.25, 0.2)
    };
    let zone_mat = materials.add(StandardMaterial {
        base_color: zone_color,
        ..default()
    });
    commands.insert_resource(ZoneGroundMaterial(zone_mat.clone()));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(7.0, 8.0))),
        MeshMaterial3d(zone_mat),
        Transform::from_xyz(9.5, 0.001, 0.0),
        WorldEntity,
    ));

    // Player
    let player = spawn_player(&mut commands, &mut meshes, &mut materials);
    commands.entity(player).insert((
        WorldEntity,
        MovementBounds {
            rects: vec![(ALLOWED_MIN, ALLOWED_MAX), (PASSWORD_MIN, PASSWORD_MAX)],
        },
    ));

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 8.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        shared_ui::FollowCamera {
            offset: Vec3::new(0.0, 8.0, 8.0),
            lerp_speed: 8.0,
            look_offset: Vec3::Y,
        },
        WorldEntity,
    ));

    // Sun (large yellow sphere in sky)
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(3.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.95, 0.4),
            emissive: LinearRgba::new(1.0, 0.9, 0.3, 1.0),
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(10.0, 20.0, -15.0),
        WorldEntity,
    ));

    // Lighting
    shared_ui::setup_level_lighting(
        &mut commands,
        10000.0,
        (-std::f32::consts::FRAC_PI_4, std::f32::consts::FRAC_PI_6, 0.0),
        Color::srgb(0.95, 0.9, 0.8),
        400.0,
        WorldEntity,
    );

    // Environment
    spawn_barriers(&mut commands, &mut meshes, &mut materials, &scoreboard);

    // HUD - minimal legend + agenda (full controls) + settings dialogs
    shared_ui::spawn_controls_legend_min(&mut commands, WorldEntity);
    shared_ui::spawn_agenda_modal(
        &mut commands,
        "Esc     Close / Menu\nWASD    Move\nSpace   Jump\nP       Pause\nC       Controls\nE       Settings\nH       Hint\nT       Tutorial\nMouse   Look\nWheel   Zoom",
        "Select   Close / Menu\nL-Stick  Move\nA        Jump\nStart    Pause\nR-Stick  Look\nLT / RT  Zoom\nC        Controls\nE        Settings",
        WorldEntity,
    );
    shared_ui::spawn_settings_modal(&mut commands, WorldEntity);
    shared_ui::spawn_objective(&mut commands, "Enter the restricted area", WorldEntity);

    // HUD - debugger hint box (top right) + centered tutorial modal.
    // Spawned regardless of solved state (both start hidden); `H`/`T` reveal them.
    shared_ui::spawn_hint_box_with_tutorial(
        &mut commands,
        "Use the debugger to find the password. Open the Tutorial for a full walkthrough.",
        300.0,
        WorldEntity,
    );
    shared_ui::spawn_hint_modal(
        &mut commands,
        "Password - Full Solution",
        PASSWORD_SOLUTION,
        WorldEntity,
    );
}

// --- Environment helpers ---

fn spawn_barriers(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    scoreboard: &Res<Scoreboard>,
) {
    if scoreboard.is_solved(1) {
        return;
    }

    let red_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.2, 0.15),
        ..default()
    });
    let white_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.95, 0.95),
        ..default()
    });
    // Square pillars with a square cap.
    let post_mesh = meshes.add(Cuboid::new(0.24, 1.0, 0.24));
    let cap_mesh = meshes.add(Cuboid::new(0.34, 0.16, 0.34));

    // Evenly spaced pillars along the restricted-zone entrance (x = 5.5),
    // from z = -4.5 to z = 4.5.
    const POST_COUNT: usize = 7;
    const Z_START: f32 = -4.5;
    const Z_END: f32 = 4.5;
    let px = 5.5;

    for i in 0..POST_COUNT {
        let t = i as f32 / (POST_COUNT as f32 - 1.0);
        let pz = Z_START + (Z_END - Z_START) * t;
        // Square post
        commands.spawn((
            Mesh3d(post_mesh.clone()),
            MeshMaterial3d(red_mat.clone()),
            Transform::from_xyz(px, 0.5, pz),
            Barrier,
            WorldEntity,
        ));
        // Square cap
        commands.spawn((
            Mesh3d(cap_mesh.clone()),
            MeshMaterial3d(white_mat.clone()),
            Transform::from_xyz(px, 1.05, pz),
            Barrier,
            WorldEntity,
        ));
    }
}

// --- Systems ---

fn detect_zone(
    player_query: Query<&Transform, With<Player>>,
    mut next_phase: ResMut<NextState<ChallengePhase>>,
    scoreboard: Res<Scoreboard>,
    game_paused: Res<GamePaused>,
) {
    if game_paused.0 {
        return;
    }
    if scoreboard.is_solved(1) {
        return;
    }
    let Ok(transform) = player_query.get_single() else {
        return;
    };
    let pos = transform.translation;
    if pos.x >= PASSWORD_MIN.x && pos.x <= PASSWORD_MAX.x
        && pos.z >= PASSWORD_MIN.y && pos.z <= PASSWORD_MAX.y
    {
        next_phase.set(ChallengePhase::PasswordPrompt);
    }
}

fn animate_barriers(
    mut barrier_state: ResMut<BarrierState>,
    scoreboard: Res<Scoreboard>,
    time: Res<Time>,
    mut barrier_query: Query<&mut Transform, With<Barrier>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    zone_mat: Res<ZoneGroundMaterial>,
) {
    if !scoreboard.is_solved(1) || barrier_state.lowered {
        return;
    }

    let delta = 2.0 * time.delta_secs();
    barrier_state.offset_y -= delta;
    if barrier_state.offset_y <= -1.5 {
        barrier_state.offset_y = -1.5;
        barrier_state.lowered = true;
    }

    for mut t in &mut barrier_query {
        t.translation.y -= delta;
    }

    // Lerp zone color from red to green
    if let Some(mat) = materials.get_mut(&zone_mat.0) {
        let progress = (-barrier_state.offset_y / 1.5).min(1.0);
        let r = 0.7 * (1.0 - progress) + 0.35 * progress;
        let g = 0.25 * (1.0 - progress) + 0.7 * progress;
        let b = 0.2 * (1.0 - progress) + 0.35 * progress;
        mat.base_color = Color::srgb(r, g, b);
    }
}

fn cleanup_world(mut commands: Commands, query: Query<Entity, With<WorldEntity>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}
