use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;

use crate::levels;
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

#[derive(Component)]
struct ShortcutHintText;

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

/// Ordered list of the level ids the player can currently pick. Sourced from
/// the roster in `src/levels.rs` — the menu never keeps its own level table.
pub(crate) fn visible_levels(revealed: bool) -> Vec<u32> {
    levels::visible(revealed).map(|l| l.id).collect()
}

/// Solved / total for the scoreboard line. Hidden levels only join the
/// denominator once revealed, so the menu doesn't advertise their existence.
fn solved_and_total(scoreboard: &Scoreboard, revealed: bool) -> (u32, u32) {
    let solved = levels::visible(revealed)
        .filter(|l| scoreboard.is_solved(l.id))
        .count() as u32;
    (solved, levels::visible_count(revealed))
}

/// The keyboard-shortcut hint line, sized to the roster (digits only go to 9).
fn shortcut_hint(revealed: bool) -> String {
    let n = levels::visible_count(revealed).min(9);
    format!("Press 1-{} or click to start  |  D-pad + A on gamepad", n)
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
    // Levels each set their own ClearColor and it is global state; without this
    // the menu keeps the sky of whichever level was played last.
    commands.insert_resource(ClearColor(Color::srgb(0.05, 0.05, 0.1)));
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
                    for l in levels::LEVELS {
                        spawn_level_button(
                            grid,
                            l.id,
                            l.id,
                            l.name,
                            scoreboard.is_solved(l.id),
                            l.hidden,
                            !l.hidden || revealed.0,
                        );
                    }
                });

            // Hint
            parent.spawn((
                Text::new(shortcut_hint(revealed.0)),
                TextFont { font_size: 16.0, ..default() },
                TextColor(Color::srgb(0.5, 0.5, 0.5)),
                ShortcutHintText,
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
            if c.as_str() == "?" {
                revealed.0 = true; // easter egg: reveal hidden levels
                continue;
            }
            // Digit N launches the Nth *visible* level, straight from the
            // roster — no per-level shortcut table to keep in sync. Hidden
            // levels gain shortcuts the moment they are revealed.
            if let Ok(n) = c.as_str().parse::<usize>() {
                if let Some(l) = n.checked_sub(1).and_then(|i| levels::visible(revealed.0).nth(i)) {
                    next_state.set(l.screen());
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
            if let Some(screen) = levels::screen_for_level(btn.0) {
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
    mut score_q: Query<&mut Text, (With<ScoreboardText>, Without<ShortcutHintText>)>,
    mut hint_q: Query<&mut Text, (With<ShortcutHintText>, Without<ScoreboardText>)>,
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
    if let Ok(mut text) = hint_q.get_single_mut() {
        let s = shortcut_hint(show);
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
                if let Some(screen) = levels::screen_for_level(level) {
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
