use bevy::prelude::*;

use crate::player::{Player, PlayerBody, PlayerHead, PlayerPhysics};
use crate::walls::spawn_maze_grid_walls;

use super::components::*;
use super::constants::PICKUP_RADIUS;
use super::resources::*;

/// Recursive backtracker maze generation on a 7x7 grid.
/// Returns (h_walls, v_walls) where:
///   h_walls[r][c] = horizontal wall between row r-1 and row r at column c (r=0..8, c=0..7)
///   v_walls[r][c] = vertical wall between col c-1 and col c at row r (r=0..7, c=0..8)
fn generate_maze_grid(rng: &mut AppleRng) -> ([[bool; 7]; 8], [[bool; 8]; 7]) {
    let mut h_walls = [[true; 7]; 8];
    let mut v_walls = [[true; 8]; 7];
    let mut visited = [[false; 7]; 7];

    // Start from cell (6, 3) — south-center, near entrance
    let mut stack: Vec<(usize, usize)> = Vec::new();
    visited[6][3] = true;
    stack.push((6, 3));

    while let Some(&(r, c)) = stack.last() {
        // Collect unvisited neighbors
        let mut neighbors = Vec::new();
        if r > 0 && !visited[r - 1][c] {
            neighbors.push((r - 1, c, 0)); // north
        }
        if r < 6 && !visited[r + 1][c] {
            neighbors.push((r + 1, c, 1)); // south
        }
        if c > 0 && !visited[r][c - 1] {
            neighbors.push((r, c - 1, 2)); // west
        }
        if c < 6 && !visited[r][c + 1] {
            neighbors.push((r, c + 1, 3)); // east
        }

        if neighbors.is_empty() {
            stack.pop();
        } else {
            let idx = (rng.next_f32() * neighbors.len() as f32) as usize;
            let idx = idx.min(neighbors.len() - 1);
            let (nr, nc, dir) = neighbors[idx];
            // Carve wall between current and neighbor
            match dir {
                0 => h_walls[r][c] = false,     // north wall of current cell
                1 => h_walls[r + 1][c] = false,  // south wall of current cell
                2 => v_walls[r][c] = false,       // west wall of current cell
                3 => v_walls[r][c + 1] = false,   // east wall of current cell
                _ => unreachable!(),
            }
            visited[nr][nc] = true;
            stack.push((nr, nc));
        }
    }

    (h_walls, v_walls)
}

pub(super) fn spawn_maze_interior(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    rng: &mut AppleRng,
) {
    let stone = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.5, 0.5),
        ..default()
    });
    let wall_h = 2.5;

    let (mut h_walls, v_walls) = generate_maze_grid(rng);

    // Carve entrance gap in south boundary (row 7) at column 3 (x=20..22)
    h_walls[7][3] = false;

    // Spawn ALL walls: boundary + interior + corner posts (zero overlap, zero gaps)
    spawn_maze_grid_walls(
        commands,
        meshes,
        &stone,
        Vec2::new(14.0, -28.0),
        2.0,
        0.4,
        wall_h,
        &h_walls,
        &v_walls,
        |cmds, e, min, max| {
            cmds.entity(e).insert((
                SolidBlock {
                    min,
                    max,
                    y_min: 0.0,
                    y_max: wall_h,
                },
                HillEntity,
                MazeInteriorWall,
            ));
        },
    );

    // Maze exit trigger zone at cell (0,6): NE corner
    commands.spawn((
        MazeExitZone {
            min: Vec2::new(26.0, -28.0),
            max: Vec2::new(28.0, -26.0),
        },
        HillEntity,
        MazeInteriorWall,
    ));

    // Exit glow indicator: green emissive floor tile
    let exit_glow = materials.add(StandardMaterial {
        base_color: Color::srgb(0.0, 0.8, 0.0),
        emissive: LinearRgba::new(0.0, 2.0, 0.0, 1.0),
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0, 0.05, 2.0))),
        MeshMaterial3d(exit_glow),
        Transform::from_xyz(27.0, 0.025, -27.0),
        HillEntity,
        MazeInteriorWall,
    ));

    // Color cube at maze exit
    let cube_color = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.9, 0.9),
        emissive: LinearRgba::new(0.5, 0.5, 0.5, 1.0),
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.5, 1.0))),
        MeshMaterial3d(cube_color),
        Transform::from_xyz(27.0, 0.75, -27.0),
        ColorCube,
        HillEntity,
        MazeInteriorWall,
    ));
}

pub(super) fn maze_exit_check_system(
    mut commands: Commands,
    player_q: Query<(Entity, &Transform), With<Player>>,
    exit_zones: Query<&MazeExitZone>,
    maze_completed: Option<Res<MazeCompleted>>,
) {
    if maze_completed.is_some() {
        return;
    }
    let Ok((player_entity, player_tf)) = player_q.get_single() else { return };
    let px = player_tf.translation.x;
    let pz = player_tf.translation.z;

    for zone in &exit_zones {
        if px >= zone.min.x && px <= zone.max.x && pz >= zone.min.y && pz <= zone.max.y {
            commands.insert_resource(MazeCompleted);
            commands.entity(player_entity).insert(SuckUpAnimation {
                start_pos: player_tf.translation,
                end_pos: Vec3::new(21.0, 0.5, -10.0),
                elapsed: 0.0,
                duration: 2.0,
            });
            break;
        }
    }
}

pub(super) fn teleporter_system(
    mut player_q: Query<(&mut Transform, &mut PlayerPhysics), (With<Player>, Without<SuckUpAnimation>)>,
    pads: Query<(&Transform, &TeleporterPad), Without<Player>>,
) {
    let Ok((mut player_tf, mut physics)) = player_q.get_single_mut() else { return };
    let pp = player_tf.translation;

    for (pad_tf, pad) in &pads {
        let center = pad_tf.translation;
        let dx = pp.x - center.x;
        let dz = pp.z - center.z;
        let dy = (pp.y - center.y).abs();
        if dx * dx + dz * dz < PICKUP_RADIUS * PICKUP_RADIUS && dy < 1.0 {
            player_tf.translation = pad.destination;
            physics.velocity = Vec3::ZERO;
            physics.grounded = false;
            return;
        }
    }
}

pub(super) fn suck_up_animation_system(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Transform, &mut PlayerPhysics, &mut SuckUpAnimation, &Children), With<Player>>,
    cube_state: Option<Res<ColorCubeState>>,
    body_q: Query<Entity, With<PlayerBody>>,
    head_q: Query<Entity, With<PlayerHead>>,
) {
    for (entity, mut transform, mut physics, mut anim, children) in &mut query {
        anim.elapsed += time.delta_secs();
        let t = (anim.elapsed / anim.duration).clamp(0.0, 1.0);

        // smoothstep
        let t_smooth = t * t * (3.0 - 2.0 * t);

        let start = anim.start_pos;
        let end = anim.end_pos;
        let x = start.x + (end.x - start.x) * t_smooth;
        let z = start.z + (end.z - start.z) * t_smooth;
        let y = start.y + (end.y - start.y) * t_smooth + 4.0 * t * (1.0 - t) * 3.0;

        transform.translation = Vec3::new(x, y, z);
        physics.velocity = Vec3::ZERO;
        physics.grounded = false;

        if t >= 1.0 {
            transform.translation = end;
            physics.grounded = true;
            commands.entity(entity).remove::<SuckUpAnimation>();
            commands.remove_resource::<MazeCompleted>();
            commands.insert_resource(MazeNeedsRebuild);

            // Apply color cube color to player body and head
            if let Some(ref state) = cube_state {
                if let Some(ref color_mat) = state.last_color {
                    for &child in children.iter() {
                        if body_q.get(child).is_ok() || head_q.get(child).is_ok() {
                            commands.entity(child).insert(MeshMaterial3d(color_mat.clone()));
                        }
                    }
                }
            }
        }
    }
}

pub(super) fn maze_rebuild_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    rebuild: Option<Res<MazeNeedsRebuild>>,
    maze_walls: Query<Entity, With<MazeInteriorWall>>,
    mut rng: ResMut<AppleRng>,
) {
    if rebuild.is_none() {
        return;
    }
    commands.remove_resource::<MazeNeedsRebuild>();
    for entity in &maze_walls {
        commands.entity(entity).despawn_recursive();
    }
    spawn_maze_interior(&mut commands, &mut meshes, &mut materials, &mut rng);
}

pub(super) fn color_cube_system(
    mut commands: Commands,
    player_q: Query<&Transform, With<Player>>,
    cube_q: Query<(Entity, &Transform), (With<ColorCube>, Without<Player>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<AppleRng>,
    mut state: ResMut<ColorCubeState>,
) {
    let Ok(player_tf) = player_q.get_single() else { return };
    let pp = player_tf.translation;

    let mut near_any = false;
    for (entity, cube_tf) in &cube_q {
        let dist = pp.distance(cube_tf.translation);
        if dist < PICKUP_RADIUS {
            near_any = true;
            if !state.player_inside {
                // Pick a random vibrant color
                let colors: [(f32, f32, f32); 8] = [
                    (1.0, 0.1, 0.1),   // red
                    (0.1, 0.4, 1.0),   // blue
                    (0.1, 0.9, 0.2),   // green
                    (1.0, 0.9, 0.1),   // yellow
                    (0.0, 0.9, 0.9),   // cyan
                    (1.0, 0.1, 0.9),   // magenta
                    (1.0, 0.5, 0.0),   // orange
                    (0.6, 0.1, 1.0),   // purple
                ];
                let idx = (rng.next_f32() * colors.len() as f32) as usize;
                let idx = idx.min(colors.len() - 1);
                let (r, g, b) = colors[idx];
                let mat = materials.add(StandardMaterial {
                    base_color: Color::srgb(r, g, b),
                    emissive: LinearRgba::new(r * 1.5, g * 1.5, b * 1.5, 1.0),
                    ..default()
                });
                commands.entity(entity).insert(MeshMaterial3d(mat.clone()));
                state.last_color = Some(mat);
                state.player_inside = true;
            }
        }
    }
    if !near_any {
        state.player_inside = false;
    }
}

pub(super) fn zip_line_trigger_system(
    mut commands: Commands,
    player_q: Query<(Entity, &Transform), (With<Player>, Without<ZipLineRide>, Without<SuckUpAnimation>)>,
    platform_q: Query<&Transform, (With<ZipLinePlatform>, Without<Player>)>,
) {
    let Ok((player_entity, player_tf)) = player_q.get_single() else { return };
    let pp = player_tf.translation;

    for plat_tf in &platform_q {
        let center = plat_tf.translation;
        let dx = pp.x - center.x;
        let dz = pp.z - center.z;
        let dy = (pp.y - center.y).abs();
        if dx * dx + dz * dz < PICKUP_RADIUS * PICKUP_RADIUS && dy < 1.5 {
            commands.entity(player_entity).insert(ZipLineRide {
                start: Vec3::new(0.0, 10.5, -2.5),
                end: Vec3::new(21.0, 0.5, -13.0),
                elapsed: 0.0,
                duration: 2.5,
            });
            return;
        }
    }
}

pub(super) fn zip_line_ride_system(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Transform, &mut PlayerPhysics, &mut ZipLineRide), With<Player>>,
) {
    for (entity, mut transform, mut physics, mut ride) in &mut query {
        ride.elapsed += time.delta_secs();
        let t = (ride.elapsed / ride.duration).clamp(0.0, 1.0);

        // smoothstep
        let t_smooth = t * t * (3.0 - 2.0 * t);

        let start = ride.start;
        let end = ride.end;
        let x = start.x + (end.x - start.x) * t_smooth;
        let z = start.z + (end.z - start.z) * t_smooth;
        // Downward arc: linear lerp with a slight upward arc that goes negative (sag)
        let y_base = start.y + (end.y - start.y) * t_smooth;
        let arc = -2.0 * t * (1.0 - t); // slight downward sag
        let y = y_base + arc;

        transform.translation = Vec3::new(x, y, z);
        physics.velocity = Vec3::ZERO;
        physics.grounded = false;

        if t >= 1.0 {
            transform.translation = end;
            physics.grounded = true;
            commands.entity(entity).remove::<ZipLineRide>();
        }
    }
}
