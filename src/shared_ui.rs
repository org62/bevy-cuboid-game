use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};

use crate::player::{Player, PlayerPhysics};

// --- Shared components ---

#[derive(Component)]
pub struct HintBox;

#[derive(Component)]
pub struct HintCloseButton;

/// Corner-box button that opens the centered tutorial modal.
#[derive(Component)]
pub struct HintTutorialButton;

/// Full-screen centered overlay holding the long-form solution.
/// Spawned hidden (`Display::None`); toggled by [`hint_tutorial_controls`].
#[derive(Component)]
pub struct HintModal;

/// Close button inside the tutorial modal.
#[derive(Component)]
pub struct HintModalCloseButton;

#[derive(Component)]
pub struct OverlayScreen;

#[derive(Component)]
pub struct FollowCamera {
    pub offset: Vec3,
    pub lerp_speed: f32,
    pub look_offset: Vec3,
}

#[derive(Resource)]
pub struct CameraOrbit {
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
}

impl Default for CameraOrbit {
    fn default() -> Self {
        Self { yaw: 0.0, pitch: 0.0, zoom: 1.0 }
    }
}

/// Tracks which input device was used most recently, so UI (e.g. the controls
/// legend) can adapt. Defaults to keyboard/mouse.
#[derive(Resource, Default)]
pub struct ActiveInput {
    pub gamepad: bool,
}

/// Text component that carries both a keyboard/mouse and a gamepad legend;
/// [`update_controls_hint`] swaps the shown string based on [`ActiveInput`].
#[derive(Component)]
pub struct ControlsHint {
    pub keyboard: String,
    pub gamepad: String,
}

/// User-adjustable mouse-look sensitivity (a multiplier on the base rate).
#[derive(Resource)]
pub struct MouseSettings {
    pub sensitivity: f32,
}

impl Default for MouseSettings {
    fn default() -> Self {
        Self { sensitivity: 0.4 }
    }
}

/// Marker for any overlay that must free the cursor while it is visible.
#[derive(Component)]
pub struct CursorReleaser;

// --- Agenda (controls) dialog ---
#[derive(Component)]
pub struct AgendaModal;
#[derive(Component)]
pub struct AgendaCloseButton;

// --- Settings dialog ---
#[derive(Component)]
pub struct SettingsModal;
#[derive(Component)]
pub struct SettingsCloseButton;
#[derive(Component)]
pub struct SensDownButton;
#[derive(Component)]
pub struct SensUpButton;
#[derive(Component)]
pub struct SensValueText;

// --- Hint box ---

pub fn spawn_hint_box(
    commands: &mut Commands,
    hint_text: &str,
    max_width: f32,
    extra: impl Bundle,
) {
    commands
        .spawn((
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                right: Val::Px(16.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(14.0)),
                row_gap: Val::Px(8.0),
                max_width: Val::Px(max_width),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.08, 0.15, 0.9)),
            BorderRadius::all(Val::Px(10.0)),
            HintBox,
            extra,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Hint"),
                TextFont { font_size: 20.0, ..default() },
                TextColor(Color::srgb(1.0, 0.85, 0.3)),
            ));
            parent.spawn((
                Node { max_width: Val::Px(max_width - 30.0), ..default() },
                Text::new(hint_text),
                TextFont { font_size: 16.0, ..default() },
                TextColor(Color::srgb(0.85, 0.85, 0.85)),
            ));
            parent
                .spawn((
                    Node {
                        align_self: AlignSelf::FlexEnd,
                        padding: UiRect::axes(Val::Px(12.0), Val::Px(4.0)),
                        ..default()
                    },
                    Button,
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.15)),
                    BorderRadius::all(Val::Px(6.0)),
                    HintCloseButton,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("[X] Close"),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(Color::srgb(0.7, 0.7, 0.7)),
                    ));
                });
        });
}

/// Toggles the hint box: `[H]` shows/hides it (it starts hidden), while `[X]`
/// or its close button hides it. The box is kept (not despawned) so `[H]` can
/// bring it back.
pub fn dismiss_hint(
    keyboard: Res<ButtonInput<KeyCode>>,
    btn_q: Query<&Interaction, (Changed<Interaction>, With<HintCloseButton>)>,
    mut hint_q: Query<&mut Node, With<HintBox>>,
) {
    if keyboard.just_pressed(KeyCode::KeyH) {
        for mut n in &mut hint_q {
            n.display = if matches!(n.display, Display::None) {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
    let hide = keyboard.just_pressed(KeyCode::KeyX)
        || btn_q.iter().any(|i| *i == Interaction::Pressed);
    if hide {
        for mut n in &mut hint_q {
            n.display = Display::None;
        }
    }
}

/// Corner hint box with a short teaser and a one-line button row: a muted
/// "Tutorial" button that opens the centered [`HintModal`], and an accented
/// "[X] Close" button. Pair it with [`spawn_hint_modal`] and drive both with
/// [`hint_tutorial_controls`]. `extra` is a cleanup marker attached to the box.
pub fn spawn_hint_box_with_tutorial(
    commands: &mut Commands,
    teaser: &str,
    max_width: f32,
    extra: impl Bundle,
) {
    commands
        .spawn((
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                right: Val::Px(16.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(14.0)),
                row_gap: Val::Px(10.0),
                max_width: Val::Px(max_width),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.08, 0.15, 0.94)),
            BorderRadius::all(Val::Px(10.0)),
            GlobalZIndex(20),
            HintBox,
            extra,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Hint"),
                TextFont { font_size: 20.0, ..default() },
                TextColor(Color::srgb(1.0, 0.85, 0.3)),
            ));
            parent.spawn((
                Node { max_width: Val::Px(max_width - 30.0), ..default() },
                Text::new(teaser),
                TextFont { font_size: 16.0, ..default() },
                TextColor(Color::srgb(0.85, 0.85, 0.85)),
            ));

            // Button row: Tutorial (muted) + Close (accent), on one line.
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        Node {
                            padding: UiRect::axes(Val::Px(12.0), Val::Px(4.0)),
                            ..default()
                        },
                        Button,
                        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.12)),
                        BorderRadius::all(Val::Px(6.0)),
                        HintTutorialButton,
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("Tutorial"),
                            TextFont { font_size: 14.0, ..default() },
                            TextColor(Color::srgb(0.78, 0.78, 0.82)),
                        ));
                    });
                    row.spawn((
                        Node {
                            padding: UiRect::axes(Val::Px(12.0), Val::Px(4.0)),
                            ..default()
                        },
                        Button,
                        BackgroundColor(Color::srgba(0.95, 0.75, 0.2, 0.92)),
                        BorderRadius::all(Val::Px(6.0)),
                        HintCloseButton,
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("[X] Close"),
                            TextFont { font_size: 14.0, ..default() },
                            TextColor(Color::srgb(0.12, 0.1, 0.05)),
                        ));
                    });
                });
        });
}

/// Spawns the centered tutorial modal for [`spawn_hint_box_with_tutorial`],
/// hidden until the "Tutorial" button is clicked. `extra` is a cleanup marker.
pub fn spawn_hint_modal(
    commands: &mut Commands,
    title: &str,
    solution: &str,
    extra: impl Bundle,
) {
    commands
        .spawn((
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            GlobalZIndex(30),
            HintModal,
            CursorReleaser,
            extra,
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        width: Val::Px(560.0),
                        max_width: Val::Percent(94.0),
                        padding: UiRect::all(Val::Px(24.0)),
                        row_gap: Val::Px(12.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.09, 0.08, 0.13, 0.98)),
                    BorderRadius::all(Val::Px(12.0)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new(title),
                        TextFont { font_size: 24.0, ..default() },
                        TextColor(Color::srgb(1.0, 0.85, 0.3)),
                    ));
                    panel.spawn((
                        Node { width: Val::Percent(100.0), ..default() },
                        Text::new(solution),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(Color::srgb(0.85, 0.88, 0.94)),
                    ));
                    panel
                        .spawn((
                            Node {
                                align_self: AlignSelf::FlexEnd,
                                padding: UiRect::axes(Val::Px(14.0), Val::Px(5.0)),
                                ..default()
                            },
                            Button,
                            BackgroundColor(Color::srgba(0.95, 0.75, 0.2, 0.92)),
                            BorderRadius::all(Val::Px(6.0)),
                            HintModalCloseButton,
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new("[X] Close"),
                                TextFont { font_size: 15.0, ..default() },
                                TextColor(Color::srgb(0.12, 0.1, 0.05)),
                            ));
                        });
                });
        });
}

/// Drives the tutorial hint UI: "Tutorial" opens the centered modal, close
/// buttons / `[X]` hide the top-most open element, and `[H]` toggles the
/// corner box so a closed hint can be brought back.
pub fn hint_tutorial_controls(
    keyboard: Res<ButtonInput<KeyCode>>,
    close_q: Query<&Interaction, (Changed<Interaction>, With<HintCloseButton>)>,
    tutorial_q: Query<&Interaction, (Changed<Interaction>, With<HintTutorialButton>)>,
    modal_close_q: Query<&Interaction, (Changed<Interaction>, With<HintModalCloseButton>)>,
    mut box_q: Query<&mut Node, (With<HintBox>, Without<HintModal>)>,
    mut modal_q: Query<&mut Node, (With<HintModal>, Without<HintBox>)>,
) {
    let modal_open = modal_q
        .iter()
        .any(|n| !matches!(n.display, Display::None));

    // [T] toggles the modal.
    if keyboard.just_pressed(KeyCode::KeyT) {
        for mut n in &mut modal_q {
            n.display = if matches!(n.display, Display::None) {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
    // "Tutorial" button opens the modal.
    if tutorial_q.iter().any(|i| *i == Interaction::Pressed) {
        for mut n in &mut modal_q {
            n.display = Display::Flex;
        }
    }

    // Close the modal (its button).
    if modal_close_q.iter().any(|i| *i == Interaction::Pressed) {
        for mut n in &mut modal_q {
            n.display = Display::None;
        }
    }

    // Close the corner box (its button).
    if close_q.iter().any(|i| *i == Interaction::Pressed) {
        for mut n in &mut box_q {
            n.display = Display::None;
        }
    }

    // [X] hides the top-most open element (modal first, else the corner box).
    if keyboard.just_pressed(KeyCode::KeyX) {
        if modal_open {
            for mut n in &mut modal_q {
                n.display = Display::None;
            }
        } else {
            for mut n in &mut box_q {
                n.display = Display::None;
            }
        }
    }

    // [H] toggles the corner hint box back on/off.
    if keyboard.just_pressed(KeyCode::KeyH) {
        for mut n in &mut box_q {
            n.display = if matches!(n.display, Display::None) {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
}

// --- Victory overlay ---

pub fn spawn_victory_overlay(
    commands: &mut Commands,
    title: &str,
    subtitle: Option<&str>,
    subtitle_font_size: f32,
    instruction: &str,
    extra: impl Bundle,
) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.15, 0.0, 0.8)),
            GlobalZIndex(10),
            OverlayScreen,
            extra,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(title),
                TextFont { font_size: 52.0, ..default() },
                TextColor(Color::srgb(0.2, 1.0, 0.2)),
            ));
            if let Some(sub) = subtitle {
                parent.spawn((
                    Text::new(sub),
                    TextFont { font_size: subtitle_font_size, ..default() },
                    TextColor(Color::srgb(0.8, 1.0, 0.8)),
                ));
            }
            parent.spawn((
                Text::new(instruction),
                TextFont { font_size: 22.0, ..default() },
                TextColor(Color::srgb(0.6, 0.8, 0.6)),
            ));
        });
}

// --- Defeat overlay ---

pub fn spawn_defeat_overlay(
    commands: &mut Commands,
    title: &str,
    title_font_size: f32,
    subtitle: Option<&str>,
    subtitle_font_size: f32,
    instruction: &str,
    bg_color: Color,
    extra: impl Bundle,
) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(bg_color),
            GlobalZIndex(10),
            OverlayScreen,
            extra,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(title),
                TextFont { font_size: title_font_size, ..default() },
                TextColor(Color::srgb(1.0, 0.2, 0.2)),
            ));
            if let Some(sub) = subtitle {
                parent.spawn((
                    Text::new(sub),
                    TextFont { font_size: subtitle_font_size, ..default() },
                    TextColor(Color::srgb(1.0, 0.6, 0.6)),
                ));
            }
            parent.spawn((
                Text::new(instruction),
                TextFont { font_size: 22.0, ..default() },
                TextColor(Color::srgb(0.8, 0.6, 0.6)),
            ));
        });
}

// --- Controls hint ---

/// Spawns the standard level HUD: the minimal bottom-left legend, the controls
/// (`C`) and settings (`E`) dialogs, and an on-start objective banner showing
/// `objective`. `extra` is the level's cleanup marker (attached to each spawned
/// root, so it must be `Clone`).
pub fn spawn_controls_hint(commands: &mut Commands, objective: &str, extra: impl Bundle + Clone) {
    spawn_controls_legend_min(commands, extra.clone());
    spawn_agenda_default(commands, extra.clone());
    spawn_settings_modal(commands, extra.clone());
    spawn_objective(commands, objective, extra);
}

/// A top-center "OBJECTIVE" banner that holds briefly then fades out. Driven by
/// [`update_objective_banner`].
#[derive(Component)]
pub struct ObjectiveBanner {
    pub timer: f32,
}

/// Spawns the objective / call-to-action banner shown when a level starts.
pub fn spawn_objective(commands: &mut Commands, objective: &str, extra: impl Bundle) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(24.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            extra,
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        padding: UiRect::axes(Val::Px(26.0), Val::Px(12.0)),
                        row_gap: Val::Px(3.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.05, 0.05, 0.08, 0.85)),
                    BorderRadius::all(Val::Px(12.0)),
                    GlobalZIndex(15),
                    ObjectiveBanner { timer: 0.0 },
                ))
                .with_children(|pill| {
                    pill.spawn((
                        Text::new("OBJECTIVE"),
                        TextFont { font_size: 14.0, ..default() },
                        TextColor(Color::srgb(1.0, 0.85, 0.3)),
                    ));
                    pill.spawn((
                        Text::new(objective),
                        TextFont { font_size: 26.0, ..default() },
                        TextColor(Color::srgb(0.96, 0.96, 0.96)),
                    ));
                });
        });
}

/// Holds the objective banner fully visible, then fades and despawns it.
pub fn update_objective_banner(
    time: Res<Time>,
    mut commands: Commands,
    mut banner_q: Query<(Entity, &mut ObjectiveBanner, &mut BackgroundColor, &Children)>,
    mut text_q: Query<&mut TextColor>,
) {
    const HOLD: f32 = 3.5;
    const FADE: f32 = 1.2;
    let dt = time.delta_secs();
    for (entity, mut banner, mut bg, children) in &mut banner_q {
        banner.timer += dt;
        let alpha = if banner.timer < HOLD {
            1.0
        } else if banner.timer < HOLD + FADE {
            1.0 - (banner.timer - HOLD) / FADE
        } else {
            commands.entity(entity).despawn_recursive();
            continue;
        };
        bg.0 = bg.0.with_alpha(0.85 * alpha);
        for &child in children.iter() {
            if let Ok(mut tc) = text_q.get_mut(child) {
                tc.0 = tc.0.with_alpha(alpha);
            }
        }
    }
}

/// Standard controls text shown in the controls dialog (keyboard/mouse), one
/// binding per line in aligned columns.
pub const AGENDA_KB_DEFAULT: &str = "Esc     Close / Menu\nWASD    Move\nSpace   Jump\nP       Pause\nC       Controls\nE       Settings\nH       Hint\nMouse   Look\nWheel   Zoom";
/// Standard controls text shown in the controls dialog (gamepad).
pub const AGENDA_GP_DEFAULT: &str = "Select   Close / Menu\nL-Stick  Move\nA        Jump\nStart    Pause\nR-Stick  Look\nLT / RT  Zoom\nC        Controls\nE        Settings";

/// Convenience: spawn the controls dialog with the standard controls text.
pub fn spawn_agenda_default(commands: &mut Commands, extra: impl Bundle) {
    spawn_agenda_modal(commands, AGENDA_KB_DEFAULT, AGENDA_GP_DEFAULT, extra);
}

/// Minimal bottom-left legend ("Esc Menu | C Controls | E Settings"). The full
/// controls live in the controls dialog ([`spawn_agenda_modal`], toggled with `C`).
pub fn spawn_controls_legend_min(commands: &mut Commands, extra: impl Bundle) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(16.0),
                left: Val::Px(16.0),
                ..default()
            },
            extra,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Esc Menu | C Controls | E Settings"),
                TextFont { font_size: 18.0, ..default() },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
                ControlsHint {
                    keyboard: "Esc Menu | C Controls | E Settings".to_string(),
                    gamepad: "Select Menu | C Controls | E Settings".to_string(),
                },
            ));
        });
}

/// Centered "Controls" dialog listing the full (device-adaptive) bindings, with
/// a Settings button and a Close button. Hidden until toggled with `G`. Drive
/// with [`agenda_controls`].
pub fn spawn_agenda_modal(
    commands: &mut Commands,
    keyboard_full: &str,
    gamepad_full: &str,
    extra: impl Bundle,
) {
    commands
        .spawn((
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            GlobalZIndex(30),
            AgendaModal,
            CursorReleaser,
            extra,
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        width: Val::Px(520.0),
                        max_width: Val::Percent(94.0),
                        padding: UiRect::all(Val::Px(24.0)),
                        row_gap: Val::Px(14.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.09, 0.08, 0.13, 0.98)),
                    BorderRadius::all(Val::Px(12.0)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("Controls"),
                        TextFont { font_size: 24.0, ..default() },
                        TextColor(Color::srgb(1.0, 0.85, 0.3)),
                    ));
                    panel.spawn((
                        Node { width: Val::Percent(100.0), ..default() },
                        Text::new(keyboard_full),
                        TextFont { font_size: 16.0, ..default() },
                        TextColor(Color::srgb(0.85, 0.88, 0.94)),
                        ControlsHint {
                            keyboard: keyboard_full.to_string(),
                            gamepad: gamepad_full.to_string(),
                        },
                    ));
                    panel
                        .spawn((
                            Node {
                                align_self: AlignSelf::FlexEnd,
                                padding: UiRect::axes(Val::Px(14.0), Val::Px(5.0)),
                                ..default()
                            },
                            Button,
                            BackgroundColor(Color::srgba(0.95, 0.75, 0.2, 0.92)),
                            BorderRadius::all(Val::Px(6.0)),
                            AgendaCloseButton,
                        ))
                        .with_children(|b| {
                            b.spawn((
                                Text::new("Close"),
                                TextFont { font_size: 15.0, ..default() },
                                TextColor(Color::srgb(0.12, 0.1, 0.05)),
                            ));
                        });
                });
        });
}

/// Centered "Settings" dialog with a mouse-sensitivity adjuster. Hidden until
/// opened from the agenda's Settings button. Drive with [`agenda_controls`].
pub fn spawn_settings_modal(commands: &mut Commands, extra: impl Bundle) {
    commands
        .spawn((
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
            GlobalZIndex(40),
            SettingsModal,
            CursorReleaser,
            extra,
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        width: Val::Px(420.0),
                        max_width: Val::Percent(92.0),
                        padding: UiRect::all(Val::Px(24.0)),
                        row_gap: Val::Px(16.0),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.09, 0.08, 0.13, 0.98)),
                    BorderRadius::all(Val::Px(12.0)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("Settings"),
                        TextFont { font_size: 24.0, ..default() },
                        TextColor(Color::srgb(1.0, 0.85, 0.3)),
                    ));
                    panel.spawn((
                        Text::new("Mouse look sensitivity"),
                        TextFont { font_size: 16.0, ..default() },
                        TextColor(Color::srgb(0.85, 0.88, 0.94)),
                    ));
                    panel
                        .spawn(Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(16.0),
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn((
                                Node {
                                    padding: UiRect::axes(Val::Px(18.0), Val::Px(4.0)),
                                    ..default()
                                },
                                Button,
                                BackgroundColor(Color::srgba(0.95, 0.75, 0.2, 0.92)),
                                BorderRadius::all(Val::Px(6.0)),
                                SensDownButton,
                            ))
                            .with_children(|b| {
                                b.spawn((
                                    Text::new("-"),
                                    TextFont { font_size: 24.0, ..default() },
                                    TextColor(Color::srgb(0.12, 0.1, 0.05)),
                                ));
                            });
                            row.spawn((
                                Text::new("0.4"),
                                TextFont { font_size: 22.0, ..default() },
                                TextColor(Color::srgb(1.0, 1.0, 1.0)),
                                SensValueText,
                            ));
                            row.spawn((
                                Node {
                                    padding: UiRect::axes(Val::Px(18.0), Val::Px(4.0)),
                                    ..default()
                                },
                                Button,
                                BackgroundColor(Color::srgba(0.95, 0.75, 0.2, 0.92)),
                                BorderRadius::all(Val::Px(6.0)),
                                SensUpButton,
                            ))
                            .with_children(|b| {
                                b.spawn((
                                    Text::new("+"),
                                    TextFont { font_size: 24.0, ..default() },
                                    TextColor(Color::srgb(0.12, 0.1, 0.05)),
                                ));
                            });
                        });
                    panel
                        .spawn((
                            Node {
                                padding: UiRect::axes(Val::Px(14.0), Val::Px(5.0)),
                                ..default()
                            },
                            Button,
                            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.15)),
                            BorderRadius::all(Val::Px(6.0)),
                            SettingsCloseButton,
                        ))
                        .with_children(|b| {
                            b.spawn((
                                Text::new("Back"),
                                TextFont { font_size: 15.0, ..default() },
                                TextColor(Color::srgb(0.85, 0.85, 0.9)),
                            ));
                        });
                });
        });
}

/// Drives the controls + settings dialogs: `C` toggles the controls menu, `E`
/// toggles the sensitivity config, and +/- adjust it.
#[allow(clippy::too_many_arguments)]
pub fn agenda_controls(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<MouseSettings>,
    agenda_close_q: Query<&Interaction, (Changed<Interaction>, With<AgendaCloseButton>)>,
    settings_close_q: Query<&Interaction, (Changed<Interaction>, With<SettingsCloseButton>)>,
    sens_down_q: Query<&Interaction, (Changed<Interaction>, With<SensDownButton>)>,
    sens_up_q: Query<&Interaction, (Changed<Interaction>, With<SensUpButton>)>,
    mut agenda_q: Query<&mut Node, (With<AgendaModal>, Without<SettingsModal>)>,
    mut settings_q: Query<&mut Node, (With<SettingsModal>, Without<AgendaModal>)>,
    mut value_q: Query<&mut Text, With<SensValueText>>,
) {
    let agenda_open = agenda_q.iter().any(|n| !matches!(n.display, Display::None));
    let settings_open = settings_q.iter().any(|n| !matches!(n.display, Display::None));

    // `C` toggles the controls dialog; opening it closes settings.
    if keyboard.just_pressed(KeyCode::KeyC) {
        let show = !agenda_open;
        for mut n in &mut agenda_q {
            n.display = if show { Display::Flex } else { Display::None };
        }
        if show {
            for mut n in &mut settings_q {
                n.display = Display::None;
            }
        }
    }

    // `E` toggles settings; opening it closes the agenda.
    if keyboard.just_pressed(KeyCode::KeyE) {
        let show = !settings_open;
        for mut n in &mut settings_q {
            n.display = if show { Display::Flex } else { Display::None };
        }
        if show {
            for mut n in &mut agenda_q {
                n.display = Display::None;
            }
        }
    }

    // Settings "Back" closes settings.
    if settings_close_q.iter().any(|i| *i == Interaction::Pressed) {
        for mut n in &mut settings_q {
            n.display = Display::None;
        }
    }

    // Agenda "Close".
    if agenda_close_q.iter().any(|i| *i == Interaction::Pressed) {
        for mut n in &mut agenda_q {
            n.display = Display::None;
        }
    }

    // Sensitivity adjust.
    if sens_down_q.iter().any(|i| *i == Interaction::Pressed) {
        settings.sensitivity = (settings.sensitivity - 0.1).max(0.1);
    }
    if sens_up_q.iter().any(|i| *i == Interaction::Pressed) {
        settings.sensitivity = (settings.sensitivity + 0.1).min(2.0);
    }

    // Keep the value readout in sync.
    for mut text in &mut value_q {
        let s = format!("{:.1}", settings.sensitivity);
        if **text != s {
            **text = s;
        }
    }
}

// --- Lighting ---

pub fn setup_level_lighting(
    commands: &mut Commands,
    illuminance: f32,
    rotation: (f32, f32, f32),
    ambient_color: Color,
    ambient_brightness: f32,
    extra: impl Bundle,
) {
    commands.spawn((
        DirectionalLight {
            illuminance,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            rotation.0,
            rotation.1,
            rotation.2,
        )),
        extra,
    ));
    commands.insert_resource(AmbientLight {
        color: ambient_color,
        brightness: ambient_brightness,
    });
}

// --- Camera follow ---

/// Inflation of occluder boxes toward the camera — acts as the camera's
/// "radius" so the near plane (0.1) never pokes through a wall face.
const OCCLUSION_PAD_XZ: f32 = 0.35;
const OCCLUSION_PAD_Y: f32 = 0.2;
/// The occlusion ray starts at the player's torso, not the feet. Load-bearing:
/// the feet rest exactly on surface tops, so an unlifted origin would sit ON
/// the top face of any walk-on occluder and false-hit at t=0 every frame.
const OCCLUSION_ORIGIN_LIFT: f32 = 0.6;
/// Never pull the camera closer than this to the player.
const MIN_CAMERA_DISTANCE: f32 = 0.6;
/// Easing rate (per second) for the camera moving back OUT when an occlusion
/// clears. Pulling IN is instant — the camera must never spend a frame
/// behind a wall.
const OCCLUSION_RECOVER_RATE: f32 = 4.0;

pub fn follow_camera_system(
    player_q: Query<(&Transform, &PlayerPhysics), (With<Player>, Without<FollowCamera>)>,
    mut cam_q: Query<(&mut Transform, &FollowCamera), Without<Player>>,
    occluders: Query<
        &crate::terrain::SolidBlock,
        With<crate::terrain::CameraOccluder>,
    >,
    orbit: Res<CameraOrbit>,
    diag: Option<Res<DiagState>>,
    mut smoothed: Local<Option<Vec3>>,
    mut occ_frac: Local<Option<f32>>,
    time: Res<Time>,
) {
    let Ok((player_tf, physics)) = player_q.get_single() else { return };
    let Ok((mut cam_tf, follow)) = cam_q.get_single_mut() else { return };
    let cam_mode = diag.map(|d| d.cam_mode).unwrap_or_default();
    let dt = time.delta_secs();
    let p = player_tf.translation;
    let s = *smoothed.get_or_insert(p);
    let new_s = if cam_mode == CamDiagMode::Rigid || s.distance_squared(p) > 100.0 {
        *occ_frac = None; // teleport: don't carry over a pulled-in distance
        p
    } else {
        let txz = (15.0 * dt).min(1.0);
        let ty_rate = if physics.grounded { 5.0 } else { 20.0 };
        let ty = (ty_rate * dt).min(1.0);
        Vec3::new(
            s.x + (p.x - s.x) * txz,
            s.y + (p.y - s.y) * ty,
            s.z + (p.z - s.z) * txz,
        )
    };
    *smoothed = Some(new_s);

    // Apply the orbit rotation directly (no position lerp) so mouse-look is
    // responsive. Follow smoothing already happens on `new_s` above; lerping
    // the translation too would double-smooth and add rotation input lag.
    let rot = Quat::from_rotation_y(orbit.yaw) * Quat::from_rotation_x(orbit.pitch);
    let target = new_s + rot * (follow.offset * orbit.zoom);

    // Occlusion: cast from the player's torso toward the desired camera spot
    // and clamp to the nearest tagged wall, so the camera dollies in front of
    // geometry instead of clipping inside it. Levels without CameraOccluder
    // entities skip the loop entirely.
    let ray_origin = new_s + follow.look_offset + Vec3::Y * OCCLUSION_ORIGIN_LIFT;
    let to_cam = target - ray_origin;
    let full = to_cam.length();
    let desired = if full > 1e-4 {
        let dir = to_cam / full;
        let mut allowed = full;
        for block in &occluders {
            if let Some(t) =
                block.ray_entry(ray_origin, dir, full, OCCLUSION_PAD_XZ, OCCLUSION_PAD_Y)
            {
                allowed = allowed.min(t);
            }
        }
        allowed = allowed.max(MIN_CAMERA_DISTANCE);
        // Track occlusion as a FRACTION of the desired distance, not an
        // absolute distance: zoom/pitch changes of the unoccluded camera then
        // apply instantly instead of lagging behind the recovery ease (an
        // absolute-distance ease made every zoom-out crawl at 4 u/s, which
        // read as a choppy/sticky camera on every level).
        let target_frac = (allowed / full).min(1.0);
        let f = occ_frac.get_or_insert(1.0);
        if target_frac < *f {
            *f = target_frac; // pull in instantly — never spend a frame in a wall
        } else {
            *f += (target_frac - *f) * (OCCLUSION_RECOVER_RATE * dt).min(1.0);
            if target_frac - *f < 0.01 {
                *f = target_frac; // settle so unoccluded frames don't keep easing
            }
        }
        ray_origin + dir * (*f * full)
    } else {
        target
    };
    // A/B diagnostic: `ExtraSmooth` restores the pre-July camera position lerp
    // on top of the follow smoothing (`FollowCamera::lerp_speed`).
    cam_tf.translation = if cam_mode == CamDiagMode::ExtraSmooth {
        let t = (follow.lerp_speed * dt).min(1.0);
        cam_tf.translation.lerp(desired, t)
    } else {
        desired
    };
    cam_tf.look_at(new_s + follow.look_offset, Vec3::Y);
}

#[allow(clippy::too_many_arguments)]
pub fn update_camera_orbit(
    gamepads: Query<&Gamepad>,
    time: Res<Time>,
    game_paused: Res<crate::GamePaused>,
    mut mouse_motion: EventReader<MouseMotion>,
    mut mouse_wheel: EventReader<MouseWheel>,
    windows: Query<&Window, With<PrimaryWindow>>,
    settings: Res<MouseSettings>,
    mut orbit: ResMut<CameraOrbit>,
) {
    if game_paused.0 {
        return;
    }
    const DEADZONE: f32 = 0.2;
    const YAW_RATE: f32 = 2.5;
    const PITCH_RATE: f32 = 1.5;
    const PITCH_CLAMP: f32 = 0.7;
    const TRIGGER_DEADZONE: f32 = 0.05;
    const ZOOM_RATE: f32 = 1.5;
    const ZOOM_MIN: f32 = 0.5;
    const ZOOM_MAX: f32 = 2.5;
    // Mouse sensitivities
    const MOUSE_YAW_SENS: f32 = 0.006;
    const MOUSE_PITCH_SENS: f32 = 0.005;
    const WHEEL_ZOOM_STEP: f32 = 0.12;
    let dt = time.delta_secs();
    for gp in &gamepads {
        let stick = gp.right_stick();
        if stick.x.abs() > DEADZONE {
            orbit.yaw -= stick.x * YAW_RATE * dt;
        }
        if stick.y.abs() > DEADZONE {
            orbit.pitch =
                (orbit.pitch + stick.y * PITCH_RATE * dt).clamp(-PITCH_CLAMP, PITCH_CLAMP);
        }
        // Only count a trigger when it's actually pressed past the deadzone.
        // Some controllers rest their trigger axes at a nonzero value (often
        // -1), which would otherwise drift the zoom every frame.
        let rt = gp.get(GamepadButton::RightTrigger2).unwrap_or(0.0);
        let lt = gp.get(GamepadButton::LeftTrigger2).unwrap_or(0.0);
        let rt = if rt > TRIGGER_DEADZONE { rt } else { 0.0 };
        let lt = if lt > TRIGGER_DEADZONE { lt } else { 0.0 };
        let zoom_delta = rt - lt;
        if zoom_delta != 0.0 {
            orbit.zoom = (orbit.zoom - zoom_delta * ZOOM_RATE * dt).clamp(ZOOM_MIN, ZOOM_MAX);
        }
    }

    // Mouse free-look: when the cursor is grabbed (see `manage_cursor_grab`)
    // raw motion turns the camera directly, no button needed. When the cursor
    // is released (menu / pause / modal), motion is ignored so it doesn't fight
    // the UI.
    let cursor_grabbed = windows
        .get_single()
        .map(|w| w.cursor_options.grab_mode != CursorGrabMode::None)
        .unwrap_or(false);
    let mut look = Vec2::ZERO;
    for ev in mouse_motion.read() {
        look += ev.delta;
    }
    if cursor_grabbed && look != Vec2::ZERO {
        let sens = settings.sensitivity;
        orbit.yaw -= look.x * MOUSE_YAW_SENS * sens;
        orbit.pitch =
            (orbit.pitch - look.y * MOUSE_PITCH_SENS * sens).clamp(-PITCH_CLAMP, PITCH_CLAMP);
    }

    // Mouse wheel zooms.
    let mut scroll = 0.0;
    for ev in mouse_wheel.read() {
        scroll += ev.y;
    }
    if scroll != 0.0 {
        orbit.zoom = (orbit.zoom - scroll * WHEEL_ZOOM_STEP).clamp(ZOOM_MIN, ZOOM_MAX);
    }
}

/// Detects the most-recently-used input device (last-used semantics) so the
/// controls legend can show the matching bindings.
pub fn detect_active_input(
    mut active: ResMut<ActiveInput>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: EventReader<MouseMotion>,
    gamepads: Query<&Gamepad>,
) {
    const WATCH: [GamepadButton; 12] = [
        GamepadButton::South,
        GamepadButton::East,
        GamepadButton::West,
        GamepadButton::North,
        GamepadButton::DPadUp,
        GamepadButton::DPadDown,
        GamepadButton::DPadLeft,
        GamepadButton::DPadRight,
        GamepadButton::Start,
        GamepadButton::Select,
        GamepadButton::LeftTrigger,
        GamepadButton::RightTrigger,
    ];
    let mut gamepad_active = false;
    for gp in &gamepads {
        if WATCH.iter().any(|b| gp.just_pressed(*b)) {
            gamepad_active = true;
        }
        if gp.left_stick().length() > 0.3 || gp.right_stick().length() > 0.3 {
            gamepad_active = true;
        }
        if gp.get(GamepadButton::RightTrigger2).unwrap_or(0.0) > 0.15
            || gp.get(GamepadButton::LeftTrigger2).unwrap_or(0.0) > 0.15
        {
            gamepad_active = true;
        }
    }
    if gamepad_active {
        active.gamepad = true;
        return;
    }

    // Keyboard/mouse activity flips back (ignore tiny mouse jitter).
    let mut drag = Vec2::ZERO;
    for ev in mouse_motion.read() {
        drag += ev.delta;
    }
    let mouse_moved = drag.length() > 2.0;
    if keyboard.get_just_pressed().next().is_some()
        || mouse_buttons.get_just_pressed().next().is_some()
        || mouse_moved
    {
        active.gamepad = false;
    }
}

/// Grabs and hides the cursor during active play so the mouse free-looks the
/// camera, and releases it when the cursor is needed for UI: in the menu, while
/// paused, when the tutorial modal is open, or when the window is unfocused.
pub fn manage_cursor_grab(
    screen: Res<State<crate::Screen>>,
    game_paused: Res<crate::GamePaused>,
    releaser_q: Query<&Node, With<CursorReleaser>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let Ok(mut window) = windows.get_single_mut() else {
        return;
    };
    let modal_open = releaser_q.iter().any(|n| !matches!(n.display, Display::None));
    let want_grab = *screen.get() != crate::Screen::Menu
        && !game_paused.0
        && !modal_open
        && window.focused;

    if want_grab {
        if window.cursor_options.grab_mode != CursorGrabMode::Locked {
            window.cursor_options.grab_mode = CursorGrabMode::Locked;
            window.cursor_options.visible = false;
        }
    } else if window.cursor_options.grab_mode != CursorGrabMode::None {
        window.cursor_options.grab_mode = CursorGrabMode::None;
        window.cursor_options.visible = true;
    }
}

// --- Diagnostics (F3 overlay, F4/F5 camera A/B modes) ---

/// Camera behavior under test. `Default` is the current shipping behavior,
/// `Rigid` disables all follow smoothing (camera glued to its target), and
/// `ExtraSmooth` re-adds the pre-July camera position lerp on top.
#[derive(Default, Clone, Copy, PartialEq)]
pub enum CamDiagMode {
    #[default]
    Default,
    Rigid,
    ExtraSmooth,
}

impl CamDiagMode {
    fn label(self) -> &'static str {
        match self {
            CamDiagMode::Default => "default",
            CamDiagMode::Rigid => "rigid (F4)",
            CamDiagMode::ExtraSmooth => "extra-smooth (F5)",
        }
    }
}

/// Runtime diagnostics state, toggled by hotkeys from any screen.
#[derive(Resource, Default)]
pub struct DiagState {
    pub overlay: bool,
    pub cam_mode: CamDiagMode,
}

#[derive(Component)]
pub struct DiagOverlayText;

/// F3 toggles the frame-time overlay; F4/F5 toggle the camera A/B modes
/// (pressing the active mode's key again returns to the default camera).
pub fn diag_hotkeys(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut diag: ResMut<DiagState>,
    mut commands: Commands,
    overlay_q: Query<Entity, With<DiagOverlayText>>,
) {
    if keyboard.just_pressed(KeyCode::F3) {
        diag.overlay = !diag.overlay;
        if diag.overlay && overlay_q.is_empty() {
            commands.spawn((
                Text::new(""),
                TextFont { font_size: 16.0, ..default() },
                TextColor(Color::srgb(0.4, 1.0, 0.4)),
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(8.0),
                    left: Val::Px(8.0),
                    ..default()
                },
                GlobalZIndex(100),
                DiagOverlayText,
            ));
        } else if !diag.overlay {
            for e in &overlay_q {
                commands.entity(e).despawn_recursive();
            }
        }
    }
    if keyboard.just_pressed(KeyCode::F4) {
        diag.cam_mode = if diag.cam_mode == CamDiagMode::Rigid {
            CamDiagMode::Default
        } else {
            CamDiagMode::Rigid
        };
    }
    if keyboard.just_pressed(KeyCode::F5) {
        diag.cam_mode = if diag.cam_mode == CamDiagMode::ExtraSmooth {
            CamDiagMode::Default
        } else {
            CamDiagMode::ExtraSmooth
        };
    }
}

/// Summary stats over a frame-time window: average, worst, and how many
/// frames deviated >50% from the window median (pacing spikes).
fn frame_stats(history: &[f32]) -> (f32, f32, usize) {
    let mut sorted = history.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = sorted[sorted.len() / 2];
    let avg = history.iter().sum::<f32>() / history.len() as f32;
    let worst = *sorted.last().unwrap();
    let spikes = history.iter().filter(|&&d| d > median * 1.5).count();
    (avg, worst, spikes)
}

/// Refreshes the overlay four times a second from [`FramePacing`]'s rolling
/// windows. `raw` is the measured frame loop time; `sim` is the delta the
/// game actually integrates with after smoothing — visible stutter with a
/// flat `sim` line points away from time-stepping, spiky `raw` with flat
/// `sim` means the smoother is absorbing pacing jitter as designed.
pub fn diag_overlay_update(
    real_time: Res<Time<Real>>,
    diag: Res<DiagState>,
    pacing: Res<crate::frame_pacing::FramePacing>,
    mut refresh: Local<f32>,
    mut text_q: Query<&mut Text, With<DiagOverlayText>>,
) {
    if !diag.overlay {
        *refresh = 0.0;
        return;
    }
    *refresh -= real_time.delta_secs();
    if *refresh > 0.0 || pacing.raw_history.len() < 10 || pacing.used_history.len() < 10 {
        return;
    }
    *refresh = 0.25;

    let (raw_avg, raw_worst, raw_spikes) = frame_stats(&pacing.raw_history);
    let (sim_avg, sim_worst, sim_spikes) = frame_stats(&pacing.used_history);

    if let Ok(mut text) = text_q.get_single_mut() {
        let s = format!(
            "fps {:>5.1}  refresh est {:>5.2}ms  cam: {}\nraw  avg {:>5.2}ms worst {:>6.2}ms spikes {:>3}/{}\nsim  avg {:>5.2}ms worst {:>6.2}ms spikes {:>3}/{}",
            1.0 / raw_avg,
            pacing.interval * 1000.0,
            diag.cam_mode.label(),
            raw_avg * 1000.0,
            raw_worst * 1000.0,
            raw_spikes,
            pacing.raw_history.len(),
            sim_avg * 1000.0,
            sim_worst * 1000.0,
            sim_spikes,
            pacing.used_history.len(),
        );
        if **text != s {
            **text = s;
        }
    }
}

/// Swaps a [`ControlsHint`]'s displayed text to match [`ActiveInput`].
pub fn update_controls_hint(
    active: Res<ActiveInput>,
    mut q: Query<(&mut Text, &ControlsHint)>,
) {
    for (mut text, hint) in &mut q {
        let want = if active.gamepad { &hint.gamepad } else { &hint.keyboard };
        if **text != *want {
            **text = want.clone();
        }
    }
}
