use bevy::prelude::*;

pub(super) const PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 0.0, 25.0);
pub(super) const CAM_OFFSET: Vec3 = Vec3::new(0.0, 15.0, 15.0);

/// Clockwise race loop that skirts the pool without crossing it.
pub(super) const RACE_WAYPOINTS: [Vec3; 14] = [
    Vec3::new( 25.0, 0.0,  27.0),  // WP0  — START far right south
    Vec3::new(-11.0, 0.0,  27.0),  // WP1  — west along south edge
    Vec3::new(-11.0, 0.0,  18.0),  // WP2  — turn north
    Vec3::new(-28.0, 0.0,  18.0),  // WP3  — continue west
    Vec3::new(-28.0, 0.0,   6.0),  // WP4  — south along west wall (near pool)
    Vec3::new(-11.0, 0.0,   6.0),  // WP5  — east past pool south side
    Vec3::new(-11.0, 0.0,  -6.0),  // WP6  — south past pool east side
    Vec3::new(-28.0, 0.0,  -6.0),  // WP7  — west
    Vec3::new(-28.0, 0.0, -22.0),  // WP8  — south along west wall
    Vec3::new( -5.0, 0.0, -22.0),  // WP9  — east across north area
    Vec3::new( -5.0, 0.0, -12.0),  // WP10 — south
    Vec3::new( 11.0, 0.0, -12.0),  // WP11 — east (skirts maze edge)
    Vec3::new( 11.0, 0.0,  10.0),  // WP12 — south along x=11 (alongside hill)
    Vec3::new( 25.0, 0.0,  10.0),  // WP13 — east to return corridor
];

pub(super) const NUM_WAYPOINTS: usize = RACE_WAYPOINTS.len();

/// Checkpoint at every turn: WP1 through WP13.
pub(super) const RACE_CHECKPOINT_INDICES: [usize; 13] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];
pub(super) const NUM_CHECKPOINTS: usize = RACE_CHECKPOINT_INDICES.len();
pub(super) const CHECKPOINT_RADIUS: f32 = 2.0;

pub(super) const RACE_BOT_SPEED_FACTOR: f32 = 0.92;
pub(super) const TRACK_WIDTH: f32 = 3.0;
pub(super) const START_ZONE_RADIUS: f32 = 2.5;
pub(super) const PICKUP_RADIUS: f32 = 1.5;
