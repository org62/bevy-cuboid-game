use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;

use crate::{Screen, Scoreboard};

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::Menu), setup_menu)
            .add_systems(
                Update,
                (menu_keyboard, menu_button_click, menu_button_hover)
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

fn setup_menu(mut commands: Commands, scoreboard: Res<Scoreboard>) {
    commands.spawn((Camera2d, MenuScreen));

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(24.0),
                ..default()
            },
            MenuScreen,
        ))
        .with_children(|parent| {
            // Title
            parent.spawn((
                Text::new("DEBUGGER CHALLENGES"),
                TextFont {
                    font_size: 52.0,
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.9, 0.1)),
            ));

            // Scoreboard
            parent.spawn((
                Text::new(format!(
                    "Solved: {} / {}",
                    scoreboard.total_solved(),
                    scoreboard.total_challenges()
                )),
                TextFont {
                    font_size: 26.0,
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.9, 0.6)),
                ScoreboardText,
            ));

            // Spacer
            parent.spawn(Node {
                height: Val::Px(16.0),
                ..default()
            });

            // Challenge #1 button
            let solved_1 = if scoreboard.password_solved {
                " [SOLVED]"
            } else {
                ""
            };
            parent
                .spawn((
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(32.0), Val::Px(12.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.2, 0.2, 0.3)),
                    ChallengeButton(1),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new(format!("#1 Password Challenge{}", solved_1)),
                        TextFont {
                            font_size: 28.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.9, 0.9, 0.9)),
                    ));
                });

            // Challenge #2 button
            let solved_2 = if scoreboard.cannon_solved {
                " [SOLVED]"
            } else {
                ""
            };
            parent
                .spawn((
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(32.0), Val::Px(12.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.2, 0.2, 0.3)),
                    ChallengeButton(2),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new(format!("#2 Cannon Gauntlet{}", solved_2)),
                        TextFont {
                            font_size: 28.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.9, 0.9, 0.9)),
                    ));
                });

            // Hint
            parent.spawn((
                Text::new("Press 1 or 2 or click to start"),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.5, 0.5)),
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
            match c.as_str() {
                "1" => next_state.set(Screen::PasswordChallenge),
                "2" => next_state.set(Screen::CannonChallenge),
                _ => {}
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
            match btn.0 {
                1 => next_state.set(Screen::PasswordChallenge),
                2 => next_state.set(Screen::CannonChallenge),
                _ => {}
            }
        }
    }
}

fn menu_button_hover(
    mut interaction: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<ChallengeButton>),
    >,
) {
    for (inter, mut bg) in &mut interaction {
        *bg = match *inter {
            Interaction::Hovered => BackgroundColor(Color::srgb(0.3, 0.3, 0.5)),
            Interaction::Pressed => BackgroundColor(Color::srgb(0.4, 0.4, 0.6)),
            Interaction::None => BackgroundColor(Color::srgb(0.2, 0.2, 0.3)),
        };
    }
}

fn cleanup_menu(mut commands: Commands, query: Query<Entity, With<MenuScreen>>) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}
