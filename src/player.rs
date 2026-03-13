use bevy::prelude::*;

use crate::GamePaused;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlayerMovementSet;

#[derive(Component)]
pub struct PauseOverlay;

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct PlayerPhysics {
    pub velocity: Vec3,
    pub grounded: bool,
    pub facing: Quat,
}

impl Default for PlayerPhysics {
    fn default() -> Self {
        Self {
            velocity: Vec3::ZERO,
            grounded: true,
            facing: Quat::from_rotation_y(std::f32::consts::PI),
        }
    }
}

#[derive(Component)]
pub struct SquashState {
    pub timer: f32,
}

#[derive(Component)]
pub struct PlayerBody;

#[derive(Component)]
pub struct PlayerHead;

pub const PLAYER_PUSHBACK: Vec3 = Vec3::new(4.0, 0.0, 0.0);

const ACCELERATION: f32 = 30.0;
const MAX_SPEED: f32 = 7.0;
const FRICTION: f32 = 15.0;
pub const GRAVITY: f32 = -25.0;
const JUMP_VELOCITY: f32 = 9.0;
const GROUND_Y: f32 = 0.0;

/// Optional resource that overrides the default gravity constant.
/// Insert this resource to change gravity for a specific level.
#[derive(Resource)]
pub struct GravityOverride(pub f32);

/// Optional resource that overrides the default ground Y level.
/// Insert this resource to allow the player to go below y=0.
#[derive(Resource)]
pub struct GroundYOverride(pub f32);

#[derive(Resource, Default)]
pub struct PowerUpState {
    pub speed_multiplier: f32,
    pub jump_multiplier: f32,
    pub reverse_facing: bool,
}

#[derive(Component)]
pub struct MovementBounds {
    pub rects: Vec<(Vec2, Vec2)>,
}

impl MovementBounds {
    pub fn clamp(&self, x: f32, z: f32) -> (f32, f32) {
        let pos = Vec2::new(x, z);
        let mut best = pos;
        let mut best_dist = f32::MAX;
        for &(min, max) in &self.rects {
            let clamped = Vec2::new(pos.x.clamp(min.x, max.x), pos.y.clamp(min.y, max.y));
            let d = pos.distance_squared(clamped);
            if d < best_dist {
                best_dist = d;
                best = clamped;
            }
        }
        (best.x, best.y)
    }
}

pub fn spawn_player(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) -> Entity {
    let coral = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.45, 0.35),
        ..default()
    });
    let white = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        ..default()
    });
    let black = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        ..default()
    });

    commands
        .spawn((
            Transform::from_xyz(0.0, GROUND_Y, 0.0)
                .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
            Visibility::default(),
            Player,
            PlayerPhysics::default(),
            SquashState { timer: 0.0 },
        ))
        .with_children(|parent| {
            // Body
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.6, 0.8, 0.5))),
                MeshMaterial3d(coral.clone()),
                Transform::from_xyz(0.0, 0.8, 0.0),
                PlayerBody,
            ));
            // Head with eyes
            parent
                .spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.65, 0.6, 0.55))),
                    MeshMaterial3d(coral),
                    Transform::from_xyz(0.0, 1.5, 0.0),
                    PlayerHead,
                ))
                .with_children(|head| {
                    // Left eye
                    head.spawn((
                        Mesh3d(meshes.add(Sphere::new(0.09))),
                        MeshMaterial3d(white.clone()),
                        Transform::from_xyz(-0.15, 0.05, -0.28),
                    ))
                    .with_children(|eye| {
                        eye.spawn((
                            Mesh3d(meshes.add(Sphere::new(0.045))),
                            MeshMaterial3d(black.clone()),
                            Transform::from_xyz(0.0, 0.0, -0.05),
                        ));
                    });
                    // Right eye
                    head.spawn((
                        Mesh3d(meshes.add(Sphere::new(0.09))),
                        MeshMaterial3d(white),
                        Transform::from_xyz(0.15, 0.05, -0.28),
                    ))
                    .with_children(|eye| {
                        eye.spawn((
                            Mesh3d(meshes.add(Sphere::new(0.045))),
                            MeshMaterial3d(black),
                            Transform::from_xyz(0.0, 0.0, -0.05),
                        ));
                    });
                });
        })
        .id()
}

pub fn player_movement(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    time: Res<Time>,
    gravity_override: Option<Res<GravityOverride>>,
    ground_y_override: Option<Res<GroundYOverride>>,
    power_ups: Option<Res<PowerUpState>>,
    game_paused: Res<GamePaused>,
    mut query: Query<(&mut Transform, &mut PlayerPhysics, &mut SquashState, &MovementBounds), With<Player>>,
) {
    if game_paused.0 {
        return;
    }
    let Ok((mut transform, mut physics, mut squash, bounds)) = query.get_single_mut() else {
        return;
    };
    let dt = time.delta_secs();

    let mut input_dir = Vec3::ZERO;
    if keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp) {
        input_dir.z -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown) {
        input_dir.z += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
        input_dir.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
        input_dir.x += 1.0;
    }

    const DEADZONE: f32 = 0.2;
    for gamepad in &gamepads {
        let stick = gamepad.left_stick();
        if stick.length() > DEADZONE {
            input_dir.x += stick.x;
            input_dir.z -= stick.y;
        }
        let dpad = gamepad.dpad();
        if dpad.length() > 0.1 {
            input_dir.x += dpad.x;
            input_dir.z -= dpad.y;
        }
    }

    let has_input = input_dir.length_squared() > 0.0;
    if has_input {
        input_dir = input_dir.normalize();
    }

    let effective_max_speed = MAX_SPEED * power_ups.as_ref().map_or(1.0, |p| if p.speed_multiplier > 0.0 { p.speed_multiplier } else { 1.0 });

    if has_input {
        physics.velocity.x += input_dir.x * ACCELERATION * dt;
        physics.velocity.z += input_dir.z * ACCELERATION * dt;
        let horiz = Vec2::new(physics.velocity.x, physics.velocity.z);
        if horiz.length() > effective_max_speed {
            let c = horiz.normalize() * effective_max_speed;
            physics.velocity.x = c.x;
            physics.velocity.z = c.y;
        }
    } else {
        let horiz = Vec2::new(physics.velocity.x, physics.velocity.z);
        let speed = horiz.length();
        if speed > 0.01 {
            let factor = ((speed - FRICTION * dt) / speed).max(0.0);
            physics.velocity.x *= factor;
            physics.velocity.z *= factor;
        } else {
            physics.velocity.x = 0.0;
            physics.velocity.z = 0.0;
        }
    }

    let gamepad_jump = gamepads.iter().any(|gp| gp.just_pressed(GamepadButton::South));
    if (keyboard.just_pressed(KeyCode::Space) || gamepad_jump) && physics.grounded {
        let jump_mult = power_ups.as_ref().map_or(1.0, |p| if p.jump_multiplier > 0.0 { p.jump_multiplier } else { 1.0 });
        physics.velocity.y = JUMP_VELOCITY * jump_mult;
        physics.grounded = false;
    }
    if !physics.grounded {
        let grav = gravity_override.as_ref().map_or(GRAVITY, |g| g.0);
        physics.velocity.y += grav * dt;
        physics.velocity.y = physics.velocity.y.max(-20.0);
    }

    let was_airborne = !physics.grounded;
    transform.translation += physics.velocity * dt;

    let (cx, cz) = bounds.clamp(transform.translation.x, transform.translation.z);
    transform.translation.x = cx;
    transform.translation.z = cz;

    let ground_y = ground_y_override.as_ref().map_or(GROUND_Y, |g| g.0);
    if transform.translation.y <= ground_y {
        transform.translation.y = ground_y;
        physics.velocity.y = 0.0;
        if was_airborne {
            squash.timer = 0.3;
        }
        physics.grounded = true;
    }

    // Face movement direction with slight tilt (smoothed to avoid flickering)
    let horiz_vel = Vec2::new(physics.velocity.x, physics.velocity.z);
    if horiz_vel.length() > 0.5 {
        let forward = Vec3::new(horiz_vel.x, 0.0, horiz_vel.y).normalize();
        let reverse = power_ups.as_ref().map_or(false, |p| p.reverse_facing);
        let facing_dir = if reverse { -forward } else { forward };
        let target_rot = Transform::default().looking_to(facing_dir, Vec3::Y).rotation;
        let turn_speed = (10.0 * dt).min(1.0);
        physics.facing = physics.facing.slerp(target_rot, turn_speed);
        let tilt = (horiz_vel.length() / MAX_SPEED) * 0.15;
        transform.rotation = physics.facing * Quat::from_rotation_x(tilt);
    } else {
        transform.rotation = transform.rotation.slerp(physics.facing, (8.0 * dt).min(1.0));
    }
}

pub fn escape_to_menu(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    game_paused: Res<GamePaused>,
    mut next_screen: ResMut<NextState<crate::Screen>>,
) {
    if game_paused.0 {
        return;
    }
    let gamepad_back = gamepads.iter().any(|gp| gp.just_pressed(GamepadButton::Select));
    if keyboard.just_pressed(KeyCode::Escape) || gamepad_back {
        next_screen.set(crate::Screen::Menu);
    }
}

pub fn toggle_pause(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut game_paused: ResMut<GamePaused>,
    mut commands: Commands,
    overlay_q: Query<Entity, With<PauseOverlay>>,
) {
    let gamepad_start = gamepads.iter().any(|gp| gp.just_pressed(GamepadButton::Start));
    if keyboard.just_pressed(KeyCode::KeyP) || gamepad_start {
        game_paused.0 = !game_paused.0;
        if game_paused.0 {
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
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                    GlobalZIndex(20),
                    PauseOverlay,
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Text::new("PAUSED"),
                        TextFont { font_size: 52.0, ..default() },
                        TextColor(Color::srgb(1.0, 1.0, 1.0)),
                    ));
                    parent.spawn((
                        Text::new("Press P or Start to resume"),
                        TextFont { font_size: 22.0, ..default() },
                        TextColor(Color::srgb(0.7, 0.7, 0.7)),
                    ));
                });
        } else {
            for entity in &overlay_q {
                commands.entity(entity).despawn_recursive();
            }
        }
    }
}

pub fn animate_player(
    time: Res<Time>,
    mut player_query: Query<(&PlayerPhysics, &mut SquashState, &Children), With<Player>>,
    mut body_query: Query<
        &mut Transform,
        (With<PlayerBody>, Without<PlayerHead>, Without<Player>),
    >,
    mut head_query: Query<
        &mut Transform,
        (With<PlayerHead>, Without<PlayerBody>, Without<Player>),
    >,
) {
    let Ok((physics, mut squash, children)) = player_query.get_single_mut() else {
        return;
    };
    let dt = time.delta_secs();
    let elapsed = time.elapsed_secs();
    let horiz_speed = Vec2::new(physics.velocity.x, physics.velocity.z).length();

    // Idle bob
    let bob = if horiz_speed < 0.5 && physics.grounded {
        (elapsed * 2.0).sin() * 0.05
    } else {
        0.0
    };

    // Squash on landing
    squash.timer = (squash.timer - dt).max(0.0);
    let (y_scale, xz_scale) = if squash.timer > 0.0 {
        let t = squash.timer / 0.3;
        (1.0 - t * 0.3, 1.0 + t * 0.15)
    } else {
        (1.0, 1.0)
    };

    for &child in children.iter() {
        if let Ok(mut t) = body_query.get_mut(child) {
            t.translation.y = 0.8 + bob;
            t.scale = Vec3::new(xz_scale, y_scale, xz_scale);
        }
        if let Ok(mut t) = head_query.get_mut(child) {
            t.translation.y = 1.5 + bob;
            t.scale = Vec3::new(xz_scale, y_scale, xz_scale);
        }
    }
}
