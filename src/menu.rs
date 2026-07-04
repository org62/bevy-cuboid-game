use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;

use crate::{Screen, Scoreboard};

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SecretsRevealed>()
            .add_systems(OnEnter(Screen::Menu), setup_menu)
            .add_systems(
                Update,
                (
                    menu_keyboard,
                    menu_button_click,
                    menu_button_hover,
                    menu_gamepad,
                    update_menu_visibility,
                )
                    .run_if(in_state(Screen::Menu)),
            )
            .add_systems(OnExit(Screen::Menu), cleanup_menu);
    }
}

#[derive(Component)]
struct MenuScreen;

#[derive(Component)]
struct ScoreboardText;

/// The internal level id (1-5, 13-15) the button launches.
#[derive(Component)]
struct ChallengeButton(u32);

/// Marks a hidden easter-egg level button (levels 13-15, shown as 101-103).
#[derive(Component)]
struct HiddenButton;

/// Whether the hidden easter-egg levels have been revealed (via the `?` key).
/// Persists for the session.
#[derive(Resource, Default)]
struct SecretsRevealed(bool);

#[derive(Resource)]
struct MenuSelection(Option<usize>);

/// Cooldown to prevent stick input from firing every frame.
#[derive(Resource)]
struct MenuNavCooldown(f32);

/// Regular levels: (internal id, name).
const REGULAR_LEVELS: [(u32, &str); 5] = [
    (1, "The Password Gate"),
    (2, "The Cannon Gauntlet"),
    (3, "The Countdown"),
    (4, "The Invisible Maze"),
    (5, "The Rigged Race"),
];

/// Hidden easter-egg levels: (internal id = display number, name).
const HIDDEN_LEVELS: [(u32, &str); 3] = [
    (101, "The Hill Fortress"),
    (102, "The Rolling Meadow"),
    (103, "The Indoor Waterpark"),
];

fn screen_for_level(level: u32) -> Option<Screen> {
    match level {
        1 => Some(Screen::PasswordChallenge),
        2 => Some(Screen::CannonChallenge),
        3 => Some(Screen::CountdownChallenge),
        4 => Some(Screen::MazeChallenge),
        5 => Some(Screen::RaceChallenge),
        101 => Some(Screen::HillChallenge),
        102 => Some(Screen::MeadowChallenge),
        103 => Some(Screen::WaterparkChallenge),
        _ => None,
    }
}

/// Ordered list of the level ids the player can currently pick.
fn visible_levels(revealed: bool) -> Vec<u32> {
    let mut v: Vec<u32> = REGULAR_LEVELS.iter().map(|(l, _)| *l).collect();
    if revealed {
        v.extend(HIDDEN_LEVELS.iter().map(|(l, _)| *l));
    }
    v
}

fn solved_and_total(scoreboard: &Scoreboard, revealed: bool) -> (u32, u32) {
    let regular = REGULAR_LEVELS
        .iter()
        .filter(|(l, _)| scoreboard.is_solved(*l))
        .count() as u32;
    if revealed {
        let hidden = HIDDEN_LEVELS
            .iter()
            .filter(|(l, _)| scoreboard.is_solved(*l))
            .count() as u32;
        (regular + hidden, 8)
    } else {
        (regular, 5)
    }
}

fn button_colors(solved: bool) -> (Color, Color) {
    if solved {
        (Color::srgb(0.15, 0.3, 0.15), Color::srgb(0.5, 1.0, 0.5))
    } else {
        (Color::srgb(0.2, 0.2, 0.3), Color::srgb(0.9, 0.9, 0.9))
    }
}

fn setup_menu(
    mut commands: Commands,
    scoreboard: Res<Scoreboard>,
    revealed: Res<SecretsRevealed>,
) {
    commands.insert_resource(MenuSelection(None));
    commands.insert_resource(MenuNavCooldown(0.0));
    commands.spawn((Camera2d, MenuScreen));

    let (solved, total) = solved_and_total(&scoreboard, revealed.0);

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
                TextFont { font_size: 42.0, ..default() },
                TextColor(Color::srgb(0.9, 0.9, 0.1)),
                Node { margin: UiRect::bottom(Val::Px(8.0)), ..default() },
            ));

            // Scoreboard
            parent.spawn((
                Text::new(format!("Solved: {} / {}", solved, total)),
                TextFont { font_size: 22.0, ..default() },
                TextColor(Color::srgb(0.6, 0.9, 0.6)),
                ScoreboardText,
                Node { margin: UiRect::bottom(Val::Px(12.0)), ..default() },
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
                    for (level, name) in REGULAR_LEVELS.iter() {
                        spawn_level_button(grid, *level, *level, name, scoreboard.is_solved(*level), false, true);
                    }
                    for (level, name) in HIDDEN_LEVELS.iter() {
                        spawn_level_button(
                            grid,
                            *level,
                            *level,
                            name,
                            scoreboard.is_solved(*level),
                            true,
                            revealed.0,
                        );
                    }
                });

            // Hint
            parent.spawn((
                Text::new("Press 1-5 or click to start  |  D-pad + A on gamepad"),
                TextFont { font_size: 16.0, ..default() },
                TextColor(Color::srgb(0.5, 0.5, 0.5)),
                Node { margin: UiRect::top(Val::Px(12.0)), ..default() },
            ));
        });
}

#[allow(clippy::too_many_arguments)]
fn spawn_level_button(
    grid: &mut ChildBuilder,
    level: u32,
    display_num: u32,
    name: &str,
    solved: bool,
    hidden: bool,
    visible: bool,
) {
    let (bg_color, text_color) = button_colors(solved);
    let mut node = Node {
        width: Val::Px(330.0),
        padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
        justify_content: JustifyContent::Center,
        ..default()
    };
    if hidden && !visible {
        node.display = Display::None;
    }

    let mut btn = grid.spawn((
        Button,
        node,
        BackgroundColor(bg_color),
        BorderRadius::all(Val::Px(6.0)),
        ChallengeButton(level),
    ));
    if hidden {
        btn.insert(HiddenButton);
    }
    btn.with_children(|b| {
        b.spawn((
            Text::new(format!("#{} {}", display_num, name)),
            TextFont { font_size: 18.0, ..default() },
            TextColor(text_color),
        ));
    });
}

fn menu_keyboard(
    mut events: EventReader<KeyboardInput>,
    mut next_state: ResMut<NextState<Screen>>,
    mut revealed: ResMut<SecretsRevealed>,
) {
    for event in events.read() {
        if !event.state.is_pressed() {
            continue;
        }
        if let Key::Character(c) = &event.logical_key {
            match c.as_str() {
                "1" => start(&mut next_state, 1),
                "2" => start(&mut next_state, 2),
                "3" => start(&mut next_state, 3),
                "4" => start(&mut next_state, 4),
                "5" => start(&mut next_state, 5),
                "?" => revealed.0 = true, // easter egg: reveal hidden levels
                _ => {}
            }
        }
    }
}

fn start(next_state: &mut ResMut<NextState<Screen>>, level: u32) {
    if let Some(screen) = screen_for_level(level) {
        next_state.set(screen);
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
    scoreboard: Res<Scoreboard>,
) {
    for (inter, mut bg, btn) in &mut interaction {
        let solved = scoreboard.is_solved(btn.0);
        let (base, _) = button_colors(solved);
        *bg = match *inter {
            Interaction::Hovered => BackgroundColor(Color::srgb(0.3, 0.3, 0.5)),
            Interaction::Pressed => BackgroundColor(Color::srgb(0.4, 0.4, 0.6)),
            Interaction::None => BackgroundColor(base),
        };
    }
}

/// Keeps the hidden buttons and the scoreboard total in sync with the reveal
/// state, so pressing `?` immediately shows the easter-egg levels.
fn update_menu_visibility(
    revealed: Res<SecretsRevealed>,
    scoreboard: Res<Scoreboard>,
    mut hidden_q: Query<&mut Node, With<HiddenButton>>,
    mut score_q: Query<&mut Text, With<ScoreboardText>>,
) {
    let show = revealed.0;
    for mut node in &mut hidden_q {
        let want = if show { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
    let (solved, total) = solved_and_total(&scoreboard, show);
    if let Ok(mut text) = score_q.get_single_mut() {
        let s = format!("Solved: {} / {}", solved, total);
        if **text != s {
            **text = s;
        }
    }
}

fn menu_gamepad(
    gamepads: Query<&Gamepad>,
    time: Res<Time>,
    revealed: Res<SecretsRevealed>,
    mut selection: ResMut<MenuSelection>,
    mut cooldown: ResMut<MenuNavCooldown>,
    mut next_state: ResMut<NextState<Screen>>,
    mut buttons: Query<(&ChallengeButton, &mut BackgroundColor), With<ChallengeButton>>,
    scoreboard: Res<Scoreboard>,
) {
    cooldown.0 = (cooldown.0 - time.delta_secs()).max(0.0);

    let visible = visible_levels(revealed.0);
    if visible.is_empty() {
        return;
    }

    let mut delta: i32 = 0;
    let mut confirm = false;

    for gamepad in &gamepads {
        if gamepad.just_pressed(GamepadButton::DPadUp) || gamepad.just_pressed(GamepadButton::DPadLeft) {
            delta -= 1;
        }
        if gamepad.just_pressed(GamepadButton::DPadDown) || gamepad.just_pressed(GamepadButton::DPadRight) {
            delta += 1;
        }
        if cooldown.0 <= 0.0 {
            let stick = gamepad.left_stick();
            if stick.y > 0.5 {
                delta -= 1;
            } else if stick.y < -0.5 {
                delta += 1;
            }
        }
        if gamepad.just_pressed(GamepadButton::South) {
            confirm = true;
        }
    }

    if delta != 0 {
        cooldown.0 = 0.18;
        let cur = selection.0.unwrap_or(0) as i32;
        let new_idx = (cur + delta).clamp(0, visible.len() as i32 - 1) as usize;
        selection.0 = Some(new_idx);

        let selected_level = visible.get(new_idx).copied();
        for (btn, mut bg) in &mut buttons {
            if Some(btn.0) == selected_level {
                *bg = BackgroundColor(Color::srgb(0.3, 0.3, 0.5));
            } else {
                let (base, _) = button_colors(scoreboard.is_solved(btn.0));
                *bg = BackgroundColor(base);
            }
        }
    }

    if confirm {
        if let Some(idx) = selection.0 {
            if let Some(&level) = visible.get(idx) {
                if let Some(screen) = screen_for_level(level) {
                    next_state.set(screen);
                }
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
