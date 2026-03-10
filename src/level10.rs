use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;

use crate::player::{
    animate_player, escape_to_menu, player_movement, spawn_player, toggle_pause, MovementBounds,
    Player, PlayerMovementSet, PlayerPhysics,
};
use crate::{GamePaused, LootPhase, Screen, Scoreboard};

pub struct Level10Plugin;

impl Plugin for Level10Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::LootChallenge), setup_loot)
            .add_systems(
                Update,
                (player_movement.in_set(PlayerMovementSet), loot_playing_update)
                    .chain()
                    .run_if(in_state(LootPhase::Playing)),
            )
            .add_systems(
                Update,
                (animate_player, loot_visual_update)
                    .after(PlayerMovementSet)
                    .run_if(in_state(Screen::LootChallenge)),
            )
            .add_systems(
                Update,
                (escape_to_menu, toggle_pause).run_if(in_state(LootPhase::Playing)),
            )
            .add_systems(
                Update,
                handle_victory.run_if(in_state(LootPhase::Victory)),
            )
            .add_systems(OnExit(Screen::LootChallenge), cleanup_loot);
    }
}

// --- Components ---

#[derive(Component)]
struct LootEntity;

#[derive(Component)]
struct LootFollowCam;

#[derive(Component)]
struct GoblinNpc;

#[derive(Component)]
struct ExitDoor;

#[derive(Component)]
struct LootHudText;

#[derive(Component)]
struct LootHintBox;

#[derive(Component)]
struct LootHintCloseButton;

#[derive(Component)]
struct OverlayScreen;

// --- Resources ---

#[repr(C)]
#[derive(Clone)]
struct LootEntry {
    item_id: u32,
    weight: f32,
    _name: [u8; 16],
}

#[derive(Resource)]
pub struct LootTable {
    entries: Vec<LootEntry>,
}

impl Default for LootTable {
    fn default() -> Self {
        Self {
            entries: vec![
                LootEntry { item_id: 0, weight: 90.0, _name: *b"Pebble\0\0\0\0\0\0\0\0\0\0" },
                LootEntry { item_id: 1, weight: 8.0, _name: *b"Green Gem\0\0\0\0\0\0\0" },
                LootEntry { item_id: 2, weight: 1.9, _name: *b"Blue Gem\0\0\0\0\0\0\0\0" },
                LootEntry { item_id: 3, weight: 0.1, _name: *b"Golden Key\0\0\0\0\0\0" },
            ],
        }
    }
}

#[derive(Resource, Default)]
pub(crate) struct PlayerLoot {
    pub(crate) has_golden_key: bool,
    total_drops: u32,
    last_drop: Option<u32>,
}

#[derive(Resource)]
struct LootCooldown {
    timer: Timer,
}

impl Default for LootCooldown {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(3.0, TimerMode::Once),
        }
    }
}

// --- Debugger-target functions ---

#[inline(never)]
fn roll_loot(table: &LootTable, random_val: f32) -> u32 {
    let total: f32 = table.entries.iter().map(|e| e.weight).sum();
    let mut roll = random_val * total;
    for entry in &table.entries {
        roll -= entry.weight;
        if roll <= 0.0 {
            return entry.item_id;
        }
    }
    0
}

#[inline(never)]
fn check_has_key(loot: &PlayerLoot) -> bool {
    loot.has_golden_key
}

// --- Constants ---

const ARENA_MIN: Vec2 = Vec2::new(-7.0, -7.0);
const ARENA_MAX: Vec2 = Vec2::new(7.0, 7.0);
const PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 0.0, 3.0);
const GOBLIN_POS: Vec3 = Vec3::new(-3.0, 0.0, -2.0);
const DOOR_POS: Vec3 = Vec3::new(6.5, 0.0, 0.0);

// --- Setup ---

fn setup_loot(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    scoreboard: Res<Scoreboard>,
) {
    commands.insert_resource(ClearColor(Color::srgb(0.18, 0.12, 0.08)));
    commands.insert_resource(LootTable::default());
    commands.insert_resource(PlayerLoot::default());
    commands.insert_resource(LootCooldown::default());

    // Cave floor
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(14.0, 14.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.25, 0.2, 0.15),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
        LootEntity,
    ));

    // Cave walls
    let wall_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.18, 0.14),
        ..default()
    });
    let wall_h = meshes.add(Cuboid::new(14.0, 5.0, 0.5));
    let wall_v = meshes.add(Cuboid::new(0.5, 5.0, 14.0));
    commands.spawn((Mesh3d(wall_h.clone()), MeshMaterial3d(wall_mat.clone()), Transform::from_xyz(0.0, 2.5, -7.0), LootEntity));
    commands.spawn((Mesh3d(wall_h), MeshMaterial3d(wall_mat.clone()), Transform::from_xyz(0.0, 2.5, 7.0), LootEntity));
    commands.spawn((Mesh3d(wall_v.clone()), MeshMaterial3d(wall_mat.clone()), Transform::from_xyz(-7.0, 2.5, 0.0), LootEntity));
    commands.spawn((Mesh3d(wall_v), MeshMaterial3d(wall_mat), Transform::from_xyz(7.0, 2.5, 0.0), LootEntity));

    // Stalactites (hanging from ceiling)
    let stalactite_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.3, 0.25),
        ..default()
    });
    let stalactite_mesh = meshes.add(Cone { radius: 0.2, height: 1.5 });
    for i in 0..6 {
        let x = ((i * 5 + 2) % 10) as f32 - 5.0;
        let z = ((i * 7 + 3) % 10) as f32 - 5.0;
        commands.spawn((
            Mesh3d(stalactite_mesh.clone()),
            MeshMaterial3d(stalactite_mat.clone()),
            Transform::from_xyz(x, 4.5, z)
                .with_rotation(Quat::from_rotation_z(std::f32::consts::PI)),
            LootEntity,
        ));
    }

    // Goblin NPC (small green cube)
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.5, 0.6, 0.5))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.7, 0.2),
            ..default()
        })),
        Transform::from_xyz(GOBLIN_POS.x, 0.3, GOBLIN_POS.z),
        GoblinNpc,
        LootEntity,
    ));

    // Fountain (blue emissive cylinder)
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.6, 0.4))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.5, 0.9),
            emissive: LinearRgba::new(0.3, 0.5, 1.5, 1.0),
            ..default()
        })),
        Transform::from_xyz(GOBLIN_POS.x, 0.2, GOBLIN_POS.z - 1.5),
        LootEntity,
    ));

    // Exit door
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.3, 2.5, 2.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.4, 0.25, 0.1),
            ..default()
        })),
        Transform::from_xyz(DOOR_POS.x, 1.25, DOOR_POS.z),
        ExitDoor,
        LootEntity,
    ));
    // Keyhole glow
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.85, 0.2),
            intensity: 2000.0,
            range: 3.0,
            ..default()
        },
        Transform::from_xyz(DOOR_POS.x - 0.3, 1.2, DOOR_POS.z),
        LootEntity,
    ));

    // Cave lighting
    commands.spawn((
        PointLight {
            color: Color::srgb(0.8, 0.6, 0.3),
            intensity: 15000.0,
            range: 15.0,
            ..default()
        },
        Transform::from_xyz(0.0, 4.0, 0.0),
        LootEntity,
    ));
    commands.spawn((
        PointLight {
            color: Color::srgb(0.3, 0.5, 0.9),
            intensity: 5000.0,
            range: 6.0,
            ..default()
        },
        Transform::from_xyz(GOBLIN_POS.x, 1.5, GOBLIN_POS.z - 1.5),
        LootEntity,
    ));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.65, 0.55, 0.45),
        brightness: 250.0,
    });

    // Player
    let player = spawn_player(&mut commands, &mut meshes, &mut materials);
    commands.entity(player).insert((
        LootEntity,
        Transform::from_xyz(PLAYER_SPAWN.x, 0.0, PLAYER_SPAWN.z)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        MovementBounds {
            rects: vec![(ARENA_MIN, ARENA_MAX)],
        },
    ));

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 10.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        LootFollowCam,
        LootEntity,
    ));

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
            BackgroundColor(Color::srgba(0.2, 0.15, 0.1, 0.85)),
            BorderRadius::all(Val::Px(12.0)),
            LootEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Drops: 0 | Last: -"),
                TextFont { font_size: 20.0, ..default() },
                TextColor(Color::srgb(1.0, 1.0, 1.0)),
                LootHudText,
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
            LootEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("[Esc] Menu | WASD Move | Space Jump | [P] Pause"),
                TextFont { font_size: 20.0, ..default() },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
            ));
        });

    // Hint
    if !scoreboard.loot_solved {
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
                LootHintBox,
                LootEntity,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new("Hint"),
                    TextFont { font_size: 20.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.85, 0.3)),
                ));
                parent.spawn((
                    Node { max_width: Val::Px(250.0), ..default() },
                    Text::new("The goblin's loot is random... or is it? Inspect the LootTable entries to see the weights. The golden key's odds are 1 in 1000."),
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
                        LootHintCloseButton,
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

fn loot_playing_update(
    time: Res<Time>,
    loot_table: Res<LootTable>,
    mut player_loot: ResMut<PlayerLoot>,
    mut cooldown: ResMut<LootCooldown>,
    mut next_phase: ResMut<NextState<LootPhase>>,
    player_q: Query<&Transform, With<Player>>,
    game_paused: Res<GamePaused>,
) {
    if game_paused.0 { return; }
    cooldown.timer.tick(time.delta());

    let Ok(pt) = player_q.get_single() else { return; };
    let pp = pt.translation;

    // Near goblin? Auto-drop on cooldown
    let dx = pp.x - GOBLIN_POS.x;
    let dz = pp.z - GOBLIN_POS.z;
    if dx * dx + dz * dz < 4.0 && cooldown.timer.finished() {
        // Generate pseudo-random value from elapsed time
        let random_val = (time.elapsed_secs() * 1000.0).fract();
        let item_id = roll_loot(&loot_table, random_val);

        player_loot.total_drops += 1;
        player_loot.last_drop = Some(item_id);

        if item_id == 3 {
            player_loot.has_golden_key = true;
        }

        cooldown.timer.reset();
    }

    // Check if player has key and is at exit door
    if check_has_key(&player_loot) {
        let dx = pp.x - DOOR_POS.x;
        let dz = pp.z - DOOR_POS.z;
        if dx * dx + dz * dz < 3.0 {
            next_phase.set(LootPhase::Victory);
        }
    }
}

// --- Visual ---

#[allow(clippy::too_many_arguments)]
fn loot_visual_update(
    time: Res<Time>,
    player_loot: Res<PlayerLoot>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    player_q: Query<&Transform, (With<Player>, Without<LootFollowCam>, Without<GoblinNpc>)>,
    mut camera_q: Query<&mut Transform, (With<LootFollowCam>, Without<Player>, Without<GoblinNpc>)>,
    mut goblin_q: Query<&mut Transform, (With<GoblinNpc>, Without<Player>, Without<LootFollowCam>)>,
    mut text_q: Query<&mut Text, With<LootHudText>>,
    hint_q: Query<Entity, With<LootHintBox>>,
    btn_q: Query<&Interaction, (Changed<Interaction>, With<LootHintCloseButton>)>,
) {
    let dt = time.delta_secs();
    let elapsed = time.elapsed_secs();

    // Camera
    if let (Ok(pt), Ok(mut ct)) = (player_q.get_single(), camera_q.get_single_mut()) {
        let target = pt.translation + Vec3::new(0.0, 10.0, 10.0);
        let t = (6.0 * dt).min(1.0);
        ct.translation = ct.translation.lerp(target, t);
        ct.look_at(pt.translation + Vec3::Y, Vec3::Y);
    }

    // Goblin idle bounce
    for mut t in &mut goblin_q {
        t.translation.y = 0.3 + (elapsed * 3.0).sin().abs() * 0.15;
    }

    // HUD
    if let Ok(mut text) = text_q.get_single_mut() {
        let last_name = match player_loot.last_drop {
            Some(0) => "Pebble",
            Some(1) => "Green Gem",
            Some(2) => "Blue Gem",
            Some(3) => "GOLDEN KEY!",
            _ => "-",
        };
        let key_str = if player_loot.has_golden_key { " [KEY]" } else { "" };
        **text = format!("Drops: {} | Last: {}{}", player_loot.total_drops, last_name, key_str);
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
    mut next_phase: ResMut<NextState<LootPhase>>,
    mut scoreboard: ResMut<Scoreboard>,
    mut player_q: Query<(&mut Transform, &mut PlayerPhysics), With<Player>>,
    overlay_q: Query<Entity, With<OverlayScreen>>,
) {
    if overlay_q.is_empty() {
        scoreboard.loot_solved = true;
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
                LootEntity,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new("DOOR UNLOCKED!"),
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
        next_phase.set(LootPhase::Playing);
        return;
    }
}

// --- Cleanup ---

fn cleanup_loot(mut commands: Commands, query: Query<Entity, With<LootEntity>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_loot_table_has_4_entries() {
        let table = LootTable::default();
        assert_eq!(table.entries.len(), 4);
    }

    #[test]
    fn default_weights_sum_to_100() {
        let table = LootTable::default();
        let total: f32 = table.entries.iter().map(|e| e.weight).sum();
        assert!((total - 100.0).abs() < 0.01);
    }

    #[test]
    fn golden_key_has_lowest_weight() {
        let table = LootTable::default();
        let key_entry = table.entries.iter().find(|e| e.item_id == 3).unwrap();
        assert!((key_entry.weight - 0.1).abs() < 0.01);
        // Odds: 0.1 / 100.0 = 0.1% = 1 in 1000
    }

    #[test]
    fn roll_loot_returns_pebble_for_low_roll() {
        let table = LootTable::default();
        // Roll at 0.0 should give first item (pebble)
        assert_eq!(roll_loot(&table, 0.0), 0);
    }

    #[test]
    fn roll_loot_returns_golden_key_for_high_roll() {
        let table = LootTable::default();
        // Roll at ~0.999 should give last item (golden key)
        assert_eq!(roll_loot(&table, 0.999), 3);
    }

    #[test]
    fn roll_loot_distribution_is_weighted() {
        let table = LootTable::default();
        let mut counts = [0u32; 4];
        let trials = 10000;

        for i in 0..trials {
            let roll = i as f32 / trials as f32;
            let item = roll_loot(&table, roll);
            counts[item as usize] += 1;
        }

        // Pebble should be ~90%, green ~8%, blue ~1.9%, key ~0.1%
        assert!(counts[0] > 8500, "Pebble count too low: {}", counts[0]);
        assert!(counts[1] > 500, "Green gem count too low: {}", counts[1]);
        assert!(counts[2] > 100, "Blue gem count too low: {}", counts[2]);
        assert!(counts[3] > 0, "Golden key never dropped: {}", counts[3]);
    }

    #[test]
    fn key_check_works() {
        let no_key = PlayerLoot { has_golden_key: false, total_drops: 0, last_drop: None };
        assert!(!check_has_key(&no_key));

        let has_key = PlayerLoot { has_golden_key: true, total_drops: 1, last_drop: Some(3) };
        assert!(check_has_key(&has_key));
    }

    #[test]
    fn debugger_scenario_boost_key_weight() {
        let mut table = LootTable::default();
        // Set golden key weight to 100.0, others to 0.0
        for entry in &mut table.entries {
            if entry.item_id == 3 {
                entry.weight = 100.0;
            } else {
                entry.weight = 0.0;
            }
        }
        // Now every non-zero roll gives golden key
        assert_eq!(roll_loot(&table, 0.5), 3);
        // At roll=0.0, total=100, roll*total=0.0, first entry (weight 0) gives 0-0=0<=0
        // so it returns item 0. A roll of 0.01 should give golden key:
        assert_eq!(roll_loot(&table, 0.01), 3);
    }

    #[test]
    fn loot_entry_names_readable_in_debugger() {
        let table = LootTable::default();
        let pebble_name = std::str::from_utf8(&table.entries[0]._name)
            .unwrap()
            .trim_end_matches('\0');
        assert_eq!(pebble_name, "Pebble");

        let key_name = std::str::from_utf8(&table.entries[3]._name)
            .unwrap()
            .trim_end_matches('\0');
        assert_eq!(key_name, "Golden Key");
    }

    #[test]
    fn grinding_is_impractical() {
        // With 3-second cooldown and 0.1% chance, expected drops for key: 1000
        // Time: 1000 * 3 = 3000 seconds = 50 minutes
        let expected_drops = 100.0 / 0.1; // 1000 drops
        let time_per_drop = 3.0; // seconds
        let total_minutes = expected_drops * time_per_drop / 60.0;
        assert!(total_minutes > 40.0, "Should take ~50 minutes, got {}", total_minutes);
    }
}
