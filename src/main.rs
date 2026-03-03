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
mod menu;
mod password;
mod player;
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

// --- Pause ---

#[derive(Resource, Default)]
pub struct GamePaused(pub bool);

// --- Scoreboard ---

#[derive(Resource, Default)]
pub struct Scoreboard {
    pub password_solved: bool,
    pub cannon_solved: bool,
    pub countdown_solved: bool,
    pub maze_solved: bool,
    pub race_solved: bool,
    pub chest_solved: bool,
    pub gravity_solved: bool,
    pub toll_solved: bool,
    pub arena_solved: bool,
    pub loot_solved: bool,
    pub clone_solved: bool,
    pub final_solved: bool,
}

impl Scoreboard {
    pub fn total_solved(&self) -> u32 {
        self.password_solved as u32
            + self.cannon_solved as u32
            + self.countdown_solved as u32
            + self.maze_solved as u32
            + self.race_solved as u32
            + self.chest_solved as u32
            + self.gravity_solved as u32
            + self.toll_solved as u32
            + self.arena_solved as u32
            + self.loot_solved as u32
            + self.clone_solved as u32
            + self.final_solved as u32
    }

    pub fn total_challenges(&self) -> u32 {
        12
    }

    pub fn is_solved(&self, level: u32) -> bool {
        match level {
            1 => self.password_solved,
            2 => self.cannon_solved,
            3 => self.countdown_solved,
            4 => self.maze_solved,
            5 => self.race_solved,
            6 => self.chest_solved,
            7 => self.gravity_solved,
            8 => self.toll_solved,
            9 => self.arena_solved,
            10 => self.loot_solved,
            11 => self.clone_solved,
            12 => self.final_solved,
            _ => false,
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
        assert_eq!(sb.total_challenges(), 12);
    }

    #[test]
    fn scoreboard_counts_correctly() {
        let mut sb = Scoreboard::default();
        sb.password_solved = true;
        sb.cannon_solved = true;
        sb.countdown_solved = true;
        assert_eq!(sb.total_solved(), 3);
    }

    #[test]
    fn scoreboard_all_solved() {
        let sb = Scoreboard {
            password_solved: true,
            cannon_solved: true,
            countdown_solved: true,
            maze_solved: true,
            race_solved: true,
            chest_solved: true,
            gravity_solved: true,
            toll_solved: true,
            arena_solved: true,
            loot_solved: true,
            clone_solved: true,
            final_solved: true,
        };
        assert_eq!(sb.total_solved(), 12);
        assert_eq!(sb.total_solved(), sb.total_challenges());
    }

    #[test]
    fn is_solved_maps_correctly() {
        let mut sb = Scoreboard::default();
        sb.maze_solved = true;
        assert!(sb.is_solved(4));
        assert!(!sb.is_solved(3));
        assert!(!sb.is_solved(0)); // invalid level
        assert!(!sb.is_solved(13)); // invalid level
    }

    #[test]
    fn all_12_levels_have_screens() {
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
        ];
        assert_eq!(screens.len(), 13); // 12 levels + menu
    }
}

fn reset_pause(
    mut game_paused: ResMut<GamePaused>,
    mut commands: Commands,
    overlay_q: Query<Entity, With<player::PauseOverlay>>,
) {
    game_paused.0 = false;
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
        .init_resource::<Scoreboard>()
        .init_resource::<GamePaused>()
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
        ));
    #[cfg(feature = "test_bot")]
    app.add_plugins(test_bot::TestBotPlugin);
    app.run();
}
