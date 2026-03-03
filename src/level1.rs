use bevy::prelude::*;

use crate::player::{
    animate_player, escape_to_menu, player_movement, spawn_player, toggle_pause, MovementBounds,
    Player,
};
use crate::{ChallengePhase, GamePaused, Screen, Scoreboard};

const ALLOWED_MIN: Vec2 = Vec2::new(-6.0, -5.0);
const ALLOWED_MAX: Vec2 = Vec2::new(6.0, 5.0);
const PASSWORD_MIN: Vec2 = Vec2::new(6.0, -4.0);
const PASSWORD_MAX: Vec2 = Vec2::new(13.0, 4.0);

pub struct Level1Plugin;

impl Plugin for Level1Plugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StarScore>()
            .add_systems(OnEnter(Screen::PasswordChallenge), setup_world)
            .add_systems(
                FixedUpdate,
                (player_movement, detect_zone)
                    .run_if(in_state(ChallengePhase::Exploring)),
            )
            .add_systems(
                Update,
                (escape_to_menu, toggle_pause).run_if(in_state(ChallengePhase::Exploring)),
            )
            .add_systems(
                Update,
                (
                    follow_camera,
                    animate_player,
                    animate_barriers,
                    update_stars,
                    rotate_orb,
                    dismiss_hint,
                )
                    .run_if(in_state(Screen::PasswordChallenge)),
            )
            .add_systems(OnExit(Screen::PasswordChallenge), cleanup_world);
    }
}

// --- Components ---

#[derive(Component)]
pub struct WorldEntity;

#[derive(Component)]
struct FollowCamera;

#[derive(Component)]
struct Barrier;

#[derive(Component)]
struct StarPickup;

#[derive(Component)]
struct StarScoreText;

#[derive(Component)]
struct PurpleOrb;

#[derive(Component)]
struct HintBox;

#[derive(Component)]
struct HintCloseButton;

// --- Resources ---

#[derive(Resource)]
struct BarrierState {
    lowered: bool,
    offset_y: f32,
}

#[derive(Resource, Default)]
pub struct StarScore {
    pub count: u32,
}

#[derive(Resource)]
struct ZoneGroundMaterial(Handle<StandardMaterial>);

// --- Setup ---

fn setup_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    scoreboard: Res<Scoreboard>,
) {
    commands.insert_resource(ClearColor(Color::srgb(0.55, 0.75, 0.95)));
    commands.insert_resource(StarScore::default());
    commands.insert_resource(BarrierState {
        lowered: scoreboard.password_solved,
        offset_y: if scoreboard.password_solved { -1.5 } else { 0.0 },
    });

    // Main ground (pastel green)
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(12.0, 10.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.4, 0.75, 0.35),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
        WorldEntity,
    ));

    // Restricted zone ground
    let zone_color = if scoreboard.password_solved {
        Color::srgb(0.35, 0.7, 0.35)
    } else {
        Color::srgb(0.7, 0.25, 0.2)
    };
    let zone_mat = materials.add(StandardMaterial {
        base_color: zone_color,
        ..default()
    });
    commands.insert_resource(ZoneGroundMaterial(zone_mat.clone()));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(7.0, 8.0))),
        MeshMaterial3d(zone_mat),
        Transform::from_xyz(9.5, 0.001, 0.0),
        WorldEntity,
    ));

    // Player
    let player = spawn_player(&mut commands, &mut meshes, &mut materials);
    commands.entity(player).insert((
        WorldEntity,
        MovementBounds {
            rects: vec![(ALLOWED_MIN, ALLOWED_MAX), (PASSWORD_MIN, PASSWORD_MAX)],
        },
    ));

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 8.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        FollowCamera,
        WorldEntity,
    ));

    // Sun (large yellow sphere in sky)
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(3.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.95, 0.4),
            emissive: LinearRgba::new(1.0, 0.9, 0.3, 1.0),
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(10.0, 20.0, -15.0),
        WorldEntity,
    ));

    // Directional light with shadows
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -std::f32::consts::FRAC_PI_4,
            std::f32::consts::FRAC_PI_6,
            0.0,
        )),
        WorldEntity,
    ));

    // Warm ambient light
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.95, 0.9, 0.8),
        brightness: 400.0,
    });

    // Environment
    spawn_trees(&mut commands, &mut meshes, &mut materials);
    spawn_rocks(&mut commands, &mut meshes, &mut materials);
    spawn_barriers(&mut commands, &mut meshes, &mut materials, &scoreboard);
    spawn_zone_decor(&mut commands, &mut meshes, &mut materials);
    spawn_stars(&mut commands, &mut meshes, &mut materials);

    // HUD - controls hint
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(16.0),
                left: Val::Px(16.0),
                ..default()
            },
            WorldEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("[Esc] Menu | WASD Move | Space Jump | [P] Pause"),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
            ));
        });

    // HUD - debugger hint box (top right, dismissible)
    if !scoreboard.password_solved {
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(16.0),
                    right: Val::Px(16.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(14.0)),
                    row_gap: Val::Px(8.0),
                    max_width: Val::Px(260.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.1, 0.08, 0.15, 0.9)),
                BorderRadius::all(Val::Px(10.0)),
                HintBox,
                WorldEntity,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new("Hint"),
                    TextFont {
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.85, 0.3)),
                ));
                parent.spawn((
                    Node {
                        max_width: Val::Px(230.0),
                        ..default()
                    },
                    Text::new("Use the debugger to find the password!"),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.85, 0.85)),
                ));
                // Close button
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
                        HintCloseButton,
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("[X] Close"),
                            TextFont {
                                font_size: 14.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.7, 0.7, 0.7)),
                        ));
                    });
            });
    }

    // HUD - star counter (pill-shaped yellow badge)
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                left: Val::Px(16.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 0.85, 0.0, 0.85)),
            BorderRadius::all(Val::Px(12.0)),
            WorldEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Stars: 0"),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::srgb(0.1, 0.1, 0.1)),
                StarScoreText,
            ));
        });

}

// --- Environment helpers ---

fn spawn_trees(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let trunk_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.3, 0.15),
        ..default()
    });
    let foliage_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.75, 0.3),
        ..default()
    });

    let positions: [(f32, f32, f32); 3] = [
        (-4.0, 0.0, -3.5),
        (-5.0, 0.0, 3.0),
        (3.0, 0.0, -4.0),
    ];

    for (i, &(px, _, pz)) in positions.iter().enumerate() {
        let s = 0.8 + 0.3 * (i as f32);
        // Trunk
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.2 * s, 1.5 * s))),
            MeshMaterial3d(trunk_mat.clone()),
            Transform::from_xyz(px, 0.75 * s, pz),
            WorldEntity,
        ));
        // Foliage (puffy sphere)
        commands.spawn((
            Mesh3d(meshes.add(Sphere::new(1.0 * s))),
            MeshMaterial3d(foliage_mat.clone()),
            Transform::from_xyz(px, 1.5 * s + 0.5 * s, pz),
            WorldEntity,
        ));
    }
}

fn spawn_rocks(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let colors = [
        Color::srgb(0.55, 0.5, 0.55),
        Color::srgb(0.5, 0.4, 0.6),
        Color::srgb(0.6, 0.55, 0.5),
    ];
    let positions: [(f32, f32, f32); 5] = [
        (-2.0, 0.2, -2.0),
        (4.0, 0.25, 2.5),
        (-5.0, 0.15, -0.5),
        (1.5, 0.2, 4.0),
        (-3.0, 0.18, 3.5),
    ];

    for (i, &(px, py, pz)) in positions.iter().enumerate() {
        let radius = 0.3 + 0.15 * (i as f32 % 3.0);
        commands.spawn((
            Mesh3d(meshes.add(Sphere::new(radius))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: colors[i % colors.len()],
                ..default()
            })),
            Transform::from_xyz(px, py, pz).with_scale(Vec3::new(1.2, 0.7, 1.0)),
            WorldEntity,
        ));
    }
}

fn spawn_barriers(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    scoreboard: &Res<Scoreboard>,
) {
    if scoreboard.password_solved {
        return;
    }

    let red_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.2, 0.15),
        ..default()
    });
    let white_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.95, 0.95),
        ..default()
    });
    let post_mesh = meshes.add(Cylinder::new(0.12, 1.0));
    let cap_mesh = meshes.add(Sphere::new(0.15));

    // Posts around restricted zone entrance
    let post_positions: [(f32, f32); 8] = [
        (5.5, -4.5),
        (5.5, -3.0),
        (5.5, -1.5),
        (5.5, 1.5),
        (5.5, 3.0),
        (5.5, 4.5),
        (5.5, -0.5),
        (5.5, 0.5),
    ];

    for &(px, pz) in &post_positions {
        // Red post
        commands.spawn((
            Mesh3d(post_mesh.clone()),
            MeshMaterial3d(red_mat.clone()),
            Transform::from_xyz(px, 0.5, pz),
            Barrier,
            WorldEntity,
        ));
        // White cap
        commands.spawn((
            Mesh3d(cap_mesh.clone()),
            MeshMaterial3d(white_mat.clone()),
            Transform::from_xyz(px, 1.05, pz),
            Barrier,
            WorldEntity,
        ));
    }

}

fn spawn_zone_decor(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    // Pedestal
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.4, 0.8))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.4, 0.4, 0.45),
            ..default()
        })),
        Transform::from_xyz(9.5, 0.4, 0.0),
        WorldEntity,
    ));
    // Purple orb
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.35))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.5, 0.15, 0.7),
            emissive: LinearRgba::new(0.6, 0.1, 0.8, 1.0),
            ..default()
        })),
        Transform::from_xyz(9.5, 1.15, 0.0),
        PurpleOrb,
        WorldEntity,
    ));
}

fn spawn_stars(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let star_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.85, 0.1),
        emissive: LinearRgba::new(1.0, 0.8, 0.1, 1.0),
        ..default()
    });
    let star_mesh = meshes.add(Cuboid::new(0.3, 0.3, 0.3));

    let positions: [(f32, f32); 5] = [
        (-3.0, -3.0),
        (4.0, -2.0),
        (-5.0, 2.0),
        (2.0, 4.0),
        (-1.0, -4.5),
    ];

    for &(x, z) in &positions {
        commands.spawn((
            Mesh3d(star_mesh.clone()),
            MeshMaterial3d(star_mat.clone()),
            Transform::from_xyz(x, 1.0, z)
                .with_rotation(Quat::from_euler(EulerRot::XYZ, 0.785, 0.785, 0.0)),
            StarPickup,
            WorldEntity,
        ));
    }
}

// --- Systems ---

fn detect_zone(
    player_query: Query<&Transform, With<Player>>,
    mut next_phase: ResMut<NextState<ChallengePhase>>,
    scoreboard: Res<Scoreboard>,
    game_paused: Res<GamePaused>,
) {
    if game_paused.0 {
        return;
    }
    if scoreboard.password_solved {
        return;
    }
    let Ok(transform) = player_query.get_single() else {
        return;
    };
    let pos = transform.translation;
    if pos.x >= PASSWORD_MIN.x && pos.x <= PASSWORD_MAX.x
        && pos.z >= PASSWORD_MIN.y && pos.z <= PASSWORD_MAX.y
    {
        next_phase.set(ChallengePhase::PasswordPrompt);
    }
}

fn follow_camera(
    player_query: Query<&Transform, (With<Player>, Without<FollowCamera>)>,
    mut camera_query: Query<&mut Transform, (With<FollowCamera>, Without<Player>)>,
    time: Res<Time>,
) {
    let Ok(pt) = player_query.get_single() else {
        return;
    };
    let Ok(mut ct) = camera_query.get_single_mut() else {
        return;
    };
    let target_pos = pt.translation + Vec3::new(0.0, 8.0, 8.0);
    let t = (8.0 * time.delta_secs()).min(1.0);
    ct.translation = ct.translation.lerp(target_pos, t);
    ct.look_at(pt.translation + Vec3::Y, Vec3::Y);
}

fn animate_barriers(
    mut barrier_state: ResMut<BarrierState>,
    scoreboard: Res<Scoreboard>,
    time: Res<Time>,
    mut barrier_query: Query<&mut Transform, With<Barrier>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    zone_mat: Res<ZoneGroundMaterial>,
) {
    if !scoreboard.password_solved || barrier_state.lowered {
        return;
    }

    let delta = 2.0 * time.delta_secs();
    barrier_state.offset_y -= delta;
    if barrier_state.offset_y <= -1.5 {
        barrier_state.offset_y = -1.5;
        barrier_state.lowered = true;
    }

    for mut t in &mut barrier_query {
        t.translation.y -= delta;
    }

    // Lerp zone color from red to green
    if let Some(mat) = materials.get_mut(&zone_mat.0) {
        let progress = (-barrier_state.offset_y / 1.5).min(1.0);
        let r = 0.7 * (1.0 - progress) + 0.35 * progress;
        let g = 0.25 * (1.0 - progress) + 0.7 * progress;
        let b = 0.2 * (1.0 - progress) + 0.35 * progress;
        mat.base_color = Color::srgb(r, g, b);
    }
}

fn update_stars(
    mut commands: Commands,
    time: Res<Time>,
    player_query: Query<&Transform, (With<Player>, Without<StarPickup>)>,
    mut star_query: Query<(Entity, &mut Transform), (With<StarPickup>, Without<Player>)>,
    mut score: ResMut<StarScore>,
    mut text_query: Query<&mut Text, With<StarScoreText>>,
    phase: Res<State<ChallengePhase>>,
) {
    let player_pos = player_query.get_single().map(|t| t.translation).ok();
    let is_exploring = *phase.get() == ChallengePhase::Exploring;

    for (entity, mut t) in &mut star_query {
        // Rotate and bob
        t.rotate_y(2.0 * time.delta_secs());
        t.translation.y =
            1.0 + ((time.elapsed_secs() * 2.0) + t.translation.x * 0.5).sin() * 0.2;

        // Collect if exploring and close to player
        if is_exploring {
            if let Some(pp) = player_pos {
                let dx = pp.x - t.translation.x;
                let dz = pp.z - t.translation.z;
                if dx * dx + dz * dz < 2.25 {
                    commands.entity(entity).despawn_recursive();
                    score.count += 1;
                    if let Ok(mut text) = text_query.get_single_mut() {
                        **text = format!("Stars: {}", score.count);
                    }
                }
            }
        }
    }
}

fn rotate_orb(time: Res<Time>, mut query: Query<&mut Transform, With<PurpleOrb>>) {
    for mut t in &mut query {
        t.rotate_y(1.5 * time.delta_secs());
        t.translation.y = 1.15 + (time.elapsed_secs() * 1.5).sin() * 0.1;
    }
}

fn dismiss_hint(
    mut commands: Commands,
    hint_query: Query<Entity, With<HintBox>>,
    button_query: Query<&Interaction, (Changed<Interaction>, With<HintCloseButton>)>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    let should_close = keyboard.just_pressed(KeyCode::KeyX)
        || button_query
            .iter()
            .any(|i| *i == Interaction::Pressed);

    if should_close {
        for entity in &hint_query {
            commands.entity(entity).despawn_recursive();
        }
    }
}

fn cleanup_world(mut commands: Commands, query: Query<Entity, With<WorldEntity>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}
