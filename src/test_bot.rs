use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;

use crate::level101::HillState;
use crate::level2::PlayerHealth;
use crate::level3::BombTimer;
use crate::level5::RacerStats;
use crate::level_kit::LevelPhase;
use crate::password::ChallengePhase;
use crate::player::{Player, PlayerPhysics};
use crate::*;

pub struct TestBotPlugin;

/// How much faster than real time the bot plays. Physics reads
/// `time.delta_secs()` (virtual time), so the simulation is unchanged — each
/// wall-clock frame just advances further. Keep moderate: at 60 fps this
/// makes dt ≈ 0.067 s, and much larger steps would distort jump arcs.
const BOT_TIME_SPEED: f32 = 4.0;

fn speed_up_time(mut time: ResMut<Time<Virtual>>) {
    time.set_relative_speed(BOT_TIME_SPEED);
}

impl Plugin for TestBotPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(BotState::new())
            .add_systems(Startup, speed_up_time)
            .add_systems(
                Update,
                bot_menu_system.run_if(in_state(Screen::Menu)),
            )
            .add_systems(
                Update,
                bot_level1.run_if(in_state(Screen::Level(crate::level1::ID))),
            )
            .add_systems(
                Update,
                bot_level2.run_if(in_state(Screen::Level(crate::level2::ID))),
            )
            .add_systems(
                Update,
                bot_level3.run_if(in_state(Screen::Level(crate::level3::ID))),
            )
            .add_systems(
                Update,
                bot_level4.run_if(in_state(Screen::Level(crate::level4::ID))),
            )
            .add_systems(
                Update,
                bot_level5.run_if(in_state(Screen::Level(crate::level5::ID))),
            )
            .add_systems(
                Update,
                bot_level13.run_if(in_state(Screen::Level(crate::level101::ID))),
            )
            .add_systems(
                Update,
                bot_level103.run_if(in_state(Screen::Level(crate::level103::ID))),
            );
    }
}

// ─── Bot state ───

#[derive(PartialEq, Clone, Debug)]
enum BotPhase {
    SelectLevel,
    WaitForLoad,
    ApplyHack,
    Navigate,
    WaitForVictory,
    DismissVictory,
    ReturnToMenu,
    Done,
}

#[derive(Resource)]
struct BotState {
    /// The menu's full level list (hidden levels included); the bot visits
    /// every entry in order, tracked by `level_idx`.
    levels: Vec<u32>,
    level_idx: usize,
    phase: BotPhase,
    timer: f32,
    waypoint: usize,
    hack_applied: bool,
}

impl BotState {
    fn new() -> Self {
        Self {
            levels: crate::menu::visible_levels(true),
            level_idx: 0,
            phase: BotPhase::SelectLevel,
            timer: 0.5,
            waypoint: 0,
            hack_applied: false,
        }
    }

    fn current_level(&self) -> u32 {
        self.levels[self.level_idx]
    }

    fn enter_level(&mut self) {
        self.phase = BotPhase::WaitForLoad;
        self.timer = 0.5;
        self.waypoint = 0;
        self.hack_applied = false;
    }

    fn next_level(&mut self) {
        if self.level_idx + 1 < self.levels.len() {
            self.level_idx += 1;
            self.phase = BotPhase::SelectLevel;
            self.timer = 0.5;
        } else {
            self.phase = BotPhase::Done;
            info!("[TestBot] All levels completed!");
        }
    }
}

// ─── Helpers ───

fn navigate_toward(
    keyboard: &mut ButtonInput<KeyCode>,
    player_pos: Vec3,
    target: Vec3,
    threshold: f32,
) -> bool {
    let dx = target.x - player_pos.x;
    let dz = target.z - player_pos.z;

    if dx * dx + dz * dz < threshold * threshold {
        keyboard.release(KeyCode::KeyW);
        keyboard.release(KeyCode::KeyS);
        keyboard.release(KeyCode::KeyA);
        keyboard.release(KeyCode::KeyD);
        return true;
    }

    if dx > 0.3 {
        keyboard.press(KeyCode::KeyD);
        keyboard.release(KeyCode::KeyA);
    } else if dx < -0.3 {
        keyboard.press(KeyCode::KeyA);
        keyboard.release(KeyCode::KeyD);
    } else {
        keyboard.release(KeyCode::KeyA);
        keyboard.release(KeyCode::KeyD);
    }

    if dz < -0.3 {
        keyboard.press(KeyCode::KeyW);
        keyboard.release(KeyCode::KeyS);
    } else if dz > 0.3 {
        keyboard.press(KeyCode::KeyS);
        keyboard.release(KeyCode::KeyW);
    } else {
        keyboard.release(KeyCode::KeyW);
        keyboard.release(KeyCode::KeyS);
    }

    false
}

fn release_all_movement(keyboard: &mut ButtonInput<KeyCode>) {
    keyboard.release(KeyCode::KeyW);
    keyboard.release(KeyCode::KeyS);
    keyboard.release(KeyCode::KeyA);
    keyboard.release(KeyCode::KeyD);
    keyboard.release(KeyCode::Space);
}

fn send_key_char(writer: &mut EventWriter<KeyboardInput>, ch: char) {
    let smol = smol_str::SmolStr::new_inline(&ch.to_string());
    writer.send(KeyboardInput {
        key_code: KeyCode::Unidentified(bevy::input::keyboard::NativeKeyCode::Unidentified),
        logical_key: Key::Character(smol),
        state: ButtonState::Pressed,
        window: Entity::PLACEHOLDER,
        repeat: false,
    });
}

/// Sends a full press+release pair. The release matters: a Pressed-only
/// event leaves the key held in `ButtonInput<KeyCode>` forever, so any later
/// synthetic press of the same key never registers as `just_pressed` and
/// `just_pressed`-based handlers (e.g. victory overlays) stop responding.
fn send_key_press(writer: &mut EventWriter<KeyboardInput>, key: Key, code: KeyCode) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        writer.send(KeyboardInput {
            key_code: code,
            logical_key: key.clone(),
            state,
            window: Entity::PLACEHOLDER,
            repeat: false,
        });
    }
}

/// Shared `ReturnToMenu` phase tick: after the grace timer, send Escape.
/// Sent as a `KeyboardInput` event (not `ButtonInput::press`): events are
/// converted in PreUpdate, so `just_pressed` is visible to every system that
/// frame regardless of scheduling order.
fn tick_return_to_menu(bot: &mut BotState, dt: f32, writer: &mut EventWriter<KeyboardInput>) {
    bot.timer -= dt;
    if bot.timer <= 0.0 {
        send_key_press(writer, Key::Escape, KeyCode::Escape);
    }
}

/// Player input is camera-relative; pin the orbit so WASD stays axis-aligned
/// even if stray mouse motion reaches the window.
fn pin_orbit(orbit: &mut shared_ui::CameraOrbit) {
    orbit.yaw = 0.0;
    orbit.pitch = 0.0;
}

// ─── Menu system ───

fn bot_menu_system(
    time: Res<Time>,
    mut bot: ResMut<BotState>,
    mut writer: EventWriter<KeyboardInput>,
    mut next_screen: ResMut<NextState<Screen>>,
    mut app_exit: EventWriter<AppExit>,
    scoreboard: Res<Scoreboard>,
) {
    if bot.phase == BotPhase::Done {
        // Quit instead of idling at the menu burning CPU after the run.
        app_exit.send(AppExit::Success);
        return;
    }

    // After returning from a level, advance. A mid-level phase while we're
    // back at the menu means the level dismissed itself (e.g. a victory
    // screen consumed a buffered key) — count it as completed rather than
    // waiting forever on a state that can no longer arrive.
    let returned_unexpectedly = matches!(
        bot.phase,
        BotPhase::ApplyHack
            | BotPhase::Navigate
            | BotPhase::WaitForVictory
            | BotPhase::DismissVictory
    );
    if returned_unexpectedly {
        info!(
            "[TestBot] Level {} returned to menu on its own (phase {:?}) — advancing",
            bot.current_level(), bot.phase
        );
    }
    if bot.phase == BotPhase::ReturnToMenu || returned_unexpectedly {
        bot.next_level();
        if bot.phase == BotPhase::Done {
            return;
        }
    }

    if bot.phase != BotPhase::SelectLevel {
        return;
    }

    bot.timer -= time.delta_secs();
    if bot.timer > 0.0 {
        return;
    }

    // Skip already-solved levels
    while scoreboard.is_solved(bot.current_level()) {
        info!("[TestBot] Level {} already solved, skipping", bot.current_level());
        bot.next_level();
        if bot.phase == BotPhase::Done {
            return;
        }
    }

    info!("[TestBot] Selecting level {}", bot.current_level());

    let (ch, code) = match bot.current_level() {
        1 => ('1', KeyCode::Digit1),
        2 => ('2', KeyCode::Digit2),
        3 => ('3', KeyCode::Digit3),
        4 => ('4', KeyCode::Digit4),
        5 => ('5', KeyCode::Digit5),
        // Hidden levels have no keyboard shortcut — switch the state directly.
        level => {
            if let Some(screen) = crate::levels::screen_for_level(level) {
                next_screen.set(screen);
                bot.enter_level();
            }
            return;
        }
    };

    let smol = smol_str::SmolStr::new_inline(&ch.to_string());
    writer.send(KeyboardInput {
        key_code: code,
        logical_key: Key::Character(smol),
        state: ButtonState::Pressed,
        window: Entity::PLACEHOLDER,
        repeat: false,
    });

    bot.enter_level();
}

// ─── Level 1: Password ───

fn bot_level1(
    time: Res<Time>,
    mut bot: ResMut<BotState>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut writer: EventWriter<KeyboardInput>,
    mut orbit: ResMut<shared_ui::CameraOrbit>,
    player_q: Query<&Transform, With<Player>>,
    challenge_phase: Res<State<ChallengePhase>>,
    level_phase: Res<State<LevelPhase>>,
) {
    if bot.phase == BotPhase::Done {
        return;
    }
    pin_orbit(&mut orbit);

    // Wait for load
    if bot.phase == BotPhase::WaitForLoad {
        bot.timer -= time.delta_secs();
        if bot.timer <= 0.0 {
            bot.phase = BotPhase::Navigate;
            info!("[TestBot] Level 1: Walking to password zone");
        }
        return;
    }

    // Navigate to password zone (x >= 6.0)
    if bot.phase == BotPhase::Navigate || bot.phase == BotPhase::ApplyHack {
        if *challenge_phase.get() == ChallengePhase::PasswordPrompt {
            // We're in the password prompt - type "sesame" + Enter
            release_all_movement(&mut keyboard);
            if !bot.hack_applied {
                info!("[TestBot] Level 1: Typing password 'sesame'");
                for ch in "sesame".chars() {
                    send_key_char(&mut writer, ch);
                }
                send_key_press(&mut writer, Key::Enter, KeyCode::Enter);
                bot.hack_applied = true;
                bot.phase = BotPhase::WaitForVictory;
                bot.timer = 0.5;
            }
            return;
        }

        if *level_phase.get() == LevelPhase::Victory {
            release_all_movement(&mut keyboard);
            bot.phase = BotPhase::WaitForVictory;
            bot.timer = 0.5;
            return;
        }

        // Walk toward x=9.5 to enter password zone
        if let Ok(pt) = player_q.get_single() {
            let target = Vec3::new(9.5, 0.0, 0.0);
            navigate_toward(&mut keyboard, pt.translation, target, 1.0);
        }
        return;
    }

    // Wait for the shared victory phase
    if bot.phase == BotPhase::WaitForVictory {
        if *level_phase.get() == LevelPhase::Victory {
            bot.phase = BotPhase::DismissVictory;
            bot.timer = 0.5;
        }
        return;
    }

    // Dismiss victory
    if bot.phase == BotPhase::DismissVictory {
        bot.timer -= time.delta_secs();
        if bot.timer <= 0.0 {
            send_key_press(&mut writer, Key::Enter, KeyCode::Enter);
            bot.phase = BotPhase::ReturnToMenu;
            bot.timer = 0.3;
        }
        return;
    }

    // Return to menu
    if bot.phase == BotPhase::ReturnToMenu {
        tick_return_to_menu(&mut bot, time.delta_secs(), &mut writer);
    }
}

// ─── Level 2: Cannon (set health to 1000) ───

fn bot_level2(
    time: Res<Time>,
    mut bot: ResMut<BotState>,
    mut writer: EventWriter<KeyboardInput>,
    mut health: Option<ResMut<PlayerHealth>>,
    level_phase: Res<State<LevelPhase>>,
) {
    if bot.phase == BotPhase::Done {
        return;
    }

    if bot.phase == BotPhase::WaitForLoad {
        bot.timer -= time.delta_secs();
        if bot.timer <= 0.0 {
            bot.phase = BotPhase::ApplyHack;
        }
        return;
    }

    if bot.phase == BotPhase::ApplyHack {
        if let Some(ref mut h) = health {
            info!("[TestBot] Level 2: Setting health to 100");
            h.current = 100.0;
            bot.phase = BotPhase::WaitForVictory;
            bot.timer = 1.0;
        }
        return;
    }

    if bot.phase == BotPhase::WaitForVictory {
        if *level_phase.get() == LevelPhase::Victory {
            bot.phase = BotPhase::DismissVictory;
            bot.timer = 0.5;
        }
        return;
    }

    if bot.phase == BotPhase::DismissVictory {
        bot.timer -= time.delta_secs();
        if bot.timer <= 0.0 {
            send_key_press(&mut writer, Key::Enter, KeyCode::Enter);
            bot.phase = BotPhase::ReturnToMenu;
            bot.timer = 0.3;
        }
        return;
    }

    if bot.phase == BotPhase::ReturnToMenu {
        tick_return_to_menu(&mut bot, time.delta_secs(), &mut writer);
    }
}

// ─── Level 3: Countdown (set defused) ───

fn bot_level3(
    time: Res<Time>,
    mut bot: ResMut<BotState>,
    mut writer: EventWriter<KeyboardInput>,
    mut bomb: Option<ResMut<BombTimer>>,
    level_phase: Res<State<LevelPhase>>,
) {
    if bot.phase == BotPhase::Done {
        return;
    }

    if bot.phase == BotPhase::WaitForLoad {
        bot.timer -= time.delta_secs();
        if bot.timer <= 0.0 {
            bot.phase = BotPhase::ApplyHack;
        }
        return;
    }

    if bot.phase == BotPhase::ApplyHack {
        let Some(ref mut b) = bomb else { return };
        if bot.waypoint == 0 {
            // Pass 1: force an explosion to exercise the shared defeat
            // overlay and the OnTransition retry reset.
            info!("[TestBot] Level 3: Forcing an explosion to test defeat/retry");
            b.remaining = 0.01;
            bot.waypoint = 1;
            return;
        }
        if *level_phase.get() == LevelPhase::Defeat {
            // Dismiss (sent every frame until the overlay accepts it).
            send_key_press(&mut writer, Key::Enter, KeyCode::Enter);
            return;
        }
        if *level_phase.get() == LevelPhase::Playing && b.remaining > 1.0 {
            // Retry landed: the hook must have reset the fuse.
            if b.defused {
                error!("[TestBot] Level 3: Retry did NOT reset the bomb (defused=true)");
            } else {
                info!(
                    "[TestBot] Level 3: Defeat/retry OK (fuse reset to {:.1})",
                    b.remaining
                );
            }
            info!("[TestBot] Level 3: Setting bomb defused = true");
            b.defused = true;
            bot.phase = BotPhase::WaitForVictory;
            bot.timer = 1.0;
        }
        return;
    }

    if bot.phase == BotPhase::WaitForVictory {
        if *level_phase.get() == LevelPhase::Victory {
            bot.phase = BotPhase::DismissVictory;
            bot.timer = 0.5;
        }
        return;
    }

    if bot.phase == BotPhase::DismissVictory {
        bot.timer -= time.delta_secs();
        if bot.timer <= 0.0 {
            send_key_press(&mut writer, Key::Enter, KeyCode::Enter);
            bot.phase = BotPhase::ReturnToMenu;
            bot.timer = 0.3;
        }
        return;
    }

    if bot.phase == BotPhase::ReturnToMenu {
        tick_return_to_menu(&mut bot, time.delta_secs(), &mut writer);
    }
}

// ─── Level 4: Maze (teleport to trophy) ───

fn bot_level4(
    time: Res<Time>,
    mut bot: ResMut<BotState>,
    mut writer: EventWriter<KeyboardInput>,
    mut orbit: ResMut<shared_ui::CameraOrbit>,
    mut player_q: Query<&mut Transform, With<Player>>,
    level_phase: Res<State<LevelPhase>>,
) {
    if bot.phase == BotPhase::Done {
        return;
    }
    pin_orbit(&mut orbit);

    if bot.phase == BotPhase::WaitForLoad {
        bot.timer -= time.delta_secs();
        if bot.timer <= 0.0 {
            bot.phase = BotPhase::ApplyHack;
        }
        return;
    }

    if bot.phase == BotPhase::ApplyHack {
        if let Ok(mut pt) = player_q.get_single_mut() {
            info!("[TestBot] Level 4: Teleporting to trophy");
            pt.translation = Vec3::new(12.0, 0.5, -10.0);
            bot.phase = BotPhase::WaitForVictory;
            bot.timer = 1.0;
        }
        return;
    }

    if bot.phase == BotPhase::WaitForVictory {
        if *level_phase.get() == LevelPhase::Victory {
            bot.phase = BotPhase::DismissVictory;
            bot.timer = 0.5;
        }
        return;
    }

    if bot.phase == BotPhase::DismissVictory {
        bot.timer -= time.delta_secs();
        if bot.timer <= 0.0 {
            send_key_press(&mut writer, Key::Enter, KeyCode::Enter);
            bot.phase = BotPhase::ReturnToMenu;
            bot.timer = 0.3;
        }
        return;
    }

    if bot.phase == BotPhase::ReturnToMenu {
        tick_return_to_menu(&mut bot, time.delta_secs(), &mut writer);
    }
}

// ─── Level 5: Race (boost speed + run laps) ───

fn bot_level5(
    time: Res<Time>,
    mut bot: ResMut<BotState>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut writer: EventWriter<KeyboardInput>,
    mut stats: Option<ResMut<RacerStats>>,
    mut orbit: ResMut<shared_ui::CameraOrbit>,
    mut player_q: Query<&mut Transform, With<Player>>,
    level_phase: Res<State<LevelPhase>>,
) {
    if bot.phase == BotPhase::Done {
        return;
    }
    // The race movement frame must stay world-aligned for waypoint driving
    // (and must stay pinned even if stray mouse motion reaches the window).
    pin_orbit(&mut orbit);

    if bot.phase == BotPhase::WaitForLoad {
        bot.timer -= time.delta_secs();
        if bot.timer <= 0.0 {
            bot.phase = BotPhase::ApplyHack;
        }
        return;
    }

    if bot.phase == BotPhase::ApplyHack {
        let Some(ref mut s) = stats else { return };
        if bot.waypoint == 0 {
            // Pass 1: lose fast — crank the AI so they finish in about a
            // second, exercising the shared defeat overlay and the retry
            // back into the Frozen countdown (`DefeatText::retry_to`).
            info!("[TestBot] Level 5: Boosting AI to lose fast (defeat/retry test)");
            s.ai_speed = 150.0;
            bot.waypoint = 1;
            bot.phase = BotPhase::Navigate;
            return;
        }
        // Pass 2: freeze the AI and teleport near the finish (one of the
        // level's intended hack strategies) so the winning drive is a few
        // units, not the whole 150-unit track.
        info!("[TestBot] Level 5: Freezing AI and teleporting near the finish");
        s.ai_speed = 0.0;
        if let Ok(mut pt) = player_q.get_single_mut() {
            pt.translation = Vec3::new(3.0, 0.0, -140.0);
        }
        bot.phase = BotPhase::Navigate;
        return;
    }

    if bot.phase == BotPhase::Navigate {
        if *level_phase.get() == LevelPhase::Victory {
            release_all_movement(&mut keyboard);
            bot.phase = BotPhase::DismissVictory;
            bot.timer = 0.5;
            return;
        }
        if *level_phase.get() == LevelPhase::Defeat {
            if bot.waypoint == 1 {
                info!("[TestBot] Level 5: Lost as expected — retrying into the countdown");
                bot.waypoint = 2;
            }
            release_all_movement(&mut keyboard);
            // Dismiss (sent every frame until the overlay accepts it).
            send_key_press(&mut writer, Key::Enter, KeyCode::Enter);
            return;
        }
        if *level_phase.get() == LevelPhase::Frozen && bot.waypoint == 2 {
            // Retry re-entered the countdown: now apply the real hack.
            info!("[TestBot] Level 5: Defeat/retry OK — back in the countdown");
            bot.waypoint = 3;
            bot.phase = BotPhase::WaitForLoad;
            bot.timer = 0.5;
            return;
        }

        // Navigate straight down the track to the finish line (hold W). With the
        // AI frozen at 0 progress there's no time pressure to cover all 150 units.
        let target = Vec3::new(3.0, 0.0, -148.0);

        if let Ok(pt) = player_q.get_single() {
            navigate_toward(&mut keyboard, pt.translation, target, 2.0);
        }
        return;
    }

    if bot.phase == BotPhase::WaitForVictory {
        if *level_phase.get() == LevelPhase::Victory {
            bot.phase = BotPhase::DismissVictory;
            bot.timer = 0.5;
        }
        return;
    }

    if bot.phase == BotPhase::DismissVictory {
        bot.timer -= time.delta_secs();
        if bot.timer <= 0.0 {
            send_key_press(&mut writer, Key::Enter, KeyCode::Enter);
            bot.phase = BotPhase::ReturnToMenu;
            bot.timer = 0.3;
        }
        return;
    }

    if bot.phase == BotPhase::ReturnToMenu {
        tick_return_to_menu(&mut bot, time.delta_secs(), &mut writer);
    }
}

// ─── Level 13: Hill Fortress (unlock gate + teleport to summit) ───

fn bot_level13(
    time: Res<Time>,
    mut bot: ResMut<BotState>,
    mut writer: EventWriter<KeyboardInput>,
    mut hill_state: Option<ResMut<HillState>>,
    mut player_q: Query<&mut Transform, With<Player>>,
    level_phase: Res<State<LevelPhase>>,
) {
    if bot.phase == BotPhase::Done {
        return;
    }

    if bot.phase == BotPhase::WaitForLoad {
        bot.timer -= time.delta_secs();
        if bot.timer <= 0.0 {
            bot.phase = BotPhase::ApplyHack;
        }
        return;
    }

    if bot.phase == BotPhase::ApplyHack {
        if let Some(ref mut hs) = hill_state {
            info!("[TestBot] Level 13: Setting gate_locked = false");
            hs.gate_locked = false;
        }
        // Teleport player to summit
        if let Ok(mut pt) = player_q.get_single_mut() {
            info!("[TestBot] Level 13: Teleporting to summit");
            pt.translation = Vec3::new(0.0, 10.5, 0.0);
        }
        bot.phase = BotPhase::WaitForVictory;
        bot.timer = 1.0;
        return;
    }

    if bot.phase == BotPhase::WaitForVictory {
        if *level_phase.get() == LevelPhase::Victory {
            bot.phase = BotPhase::DismissVictory;
            bot.timer = 0.5;
        }
        return;
    }

    if bot.phase == BotPhase::DismissVictory {
        bot.timer -= time.delta_secs();
        if bot.timer <= 0.0 {
            send_key_press(&mut writer, Key::Enter, KeyCode::Enter);
            bot.phase = BotPhase::ReturnToMenu;
            bot.timer = 0.3;
        }
        return;
    }

    if bot.phase == BotPhase::ReturnToMenu {
        tick_return_to_menu(&mut bot, time.delta_secs(), &mut writer);
    }
}

/// Per-level probe timeout, in virtual seconds (8 wall-clock seconds).
const PROBE_TIMEOUT: f32 = 8.0 * BOT_TIME_SPEED;

// ─── Level 103: Waterpark (deck → slide → pool run) ───
//
// Exercises the shared terrain collision end-to-end: walking the deck, the
// step-down ease onto the first slide segment, the slide ride, and the pool
// splash. The bot teleports onto the deck (same style as the level 4/101
// hacks) — jump-climbing the 1 m stair risers is too timing-sensitive for a
// bot, and the slide run is what covers the terrain engine.

fn bot_level103(
    time: Res<Time>,
    mut bot: ResMut<BotState>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut writer: EventWriter<KeyboardInput>,
    mut orbit: ResMut<shared_ui::CameraOrbit>,
    mut player_q: Query<(&mut Transform, &mut PlayerPhysics), With<Player>>,
    mut elapsed: Local<f32>,
) {
    if bot.phase == BotPhase::Done {
        return;
    }
    let dt = time.delta_secs();
    pin_orbit(&mut orbit);

    if bot.phase == BotPhase::WaitForLoad {
        bot.timer -= dt;
        if bot.timer <= 0.0 {
            bot.phase = BotPhase::ApplyHack;
        }
        return;
    }

    if bot.phase == BotPhase::ApplyHack {
        let Ok((mut tf, mut physics)) = player_q.get_single_mut() else { return };
        info!("[TestBot] Level 103: Teleporting onto the deck, riding a slide to the pool");
        tf.translation = Vec3::new(0.0, crate::level103::DECK_TOP, -16.0);
        physics.velocity = Vec3::ZERO;
        *elapsed = 0.0;
        bot.phase = BotPhase::Navigate;
        return;
    }

    if bot.phase == BotPhase::Navigate {
        let Ok((tf, _)) = player_q.get_single() else { return };
        let p = tf.translation;
        *elapsed += dt;

        if *elapsed > PROBE_TIMEOUT {
            release_all_movement(&mut keyboard);
            error!(
                "[TestBot] Level 103 probe FAILED: timed out (pos {:.1} {:.1} {:.1})",
                p.x, p.y, p.z
            );
            bot.phase = BotPhase::ReturnToMenu;
            bot.timer = 0.3;
            return;
        }

        // Walk south over the deck edge onto the center slide; the slide
        // then carries the player down into the pool.
        navigate_toward(&mut keyboard, p, Vec3::new(0.0, 0.0, -6.0), 1.5);

        // Success: the slide dropped us into the pool (past the slide
        // bottoms, inside the pool's XZ extents).
        if p.y < 0.0
            && p.z > crate::level103::SLIDE_BOTTOM_Z + 0.1
            && p.z < crate::level103::POOL_Z
            && p.x.abs() < crate::level103::POOL_X
        {
            release_all_movement(&mut keyboard);
            info!(
                "[TestBot] Level 103 probe OK: rode the slide into the pool (y={:.2})",
                p.y
            );
            bot.phase = BotPhase::ReturnToMenu;
            bot.timer = 0.3;
        }
        return;
    }

    if bot.phase == BotPhase::ReturnToMenu {
        tick_return_to_menu(&mut bot, dt, &mut writer);
    }
}
