use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;

use crate::player::{
    animate_player, escape_to_menu, player_movement, spawn_player, toggle_pause, MovementBounds,
    Player, PlayerPhysics,
};
use crate::{GamePaused, RacePhase, Screen, Scoreboard};

pub struct Level5Plugin;

impl Plugin for Level5Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::RaceChallenge), setup_race)
            .add_systems(
                FixedUpdate,
                (player_movement, race_playing_update)
                    .chain()
                    .run_if(in_state(RacePhase::Playing)),
            )
            .add_systems(
                Update,
                (animate_player, race_visual_update).run_if(in_state(Screen::RaceChallenge)),
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
    lap: i32,
}

#[derive(Component)]
struct RaceHudText;

#[derive(Component)]
struct RaceHintBox;

#[derive(Component)]
struct RaceHintCloseButton;

#[derive(Component)]
struct OverlayScreen;

// --- Resources ---

#[repr(C)]
#[derive(Resource)]
pub struct RacerStats {
    pub player_speed: f32,
    pub ai_speed: f32,
    pub laps_to_win: i32,
}

impl Default for RacerStats {
    fn default() -> Self {
        Self {
            player_speed: 3.0,
            ai_speed: 20.0,
            laps_to_win: 3,
        }
    }
}

#[derive(Resource)]
pub(crate) struct PlayerRaceState {
    pub(crate) progress: f32,
    pub(crate) lap: i32,
}

impl Default for PlayerRaceState {
    fn default() -> Self {
        Self {
            progress: 0.0,
            lap: 0,
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
fn check_race_victory(player_lap: i32, laps_to_win: i32) -> bool {
    player_lap >= laps_to_win
}

// --- Track geometry (oval) ---

const TRACK_CENTER: Vec3 = Vec3::new(0.0, 0.0, 0.0);
const TRACK_RADIUS_X: f32 = 10.0;
const TRACK_RADIUS_Z: f32 = 6.0;
const TRACK_WIDTH: f32 = 4.0;
const ARENA_MIN: Vec2 = Vec2::new(-14.0, -10.0);
const ARENA_MAX: Vec2 = Vec2::new(14.0, 10.0);
const PLAYER_SPAWN: Vec3 = Vec3::new(10.0, 0.0, 0.0);

fn track_position(progress: f32, lane_offset: f32) -> Vec3 {
    let angle = progress * std::f32::consts::TAU;
    let r_x = TRACK_RADIUS_X + lane_offset;
    let r_z = TRACK_RADIUS_Z + lane_offset;
    Vec3::new(
        TRACK_CENTER.x + angle.cos() * r_x,
        0.0,
        TRACK_CENTER.z + angle.sin() * r_z,
    )
}

fn progress_from_position(pos: Vec3) -> f32 {
    let dx = pos.x - TRACK_CENTER.x;
    let dz = pos.z - TRACK_CENTER.z;
    let angle = dz.atan2(dx);
    let progress = angle / std::f32::consts::TAU;
    if progress < 0.0 { progress + 1.0 } else { progress }
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

    // Ground
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(30.0, 22.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.6, 0.25),
            ..default()
        })),
        Transform::from_xyz(0.0, -0.01, 0.0),
        RaceEntity,
    ));

    // Track surface (oval ring approximated with segments)
    let track_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.35, 0.4),
        ..default()
    });
    for i in 0..32 {
        let p = i as f32 / 32.0;
        let pos = track_position(p, 0.0);
        let next_pos = track_position((i + 1) as f32 / 32.0, 0.0);
        let dir = next_pos - pos;
        let len = dir.length();
        let mid = (pos + next_pos) * 0.5;
        let angle = dir.z.atan2(dir.x);

        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(len, 0.05, TRACK_WIDTH))),
            MeshMaterial3d(track_mat.clone()),
            Transform::from_xyz(mid.x, 0.01, mid.z)
                .with_rotation(Quat::from_rotation_y(-angle)),
            RaceEntity,
        ));
    }

    // Finish line
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.3, 0.1, TRACK_WIDTH))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 1.0, 1.0),
            ..default()
        })),
        Transform::from_xyz(TRACK_RADIUS_X, 0.05, 0.0),
        RaceEntity,
    ));
    // Finish arch
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.2, 3.0, 0.2))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.9, 0.9, 0.9),
            ..default()
        })),
        Transform::from_xyz(TRACK_RADIUS_X, 1.5, -TRACK_WIDTH / 2.0),
        RaceEntity,
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.2, 3.0, 0.2))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.9, 0.9, 0.9),
            ..default()
        })),
        Transform::from_xyz(TRACK_RADIUS_X, 1.5, TRACK_WIDTH / 2.0),
        RaceEntity,
    ));

    // AI racers
    let ai_colors = [
        Color::srgb(0.2, 0.4, 0.9),
        Color::srgb(0.9, 0.8, 0.1),
        Color::srgb(0.1, 0.8, 0.3),
    ];
    let ai_lanes = [(-1.0f32), 0.0, 1.0];
    for (i, (&color, &lane)) in ai_colors.iter().zip(ai_lanes.iter()).enumerate() {
        let start_pos = track_position(0.0, lane);
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.5, 0.8, 0.5))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                ..default()
            })),
            Transform::from_xyz(start_pos.x, 0.4, start_pos.z),
            AiRacer {
                lane: i as u8,
                progress: 0.0,
                lap: 0,
            },
            RaceEntity,
        ));
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
                Text::new("Lap 0 / 3 | Pos: 4th"),
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
                    Text::new("Your legs feel like lead! The race is rigged. Your speed is a float -- find where compute_player_race_speed() reads it."),
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

// --- Gameplay ---

fn race_playing_update(
    time: Res<Time>,
    stats: Res<RacerStats>,
    mut player_race: ResMut<PlayerRaceState>,
    mut next_phase: ResMut<NextState<RacePhase>>,
    player_q: Query<&Transform, (With<Player>, Without<AiRacer>)>,
    mut ai_q: Query<(&mut AiRacer, &mut Transform), Without<Player>>,
    game_paused: Res<GamePaused>,
) {
    if game_paused.0 { return; }
    let dt = time.delta_secs();

    // Update player race progress from position
    if let Ok(pt) = player_q.get_single() {
        let prev_progress = player_race.progress;
        let new_progress = progress_from_position(pt.translation);

        // Detect lap completion (crossing from ~1.0 back to ~0.0)
        if prev_progress > 0.8 && new_progress < 0.2 {
            player_race.lap += 1;
        }
        player_race.progress = new_progress;
    }

    // Check player victory
    if check_race_victory(player_race.lap, stats.laps_to_win) {
        next_phase.set(RacePhase::Victory);
        return;
    }

    // Update AI racers
    let _player_speed = compute_player_race_speed(&stats);
    let ai_speed = compute_ai_race_speed(&stats);

    for (mut ai, mut t) in &mut ai_q {
        let speed_factor = ai_speed / (TRACK_RADIUS_X * std::f32::consts::TAU);
        ai.progress += speed_factor * dt;
        if ai.progress >= 1.0 {
            ai.progress -= 1.0;
            ai.lap += 1;
        }

        let lane_offset = (ai.lane as f32 - 1.0) * 1.2;
        let pos = track_position(ai.progress, lane_offset);
        t.translation = Vec3::new(pos.x, 0.4, pos.z);

        // Check AI victory
        if ai.lap >= stats.laps_to_win {
            next_phase.set(RacePhase::Lost);
            return;
        }
    }
}

// --- Visual ---

#[allow(clippy::too_many_arguments)]
fn race_visual_update(
    time: Res<Time>,
    stats: Res<RacerStats>,
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

    // Camera
    if let (Ok(pt), Ok(mut ct)) = (player_q.get_single(), camera_q.get_single_mut()) {
        let target = pt.translation + Vec3::new(0.0, 14.0, 14.0);
        let t = (6.0 * dt).min(1.0);
        ct.translation = ct.translation.lerp(target, t);
        ct.look_at(pt.translation + Vec3::Y, Vec3::Y);
    }

    // HUD
    if let Ok(mut text) = text_q.get_single_mut() {
        let mut position = 1;
        for ai in &ai_q {
            let ai_total = ai.lap as f32 + ai.progress;
            let player_total = player_race.lap as f32 + player_race.progress;
            if ai_total > player_total {
                position += 1;
            }
        }
        let pos_str = match position {
            1 => "1st",
            2 => "2nd",
            3 => "3rd",
            _ => "4th",
        };
        **text = format!(
            "Lap {} / {} | Pos: {}",
            player_race.lap, stats.laps_to_win, pos_str
        );
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
    mut next_phase: ResMut<NextState<RacePhase>>,
    mut scoreboard: ResMut<Scoreboard>,
    mut player_q: Query<(&mut Transform, &mut PlayerPhysics), With<Player>>,
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
        if let Ok((mut t, mut p)) = player_q.get_single_mut() {
            t.translation = PLAYER_SPAWN;
            p.velocity = Vec3::ZERO;
            p.grounded = true;
        }
        next_phase.set(RacePhase::Playing);
        return;
    }
}

// --- Lost ---

fn handle_lost(
    mut commands: Commands,
    mut events: EventReader<KeyboardInput>,
    mut next_phase: ResMut<NextState<RacePhase>>,
    mut stats: ResMut<RacerStats>,
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
        for (mut ai, mut t) in &mut ai_q {
            ai.progress = 0.0;
            ai.lap = 0;
            let lane_offset = (ai.lane as f32 - 1.0) * 1.2;
            let pos = track_position(0.0, lane_offset);
            t.translation = Vec3::new(pos.x, 0.4, pos.z);
        }
        if let Ok((mut t, mut p)) = player_q.get_single_mut() {
            t.translation = PLAYER_SPAWN;
            p.velocity = Vec3::ZERO;
            p.grounded = true;
        }
        next_phase.set(RacePhase::Playing);
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

    #[test]
    fn player_speed_is_rigged() {
        let stats = RacerStats::default();
        assert_eq!(compute_player_race_speed(&stats), 3.0);
        assert_eq!(compute_ai_race_speed(&stats), 20.0);
        // AI is almost 7x faster than player
        assert!(stats.ai_speed / stats.player_speed > 6.0);
    }

    #[test]
    fn race_victory_at_3_laps() {
        let stats = RacerStats::default();
        assert!(!check_race_victory(0, stats.laps_to_win));
        assert!(!check_race_victory(1, stats.laps_to_win));
        assert!(!check_race_victory(2, stats.laps_to_win));
        assert!(check_race_victory(3, stats.laps_to_win));
        assert!(check_race_victory(4, stats.laps_to_win));
    }

    #[test]
    fn ai_wins_before_player_normally() {
        let stats = RacerStats::default();
        // Simulate race: AI finishes 3 laps before player
        let track_len = TRACK_RADIUS_X * std::f32::consts::TAU;
        let ai_time_per_lap = track_len / stats.ai_speed;
        let player_time_per_lap = track_len / stats.player_speed;

        let ai_total = ai_time_per_lap * stats.laps_to_win as f32;
        let player_total = player_time_per_lap * stats.laps_to_win as f32;

        // AI finishes WAY before player
        assert!(ai_total < player_total,
            "AI finishes in {:.1}s, player in {:.1}s", ai_total, player_total);
    }

    #[test]
    fn debugger_scenario_boost_player_speed() {
        let mut stats = RacerStats::default();
        stats.player_speed = 50.0; // debugger sets this
        assert_eq!(compute_player_race_speed(&stats), 50.0);
        assert!(stats.player_speed > stats.ai_speed);
    }

    #[test]
    fn debugger_scenario_reduce_ai_speed() {
        let mut stats = RacerStats::default();
        stats.ai_speed = 1.0; // debugger sets this
        assert_eq!(compute_ai_race_speed(&stats), 1.0);
        assert!(stats.player_speed > stats.ai_speed);
    }

    #[test]
    fn debugger_scenario_set_laps_to_1() {
        let mut stats = RacerStats::default();
        stats.laps_to_win = 1; // debugger sets this
        assert!(check_race_victory(1, stats.laps_to_win));
    }

    #[test]
    fn track_positions_form_oval() {
        let p0 = track_position(0.0, 0.0);
        let p25 = track_position(0.25, 0.0);
        let p50 = track_position(0.5, 0.0);
        let p75 = track_position(0.75, 0.0);

        // Should form an oval shape
        assert!((p0.x - TRACK_RADIUS_X).abs() < 0.1); // right side
        assert!((p50.x + TRACK_RADIUS_X).abs() < 0.1); // left side
        assert!(p25.z.abs() > 1.0); // top
        assert!(p75.z.abs() > 1.0); // bottom
    }
}
