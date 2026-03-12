use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;

use crate::player::{
    animate_player, escape_to_menu, player_movement, spawn_player, toggle_pause, MovementBounds,
    Player, PlayerMovementSet, PlayerPhysics,
};
use crate::{FinalPhase, GamePaused, Screen, Scoreboard};

pub struct Level12Plugin;

impl Plugin for Level12Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::FinalChallenge), setup_final)
            .add_systems(
                Update,
                (player_movement.in_set(PlayerMovementSet), final_playing_update)
                    .chain()
                    .run_if(in_state(FinalPhase::Playing)),
            )
            .add_systems(
                Update,
                (animate_player, final_visual_update)
                    .after(PlayerMovementSet)
                    .run_if(in_state(Screen::FinalChallenge)),
            )
            .add_systems(
                Update,
                (escape_to_menu, toggle_pause).run_if(in_state(FinalPhase::Playing)),
            )
            .add_systems(
                Update,
                handle_victory.run_if(in_state(FinalPhase::Victory)),
            )
            .add_systems(OnExit(Screen::FinalChallenge), cleanup_final);
    }
}

// --- Components ---

#[derive(Component)]
struct FinalEntity;

#[derive(Component)]
struct FinalFollowCam;

#[derive(Component)]
struct RoomHudText;

#[derive(Component)]
struct OverlayScreen;

#[derive(Component)]
pub(crate) struct GuardianNpc {
    pub(crate) health: f32,
    damage_per_tick: f32,
    attack_timer: f32,
}

// --- Resources ---

#[derive(Resource)]
pub(crate) struct FinalState {
    pub(crate) current_room: u32, // 0..3
    rooms_cleared: [bool; 4],
    keypad_input: String,
    player_health: f32,
}

impl Default for FinalState {
    fn default() -> Self {
        Self {
            current_room: 0,
            rooms_cleared: [false; 4],
            keypad_input: String::new(),
            player_health: 100.0,
        }
    }
}

// Room 2: Quicksand
#[repr(C)]
#[derive(Resource)]
pub struct QuicksandState {
    pub sink_rate: f32,
    pub elapsed: f32,
    pub last_double: f32,
}

impl Default for QuicksandState {
    fn default() -> Self {
        Self {
            sink_rate: 0.5,
            elapsed: 0.0,
            last_double: 0.0,
        }
    }
}

// Room 3: Vault
#[repr(C)]
pub(crate) struct VaultLock {
    _pins: [u32; 3],
    pub(crate) locked: bool,
}

#[derive(Resource)]
pub struct VaultDoor {
    pub(crate) lock: Box<VaultLock>,
}

impl Default for VaultDoor {
    fn default() -> Self {
        Self {
            lock: Box::new(VaultLock {
                _pins: [0xDEAD, 0xBEEF, 0xCAFE],
                locked: true,
            }),
        }
    }
}

// Room 4: Weight
#[repr(C)]
#[derive(Resource)]
pub struct PlayerWeight {
    pub base: f32,
    pub equipment: f32,
    pub _penalty: f32,
    pub bonus: f32,
}

impl Default for PlayerWeight {
    fn default() -> Self {
        Self {
            base: 10.0,
            equipment: 5.0,
            _penalty: -1000.0,
            bonus: 0.0,
        }
    }
}

// --- Debugger-target functions ---

// Room 1
#[inline(never)]
fn verify_access_code(input: &str) -> bool {
    let mut code: u32 = 7;
    for i in 0u32..6 {
        code = code.wrapping_mul(13).wrapping_add(i * 3);
    }
    let expected = format!("{:06}", code % 1_000_000);
    input == expected
}

// Room 2
#[inline(never)]
fn apply_quicksand(sink_rate: f32, player_y: f32, dt: f32) -> f32 {
    player_y - sink_rate * dt
}

// Room 3
#[inline(never)]
fn guardian_attack(damage: f32, target_health: &mut f32) {
    *target_health -= damage;
}

// Room 4
#[inline(never)]
fn compute_player_weight(pw: &PlayerWeight) -> f32 {
    pw.base + pw.equipment + pw._penalty + pw.bonus
}

// --- Constants ---

const PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 0.0, 3.0);
// Each room is offset along X
const ROOM_WIDTH: f32 = 14.0;

fn room_offset(room: u32) -> Vec3 {
    Vec3::new(room as f32 * ROOM_WIDTH, 0.0, 0.0)
}

fn room_bounds(room: u32) -> (Vec2, Vec2) {
    let ox = room as f32 * ROOM_WIDTH;
    (Vec2::new(ox - 6.0, -6.0), Vec2::new(ox + 6.0, 6.0))
}

// --- Setup ---

fn setup_final(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(ClearColor(Color::srgb(0.15, 0.12, 0.1)));
    commands.insert_resource(FinalState::default());
    commands.insert_resource(QuicksandState::default());
    commands.insert_resource(VaultDoor::default());
    commands.insert_resource(PlayerWeight::default());

    let floor_mesh = meshes.add(Plane3d::default().mesh().size(12.0, 12.0));
    let wall_h = meshes.add(Cuboid::new(12.0, 4.0, 0.4));
    let wall_v = meshes.add(Cuboid::new(0.4, 4.0, 12.0));

    let room_colors = [
        Color::srgb(0.35, 0.33, 0.3),  // Room 1: stone
        Color::srgb(0.6, 0.5, 0.3),    // Room 2: sand
        Color::srgb(0.3, 0.3, 0.35),   // Room 3: vault
        Color::srgb(0.4, 0.35, 0.25),  // Room 4: grand hall
    ];

    let wall_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.28, 0.25),
        ..default()
    });

    for room in 0..4u32 {
        let off = room_offset(room);

        // Floor
        commands.spawn((
            Mesh3d(floor_mesh.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: room_colors[room as usize],
                ..default()
            })),
            Transform::from_translation(off),
            FinalEntity,
        ));

        // Walls (3 sides, front open for previous room, back has door)
        commands.spawn((
            Mesh3d(wall_h.clone()), MeshMaterial3d(wall_mat.clone()),
            Transform::from_translation(off + Vec3::new(0.0, 2.0, -6.0)),
            FinalEntity,
        ));
        commands.spawn((
            Mesh3d(wall_h.clone()), MeshMaterial3d(wall_mat.clone()),
            Transform::from_translation(off + Vec3::new(0.0, 2.0, 6.0)),
            FinalEntity,
        ));
        commands.spawn((
            Mesh3d(wall_v.clone()), MeshMaterial3d(wall_mat.clone()),
            Transform::from_translation(off + Vec3::new(-6.0, 2.0, 0.0)),
            FinalEntity,
        ));

        // Right wall (with door gap or solid)
        if room < 3 {
            // Door frame - upper and lower parts leaving gap
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.4, 1.5, 4.0))),
                MeshMaterial3d(wall_mat.clone()),
                Transform::from_translation(off + Vec3::new(6.0, 3.25, -3.0)),
                FinalEntity,
            ));
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.4, 1.5, 4.0))),
                MeshMaterial3d(wall_mat.clone()),
                Transform::from_translation(off + Vec3::new(6.0, 3.25, 3.0)),
                FinalEntity,
            ));
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.4, 4.0, 1.5))),
                MeshMaterial3d(wall_mat.clone()),
                Transform::from_translation(off + Vec3::new(6.0, 2.0, -5.25)),
                FinalEntity,
            ));
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.4, 4.0, 1.5))),
                MeshMaterial3d(wall_mat.clone()),
                Transform::from_translation(off + Vec3::new(6.0, 2.0, 5.25)),
                FinalEntity,
            ));
        } else {
            commands.spawn((
                Mesh3d(wall_v.clone()), MeshMaterial3d(wall_mat.clone()),
                Transform::from_translation(off + Vec3::new(6.0, 2.0, 0.0)),
                FinalEntity,
            ));
        }

        // Room-specific lighting
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.8, 0.5),
                intensity: 12000.0,
                range: 12.0,
                ..default()
            },
            Transform::from_translation(off + Vec3::new(0.0, 3.5, 0.0)),
            FinalEntity,
        ));
    }

    // === Room 1: Keypad ===
    let r1 = room_offset(0);
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.5, 0.3))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.4, 0.4, 0.45),
            emissive: LinearRgba::new(0.1, 0.3, 0.1, 1.0),
            ..default()
        })),
        Transform::from_translation(r1 + Vec3::new(3.0, 1.0, -4.0)),
        FinalEntity,
    ));

    // === Room 2: Sandy quicksand floor marker ===
    let r2 = room_offset(1);
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(10.0, 10.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.7, 0.6, 0.3, 0.4),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })),
        Transform::from_translation(r2 + Vec3::new(0.0, 0.02, 0.0)),
        FinalEntity,
    ));
    // Elevated exit platform
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(3.0, 1.0, 3.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.4, 0.38, 0.35),
            ..default()
        })),
        Transform::from_translation(r2 + Vec3::new(4.0, 0.5, 0.0)),
        FinalEntity,
    ));

    // === Room 3: Guardian NPCs ===
    let r3 = room_offset(2);
    let guardian_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.3, 0.3),
        ..default()
    });
    let guardian_mesh = meshes.add(Cuboid::new(0.8, 1.5, 0.8));
    for (_i, pos) in [Vec3::new(-2.0, 0.75, -2.0), Vec3::new(2.0, 0.75, 2.0)].iter().enumerate() {
        commands.spawn((
            Mesh3d(guardian_mesh.clone()),
            MeshMaterial3d(guardian_mat.clone()),
            Transform::from_translation(r3 + *pos),
            GuardianNpc {
                health: 100.0,
                damage_per_tick: 15.0,
                attack_timer: 0.0,
            },
            FinalEntity,
        ));
    }
    // Vault door
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.3, 2.5, 2.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.35, 0.35, 0.4),
            ..default()
        })),
        Transform::from_translation(r3 + Vec3::new(5.0, 1.25, 0.0)),
        FinalEntity,
    ));

    // === Room 4: Giant scale ===
    let r4 = room_offset(3);
    // Scale base
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.3, 2.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.7, 0.6, 0.2),
            ..default()
        })),
        Transform::from_translation(r4 + Vec3::new(0.0, 1.0, 0.0)),
        FinalEntity,
    ));
    // Scale beam
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(5.0, 0.15, 0.3))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.7, 0.6, 0.2),
            ..default()
        })),
        Transform::from_translation(r4 + Vec3::new(0.0, 2.1, 0.0)),
        FinalEntity,
    ));
    // Scale pans
    let pan_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.6, 0.55, 0.2),
        ..default()
    });
    let pan_mesh = meshes.add(Cylinder::new(0.8, 0.05));
    commands.spawn((
        Mesh3d(pan_mesh.clone()), MeshMaterial3d(pan_mat.clone()),
        Transform::from_translation(r4 + Vec3::new(-2.0, 1.5, 0.0)),
        FinalEntity,
    ));
    commands.spawn((
        Mesh3d(pan_mesh), MeshMaterial3d(pan_mat),
        Transform::from_translation(r4 + Vec3::new(2.0, 1.5, 0.0)),
        FinalEntity,
    ));
    // Weights on counter-side
    let weight_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.3, 0.32),
        ..default()
    });
    let weight_mesh = meshes.add(Cuboid::new(0.3, 0.3, 0.3));
    for i in 0..3 {
        commands.spawn((
            Mesh3d(weight_mesh.clone()),
            MeshMaterial3d(weight_mat.clone()),
            Transform::from_translation(r4 + Vec3::new(1.8 + i as f32 * 0.35, 1.7, 0.0)),
            FinalEntity,
        ));
    }

    // Player
    let (bmin, bmax) = room_bounds(0);
    let player = spawn_player(&mut commands, &mut meshes, &mut materials);
    commands.entity(player).insert((
        FinalEntity,
        Transform::from_xyz(PLAYER_SPAWN.x, 0.0, PLAYER_SPAWN.z)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        MovementBounds {
            rects: vec![(bmin, bmax)],
        },
    ));

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 10.0, 12.0).looking_at(Vec3::ZERO, Vec3::Y),
        FinalFollowCam,
        FinalEntity,
    ));

    // Ambient
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.65, 0.55, 0.45),
        brightness: 300.0,
    });

    // HUD - room indicator
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                left: Val::Px(16.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.15, 0.1, 0.2, 0.85)),
            BorderRadius::all(Val::Px(12.0)),
            FinalEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Room 1/4: The Sealed Door"),
                TextFont { font_size: 22.0, ..default() },
                TextColor(Color::srgb(1.0, 1.0, 1.0)),
                RoomHudText,
            ));
        });

    // No hint for Level 12!
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(16.0),
                left: Val::Px(16.0),
                ..default()
            },
            FinalEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("[Esc] Menu | WASD Move | Space Jump | [P] Pause | You're on your own now."),
                TextFont { font_size: 18.0, ..default() },
                TextColor(Color::srgb(0.7, 0.7, 0.7)),
            ));
        });
}

// --- Gameplay ---

#[allow(clippy::too_many_arguments)]
fn final_playing_update(
    time: Res<Time>,
    mut final_state: ResMut<FinalState>,
    mut quicksand: ResMut<QuicksandState>,
    vault_door: Res<VaultDoor>,
    player_weight: Res<PlayerWeight>,
    mut next_phase: ResMut<NextState<FinalPhase>>,
    mut events: EventReader<KeyboardInput>,
    mut player_q: Query<(&mut Transform, &mut PlayerPhysics, &mut MovementBounds), With<Player>>,
    mut guardians: Query<&mut GuardianNpc>,
    game_paused: Res<GamePaused>,
) {
    if game_paused.0 { return; }
    let dt = time.delta_secs();
    let room = final_state.current_room;

    match room {
        // Room 1: Keypad
        0 => {
            for event in events.read() {
                if !event.state.is_pressed() { continue; }
                match &event.logical_key {
                    Key::Character(c) => {
                        for ch in c.as_str().chars() {
                            if ch.is_ascii_digit() && final_state.keypad_input.len() < 6 {
                                final_state.keypad_input.push(ch);
                            }
                        }
                    }
                    Key::Enter => {
                        if verify_access_code(&final_state.keypad_input) {
                            final_state.rooms_cleared[0] = true;
                            advance_room(&mut final_state, &mut player_q);
                        } else {
                            final_state.keypad_input.clear();
                        }
                    }
                    Key::Backspace => {
                        final_state.keypad_input.pop();
                    }
                    _ => {}
                }
            }
        }

        // Room 2: Quicksand
        1 => {
            quicksand.elapsed += dt;
            if quicksand.elapsed - quicksand.last_double >= 5.0 {
                quicksand.sink_rate *= 2.0;
                quicksand.last_double = quicksand.elapsed;
            }

            let mut should_advance = false;
            let mut should_reset = false;

            if let Ok((mut pt, mut pp, _)) = player_q.get_single_mut() {
                let new_y = apply_quicksand(quicksand.sink_rate, pt.translation.y, dt);
                pt.translation.y = new_y;

                // On the elevated exit platform?
                let off = room_offset(1);
                let on_platform =
                    pt.translation.x > off.x + 2.5 && pt.translation.x < off.x + 5.5
                    && pt.translation.z > off.z - 1.5 && pt.translation.z < off.z + 1.5
                    && pt.translation.y >= 0.8;

                if on_platform {
                    final_state.rooms_cleared[1] = true;
                    should_advance = true;
                }

                if pt.translation.y < -3.0 {
                    pt.translation.y = 0.0;
                    pp.velocity = Vec3::ZERO;
                    should_reset = true;
                }
            }

            if should_advance {
                advance_room(&mut final_state, &mut player_q);
            }
            if should_reset {
                quicksand.sink_rate = 0.5;
                quicksand.elapsed = 0.0;
                quicksand.last_double = 0.0;
            }
        }

        // Room 3: Guarded Vault
        2 => {
            // Guardians attack player
            let mut total_damage = 0.0f32;
            for mut g in &mut guardians {
                if g.health <= 0.0 { continue; }
                g.attack_timer += dt;
                if g.attack_timer >= 1.0 {
                    g.attack_timer -= 1.0;
                    total_damage += g.damage_per_tick;
                }
            }
            if total_damage > 0.0 {
                guardian_attack(total_damage, &mut final_state.player_health);
            }

            if final_state.player_health <= 0.0 {
                // Reset room
                final_state.player_health = 100.0;
                for mut g in &mut guardians {
                    g.health = 100.0;
                    g.attack_timer = 0.0;
                }
                if let Ok((mut pt, mut pp, _)) = player_q.get_single_mut() {
                    let off = room_offset(2);
                    pt.translation = off + Vec3::new(-3.0, 0.0, 0.0);
                    pp.velocity = Vec3::ZERO;
                }
            }

            // Check if vault is unlocked AND guardians defeated
            let guardians_dead = guardians.iter().all(|g| g.health <= 0.0);
            let vault_unlocked = !vault_door.lock.locked;
            if guardians_dead && vault_unlocked {
                final_state.rooms_cleared[2] = true;
                advance_room(&mut final_state, &mut player_q);
            }
        }

        // Room 4: Weighted Scale
        3 => {
            let weight = compute_player_weight(&player_weight);
            // Need positive weight (counter-side is 20.0)
            if weight > 20.0 {
                final_state.rooms_cleared[3] = true;
                next_phase.set(FinalPhase::Victory);
            }
        }

        _ => {}
    }
}

fn advance_room(
    final_state: &mut ResMut<FinalState>,
    player_q: &mut Query<(&mut Transform, &mut PlayerPhysics, &mut MovementBounds), With<Player>>,
) {
    let next_room = final_state.current_room + 1;
    if next_room >= 4 {
        final_state.current_room = 3;
        return;
    }
    final_state.current_room = next_room;

    if let Ok((mut pt, mut pp, mut bounds)) = player_q.get_single_mut() {
        let off = room_offset(next_room);
        pt.translation = off + Vec3::new(-3.0, 0.0, 0.0);
        pp.velocity = Vec3::ZERO;
        pp.grounded = true;

        let (bmin, bmax) = room_bounds(next_room);
        bounds.rects = vec![(bmin, bmax)];
    }
}

// --- Visual ---

#[allow(clippy::too_many_arguments)]
fn final_visual_update(
    time: Res<Time>,
    final_state: Res<FinalState>,
    player_q: Query<&Transform, (With<Player>, Without<FinalFollowCam>)>,
    mut camera_q: Query<&mut Transform, (With<FinalFollowCam>, Without<Player>)>,
    mut text_q: Query<&mut Text, With<RoomHudText>>,
) {
    let dt = time.delta_secs();

    // Camera
    if let (Ok(pt), Ok(mut ct)) = (player_q.get_single(), camera_q.get_single_mut()) {
        let target = pt.translation + Vec3::new(0.0, 10.0, 12.0);
        let t = (6.0 * dt).min(1.0);
        ct.translation = ct.translation.lerp(target, t);
        ct.look_at(pt.translation + Vec3::Y, Vec3::Y);
    }

    // HUD
    if let Ok(mut text) = text_q.get_single_mut() {
        let room_name = match final_state.current_room {
            0 => {
                let code_display = if final_state.keypad_input.is_empty() {
                    "______".to_string()
                } else {
                    format!("{:_<6}", final_state.keypad_input)
                };
                format!("Room 1/4: The Sealed Door [{}]", code_display)
            }
            1 => "Room 2/4: The Quicksand Floor".to_string(),
            2 => format!("Room 3/4: The Guarded Vault (HP: {:.0})", final_state.player_health),
            3 => "Room 4/4: The Weighted Scale".to_string(),
            _ => "???".to_string(),
        };
        **text = room_name;
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
        scoreboard.final_solved = true;
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
                BackgroundColor(Color::srgba(0.05, 0.0, 0.15, 0.85)),
                GlobalZIndex(10),
                OverlayScreen,
                FinalEntity,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new("THE FINAL EXAM - COMPLETE!"),
                    TextFont { font_size: 48.0, ..default() },
                    TextColor(Color::srgb(1.0, 0.85, 0.2)),
                ));
                parent.spawn((
                    Text::new("You've mastered the debugger!"),
                    TextFont { font_size: 28.0, ..default() },
                    TextColor(Color::srgb(0.8, 0.9, 1.0)),
                ));
                parent.spawn((
                    Text::new("Press any key to continue"),
                    TextFont { font_size: 22.0, ..default() },
                    TextColor(Color::srgb(0.6, 0.7, 0.8)),
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

// --- Cleanup ---

fn cleanup_final(mut commands: Commands, query: Query<Entity, With<FinalEntity>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Room 1: Access Code ---

    #[test]
    fn access_code_is_deterministic() {
        // The code is computed, not random
        let code1 = {
            let mut code: u32 = 7;
            for i in 0u32..6 {
                code = code.wrapping_mul(13).wrapping_add(i * 3);
            }
            format!("{:06}", code % 1_000_000)
        };
        // Should match the verify function
        assert!(verify_access_code(&code1));
    }

    #[test]
    fn wrong_access_code_rejected() {
        assert!(!verify_access_code("000000"));
        assert!(!verify_access_code("123456"));
        assert!(!verify_access_code(""));
    }

    #[test]
    fn access_code_is_6_digits() {
        let mut code: u32 = 7;
        for i in 0u32..6 {
            code = code.wrapping_mul(13).wrapping_add(i * 3);
        }
        let code_str = format!("{:06}", code % 1_000_000);
        assert_eq!(code_str.len(), 6);
        assert!(code_str.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn debugger_scenario_inspect_access_code() {
        // Simulates: player breaks on verify_access_code, reads `expected`
        let mut code: u32 = 7;
        for i in 0u32..6 {
            code = code.wrapping_mul(13).wrapping_add(i * 3);
        }
        let expected = format!("{:06}", code % 1_000_000);
        // Player types this code
        assert!(verify_access_code(&expected));
    }

    // --- Room 2: Quicksand ---

    #[test]
    fn quicksand_sinks_player() {
        let new_y = apply_quicksand(0.5, 1.0, 1.0);
        assert!((new_y - 0.5).abs() < 0.001);
    }

    #[test]
    fn quicksand_rate_increases() {
        let mut qs = QuicksandState::default();
        assert!((qs.sink_rate - 0.5).abs() < 0.001);

        // After 5 seconds, rate doubles
        qs.elapsed = 5.0;
        if qs.elapsed - qs.last_double >= 5.0 {
            qs.sink_rate *= 2.0;
            qs.last_double = qs.elapsed;
        }
        assert!((qs.sink_rate - 1.0).abs() < 0.001);
    }

    #[test]
    fn quicksand_eventually_unstoppable() {
        let mut qs = QuicksandState::default();
        // Simulate 30 seconds
        for _ in 0..6 {
            qs.elapsed += 5.0;
            qs.sink_rate *= 2.0;
        }
        // After 30 seconds, rate is 0.5 * 2^6 = 32.0
        assert!((qs.sink_rate - 32.0).abs() < 0.1);
        // Sinks player 32 units per second - impossible to escape
    }

    #[test]
    fn debugger_scenario_zero_sink_rate() {
        let new_y = apply_quicksand(0.0, 1.0, 1.0);
        assert!((new_y - 1.0).abs() < 0.001); // no sinking
    }

    // --- Room 3: Guardians ---

    #[test]
    fn guardian_attack_reduces_health() {
        let mut health = 100.0f32;
        guardian_attack(15.0, &mut health);
        assert!((health - 85.0).abs() < 0.001);
    }

    #[test]
    fn guardian_attack_can_kill() {
        let mut health = 10.0f32;
        guardian_attack(20.0, &mut health);
        assert!(health < 0.0);
    }

    #[test]
    fn vault_lock_behind_box() {
        let vault = VaultDoor::default();
        assert!(vault.lock.locked);
        // Padding exists
        assert_eq!(vault.lock._pins, [0xDEAD, 0xBEEF, 0xCAFE]);
    }

    #[test]
    fn debugger_scenario_unlock_vault() {
        let mut vault = VaultDoor::default();
        vault.lock.locked = false; // debugger sets this
        assert!(!vault.lock.locked);
    }

    // --- Room 4: Weighted Scale ---

    #[test]
    fn player_weight_is_negative() {
        let pw = PlayerWeight::default();
        let weight = compute_player_weight(&pw);
        // 10.0 + 5.0 + (-1000.0) + 0.0 = -985.0
        assert!((weight - (-985.0)).abs() < 0.001);
        assert!(weight < 0.0);
    }

    #[test]
    fn player_weight_cannot_outweigh_20() {
        let pw = PlayerWeight::default();
        let weight = compute_player_weight(&pw);
        assert!(weight < 20.0, "Weight is {}, should be way below 20", weight);
    }

    #[test]
    fn penalty_field_is_sabotage() {
        let pw = PlayerWeight::default();
        assert!((pw._penalty - (-1000.0)).abs() < 0.001);
        // Without penalty: 10 + 5 + 0 = 15, still < 20
        // Even fixing penalty to 0.0 isn't enough!
        // Player needs to also set bonus
    }

    #[test]
    fn debugger_scenario_fix_weight() {
        let mut pw = PlayerWeight::default();
        pw._penalty = 0.0; // debugger fixes this
        pw.bonus = 10.0;   // debugger adds bonus
        let weight = compute_player_weight(&pw);
        // 10 + 5 + 0 + 10 = 25 > 20
        assert!(weight > 20.0);
    }

    #[test]
    fn debugger_scenario_fix_penalty_only() {
        let mut pw = PlayerWeight::default();
        pw._penalty = 0.0; // debugger fixes this
        let weight = compute_player_weight(&pw);
        // 10 + 5 + 0 + 0 = 15, still < 20
        assert!(weight < 20.0, "Just fixing penalty isn't enough, weight = {}", weight);
    }

    #[test]
    fn debugger_scenario_set_penalty_positive() {
        let mut pw = PlayerWeight::default();
        pw._penalty = 100.0; // debugger sets this to positive
        let weight = compute_player_weight(&pw);
        // 10 + 5 + 100 + 0 = 115 > 20
        assert!(weight > 20.0);
    }

    // --- Cross-room integration ---

    #[test]
    fn all_rooms_have_different_puzzles() {
        // Room 1: code verification (string)
        assert!(!verify_access_code("wrong"));

        // Room 2: quicksand (float manipulation)
        let y = apply_quicksand(0.5, 1.0, 1.0);
        assert!(y < 1.0);

        // Room 3: guardian damage (pointer + combat)
        let mut hp = 100.0f32;
        guardian_attack(15.0, &mut hp);
        assert!(hp < 100.0);

        // Room 4: weight calculation (hidden field)
        let pw = PlayerWeight::default();
        assert!(compute_player_weight(&pw) < 0.0);
    }

    #[test]
    fn room_offsets_are_sequential() {
        for i in 0..4u32 {
            let off = room_offset(i);
            assert!((off.x - i as f32 * ROOM_WIDTH).abs() < 0.001);
            assert_eq!(off.y, 0.0);
            assert_eq!(off.z, 0.0);
        }
    }

    #[test]
    fn room_bounds_dont_overlap() {
        for i in 0..4u32 {
            for j in (i + 1)..4u32 {
                let (min_i, max_i) = room_bounds(i);
                let (min_j, max_j) = room_bounds(j);
                // X ranges should not overlap
                assert!(max_i.x <= min_j.x || max_j.x <= min_i.x,
                    "Rooms {} and {} X ranges overlap", i, j);
            }
        }
    }
}
