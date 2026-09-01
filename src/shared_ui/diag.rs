//! The F3 frame-pacing overlay (see CLAUDE.md: first stop for any
//! "choppy" or "mouse is wrong" report).

use bevy::prelude::*;

// --- Diagnostics (F3 frame-pacing overlay) ---

/// Runtime diagnostics state, toggled by hotkeys from any screen.
#[derive(Resource, Default)]
pub struct DiagState {
    pub overlay: bool,
}

#[derive(Component)]
pub struct DiagOverlayText;

/// F3 toggles the frame-time overlay.
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
    raw_mouse: Res<crate::raw_mouse::RawMouse>,
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
            "fps {:>5.1}  refresh est {:>5.2}ms  drift {:>+5.1}ms\nraw  avg {:>5.2}ms worst {:>6.2}ms spikes {:>3}/{}\nsim  avg {:>5.2}ms worst {:>6.2}ms spikes {:>3}/{}\nmouse {} delta {:>7.1},{:>7.1}",
            1.0 / raw_avg,
            pacing.interval * 1000.0,
            pacing.drift_ms(),
            raw_avg * 1000.0,
            raw_worst * 1000.0,
            raw_spikes,
            pacing.raw_history.len(),
            sim_avg * 1000.0,
            sim_worst * 1000.0,
            sim_spikes,
            pacing.used_history.len(),
            if raw_mouse.absolute { "absolute (rdp/vm)" } else { "relative        " },
            raw_mouse.delta.x,
            raw_mouse.delta.y,
        );
        if **text != s {
            **text = s;
        }
    }
}
