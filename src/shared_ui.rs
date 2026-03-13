use bevy::prelude::*;

use crate::player::Player;

// --- Shared components ---

#[derive(Component)]
pub struct HintBox;

#[derive(Component)]
pub struct HintCloseButton;

#[derive(Component)]
pub struct OverlayScreen;

#[derive(Component)]
pub struct FollowCamera {
    pub offset: Vec3,
    pub lerp_speed: f32,
    pub look_offset: Vec3,
}

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

pub fn dismiss_hint(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    hint_q: Query<Entity, With<HintBox>>,
    btn_q: Query<&Interaction, (Changed<Interaction>, With<HintCloseButton>)>,
) {
    let should_close = keyboard.just_pressed(KeyCode::KeyX)
        || btn_q.iter().any(|i| *i == Interaction::Pressed);
    if should_close {
        for entity in &hint_q {
            commands.entity(entity).despawn_recursive();
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

pub fn spawn_controls_hint(commands: &mut Commands, text: &str, extra: impl Bundle) {
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
                Text::new(text),
                TextFont { font_size: 20.0, ..default() },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
            ));
        });
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

pub fn follow_camera_system(
    player_q: Query<&Transform, (With<Player>, Without<FollowCamera>)>,
    mut cam_q: Query<(&mut Transform, &FollowCamera), Without<Player>>,
    time: Res<Time>,
) {
    let Ok(player_tf) = player_q.get_single() else { return };
    let Ok((mut cam_tf, follow)) = cam_q.get_single_mut() else { return };
    let target_pos = player_tf.translation + follow.offset;
    let t = (follow.lerp_speed * time.delta_secs()).min(1.0);
    cam_tf.translation = cam_tf.translation.lerp(target_pos, t);
    cam_tf.look_at(player_tf.translation + follow.look_offset, Vec3::Y);
}
