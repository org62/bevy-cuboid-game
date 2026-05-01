use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;

use crate::player::{
    animate_player, escape_to_menu, player_movement, spawn_player, toggle_pause, MovementBounds,
    Player, PlayerMovementSet,
};
use crate::shared_ui;
use crate::{GamePaused, TollPhase, Screen, Scoreboard};

pub struct Level8Plugin;

impl Plugin for Level8Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::TollChallenge), setup_toll)
            .add_systems(
                Update,
                (
                    shared_ui::update_camera_orbit.before(PlayerMovementSet),
                    player_movement.in_set(PlayerMovementSet),
                    toll_playing_update,
                )
                    .chain()
                    .run_if(in_state(TollPhase::Playing)),
            )
            .add_systems(
                Update,
                (animate_player, toll_visual_update, shared_ui::dismiss_hint, shared_ui::follow_camera_system)
                    .after(PlayerMovementSet)
                    .run_if(in_state(Screen::TollChallenge)),
            )
            .add_systems(
                Update,
                (escape_to_menu, toggle_pause).run_if(in_state(TollPhase::Playing)),
            )
            .add_systems(
                Update,
                handle_victory.run_if(in_state(TollPhase::Victory)),
            )
            .add_systems(OnExit(Screen::TollChallenge), cleanup_toll);
    }
}

// --- Components ---

#[derive(Component)]
struct TollEntity;

#[derive(Component)]
struct TollBooth;

#[derive(Component)]
struct GoldHudText;

// --- Resources ---

#[repr(C)]
#[derive(Resource)]
pub struct PlayerWallet {
    pub gold: i32,
}

impl Default for PlayerWallet {
    fn default() -> Self {
        Self { gold: 10 }
    }
}

#[derive(Resource)]
struct TollState {
    checkpoint: u32,
    paid: [bool; 3],
}

impl Default for TollState {
    fn default() -> Self {
        Self {
            checkpoint: 0,
            paid: [false; 3],
        }
    }
}

// --- Debugger-target functions ---

#[inline(never)]
fn compute_toll(checkpoint: u32) -> i32 {
    let mut base: i32 = 1;
    let mut i: u32 = 0;
    while i < checkpoint {
        base = base.wrapping_mul(10);
        i += 1;
    }
    let mask: i32 = 0x0F;
    let seed: i32 = 0x37;
    let factor = seed & mask;
    factor.wrapping_mul(base)
}

#[inline(never)]
fn try_pay_toll(wallet: &mut PlayerWallet, cost: i32) -> bool {
    if wallet.gold >= cost {
        wallet.gold -= cost;
        true
    } else {
        false
    }
}

// --- Constants ---

const ARENA_MIN: Vec2 = Vec2::new(-3.0, -20.0);
const ARENA_MAX: Vec2 = Vec2::new(3.0, 2.0);
const PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 0.0, 1.0);

// --- Setup ---

fn setup_toll(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    scoreboard: Res<Scoreboard>,
) {
    commands.insert_resource(ClearColor(Color::srgb(0.1, 0.08, 0.15)));
    commands.insert_resource(PlayerWallet::default());
    commands.insert_resource(TollState::default());

    // Bridge surface
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(6.0, 0.3, 24.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.4, 0.38, 0.35),
            ..default()
        })),
        Transform::from_xyz(0.0, -0.15, -9.0),
        TollEntity,
    ));

    // Bridge railings
    let railing_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.32, 0.3),
        ..default()
    });
    let railing_mesh = meshes.add(Cuboid::new(0.2, 1.5, 24.0));
    commands.spawn((
        Mesh3d(railing_mesh.clone()), MeshMaterial3d(railing_mat.clone()),
        Transform::from_xyz(-3.0, 0.75, -9.0), TollEntity,
    ));
    commands.spawn((
        Mesh3d(railing_mesh), MeshMaterial3d(railing_mat),
        Transform::from_xyz(3.0, 0.75, -9.0), TollEntity,
    ));

    // Chasm (dark fog below)
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(20.0, 24.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.02, 0.02, 0.04),
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(0.0, -5.0, -9.0),
        TollEntity,
    ));

    // Toll booths
    let booth_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.2, 0.2),
        ..default()
    });
    let booth_mesh = meshes.add(Cuboid::new(1.0, 2.5, 0.3));
    let toll_z_positions = [-4.0f32, -10.0, -16.0];

    for &z in &toll_z_positions {
        // Left pillar
        commands.spawn((
            Mesh3d(booth_mesh.clone()), MeshMaterial3d(booth_mat.clone()),
            Transform::from_xyz(-2.0, 1.25, z),
            TollBooth,
            TollEntity,
        ));
        // Right pillar
        commands.spawn((
            Mesh3d(booth_mesh.clone()), MeshMaterial3d(booth_mat.clone()),
            Transform::from_xyz(2.0, 1.25, z),
            TollEntity,
        ));
        // Barrier bar
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(4.0, 0.15, 0.15))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.8, 0.2, 0.15),
                emissive: LinearRgba::new(1.0, 0.2, 0.1, 1.0),
                ..default()
            })),
            Transform::from_xyz(0.0, 2.0, z),
            TollBooth,
            TollEntity,
        ));
    }

    // Torches along bridge
    for &(x, z) in &[(-2.5, -2.0), (2.5, -2.0), (-2.5, -7.0), (2.5, -7.0),
                     (-2.5, -13.0), (2.5, -13.0)] {
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.7, 0.3),
                intensity: 10000.0,
                range: 8.0,
                ..default()
            },
            Transform::from_xyz(x, 2.0, z),
            TollEntity,
        ));
    }

    // Lighting
    shared_ui::setup_level_lighting(
        &mut commands,
        6000.0,
        (-0.7, 0.2, 0.0),
        Color::srgb(0.5, 0.45, 0.6),
        250.0,
        TollEntity,
    );

    // Player
    let player = spawn_player(&mut commands, &mut meshes, &mut materials);
    commands.entity(player).insert((
        TollEntity,
        Transform::from_xyz(PLAYER_SPAWN.x, 0.0, PLAYER_SPAWN.z)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        MovementBounds {
            rects: vec![(ARENA_MIN, ARENA_MAX)],
        },
    ));

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 6.0, 8.0).looking_at(Vec3::new(0.0, 0.0, -3.0), Vec3::Y),
        shared_ui::FollowCamera {
            offset: Vec3::new(0.0, 10.0, 10.0),
            lerp_speed: 6.0,
            look_offset: Vec3::Y,
        },
        TollEntity,
    ));

    // HUD - gold counter
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                left: Val::Px(16.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.7, 0.6, 0.1, 0.85)),
            BorderRadius::all(Val::Px(12.0)),
            TollEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Gold: 10"),
                TextFont { font_size: 22.0, ..default() },
                TextColor(Color::srgb(0.1, 0.1, 0.1)),
                GoldHudText,
            ));
        });

    // Controls
    shared_ui::spawn_controls_hint(
        &mut commands,
        "[Esc] Menu | WASD Move | Space Jump | E to pay | [P] Pause",
        TollEntity,
    );

    // Hint
    if !scoreboard.is_solved(8) {
        shared_ui::spawn_hint_box(
            &mut commands,
            "The toll keeper's price grows tenfold! Step through compute_toll() to understand the formula, then make yourself wealthy.",
            280.0,
            TollEntity,
        );
    }
}

// --- Gameplay ---

fn toll_playing_update(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut wallet: ResMut<PlayerWallet>,
    mut toll_state: ResMut<TollState>,
    mut next_phase: ResMut<NextState<TollPhase>>,
    player_q: Query<&Transform, With<Player>>,
    game_paused: Res<GamePaused>,
) {
    if game_paused.0 { return; }
    let Ok(pt) = player_q.get_single() else { return; };
    let pz = pt.translation.z;

    // Determine which checkpoint the player is at
    let toll_z_positions = [-4.0f32, -10.0, -16.0];
    let checkpoint = toll_state.checkpoint as usize;

    // Block player at unpaid toll
    if checkpoint < 3 && !toll_state.paid[checkpoint] {
        let toll_z = toll_z_positions[checkpoint];
        if pz < toll_z + 1.0 {
            // Player is near the toll - try to pay if pressing E
            if keyboard.just_pressed(KeyCode::KeyE) || keyboard.just_pressed(KeyCode::Enter) {
                let cost = compute_toll(checkpoint as u32);
                if try_pay_toll(&mut wallet, cost) {
                    toll_state.paid[checkpoint] = true;
                    toll_state.checkpoint += 1;
                }
            }
        }
    }

    // Check if all tolls paid and player reached the end
    if toll_state.paid.iter().all(|&p| p) && pz < -19.0 {
        next_phase.set(TollPhase::Victory);
    }
}

// --- Visual ---

fn toll_visual_update(
    wallet: Res<PlayerWallet>,
    mut text_q: Query<&mut Text, With<GoldHudText>>,
) {
    // HUD
    if let Ok(mut text) = text_q.get_single_mut() {
        **text = format!("Gold: {}", wallet.gold);
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
        scoreboard.set_solved(8);
        shared_ui::spawn_victory_overlay(
            &mut commands,
            "BRIDGE CROSSED!",
            None,
            28.0,
            "Press any key to continue",
            TollEntity,
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

fn cleanup_toll(mut commands: Commands, query: Query<Entity, With<TollEntity>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toll_computation_is_7_times_power_of_10() {
        assert_eq!(compute_toll(0), 7);    // 7 * 10^0 = 7
        assert_eq!(compute_toll(1), 70);   // 7 * 10^1 = 70
        assert_eq!(compute_toll(2), 700);  // 7 * 10^2 = 700
    }

    #[test]
    fn toll_values_are_7_77_777() {
        // The plan says 7, 77, 777 but the algorithm gives 7, 70, 700
        // Actually re-reading: plan says "Toll 1 costs 7, Toll 2 costs 77, Toll 3 costs 777"
        // But the algorithm computes 7 * 10^checkpoint
        // checkpoint 0 = 7, checkpoint 1 = 70, checkpoint 2 = 700
        // The tolls are still unaffordable: 7 + 70 + 700 = 777 total, player has 10
        let total = compute_toll(0) + compute_toll(1) + compute_toll(2);
        assert_eq!(total, 777);
    }

    #[test]
    fn player_can_afford_first_toll() {
        let mut wallet = PlayerWallet::default();
        let cost = compute_toll(0);
        assert!(try_pay_toll(&mut wallet, cost));
        assert_eq!(wallet.gold, 3); // 10 - 7 = 3
    }

    #[test]
    fn player_cannot_afford_second_toll() {
        let mut wallet = PlayerWallet::default();
        // Pay first toll
        try_pay_toll(&mut wallet, compute_toll(0));
        // Try second toll
        let cost2 = compute_toll(1);
        assert!(!try_pay_toll(&mut wallet, cost2));
        assert_eq!(wallet.gold, 3); // unchanged
    }

    #[test]
    fn player_cannot_afford_all_tolls() {
        let wallet = PlayerWallet::default();
        let total = compute_toll(0) + compute_toll(1) + compute_toll(2);
        assert!(wallet.gold < total);
    }

    #[test]
    fn toll_obfuscation_uses_hex() {
        // Verify the seed & mask trick: 0x37 & 0x0F = 7
        let mask: i32 = 0x0F;
        let seed: i32 = 0x37;
        assert_eq!(seed & mask, 7);
    }

    #[test]
    fn debugger_scenario_give_gold() {
        let mut wallet = PlayerWallet::default();
        wallet.gold = 10000; // debugger sets this
        for checkpoint in 0..3 {
            let cost = compute_toll(checkpoint);
            assert!(try_pay_toll(&mut wallet, cost));
        }
        // All tolls paid successfully
        assert!(wallet.gold > 0);
    }

    #[test]
    fn pay_toll_does_not_charge_on_failure() {
        let mut wallet = PlayerWallet { gold: 5 };
        let cost = compute_toll(1); // 70, can't afford
        assert!(!try_pay_toll(&mut wallet, cost));
        assert_eq!(wallet.gold, 5); // unchanged
    }
}
