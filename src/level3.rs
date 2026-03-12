use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;

use crate::player::{
    animate_player, escape_to_menu, player_movement, spawn_player, toggle_pause, MovementBounds,
    Player, PlayerMovementSet, PlayerPhysics,
};
use crate::{CountdownPhase, GamePaused, Screen, Scoreboard};

pub struct Level3Plugin;

impl Plugin for Level3Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::CountdownChallenge), setup_countdown)
            .add_systems(
                Update,
                (player_movement.in_set(PlayerMovementSet), countdown_playing_update)
                    .chain()
                    .run_if(in_state(CountdownPhase::Playing)),
            )
            .add_systems(
                Update,
                (animate_player, countdown_visual_update)
                    .after(PlayerMovementSet)
                    .run_if(in_state(Screen::CountdownChallenge)),
            )
            .add_systems(
                Update,
                (escape_to_menu, toggle_pause).run_if(in_state(CountdownPhase::Playing)),
            )
            .add_systems(
                Update,
                handle_victory.run_if(in_state(CountdownPhase::Victory)),
            )
            .add_systems(
                Update,
                handle_exploded.run_if(in_state(CountdownPhase::Exploded)),
            )
            .add_systems(OnExit(Screen::CountdownChallenge), cleanup_countdown);
    }
}

// --- Components ---

#[derive(Component)]
struct CountdownEntity;

#[derive(Component)]
struct CountdownFollowCam;

#[derive(Component)]
struct TimerHudText;

#[derive(Component)]
struct TimeCrystal;

#[derive(Component)]
struct BombVisual;

#[derive(Component)]
struct CountdownHintBox;

#[derive(Component)]
struct CountdownHintCloseButton;

#[derive(Component)]
struct OverlayScreen;

// --- Resources ---

#[repr(C)]
#[derive(Resource)]
pub struct BombTimer {
    pub remaining: f32,
    pub defused: bool,
}

impl Default for BombTimer {
    fn default() -> Self {
        Self {
            remaining: 30.0,
            defused: false,
        }
    }
}

// --- Debugger-target functions ---

#[inline(never)]
fn tick_bomb_timer(timer: &mut BombTimer, delta: f32) {
    if timer.defused {
        return;
    }
    timer.remaining -= delta;
    if timer.remaining <= -10.0 {
        timer.defused = true;
    }
}

#[inline(never)]
fn check_bomb_defused(timer: &BombTimer) -> bool {
    timer.defused
}

// --- Constants ---

const ARENA_MIN: Vec2 = Vec2::new(-8.0, -8.0);
const ARENA_MAX: Vec2 = Vec2::new(8.0, 8.0);
const PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 0.0, 5.0);

// --- Setup ---

fn setup_countdown(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    scoreboard: Res<Scoreboard>,
) {
    commands.insert_resource(ClearColor(Color::srgb(0.2, 0.18, 0.15)));
    commands.insert_resource(BombTimer::default());

    // Dark stone floor
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(16.0, 16.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.28, 0.25),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
        CountdownEntity,
    ));

    // Bomb - black sphere with red ring
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.8))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.1, 0.1, 0.1),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.8, 0.0),
        BombVisual,
        CountdownEntity,
    ));
    // Red pulsing ring (torus approximation - flat cylinder)
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(1.0, 0.1))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.9, 0.1, 0.05),
            emissive: LinearRgba::new(2.0, 0.2, 0.05, 1.0),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.8, 0.0),
        BombVisual,
        CountdownEntity,
    ));

    // Time extension crystals on elevated platforms
    let crystal_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.6, 1.0),
        emissive: LinearRgba::new(0.4, 0.7, 2.0, 1.0),
        ..default()
    });
    let crystal_mesh = meshes.add(Cuboid::new(0.3, 0.5, 0.3));
    let platform_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.4, 0.35, 0.3),
        ..default()
    });
    let platform_mesh = meshes.add(Cuboid::new(1.5, 0.3, 1.5));

    let crystal_positions = [
        Vec3::new(-5.0, 0.0, -5.0),
        Vec3::new(5.0, 0.0, -5.0),
        Vec3::new(-5.0, 0.0, 5.0),
        Vec3::new(5.0, 0.0, 5.0),
        Vec3::new(0.0, 0.0, 6.0),
    ];

    for pos in &crystal_positions {
        // Platform
        commands.spawn((
            Mesh3d(platform_mesh.clone()),
            MeshMaterial3d(platform_mat.clone()),
            Transform::from_xyz(pos.x, 0.5, pos.z),
            CountdownEntity,
        ));
        // Crystal
        commands.spawn((
            Mesh3d(crystal_mesh.clone()),
            MeshMaterial3d(crystal_mat.clone()),
            Transform::from_xyz(pos.x, 1.1, pos.z),
            TimeCrystal,
            CountdownEntity,
        ));
    }

    // Player
    let player = spawn_player(&mut commands, &mut meshes, &mut materials);
    commands.entity(player).insert((
        CountdownEntity,
        Transform::from_xyz(PLAYER_SPAWN.x, 0.0, PLAYER_SPAWN.z)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        MovementBounds {
            rects: vec![(ARENA_MIN, ARENA_MAX)],
        },
    ));

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 10.0, 12.0).looking_at(Vec3::ZERO, Vec3::Y),
        CountdownFollowCam,
        CountdownEntity,
    ));

    // Lighting
    commands.spawn((
        DirectionalLight {
            illuminance: 5000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -std::f32::consts::FRAC_PI_4,
            std::f32::consts::FRAC_PI_6,
            0.0,
        )),
        CountdownEntity,
    ));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.9, 0.7, 0.5),
        brightness: 200.0,
    });

    // Orange glow cracks (point lights on floor)
    for pos in [
        Vec3::new(-3.0, 0.3, 2.0),
        Vec3::new(3.0, 0.3, -2.0),
        Vec3::new(-1.0, 0.3, -6.0),
    ] {
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.5, 0.1),
                intensity: 5000.0,
                range: 5.0,
                ..default()
            },
            Transform::from_translation(pos),
            CountdownEntity,
        ));
    }

    // HUD - timer display
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                left: Val::Px(16.0),
                padding: UiRect::axes(Val::Px(20.0), Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.8, 0.1, 0.05, 0.85)),
            BorderRadius::all(Val::Px(12.0)),
            CountdownEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("30.0"),
                TextFont {
                    font_size: 32.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 1.0, 1.0)),
                TimerHudText,
            ));
        });

    // HUD - controls
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(16.0),
                left: Val::Px(16.0),
                ..default()
            },
            CountdownEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("[Esc] Menu | WASD Move | Space Jump | [P] Pause"),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
            ));
        });

    // HUD - hint box
    if !scoreboard.countdown_solved {
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
                CountdownHintBox,
                CountdownEntity,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new("Hint"),
                    TextFont {
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.85, 0.3)),
                ));
                parent.spawn((
                    Node {
                        max_width: Val::Px(250.0),
                        ..default()
                    },
                    Text::new(
                        "The bomb is ticking... Can you stop time itself? Examine tick_bomb_timer() in the debugger.",
                    ),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
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
                        CountdownHintCloseButton,
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("[X] Close"),
                            TextFont {
                                font_size: 14.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.7, 0.7, 0.7)),
                        ));
                    });
            });
    }
}

// --- Gameplay system ---

fn countdown_playing_update(
    mut commands: Commands,
    time: Res<Time>,
    mut bomb: ResMut<BombTimer>,
    mut next_phase: ResMut<NextState<CountdownPhase>>,
    player_q: Query<&Transform, (With<Player>, Without<TimeCrystal>)>,
    mut crystal_q: Query<(Entity, &Transform), (With<TimeCrystal>, Without<Player>)>,
    game_paused: Res<GamePaused>,
) {
    if game_paused.0 { return; }
    let dt = time.delta_secs();
    tick_bomb_timer(&mut bomb, dt);

    if check_bomb_defused(&bomb) {
        next_phase.set(CountdownPhase::Victory);
        return;
    }

    if bomb.remaining <= 0.0 && !bomb.defused {
        next_phase.set(CountdownPhase::Exploded);
        return;
    }

    // Crystal collection
    let Ok(player_t) = player_q.get_single() else {
        return;
    };
    let pp = player_t.translation;
    for (entity, ct) in &mut crystal_q {
        let dx = pp.x - ct.translation.x;
        let dz = pp.z - ct.translation.z;
        if dx * dx + dz * dz < 2.0 {
            bomb.remaining += 3.0;
            commands.entity(entity).despawn_recursive();
        }
    }
}

// --- Visual update ---

#[allow(clippy::too_many_arguments)]
fn countdown_visual_update(
    time: Res<Time>,
    bomb: Res<BombTimer>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    player_q: Query<&Transform, (With<Player>, Without<CountdownFollowCam>)>,
    mut camera_q: Query<
        &mut Transform,
        (With<CountdownFollowCam>, Without<Player>),
    >,
    mut text_q: Query<(&mut Text, &mut TextColor), With<TimerHudText>>,
    hint_q: Query<Entity, With<CountdownHintBox>>,
    btn_q: Query<&Interaction, (Changed<Interaction>, With<CountdownHintCloseButton>)>,
    mut crystal_q: Query<&mut Transform, (With<TimeCrystal>, Without<Player>, Without<CountdownFollowCam>)>,
) {
    let dt = time.delta_secs();
    let elapsed = time.elapsed_secs();

    // Camera follow
    if let (Ok(pt), Ok(mut ct)) = (player_q.get_single(), camera_q.get_single_mut()) {
        let target = pt.translation + Vec3::new(0.0, 10.0, 12.0);
        let t = (8.0 * dt).min(1.0);
        ct.translation = ct.translation.lerp(target, t);
        ct.look_at(pt.translation + Vec3::Y, Vec3::Y);
    }

    // Timer HUD
    if let Ok((mut text, mut color)) = text_q.get_single_mut() {
        let display = bomb.remaining.max(0.0);
        **text = format!("{:.1}", display);
        if bomb.remaining < 10.0 {
            *color = TextColor(Color::srgb(1.0, 0.3, 0.3));
        } else {
            *color = TextColor(Color::srgb(1.0, 1.0, 1.0));
        }
    }

    // Crystal bob + rotate
    for mut t in &mut crystal_q {
        t.rotate_y(2.0 * dt);
        t.translation.y = 1.1 + (elapsed * 2.0 + t.translation.x).sin() * 0.15;
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
    mut next_screen: ResMut<NextState<Screen>>,
    mut scoreboard: ResMut<Scoreboard>,
    overlay_q: Query<Entity, With<OverlayScreen>>,
) {
    if overlay_q.is_empty() {
        scoreboard.countdown_solved = true;
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
                CountdownEntity,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new("BOMB DEFUSED!"),
                    TextFont { font_size: 52.0, ..default() },
                    TextColor(Color::srgb(0.2, 1.0, 0.2)),
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
        next_screen.set(Screen::Menu);
        return;
    }
}

// --- Exploded ---

fn handle_exploded(
    mut commands: Commands,
    mut events: EventReader<KeyboardInput>,
    mut next_phase: ResMut<NextState<CountdownPhase>>,
    mut bomb: ResMut<BombTimer>,
    mut player_q: Query<(&mut Transform, &mut PlayerPhysics), With<Player>>,
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
                BackgroundColor(Color::srgba(0.3, 0.05, 0.0, 0.85)),
                GlobalZIndex(10),
                OverlayScreen,
                CountdownEntity,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new("BOOM!"),
                    TextFont { font_size: 64.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.3, 0.1)),
                ));
                parent.spawn((
                    Text::new("The bomb exploded!"),
                    TextFont { font_size: 28.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.6, 0.4)),
                ));
                parent.spawn((
                    Text::new("Press any key to retry"),
                    TextFont { font_size: 22.0, ..default() },
                    TextColor(Color::srgb(0.8, 0.6, 0.6)),
                ));
            });
    }

    for event in events.read() {
        if !event.state.is_pressed() { continue; }
        for entity in &overlay_q {
            commands.entity(entity).despawn_recursive();
        }
        *bomb = BombTimer::default();
        if let Ok((mut t, mut p)) = player_q.get_single_mut() {
            t.translation = PLAYER_SPAWN;
            p.velocity = Vec3::ZERO;
            p.grounded = true;
        }
        next_phase.set(CountdownPhase::Playing);
        return;
    }
}

// --- Cleanup ---

fn cleanup_countdown(mut commands: Commands, query: Query<Entity, With<CountdownEntity>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bomb_ticks_down() {
        let mut timer = BombTimer { remaining: 30.0, defused: false };
        tick_bomb_timer(&mut timer, 1.0);
        assert!((timer.remaining - 29.0).abs() < 0.001);
    }

    #[test]
    fn bomb_does_not_tick_when_defused() {
        let mut timer = BombTimer { remaining: 20.0, defused: true };
        tick_bomb_timer(&mut timer, 5.0);
        assert!((timer.remaining - 20.0).abs() < 0.001);
    }

    #[test]
    fn bomb_defuses_at_negative_10() {
        let mut timer = BombTimer { remaining: 0.0, defused: false };
        // Tick past -10
        tick_bomb_timer(&mut timer, 11.0);
        assert!(timer.defused);
        assert!(check_bomb_defused(&timer));
    }

    #[test]
    fn bomb_does_not_defuse_at_negative_5() {
        let mut timer = BombTimer { remaining: 0.0, defused: false };
        tick_bomb_timer(&mut timer, 5.0);
        assert!(!timer.defused);
    }

    #[test]
    fn crystals_not_enough_to_survive() {
        // 30 seconds start, 5 crystals * 3 sec each = 15 extra = 45 total
        // Still runs out eventually (player can't reach defuse panel)
        let mut timer = BombTimer { remaining: 30.0, defused: false };
        // Add all 5 crystals
        timer.remaining += 15.0; // 45 seconds total
        // Tick 46 seconds
        for _ in 0..460 {
            tick_bomb_timer(&mut timer, 0.1);
            if timer.remaining <= 0.0 && !timer.defused {
                break;
            }
        }
        // Timer ran out without defusing (no hidden trigger at -10 reached)
        assert!(timer.remaining <= 0.0);
        // The normal tick won't reach -10 because we stop checking at 0
        // (in the actual game, the Exploded phase triggers at 0)
    }

    #[test]
    fn debugger_scenario_set_defused_true() {
        // Simulates: player pauses debugger, sets timer.defused = true
        let mut timer = BombTimer { remaining: 15.0, defused: false };
        timer.defused = true; // debugger sets this
        assert!(check_bomb_defused(&timer));
        // Timer stops ticking
        tick_bomb_timer(&mut timer, 100.0);
        assert!((timer.remaining - 15.0).abs() < 0.001);
    }

    #[test]
    fn debugger_scenario_set_remaining_below_negative_10() {
        // Simulates: player sets remaining to -11.0
        let mut timer = BombTimer { remaining: -11.0, defused: false };
        tick_bomb_timer(&mut timer, 0.0);
        // The check is remaining <= -10.0, and we just ticked with 0 delta
        // remaining is -11.0 which is <= -10.0, so defused should be true
        assert!(timer.defused);
    }

    #[test]
    fn full_countdown_simulation() {
        // Simulate 30 seconds of ticking at 60fps
        let mut timer = BombTimer::default();
        let dt = 1.0 / 60.0;
        let mut frames = 0;
        while timer.remaining > 0.0 && !timer.defused {
            tick_bomb_timer(&mut timer, dt);
            frames += 1;
            if frames > 2000 { break; } // safety
        }
        // Should have ticked for ~30 seconds = ~1800 frames
        assert!(frames >= 1790 && frames <= 1810,
            "Expected ~1800 frames, got {}", frames);
        assert!(!timer.defused); // shouldn't auto-defuse through normal play
    }
}
