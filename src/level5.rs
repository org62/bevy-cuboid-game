use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;

use crate::player::{
    animate_player, escape_to_menu, player_movement, spawn_player, toggle_pause, MovementBounds,
    Player, PlayerMovementSet, PlayerPhysics, PowerUpState,
};
use crate::shared_ui;
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
                (escape_to_menu, toggle_pause).run_if(in_state(RacePhase::Countdown)),
            )
            .add_systems(
                Update,
                (player_movement.in_set(PlayerMovementSet), race_playing_update)
                    .chain()
                    .run_if(in_state(RacePhase::Playing)),
            )
            .add_systems(
                Update,
                (animate_player, race_visual_update, shared_ui::dismiss_hint)
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

#[derive(Component, Clone, Copy)]
struct RaceEntity;

#[derive(Component)]
struct RaceFollowCam;

#[derive(Component)]
struct AiRacer {
    lane: u8,
    speed_multiplier: f32,
    color: Color,
}

/// One row of the standings HUD. `0` is the row slot (0 = leader), rewritten
/// and recolored every frame from the sorted racer list.
#[derive(Component)]
struct RaceHudRow(usize);

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
    /// Speed the AI racers advance at. This is the only "rule" the player can
    /// weaken to win by slowing the field down.
    pub ai_speed: f32,
}

impl Default for RacerStats {
    fn default() -> Self {
        Self {
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
const PLAYER_COLOR: Color = Color::srgb(1.0, 0.45, 0.35); // matches spawn_player coral
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

#[inline(never)]
fn track_position(progress: f32, lane_x: f32) -> Vec3 {
    Vec3::new(
        lane_x,
        0.0,
        TRACK_START_Z - progress * TRACK_LENGTH,
    )
}

#[inline(never)]
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
                    speed_multiplier: random_ai_multiplier(i as u32),
                    color,
                },
                RaceEntity,
            ))
            .with_children(|parent| {
                // Body
                parent.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.6, 0.8, 0.5))),
                    MeshMaterial3d(ai_mat.clone()),
                    Transform::from_xyz(0.0, 0.4, 0.0),
                ));
                // Head (no eyes)
                parent.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.65, 0.6, 0.55))),
                    MeshMaterial3d(ai_mat),
                    Transform::from_xyz(0.0, 1.1, 0.0),
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
    shared_ui::setup_level_lighting(
        &mut commands,
        10000.0,
        (-0.7, 0.3, 0.0),
        Color::srgb(0.9, 0.9, 1.0),
        400.0,
        RaceEntity,
    );

    // HUD — live standings: one row per racer (progress + position), sorted by
    // race position each frame and tinted with each racer's own color.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                left: Val::Px(16.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                min_width: Val::Px(280.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.1, 0.2, 0.85)),
            BorderRadius::all(Val::Px(12.0)),
            RaceEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("STANDINGS"),
                TextFont { font_size: 16.0, ..default() },
                TextColor(Color::srgb(0.7, 0.7, 0.8)),
            ));
            // 4 rows: 3 AI + player.
            for slot in 0..4 {
                parent.spawn((
                    Text::new(""),
                    TextFont { font_size: 18.0, ..default() },
                    TextColor(Color::WHITE),
                    RaceHudRow(slot),
                ));
            }
        });

    // Controls
    shared_ui::spawn_controls_hint(
        &mut commands,
        "Win the race",
        RaceEntity,
    );

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
    if !scoreboard.is_solved(5) {
        shared_ui::spawn_hint_box(
            &mut commands,
            "You can't out-drive the AI head-on. But your finish is judged only by how far you are down the track. Find the coordinate that changes as you drive (like the maze's height trick) and move yourself to the finish line -- or find the value that drives the AI and weaken it.",
            300.0,
            RaceEntity,
        );
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
    game_paused: Res<GamePaused>,
) {
    if game_paused.0 { return; }
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
#[inline(never)]
fn race_playing_update(
    time: Res<Time>,
    stats: Res<RacerStats>,
    mut commands: Commands,
    mut key_events: EventReader<KeyboardInput>,
    mut player_race: ResMut<PlayerRaceState>,
    mut next_phase: ResMut<NextState<RacePhase>>,
    player_q: Query<&Transform, (With<Player>, Without<AiRacer>)>,
    mut ai_q: Query<(&AiRacer, &mut Transform), Without<Player>>,
    game_paused: Res<GamePaused>,
    mut power_up_state: Option<ResMut<PowerUpState>>,
) {
    if game_paused.0 { return; }

    handle_boost_easter_egg(&mut commands, &mut key_events, &mut power_up_state);

    let dt = time.delta_secs();

    update_player_progress(&player_q, &mut player_race);
    if check_race_finished(player_race.progress) {
        next_phase.set(RacePhase::Victory);
        return;
    }

    if advance_ai_racers(&stats, &mut ai_q, dt) {
        next_phase.set(RacePhase::Lost);
        return;
    }
}

/// Easter egg: pressing `?` (once) grants a 2x speed boost via `PowerUpState`.
#[inline(never)]
fn handle_boost_easter_egg(
    commands: &mut Commands,
    key_events: &mut EventReader<KeyboardInput>,
    power_up_state: &mut Option<ResMut<PowerUpState>>,
) {
    let has_boost = power_up_state.as_ref().map_or(false, |p| p.speed_multiplier > 0.0);
    if has_boost {
        return;
    }
    for event in key_events.read() {
        if event.state.is_pressed() {
            if let bevy::input::keyboard::Key::Character(ref ch) = event.logical_key {
                if ch.as_str() == "?" {
                    if let Some(state) = power_up_state.as_mut() {
                        state.speed_multiplier = 2.0;
                    } else {
                        commands.insert_resource(PowerUpState { speed_multiplier: 2.0, ..default() });
                    }
                    break;
                }
            }
        }
    }
}

/// Derive the player's race progress from their current position (position is
/// the single source of truth for every racer).
#[inline(never)]
fn update_player_progress(
    player_q: &Query<&Transform, (With<Player>, Without<AiRacer>)>,
    player_race: &mut PlayerRaceState,
) {
    if let Ok(pt) = player_q.get_single() {
        player_race.progress = progress_from_position(pt.translation);
    }
}

/// Advance every AI racer down the track by its speed. Returns `true` if any AI
/// crossed the finish line this frame (i.e. the player has lost).
#[inline(never)]
fn advance_ai_racers(
    stats: &RacerStats,
    ai_q: &mut Query<(&AiRacer, &mut Transform), Without<Player>>,
    dt: f32,
) -> bool {
    let base_speed = compute_ai_race_speed(stats);
    for (ai, mut t) in ai_q.iter_mut() {
        // World units per second, same units as the player's movement speed.
        let speed = base_speed * ai.speed_multiplier;
        t.translation.z -= speed * dt;

        if check_race_finished(progress_from_position(t.translation)) {
            return true;
        }
    }
    false
}

// --- Visual ---

fn race_visual_update(
    time: Res<Time>,
    player_race: Res<PlayerRaceState>,
    player_q: Query<&Transform, (With<Player>, Without<RaceFollowCam>, Without<AiRacer>)>,
    mut camera_q: Query<&mut Transform, (With<RaceFollowCam>, Without<Player>, Without<AiRacer>)>,
    ai_q: Query<(&AiRacer, &Transform), (Without<Player>, Without<RaceFollowCam>)>,
    mut row_q: Query<(&mut Text, &mut TextColor, &RaceHudRow)>,
) {
    let dt = time.delta_secs();

    // Camera follows player from behind and above (race-specific: looks ahead)
    if let (Ok(pt), Ok(mut ct)) = (player_q.get_single(), camera_q.get_single_mut()) {
        let target = pt.translation + Vec3::new(0.0, 12.0, 10.0);
        let t = (6.0 * dt).min(1.0);
        ct.translation = ct.translation.lerp(target, t);
        ct.look_at(pt.translation + Vec3::new(0.0, 0.0, -4.0), Vec3::Y);
    }

    // Collect every racer: (label, color, progress, position).
    let mut rows: Vec<(String, Color, f32, Vec3)> = Vec::new();
    if let Ok(pt) = player_q.get_single() {
        rows.push(("You".to_string(), PLAYER_COLOR, player_race.progress, pt.translation));
    }
    for (ai, t) in &ai_q {
        let progress = progress_from_position(t.translation);
        rows.push((format!("AI {}", ai.lane + 1), ai.color, progress, t.translation));
    }

    // Sort by race position: furthest progress first.
    rows.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    // Fill each HUD slot with the racer at that standing, in their own color.
    for (mut text, mut color, slot) in &mut row_q {
        if let Some((label, c, progress, _pos)) = rows.get(slot.0) {
            let pct = (progress * 100.0).min(100.0);
            **text = format!("{}. {}  {:.0}%", slot.0 + 1, label, pct);
            color.0 = *c;
        } else {
            **text = String::new();
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
        scoreboard.set_solved(5);
        shared_ui::spawn_victory_overlay(
            &mut commands,
            "YOU WIN THE RACE!",
            None,
            0.0,
            "Press any key to continue",
            RaceEntity,
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
    overlay_q: Query<Entity, With<shared_ui::OverlayScreen>>,
) {
    if overlay_q.is_empty() {
        shared_ui::spawn_defeat_overlay(
            &mut commands,
            "YOU LOST!",
            52.0,
            Some("An AI racer finished first!"),
            28.0,
            "Press any key to retry",
            Color::srgba(0.2, 0.0, 0.0, 0.8),
            RaceEntity,
        );
    }

    for event in events.read() {
        if !event.state.is_pressed() { continue; }
        for entity in &overlay_q {
            commands.entity(entity).despawn_recursive();
        }
        *stats = RacerStats::default();
        *player_race = PlayerRaceState::default();
        commands.remove_resource::<PowerUpState>();
        race_seed.0 += 1;
        for (mut ai, mut t) in &mut ai_q {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    // The player's top speed is the engine's MAX_SPEED (private to player.rs).
    const PLAYER_MAX_SPEED: f32 = 7.0;

    #[test]
    fn ai_speed_matches_player_baseline() {
        let stats = RacerStats::default();
        // AI base speed equals the player's top speed; the per-racer multiplier
        // (1.05-1.15x) is what makes the field faster than the player.
        assert_eq!(compute_ai_race_speed(&stats), PLAYER_MAX_SPEED);
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
        let player_time = TRACK_LENGTH / PLAYER_MAX_SPEED;

        assert!(ai_time < player_time,
            "AI finishes in {:.1}s, player in {:.1}s", ai_time, player_time);
    }

    #[test]
    fn debugger_scenario_reduce_ai_speed() {
        let mut stats = RacerStats::default();
        stats.ai_speed = 1.0;
        assert_eq!(compute_ai_race_speed(&stats), 1.0);
        // Slower than the player's top speed, so the player can now finish first.
        assert!(stats.ai_speed < PLAYER_MAX_SPEED);
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
