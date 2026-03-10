use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;

use crate::player::{
    animate_player, escape_to_menu, player_movement, spawn_player, toggle_pause, MovementBounds,
    Player, PlayerMovementSet, PlayerPhysics,
};
use crate::{ArenaPhase, GamePaused, Screen, Scoreboard};

pub struct Level9Plugin;

impl Plugin for Level9Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::ArenaChallenge), setup_arena)
            .add_systems(
                Update,
                (player_movement.in_set(PlayerMovementSet), arena_playing_update)
                    .chain()
                    .run_if(in_state(ArenaPhase::Playing)),
            )
            .add_systems(
                Update,
                (animate_player, arena_visual_update)
                    .after(PlayerMovementSet)
                    .run_if(in_state(Screen::ArenaChallenge)),
            )
            .add_systems(
                Update,
                (escape_to_menu, toggle_pause).run_if(in_state(ArenaPhase::Playing)),
            )
            .add_systems(
                Update,
                handle_victory.run_if(in_state(ArenaPhase::Victory)),
            )
            .add_systems(
                Update,
                handle_lost.run_if(in_state(ArenaPhase::Lost)),
            )
            .add_systems(OnExit(Screen::ArenaChallenge), cleanup_arena);
    }
}

// --- Components ---

#[derive(Component)]
struct ArenaEntity;

#[derive(Component)]
struct ArenaFollowCam;

#[repr(C)]
#[derive(Component)]
pub(crate) struct Fighter {
    pub(crate) health: f32,
    pub(crate) team: i32,
    name: [u8; 16],
    attack_timer: f32,
    _decoy: i32,
}

#[derive(Component)]
struct TeamHudText;

#[derive(Component)]
struct ArenaHintBox;

#[derive(Component)]
struct ArenaHintCloseButton;

#[derive(Component)]
struct OverlayScreen;

// --- Resources ---

#[derive(Resource)]
struct ArenaState {
    elapsed: f32,
    draw_timeout: f32,
}

impl Default for ArenaState {
    fn default() -> Self {
        Self {
            elapsed: 0.0,
            draw_timeout: 60.0,
        }
    }
}

// --- Debugger-target functions ---

#[inline(never)]
fn apply_arena_damage(fighter: &mut Fighter, damage: f32) {
    fighter.health -= damage;
    if fighter.health < 0.0 {
        fighter.health = 0.0;
    }
}

#[inline(never)]
fn check_arena_victory(allies_alive: bool, enemies_alive: bool) -> i32 {
    match (allies_alive, enemies_alive) {
        (true, false) => 1,
        (false, _) => -1,
        _ => 0,
    }
}

// --- Constants ---

const ARENA_MIN: Vec2 = Vec2::new(-8.0, -8.0);
const ARENA_MAX: Vec2 = Vec2::new(8.0, 8.0);
const PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 0.0, 7.0);

fn make_name(s: &str) -> [u8; 16] {
    let mut arr = [0u8; 16];
    for (i, b) in s.bytes().enumerate() {
        if i >= 16 { break; }
        arr[i] = b;
    }
    arr
}

// --- Setup ---

fn setup_arena(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    scoreboard: Res<Scoreboard>,
) {
    commands.insert_resource(ClearColor(Color::srgb(0.5, 0.4, 0.3)));
    commands.insert_resource(ArenaState::default());

    // Sandy arena floor
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(16.0, 16.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.76, 0.65, 0.45),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
        ArenaEntity,
    ));

    // Colosseum walls
    let wall_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.6, 0.55, 0.45),
        ..default()
    });
    let wall_mesh = meshes.add(Cuboid::new(16.0, 5.0, 0.5));
    for (pos, rot) in [
        (Vec3::new(0.0, 2.5, -8.0), 0.0f32),
        (Vec3::new(0.0, 2.5, 8.0), 0.0),
        (Vec3::new(-8.0, 2.5, 0.0), std::f32::consts::FRAC_PI_2),
        (Vec3::new(8.0, 2.5, 0.0), std::f32::consts::FRAC_PI_2),
    ] {
        commands.spawn((
            Mesh3d(wall_mesh.clone()), MeshMaterial3d(wall_mat.clone()),
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(rot)),
            ArenaEntity,
        ));
    }

    // Spectator arches (decorative)
    let arch_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.5, 0.4),
        ..default()
    });
    let arch_mesh = meshes.add(Cuboid::new(0.3, 1.5, 0.3));
    for i in 0..8 {
        let x = (i as f32 - 3.5) * 2.0;
        commands.spawn((
            Mesh3d(arch_mesh.clone()), MeshMaterial3d(arch_mat.clone()),
            Transform::from_xyz(x, 5.75, -7.8), ArenaEntity,
        ));
        commands.spawn((
            Mesh3d(arch_mesh.clone()), MeshMaterial3d(arch_mat.clone()),
            Transform::from_xyz(x, 5.75, 7.8), ArenaEntity,
        ));
    }

    // Blue team allies
    let blue_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.4, 0.9),
        ..default()
    });
    let fighter_mesh = meshes.add(Cuboid::new(0.7, 1.2, 0.7));

    let ally_data = [
        ("Bolt", Vec3::new(-3.0, 0.6, -3.0)),
        ("Spark", Vec3::new(-3.0, 0.6, 3.0)),
    ];
    for (name, pos) in &ally_data {
        commands.spawn((
            Mesh3d(fighter_mesh.clone()),
            MeshMaterial3d(blue_mat.clone()),
            Transform::from_translation(*pos),
            Fighter {
                health: 100.0,
                team: 1,
                name: make_name(name),
                attack_timer: 0.0,
                _decoy: 42,
            },
            ArenaEntity,
        ));
    }

    // Red team enemies
    let red_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.2, 0.15),
        ..default()
    });

    let enemy_data = [
        ("Fang", Vec3::new(3.0, 0.6, -3.0)),
        ("Claw", Vec3::new(3.0, 0.6, 3.0)),
    ];
    for (name, pos) in &enemy_data {
        commands.spawn((
            Mesh3d(fighter_mesh.clone()),
            MeshMaterial3d(red_mat.clone()),
            Transform::from_translation(*pos),
            Fighter {
                health: 500.0,
                team: 2,
                name: make_name(name),
                attack_timer: 0.0,
                _decoy: -99,
            },
            ArenaEntity,
        ));
    }

    // Player
    let player = spawn_player(&mut commands, &mut meshes, &mut materials);
    commands.entity(player).insert((
        ArenaEntity,
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
        ArenaFollowCam,
        ArenaEntity,
    ));

    // Lighting
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.7, 0.3, 0.0)),
        ArenaEntity,
    ));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.9, 0.85, 0.7),
        brightness: 400.0,
    });

    // HUD - team status
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                left: Val::Px(16.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.1, 0.15, 0.85)),
            BorderRadius::all(Val::Px(12.0)),
            ArenaEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Blue: 200 HP | Red: 1000 HP | 0.0s"),
                TextFont { font_size: 20.0, ..default() },
                TextColor(Color::srgb(1.0, 1.0, 1.0)),
                TeamHudText,
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
            ArenaEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("[Esc] Menu | WASD Move | Space Jump | [P] Pause"),
                TextFont { font_size: 20.0, ..default() },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
            ));
        });

    // Hint
    if !scoreboard.arena_solved {
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
                ArenaHintBox,
                ArenaEntity,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new("Hint"),
                    TextFont { font_size: 20.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.85, 0.3)),
                ));
                parent.spawn((
                    Node { max_width: Val::Px(250.0), ..default() },
                    Text::new("The same function damages everyone! Stopping damage causes a draw. Look at the team field on each Fighter when apply_arena_damage() is called."),
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
                        ArenaHintCloseButton,
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

fn arena_playing_update(
    time: Res<Time>,
    mut arena_state: ResMut<ArenaState>,
    mut next_phase: ResMut<NextState<ArenaPhase>>,
    mut fighters: Query<&mut Fighter>,
    game_paused: Res<GamePaused>,
) {
    if game_paused.0 { return; }
    let dt = time.delta_secs();
    arena_state.elapsed += dt;

    // Apply damage to all fighters
    for mut fighter in &mut fighters {
        fighter.attack_timer += dt;
        fighter._decoy = (arena_state.elapsed * 100.0) as i32 % 999;

        if fighter.health <= 0.0 {
            continue;
        }

        let tick_interval = 0.5;
        if fighter.attack_timer >= tick_interval {
            fighter.attack_timer -= tick_interval;

            let damage = if fighter.team == 1 {
                // Allies take heavy damage (5-10)
                5.0 + (arena_state.elapsed * 3.0).sin().abs() * 5.0
            } else {
                // Enemies take light damage (1-2)
                1.0 + (arena_state.elapsed * 7.0).sin().abs() * 1.0
            };
            apply_arena_damage(&mut fighter, damage);
        }
    }

    // Check victory conditions
    let mut allies_alive = false;
    let mut enemies_alive = false;
    for fighter in &fighters {
        if fighter.health > 0.0 {
            if fighter.team == 1 {
                allies_alive = true;
            } else {
                enemies_alive = true;
            }
        }
    }

    let result = check_arena_victory(allies_alive, enemies_alive);
    match result {
        1 => next_phase.set(ArenaPhase::Victory),
        -1 => next_phase.set(ArenaPhase::Lost),
        _ => {}
    }

    // Draw timeout
    if arena_state.elapsed >= arena_state.draw_timeout {
        next_phase.set(ArenaPhase::Lost);
    }
}

// --- Visual ---

#[allow(clippy::too_many_arguments)]
fn arena_visual_update(
    time: Res<Time>,
    arena_state: Res<ArenaState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    player_q: Query<&Transform, (With<Player>, Without<ArenaFollowCam>)>,
    mut camera_q: Query<&mut Transform, (With<ArenaFollowCam>, Without<Player>)>,
    fighters: Query<&Fighter>,
    mut text_q: Query<&mut Text, With<TeamHudText>>,
    hint_q: Query<Entity, With<ArenaHintBox>>,
    btn_q: Query<&Interaction, (Changed<Interaction>, With<ArenaHintCloseButton>)>,
) {
    let dt = time.delta_secs();

    // Camera
    if let (Ok(pt), Ok(mut ct)) = (player_q.get_single(), camera_q.get_single_mut()) {
        let target = pt.translation + Vec3::new(0.0, 12.0, 12.0);
        let t = (6.0 * dt).min(1.0);
        ct.translation = ct.translation.lerp(target, t);
        ct.look_at(Vec3::ZERO + Vec3::Y, Vec3::Y);
    }

    // HUD
    if let Ok(mut text) = text_q.get_single_mut() {
        let mut blue_hp = 0.0f32;
        let mut red_hp = 0.0f32;
        for f in &fighters {
            if f.team == 1 { blue_hp += f.health; }
            else { red_hp += f.health; }
        }
        **text = format!(
            "Blue: {:.0} HP | Red: {:.0} HP | {:.1}s",
            blue_hp, red_hp, arena_state.elapsed
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
    mut next_phase: ResMut<NextState<ArenaPhase>>,
    mut scoreboard: ResMut<Scoreboard>,
    mut player_q: Query<(&mut Transform, &mut PlayerPhysics), With<Player>>,
    overlay_q: Query<Entity, With<OverlayScreen>>,
) {
    if overlay_q.is_empty() {
        scoreboard.arena_solved = true;
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
                ArenaEntity,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new("VICTORY!"),
                    TextFont { font_size: 52.0, ..default() },
                    TextColor(Color::srgb(0.2, 1.0, 0.2)),
                ));
                parent.spawn((
                    Text::new("Your team won the battle!"),
                    TextFont { font_size: 28.0, ..default() },
                    TextColor(Color::srgb(0.8, 1.0, 0.8)),
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
        next_phase.set(ArenaPhase::Playing);
        return;
    }
}

// --- Lost ---

fn handle_lost(
    mut commands: Commands,
    mut events: EventReader<KeyboardInput>,
    mut next_phase: ResMut<NextState<ArenaPhase>>,
    mut arena_state: ResMut<ArenaState>,
    mut player_q: Query<(&mut Transform, &mut PlayerPhysics), (With<Player>, Without<Fighter>)>,
    mut fighters: Query<&mut Fighter>,
    overlay_q: Query<Entity, With<OverlayScreen>>,
) {
    if overlay_q.is_empty() {
        let reason = if arena_state.elapsed >= arena_state.draw_timeout {
            "Draw! Nobody won in time."
        } else {
            "Your team was defeated!"
        };
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
                ArenaEntity,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new("DEFEAT"),
                    TextFont { font_size: 52.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.2, 0.2)),
                ));
                parent.spawn((
                    Text::new(reason),
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
        *arena_state = ArenaState::default();
        // Reset fighters
        for mut f in &mut fighters {
            if f.team == 1 {
                f.health = 100.0;
            } else {
                f.health = 500.0;
            }
            f.attack_timer = 0.0;
        }
        if let Ok((mut t, mut p)) = player_q.get_single_mut() {
            t.translation = PLAYER_SPAWN;
            p.velocity = Vec3::ZERO;
            p.grounded = true;
        }
        next_phase.set(ArenaPhase::Playing);
        return;
    }
}

// --- Cleanup ---

fn cleanup_arena(mut commands: Commands, query: Query<Entity, With<ArenaEntity>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arena_damage_reduces_health() {
        let mut fighter = Fighter {
            health: 100.0, team: 1, name: make_name("Test"),
            attack_timer: 0.0, _decoy: 0,
        };
        apply_arena_damage(&mut fighter, 10.0);
        assert!((fighter.health - 90.0).abs() < 0.001);
    }

    #[test]
    fn arena_damage_floors_at_zero() {
        let mut fighter = Fighter {
            health: 5.0, team: 2, name: make_name("Test"),
            attack_timer: 0.0, _decoy: 0,
        };
        apply_arena_damage(&mut fighter, 20.0);
        assert_eq!(fighter.health, 0.0);
    }

    #[test]
    fn victory_when_allies_alive_enemies_dead() {
        assert_eq!(check_arena_victory(true, false), 1);
    }

    #[test]
    fn loss_when_allies_dead() {
        assert_eq!(check_arena_victory(false, true), -1);
        assert_eq!(check_arena_victory(false, false), -1);
    }

    #[test]
    fn ongoing_when_both_alive() {
        assert_eq!(check_arena_victory(true, true), 0);
    }

    #[test]
    fn fight_is_rigged_against_allies() {
        // Simulate: allies take 5-10 dmg/tick, enemies 1-2 dmg/tick
        // Allies have 100 HP each, enemies have 500 HP each
        let mut allies = [
            Fighter { health: 100.0, team: 1, name: make_name("Bolt"), attack_timer: 0.0, _decoy: 0 },
            Fighter { health: 100.0, team: 1, name: make_name("Spark"), attack_timer: 0.0, _decoy: 0 },
        ];
        let mut enemies = [
            Fighter { health: 500.0, team: 2, name: make_name("Fang"), attack_timer: 0.0, _decoy: 0 },
            Fighter { health: 500.0, team: 2, name: make_name("Claw"), attack_timer: 0.0, _decoy: 0 },
        ];

        // Simulate ticks
        for tick in 0..200 {
            let t = tick as f32;
            for ally in &mut allies {
                if ally.health > 0.0 {
                    let dmg = 5.0 + (t * 3.0).sin().abs() * 5.0;
                    apply_arena_damage(ally, dmg);
                }
            }
            for enemy in &mut enemies {
                if enemy.health > 0.0 {
                    let dmg = 1.0 + (t * 7.0).sin().abs() * 1.0;
                    apply_arena_damage(enemy, dmg);
                }
            }
        }

        let allies_alive = allies.iter().any(|f| f.health > 0.0);
        let enemies_alive = enemies.iter().any(|f| f.health > 0.0);

        // Allies should die first (fight is rigged)
        assert!(!allies_alive, "Allies should be dead");
        assert!(enemies_alive, "Enemies should still be alive");
        assert_eq!(check_arena_victory(allies_alive, enemies_alive), -1);
    }

    #[test]
    fn debugger_scenario_kill_enemies() {
        // Simulates: player sets enemy health to 0 when apply_arena_damage is called
        let mut enemy = Fighter {
            health: 500.0, team: 2, name: make_name("Fang"),
            attack_timer: 0.0, _decoy: 0,
        };
        enemy.health = 0.0; // debugger sets this
        assert_eq!(enemy.health, 0.0);

        // With enemies dead, allies win
        assert_eq!(check_arena_victory(true, false), 1);
    }

    #[test]
    fn nopping_damage_causes_draw() {
        // If apply_arena_damage is NOPped (skipped), nobody dies
        // After 60s timeout, that's a draw/loss, not a win
        // So NOPping is NOT a valid solution
        let allies_alive = true;
        let enemies_alive = true;
        assert_eq!(check_arena_victory(allies_alive, enemies_alive), 0); // ongoing
        // After timeout, game declares loss (draw counts as loss)
    }

    #[test]
    fn team_field_distinguishes_fighters() {
        let ally = Fighter {
            health: 100.0, team: 1, name: make_name("Bolt"),
            attack_timer: 0.0, _decoy: 42,
        };
        let enemy = Fighter {
            health: 500.0, team: 2, name: make_name("Fang"),
            attack_timer: 0.0, _decoy: -99,
        };
        assert_eq!(ally.team, 1);
        assert_eq!(enemy.team, 2);
    }
}
