use bevy::prelude::*;

use crate::player::{
    spawn_player, GroundYOverride, MovementBounds, PowerUpState,
};
use crate::shared_ui;
use crate::Screen;

use super::components::*;
use super::constants::*;
use super::maze;
use super::powerups::random_apple_pos;
use super::race;
use super::resources::*;

pub(super) fn setup_hill(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(ClearColor(Color::srgb(0.45, 0.7, 0.9)));
    commands.insert_resource(HillState::default());
    commands.insert_resource(GroundYOverride(-3.0));
    commands.insert_resource(PowerUpState::default());

    let gray = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.5, 0.5),
        ..default()
    });
    let dark_gray = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.35, 0.35),
        ..default()
    });
    let green = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.7, 0.2),
        ..default()
    });
    let brown = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.35, 0.2),
        ..default()
    });
    let ice_blue = materials.add(StandardMaterial {
        base_color: Color::srgba(0.5, 0.8, 1.0, 0.85),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let _red = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.15, 0.1),
        ..default()
    });
    let gold = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.85, 0.0),
        ..default()
    });
    let water_blue = materials.add(StandardMaterial {
        base_color: Color::srgba(0.2, 0.4, 0.8, 0.5),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let dark_green = materials.add(StandardMaterial {
        base_color: Color::srgb(0.15, 0.4, 0.1),
        ..default()
    });
    let stone = materials.add(StandardMaterial {
        base_color: Color::srgb(0.6, 0.58, 0.55),
        ..default()
    });
    let fence_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.4, 0.25, 0.15),
        ..default()
    });
    let _white = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        ..default()
    });

    // Ground plane 60x60 — split to leave hole for pool at (-18, 0) size 8x6
    // Pool spans x=-22..-14, z=-3..3
    // Right section: x=-14 to 30 (width 44)
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(44.0, 0.2, 60.0))),
        MeshMaterial3d(green.clone()),
        Transform::from_xyz(8.0, -0.1, 0.0),
        HillEntity,
    ));
    commands.spawn((
        TerrainSurface { min: Vec2::new(-14.5, -30.0), max: Vec2::new(30.0, 30.0), y: 0.0 },
        HillEntity,
    ));
    // Left section: x=-30 to -22 (width 8)
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(8.0, 0.2, 60.0))),
        MeshMaterial3d(green.clone()),
        Transform::from_xyz(-26.0, -0.1, 0.0),
        HillEntity,
    ));
    commands.spawn((
        TerrainSurface { min: Vec2::new(-30.0, -30.0), max: Vec2::new(-21.5, 30.0), y: 0.0 },
        HillEntity,
    ));
    // Top strip (behind pool): x=-22..-14, z=-30..-3 (width 8, depth 27)
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(8.0, 0.2, 27.0))),
        MeshMaterial3d(green.clone()),
        Transform::from_xyz(-18.0, -0.1, -16.5),
        HillEntity,
    ));
    commands.spawn((
        TerrainSurface { min: Vec2::new(-22.5, -30.0), max: Vec2::new(-13.5, -2.5), y: 0.0 },
        HillEntity,
    ));
    // Bottom strip (in front of pool): x=-22..-14, z=3..30 (width 8, depth 27)
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(8.0, 0.2, 27.0))),
        MeshMaterial3d(green.clone()),
        Transform::from_xyz(-18.0, -0.1, 16.5),
        HillEntity,
    ));
    commands.spawn((
        TerrainSurface { min: Vec2::new(-22.5, 2.5), max: Vec2::new(-13.5, 30.0), y: 0.0 },
        HillEntity,
    ));

    // --- Hill tiers (stacked cuboids with tunnel gap in base) ---

    // --- Hill tiers: 10 steps of 1-unit height, each 0.8 units narrower per side ---
    // Each tier is jumpable (max jump ~1.6 units).
    // Tiers 0-2 (y=0..3) have a 3-unit wide tunnel gap along x=0 for a walk-through passage.
    // Tier i: height 1.0, y_center = 0.5 + i, half_size = 9.0 - i * 0.8
    let tunnel_height = 3; // first 3 tiers have tunnel gap
    let tunnel_half_w = 1.5_f32; // tunnel is 3 units wide
    for i in 0..10u32 {
        let h = 1.0_f32;
        let y_center = 0.5 + i as f32;
        let y_top = y_center + h / 2.0;
        let y_bot = y_center - h / 2.0;
        let half = 9.0 - i as f32 * 0.8; // from 9.0 down to 1.8

        if (i as i32) < tunnel_height && half > tunnel_half_w {
            // Split tier into two halves with tunnel gap
            let side_w = half - tunnel_half_w;
            // Left half
            let left_cx = -(tunnel_half_w + side_w / 2.0);
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(side_w, h, half * 2.0))),
                MeshMaterial3d(gray.clone()),
                Transform::from_xyz(left_cx, y_center, 0.0),
                HillEntity,
            ));
            commands.spawn((
                TerrainSurface { min: Vec2::new(-half, -half), max: Vec2::new(-tunnel_half_w, half), y: y_top },
                SolidBlock { min: Vec2::new(-half, -half), max: Vec2::new(-tunnel_half_w, half), y_min: y_bot, y_max: y_top },
                HillEntity,
            ));
            // Right half
            let right_cx = tunnel_half_w + side_w / 2.0;
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(side_w, h, half * 2.0))),
                MeshMaterial3d(gray.clone()),
                Transform::from_xyz(right_cx, y_center, 0.0),
                HillEntity,
            ));
            commands.spawn((
                TerrainSurface { min: Vec2::new(tunnel_half_w, -half), max: Vec2::new(half, half), y: y_top },
                SolidBlock { min: Vec2::new(tunnel_half_w, -half), max: Vec2::new(half, half), y_min: y_bot, y_max: y_top },
                HillEntity,
            ));
        } else {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(half * 2.0, h, half * 2.0))),
                MeshMaterial3d(gray.clone()),
                Transform::from_xyz(0.0, y_center, 0.0),
                HillEntity,
            ));
            commands.spawn((
                TerrainSurface { min: Vec2::new(-half, -half), max: Vec2::new(half, half), y: y_top },
                SolidBlock { min: Vec2::new(-half, -half), max: Vec2::new(half, half), y_min: y_bot, y_max: y_top },
                HillEntity,
            ));
        }
    }

    // Tunnel ceiling (at y=3, spanning the tunnel length)
    let base_half = 9.0_f32;
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(3.0, 0.3, base_half * 2.0))),
        MeshMaterial3d(dark_gray.clone()),
        Transform::from_xyz(0.0, 3.15, 0.0),
        HillEntity,
    ));

    // Tunnel floor terrain (ground level through tunnel)
    commands.spawn((
        TerrainSurface { min: Vec2::new(-tunnel_half_w, -base_half), max: Vec2::new(tunnel_half_w, base_half), y: 0.0 },
        HillEntity,
    ));

    // --- Slide on east side (10 stepped platforms from summit y=9 down to y=0) ---
    for i in 0..10 {
        let slide_y = 9.0 - i as f32 * 0.9;
        let slide_x = 5.0 + i as f32 * 2.0; // from x=5 to x=23
        let step_h = 1.0_f32; // solid height for collision
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(2.0, step_h, 3.0))),
            MeshMaterial3d(ice_blue.clone()),
            Transform::from_xyz(slide_x, slide_y, 0.0),
            HillEntity,
        ));
        let y_top = slide_y + step_h / 2.0;
        let y_bot = slide_y - step_h / 2.0;
        commands.spawn((
            TerrainSurface {
                min: Vec2::new(slide_x - 1.0, -1.5),
                max: Vec2::new(slide_x + 1.0, 1.5),
                y: y_top,
            },
            SolidBlock {
                min: Vec2::new(slide_x - 1.0, -1.5),
                max: Vec2::new(slide_x + 1.0, 1.5),
                y_min: y_bot,
                y_max: y_top,
            },
            HillEntity,
        ));
        commands.spawn((
            SlideSegment {
                min: Vec2::new(slide_x - 1.0, -1.5),
                max: Vec2::new(slide_x + 1.0, 1.5),
                y: y_top,
            },
            HillEntity,
        ));
    }

    // --- Upper ice slide (sky level, 6 segments from y=17 down to y=12.5) ---
    for i in 0..6 {
        let slide_y = 17.0 - i as f32 * 0.9;
        let slide_x = i as f32 * 1.0;
        let step_h = 1.0_f32;
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(2.0, step_h, 3.0))),
            MeshMaterial3d(ice_blue.clone()),
            Transform::from_xyz(slide_x, slide_y, 0.0),
            HillEntity,
        ));
        let y_top = slide_y + step_h / 2.0;
        let y_bot = slide_y - step_h / 2.0;
        commands.spawn((
            TerrainSurface {
                min: Vec2::new(slide_x - 1.0, -1.5),
                max: Vec2::new(slide_x + 1.0, 1.5),
                y: y_top,
            },
            SolidBlock {
                min: Vec2::new(slide_x - 1.0, -1.5),
                max: Vec2::new(slide_x + 1.0, 1.5),
                y_min: y_bot,
                y_max: y_top,
            },
            HillEntity,
        ));
        commands.spawn((
            SlideSegment {
                min: Vec2::new(slide_x - 1.0, -1.5),
                max: Vec2::new(slide_x + 1.0, 1.5),
                y: y_top,
            },
            HillEntity,
        ));
    }

    // --- Water slide on west side (12 stepped platforms from hilltop down to pool) ---
    for i in 0..12 {
        let slide_x = -2.0 - i as f32 * 1.1;
        let slide_y = 9.5 - i as f32 * 0.9;
        let step_h = 0.5_f32;
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(1.2, step_h, 3.0))),
            MeshMaterial3d(water_blue.clone()),
            Transform::from_xyz(slide_x, slide_y, 0.0),
            HillEntity,
        ));
        let y_top = slide_y + step_h / 2.0;
        let y_bot = slide_y - step_h / 2.0;
        commands.spawn((
            TerrainSurface {
                min: Vec2::new(slide_x - 0.6, -1.5),
                max: Vec2::new(slide_x + 0.6, 1.5),
                y: y_top,
            },
            SolidBlock {
                min: Vec2::new(slide_x - 0.6, -1.5),
                max: Vec2::new(slide_x + 0.6, 1.5),
                y_min: y_bot,
                y_max: y_top,
            },
            WaterSlideSegment {
                min: Vec2::new(slide_x - 0.6, -1.5),
                max: Vec2::new(slide_x + 0.6, 1.5),
                y: y_top,
            },
            HillEntity,
        ));
    }

    // --- Victory flag at hilltop ---
    // Pole
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.1, 3.0, 0.1))),
        MeshMaterial3d(brown.clone()),
        Transform::from_xyz(0.0, 11.5, 0.0),
        HillEntity,
    ));
    // Flag
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 0.6, 0.05))),
        MeshMaterial3d(gold.clone()),
        Transform::from_xyz(0.5, 12.5, 0.0),
        HillEntity,
    ));

    // --- Exploration elements ---

    // Trees (trunk + canopy)
    let tree_positions = [
        Vec3::new(-15.0, 0.0, -10.0),
        Vec3::new(-20.0, 0.0, 5.0),
        Vec3::new(20.0, 0.0, 10.0),
        Vec3::new(-10.0, 0.0, 20.0),
        Vec3::new(25.0, 0.0, -5.0),
        Vec3::new(-25.0, 0.0, -20.0),
        Vec3::new(10.0, 0.0, 22.0),
    ];
    let trunk_mesh = meshes.add(Cuboid::new(0.4, 2.0, 0.4));
    let canopy_mesh = meshes.add(Cuboid::new(2.0, 2.0, 2.0));
    for pos in &tree_positions {
        commands.spawn((
            Mesh3d(trunk_mesh.clone()),
            MeshMaterial3d(brown.clone()),
            Transform::from_xyz(pos.x, 1.0, pos.z),
            HillEntity,
        ));
        commands.spawn((
            Mesh3d(canopy_mesh.clone()),
            MeshMaterial3d(dark_green.clone()),
            Transform::from_xyz(pos.x, 3.0, pos.z),
            HillEntity,
        ));
    }

    // Rocks
    let rock_positions = [
        Vec3::new(14.0, 0.3, 12.0),
        Vec3::new(-8.0, 0.3, -18.0),
        Vec3::new(8.0, 0.3, 18.0),
        Vec3::new(-22.0, 0.3, 15.0),
        Vec3::new(18.0, 0.3, 20.0),
        Vec3::new(12.0, 0.3, -20.0),
    ];
    let rock_mesh = meshes.add(Cuboid::new(1.2, 0.6, 0.9));
    for pos in &rock_positions {
        commands.spawn((
            Mesh3d(rock_mesh.clone()),
            MeshMaterial3d(stone.clone()),
            Transform::from_xyz(pos.x, pos.y, pos.z),
            HillEntity,
        ));
    }

    // --- Zip line from hill top to maze entrance ---
    let zip_start = Vec3::new(0.0, 10.5, -2.5);
    let zip_end = Vec3::new(21.0, 0.5, -13.0);
    let zip_pole_brown = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.3, 0.15),
        ..default()
    });
    // Start pole
    let start_pole_h = 4.0;
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.2, start_pole_h, 0.2))),
        MeshMaterial3d(zip_pole_brown.clone()),
        Transform::from_xyz(zip_start.x, 10.0 + start_pole_h / 2.0, zip_start.z),
        HillEntity,
    ));
    // End pole
    let end_pole_h = 2.0;
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.2, end_pole_h, 0.2))),
        MeshMaterial3d(zip_pole_brown),
        Transform::from_xyz(zip_end.x, end_pole_h / 2.0, zip_end.z),
        HillEntity,
    ));
    // Cable between pole tops
    let cable_start = Vec3::new(zip_start.x, 10.0 + start_pole_h, zip_start.z);
    let cable_end = Vec3::new(zip_end.x, end_pole_h, zip_end.z);
    let cable_mid = (cable_start + cable_end) / 2.0;
    let cable_dir = cable_end - cable_start;
    let cable_len = cable_dir.length();
    let cable_rot = Quat::from_rotation_arc(Vec3::Y, cable_dir.normalize());
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.08, cable_len, 0.08))),
        MeshMaterial3d(dark_gray.clone()),
        Transform::from_translation(cable_mid).with_rotation(cable_rot),
        HillEntity,
    ));
    // Start platform (trigger)
    let zip_platform_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.85, 0.0),
        emissive: LinearRgba::new(1.0, 0.7, 0.0, 1.0),
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.5, 0.2, 1.5))),
        MeshMaterial3d(zip_platform_mat),
        Transform::from_xyz(zip_start.x, 10.1, zip_start.z),
        ZipLinePlatform,
        HillEntity,
    ));

    // --- Maze (east side, ground level) ---
    let mut maze_rng = AppleRng::new();

    // All maze walls (boundary + interior + corner posts) — spawned as one grid system
    maze::spawn_maze_interior(&mut commands, &mut meshes, &mut materials, &mut maze_rng);

    // --- Teleporter pad (gold glowing platform) ---
    let teleporter_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.85, 0.0),
        emissive: LinearRgba::new(1.0, 0.7, 0.0, 1.0),
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.5, 0.3, 2.5))),
        MeshMaterial3d(teleporter_mat),
        Transform::from_xyz(21.0, 0.15, -10.0),
        TeleporterPad { destination: Vec3::new(0.0, 17.5, 0.0) },
        HillEntity,
    ));

    // --- Chipmunk statue (east side) ---
    let tan = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.65, 0.4),
        ..default()
    });
    let dark_brown = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.2, 0.1),
        ..default()
    });
    commands
        .spawn((
            Transform::from_xyz(18.0, 0.0, -8.0),
            Visibility::default(),
            HillEntity,
        ))
        .with_children(|parent| {
            // Pedestal
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(2.0, 0.4, 2.0))),
                MeshMaterial3d(stone.clone()),
                Transform::from_xyz(0.0, 0.2, 0.0),
            ));
            // Body
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.7, 1.0, 0.6))),
                MeshMaterial3d(brown.clone()),
                Transform::from_xyz(0.0, 1.0, 0.0),
            ));
            // Belly
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.5, 0.6, 0.1))),
                MeshMaterial3d(tan.clone()),
                Transform::from_xyz(0.0, 0.9, -0.3),
            ));
            // Head
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.6, 0.55, 0.55))),
                MeshMaterial3d(brown.clone()),
                Transform::from_xyz(0.0, 1.8, 0.0),
            ));
            // Left cheek
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.15, 0.2, 0.1))),
                MeshMaterial3d(tan.clone()),
                Transform::from_xyz(-0.2, 1.75, -0.3),
            ));
            // Right cheek
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.15, 0.2, 0.1))),
                MeshMaterial3d(tan.clone()),
                Transform::from_xyz(0.2, 1.75, -0.3),
            ));
            // Left ear
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.12, 0.15, 0.08))),
                MeshMaterial3d(brown.clone()),
                Transform::from_xyz(-0.2, 2.15, 0.0),
            ));
            // Right ear
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.12, 0.15, 0.08))),
                MeshMaterial3d(brown.clone()),
                Transform::from_xyz(0.2, 2.15, 0.0),
            ));
            // Nose
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.08, 0.08, 0.08))),
                MeshMaterial3d(dark_brown.clone()),
                Transform::from_xyz(0.0, 1.78, -0.3),
            ));
            // Left eye
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.08, 0.08, 0.08))),
                MeshMaterial3d(dark_brown.clone()),
                Transform::from_xyz(-0.12, 1.85, -0.28),
            ));
            // Right eye
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.08, 0.08, 0.08))),
                MeshMaterial3d(dark_brown.clone()),
                Transform::from_xyz(0.12, 1.85, -0.28),
            ));
            // Tail
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.2, 0.8, 0.2))),
                MeshMaterial3d(brown.clone()),
                Transform::from_xyz(0.0, 1.2, 0.4),
            ));
            // Back stripe
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.1, 0.6, 0.1))),
                MeshMaterial3d(dark_brown),
                Transform::from_xyz(0.0, 1.0, 0.31),
            ));
        });

    // Sunken pool (left of hill) — player falls in and must jump out
    let pool_depth = 1.0_f32;
    let pool_x = -18.0_f32;
    let pool_z = 0.0_f32;
    let pool_w = 8.0_f32;
    let pool_l = 6.0_f32;
    let pool_x_min = pool_x - pool_w / 2.0; // -22
    let pool_x_max = pool_x + pool_w / 2.0; // -14
    let pool_z_min = pool_z - pool_l / 2.0; // -3
    let pool_z_max = pool_z + pool_l / 2.0; //  3
    // Pool floor
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(pool_w, 0.1, pool_l))),
        MeshMaterial3d(water_blue.clone()),
        Transform::from_xyz(pool_x, -pool_depth, pool_z),
        HillEntity,
    ));
    commands.spawn((
        TerrainSurface {
            min: Vec2::new(pool_x_min, pool_z_min),
            max: Vec2::new(pool_x_max, pool_z_max),
            y: -pool_depth + 0.05,
        },
        HillEntity,
    ));
    // Pool shore walls — solid blocks so player bumps into edges and must jump out
    // These are the ground edges around the pool, acting as walls from the pool side.
    let wall_thick = 0.5;
    // East shore (x = pool_x_max)
    commands.spawn((
        SolidBlock {
            min: Vec2::new(pool_x_max, pool_z_min),
            max: Vec2::new(pool_x_max + wall_thick, pool_z_max),
            y_min: -pool_depth,
            y_max: 0.0,
        },
        HillEntity,
    ));
    // West shore (x = pool_x_min)
    commands.spawn((
        SolidBlock {
            min: Vec2::new(pool_x_min - wall_thick, pool_z_min),
            max: Vec2::new(pool_x_min, pool_z_max),
            y_min: -pool_depth,
            y_max: 0.0,
        },
        HillEntity,
    ));
    // North shore (z = pool_z_min)
    commands.spawn((
        SolidBlock {
            min: Vec2::new(pool_x_min, pool_z_min - wall_thick),
            max: Vec2::new(pool_x_max, pool_z_min),
            y_min: -pool_depth,
            y_max: 0.0,
        },
        HillEntity,
    ));
    // South shore (z = pool_z_max)
    commands.spawn((
        SolidBlock {
            min: Vec2::new(pool_x_min, pool_z_max),
            max: Vec2::new(pool_x_max, pool_z_max + wall_thick),
            y_min: -pool_depth,
            y_max: 0.0,
        },
        HillEntity,
    ));
    // Water surface (translucent)
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(pool_w, 0.05, pool_l))),
        MeshMaterial3d(water_blue),
        Transform::from_xyz(pool_x, -0.3, pool_z),
        HillEntity,
    ));

    // Path stones from spawn to hill
    let path_stone_mesh = meshes.add(Cuboid::new(1.5, 0.1, 1.0));
    for i in 0..6 {
        let z = 24.0 - i as f32 * 3.0;
        commands.spawn((
            Mesh3d(path_stone_mesh.clone()),
            MeshMaterial3d(stone.clone()),
            Transform::from_xyz(0.0, 0.01, z),
            HillEntity,
        ));
    }

    // Boundary fences (4 walls around the 60x60 area)
    let fence_thickness = 0.3;
    let fence_height = 1.5;
    // North
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(60.0, fence_height, fence_thickness))),
        MeshMaterial3d(fence_mat.clone()),
        Transform::from_xyz(0.0, fence_height / 2.0, -30.0),
        HillEntity,
    ));
    // South
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(60.0, fence_height, fence_thickness))),
        MeshMaterial3d(fence_mat.clone()),
        Transform::from_xyz(0.0, fence_height / 2.0, 30.0),
        HillEntity,
    ));
    // East
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(fence_thickness, fence_height, 60.0))),
        MeshMaterial3d(fence_mat.clone()),
        Transform::from_xyz(30.0, fence_height / 2.0, 0.0),
        HillEntity,
    ));
    // West
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(fence_thickness, fence_height, 60.0))),
        MeshMaterial3d(fence_mat),
        Transform::from_xyz(-30.0, fence_height / 2.0, 0.0),
        HillEntity,
    ));

    // --- Lighting ---
    shared_ui::setup_level_lighting(
        &mut commands,
        12000.0,
        (-0.8, 0.3, 0.0),
        Color::WHITE,
        400.0,
        HillEntity,
    );

    // --- Player ---
    let player = spawn_player(&mut commands, &mut meshes, &mut materials);
    commands.entity(player).insert((
        Transform::from_xyz(PLAYER_SPAWN.x, PLAYER_SPAWN.y, PLAYER_SPAWN.z)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        MovementBounds {
            rects: vec![(Vec2::new(-29.5, -29.5), Vec2::new(29.5, 29.5))],
        },
        HillEntity,
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
        shared_ui::FollowCamera { offset: CAM_OFFSET, lerp_speed: 12.0, look_offset: Vec3::ZERO },
        HillEntity,
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
            HillEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Level 13: The Hill Fortress"),
                TextFont { font_size: 26.0, ..default() },
                TextColor(Color::WHITE),
                HillEntity,
            ));
            parent.spawn((
                Text::new("Reach the flag at the summit!"),
                TextFont { font_size: 18.0, ..default() },
                TextColor(Color::srgb(0.9, 0.9, 0.5)),
                HudText,
                HillEntity,
            ));
        });

    // Hint text at bottom
    shared_ui::spawn_controls_hint(&mut commands, "[ESC] Menu  |  [WASD] Move  |  [Space] Jump  |  [P] Pause", HillEntity);

    // --- Power-up apples ---
    let apple_green = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.9, 0.2),
        ..default()
    });
    let apple_red = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.15, 0.15),
        ..default()
    });
    let apple_purple = materials.add(StandardMaterial {
        base_color: Color::srgb(0.7, 0.2, 0.9),
        ..default()
    });
    let stem_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.3, 0.15),
        ..default()
    });

    let apple_sphere = meshes.add(Cuboid::new(0.6, 0.6, 0.6));
    let stem_mesh = meshes.add(Cuboid::new(0.08, 0.25, 0.08));

    let apple_assets = AppleAssets {
        sphere: apple_sphere.clone(),
        stem: stem_mesh.clone(),
        green: apple_green.clone(),
        red: apple_red.clone(),
        purple: apple_purple.clone(),
        stem_mat: stem_mat.clone(),
    };

    let mut apple_rng = AppleRng::new();
    let apples = [
        (random_apple_pos(&mut apple_rng), AppleKind::Speed, apple_green),
        (random_apple_pos(&mut apple_rng), AppleKind::Jump, apple_red),
        (random_apple_pos(&mut apple_rng), AppleKind::Backwards, apple_purple),
    ];
    for (pos, kind, mat) in apples {
        commands
            .spawn((
                Transform::from_translation(pos),
                Visibility::default(),
                PowerUpApple { kind },
                HillEntity,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Mesh3d(apple_sphere.clone()),
                    MeshMaterial3d(mat),
                    Transform::IDENTITY,
                ));
                parent.spawn((
                    Mesh3d(stem_mesh.clone()),
                    MeshMaterial3d(stem_mat.clone()),
                    Transform::from_xyz(0.0, 0.45, 0.0),
                ));
            });
    }

    commands.insert_resource(ActivePowerUps::default());
    commands.insert_resource(apple_assets);
    commands.insert_resource(apple_rng);
    commands.insert_resource(ColorCubeState::default());

    // --- Race track (snake-like with 90° turns, pool bridge, checkpoints) ---
    let track_dark_gray = materials.add(StandardMaterial {
        base_color: Color::srgb(0.25, 0.25, 0.25),
        ..default()
    });
    let track_white = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        ..default()
    });

    // Track surface segments between consecutive waypoints
    for i in 0..NUM_WAYPOINTS {
        let a = RACE_WAYPOINTS[i];
        let b = RACE_WAYPOINTS[(i + 1) % NUM_WAYPOINTS];
        let mid = (a + b) / 2.0;
        let dir = b - a;
        let len = dir.length();
        let angle = f32::atan2(dir.x, dir.z);

        let (height, y_pos) = (0.04, 0.02);

        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(TRACK_WIDTH, height, len))),
            MeshMaterial3d(track_dark_gray.clone()),
            Transform::from_xyz(mid.x, y_pos, mid.z)
                .with_rotation(Quat::from_rotation_y(angle)),
            HillEntity,
        ));
    }

    // Corner fills at each waypoint (square patches to fill 90° turn gaps)
    let corner_mesh = meshes.add(Cuboid::new(TRACK_WIDTH, 0.04, TRACK_WIDTH));
    for wp in &RACE_WAYPOINTS {
        commands.spawn((
            Mesh3d(corner_mesh.clone()),
            MeshMaterial3d(track_dark_gray.clone()),
            Transform::from_xyz(wp.x, 0.02, wp.z),
            HillEntity,
        ));
    }

    // Checkpoint materials (unlit = dim gray, player = blue, bot = green)
    let cp_unlit = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.3, 0.3),
        ..default()
    });
    let cp_player_lit = materials.add(StandardMaterial {
        base_color: Color::srgb(0.1, 0.5, 1.0),
        emissive: LinearRgba::new(0.1, 0.5, 1.5, 1.0),
        ..default()
    });
    let cp_bot_lit = materials.add(StandardMaterial {
        base_color: Color::srgb(0.1, 0.8, 0.2),
        emissive: LinearRgba::new(0.1, 1.2, 0.2, 1.0),
        ..default()
    });
    commands.insert_resource(CheckpointMaterials {
        unlit: cp_unlit.clone(),
        player_lit: cp_player_lit,
        bot_lit: cp_bot_lit,
    });

    // Checkpoint markers: two pillars per checkpoint (player = left, bot = right)
    // Offset 2 units back along the incoming segment so they sit before the 90° turn.
    let pillar_mesh = meshes.add(Cuboid::new(0.3, 1.0, 0.3));
    let cp_pull_back = 2.0_f32; // distance before the turn
    for (i, &cp_idx) in RACE_CHECKPOINT_INDICES.iter().enumerate() {
        let wp = RACE_WAYPOINTS[cp_idx];
        let prev = if cp_idx == 0 { RACE_WAYPOINTS[NUM_WAYPOINTS - 1] } else { RACE_WAYPOINTS[cp_idx - 1] };
        let incoming_dir = (wp - prev).normalize();
        // Pull checkpoint back along the incoming segment
        let pos = wp - incoming_dir * cp_pull_back;
        // Perpendicular in XZ plane for side offset
        let perp = Vec3::new(-incoming_dir.z, 0.0, incoming_dir.x);
        let side = perp * (TRACK_WIDTH * 0.4);
        // Player pillar (left side of track)
        commands.spawn((
            Mesh3d(pillar_mesh.clone()),
            MeshMaterial3d(cp_unlit.clone()),
            Transform::from_xyz(pos.x - side.x, 0.5, pos.z - side.z),
            RaceCheckpointPlayer { index: i },
            HillEntity,
        ));
        // Bot pillar (right side of track)
        commands.spawn((
            Mesh3d(pillar_mesh.clone()),
            MeshMaterial3d(cp_unlit.clone()),
            Transform::from_xyz(pos.x + side.x, 0.5, pos.z + side.z),
            RaceCheckpointBot { index: i },
            HillEntity,
        ));
    }

    // --- Start line: white line at WP0, shifted 2 units left (-x) into the race direction ---
    // Perpendicular to -x is along z, so the line runs along z.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.25, 0.05, TRACK_WIDTH))),
        MeshMaterial3d(track_white.clone()),
        Transform::from_xyz(RACE_WAYPOINTS[0].x - 2.0, 0.05, RACE_WAYPOINTS[0].z),
        HillEntity,
    ));

    // --- Finish area: checkerboard 6 meters before start on the closing segment (x=25, z≈22) ---
    {
        let checker_black = materials.add(StandardMaterial {
            base_color: Color::srgb(0.15, 0.15, 0.15),
            ..default()
        });
        let finish_z = RACE_WAYPOINTS[0].z - 7.0; // ~7m before start
        let cell = 0.5_f32;
        let cols = (TRACK_WIDTH / cell) as i32;
        let rows = 5_i32;
        let base_x = RACE_WAYPOINTS[0].x - TRACK_WIDTH / 2.0 + cell / 2.0;
        let base_z = finish_z - (rows as f32 * cell) / 2.0;
        let cell_mesh = meshes.add(Cuboid::new(cell, 0.04, cell));
        // White finish line at leading edge
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(TRACK_WIDTH, 0.05, 0.25))),
            MeshMaterial3d(track_white.clone()),
            Transform::from_xyz(RACE_WAYPOINTS[0].x, 0.05, base_z + rows as f32 * cell + 0.2),
            HillEntity,
        ));
        for r in 0..rows {
            for c in 0..cols {
                let is_white = (r + c) % 2 == 0;
                let mat = if is_white { track_white.clone() } else { checker_black.clone() };
                commands.spawn((
                    Mesh3d(cell_mesh.clone()),
                    MeshMaterial3d(mat),
                    Transform::from_xyz(base_x + c as f32 * cell, 0.05, base_z + r as f32 * cell),
                    HillEntity,
                ));
            }
        }
    }

    // --- White center-line dots along every track segment ---
    {
        let dot_mesh = meshes.add(Cuboid::new(0.25, 0.05, 0.25));
        let dot_spacing = 3.0_f32;
        for i in 0..NUM_WAYPOINTS {
            let a = RACE_WAYPOINTS[i];
            let b = RACE_WAYPOINTS[(i + 1) % NUM_WAYPOINTS];
            let seg_dir = b - a;
            let seg_len = seg_dir.length();
            if seg_len < 0.1 { continue; }
            let seg_norm = seg_dir / seg_len;
            // Start dots 1.5 units from waypoint to avoid cluttering turns
            let mut d = 1.5_f32;
            while d < seg_len - 1.0 {
                let pos = a + seg_norm * d;
                commands.spawn((
                    Mesh3d(dot_mesh.clone()),
                    MeshMaterial3d(track_white.clone()),
                    Transform::from_xyz(pos.x, 0.05, pos.z),
                    HillEntity,
                ));
                d += dot_spacing;
            }
        }
    }

    // Spawn the race bot
    race::spawn_race_bot(&mut commands, &mut meshes, &mut materials);

    // Race state resource
    commands.insert_resource(HillRaceState::default());

    // Race status text (bottom-center, above hint)
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(60.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            HillEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(""),
                TextFont { font_size: 20.0, ..default() },
                TextColor(Color::WHITE),
                RaceStatusText,
                HillEntity,
            ));
        });

    // Race countdown text (large centered overlay)
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        HillEntity,
    )).with_children(|parent| {
        parent.spawn((
            Text::new(""),
            TextFont { font_size: 72.0, ..default() },
            TextColor(Color::srgb(1.0, 1.0, 0.0)),
            RaceCountdownText,
            HillEntity,
        ));
    });

    // --- Power-up progress bar UI (top-right) ---
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(12.0),
                top: Val::Px(12.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
            PowerUpBarContainer,
            HillEntity,
        ))
        .with_children(|parent| {
            let bar_defs: [(AppleKind, Color, &str); 3] = [
                (AppleKind::Speed, Color::srgb(0.2, 0.9, 0.2), "Speed"),
                (AppleKind::Jump, Color::srgb(0.9, 0.15, 0.15), "Jump"),
                (AppleKind::Backwards, Color::srgb(0.7, 0.2, 0.9), "Reverse"),
            ];
            for (kind, color, label) in bar_defs {
                parent
                    .spawn((
                        Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(6.0),
                            display: Display::None,
                            ..default()
                        },
                        PowerUpBarBg { kind },
                        HillEntity,
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Node {
                                min_width: Val::Px(55.0),
                                ..default()
                            },
                            Text::new(label),
                            TextFont { font_size: 13.0, ..default() },
                            TextColor(color),
                        ));
                        row.spawn((
                            Node {
                                width: Val::Px(200.0),
                                height: Val::Px(12.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
                        ))
                        .with_children(|bg| {
                            bg.spawn((
                                Node {
                                    width: Val::Px(200.0),
                                    height: Val::Px(12.0),
                                    ..default()
                                },
                                BackgroundColor(color),
                                PowerUpBar { kind },
                            ));
                        });
                    });
            }
        });

    // Gate hint sign
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(40.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        HillEntity,
    )).with_children(|parent| {
        parent.spawn((
            Text::new("Hint: HillState.gate_locked / HillState.slide_friction"),
            TextFont { font_size: 13.0, ..default() },
            TextColor(Color::srgb(0.6, 0.6, 0.6)),
            HillEntity,
        ));
    });
}

pub(super) fn handle_victory(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_screen: ResMut<NextState<Screen>>,
    overlay_q: Query<Entity, With<shared_ui::OverlayScreen>>,
) {
    if overlay_q.is_empty() {
        shared_ui::spawn_victory_overlay(
            &mut commands,
            "SUMMIT REACHED!",
            Some("You conquered the Hill Fortress!"),
            22.0,
            "Press ENTER to return to menu",
            HillEntity,
        );
    }

    if keyboard.just_pressed(KeyCode::Enter) {
        next_screen.set(Screen::Menu);
    }
}

pub(super) fn cleanup_hill(
    mut commands: Commands,
    query: Query<Entity, With<HillEntity>>,
) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
    commands.remove_resource::<HillState>();
    commands.remove_resource::<GroundYOverride>();
    commands.remove_resource::<ActivePowerUps>();
    commands.remove_resource::<PowerUpState>();
    commands.remove_resource::<AppleAssets>();
    commands.remove_resource::<AppleRng>();
    commands.remove_resource::<MazeCompleted>();
    commands.remove_resource::<MazeNeedsRebuild>();
    commands.remove_resource::<ColorCubeState>();
    commands.remove_resource::<HillRaceState>();
    commands.remove_resource::<CheckpointMaterials>();
}
