mod components;
mod constants;
mod debugger;
mod maze;
mod powerups;
mod race;
mod resources;
mod setup;
mod terrain;

use bevy::prelude::*;

use crate::level101::components::HillEntity;
use crate::level_kit::{self, GameplaySet, LevelPhase};
use crate::Screen;

pub const ID: u32 = 101;
const SCREEN: Screen = Screen::Level(ID);

#[cfg(feature = "test_bot")]
pub use resources::HillState;

use maze::*;
use powerups::*;
use race::*;
use setup::*;
use terrain::*;

pub fn register(app: &mut App) {
    app.add_systems(OnEnter(SCREEN), setup_hill)
        .add_systems(
            Update,
            summit_victory_check
                .in_set(GameplaySet::Logic)
                .run_if(level_kit::in_phase(SCREEN, LevelPhase::Playing)),
        )
            // Scripted motion: each of these overrides the collision-resolved
            // player position, so they all have to land before the camera reads
            // it (this is what the old hand-written `.after(...)` chain on
            // `follow_camera_system` was doing).
            .add_systems(
                Update,
                (
                    slide_force_system,
                    water_slide_system,
                    suck_up_animation_system,
                    teleporter_system,
                    zip_line_trigger_system,
                    zip_line_ride_system,
                )
                    .in_set(GameplaySet::Scripted)
                    .run_if(in_state(SCREEN)),
            )
            .add_systems(
                Update,
                (
                    apple_collection_system,
                    power_up_timer_system,
                    power_up_bar_ui_system,
                    apple_bob_system,
                    maze_exit_check_system,
                    maze_rebuild_system,
                    color_cube_system,
                )
                    .in_set(GameplaySet::Logic)
                    .run_if(in_state(SCREEN)),
            )
            .add_systems(
                Update,
                (
                    race_trigger_system,
                    race_countdown_system,
                    race_bot_movement_system,
                    race_player_tracking_system,
                    race_checkpoint_light_system,
                    race_result_system,
                )
                    .chain()
                    .in_set(GameplaySet::Logic)
                    .run_if(in_state(SCREEN)),
            )
            .add_systems(
            OnExit(SCREEN),
            (level_kit::despawn_level::<HillEntity>, cleanup_hill),
        );
}
