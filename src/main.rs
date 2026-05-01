mod level1;
mod level2;
mod level3;
mod level4;
mod level5;
mod level6;
mod level7;
mod level8;
mod level9;
mod level10;
mod level11;
mod level12;
mod level13;
mod level14;
mod level15;
mod menu;
mod password;
mod player;
pub mod shared_ui;
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
    ChestChallenge,
    GravityChallenge,
    TollChallenge,
    ArenaChallenge,
    LootChallenge,
    CloneChallenge,
    FinalChallenge,
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

// --- Chest phases (Level 6) ---

#[derive(SubStates, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[source(Screen = Screen::ChestChallenge)]
pub enum ChestPhase {
    #[default]
    Playing,
    Victory,
}

// --- Gravity phases (Level 7) ---

#[derive(SubStates, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[source(Screen = Screen::GravityChallenge)]
pub enum GravityPhase {
    #[default]
    Playing,
    Victory,
}

// --- Toll phases (Level 8) ---

#[derive(SubStates, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[source(Screen = Screen::TollChallenge)]
pub enum TollPhase {
    #[default]
    Playing,
    Victory,
}

// --- Arena phases (Level 9) ---

#[derive(SubStates, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[source(Screen = Screen::ArenaChallenge)]
pub enum ArenaPhase {
    #[default]
    Playing,
    Victory,
    Lost,
}

// --- Loot phases (Level 10) ---

#[derive(SubStates, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[source(Screen = Screen::LootChallenge)]
pub enum LootPhase {
    #[default]
    Playing,
    Victory,
}

// --- Clone phases (Level 11) ---

#[derive(SubStates, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[source(Screen = Screen::CloneChallenge)]
pub enum ClonePhase {
    #[default]
    Playing,
    Victory,
}

// --- Final phases (Level 12) ---

#[derive(SubStates, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[source(Screen = Screen::FinalChallenge)]
pub enum FinalPhase {
    #[default]
    Playing,
    Victory,
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

#[derive(Resource)]
pub struct Scoreboard {
    solved: [bool; 15],
}

impl Default for Scoreboard {
    fn default() -> Self {
        Self { solved: [false; 15] }
    }
}

impl Scoreboard {
    pub fn total_solved(&self) -> u32 {
        self.solved.iter().filter(|&&s| s).count() as u32
    }

    pub fn total_challenges(&self) -> u32 {
        15
    }

    pub fn is_solved(&self, level: u32) -> bool {
        level.checked_sub(1)
            .and_then(|i| self.solved.get(i as usize))
            .copied()
            .unwrap_or(false)
    }

    pub fn set_solved(&mut self, level: u32) {
        if let Some(slot) = level.checked_sub(1).and_then(|i| self.solved.get_mut(i as usize)) {
            *slot = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoreboard_starts_empty() {
        let sb = Scoreboard::default();
        assert_eq!(sb.total_solved(), 0);
        assert_eq!(sb.total_challenges(), 15);
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
        for level in 1..=15 {
            sb.set_solved(level);
        }
        assert_eq!(sb.total_solved(), 15);
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
    fn all_15_levels_have_screens() {
        // Verify Screen enum has all variants
        let screens = [
            Screen::Menu,
            Screen::PasswordChallenge,
            Screen::CannonChallenge,
            Screen::CountdownChallenge,
            Screen::MazeChallenge,
            Screen::RaceChallenge,
            Screen::ChestChallenge,
            Screen::GravityChallenge,
            Screen::TollChallenge,
            Screen::ArenaChallenge,
            Screen::LootChallenge,
            Screen::CloneChallenge,
            Screen::FinalChallenge,
            Screen::HillChallenge,
            Screen::MeadowChallenge,
            Screen::WaterparkChallenge,
        ];
        assert_eq!(screens.len(), 16); // 15 levels + menu
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
        .add_sub_state::<ChestPhase>()
        .add_sub_state::<GravityPhase>()
        .add_sub_state::<TollPhase>()
        .add_sub_state::<ArenaPhase>()
        .add_sub_state::<LootPhase>()
        .add_sub_state::<ClonePhase>()
        .add_sub_state::<FinalPhase>()
        .add_sub_state::<HillPhase>()
        .add_sub_state::<MeadowPhase>()
        .add_sub_state::<WaterparkPhase>()
        .init_resource::<Scoreboard>()
        .init_resource::<GamePaused>()
        .init_resource::<shared_ui::CameraOrbit>()
        .add_systems(OnEnter(Screen::Menu), reset_pause)
        .add_plugins((
            menu::MenuPlugin,
            level1::Level1Plugin,
            password::PasswordPlugin,
            level2::Level2Plugin,
            level3::Level3Plugin,
            level4::Level4Plugin,
            level5::Level5Plugin,
            level6::Level6Plugin,
            level7::Level7Plugin,
            level8::Level8Plugin,
            level9::Level9Plugin,
            level10::Level10Plugin,
            level11::Level11Plugin,
            level12::Level12Plugin,
            level13::Level13Plugin,
        ))
        .add_plugins(level14::Level14Plugin)
        .add_plugins(level15::Level15Plugin);
    #[cfg(feature = "test_bot")]
    app.add_plugins(test_bot::TestBotPlugin);
    app.run();
}
