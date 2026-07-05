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

use crate::player::{animate_player, escape_to_menu, player_movement, toggle_pause, PlayerMovementSet};
use crate::shared_ui;
use crate::terrain::terrain_collision;
use crate::{HillPhase, Screen};

#[cfg(feature = "test_bot")]
pub use resources::HillState;

use maze::*;
use powerups::*;
use race::*;
use setup::*;
use terrain::*;

pub struct Level101Plugin;

impl Plugin for Level101Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::HillChallenge), setup_hill)
            .add_systems(
                Update,
                (
                    shared_ui::update_camera_orbit.before(PlayerMovementSet),
                    player_movement.in_set(PlayerMovementSet),
                    terrain_collision,
                    summit_victory_check,
                )
                    .chain()
                    .run_if(in_state(HillPhase::Playing)),
            )
            .add_systems(
                Update,
                (
                    animate_player,
                    slide_force_system,
                    water_slide_system,
                    apple_collection_system,
                    power_up_timer_system,
                    power_up_bar_ui_system,
                    apple_bob_system,
                    maze_exit_check_system,
                    suck_up_animation_system,
                    teleporter_system,
                    maze_rebuild_system,
                    color_cube_system,
                    zip_line_trigger_system,
                    zip_line_ride_system,
                )
                    .after(PlayerMovementSet)
                    .run_if(in_state(Screen::HillChallenge)),
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
                    .after(PlayerMovementSet)
                    .run_if(in_state(Screen::HillChallenge)),
            )
            .add_systems(
                Update,
                (escape_to_menu, toggle_pause).run_if(in_state(HillPhase::Playing)),
            )
            .add_systems(
                Update,
                shared_ui::follow_camera_system
                    .after(PlayerMovementSet)
                    .after(suck_up_animation_system)
                    .after(zip_line_ride_system)
                    .run_if(in_state(Screen::HillChallenge)),
            )
            .add_systems(
                Update,
                handle_victory.run_if(in_state(HillPhase::Victory)),
            )
            .add_systems(OnExit(Screen::HillChallenge), cleanup_hill);
    }
}
