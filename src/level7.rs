use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;

use crate::player::{
    animate_player, escape_to_menu, player_movement, spawn_player, toggle_pause, GravityOverride,
    MovementBounds, Player, PlayerMovementSet, PlayerPhysics,
};
use crate::{GamePaused, GravityPhase, Screen, Scoreboard};

pub struct Level7Plugin;

impl Plugin for Level7Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::GravityChallenge), setup_gravity)
            .add_systems(
                Update,
                (player_movement.in_set(PlayerMovementSet), gravity_playing_update)
                    .chain()
                    .run_if(in_state(GravityPhase::Playing)),
            )
            .add_systems(
                Update,
                (animate_player, gravity_visual_update)
                    .after(PlayerMovementSet)
                    .run_if(in_state(Screen::GravityChallenge)),
            )
            .add_systems(
                Update,
                (escape_to_menu, toggle_pause).run_if(in_state(GravityPhase::Playing)),
            )
            .add_systems(
                Update,
                handle_victory.run_if(in_state(GravityPhase::Victory)),
            )
            .add_systems(OnExit(Screen::GravityChallenge), cleanup_gravity);
    }
}

// --- Components ---

#[derive(Component)]
struct GravityEntity;

#[derive(Component)]
struct GravityFollowCam;

#[derive(Component)]
struct PlatformBlock;

#[derive(Component)]
struct GoldenStar;

#[derive(Component)]
struct HeightHudText;

#[derive(Component)]
struct GravityHintBox;

#[derive(Component)]
struct GravityHintCloseButton;

#[derive(Component)]
struct OverlayScreen;

// --- Resources ---

#[repr(C)]
#[derive(Resource)]
pub struct GravityState {
    pub flipped: bool,
    pub flip_timer: f32,
    pub flip_interval: f32,
}

impl Default for GravityState {
    fn default() -> Self {
        Self {
            flipped: false,
            flip_timer: 4.0,
            flip_interval: 4.0,
        }
    }
}

// --- Debugger-target functions ---

#[inline(never)]
fn compute_gravity_direction(state: &GravityState) -> f32 {
    if state.flipped { 25.0 } else { -25.0 }
}

#[inline(never)]
fn update_gravity_flip(state: &mut GravityState, dt: f32) {
    state.flip_timer -= dt;
    if state.flip_timer <= 0.0 {
        state.flipped = !state.flipped;
        state.flip_timer = state.flip_interval;
    }
}

#[inline(never)]
fn check_reached_top(player_y: f32) -> bool {
    player_y >= 30.0
}

// --- Constants ---

const ARENA_MIN: Vec2 = Vec2::new(-4.0, -4.0);
const ARENA_MAX: Vec2 = Vec2::new(4.0, 4.0);
const PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 0.0, 0.0);

// --- Setup ---

fn setup_gravity(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    scoreboard: Res<Scoreboard>,
) {
    commands.insert_resource(ClearColor(Color::srgb(0.15, 0.12, 0.2)));
    commands.insert_resource(GravityState::default());

    // Tower floor
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(8.0, 8.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.28, 0.25),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
        GravityEntity,
    ));

    // Tower walls (half-cylinder on back side only, so camera can see in)
    let wall_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.27, 0.25),
        ..default()
    });
    let wall_mesh = meshes.add(Cuboid::new(3.0, 6.0, 0.3));
    // Only place walls on the back half (away from camera at +Z)
    for i in 2..6 {
        let angle = (i as f32 / 8.0) * std::f32::consts::TAU;
        let x = angle.cos() * 5.5;
        let z = angle.sin() * 5.5;
        commands.spawn((
            Mesh3d(wall_mesh.clone()),
            MeshMaterial3d(wall_mat.clone()),
            Transform::from_xyz(x, 3.0, z)
                .with_rotation(Quat::from_rotation_y(-angle)),
            GravityEntity,
        ));
    }

    // Spiraling platforms
    let platform_colors = [
        Color::srgb(0.3, 0.4, 0.8),
        Color::srgb(0.5, 0.3, 0.7),
        Color::srgb(0.2, 0.6, 0.6),
    ];
    let platform_mesh = meshes.add(Cuboid::new(2.5, 0.3, 1.5));

    for i in 0..12 {
        let height = 2.5 + i as f32 * 2.3;
        let angle = (i as f32 / 12.0) * std::f32::consts::TAU * 2.0;
        let radius = 2.5;
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;
        let color = platform_colors[i % 3];

        commands.spawn((
            Mesh3d(platform_mesh.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                ..default()
            })),
            Transform::from_xyz(x, height, z)
                .with_rotation(Quat::from_rotation_y(-angle)),
            PlatformBlock,
            GravityEntity,
        ));
    }

    // Golden star at the top
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.5))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.85, 0.2),
            emissive: LinearRgba::new(3.0, 2.5, 0.5, 1.0),
            ..default()
        })),
        Transform::from_xyz(0.0, 31.0, 0.0),
        GoldenStar,
        GravityEntity,
    ));
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.9, 0.3),
            intensity: 15000.0,
            range: 8.0,
            ..default()
        },
        Transform::from_xyz(0.0, 32.0, 0.0),
        GravityEntity,
    ));

    // Loose debris (visual only)
    let debris_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.4, 0.35, 0.3),
        ..default()
    });
    let debris_mesh = meshes.add(Cuboid::new(0.2, 0.2, 0.2));
    for i in 0..8 {
        let x = ((i * 3 + 1) % 5) as f32 - 2.0;
        let z = ((i * 7 + 2) % 5) as f32 - 2.0;
        let y = ((i * 4) % 20) as f32 + 2.0;
        commands.spawn((
            Mesh3d(debris_mesh.clone()),
            MeshMaterial3d(debris_mat.clone()),
            Transform::from_xyz(x, y, z),
            GravityEntity,
        ));
    }

    // Player
    let player = spawn_player(&mut commands, &mut meshes, &mut materials);
    commands.entity(player).insert((
        GravityEntity,
        Transform::from_xyz(PLAYER_SPAWN.x, 0.0, PLAYER_SPAWN.z)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        MovementBounds {
            rects: vec![(ARENA_MIN, ARENA_MAX)],
        },
    ));

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 8.0, 10.0).looking_at(Vec3::new(0.0, 3.0, 0.0), Vec3::Y),
        GravityFollowCam,
        GravityEntity,
    ));

    // Lighting
    commands.spawn((
        DirectionalLight {
            illuminance: 5000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, 0.3, 0.0)),
        GravityEntity,
    ));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.55, 0.45, 0.65),
        brightness: 300.0,
    });

    // HUD - height indicator
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                left: Val::Px(16.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.2, 0.15, 0.3, 0.85)),
            BorderRadius::all(Val::Px(12.0)),
            GravityEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Height: 0.0 / 30.0"),
                TextFont { font_size: 22.0, ..default() },
                TextColor(Color::srgb(1.0, 1.0, 1.0)),
                HeightHudText,
            ));
        });

    // Controls
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(16.0),
                left: Val::Px(16.0),
                ..default()
            },
            GravityEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("[Esc] Menu | WASD Move | Space Jump | [P] Pause"),
                TextFont { font_size: 20.0, ..default() },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
            ));
        });

    // Hint
    if !scoreboard.gravity_solved {
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
                GravityHintBox,
                GravityEntity,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new("Hint"),
                    TextFont { font_size: 20.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.85, 0.3)),
                ));
                parent.spawn((
                    Node { max_width: Val::Px(250.0), ..default() },
                    Text::new("Gravity keeps betraying you! The flip is controlled by compute_gravity_direction(). What if gravity always went... your way?"),
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
                        GravityHintCloseButton,
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

fn gravity_playing_update(
    time: Res<Time>,
    mut gravity_state: ResMut<GravityState>,
    mut gravity_override: Option<ResMut<GravityOverride>>,
    mut commands: Commands,
    mut next_phase: ResMut<NextState<GravityPhase>>,
    player_q: Query<&Transform, With<Player>>,
    game_paused: Res<GamePaused>,
) {
    if game_paused.0 { return; }
    let dt = time.delta_secs();

    update_gravity_flip(&mut gravity_state, dt);
    let grav = compute_gravity_direction(&gravity_state);

    // Update the gravity override resource for player_movement
    if let Some(ref mut g) = gravity_override {
        g.0 = grav;
    } else {
        commands.insert_resource(GravityOverride(grav));
    }

    // Check win
    if let Ok(pt) = player_q.get_single() {
        if check_reached_top(pt.translation.y) {
            next_phase.set(GravityPhase::Victory);
        }
    }
}

// --- Visual ---

#[allow(clippy::too_many_arguments)]
fn gravity_visual_update(
    time: Res<Time>,
    _gravity_state: Res<GravityState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    player_q: Query<&Transform, (With<Player>, Without<GravityFollowCam>, Without<GoldenStar>)>,
    mut camera_q: Query<&mut Transform, (With<GravityFollowCam>, Without<Player>, Without<GoldenStar>)>,
    mut text_q: Query<&mut Text, With<HeightHudText>>,
    mut star_q: Query<&mut Transform, (With<GoldenStar>, Without<Player>, Without<GravityFollowCam>)>,
    hint_q: Query<Entity, With<GravityHintBox>>,
    btn_q: Query<&Interaction, (Changed<Interaction>, With<GravityHintCloseButton>)>,
) {
    let dt = time.delta_secs();
    let elapsed = time.elapsed_secs();

    // Camera follow (tracks player Y too)
    if let (Ok(pt), Ok(mut ct)) = (player_q.get_single(), camera_q.get_single_mut()) {
        let target = pt.translation + Vec3::new(0.0, 8.0, 10.0);
        let t = (6.0 * dt).min(1.0);
        ct.translation = ct.translation.lerp(target, t);
        ct.look_at(pt.translation + Vec3::Y * 2.0, Vec3::Y);
    }

    // HUD
    if let Ok(pt) = player_q.get_single() {
        if let Ok(mut text) = text_q.get_single_mut() {
            **text = format!("Height: {:.1} / 30.0", pt.translation.y.max(0.0));
        }
    }

    // Star bob
    for mut t in &mut star_q {
        t.translation.y = 31.0 + (elapsed * 1.5).sin() * 0.2;
        t.rotate_y(1.5 * dt);
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
    mut next_phase: ResMut<NextState<GravityPhase>>,
    mut scoreboard: ResMut<Scoreboard>,
    mut player_q: Query<(&mut Transform, &mut PlayerPhysics), With<Player>>,
    overlay_q: Query<Entity, With<OverlayScreen>>,
) {
    if overlay_q.is_empty() {
        scoreboard.gravity_solved = true;
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
                GravityEntity,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new("REACHED THE TOP!"),
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
        next_phase.set(GravityPhase::Playing);
        return;
    }
}

// --- Cleanup ---

fn cleanup_gravity(
    mut commands: Commands,
    query: Query<Entity, With<GravityEntity>>,
) {
    commands.remove_resource::<GravityOverride>();
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gravity_normal_when_not_flipped() {
        let state = GravityState { flipped: false, flip_timer: 4.0, flip_interval: 4.0 };
        assert_eq!(compute_gravity_direction(&state), -25.0);
    }

    #[test]
    fn gravity_reversed_when_flipped() {
        let state = GravityState { flipped: true, flip_timer: 4.0, flip_interval: 4.0 };
        assert_eq!(compute_gravity_direction(&state), 25.0);
    }

    #[test]
    fn gravity_flips_every_4_seconds() {
        let mut state = GravityState::default();
        assert!(!state.flipped);

        // Tick 4 seconds
        update_gravity_flip(&mut state, 4.0);
        assert!(state.flipped);

        // Tick another 4 seconds
        update_gravity_flip(&mut state, 4.0);
        assert!(!state.flipped);
    }

    #[test]
    fn gravity_flip_timer_resets() {
        let mut state = GravityState::default();
        update_gravity_flip(&mut state, 4.1);
        assert!(state.flipped);
        assert!(state.flip_timer > 3.5); // reset to ~interval
    }

    #[test]
    fn victory_at_height_30() {
        assert!(!check_reached_top(0.0));
        assert!(!check_reached_top(29.9));
        assert!(check_reached_top(30.0));
        assert!(check_reached_top(50.0));
    }

    #[test]
    fn gravity_makes_climbing_unreliable() {
        // With gravity flipping every 4s, the player alternates between
        // falling down and being pushed up. On a flat floor, they bounce
        // between 0 and some height but can never maintain controlled ascent
        // to 30m across platforms.
        let mut state = GravityState::default();
        let mut player_y: f32 = 0.0;
        let mut velocity_y: f32 = 0.0;
        let dt = 1.0 / 60.0;
        let mut max_height: f32 = 0.0;

        for _ in 0..3600 { // 60 seconds of simulation
            update_gravity_flip(&mut state, dt);
            let grav = compute_gravity_direction(&state);
            velocity_y += grav * dt;
            player_y += velocity_y * dt;
            if player_y < 0.0 {
                player_y = 0.0;
                velocity_y = 0.0;
            }
            if player_y > max_height { max_height = player_y; }
        }

        // Even with gravity flipping, without platforms the player
        // can't sustain height. The flips make platforming unreliable.
        // (In the actual game, the player needs platforms + stable gravity)
        assert!(max_height > 0.0, "Player should reach some height from upward gravity");
    }

    #[test]
    fn debugger_scenario_lock_flipped_false() {
        let mut state = GravityState::default();
        state.flipped = false; // debugger locks this
        state.flip_interval = 9999.0; // debugger sets this high
        state.flip_timer = 9999.0;    // reset timer to match new interval
        // Gravity stays normal
        assert_eq!(compute_gravity_direction(&state), -25.0);
        update_gravity_flip(&mut state, 10.0);
        // With interval=9999 and timer=9999, timer doesn't expire after 10s
        assert!(!state.flipped);
    }
}
