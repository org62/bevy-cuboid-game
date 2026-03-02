mod level1;
mod level2;
mod menu;
mod password;
mod player;

use bevy::prelude::*;
use bevy::window::PresentMode;

// --- Screens ---

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Screen {
    #[default]
    Menu,
    PasswordChallenge,
    CannonChallenge,
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

// --- Scoreboard ---

#[derive(Resource, Default)]
pub struct Scoreboard {
    pub password_solved: bool,
    pub cannon_solved: bool,
}

impl Scoreboard {
    pub fn total_solved(&self) -> u32 {
        self.password_solved as u32 + self.cannon_solved as u32
    }

    pub fn total_challenges(&self) -> u32 {
        2
    }
}

// --- App entry ---

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
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
        .init_resource::<Scoreboard>()
        .add_plugins((
            menu::MenuPlugin,
            level1::Level1Plugin,
            password::PasswordPlugin,
            level2::Level2Plugin,
        ))
        .run();
}
