use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;

use crate::level10::PlayerLoot;
use crate::level11::CloneData;
use crate::level12::{FinalState, GuardianNpc, PlayerWeight, QuicksandState, VaultDoor};
use crate::level2::PlayerHealth;
use crate::level3::BombTimer;
use crate::level5::RacerStats;
use crate::level6::Inventory;
use crate::level9::Fighter;
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
                bot_level6.run_if(in_state(Screen::ChestChallenge)),
            )
            .add_systems(
                Update,
                bot_level7.run_if(in_state(Screen::GravityChallenge)),
            )
            .add_systems(
                Update,
                bot_level8.run_if(in_state(Screen::TollChallenge)),
            )
            .add_systems(
                Update,
                bot_level9.run_if(in_state(Screen::ArenaChallenge)),
            )
            .add_systems(
                Update,
                bot_level10.run_if(in_state(Screen::LootChallenge)),
            )
            .add_systems(
                Update,
                bot_level11.run_if(in_state(Screen::CloneChallenge)),
            )
            .add_systems(
                Update,
                bot_level12.run_if(in_state(Screen::FinalChallenge)),
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
        if self.current_level > 12 {
            self.phase = BotPhase::Done;
            info!("[TestBot] All 12 levels completed!");
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
    while bot.current_level <= 12 && scoreboard.is_solved(bot.current_level) {
        info!("[TestBot] Level {} already solved, skipping", bot.current_level);
        bot.current_level += 1;
    }
    if bot.current_level > 12 {
        bot.phase = BotPhase::Done;
        info!("[TestBot] All 12 levels completed!");
        return;
    }

    info!("[TestBot] Selecting level {}", bot.current_level);

    let (ch, code) = match bot.current_level {
        1 => ('1', KeyCode::Digit1),
        2 => ('2', KeyCode::Digit2),
        3 => ('3', KeyCode::Digit3),
        4 => ('4', KeyCode::Digit4),
        5 => ('5', KeyCode::Digit5),
        6 => ('6', KeyCode::Digit6),
        7 => ('7', KeyCode::Digit7),
        8 => ('8', KeyCode::Digit8),
        9 => ('9', KeyCode::Digit9),
        10 => ('0', KeyCode::Digit0),
        11 => ('-', KeyCode::Minus),
        12 => ('=', KeyCode::Equal),
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
            info!("[TestBot] Level 2: Setting health to 1000");
            h.current = 1000;
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
            info!("[TestBot] Level 5: Boosting player speed to 50");
            s.player_speed = 50.0;
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

        // Navigate around the oval track using waypoints
        const NUM_WAYPOINTS: usize = 16;
        let wp_idx = bot.waypoint % NUM_WAYPOINTS;
        let progress = wp_idx as f32 / NUM_WAYPOINTS as f32;
        let angle = progress * std::f32::consts::TAU;
        let target = Vec3::new(
            angle.cos() * 10.0,
            0.0,
            angle.sin() * 6.0,
        );

        if let Ok(pt) = player_q.get_single() {
            if navigate_toward(&mut keyboard, pt.translation, target, 2.0) {
                bot.waypoint += 1;
            }
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

// ─── Level 6: Chests (set key count + walk to chests) ───

fn bot_level6(
    time: Res<Time>,
    mut bot: ResMut<BotState>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut writer: EventWriter<KeyboardInput>,
    mut inventory: Option<ResMut<Inventory>>,
    player_q: Query<&Transform, With<Player>>,
    chest_phase: Res<State<ChestPhase>>,
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
        if let Some(ref mut inv) = inventory {
            info!("[TestBot] Level 6: Setting key count to 5");
            inv.keys.count = 5;
            bot.phase = BotPhase::Navigate;
            bot.waypoint = 0;
        }
        return;
    }

    if bot.phase == BotPhase::Navigate {
        if *chest_phase.get() == ChestPhase::Victory {
            release_all_movement(&mut keyboard);
            bot.phase = BotPhase::DismissVictory;
            bot.timer = 0.5;
            return;
        }

        let chest_positions: [Vec3; 5] = [
            Vec3::new(-4.0, 0.0, -2.5),
            Vec3::new(-2.0, 0.0, -2.5),
            Vec3::new(0.0, 0.0, -2.5),
            Vec3::new(2.0, 0.0, -2.5),
            Vec3::new(4.0, 0.0, -2.5),
        ];

        if bot.waypoint < 5 {
            let target = chest_positions[bot.waypoint];
            if let Ok(pt) = player_q.get_single() {
                if navigate_toward(&mut keyboard, pt.translation, target, 1.0) {
                    // Press E to open chest
                    release_all_movement(&mut keyboard);
                    keyboard.press(KeyCode::KeyE);
                    bot.waypoint += 1;
                    bot.timer = 0.3;
                } else {
                    keyboard.release(KeyCode::KeyE);
                }
            }
        } else {
            // All chests visited, wait for victory
            release_all_movement(&mut keyboard);
            keyboard.release(KeyCode::KeyE);
            bot.phase = BotPhase::WaitForVictory;
        }
        return;
    }

    if bot.phase == BotPhase::WaitForVictory {
        if *chest_phase.get() == ChestPhase::Victory {
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

// ─── Level 7: Gravity (teleport to top) ───

fn bot_level7(
    time: Res<Time>,
    mut bot: ResMut<BotState>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut writer: EventWriter<KeyboardInput>,
    mut player_q: Query<&mut Transform, With<Player>>,
    gravity_phase: Res<State<GravityPhase>>,
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
            info!("[TestBot] Level 7: Teleporting to y=31");
            pt.translation = Vec3::new(0.0, 31.0, 0.0);
            bot.phase = BotPhase::WaitForVictory;
            bot.timer = 1.0;
        }
        return;
    }

    if bot.phase == BotPhase::WaitForVictory {
        if *gravity_phase.get() == GravityPhase::Victory {
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

// ─── Level 8: Toll (set gold + walk bridge) ───

fn bot_level8(
    time: Res<Time>,
    mut bot: ResMut<BotState>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut writer: EventWriter<KeyboardInput>,
    mut wallet: Option<ResMut<crate::level8::PlayerWallet>>,
    player_q: Query<&Transform, With<Player>>,
    toll_phase: Res<State<TollPhase>>,
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
        if let Some(ref mut w) = wallet {
            info!("[TestBot] Level 8: Setting gold to 10000");
            w.gold = 10000;
            bot.phase = BotPhase::Navigate;
            bot.waypoint = 0;
        }
        return;
    }

    if bot.phase == BotPhase::Navigate {
        if *toll_phase.get() == TollPhase::Victory {
            release_all_movement(&mut keyboard);
            keyboard.release(KeyCode::KeyE);
            bot.phase = BotPhase::DismissVictory;
            bot.timer = 0.5;
            return;
        }

        // Walk along bridge, pressing E at toll positions
        // Toll Z positions: -4.0, -10.0, -16.0, then past -19.0
        let waypoints: [Vec3; 4] = [
            Vec3::new(0.0, 0.0, -4.5),
            Vec3::new(0.0, 0.0, -10.5),
            Vec3::new(0.0, 0.0, -16.5),
            Vec3::new(0.0, 0.0, -20.0),
        ];

        if bot.waypoint < 4 {
            let target = waypoints[bot.waypoint];
            if let Ok(pt) = player_q.get_single() {
                if navigate_toward(&mut keyboard, pt.translation, target, 1.0) {
                    release_all_movement(&mut keyboard);
                    keyboard.press(KeyCode::KeyE);
                    bot.waypoint += 1;
                    bot.timer = 0.3;
                } else {
                    keyboard.release(KeyCode::KeyE);
                }
            }
        } else {
            release_all_movement(&mut keyboard);
            keyboard.release(KeyCode::KeyE);
            bot.phase = BotPhase::WaitForVictory;
        }
        return;
    }

    if bot.phase == BotPhase::WaitForVictory {
        if *toll_phase.get() == TollPhase::Victory {
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

// ─── Level 9: Arena (kill enemies) ───

fn bot_level9(
    time: Res<Time>,
    mut bot: ResMut<BotState>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut writer: EventWriter<KeyboardInput>,
    mut fighters: Query<&mut Fighter>,
    arena_phase: Res<State<ArenaPhase>>,
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
        info!("[TestBot] Level 9: Killing all enemies");
        for mut f in &mut fighters {
            if f.team == 2 {
                f.health = 0.0;
            }
        }
        bot.phase = BotPhase::WaitForVictory;
        bot.timer = 1.0;
        return;
    }

    if bot.phase == BotPhase::WaitForVictory {
        if *arena_phase.get() == ArenaPhase::Victory {
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

// ─── Level 10: Loot (set golden key + walk to door) ───

fn bot_level10(
    time: Res<Time>,
    mut bot: ResMut<BotState>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut writer: EventWriter<KeyboardInput>,
    mut loot: Option<ResMut<PlayerLoot>>,
    player_q: Query<&Transform, With<Player>>,
    loot_phase: Res<State<LootPhase>>,
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
        if let Some(ref mut l) = loot {
            info!("[TestBot] Level 10: Setting has_golden_key = true");
            l.has_golden_key = true;
            bot.phase = BotPhase::Navigate;
        }
        return;
    }

    if bot.phase == BotPhase::Navigate {
        if *loot_phase.get() == LootPhase::Victory {
            release_all_movement(&mut keyboard);
            bot.phase = BotPhase::DismissVictory;
            bot.timer = 0.5;
            return;
        }

        // Walk to exit door at (6.5, 0, 0)
        if let Ok(pt) = player_q.get_single() {
            let target = Vec3::new(6.5, 0.0, 0.0);
            navigate_toward(&mut keyboard, pt.translation, target, 1.0);
        }
        return;
    }

    if bot.phase == BotPhase::WaitForVictory {
        if *loot_phase.get() == LootPhase::Victory {
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

// ─── Level 11: Clone (flip invincible + walk to mirror position) ───

fn bot_level11(
    time: Res<Time>,
    mut bot: ResMut<BotState>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut writer: EventWriter<KeyboardInput>,
    mut clone_q: Query<&mut CloneData>,
    player_q: Query<&Transform, With<Player>>,
    clone_phase: Res<State<ClonePhase>>,
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
        for mut cd in &mut clone_q {
            info!("[TestBot] Level 11: Setting invincible = false");
            cd.invincible = false;
        }
        bot.phase = BotPhase::Navigate;
        return;
    }

    if bot.phase == BotPhase::Navigate {
        if *clone_phase.get() == ClonePhase::Victory {
            release_all_movement(&mut keyboard);
            bot.phase = BotPhase::DismissVictory;
            bot.timer = 0.5;
            return;
        }

        // Walk player to (-10, 0, 0) so clone mirrors to (10, 0, 0) in trap zone
        if let Ok(pt) = player_q.get_single() {
            let target = Vec3::new(-10.0, 0.0, 0.0);
            navigate_toward(&mut keyboard, pt.translation, target, 1.0);
        }
        return;
    }

    if bot.phase == BotPhase::WaitForVictory {
        if *clone_phase.get() == ClonePhase::Victory {
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

// ─── Level 12: Final Challenge (4 rooms) ───

#[allow(clippy::too_many_arguments)]
fn bot_level12(
    time: Res<Time>,
    mut bot: ResMut<BotState>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut writer: EventWriter<KeyboardInput>,
    final_state: Option<ResMut<FinalState>>,
    mut quicksand: Option<ResMut<QuicksandState>>,
    mut vault_door: Option<ResMut<VaultDoor>>,
    mut player_weight: Option<ResMut<PlayerWeight>>,
    mut guardians: Query<&mut GuardianNpc>,
    player_q: Query<&Transform, With<Player>>,
    final_phase: Res<State<FinalPhase>>,
) {
    if bot.phase == BotPhase::Done {
        return;
    }

    if bot.phase == BotPhase::WaitForLoad {
        bot.timer -= time.delta_secs();
        if bot.timer <= 0.0 {
            bot.phase = BotPhase::ApplyHack;
            bot.waypoint = 0;
        }
        return;
    }

    // For level 12, we handle rooms sequentially in ApplyHack + Navigate
    if bot.phase == BotPhase::ApplyHack || bot.phase == BotPhase::Navigate {
        if *final_phase.get() == FinalPhase::Victory {
            release_all_movement(&mut keyboard);
            bot.phase = BotPhase::DismissVictory;
            bot.timer = 0.5;
            return;
        }

        let current_room = final_state.as_ref().map(|s| s.current_room).unwrap_or(0);

        match current_room {
            // Room 1: Type access code "888220" + Enter
            0 => {
                if !bot.hack_applied {
                    info!("[TestBot] Level 12 Room 1: Typing code 888220");
                    for ch in "888220".chars() {
                        send_key_char(&mut writer, ch);
                    }
                    send_key_press(&mut writer, Key::Enter, KeyCode::Enter);
                    bot.hack_applied = true;
                    bot.timer = 0.5;
                } else {
                    bot.timer -= time.delta_secs();
                    if bot.timer <= 0.0 {
                        // Room should have advanced; reset hack flag for next room
                        bot.hack_applied = false;
                        bot.timer = 0.5;
                    }
                }
            }

            // Room 2: Set sink_rate = 0, navigate to platform, jump on it
            1 => {
                if !bot.hack_applied {
                    if let Some(ref mut qs) = quicksand {
                        info!("[TestBot] Level 12 Room 2: Setting sink_rate = 0");
                        qs.sink_rate = 0.0;
                        bot.hack_applied = true;
                        bot.phase = BotPhase::Navigate;
                    }
                    return;
                }

                // Navigate to platform at room_offset(1) + (4, 0, 0) = (18, 0, 0)
                if let Ok(pt) = player_q.get_single() {
                    let target = Vec3::new(18.0, 0.0, 0.0);
                    if navigate_toward(&mut keyboard, pt.translation, target, 1.5) {
                        // Jump onto platform
                        keyboard.press(KeyCode::Space);
                        bot.timer = 0.5;
                    }
                }

                // Check if we advanced
                if final_state.as_ref().map(|s| s.current_room).unwrap_or(1) > 1 {
                    release_all_movement(&mut keyboard);
                    keyboard.release(KeyCode::Space);
                    bot.hack_applied = false;
                    bot.timer = 0.5;
                }
            }

            // Room 3: Kill guardians + unlock vault
            2 => {
                if !bot.hack_applied {
                    info!("[TestBot] Level 12 Room 3: Killing guardians + unlocking vault");
                    for mut g in &mut guardians {
                        g.health = 0.0;
                    }
                    if let Some(ref mut vd) = vault_door {
                        vd.lock.locked = false;
                    }
                    bot.hack_applied = false; // will re-check
                    bot.phase = BotPhase::Navigate;
                    // Mark as applied via waypoint
                    bot.waypoint = 100;
                }

                // Wait for room to advance
                if final_state.as_ref().map(|s| s.current_room).unwrap_or(2) > 2 {
                    bot.hack_applied = false;
                    bot.waypoint = 0;
                    bot.timer = 0.5;
                } else if bot.waypoint == 100 {
                    // Re-apply each frame until it takes
                    for mut g in &mut guardians {
                        g.health = 0.0;
                    }
                    if let Some(ref mut vd) = vault_door {
                        vd.lock.locked = false;
                    }
                }
            }

            // Room 4: Set penalty to 100
            3 => {
                if let Some(ref mut pw) = player_weight {
                    info!("[TestBot] Level 12 Room 4: Setting _penalty = 100.0");
                    pw._penalty = 100.0;
                    bot.phase = BotPhase::WaitForVictory;
                }
            }

            _ => {}
        }
        return;
    }

    if bot.phase == BotPhase::WaitForVictory {
        if *final_phase.get() == FinalPhase::Victory {
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
