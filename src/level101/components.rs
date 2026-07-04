use bevy::prelude::*;

#[derive(Component, Clone, Copy)]
pub(super) struct HillEntity;

#[derive(Component)]
pub(super) struct HudText;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum AppleKind {
    Speed,
    Jump,
    Backwards,
}

#[derive(Component)]
pub(super) struct PowerUpApple {
    pub kind: AppleKind,
}

#[derive(Component)]
pub(super) struct PowerUpBarContainer;

#[derive(Component)]
pub(super) struct PowerUpBar {
    pub kind: AppleKind,
}

#[derive(Component)]
pub(super) struct PowerUpBarBg {
    pub kind: AppleKind,
}

/// Marks a terrain surface for collision.
/// min/max define the XZ bounds, y is the surface height.
#[derive(Component)]
pub(super) struct TerrainSurface {
    pub min: Vec2,
    pub max: Vec2,
    pub y: f32,
}

/// Solid wall collision box - blocks horizontal movement.
/// min/max are XZ bounds, y_min/y_max are vertical bounds.
#[derive(Component)]
pub(super) struct SolidBlock {
    pub min: Vec2,
    pub max: Vec2,
    pub y_min: f32,
    pub y_max: f32,
}

/// Marks a slide segment for the slide force system.
#[derive(Component)]
pub(super) struct SlideSegment {
    pub min: Vec2,
    pub max: Vec2,
    pub y: f32,
}

/// Marks a water slide segment that auto-carries the player toward the pool (-x direction).
#[derive(Component)]
pub(super) struct WaterSlideSegment {
    pub min: Vec2,
    pub max: Vec2,
    pub y: f32,
}

#[derive(Component)]
pub(super) struct MazeExitZone {
    pub min: Vec2,
    pub max: Vec2,
}

#[derive(Component)]
pub(super) struct SuckUpAnimation {
    pub start_pos: Vec3,
    pub end_pos: Vec3,
    pub elapsed: f32,
    pub duration: f32,
}

#[derive(Component)]
pub(super) struct MazeInteriorWall;

#[derive(Component)]
pub(super) struct TeleporterPad {
    pub destination: Vec3,
}

#[derive(Component)]
pub(super) struct ColorCube;

#[derive(Component)]
pub(super) struct RaceBot {
    pub progress: f32,
    pub speed: f32,
}

/// Player half of a checkpoint pillar.
#[derive(Component)]
pub(super) struct RaceCheckpointPlayer {
    pub index: usize,
}

/// Bot half of a checkpoint pillar.
#[derive(Component)]
pub(super) struct RaceCheckpointBot {
    pub index: usize,
}

#[derive(Component)]
pub(super) struct RaceStatusText;

#[derive(Component)]
pub(super) struct RaceCountdownText;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) enum HillRacePhase {
    #[default]
    Idle,
    Countdown,
    Racing,
    Won,
    Lost,
}

#[derive(Component)]
pub(super) struct ZipLineRide {
    pub start: Vec3,
    pub end: Vec3,
    pub elapsed: f32,
    pub duration: f32,
}

#[derive(Component)]
pub(super) struct ZipLinePlatform;
