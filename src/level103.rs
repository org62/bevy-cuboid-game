use bevy::prelude::*;

use crate::player::{
    animate_player, escape_to_menu, player_movement, spawn_player, toggle_pause, GroundYOverride,
    MovementBounds, Player, PlayerMovementSet, PlayerPhysics,
};
use crate::shared_ui;
use crate::terrain::{
    terrain_collision, CameraOccluder, SolidBlock, TerrainConfig, TerrainSurface,
    WaterSlideSegment, SLIDE_CARRY_SPEED,
};
use crate::{Screen, Scoreboard, WaterparkPhase};

pub struct Level103Plugin;

impl Plugin for Level103Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::WaterparkChallenge), setup_waterpark)
            .add_systems(
                Update,
                (
                    shared_ui::update_camera_orbit.before(PlayerMovementSet),
                    player_movement.in_set(PlayerMovementSet),
                    terrain_collision,
                    water_slide_system,
                    (animate_player, shared_ui::follow_camera_system),
                    snack_eat_system,
                    slides_complete_check,
                    update_progress_text,
                )
                    .chain()
                    .run_if(in_state(WaterparkPhase::Playing)),
            )
            .add_systems(
                Update,
                (escape_to_menu, toggle_pause).run_if(in_state(WaterparkPhase::Playing)),
            )
            .add_systems(
                Update,
                handle_victory.run_if(in_state(WaterparkPhase::Victory)),
            )
            .add_systems(OnExit(Screen::WaterparkChallenge), cleanup_waterpark);
    }
}

// --- Components ---

#[derive(Component, Clone, Copy)]
struct WaterparkEntity;

/// Which colored slide a [`WaterSlideSegment`] belongs to (index into
/// [`SlidesRidden`]).
#[derive(Component)]
struct SlideId(usize);

#[derive(Component)]
struct SnackItem {
    /// > 0 while hidden; counts down to 0 then the snack reappears.
    respawn_timer: f32,
}

#[derive(Component)]
struct ProgressText;

#[derive(Resource, Default)]
struct SlidesRidden([bool; 5]);

// --- Constants ---

const ROOM_X: f32 = 25.0;
const ROOM_Z: f32 = 20.0;
// Short perimeter walls so the over-the-shoulder camera can see over them.
// (A taller enclosure would put the wall mesh between the camera and the player.)
const WALL_HEIGHT: f32 = 2.5;
pub(crate) const POOL_X: f32 = 7.0;
pub(crate) const POOL_Z: f32 = 7.0;
const POOL_DEPTH: f32 = 2.0;
const WATER_Y: f32 = -0.3;
pub(crate) const DECK_TOP: f32 = 8.0;
const PLAYER_SPAWN: Vec3 = Vec3::new(15.0, 0.0, 15.0);
const CAM_OFFSET: Vec3 = Vec3::new(0.0, 22.0, 18.0);

// --- Setup ---

fn setup_waterpark(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(ClearColor(Color::srgb(0.7, 0.85, 0.95)));
    commands.insert_resource(GroundYOverride(-2.5));
    commands.insert_resource(TerrainConfig::standard(-POOL_DEPTH - 0.5));
    commands.insert_resource(SlidesRidden::default());

    // --- Materials ---
    let tile = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.88, 0.92),
        ..default()
    });
    let wall_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.75, 0.85),
        ..default()
    });
    let ceiling_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.95, 0.92),
        ..default()
    });
    let pool_floor_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.55, 0.85),
        ..default()
    });
    let water_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.35, 0.7, 0.95, 0.55),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let deck_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.78, 0.74, 0.62),
        ..default()
    });
    let stair_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.7, 0.55, 0.4),
        ..default()
    });
    let table_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.6, 0.4, 0.25),
        ..default()
    });

    // --- Floor (4 pieces around the pool, 2 units thick so the inner side
    //      faces serve as the visible pool walls) ---
    spawn_floor_piece(
        &mut commands, &mut meshes, &tile,
        Vec2::new(-ROOM_X, -ROOM_Z), Vec2::new(ROOM_X, -POOL_Z),
    );
    spawn_floor_piece(
        &mut commands, &mut meshes, &tile,
        Vec2::new(-ROOM_X, POOL_Z), Vec2::new(ROOM_X, ROOM_Z),
    );
    spawn_floor_piece(
        &mut commands, &mut meshes, &tile,
        Vec2::new(-ROOM_X, -POOL_Z), Vec2::new(-POOL_X, POOL_Z),
    );
    spawn_floor_piece(
        &mut commands, &mut meshes, &tile,
        Vec2::new(POOL_X, -POOL_Z), Vec2::new(ROOM_X, POOL_Z),
    );

    // --- Pool floor + water surface ---
    let pw = POOL_X * 2.0;
    let pl = POOL_Z * 2.0;
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(pw, 0.1, pl))),
        MeshMaterial3d(pool_floor_mat.clone()),
        Transform::from_xyz(0.0, -POOL_DEPTH, 0.0),
        WaterparkEntity,
    ));
    commands.spawn((
        TerrainSurface {
            min: Vec2::new(-POOL_X, -POOL_Z),
            max: Vec2::new(POOL_X, POOL_Z),
            y: -POOL_DEPTH + 0.05,
        },
        WaterparkEntity,
    ));
    // Pool shore SolidBlocks (player can fall in from above; can't walk through walls from inside)
    let shore_thick = 0.5;
    for (min, max) in [
        // North shore
        (Vec2::new(-POOL_X, -POOL_Z - shore_thick), Vec2::new(POOL_X, -POOL_Z)),
        // South shore
        (Vec2::new(-POOL_X, POOL_Z), Vec2::new(POOL_X, POOL_Z + shore_thick)),
        // West shore
        (Vec2::new(-POOL_X - shore_thick, -POOL_Z), Vec2::new(-POOL_X, POOL_Z)),
        // East shore
        (Vec2::new(POOL_X, -POOL_Z), Vec2::new(POOL_X + shore_thick, POOL_Z)),
    ] {
        commands.spawn((
            SolidBlock {
                min, max,
                y_min: -POOL_DEPTH,
                y_max: 0.0,
            },
            CameraOccluder,
            WaterparkEntity,
        ));
    }
    // Water surface (translucent, visual only)
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(pw, 0.05, pl))),
        MeshMaterial3d(water_mat),
        Transform::from_xyz(0.0, WATER_Y, 0.0),
        WaterparkEntity,
    ));

    // --- Pool exit stairs (south-center, NOT the corner). Placing them in the
    //      SE corner caused an oscillating pushout between the stairs' east
    //      edge, the pool east wall, and the pool south wall — the player got
    //      stuck. Inset from both side walls keeps the geometry conflict-free. ---
    let stair_step_h = 0.4_f32;
    let stair_step_count = 5;
    let stair_x_min = -1.0;
    let stair_x_max = 1.0;
    for i in 0..stair_step_count {
        // i=0 deepest, i=4 at pool edge.
        let y_top = -POOL_DEPTH + (i as f32 + 1.0) * stair_step_h;
        let z_min = POOL_Z - (stair_step_count - i) as f32 * 0.5;
        let z_max = z_min + 0.5;
        spawn_block(
            &mut commands, &mut meshes, &pool_floor_mat,
            Vec2::new(stair_x_min, z_min), Vec2::new(stair_x_max, z_max),
            -POOL_DEPTH, y_top,
            false, // walk-on steps: don't occlude the camera
        );
    }

    // --- Outer walls (visible mesh + SolidBlock) ---
    spawn_wall(
        &mut commands, &mut meshes, &wall_mat,
        Vec2::new(-ROOM_X - 0.5, -ROOM_Z - 0.5), Vec2::new(ROOM_X + 0.5, -ROOM_Z),
    );
    spawn_wall(
        &mut commands, &mut meshes, &wall_mat,
        Vec2::new(-ROOM_X - 0.5, ROOM_Z), Vec2::new(ROOM_X + 0.5, ROOM_Z + 0.5),
    );
    spawn_wall(
        &mut commands, &mut meshes, &wall_mat,
        Vec2::new(-ROOM_X - 0.5, -ROOM_Z), Vec2::new(-ROOM_X, ROOM_Z),
    );
    spawn_wall(
        &mut commands, &mut meshes, &wall_mat,
        Vec2::new(ROOM_X, -ROOM_Z), Vec2::new(ROOM_X + 0.5, ROOM_Z),
    );

    // (Ceiling intentionally omitted — the camera looks down from y=22, so any
    // ceiling mesh would block the view.)
    let _ = ceiling_mat;

    // --- Slide deck (one big block at the north end, top y=8) ---
    let deck_min = Vec2::new(-12.0, -19.0);
    let deck_max = Vec2::new(18.0, -14.0);
    spawn_block(
        &mut commands, &mut meshes, &deck_mat,
        deck_min, deck_max, 0.0, DECK_TOP,
        true, // 8-unit-tall block: the camera must not see through it
    );

    // --- Staircase from floor up to the deck (right side, x=14..18) ---
    // 8 steps of 1m height, z spanning from spawn area (z=14) up to deck (z=-14).
    let step_count = 8;
    let stair_x_min = 14.0;
    let stair_x_max = 18.0;
    let stair_z_start = 14.0;
    let stair_z_end = -14.0;
    let step_depth = (stair_z_start - stair_z_end) / step_count as f32; // 3.5
    for i in 0..step_count {
        let y_top = (i as f32 + 1.0) * 1.0;
        let z_max = stair_z_start - i as f32 * step_depth;
        let z_min = z_max - step_depth;
        spawn_block(
            &mut commands, &mut meshes, &stair_mat,
            Vec2::new(stair_x_min, z_min), Vec2::new(stair_x_max, z_max),
            0.0, y_top,
            false, // walk-on steps: don't occlude the camera
        );
    }

    // --- Slides: 5 colored slides descending from deck into pool ---
    let slide_colors: [(Color, &str); 5] = [
        (Color::srgb(0.95, 0.25, 0.25), "red"),
        (Color::srgb(0.95, 0.85, 0.2), "yellow"),
        (Color::srgb(0.3, 0.85, 0.35), "green"),
        (Color::srgb(0.25, 0.55, 0.95), "blue"),
        (Color::srgb(0.7, 0.35, 0.85), "purple"),
    ];
    // Slide centers must stay inside the pool x-range (±7) minus half the slide
    // width (0.7) so the rider lands in the water and not on the deck floor.
    let slide_x_centers = [-6.0, -3.0, 0.0, 3.0, 6.0];
    let slide_segments = 10;
    let slide_w = 1.4;
    let slide_step_z = 0.75; // each segment is 0.75 long in z
    let slide_z_start = -14.0; // first segment top edge (joins deck south edge)
    let slide_step_y = 0.75; // each segment drops 0.75 in y

    for (col_i, &x_center) in slide_x_centers.iter().enumerate() {
        let (color, _name) = slide_colors[col_i];
        let mat = materials.add(StandardMaterial { base_color: color, ..default() });
        for i in 0..slide_segments {
            let z_min = slide_z_start + i as f32 * slide_step_z;
            let z_max = z_min + slide_step_z;
            let y_top = DECK_TOP - (i as f32 + 1.0) * slide_step_y; // first segment top y=7.25, last y=0.5
            let seg_h = 0.4_f32;
            let y_bot = y_top - seg_h;
            let cx = x_center;
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(slide_w, seg_h, slide_step_z))),
                MeshMaterial3d(mat.clone()),
                Transform::from_xyz(cx, y_top - seg_h / 2.0, z_min + slide_step_z / 2.0),
                WaterparkEntity,
            ));
            let min = Vec2::new(cx - slide_w / 2.0, z_min);
            let max = Vec2::new(cx + slide_w / 2.0, z_max);
            // Ride-on geometry: deliberately NOT a CameraOccluder — the camera
            // trails directly over the slide while the player rides it.
            commands.spawn((
                TerrainSurface { min, max, y: y_top },
                SolidBlock { min, max, y_min: y_bot, y_max: y_top },
                WaterSlideSegment {
                    min, max, y: y_top,
                    direction: Vec3::new(0.0, 0.0, 1.0),
                },
                SlideId(col_i),
                WaterparkEntity,
            ));
        }
    }

    // --- Snack table (low platform with food cuboids) ---
    let snack_min = Vec2::new(-19.0, -16.0);
    let snack_max = Vec2::new(-15.0, -12.0);
    spawn_block(
        &mut commands, &mut meshes, &table_mat,
        snack_min, snack_max, 0.0, 1.0,
        false, // low walk-on table
    );
    // Food cuboids on top of table
    let snack_center_x = (snack_min.x + snack_max.x) / 2.0;
    let snack_center_z = (snack_min.y + snack_max.y) / 2.0;
    let food_items: [(Color, Vec3, Vec3); 4] = [
        // (color, size, offset)
        (Color::srgb(1.0, 0.4, 0.4), Vec3::new(0.6, 0.4, 0.6), Vec3::new(-1.2, 0.0, -0.8)),
        (Color::srgb(1.0, 0.85, 0.3), Vec3::new(0.5, 0.5, 0.5), Vec3::new(0.4, 0.0, -1.0)),
        (Color::srgb(0.4, 0.8, 0.4), Vec3::new(0.7, 0.3, 0.5), Vec3::new(-0.3, 0.0, 0.8)),
        (Color::srgb(0.95, 0.95, 0.85), Vec3::new(0.8, 0.5, 0.6), Vec3::new(1.2, 0.0, 0.6)),
    ];
    for (color, size, offset) in food_items {
        let mat = materials.add(StandardMaterial { base_color: color, ..default() });
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
            MeshMaterial3d(mat),
            Transform::from_xyz(
                snack_center_x + offset.x,
                1.0 + size.y / 2.0,
                snack_center_z + offset.z,
            ),
            Visibility::Inherited,
            SnackItem { respawn_timer: 0.0 },
            WaterparkEntity,
        ));
    }

    // --- Lighting (indoor: cool white, brighter ambient) ---
    shared_ui::setup_level_lighting(
        &mut commands,
        8000.0,
        (-0.6, 0.4, 0.0),
        Color::srgb(0.95, 0.95, 1.0),
        1200.0,
        WaterparkEntity,
    );

    // --- Player ---
    let player = spawn_player(&mut commands, &mut meshes, &mut materials);
    commands.entity(player).insert((
        Transform::from_xyz(PLAYER_SPAWN.x, PLAYER_SPAWN.y, PLAYER_SPAWN.z)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        MovementBounds {
            rects: vec![(
                Vec2::new(-ROOM_X + 0.5, -ROOM_Z + 0.5),
                Vec2::new(ROOM_X - 0.5, ROOM_Z - 0.5),
            )],
        },
        WaterparkEntity,
    ));

    // --- Camera ---
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(
            PLAYER_SPAWN.x + CAM_OFFSET.x,
            PLAYER_SPAWN.y + CAM_OFFSET.y,
            PLAYER_SPAWN.z + CAM_OFFSET.z,
        )
        .looking_at(PLAYER_SPAWN, Vec3::Y),
        shared_ui::FollowCamera {
            offset: CAM_OFFSET,
            lerp_speed: 10.0,
            look_offset: Vec3::ZERO,
        },
        WaterparkEntity,
    ));

    // --- HUD ---
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(12.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
            WaterparkEntity,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("Level 15: The Indoor Waterpark"),
                TextFont { font_size: 26.0, ..default() },
                TextColor(Color::WHITE),
                WaterparkEntity,
            ));
            p.spawn((
                Text::new("Slides ridden: 0 / 5"),
                TextFont { font_size: 18.0, ..default() },
                TextColor(Color::srgb(0.9, 0.9, 0.5)),
                ProgressText,
                WaterparkEntity,
            ));
        });
    shared_ui::spawn_controls_hint(
        &mut commands,
        "Ride all 5 slides",
        WaterparkEntity,
    );
}

// --- Spawn helpers ---

fn spawn_floor_piece(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    material: &Handle<StandardMaterial>,
    min: Vec2,
    max: Vec2,
) {
    let w = max.x - min.x;
    let l = max.y - min.y;
    let cx = (min.x + max.x) / 2.0;
    let cz = (min.y + max.y) / 2.0;
    // 2-thick slab from y=-2 to y=0; the side faces serve as the visible pool walls.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(w, 2.0, l))),
        MeshMaterial3d(material.clone()),
        Transform::from_xyz(cx, -1.0, cz),
        WaterparkEntity,
    ));
    commands.spawn((
        TerrainSurface { min, max, y: 0.0 },
        WaterparkEntity,
    ));
}

fn spawn_wall(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    material: &Handle<StandardMaterial>,
    min: Vec2,
    max: Vec2,
) {
    let w = max.x - min.x;
    let l = max.y - min.y;
    let cx = (min.x + max.x) / 2.0;
    let cz = (min.y + max.y) / 2.0;
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(w, WALL_HEIGHT, l))),
        MeshMaterial3d(material.clone()),
        Transform::from_xyz(cx, WALL_HEIGHT / 2.0, cz),
        WaterparkEntity,
    ));
    commands.spawn((
        SolidBlock { min, max, y_min: 0.0, y_max: WALL_HEIGHT },
        CameraOccluder,
        WaterparkEntity,
    ));
}

fn spawn_block(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    material: &Handle<StandardMaterial>,
    min: Vec2,
    max: Vec2,
    y_min: f32,
    y_max: f32,
    camera_occluder: bool,
) {
    let w = max.x - min.x;
    let l = max.y - min.y;
    let h = y_max - y_min;
    let cx = (min.x + max.x) / 2.0;
    let cz = (min.y + max.y) / 2.0;
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(w, h, l))),
        MeshMaterial3d(material.clone()),
        Transform::from_xyz(cx, (y_min + y_max) / 2.0, cz),
        WaterparkEntity,
    ));
    let mut collider = commands.spawn((
        TerrainSurface { min, max, y: y_max },
        SolidBlock { min, max, y_min, y_max },
        WaterparkEntity,
    ));
    if camera_occluder {
        collider.insert(CameraOccluder);
    }
}

// --- Water slide system ---
// Each segment pushes the player along its `direction`, AND the player's y is
// overridden every frame with the slide's continuous slope (so the descent is
// one smooth ramp rather than 10 micro-teleports between segment surfaces).

// Slide endpoints — all 5 slides share the same z range and slope.
const SLIDE_TOP_Y: f32 = 8.0;
const SLIDE_BOTTOM_Y: f32 = 0.5;
const SLIDE_TOP_Z: f32 = -14.0;
pub(crate) const SLIDE_BOTTOM_Z: f32 = -6.5;

fn water_slide_system(
    slides: Query<(&WaterSlideSegment, &SlideId)>,
    mut player_q: Query<(&mut Transform, &mut PlayerPhysics), With<Player>>,
    mut ridden: ResMut<SlidesRidden>,
) {
    let Ok((mut transform, mut physics)) = player_q.get_single_mut() else { return };
    for (seg, slide_id) in &slides {
        if seg.carries(transform.translation) {
            let pz = transform.translation.z;
            physics.velocity.x = seg.direction.x * SLIDE_CARRY_SPEED;
            physics.velocity.z = seg.direction.z * SLIDE_CARRY_SPEED;
            let t = ((pz - SLIDE_TOP_Z) / (SLIDE_BOTTOM_Z - SLIDE_TOP_Z)).clamp(0.0, 1.0);
            transform.translation.y = SLIDE_TOP_Y + (SLIDE_BOTTOM_Y - SLIDE_TOP_Y) * t;
            physics.velocity.y = 0.0;
            if let Some(slot) = ridden.0.get_mut(slide_id.0) {
                *slot = true;
            }
            break;
        }
    }
}

// --- Snacks: eat on touch, respawn after a few seconds ---

const SNACK_RESPAWN_SECS: f32 = 4.0;
const SNACK_EAT_RADIUS_SQ: f32 = 0.9 * 0.9;

fn snack_eat_system(
    time: Res<Time>,
    player_q: Query<&Transform, With<Player>>,
    mut snacks: Query<(&Transform, &mut Visibility, &mut SnackItem)>,
) {
    let Ok(player_tf) = player_q.get_single() else { return };
    let p = player_tf.translation;
    for (tf, mut vis, mut snack) in &mut snacks {
        if snack.respawn_timer > 0.0 {
            snack.respawn_timer -= time.delta_secs();
            if snack.respawn_timer <= 0.0 {
                snack.respawn_timer = 0.0;
                *vis = Visibility::Inherited;
            }
            continue;
        }
        if p.distance_squared(tf.translation) < SNACK_EAT_RADIUS_SQ {
            *vis = Visibility::Hidden;
            snack.respawn_timer = SNACK_RESPAWN_SECS;
        }
    }
}

// --- Win condition: ride all 5 colored slides ---

fn slides_complete_check(
    ridden: Res<SlidesRidden>,
    mut scoreboard: ResMut<Scoreboard>,
    mut next_phase: ResMut<NextState<WaterparkPhase>>,
) {
    if ridden.0.iter().all(|&b| b) {
        scoreboard.set_solved(103);
        next_phase.set(WaterparkPhase::Victory);
    }
}

fn update_progress_text(
    ridden: Res<SlidesRidden>,
    mut text_q: Query<&mut Text, With<ProgressText>>,
) {
    if !ridden.is_changed() { return; }
    let count = ridden.0.iter().filter(|&&b| b).count();
    if let Ok(mut text) = text_q.get_single_mut() {
        *text = Text::new(format!("Slides ridden: {} / 5", count));
    }
}

// --- Victory ---

fn handle_victory(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    overlay_q: Query<Entity, With<shared_ui::OverlayScreen>>,
    mut next_screen: ResMut<NextState<Screen>>,
) {
    if overlay_q.is_empty() {
        shared_ui::spawn_victory_overlay(
            &mut commands,
            "WATERPARK COMPLETE!",
            Some("You rode every colored slide!"),
            22.0,
            "Press ENTER to return to menu",
            WaterparkEntity,
        );
    }
    let pressed = keyboard.just_pressed(KeyCode::Enter)
        || gamepads.iter().any(|g| g.just_pressed(GamepadButton::South));
    if pressed {
        next_screen.set(Screen::Menu);
    }
}

// --- Cleanup ---

fn cleanup_waterpark(mut commands: Commands, query: Query<Entity, With<WaterparkEntity>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
    commands.remove_resource::<GroundYOverride>();
    commands.remove_resource::<TerrainConfig>();
    commands.remove_resource::<SlidesRidden>();
}
