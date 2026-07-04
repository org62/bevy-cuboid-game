use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;

use crate::level101::HillState;
use crate::level2::PlayerHealth;
use crate::level3::BombTimer;
use crate::level5::RacerStats;
use crate::player::Player;
use crate::*;

pub struct TestBotPlugin;

impl Plugin for TestBotPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(BotState::new())
            .add_systems(
                Update,
                bot_menu_system.run_if(in_state(Screen::Menu)),
            )
            .add_systems(
                Update,
                bot_level1.run_if(in_state(Screen::PasswordChallenge)),
            )
            .add_systems(
                Update,
                bot_level2.run_if(in_state(Screen::CannonChallenge)),
            )
            .add_systems(
                Update,
                bot_level3.run_if(in_state(Screen::CountdownChallenge)),
            )
            .add_systems(
                Update,
                bot_level4.run_if(in_state(Screen::MazeChallenge)),
            )
            .add_systems(
                Update,
                bot_level5.run_if(in_state(Screen::RaceChallenge)),
            )
            .add_systems(
                Update,
                bot_level13.run_if(in_state(Screen::HillChallenge)),
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
    current_level: u32,
    phase: BotPhase,
    timer: f32,
    waypoint: usize,
    hack_applied: bool,
}

impl BotState {
    fn new() -> Self {
        Self {
            current_level: 1,
            phase: BotPhase::SelectLevel,
            timer: 0.5,
            waypoint: 0,
            hack_applied: false,
        }
    }

    fn enter_level(&mut self) {
        self.phase = BotPhase::WaitForLoad;
        self.timer = 0.5;
        self.waypoint = 0;
        self.hack_applied = false;
    }

    fn next_level(&mut self) {
        self.current_level += 1;
        if self.current_level > 5 {
            self.phase = BotPhase::Done;
            info!("[TestBot] All levels completed!");
        } else {
            self.phase = BotPhase::SelectLevel;
            self.timer = 0.5;
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

fn send_key_press(writer: &mut EventWriter<KeyboardInput>, key: Key, code: KeyCode) {
    writer.send(KeyboardInput {
        key_code: code,
        logical_key: key,
        state: ButtonState::Pressed,
        window: Entity::PLACEHOLDER,
        repeat: false,
    });
}

// ─── Menu system ───

fn bot_menu_system(
    time: Res<Time>,
    mut bot: ResMut<BotState>,
    mut writer: EventWriter<KeyboardInput>,
    scoreboard: Res<Scoreboard>,
) {
    if bot.phase == BotPhase::Done {
        return;
    }

    // After returning from a level, advance
    if bot.phase == BotPhase::ReturnToMenu {
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
    while bot.current_level <= 5 && scoreboard.is_solved(bot.current_level) {
        info!("[TestBot] Level {} already solved, skipping", bot.current_level);
        bot.current_level += 1;
    }
    if bot.current_level > 5 {
        bot.phase = BotPhase::Done;
        info!("[TestBot] All levels completed!");
        return;
    }

    info!("[TestBot] Selecting level {}", bot.current_level);

    let (ch, code) = match bot.current_level {
        1 => ('1', KeyCode::Digit1),
        2 => ('2', KeyCode::Digit2),
        3 => ('3', KeyCode::Digit3),
        4 => ('4', KeyCode::Digit4),
        5 => ('5', KeyCode::Digit5),
        _ => return,
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
    player_q: Query<&Transform, With<Player>>,
    challenge_phase: Res<State<ChallengePhase>>,
) {
    if bot.phase == BotPhase::Done {
        return;
    }

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

        if *challenge_phase.get() == ChallengePhase::AccessGranted {
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

    // Wait for victory / AccessGranted
    if bot.phase == BotPhase::WaitForVictory {
        if *challenge_phase.get() == ChallengePhase::AccessGranted {
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
        bot.timer -= time.delta_secs();
        if bot.timer <= 0.0 {
            keyboard.press(KeyCode::Escape);
        }
    }
}

// ─── Level 2: Cannon (set health to 1000) ───

fn bot_level2(
    time: Res<Time>,
    mut bot: ResMut<BotState>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut writer: EventWriter<KeyboardInput>,
    mut health: Option<ResMut<PlayerHealth>>,
    cannon_phase: Res<State<CannonPhase>>,
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
        if *cannon_phase.get() == CannonPhase::Victory {
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
        bot.timer -= time.delta_secs();
        if bot.timer <= 0.0 {
            keyboard.press(KeyCode::Escape);
        }
    }
}

// ─── Level 3: Countdown (set defused) ───

fn bot_level3(
    time: Res<Time>,
    mut bot: ResMut<BotState>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut writer: EventWriter<KeyboardInput>,
    mut bomb: Option<ResMut<BombTimer>>,
    countdown_phase: Res<State<CountdownPhase>>,
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
        if let Some(ref mut b) = bomb {
            info!("[TestBot] Level 3: Setting bomb defused = true");
            b.defused = true;
            bot.phase = BotPhase::WaitForVictory;
            bot.timer = 1.0;
        }
        return;
    }

    if bot.phase == BotPhase::WaitForVictory {
        if *countdown_phase.get() == CountdownPhase::Victory {
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
        bot.timer -= time.delta_secs();
        if bot.timer <= 0.0 {
            keyboard.press(KeyCode::Escape);
        }
    }
}

// ─── Level 4: Maze (teleport to trophy) ───

fn bot_level4(
    time: Res<Time>,
    mut bot: ResMut<BotState>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut writer: EventWriter<KeyboardInput>,
    mut player_q: Query<&mut Transform, With<Player>>,
    maze_phase: Res<State<MazePhase>>,
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
        if let Ok(mut pt) = player_q.get_single_mut() {
            info!("[TestBot] Level 4: Teleporting to trophy");
            pt.translation = Vec3::new(12.0, 0.5, -10.0);
            bot.phase = BotPhase::WaitForVictory;
            bot.timer = 1.0;
        }
        return;
    }

    if bot.phase == BotPhase::WaitForVictory {
        if *maze_phase.get() == MazePhase::Victory {
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
        bot.timer -= time.delta_secs();
        if bot.timer <= 0.0 {
            keyboard.press(KeyCode::Escape);
        }
    }
}

// ─── Level 5: Race (boost speed + run laps) ───

fn bot_level5(
    time: Res<Time>,
    mut bot: ResMut<BotState>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut writer: EventWriter<KeyboardInput>,
    mut stats: Option<ResMut<RacerStats>>,
    player_q: Query<&Transform, With<Player>>,
    race_phase: Res<State<RacePhase>>,
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
        if let Some(ref mut s) = stats {
            info!("[TestBot] Level 5: Freezing AI speed to 0 so they can't finish");
            s.ai_speed = 0.0;
            bot.phase = BotPhase::Navigate;
            bot.waypoint = 0;
        }
        return;
    }

    if bot.phase == BotPhase::Navigate {
        if *race_phase.get() == RacePhase::Victory {
            release_all_movement(&mut keyboard);
            bot.phase = BotPhase::DismissVictory;
            bot.timer = 0.5;
            return;
        }
        if *race_phase.get() == RacePhase::Lost {
            // Dismiss lost overlay and retry
            send_key_press(&mut writer, Key::Enter, KeyCode::Enter);
            bot.phase = BotPhase::WaitForLoad;
            bot.timer = 0.5;
            bot.hack_applied = false;
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
        if *race_phase.get() == RacePhase::Victory {
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
        bot.timer -= time.delta_secs();
        if bot.timer <= 0.0 {
            keyboard.press(KeyCode::Escape);
        }
    }
}

// ─── Level 13: Hill Fortress (unlock gate + teleport to summit) ───

fn bot_level13(
    time: Res<Time>,
    mut bot: ResMut<BotState>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut writer: EventWriter<KeyboardInput>,
    mut hill_state: Option<ResMut<HillState>>,
    mut player_q: Query<&mut Transform, With<Player>>,
    hill_phase: Res<State<HillPhase>>,
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
        if *hill_phase.get() == HillPhase::Victory {
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
        bot.timer -= time.delta_secs();
        if bot.timer <= 0.0 {
            keyboard.press(KeyCode::Escape);
        }
    }
}
