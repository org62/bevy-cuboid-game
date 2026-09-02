use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;

use bevy::state::state::FreelyMutableState;

use crate::level1::SCREEN;
use crate::level_kit::LevelPhase;
use crate::player::{Player, PlayerPhysics, PLAYER_PUSHBACK};
use crate::shared_ui::TextInputActive;
use crate::Screen;

/// Level 1's private prompt state machine, layered on top of the shared
/// [`LevelPhase`]: `Exploring` maps to `Playing`, the two prompt states run
/// under `Frozen` (sim halted, keyboard captured as text), and a correct
/// password hands off to `LevelPhase::Victory` for the shared victory flow.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum ChallengePhase {
    #[default]
    Exploring,
    PasswordPrompt,
    WrongPassword,
}

impl States for ChallengePhase {
    const DEPENDENCY_DEPTH: usize = <Screen as States>::DEPENDENCY_DEPTH + 1;
}

impl SubStates for ChallengePhase {
    type SourceStates = Screen;

    fn should_exist(source: Screen) -> Option<Self> {
        (source == SCREEN).then(Self::default)
    }
}

impl FreelyMutableState for ChallengePhase {}

pub fn register(app: &mut App) {
    app.add_sub_state::<ChallengePhase>()
        .add_systems(OnEnter(SCREEN), init_attempt_counter)
        .add_systems(
            OnEnter(ChallengePhase::PasswordPrompt),
            (setup_password_overlay, freeze_for_prompt),
        )
        .add_systems(
            Update,
            (handle_password_input, update_password_display)
                .chain()
                .run_if(in_state(ChallengePhase::PasswordPrompt).and(in_state(LevelPhase::Frozen))),
        )
        .add_systems(
            Update,
            handle_wrong_password.run_if(in_state(ChallengePhase::WrongPassword)),
        )
        .add_systems(
            OnEnter(ChallengePhase::Exploring),
            (cleanup_password_overlay, resume_from_prompt),
        )
        // A correct password wins via the shared flow; drop the prompt so the
        // victory overlay isn't stacked on top of it.
        .add_systems(
            OnEnter(LevelPhase::Victory),
            (cleanup_password_overlay, resume_from_prompt).run_if(in_state(SCREEN)),
        )
        .add_systems(OnExit(SCREEN), cleanup_password_overlay);
}

/// The prompt captures the keyboard: halt the sim and disable global hotkeys.
fn freeze_for_prompt(mut commands: Commands, mut next_phase: ResMut<NextState<LevelPhase>>) {
    commands.insert_resource(TextInputActive);
    next_phase.set(LevelPhase::Frozen);
}

/// Release the keyboard capture and, if the sim is still frozen on the
/// prompt's account, resume it. Guarded so the hand-off to
/// `LevelPhase::Victory` is never clobbered back to `Playing`.
fn resume_from_prompt(
    mut commands: Commands,
    phase: Res<State<LevelPhase>>,
    mut next_phase: ResMut<NextState<LevelPhase>>,
) {
    commands.remove_resource::<TextInputActive>();
    if *phase.get() == LevelPhase::Frozen {
        next_phase.set(LevelPhase::Playing);
    }
}

// --- Resources ---

#[derive(Resource, Default)]
struct PasswordInput {
    text: String,
}

#[derive(Resource, Default)]
struct AttemptCounter {
    count: u32,
}

// --- Marker components ---

#[derive(Component)]
struct PasswordOverlay;

#[derive(Component)]
struct InputDisplayText;

#[derive(Component)]
struct ResultText;

#[derive(Component)]
struct AttemptCounterText;

// --- Password check (the debugger target) ---

#[inline(never)]
fn check_password(input: &str) -> bool {
    let correct: &[u8] = b"sesame";
    let input_bytes = input.as_bytes();
    // Deliberately NOT length-gated up front: the whole point of this level is
    // that a player can park a read watchpoint on their input buffer and watch
    // the comparison walk it byte by byte. An `if len != 6` gate skips the
    // compare loop entirely for any wrong-length input, so a player who typed
    // "aaa" sees nothing fire and concludes the buffer is never read.
    //
    // Instead each byte is guarded by only the bounds check it immediately
    // needs, nested one level per character. Short inputs are compared as far
    // as they go (so the watchpoint fires), then bail out; the innermost check
    // is `len() == 6`, which rejects a longer input like "sesameXX" that
    // matches on its prefix.
    if input_bytes.len() > 0 && input_bytes[0] == correct[0] {
        if input_bytes.len() > 1 && input_bytes[1] == correct[1] {
            if input_bytes.len() > 2 && input_bytes[2] == correct[2] {
                if input_bytes.len() > 3 && input_bytes[3] == correct[3] {
                    if input_bytes.len() > 4 && input_bytes[4] == correct[4] {
                        if input_bytes.len() > 5 && input_bytes[5] == correct[5] {
                            return input_bytes.len() == correct.len();
                        }
                    }
                }
            }
        }
    }
    false
}

// --- Systems ---

fn init_attempt_counter(mut commands: Commands) {
    commands.insert_resource(AttemptCounter::default());
}

fn setup_password_overlay(mut commands: Commands) {
    commands.insert_resource(PasswordInput::default());

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            GlobalZIndex(10),
            PasswordOverlay,
        ))
        .with_children(|parent| {
            // Centered panel
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(16.0),
                    padding: UiRect::all(Val::Px(40.0)),
                    ..default()
                })
                .with_children(|panel| {
                    // Title
                    panel.spawn((
                        Text::new("PASSWORD ZONE"),
                        TextFont {
                            font_size: 42.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.9, 0.9, 0.1)),
                    ));

                    // Prompt
                    panel.spawn((
                        Text::new("Enter the password:"),
                        TextFont {
                            font_size: 26.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.8, 0.8, 0.8)),
                    ));

                    // Input display (asterisks)
                    panel.spawn((
                        Text::new("_"),
                        TextFont {
                            font_size: 30.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 1.0, 1.0)),
                        InputDisplayText,
                    ));

                    // Result text
                    panel.spawn((
                        Text::new(""),
                        TextFont {
                            font_size: 34.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 1.0, 1.0)),
                        ResultText,
                    ));

                    // Attempt counter
                    panel.spawn((
                        Text::new("Attempts: 0"),
                        TextFont {
                            font_size: 20.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.5, 0.5, 0.5)),
                        AttemptCounterText,
                    ));

                    // Escape hint
                    panel.spawn((
                        Text::new("[Esc] Back"),
                        TextFont {
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.4, 0.4, 0.4)),
                    ));
                });
        });
}

fn handle_password_input(
    mut events: EventReader<KeyboardInput>,
    mut input: ResMut<PasswordInput>,
    mut attempts: ResMut<AttemptCounter>,
    mut next_phase: ResMut<NextState<ChallengePhase>>,
    mut next_level_phase: ResMut<NextState<LevelPhase>>,
    mut player_query: Query<(&mut Transform, &mut PlayerPhysics), With<Player>>,
) {
    for event in events.read() {
        if !event.state.is_pressed() {
            continue;
        }

        if event.logical_key == Key::Escape {
            if let Ok((mut transform, mut physics)) = player_query.get_single_mut() {
                transform.translation = PLAYER_PUSHBACK;
                physics.velocity = Vec3::ZERO;
                physics.grounded = true;
                physics.facing = Quat::from_rotation_y(std::f32::consts::PI);
            }
            next_phase.set(ChallengePhase::Exploring);
            return;
        }

        match &event.logical_key {
            Key::Character(c) => {
                let s = c.as_str();
                for ch in s.chars() {
                    if ch.is_ascii_graphic() || ch == ' ' {
                        input.text.push(ch);
                    }
                }
            }
            Key::Enter => {
                if check_password(&input.text) {
                    // The shared victory flow marks the scoreboard and shows
                    // the overlay ("ACCESS GRANTED!", see the level's
                    // `VictoryText`).
                    next_level_phase.set(LevelPhase::Victory);
                } else {
                    attempts.count += 1;
                    next_phase.set(ChallengePhase::WrongPassword);
                }
            }
            Key::Backspace => {
                input.text.pop();
            }
            _ => {}
        }
    }
}

fn update_password_display(
    input: Res<PasswordInput>,
    attempts: Res<AttemptCounter>,
    mut input_query: Query<
        &mut Text,
        (
            With<InputDisplayText>,
            Without<ResultText>,
            Without<AttemptCounterText>,
        ),
    >,
    mut counter_query: Query<
        &mut Text,
        (
            With<AttemptCounterText>,
            Without<InputDisplayText>,
            Without<ResultText>,
        ),
    >,
) {
    if let Ok(mut text) = input_query.get_single_mut() {
        if input.text.is_empty() {
            **text = "_".to_string();
        } else {
            **text = "*".repeat(input.text.len());
        }
    }

    if let Ok(mut text) = counter_query.get_single_mut() {
        **text = format!("Attempts: {}", attempts.count);
    }
}

// Both dismiss handlers below use `ButtonInput::just_pressed` rather than
// reading `KeyboardInput` events: a fresh system's EventReader starts by
// consuming every still-buffered event, so the Enter that SUBMITTED the
// password (alive for two frames) would immediately dismiss the result
// screen the player was meant to read.

fn handle_wrong_password(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_phase: ResMut<NextState<ChallengePhase>>,
    mut player_query: Query<(&mut Transform, &mut PlayerPhysics), With<Player>>,
    mut result_query: Query<(&mut Text, &mut TextColor), With<ResultText>>,
) {
    // Show wrong password text
    if let Ok((mut text, mut color)) = result_query.get_single_mut() {
        **text = "WRONG PASSWORD - press any key".to_string();
        *color = TextColor(Color::srgb(1.0, 0.3, 0.3));
    }

    if keyboard.get_just_pressed().next().is_some() {
        if let Ok((mut transform, mut physics)) = player_query.get_single_mut() {
            transform.translation = PLAYER_PUSHBACK;
            physics.velocity = Vec3::ZERO;
            physics.grounded = true;
            physics.facing = Quat::from_rotation_y(std::f32::consts::PI);
        }
        next_phase.set(ChallengePhase::Exploring);
    }
}

fn cleanup_password_overlay(
    mut commands: Commands,
    query: Query<Entity, With<PasswordOverlay>>,
) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correct_password_accepted() {
        assert!(check_password("sesame"));
    }

    #[test]
    fn wrong_password_rejected() {
        assert!(!check_password("password"));
        assert!(!check_password(""));
        assert!(!check_password("SESAME"));
        assert!(!check_password("sesam"));
        assert!(!check_password("sesamee"));
    }

    #[test]
    fn password_is_byte_by_byte() {
        // Verify partial matches still fail
        assert!(!check_password("sesam\0"));
        assert!(!check_password("sesame\0"));
    }

    #[test]
    fn short_and_long_inputs_are_rejected_without_panicking() {
        // Every prefix of the password is compared as far as it goes (that is
        // what makes the watchpoint fire) and then rejected - none of these may
        // index past the end of the input.
        for len in 0..6 {
            assert!(!check_password(&"sesame"[..len]), "prefix of len {len}");
        }
        // A longer input matching on its prefix must still fail.
        assert!(!check_password("sesameXX"));
        // Wrong-length inputs that diverge early are fine too.
        assert!(!check_password("a"));
        assert!(!check_password("aaaaaaaaaaaa"));
    }

    #[test]
    fn debugger_scenario_find_password() {
        // Simulates: player sets breakpoint on check_password,
        // inspects `correct` variable, finds b"sesame", types it in
        let found_password = std::str::from_utf8(b"sesame").unwrap();
        assert!(check_password(found_password));
    }
}
