use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;

use crate::player::{
    animate_player, escape_to_menu, player_movement, spawn_player, toggle_pause, MovementBounds,
    Player, PlayerMovementSet, PlayerPhysics,
};
use crate::shared_ui;
use crate::{CountdownPhase, GamePaused, Screen, Scoreboard};

/// Long-form walkthrough shown in the tutorial modal (opened with T).
const BOMB_TUTORIAL: &str = "\
The bomb's fuse (BombTimer.remaining, an f32) starts at 20s, and the bomb explodes the instant it reaches 0. To win you must survive 5 more seconds - the bomb defuses itself once it has stayed alive for 25s. There is no way to last that long by playing, so you must stop the fuse from reaching 0.

Method 1 - freeze the value:
1) Find BombTimer.remaining in memory: search for the countdown (e.g. 20.0) as a 4-byte float, then narrow the results as it ticks down.
2) Freeze that address at a positive value - use your debugger's freeze/lock feature, or run a small loop/thread that rewrites (say) 20.0 to it every frame.
3) The fuse never hits 0, the survival timer keeps counting, and the bomb defuses at 25s.

Method 2 - patch out the decrement:
1) Break on tick_bomb_timer and find the instruction doing remaining -= delta (a floating-point subtract writing back to the remaining field).
2) Replace it with NOPs so the subtraction never runs.
3) The fuse stays put, the survival timer reaches 25s, and the bomb defuses.";

pub struct Level3Plugin;

impl Plugin for Level3Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::CountdownChallenge), setup_countdown)
            .add_systems(
                Update,
                (
                    shared_ui::update_camera_orbit.before(PlayerMovementSet),
                    player_movement.in_set(PlayerMovementSet),
                    countdown_playing_update,
                )
                    .chain()
                    .run_if(in_state(CountdownPhase::Playing)),
            )
            .add_systems(
                Update,
                (
                    animate_player,
                    countdown_visual_update,
                    shared_ui::follow_camera_system,
                    shared_ui::hint_tutorial_controls,
                )
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

#[derive(Component, Clone, Copy)]
struct CountdownEntity;

#[derive(Component)]
struct TimerHudText;

#[derive(Component)]
struct BombVisual;

// --- Resources ---

#[repr(C)]
#[derive(Resource)]
pub struct BombTimer {
    /// The fuse. Counts down; the bomb explodes when this reaches 0. The player
    /// must keep this above 0 (by freezing it or NOP-ing the decrement below).
    pub remaining: f32,
    /// Real time the bomb has stayed alive. It auto-defuses at `DEFUSE_AT`.
    pub elapsed: f32,
    pub defused: bool,
}

impl Default for BombTimer {
    fn default() -> Self {
        Self {
            remaining: 20.0,
            elapsed: 0.0,
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
    // Freeze target / NOP target: keeping this subtraction from running (or
    // rewriting `remaining` to a positive value) stops the fuse burning down.
    timer.remaining -= delta;
    // The bomb defuses itself once it has survived DEFUSE_AT seconds.
    timer.elapsed += delta;
    if timer.elapsed >= DEFUSE_AT {
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
/// The bomb auto-defuses once it has stayed alive this many seconds: 5 seconds
/// past the 20s fuse. The fuse always burns out at 20s first, so it must be
/// frozen or its decrement patched out to survive the extra 5 seconds.
const DEFUSE_AT: f32 = 25.0;

// --- Setup ---

fn setup_countdown(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    _scoreboard: Res<Scoreboard>,
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
        shared_ui::FollowCamera {
            offset: Vec3::new(0.0, 10.0, 12.0),
            
            look_offset: Vec3::Y,
        },
        CountdownEntity,
    ));

    // Lighting
    shared_ui::setup_level_lighting(
        &mut commands,
        5000.0,
        (-std::f32::consts::FRAC_PI_4, std::f32::consts::FRAC_PI_6, 0.0),
        Color::srgb(0.9, 0.7, 0.5),
        200.0,
        CountdownEntity,
    );

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
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(20.0), Val::Px(8.0)),
                row_gap: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.8, 0.1, 0.05, 0.85)),
            BorderRadius::all(Val::Px(12.0)),
            CountdownEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Fuse: 20.0"),
                TextFont {
                    font_size: 32.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 1.0, 1.0)),
                TimerHudText,
            ));
        });

    // HUD - controls
    shared_ui::spawn_controls_hint(
        &mut commands,
        "Survive 5 seconds after the bomb would explode",
        CountdownEntity,
    );

    // HUD - hint box + tutorial modal (hidden; H reveals the hint, T the tutorial)
    shared_ui::spawn_hint_box_with_tutorial(
        &mut commands,
        "The bomb explodes at 0, but you must last 5 more seconds. Freeze the fuse, or stop it from ticking down.",
        320.0,
        CountdownEntity,
    );
    shared_ui::spawn_hint_modal(
        &mut commands,
        "Bomb - Full Solution",
        BOMB_TUTORIAL,
        CountdownEntity,
    );
}

// --- Gameplay system ---

fn countdown_playing_update(
    time: Res<Time>,
    mut bomb: ResMut<BombTimer>,
    mut next_phase: ResMut<NextState<CountdownPhase>>,
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
    }
}

// --- Visual update ---

fn countdown_visual_update(
    bomb: Res<BombTimer>,
    mut text_q: Query<(&mut Text, &mut TextColor), With<TimerHudText>>,
) {
    // Fuse HUD
    if let Ok((mut text, mut color)) = text_q.get_single_mut() {
        let display = bomb.remaining.max(0.0);
        **text = format!("Fuse: {:.1}", display);
        if bomb.remaining < 10.0 {
            *color = TextColor(Color::srgb(1.0, 0.3, 0.3));
        } else {
            *color = TextColor(Color::srgb(1.0, 1.0, 1.0));
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
        scoreboard.set_solved(3);
        shared_ui::spawn_victory_overlay(
            &mut commands,
            "BOMB DEFUSED!",
            None,
            0.0,
            "Press any key to continue",
            CountdownEntity,
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

// --- Exploded ---

fn handle_exploded(
    mut commands: Commands,
    mut events: EventReader<KeyboardInput>,
    mut next_phase: ResMut<NextState<CountdownPhase>>,
    mut bomb: ResMut<BombTimer>,
    mut player_q: Query<(&mut Transform, &mut PlayerPhysics), With<Player>>,
    overlay_q: Query<Entity, With<shared_ui::OverlayScreen>>,
) {
    if overlay_q.is_empty() {
        shared_ui::spawn_defeat_overlay(
            &mut commands,
            "BOOM!",
            64.0,
            Some("The bomb exploded!"),
            28.0,
            "Press any key to retry",
            Color::srgba(0.3, 0.05, 0.0, 0.85),
            CountdownEntity,
        );
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
        let mut timer = BombTimer { remaining: 20.0, elapsed: 0.0, defused: false };
        tick_bomb_timer(&mut timer, 1.0);
        assert!((timer.remaining - 19.0).abs() < 0.001);
        assert!((timer.elapsed - 1.0).abs() < 0.001);
    }

    #[test]
    fn bomb_does_not_tick_when_defused() {
        let mut timer = BombTimer { remaining: 20.0, elapsed: 0.0, defused: true };
        tick_bomb_timer(&mut timer, 5.0);
        assert!((timer.remaining - 20.0).abs() < 0.001);
        assert!((timer.elapsed - 0.0).abs() < 0.001);
    }

    #[test]
    fn bomb_defuses_after_survival_window() {
        // Kept alive (fuse never runs out here), the bomb auto-defuses at DEFUSE_AT.
        let mut timer = BombTimer { remaining: 1000.0, elapsed: 0.0, defused: false };
        let steps = (DEFUSE_AT / 0.1) as i32 + 2;
        for _ in 0..steps {
            tick_bomb_timer(&mut timer, 0.1);
        }
        assert!(timer.defused);
        assert!(check_bomb_defused(&timer));
    }

    #[test]
    fn normal_play_explodes_before_defuse() {
        // The 20s fuse burns out before the 25s survival window.
        let mut timer = BombTimer { remaining: 20.0, elapsed: 0.0, defused: false };
        loop {
            tick_bomb_timer(&mut timer, 0.1);
            if timer.defused {
                break;
            }
            if timer.remaining <= 0.0 {
                break; // would explode (handled by the Exploded phase in-game)
            }
        }
        assert!(!timer.defused);
        assert!(timer.elapsed < DEFUSE_AT);
    }

    #[test]
    fn debugger_scenario_freeze_timer() {
        // Freezing `remaining` at a positive value lets `elapsed` reach DEFUSE_AT.
        let mut timer = BombTimer { remaining: 20.0, elapsed: 0.0, defused: false };
        let steps = (DEFUSE_AT / 0.1) as i32 + 2;
        for _ in 0..steps {
            tick_bomb_timer(&mut timer, 0.1);
            timer.remaining = 20.0; // freeze: keep rewriting a safe value
        }
        assert!(timer.defused);
        assert!(check_bomb_defused(&timer));
    }

    #[test]
    fn debugger_scenario_patch_out_decrement() {
        // NOP-ing `remaining -= delta` is modelled by advancing time without
        // letting the fuse drop: elapsed still reaches DEFUSE_AT and defuses.
        let mut timer = BombTimer { remaining: 20.0, elapsed: 0.0, defused: false };
        let steps = (DEFUSE_AT / 0.1) as i32 + 2;
        for _ in 0..steps {
            let before = timer.remaining;
            tick_bomb_timer(&mut timer, 0.1);
            timer.remaining = before; // decrement patched out
        }
        assert!(timer.defused);
        assert!((timer.remaining - 20.0).abs() < 0.001);
    }
}
