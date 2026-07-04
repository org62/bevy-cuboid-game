use bevy::prelude::*;

use super::components::{AppleKind, HillRacePhase};
use super::constants::NUM_CHECKPOINTS;

#[repr(C)]
#[derive(Resource)]
pub struct HillState {
    pub gate_locked: bool,
    pub slide_friction: f32,
    pub _padding: [u8; 4],
}

impl Default for HillState {
    fn default() -> Self {
        Self {
            gate_locked: true,
            slide_friction: -15.0,
            _padding: [0; 4],
        }
    }
}

#[derive(Resource, Default)]
pub(super) struct ActivePowerUps {
    pub speed_timer: f32,
    pub jump_timer: f32,
    pub backwards_timer: f32,
    /// Respawn cooldowns per kind: (kind, remaining_secs)
    pub respawn_timers: Vec<(AppleKind, f32)>,
}

/// Simple pseudo-random number generator (xorshift64).
#[derive(Resource)]
pub(super) struct AppleRng {
    state: u64,
}

impl AppleRng {
    pub fn new() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        Self { state: seed | 1 }
    }

    pub fn next_f32(&mut self) -> f32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        (self.state as u32 as f32) / (u32::MAX as f32)
    }

    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }
}

#[derive(Resource)]
pub(super) struct AppleAssets {
    pub sphere: Handle<Mesh>,
    pub stem: Handle<Mesh>,
    pub green: Handle<StandardMaterial>,
    pub red: Handle<StandardMaterial>,
    pub purple: Handle<StandardMaterial>,
    pub stem_mat: Handle<StandardMaterial>,
}

#[derive(Resource)]
pub(super) struct MazeCompleted;

#[derive(Resource)]
pub(super) struct MazeNeedsRebuild;

#[derive(Resource)]
pub(super) struct HillRaceState {
    pub phase: HillRacePhase,
    /// Which checkpoints the player has individually reached (any order).
    pub player_checkpoints: [bool; NUM_CHECKPOINTS],
    /// Which checkpoints the bot has passed.
    pub bot_checkpoints: [bool; NUM_CHECKPOINTS],
    pub result_timer: f32,
    pub countdown_timer: f32,
}

impl Default for HillRaceState {
    fn default() -> Self {
        Self {
            phase: HillRacePhase::Idle,
            player_checkpoints: [false; NUM_CHECKPOINTS],
            bot_checkpoints: [false; NUM_CHECKPOINTS],
            result_timer: 0.0,
            countdown_timer: 0.0,
        }
    }
}

#[derive(Resource)]
pub(super) struct CheckpointMaterials {
    pub unlit: Handle<StandardMaterial>,
    pub player_lit: Handle<StandardMaterial>,
    pub bot_lit: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
pub(super) struct ColorCubeState {
    pub player_inside: bool,
    pub last_color: Option<Handle<StandardMaterial>>,
}
