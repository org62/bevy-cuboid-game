use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;

use crate::player::{
    animate_player, escape_to_menu, player_movement, spawn_player, toggle_pause, MovementBounds,
    Player, PlayerMovementSet, PlayerPhysics, SpeedBoostMultiplier,
};
use crate::{GamePaused, RacePhase, Screen, Scoreboard};

pub struct Level5Plugin;

impl Plugin for Level5Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::RaceChallenge), setup_race)
            .add_systems(
                Update,
                countdown_update.run_if(in_state(RacePhase::Countdown)),
            )
            .add_systems(
                Update,
                escape_to_menu.run_if(in_state(RacePhase::Countdown)),
            )
            .add_systems(
                Update,
                (player_movement.in_set(PlayerMovementSet), race_playing_update)
                    .chain()
                    .run_if(in_state(RacePhase::Playing)),
            )
            .add_systems(
                Update,
                (animate_player, race_visual_update)
                    .after(PlayerMovementSet)
                    .run_if(in_state(Screen::RaceChallenge)),
            )
            .add_systems(
                Update,
                (escape_to_menu, toggle_pause).run_if(in_state(RacePhase::Playing)),
            )
            .add_systems(
                Update,
                handle_victory.run_if(in_state(RacePhase::Victory)),
            )
            .add_systems(
                Update,
                handle_lost.run_if(in_state(RacePhase::Lost)),
            )
            .add_systems(OnExit(Screen::RaceChallenge), cleanup_race);
    }
}

// --- Components ---

#[derive(Component)]
struct RaceEntity;

#[derive(Component)]
struct RaceFollowCam;

#[derive(Component)]
struct AiRacer {
    lane: u8,
    progress: f32,
    speed_multiplier: f32,
}

#[derive(Component)]
struct RaceHudText;

#[derive(Component)]
struct RaceHintBox;

#[derive(Component)]
struct RaceHintCloseButton;

#[derive(Component)]
struct OverlayScreen;

#[derive(Component)]
struct CountdownText;

// --- Resources ---

#[derive(Resource)]
struct CountdownTimer {
    timer: Timer,
    stage: u8, // 3, 2, 1, 0(GO)
}

#[derive(Resource, Default)]
struct RaceSeed(u32);

#[repr(C)]
#[derive(Resource)]
pub struct RacerStats {
    pub player_speed: f32,
    pub ai_speed: f32,
}

impl Default for RacerStats {
    fn default() -> Self {
        Self {
            player_speed: 7.0,
            ai_speed: 7.0,
        }
    }
}

#[derive(Resource)]
pub(crate) struct PlayerRaceState {
    pub(crate) progress: f32,
}

impl Default for PlayerRaceState {
    fn default() -> Self {
        Self {
            progress: 0.0,
        }
    }
}

// --- Debugger-target functions ---

#[inline(never)]
fn compute_player_race_speed(stats: &RacerStats) -> f32 {
    stats.player_speed
}

#[inline(never)]
fn compute_ai_race_speed(stats: &RacerStats) -> f32 {
    stats.ai_speed
}

#[inline(never)]
fn check_race_finished(progress: f32) -> bool {
    progress >= 0.995
}

// --- Track geometry (straight line along -Z) ---

const TRACK_START_Z: f32 = 2.0;
const TRACK_END_Z: f32 = -148.0;
const TRACK_LENGTH: f32 = TRACK_START_Z - TRACK_END_Z; // 150.0
const TRACK_WIDTH: f32 = 8.0;
#[cfg(test)]
const LANE_COUNT: usize = 4; // 3 AI + 1 player
const PLAYER_LANE_X: f32 = 3.0; // rightmost lane center
const ARENA_MIN: Vec2 = Vec2::new(-6.0, -150.0);
const ARENA_MAX: Vec2 = Vec2::new(6.0, 4.0);
const PLAYER_SPAWN: Vec3 = Vec3::new(PLAYER_LANE_X, 0.0, TRACK_START_Z);
const AI_LANE_XS: [f32; 3] = [-3.0, -1.0, 1.0];

fn random_ai_multiplier(seed: u32) -> f32 {
    // Pseudo-random in 1.05..1.15 range
    let hash = seed.wrapping_mul(2654435761);
    let frac = (hash % 10000) as f32 / 10000.0;
    1.05 + frac * 0.10
}

fn track_position(progress: f32, lane_x: f32) -> Vec3 {
    Vec3::new(
        lane_x,
        0.0,
        TRACK_START_Z - progress * TRACK_LENGTH,
    )
}

fn progress_from_position(pos: Vec3) -> f32 {
    ((TRACK_START_Z - pos.z) / TRACK_LENGTH).clamp(0.0, 1.0)
}

// --- Setup ---

fn setup_race(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    scoreboard: Res<Scoreboard>,
) {
    commands.insert_resource(ClearColor(Color::srgb(0.4, 0.6, 0.85)));
    commands.insert_resource(RacerStats::default());
    commands.insert_resource(PlayerRaceState::default());
    commands.init_resource::<RaceSeed>();
    commands.insert_resource(CountdownTimer {
        timer: Timer::from_seconds(1.0, TimerMode::Repeating),
        stage: 3,
    });

    // Ground
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(20.0, 200.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.6, 0.25),
            ..default()
        })),
        Transform::from_xyz(0.0, -0.01, -73.0),
        RaceEntity,
    ));

    // Track surface (single long strip)
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(TRACK_WIDTH, 0.05, TRACK_LENGTH))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.35, 0.35, 0.4),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.01, TRACK_START_Z - TRACK_LENGTH / 2.0),
        RaceEntity,
    ));

    // Lane dividers (dashed white lines) — 3 lines between 4 lanes
    let lane_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.9, 0.9),
        ..default()
    });
    let divider_xs = [-2.0f32, 0.0, 2.0]; // between lanes at -3, -1, 1, 3
    for &divider_x in &divider_xs {
        for i in 0..75 {
            let z = TRACK_START_Z - (i as f32 * 2.0 + 0.5);
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.08, 0.06, 1.0))),
                MeshMaterial3d(lane_mat.clone()),
                Transform::from_xyz(divider_x, 0.04, z),
                RaceEntity,
            ));
        }
    }

    // Start line
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(TRACK_WIDTH, 0.1, 0.3))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 1.0, 1.0),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.05, TRACK_START_Z),
        RaceEntity,
    ));

    // Finish line
    let finish_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.2, 0.2),
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(TRACK_WIDTH, 0.1, 0.3))),
        MeshMaterial3d(finish_mat.clone()),
        Transform::from_xyz(0.0, 0.05, TRACK_END_Z),
        RaceEntity,
    ));
    // Finish arch
    let arch_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.9, 0.9),
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.2, 3.0, 0.2))),
        MeshMaterial3d(arch_mat.clone()),
        Transform::from_xyz(-TRACK_WIDTH / 2.0, 1.5, TRACK_END_Z),
        RaceEntity,
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.2, 3.0, 0.2))),
        MeshMaterial3d(arch_mat.clone()),
        Transform::from_xyz(TRACK_WIDTH / 2.0, 1.5, TRACK_END_Z),
        RaceEntity,
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(TRACK_WIDTH, 0.2, 0.2))),
        MeshMaterial3d(finish_mat),
        Transform::from_xyz(0.0, 3.0, TRACK_END_Z),
        RaceEntity,
    ));

    // AI racers (body + head like the hero, but no eyes)
    let ai_colors = [
        Color::srgb(0.2, 0.4, 0.9),
        Color::srgb(0.9, 0.8, 0.1),
        Color::srgb(0.1, 0.8, 0.3),
    ];
    for (i, (&color, &lane_x)) in ai_colors.iter().zip(AI_LANE_XS.iter()).enumerate() {
        let start_pos = track_position(0.0, lane_x);
        let ai_mat = materials.add(StandardMaterial {
            base_color: color,
            ..default()
        });
        commands
            .spawn((
                Transform::from_xyz(start_pos.x, 0.0, start_pos.z),
                Visibility::default(),
                AiRacer {
                    lane: i as u8,
                    progress: 0.0,
                    speed_multiplier: random_ai_multiplier(i as u32),
                },
                RaceEntity,
            ))
            .with_children(|parent| {
                // Body
                parent.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.6, 0.8, 0.5))),
                    MeshMaterial3d(ai_mat.clone()),
                    Transform::from_xyz(0.0, 0.8, 0.0),
                ));
                // Head (no eyes)
                parent.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.65, 0.6, 0.55))),
                    MeshMaterial3d(ai_mat),
                    Transform::from_xyz(0.0, 1.5, 0.0),
                ));
            });
    }

    // Player
    let player = spawn_player(&mut commands, &mut meshes, &mut materials);
    commands.entity(player).insert((
        RaceEntity,
        Transform::from_xyz(PLAYER_SPAWN.x, 0.0, PLAYER_SPAWN.z)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        MovementBounds {
            rects: vec![(ARENA_MIN, ARENA_MAX)],
        },
    ));

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 14.0, 14.0).looking_at(Vec3::ZERO, Vec3::Y),
        RaceFollowCam,
        RaceEntity,
    ));

    // Light
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.7, 0.3, 0.0)),
        RaceEntity,
    ));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.9, 0.9, 1.0),
        brightness: 400.0,
    });

    // HUD
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                left: Val::Px(16.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.1, 0.2, 0.85)),
            BorderRadius::all(Val::Px(12.0)),
            RaceEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Pos: 4th | 0%"),
                TextFont { font_size: 22.0, ..default() },
                TextColor(Color::srgb(1.0, 1.0, 1.0)),
                RaceHudText,
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
            RaceEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("[Esc] Menu | WASD Move | Space Jump | [P] Pause"),
                TextFont { font_size: 20.0, ..default() },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
            ));
        });

    // Countdown overlay
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            GlobalZIndex(20),
            CountdownText,
            RaceEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("3"),
                TextFont { font_size: 120.0, ..default() },
                TextColor(Color::srgb(1.0, 1.0, 1.0)),
                CountdownText,
            ));
        });

    // Hint
    if !scoreboard.race_solved {
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
                RaceHintBox,
                RaceEntity,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new("Hint"),
                    TextFont { font_size: 20.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.85, 0.3)),
                ));
                parent.spawn((
                    Node { max_width: Val::Px(250.0), ..default() },
                    Text::new("The race is rigged -- the numbers are stacked against you. A RacerStats resource controls everything: speeds, laps. What if you could rewrite the rules?"),
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
                        RaceHintCloseButton,
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

// --- Countdown ---

fn spawn_countdown_ui(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            GlobalZIndex(20),
            CountdownText,
            RaceEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("3"),
                TextFont { font_size: 120.0, ..default() },
                TextColor(Color::srgb(1.0, 1.0, 1.0)),
                CountdownText,
            ));
        });
}

fn countdown_update(
    time: Res<Time>,
    mut countdown: ResMut<CountdownTimer>,
    mut next_phase: ResMut<NextState<RacePhase>>,
    mut commands: Commands,
    mut text_q: Query<&mut Text, With<CountdownText>>,
    countdown_entities: Query<Entity, (With<CountdownText>, With<Node>)>,
) {
    countdown.timer.tick(time.delta());
    if countdown.timer.just_finished() {
        if countdown.stage == 0 {
            // Remove countdown UI and start playing
            for entity in &countdown_entities {
                commands.entity(entity).despawn_recursive();
            }
            next_phase.set(RacePhase::Playing);
            return;
        }
        countdown.stage -= 1;
        let label = if countdown.stage == 0 {
            "GO!".to_string()
        } else {
            countdown.stage.to_string()
        };
        for mut text in &mut text_q {
            **text = label.clone();
        }
    }
}

// --- Gameplay ---

fn race_playing_update(
    time: Res<Time>,
    stats: Res<RacerStats>,
    mut commands: Commands,
    mut key_events: EventReader<KeyboardInput>,
    mut player_race: ResMut<PlayerRaceState>,
    mut next_phase: ResMut<NextState<RacePhase>>,
    player_q: Query<&Transform, (With<Player>, Without<AiRacer>)>,
    mut ai_q: Query<(&mut AiRacer, &mut Transform), Without<Player>>,
    game_paused: Res<GamePaused>,
    existing_boost: Option<Res<SpeedBoostMultiplier>>,
) {
    if game_paused.0 { return; }

    // Easter egg: ? key gives 2x speed boost
    if existing_boost.is_none() {
        for event in key_events.read() {
            if event.state.is_pressed() {
                if let bevy::input::keyboard::Key::Character(ref ch) = event.logical_key {
                    if ch.as_str() == "?" {
                        commands.insert_resource(SpeedBoostMultiplier(2.0));
                        break;
                    }
                }
            }
        }
    }
    let dt = time.delta_secs();

    // Update player race progress from position
    if let Ok(pt) = player_q.get_single() {
        player_race.progress = progress_from_position(pt.translation);
    }

    // Check player victory
    if check_race_finished(player_race.progress) {
        next_phase.set(RacePhase::Victory);
        return;
    }

    // Update AI racers
    let _player_speed = compute_player_race_speed(&stats);
    let base_speed = compute_ai_race_speed(&stats);

    for (mut ai, mut t) in &mut ai_q {
        let speed_factor = (base_speed * ai.speed_multiplier) / TRACK_LENGTH;
        ai.progress += speed_factor * dt;

        let lane_x = AI_LANE_XS[ai.lane as usize];
        let pos = track_position(ai.progress, lane_x);
        t.translation = Vec3::new(pos.x, 0.0, pos.z);

        // Check AI victory
        if check_race_finished(ai.progress) {
            next_phase.set(RacePhase::Lost);
            return;
        }
    }
}

// --- Visual ---

#[allow(clippy::too_many_arguments)]
fn race_visual_update(
    time: Res<Time>,
    player_race: Res<PlayerRaceState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    player_q: Query<&Transform, (With<Player>, Without<RaceFollowCam>)>,
    mut camera_q: Query<&mut Transform, (With<RaceFollowCam>, Without<Player>)>,
    mut text_q: Query<&mut Text, With<RaceHudText>>,
    ai_q: Query<&AiRacer>,
    hint_q: Query<Entity, With<RaceHintBox>>,
    btn_q: Query<&Interaction, (Changed<Interaction>, With<RaceHintCloseButton>)>,
) {
    let dt = time.delta_secs();

    // Camera follows player from behind and above
    if let (Ok(pt), Ok(mut ct)) = (player_q.get_single(), camera_q.get_single_mut()) {
        let target = pt.translation + Vec3::new(0.0, 12.0, 10.0);
        let t = (6.0 * dt).min(1.0);
        ct.translation = ct.translation.lerp(target, t);
        ct.look_at(pt.translation + Vec3::new(0.0, 0.0, -4.0), Vec3::Y);
    }

    // HUD
    if let Ok(mut text) = text_q.get_single_mut() {
        let mut position = 1;
        for ai in &ai_q {
            if ai.progress > player_race.progress {
                position += 1;
            }
        }
        let pos_str = match position {
            1 => "1st",
            2 => "2nd",
            3 => "3rd",
            _ => "4th",
        };
        let pct = (player_race.progress * 100.0).min(100.0) as u32;
        **text = format!("Pos: {} | {}%", pos_str, pct);
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
        scoreboard.race_solved = true;
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
                RaceEntity,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new("YOU WIN THE RACE!"),
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

// --- Lost ---

fn handle_lost(
    mut commands: Commands,
    mut events: EventReader<KeyboardInput>,
    mut next_phase: ResMut<NextState<RacePhase>>,
    mut stats: ResMut<RacerStats>,
    mut race_seed: ResMut<RaceSeed>,
    mut player_race: ResMut<PlayerRaceState>,
    mut player_q: Query<(&mut Transform, &mut PlayerPhysics), (With<Player>, Without<AiRacer>)>,
    mut ai_q: Query<(&mut AiRacer, &mut Transform), Without<Player>>,
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
                BackgroundColor(Color::srgba(0.2, 0.0, 0.0, 0.8)),
                GlobalZIndex(10),
                OverlayScreen,
                RaceEntity,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new("YOU LOST!"),
                    TextFont { font_size: 52.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.2, 0.2)),
                ));
                parent.spawn((
                    Text::new("An AI racer finished first!"),
                    TextFont { font_size: 28.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.6, 0.6)),
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
        *stats = RacerStats::default();
        *player_race = PlayerRaceState::default();
        commands.remove_resource::<SpeedBoostMultiplier>();
        race_seed.0 += 1;
        for (mut ai, mut t) in &mut ai_q {
            ai.progress = 0.0;
            ai.speed_multiplier = random_ai_multiplier(race_seed.0 * 3 + ai.lane as u32);
            let lane_x = AI_LANE_XS[ai.lane as usize];
            let pos = track_position(0.0, lane_x);
            t.translation = Vec3::new(pos.x, 0.0, pos.z);
        }
        if let Ok((mut t, mut p)) = player_q.get_single_mut() {
            t.translation = PLAYER_SPAWN;
            p.velocity = Vec3::ZERO;
            p.grounded = true;
        }
        spawn_countdown_ui(&mut commands);
        commands.insert_resource(CountdownTimer {
            timer: Timer::from_seconds(1.0, TimerMode::Repeating),
            stage: 3,
        });
        next_phase.set(RacePhase::Countdown);
        return;
    }
}

// --- Cleanup ---

fn cleanup_race(mut commands: Commands, query: Query<Entity, With<RaceEntity>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
    commands.remove_resource::<SpeedBoostMultiplier>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_speed_is_rigged() {
        let stats = RacerStats::default();
        assert_eq!(compute_player_race_speed(&stats), 7.0);
        assert_eq!(compute_ai_race_speed(&stats), 7.0);
        // AI base speed equals player speed; multiplier makes them 1.05-1.15x faster
        for seed in 0..100 {
            let m = random_ai_multiplier(seed);
            assert!(m >= 1.05 && m <= 1.15, "multiplier {m} out of range for seed {seed}");
        }
    }

    #[test]
    fn race_finish_detection() {
        assert!(!check_race_finished(0.0));
        assert!(!check_race_finished(0.5));
        assert!(!check_race_finished(0.99));
        assert!(check_race_finished(0.995));
        assert!(check_race_finished(1.0));
    }

    #[test]
    fn ai_wins_before_player_normally() {
        let stats = RacerStats::default();
        // Any AI racer with multiplier > 1.0 is faster than player
        let min_mult = 1.05_f32;
        let ai_time = TRACK_LENGTH / (stats.ai_speed * min_mult);
        let player_time = TRACK_LENGTH / stats.player_speed;

        assert!(ai_time < player_time,
            "AI finishes in {:.1}s, player in {:.1}s", ai_time, player_time);
    }

    #[test]
    fn debugger_scenario_boost_player_speed() {
        let mut stats = RacerStats::default();
        stats.player_speed = 50.0;
        assert_eq!(compute_player_race_speed(&stats), 50.0);
        assert!(stats.player_speed > stats.ai_speed);
    }

    #[test]
    fn debugger_scenario_reduce_ai_speed() {
        let mut stats = RacerStats::default();
        stats.ai_speed = 1.0;
        assert_eq!(compute_ai_race_speed(&stats), 1.0);
        assert!(stats.player_speed > stats.ai_speed);
    }

    #[test]
    fn track_positions_form_straight_line() {
        let p0 = track_position(0.0, 0.0);
        let p50 = track_position(0.5, 0.0);
        let p100 = track_position(1.0, 0.0);

        assert!((p0.x).abs() < 0.01);
        assert!((p50.x).abs() < 0.01);
        assert!((p100.x).abs() < 0.01);

        assert!((p0.z - TRACK_START_Z).abs() < 0.01);
        assert!((p100.z - TRACK_END_Z).abs() < 0.01);
        assert!(p50.z < p0.z);
        assert!(p100.z < p50.z);
    }

    #[test]
    fn progress_from_position_correct() {
        let start = Vec3::new(0.0, 0.0, TRACK_START_Z);
        let end = Vec3::new(0.0, 0.0, TRACK_END_Z);
        let mid = Vec3::new(0.0, 0.0, (TRACK_START_Z + TRACK_END_Z) / 2.0);

        assert!((progress_from_position(start)).abs() < 0.01);
        assert!((progress_from_position(end) - 1.0).abs() < 0.01);
        assert!((progress_from_position(mid) - 0.5).abs() < 0.01);
    }

    #[test]
    fn four_lanes_three_dividers() {
        assert_eq!(LANE_COUNT, 4);
        // 3 divider lines between 4 lanes
        assert_eq!(LANE_COUNT - 1, 3);
    }
}
