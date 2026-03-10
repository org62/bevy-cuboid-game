use bevy::prelude::*;

/// Spawn a single axis-aligned wall cuboid and return the entity.
pub fn spawn_wall_rect(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    material: &Handle<StandardMaterial>,
    x_min: f32,
    x_max: f32,
    z_min: f32,
    z_max: f32,
    height: f32,
) -> Entity {
    let w = x_max - x_min;
    let d = z_max - z_min;
    commands
        .spawn((
            Mesh3d(meshes.add(Cuboid::new(w, height, d))),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(
                (x_min + x_max) / 2.0,
                height / 2.0,
                (z_min + z_max) / 2.0,
            ),
        ))
        .id()
}

/// Spawn a complete grid-based maze including boundary walls, interior walls,
/// and corner posts. All pieces tile with zero overlap and zero gaps.
///
/// Boundary walls (h_walls rows 0/7, v_walls cols 0/7) are included and
/// centered on the grid edge, extending half-thickness outward.
///
/// `extra` closure is called on every spawned entity to add markers/collision.
pub fn spawn_maze_grid_walls(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    material: &Handle<StandardMaterial>,
    origin: Vec2,
    cell_size: f32,
    wall_thickness: f32,
    wall_height: f32,
    h_walls: &[[bool; 7]; 8],
    v_walls: &[[bool; 8]; 7],
    mut extra: impl FnMut(&mut Commands, Entity, Vec2, Vec2),
) {
    let ht = wall_thickness / 2.0;

    // H-walls: all 8 rows (0=north boundary, 7=south boundary, 1-6=interior)
    // Each wall is inset by ht on each end in X; corner posts fill the gaps.
    for r in 0..8 {
        for c in 0..7_usize {
            if h_walls[r][c] {
                let x_min = origin.x + c as f32 * cell_size + ht;
                let x_max = origin.x + (c + 1) as f32 * cell_size - ht;
                let z_ctr = origin.y + r as f32 * cell_size;
                let zmin = z_ctr - ht;
                let zmax = z_ctr + ht;
                let e = spawn_wall_rect(commands, meshes, material, x_min, x_max, zmin, zmax, wall_height);
                extra(commands, e, Vec2::new(x_min, zmin), Vec2::new(x_max, zmax));
            }
        }
    }

    // V-walls: all 8 columns (0=west boundary, 7=east boundary, 1-6=interior)
    // Each wall is inset by ht on each end in Z; corner posts fill the gaps.
    for r in 0..7_usize {
        for c in 0..8 {
            if v_walls[r][c] {
                let x_ctr = origin.x + c as f32 * cell_size;
                let xmin = x_ctr - ht;
                let xmax = x_ctr + ht;
                let z_min = origin.y + r as f32 * cell_size + ht;
                let z_max = origin.y + (r + 1) as f32 * cell_size - ht;
                let e = spawn_wall_rect(commands, meshes, material, xmin, xmax, z_min, z_max, wall_height);
                extra(commands, e, Vec2::new(xmin, z_min), Vec2::new(xmax, z_max));
            }
        }
    }

    // Corner posts at every grid vertex where any adjacent wall exists.
    // Vertices range from (0,0) to (7,7) — includes boundary corners.
    for r in 0..8_usize {
        for c in 0..8_usize {
            let has_h_west = c > 0 && h_walls[r][c - 1];
            let has_h_east = c < 7 && h_walls[r][c];
            let has_v_north = r > 0 && v_walls[r - 1][c];
            let has_v_south = r < 7 && v_walls[r][c];

            if has_h_west || has_h_east || has_v_north || has_v_south {
                let cx = origin.x + c as f32 * cell_size;
                let cz = origin.y + r as f32 * cell_size;
                let xmin = cx - ht;
                let xmax = cx + ht;
                let zmin = cz - ht;
                let zmax = cz + ht;
                let e = spawn_wall_rect(commands, meshes, material, xmin, xmax, zmin, zmax, wall_height);
                extra(commands, e, Vec2::new(xmin, zmin), Vec2::new(xmax, zmax));
            }
        }
    }
}
