use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;

use crate::{Screen, Scoreboard};

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::Menu), setup_menu)
            .add_systems(
                Update,
                (menu_keyboard, menu_button_click, menu_button_hover, menu_gamepad)
                    .run_if(in_state(Screen::Menu)),
            )
            .add_systems(OnExit(Screen::Menu), cleanup_menu);
    }
}

#[derive(Component)]
struct MenuScreen;

#[derive(Component)]
struct ScoreboardText;

#[derive(Component)]
struct ChallengeButton(u32);

#[derive(Resource)]
struct MenuSelection(Option<usize>);

/// Cooldown to prevent stick input from firing every frame.
#[derive(Resource)]
struct MenuNavCooldown(f32);

const LEVEL_NAMES: [&str; 15] = [
    "The Password Gate",
    "The Cannon Gauntlet",
    "The Countdown",
    "The Invisible Maze",
    "The Rigged Race",
    "The Locked Chest",
    "Gravity Flip",
    "The Phantom Toll",
    "Friendly Fire",
    "The Loot Goblin",
    "The Doppelganger",
    "The Final Exam",
    "The Hill Fortress",
    "The Rolling Meadow",
    "The Indoor Waterpark",
];

fn screen_for_level(level: u32) -> Option<Screen> {
    match level {
        1 => Some(Screen::PasswordChallenge),
        2 => Some(Screen::CannonChallenge),
        3 => Some(Screen::CountdownChallenge),
        4 => Some(Screen::MazeChallenge),
        5 => Some(Screen::RaceChallenge),
        6 => Some(Screen::ChestChallenge),
        7 => Some(Screen::GravityChallenge),
        8 => Some(Screen::TollChallenge),
        9 => Some(Screen::ArenaChallenge),
        10 => Some(Screen::LootChallenge),
        11 => Some(Screen::CloneChallenge),
        12 => Some(Screen::FinalChallenge),
        13 => Some(Screen::HillChallenge),
        14 => Some(Screen::MeadowChallenge),
        15 => Some(Screen::WaterparkChallenge),
        _ => None,
    }
}

fn setup_menu(mut commands: Commands, scoreboard: Res<Scoreboard>) {
    commands.insert_resource(MenuSelection(None));
    commands.insert_resource(MenuNavCooldown(0.0));
    commands.spawn((Camera2d, MenuScreen));

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                padding: UiRect::all(Val::Px(16.0)),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            MenuScreen,
        ))
        .with_children(|parent| {
            // Title
            parent.spawn((
                Text::new("DEBUGGER CHALLENGES"),
                TextFont {
                    font_size: 42.0,
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.9, 0.1)),
                Node {
                    margin: UiRect::bottom(Val::Px(8.0)),
                    ..default()
                },
            ));

            // Scoreboard
            parent.spawn((
                Text::new(format!(
                    "Solved: {} / {}",
                    scoreboard.total_solved(),
                    scoreboard.total_challenges()
                )),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.9, 0.6)),
                ScoreboardText,
                Node {
                    margin: UiRect::bottom(Val::Px(12.0)),
                    ..default()
                },
            ));

            // Level buttons grid (2 columns)
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    column_gap: Val::Px(10.0),
                    row_gap: Val::Px(8.0),
                    max_width: Val::Px(700.0),
                    ..default()
                })
                .with_children(|grid| {
                    for (i, name) in LEVEL_NAMES.iter().enumerate() {
                        let level = (i + 1) as u32;
                        let solved = scoreboard.is_solved(level);
                        let label = if solved {
                            format!("#{} {}", level, name)
                        } else {
                            format!("#{} {}", level, name)
                        };
                        let bg_color = if solved {
                            Color::srgb(0.15, 0.3, 0.15)
                        } else {
                            Color::srgb(0.2, 0.2, 0.3)
                        };
                        let text_color = if solved {
                            Color::srgb(0.5, 1.0, 0.5)
                        } else {
                            Color::srgb(0.9, 0.9, 0.9)
                        };

                        grid.spawn((
                            Button,
                            Node {
                                width: Val::Px(330.0),
                                padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            BackgroundColor(bg_color),
                            BorderRadius::all(Val::Px(6.0)),
                            ChallengeButton(level),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new(label),
                                TextFont {
                                    font_size: 18.0,
                                    ..default()
                                },
                                TextColor(text_color),
                            ));
                        });
                    }
                });

            // Hint
            parent.spawn((
                Text::new("Press 1-9, 0, -, =, \\, ], [ or click to start  |  D-pad + A on gamepad"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.5, 0.5)),
                Node {
                    margin: UiRect::top(Val::Px(12.0)),
                    ..default()
                },
            ));
        });
}

fn menu_keyboard(
    mut events: EventReader<KeyboardInput>,
    mut next_state: ResMut<NextState<Screen>>,
) {
    for event in events.read() {
        if !event.state.is_pressed() {
            continue;
        }
        if let Key::Character(c) = &event.logical_key {
            let level = match c.as_str() {
                "1" => Some(1u32),
                "2" => Some(2),
                "3" => Some(3),
                "4" => Some(4),
                "5" => Some(5),
                "6" => Some(6),
                "7" => Some(7),
                "8" => Some(8),
                "9" => Some(9),
                "0" => Some(10),
                "-" => Some(11),
                "=" => Some(12),
                "\\" => Some(13),
                "]" => Some(14),
                "[" => Some(15),
                _ => None,
            };
            if let Some(l) = level {
                if let Some(screen) = screen_for_level(l) {
                    next_state.set(screen);
                }
            }
        }
    }
}

fn menu_button_click(
    interaction: Query<(&Interaction, &ChallengeButton), Changed<Interaction>>,
    mut next_state: ResMut<NextState<Screen>>,
) {
    for (inter, btn) in &interaction {
        if *inter == Interaction::Pressed {
            if let Some(screen) = screen_for_level(btn.0) {
                next_state.set(screen);
            }
        }
    }
}

fn menu_button_hover(
    mut interaction: Query<
        (&Interaction, &mut BackgroundColor, &ChallengeButton),
        (Changed<Interaction>, With<ChallengeButton>),
    >,
    selection: Option<Res<MenuSelection>>,
) {
    let sel = selection.and_then(|s| s.0);
    for (inter, mut bg, btn) in &mut interaction {
        let is_selected = sel == Some(btn.0 as usize - 1);
        *bg = match *inter {
            Interaction::Hovered => BackgroundColor(Color::srgb(0.3, 0.3, 0.5)),
            Interaction::Pressed => BackgroundColor(Color::srgb(0.4, 0.4, 0.6)),
            Interaction::None if is_selected => BackgroundColor(Color::srgb(0.3, 0.3, 0.5)),
            Interaction::None => BackgroundColor(Color::srgb(0.2, 0.2, 0.3)),
        };
    }
}

const NUM_LEVELS: usize = 15;
const MENU_COLS: usize = 2;

fn menu_gamepad(
    gamepads: Query<&Gamepad>,
    time: Res<Time>,
    mut selection: ResMut<MenuSelection>,
    mut cooldown: ResMut<MenuNavCooldown>,
    mut next_state: ResMut<NextState<Screen>>,
    mut buttons: Query<(&ChallengeButton, &mut BackgroundColor), With<ChallengeButton>>,
    scoreboard: Res<Scoreboard>,
) {
    cooldown.0 = (cooldown.0 - time.delta_secs()).max(0.0);

    let mut dx: i32 = 0;
    let mut dy: i32 = 0;
    let mut confirm = false;

    for gamepad in &gamepads {
        // D-pad (digital, use just_pressed for crisp nav)
        if gamepad.just_pressed(GamepadButton::DPadUp) {
            dy -= 1;
        }
        if gamepad.just_pressed(GamepadButton::DPadDown) {
            dy += 1;
        }
        if gamepad.just_pressed(GamepadButton::DPadLeft) {
            dx -= 1;
        }
        if gamepad.just_pressed(GamepadButton::DPadRight) {
            dx += 1;
        }

        // Left stick (with cooldown)
        if cooldown.0 <= 0.0 {
            let stick = gamepad.left_stick();
            if stick.y > 0.5 {
                dy -= 1;
            } else if stick.y < -0.5 {
                dy += 1;
            }
            if stick.x > 0.5 {
                dx += 1;
            } else if stick.x < -0.5 {
                dx -= 1;
            }
        }

        if gamepad.just_pressed(GamepadButton::South) {
            confirm = true;
        }
    }

    if dx != 0 || dy != 0 {
        cooldown.0 = 0.18;
        let cur = selection.0.unwrap_or(0);
        let col = cur % MENU_COLS;
        let row = cur / MENU_COLS;

        let new_col = (col as i32 + dx).clamp(0, (MENU_COLS - 1) as i32) as usize;
        let new_row = (row as i32 + dy).clamp(0, ((NUM_LEVELS - 1) / MENU_COLS) as i32) as usize;
        let mut new_idx = new_row * MENU_COLS + new_col;
        if new_idx >= NUM_LEVELS {
            new_idx = NUM_LEVELS - 1;
        }
        selection.0 = Some(new_idx);

        // Update button colors
        for (btn, mut bg) in &mut buttons {
            let idx = btn.0 as usize - 1;
            let solved = scoreboard.is_solved(btn.0);
            if idx == new_idx {
                *bg = BackgroundColor(Color::srgb(0.3, 0.3, 0.5));
            } else if solved {
                *bg = BackgroundColor(Color::srgb(0.15, 0.3, 0.15));
            } else {
                *bg = BackgroundColor(Color::srgb(0.2, 0.2, 0.3));
            }
        }
    }

    if confirm {
        if let Some(idx) = selection.0 {
            let level = idx as u32 + 1;
            if let Some(screen) = screen_for_level(level) {
                next_state.set(screen);
            }
        }
    }
}

fn cleanup_menu(mut commands: Commands, query: Query<Entity, With<MenuScreen>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
    commands.remove_resource::<MenuSelection>();
    commands.remove_resource::<MenuNavCooldown>();
}
