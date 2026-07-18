mod level1;
mod level2;
mod level3;
mod level4;
mod level5;
mod level101;
mod level102;
mod level103;
mod menu;
mod password;
mod player;
pub mod shared_ui;
pub mod terrain;
pub mod walls;
#[cfg(feature = "test_bot")]
mod test_bot;

use bevy::prelude::*;
use bevy::window::PresentMode;

// --- Screens ---

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Screen {
    #[default]
    Menu,
    PasswordChallenge,
    CannonChallenge,
    CountdownChallenge,
    MazeChallenge,
    RaceChallenge,
    HillChallenge,
    MeadowChallenge,
    WaterparkChallenge,
}

// --- Challenge phases (sub-state of PasswordChallenge) ---

#[derive(SubStates, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[source(Screen = Screen::PasswordChallenge)]
pub enum ChallengePhase {
    #[default]
    Exploring,
    PasswordPrompt,
    AccessGranted,
    WrongPassword,
}

// --- Cannon phases (sub-state of CannonChallenge) ---

#[derive(SubStates, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[source(Screen = Screen::CannonChallenge)]
pub enum CannonPhase {
    #[default]
    Playing,
    Victory,
    Dead,
}

// --- Countdown phases (Level 3) ---

#[derive(SubStates, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[source(Screen = Screen::CountdownChallenge)]
pub enum CountdownPhase {
    #[default]
    Playing,
    Victory,
    Exploded,
}

// --- Maze phases (Level 4) ---

#[derive(SubStates, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[source(Screen = Screen::MazeChallenge)]
pub enum MazePhase {
    #[default]
    Playing,
    Victory,
}

// --- Race phases (Level 5) ---

#[derive(SubStates, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[source(Screen = Screen::RaceChallenge)]
pub enum RacePhase {
    #[default]
    Countdown,
    Playing,
    Victory,
    Lost,
}

// --- Hill phases (Level 13) ---

#[derive(SubStates, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[source(Screen = Screen::HillChallenge)]
pub enum HillPhase {
    #[default]
    Playing,
    Victory,
}

// --- Meadow phases (Level 14) ---

#[derive(SubStates, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[source(Screen = Screen::MeadowChallenge)]
pub enum MeadowPhase {
    #[default]
    Playing,
    Victory,
}

// --- Waterpark phases (Level 15) ---

#[derive(SubStates, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[source(Screen = Screen::WaterparkChallenge)]
pub enum WaterparkPhase {
    #[default]
    Playing,
    Victory,
}

// --- Pause ---

#[derive(Resource, Default)]
pub struct GamePaused(pub bool);

// --- Scoreboard ---

#[derive(Resource, Default)]
pub struct Scoreboard {
    solved: std::collections::HashSet<u32>,
}

impl Scoreboard {
    pub fn total_solved(&self) -> u32 {
        self.solved.len() as u32
    }

    pub fn total_challenges(&self) -> u32 {
        // 5 regular levels + 3 hidden easter-egg levels.
        8
    }

    pub fn is_solved(&self, level: u32) -> bool {
        self.solved.contains(&level)
    }

    pub fn set_solved(&mut self, level: u32) {
        self.solved.insert(level);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoreboard_starts_empty() {
        let sb = Scoreboard::default();
        assert_eq!(sb.total_solved(), 0);
        assert_eq!(sb.total_challenges(), 8);
    }

    #[test]
    fn scoreboard_counts_correctly() {
        let mut sb = Scoreboard::default();
        sb.set_solved(1);
        sb.set_solved(2);
        sb.set_solved(3);
        assert_eq!(sb.total_solved(), 3);
    }

    #[test]
    fn scoreboard_all_solved() {
        let mut sb = Scoreboard::default();
        for level in [1, 2, 3, 4, 5, 101, 102, 103] {
            sb.set_solved(level);
        }
        assert_eq!(sb.total_solved(), 8);
        assert_eq!(sb.total_solved(), sb.total_challenges());
    }

    #[test]
    fn is_solved_maps_correctly() {
        let mut sb = Scoreboard::default();
        sb.set_solved(4);
        assert!(sb.is_solved(4));
        assert!(!sb.is_solved(3));
        assert!(!sb.is_solved(0)); // invalid level
        assert!(!sb.is_solved(16)); // invalid level
    }

    #[test]
    fn active_levels_have_screens() {
        let screens = [
            Screen::Menu,
            Screen::PasswordChallenge,
            Screen::CannonChallenge,
            Screen::CountdownChallenge,
            Screen::MazeChallenge,
            Screen::RaceChallenge,
            Screen::HillChallenge,
            Screen::MeadowChallenge,
            Screen::WaterparkChallenge,
        ];
        assert_eq!(screens.len(), 9); // 8 levels + menu
    }
}

fn reset_pause(
    mut game_paused: ResMut<GamePaused>,
    mut camera_orbit: ResMut<shared_ui::CameraOrbit>,
    mut commands: Commands,
    overlay_q: Query<Entity, With<player::PauseOverlay>>,
) {
    game_paused.0 = false;
    camera_orbit.yaw = 0.0;
    camera_orbit.pitch = 0.0;
    camera_orbit.zoom = 1.0;
    for entity in &overlay_q {
        commands.entity(entity).despawn_recursive();
    }
}

// --- App entry ---

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Debugger Challenges".to_string(),
                resolution: (800.0, 500.0).into(),
                present_mode: PresentMode::Fifo,
                ..default()
            }),
            ..default()
        }))
        .init_state::<Screen>()
        .add_sub_state::<ChallengePhase>()
        .add_sub_state::<CannonPhase>()
        .add_sub_state::<CountdownPhase>()
        .add_sub_state::<MazePhase>()
        .add_sub_state::<RacePhase>()
        .add_sub_state::<HillPhase>()
        .add_sub_state::<MeadowPhase>()
        .add_sub_state::<WaterparkPhase>()
        .init_resource::<Scoreboard>()
        .init_resource::<GamePaused>()
        .init_resource::<shared_ui::CameraOrbit>()
        .init_resource::<shared_ui::ActiveInput>()
        .init_resource::<shared_ui::MouseSettings>()
        .init_resource::<shared_ui::DiagState>()
        .add_systems(OnEnter(Screen::Menu), reset_pause)
        .add_systems(
            Update,
            (
                shared_ui::detect_active_input,
                shared_ui::update_controls_hint,
                shared_ui::manage_cursor_grab,
                shared_ui::update_objective_banner,
                shared_ui::diag_hotkeys,
                shared_ui::diag_overlay_update,
            ),
        )
        // Controls (C) / Settings (E) dialogs, everywhere except while typing
        // the Level 1 password (where E is text input).
        .add_systems(
            Update,
            shared_ui::agenda_controls.run_if(not(in_state(ChallengePhase::PasswordPrompt))),
        )
        .add_plugins((
            menu::MenuPlugin,
            level1::Level1Plugin,
            password::PasswordPlugin,
            level2::Level2Plugin,
            level3::Level3Plugin,
            level4::Level4Plugin,
            level5::Level5Plugin,
            level101::Level101Plugin,
            level102::Level102Plugin,
            level103::Level103Plugin,
        ));
    #[cfg(feature = "test_bot")]
    app.add_plugins(test_bot::TestBotPlugin);
    app.run();
}
