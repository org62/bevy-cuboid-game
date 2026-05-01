use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;

use crate::player::{
    animate_player, escape_to_menu, player_movement, spawn_player, toggle_pause, MovementBounds,
    Player, PlayerMovementSet,
};
use crate::shared_ui;
use crate::{ChestPhase, GamePaused, Screen, Scoreboard};

pub struct Level6Plugin;

impl Plugin for Level6Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::ChestChallenge), setup_chest)
            .add_systems(
                Update,
                (
                    shared_ui::update_camera_orbit.before(PlayerMovementSet),
                    player_movement.in_set(PlayerMovementSet),
                    chest_playing_update,
                )
                    .chain()
                    .run_if(in_state(ChestPhase::Playing)),
            )
            .add_systems(
                Update,
                (animate_player, chest_visual_update, shared_ui::dismiss_hint, shared_ui::follow_camera_system)
                    .after(PlayerMovementSet)
                    .run_if(in_state(Screen::ChestChallenge)),
            )
            .add_systems(
                Update,
                (escape_to_menu, toggle_pause).run_if(in_state(ChestPhase::Playing)),
            )
            .add_systems(
                Update,
                handle_victory.run_if(in_state(ChestPhase::Victory)),
            )
            .add_systems(OnExit(Screen::ChestChallenge), cleanup_chest);
    }
}

// --- Components ---

#[derive(Component)]
struct ChestEntity;

#[derive(Component)]
struct TreasureChest {
    opened: bool,
}

#[derive(Component)]
struct KeyHudText;

#[derive(Component)]
struct ChestInteractPrompt;

#[derive(Component)]
struct NoKeyWarningText;

// --- Resources ---

#[derive(Resource, Default)]
struct NoKeyWarning {
    timer: f32,
}

#[repr(C)]
pub(crate) struct KeyRing {
    _padding_a: [u32; 4],
    pub(crate) count: i32,
    _padding_b: [u32; 2],
}

#[derive(Resource)]
pub struct Inventory {
    pub(crate) keys: Box<KeyRing>,
}

impl Default for Inventory {
    fn default() -> Self {
        Self {
            keys: Box::new(KeyRing {
                _padding_a: [0xDEADBEEF; 4],
                count: 0,
                _padding_b: [0xCAFEBABE; 2],
            }),
        }
    }
}

#[derive(Resource, Default)]
pub(crate) struct ChestsOpened(pub(crate) u32);

// --- Debugger-target functions ---

#[inline(never)]
fn try_open_chest(inventory: &mut Inventory) -> bool {
    let keys = &mut *inventory.keys;
    if keys.count > 0 {
        keys.count -= 1;
        true
    } else {
        false
    }
}

#[inline(never)]
fn check_all_chests_opened(opened: &ChestsOpened) -> bool {
    opened.0 >= 5
}

// --- Constants ---

const ARENA_MIN: Vec2 = Vec2::new(-7.0, -5.0);
const ARENA_MAX: Vec2 = Vec2::new(7.0, 5.0);
const PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 0.0, 3.0);

// --- Setup ---

fn setup_chest(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    scoreboard: Res<Scoreboard>,
) {
    commands.insert_resource(ClearColor(Color::srgb(0.12, 0.1, 0.08)));
    commands.insert_resource(Inventory::default());
    commands.insert_resource(NoKeyWarning::default());
    commands.insert_resource(ChestsOpened::default());

    // Stone floor
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(14.0, 10.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.28, 0.25),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
        ChestEntity,
    ));

    // Torch lights
    for pos in [
        Vec3::new(-5.0, 2.5, -4.5),
        Vec3::new(5.0, 2.5, -4.5),
        Vec3::new(-5.0, 2.5, 4.5),
        Vec3::new(5.0, 2.5, 4.5),
    ] {
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.7, 0.3),
                intensity: 12000.0,
                range: 12.0,
                ..default()
            },
            Transform::from_translation(pos),
            ChestEntity,
        ));
    }
    shared_ui::setup_level_lighting(
        &mut commands,
        0.0,
        (-0.8, 0.3, 0.0),
        Color::srgb(0.8, 0.65, 0.45),
        250.0,
        ChestEntity,
    );

    // Treasure chests on pedestals
    let chest_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.35, 0.15),
        ..default()
    });
    let pedestal_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.4, 0.38, 0.35),
        ..default()
    });
    let chest_mesh = meshes.add(Cuboid::new(0.7, 0.5, 0.5));
    let pedestal_mesh = meshes.add(Cylinder::new(0.5, 0.6));

    let chest_positions = [
        Vec3::new(-4.0, 0.0, -2.5),
        Vec3::new(-2.0, 0.0, -2.5),
        Vec3::new(0.0, 0.0, -2.5),
        Vec3::new(2.0, 0.0, -2.5),
        Vec3::new(4.0, 0.0, -2.5),
    ];

    for pos in &chest_positions {
        // Pedestal
        commands.spawn((
            Mesh3d(pedestal_mesh.clone()),
            MeshMaterial3d(pedestal_mat.clone()),
            Transform::from_xyz(pos.x, 0.3, pos.z),
            ChestEntity,
        ));
        // Chest
        commands.spawn((
            Mesh3d(chest_mesh.clone()),
            MeshMaterial3d(chest_mat.clone()),
            Transform::from_xyz(pos.x, 0.85, pos.z),
            TreasureChest {
                opened: false,
            },
            ChestEntity,
        ));
    }

    // Scatter some coins on floor (decorative)
    let coin_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.85, 0.2),
        emissive: LinearRgba::new(0.5, 0.4, 0.1, 1.0),
        ..default()
    });
    let coin_mesh = meshes.add(Cylinder::new(0.08, 0.02));
    for i in 0..12 {
        let x = ((i * 7 + 3) % 12) as f32 - 6.0;
        let z = ((i * 11 + 1) % 8) as f32 - 4.0;
        commands.spawn((
            Mesh3d(coin_mesh.clone()),
            MeshMaterial3d(coin_mat.clone()),
            Transform::from_xyz(x, 0.01, z),
            ChestEntity,
        ));
    }

    // Player
    let player = spawn_player(&mut commands, &mut meshes, &mut materials);
    commands.entity(player).insert((
        ChestEntity,
        Transform::from_xyz(PLAYER_SPAWN.x, 0.0, PLAYER_SPAWN.z)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        MovementBounds {
            rects: vec![(ARENA_MIN, ARENA_MAX)],
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
        ChestEntity,
    ));

    // HUD - key counter
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                left: Val::Px(16.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.5, 0.5, 0.55, 0.85)),
            BorderRadius::all(Val::Px(12.0)),
            ChestEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Keys: 0 | Chests: 0/5"),
                TextFont { font_size: 22.0, ..default() },
                TextColor(Color::srgb(1.0, 1.0, 1.0)),
                KeyHudText,
            ));
        });

    // Interact prompt (positioned via visual update, starts hidden)
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            ..default()
        },
        Text::new("[E] Open"),
        TextFont { font_size: 20.0, ..default() },
        TextColor(Color::srgb(1.0, 0.9, 0.4)),
        Visibility::Hidden,
        ChestInteractPrompt,
        ChestEntity,
    ));

    // No-key warning (centered, starts hidden)
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                top: Val::Percent(35.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Visibility::Hidden,
            NoKeyWarningText,
            ChestEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.6, 0.1, 0.05, 0.85)),
                BorderRadius::all(Val::Px(8.0)),
            ))
            .with_children(|bg| {
                bg.spawn((
                    Text::new("You need a key to open this chest!"),
                    TextFont { font_size: 22.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.85, 0.7)),
                ));
            });
        });

    // Controls
    shared_ui::spawn_controls_hint(
        &mut commands,
        "[Esc] Menu | WASD Move | Space Jump | [P] Pause",
        ChestEntity,
    );

    // Hint
    if !scoreboard.is_solved(6) {
        shared_ui::spawn_hint_box(
            &mut commands,
            "No keys to be found... The inventory holds a pointer to your key ring. Set a breakpoint on try_open_chest() and inspect inventory.keys.",
            280.0,
            ChestEntity,
        );
    }
}

// --- Gameplay ---

fn chest_playing_update(
    mut inventory: ResMut<Inventory>,
    mut chests_opened: ResMut<ChestsOpened>,
    mut next_phase: ResMut<NextState<ChestPhase>>,
    player_q: Query<&Transform, (With<Player>, Without<TreasureChest>)>,
    mut chest_q: Query<(&Transform, &mut TreasureChest), Without<Player>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    game_paused: Res<GamePaused>,
    mut no_key_warning: ResMut<NoKeyWarning>,
) {
    if game_paused.0 { return; }
    let Ok(pt) = player_q.get_single() else { return; };
    let pp = pt.translation;

    // Check if player is near a closed chest and presses E or Enter
    if keyboard.just_pressed(KeyCode::KeyE) || keyboard.just_pressed(KeyCode::Enter) {
        for (ct, mut chest) in &mut chest_q {
            if chest.opened { continue; }
            let dx = pp.x - ct.translation.x;
            let dz = pp.z - ct.translation.z;
            if dx * dx + dz * dz < 2.5 {
                if try_open_chest(&mut inventory) {
                    chest.opened = true;
                    chests_opened.0 += 1;
                } else {
                    no_key_warning.timer = 2.0;
                }
                break;
            }
        }
    }

    if check_all_chests_opened(&chests_opened) {
        next_phase.set(ChestPhase::Victory);
    }
}

// --- Visual ---

#[allow(clippy::too_many_arguments)]
fn chest_visual_update(
    time: Res<Time>,
    inventory: Res<Inventory>,
    chests_opened: Res<ChestsOpened>,
    player_q: Query<&Transform, (With<Player>, Without<shared_ui::FollowCamera>, Without<TreasureChest>)>,
    camera_q: Query<
        (&Camera, &GlobalTransform),
        (With<shared_ui::FollowCamera>, Without<Player>, Without<TreasureChest>),
    >,
    chest_q: Query<(&Transform, &TreasureChest), (Without<Player>, Without<shared_ui::FollowCamera>)>,
    mut text_q: Query<&mut Text, (With<KeyHudText>, Without<ChestInteractPrompt>)>,
    mut prompt_q: Query<(&mut Node, &mut Visibility), (With<ChestInteractPrompt>, Without<NoKeyWarningText>)>,
    mut warning_q: Query<&mut Visibility, (With<NoKeyWarningText>, Without<ChestInteractPrompt>)>,
    mut no_key_warning: ResMut<NoKeyWarning>,
) {
    let dt = time.delta_secs();

    // Camera data for world-to-screen projection
    let cam_data = camera_q.get_single().ok().map(|(camera, cam_gt)| {
        (camera.clone(), cam_gt.clone())
    });

    // Interact prompt: find nearest unopened chest within range
    if let Ok(pt) = player_q.get_single() {
        let pp = pt.translation;
        let mut nearest: Option<Vec3> = None;
        let mut nearest_dist = f32::MAX;
        for (ct, chest) in &chest_q {
            if chest.opened { continue; }
            let dx = pp.x - ct.translation.x;
            let dz = pp.z - ct.translation.z;
            let dist_sq = dx * dx + dz * dz;
            if dist_sq < 2.5 && dist_sq < nearest_dist {
                nearest_dist = dist_sq;
                nearest = Some(ct.translation);
            }
        }

        if let Ok((mut node, mut vis)) = prompt_q.get_single_mut() {
            if let (Some(chest_pos), Some((camera, cam_gt))) = (nearest, &cam_data) {
                let world_pos = chest_pos + Vec3::new(0.0, 1.8, 0.0);
                if let Ok(ndc) = camera.world_to_viewport(cam_gt, world_pos) {
                    node.left = Val::Px(ndc.x - 30.0);
                    node.top = Val::Px(ndc.y - 12.0);
                    *vis = Visibility::Visible;
                } else {
                    *vis = Visibility::Hidden;
                }
            } else {
                *vis = Visibility::Hidden;
            }
        }
    }

    // HUD
    if let Ok(mut text) = text_q.get_single_mut() {
        **text = format!(
            "Keys: {} | Chests: {}/5",
            inventory.keys.count, chests_opened.0
        );
    }

    // No-key warning
    if no_key_warning.timer > 0.0 {
        no_key_warning.timer = (no_key_warning.timer - dt).max(0.0);
    }
    if let Ok(mut vis) = warning_q.get_single_mut() {
        *vis = if no_key_warning.timer > 0.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
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
        scoreboard.set_solved(6);
        shared_ui::spawn_victory_overlay(
            &mut commands,
            "ALL CHESTS OPENED!",
            None,
            0.0,
            "Press any key to continue",
            ChestEntity,
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

fn cleanup_chest(mut commands: Commands, query: Query<Entity, With<ChestEntity>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_starts_with_zero_keys() {
        let inv = Inventory::default();
        assert_eq!(inv.keys.count, 0);
    }

    #[test]
    fn try_open_chest_fails_without_keys() {
        let mut inv = Inventory::default();
        assert!(!try_open_chest(&mut inv));
    }

    #[test]
    fn try_open_chest_succeeds_with_keys() {
        let mut inv = Inventory::default();
        inv.keys.count = 5;
        assert!(try_open_chest(&mut inv));
        assert_eq!(inv.keys.count, 4);
    }

    #[test]
    fn open_all_five_chests() {
        let mut inv = Inventory::default();
        inv.keys.count = 5;
        let mut opened = ChestsOpened(0);

        for _ in 0..5 {
            assert!(try_open_chest(&mut inv));
            opened.0 += 1;
        }

        assert!(check_all_chests_opened(&opened));
        assert_eq!(inv.keys.count, 0);
    }

    #[test]
    fn five_chests_needed_for_victory() {
        assert!(!check_all_chests_opened(&ChestsOpened(0)));
        assert!(!check_all_chests_opened(&ChestsOpened(4)));
        assert!(check_all_chests_opened(&ChestsOpened(5)));
        assert!(check_all_chests_opened(&ChestsOpened(10)));
    }

    #[test]
    fn padding_fields_are_decoys() {
        let inv = Inventory::default();
        // Padding fields have sentinel values
        assert_eq!(inv.keys._padding_a, [0xDEADBEEF; 4]);
        assert_eq!(inv.keys._padding_b, [0xCAFEBABE; 2]);
    }

    #[test]
    fn debugger_scenario_set_key_count() {
        // Simulates: player follows Box pointer, sets count = 5
        let mut inv = Inventory::default();
        inv.keys.count = 5; // debugger sets this
        let mut opened = ChestsOpened(0);

        for _ in 0..5 {
            assert!(try_open_chest(&mut inv));
            opened.0 += 1;
        }
        assert!(check_all_chests_opened(&opened));
    }

    #[test]
    fn box_indirection_exists() {
        // Verify the key count is behind a Box (pointer indirection)
        let inv = Inventory::default();
        let ptr = &*inv.keys as *const KeyRing;
        let inv_ptr = &inv as *const Inventory;
        // The KeyRing is heap-allocated, so its address should differ from Inventory's stack address
        assert_ne!(ptr as usize, inv_ptr as usize);
    }
}
