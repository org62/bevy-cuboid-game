//! Shared per-level scaffolding: the fixed frame ordering, the shared
//! [`LevelPhase`] state machine, the core systems every level runs, the
//! victory/defeat flow, and the between-level resource sweep — all installed
//! ONCE by [`install`]. A level plugin registers only what is unique to it.
//!
//! **Why the ordering exists.** The player transform is written by several
//! systems in one frame (movement integrates it, terrain collision resolves
//! it, scripted motion overrides it) and read by the follow camera. Wiring
//! that by hand with `.after(...)` left the camera unordered against terrain
//! collision, so it could frame the pre-collision player position — the raw
//! ballistic one that dips into geometry — which reads as camera judder on
//! stairs and hills. [`GameplaySet`] makes the ordering a property of the
//! engine instead of something each new level has to remember.
//!
//! A level module's shape:
//!
//! ```ignore
//! pub const ID: u32 = 7;
//! const SCREEN: Screen = Screen::Level(ID);
//!
//! pub fn register(app: &mut App) {
//!     app.add_systems(OnEnter(SCREEN), setup_foo)
//!         .add_systems(
//!             Update,
//!             foo_logic
//!                 .in_set(GameplaySet::Logic)
//!                 .run_if(level_kit::in_phase(SCREEN, LevelPhase::Playing)),
//!         )
//!         .add_systems(OnExit(SCREEN), level_kit::despawn_level::<FooEntity>);
//! }
//! ```
//!
//! To win, a level sets `NextState<LevelPhase>` to `Victory` (after inserting
//! a [`VictoryText`] in setup for its own wording); the shared flow marks the
//! scoreboard from the current `Screen::Level(id)`, shows the overlay and
//! returns to the menu on any key. `Defeat` works the same via [`DefeatText`],
//! whose `retry_to` says which phase the retry re-enters; level-specific
//! retry resets hook `OnTransition { exited: Defeat, entered: retry_to }`
//! gated on their own screen.

use bevy::prelude::*;
use bevy::state::state::FreelyMutableState;

use crate::levels;
use crate::player::{
    animate_player, escape_to_menu, player_movement, toggle_pause, GravityOverride,
    GroundYOverride, PauseOverlay, PlayerMovementSet, PowerUpState,
};
use crate::shared_ui::{self, FollowCamera, OverlayScreen, TextInputActive};
use crate::terrain::{terrain_collision, TerrainConfig};
use crate::{GamePaused, Scoreboard, Screen};

// --- Frame ordering ---

/// The fixed order in which a frame of gameplay resolves. Configured once by
/// [`install`]; levels place their own systems in the set that matches what
/// the system *does*, and ordering follows for free.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameplaySet {
    /// Look/orbit input. Before movement, so this frame's input steers this
    /// frame's motion (movement is camera-relative).
    Input,
    /// `player_movement`: integrates velocity into the player transform.
    Movement,
    /// `terrain_collision`: resolves the integrated transform against level
    /// geometry. Everything downstream sees the *settled* position.
    Collision,
    /// Level-specific scripted motion that overrides the resolved position —
    /// slides, ziplines, teleporters, cutscenes. Must run before `Camera`, or
    /// the camera trails the script by a frame.
    Scripted,
    /// Systems that only *read* the final player transform: the follow camera
    /// and the player's squash/bob animation.
    Camera,
    /// Level logic reacting to the settled frame: goal checks, timers, HUD,
    /// pickups, pause/exit.
    Logic,
}

// --- The shared per-level phase ---

/// The phase every level runs in, as a sub-state that exists on any
/// `Screen::Level(_)`. One shared type (instead of a per-level enum) is what
/// lets the sim systems and the victory/defeat flow be registered once,
/// globally.
///
/// Two different gates, deliberately:
/// - the simulation (look input, movement, collision, pause/exit) runs only
///   in `Playing`, so it freezes behind a victory/defeat/prompt overlay;
/// - the visuals (follow camera, player animation) run in *every* phase, so
///   the scene stays alive and framed while such an overlay is up.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum LevelPhase {
    /// The level froze the sim behind its own overlay or cutscene — the
    /// race's 3-2-1 countdown, the password prompt. The scene stays alive.
    Frozen,
    #[default]
    Playing,
    /// Solved. The shared flow marks the scoreboard, shows the victory
    /// overlay and returns to the menu on any key.
    Victory,
    /// Failed. The shared flow shows the defeat overlay and re-enters
    /// `DefeatText::retry_to` on any key.
    Defeat,
}

impl States for LevelPhase {
    const DEPENDENCY_DEPTH: usize = <Screen as States>::DEPENDENCY_DEPTH + 1;
}

impl SubStates for LevelPhase {
    type SourceStates = Screen;

    fn should_exist(source: Screen) -> Option<Self> {
        match source {
            // Whether a level opens frozen (scripted intro) is roster data,
            // so the initial phase can never disagree with the level.
            Screen::Level(id) => Some(if levels::info(id).is_some_and(|l| l.starts_frozen) {
                LevelPhase::Frozen
            } else {
                LevelPhase::Playing
            }),
            Screen::Menu => None,
        }
    }
}

impl FreelyMutableState for LevelPhase {}

// --- Run-condition helpers ---

/// Run condition: any level is active (in any phase). The visual half of the
/// kit runs under this, so scenes stay alive behind overlays.
pub fn in_any_level(phase: Option<Res<State<LevelPhase>>>) -> bool {
    phase.is_some()
}

/// Run condition: `screen` is active *and* its [`LevelPhase`] is `phase`.
/// The standard gate for a level's own phase-specific systems.
pub fn in_phase(
    screen: Screen,
    phase: LevelPhase,
) -> impl FnMut(Option<Res<State<Screen>>>, Option<Res<State<LevelPhase>>>) -> bool + Clone {
    move |s, p| {
        s.is_some_and(|s| *s.get() == screen) && p.is_some_and(|p| *p.get() == phase)
    }
}

// --- Victory / defeat configuration ---

/// Wording of the shared victory overlay. Insert in the level's setup; when
/// absent the flow falls back to these defaults. Swept away on returning to
/// the menu.
#[derive(Resource, Clone)]
pub struct VictoryText {
    pub title: String,
    pub subtitle: Option<String>,
    pub subtitle_font_size: f32,
}

impl Default for VictoryText {
    fn default() -> Self {
        Self { title: "LEVEL COMPLETE!".into(), subtitle: None, subtitle_font_size: 22.0 }
    }
}

impl VictoryText {
    pub fn new(title: &str) -> Self {
        Self { title: title.into(), ..default() }
    }

    pub fn with_subtitle(title: &str, subtitle: &str) -> Self {
        Self { title: title.into(), subtitle: Some(subtitle.into()), ..default() }
    }
}

/// Wording of the shared defeat overlay, plus which phase a retry re-enters
/// (`Playing` for most levels; the race goes back to its `Frozen` countdown).
/// Insert in the level's setup. Swept away on returning to the menu.
#[derive(Resource, Clone)]
pub struct DefeatText {
    pub title: String,
    pub title_font_size: f32,
    pub subtitle: Option<String>,
    pub subtitle_font_size: f32,
    pub background: Color,
    pub retry_to: LevelPhase,
}

impl Default for DefeatText {
    fn default() -> Self {
        Self {
            title: "YOU FAILED".into(),
            title_font_size: 52.0,
            subtitle: None,
            subtitle_font_size: 28.0,
            background: Color::srgba(0.2, 0.0, 0.0, 0.8),
            retry_to: LevelPhase::Playing,
        }
    }
}

impl DefeatText {
    pub fn new(title: &str) -> Self {
        Self { title: title.into(), ..default() }
    }

    pub fn with_subtitle(title: &str, subtitle: &str) -> Self {
        Self { title: title.into(), subtitle: Some(subtitle.into()), ..default() }
    }
}

// --- Install ---

/// Install everything every level shares. Called once from `main`.
pub fn install(app: &mut App) {
    app.configure_sets(
        Update,
        (
            GameplaySet::Input,
            GameplaySet::Movement,
            GameplaySet::Collision,
            GameplaySet::Scripted,
            GameplaySet::Camera,
            GameplaySet::Logic,
        )
            .chain(),
    );
    app.add_sub_state::<LevelPhase>();

    app.add_systems(
        Update,
        (
            // Orbit look input, only where a `FollowCamera` exists to obey
            // it: movement is orbit-relative, so on a level with a fixed
            // bespoke camera (the race) a live orbit would invisibly rotate
            // the player's movement frame while the view stays put.
            shared_ui::update_camera_orbit
                .in_set(GameplaySet::Input)
                .run_if(in_state(LevelPhase::Playing).and(any_with_component::<FollowCamera>)),
            player_movement
                .in_set(PlayerMovementSet)
                .in_set(GameplaySet::Movement)
                .run_if(in_state(LevelPhase::Playing)),
            // No-op on levels that never insert a `TerrainConfig`, so a new
            // level gets terrain collision just by inserting the resource.
            terrain_collision
                .in_set(GameplaySet::Collision)
                .run_if(in_state(LevelPhase::Playing)),
            // Visuals for the whole screen: the scene stays alive and framed
            // behind victory/defeat/prompt overlays. Levels with a bespoke
            // camera (the race chase cam) simply spawn a camera without a
            // `FollowCamera` component and the shared system no-ops.
            (animate_player, shared_ui::follow_camera_system)
                .chain()
                .in_set(GameplaySet::Camera)
                .run_if(in_any_level),
            (escape_to_menu, toggle_pause)
                .in_set(GameplaySet::Logic)
                .run_if(in_state(LevelPhase::Playing)),
            shared_ui::hint_tutorial_controls
                .in_set(GameplaySet::Logic)
                .run_if(in_any_level.and(not(resource_exists::<TextInputActive>))),
            victory_flow
                .in_set(GameplaySet::Logic)
                .run_if(in_state(LevelPhase::Victory)),
            defeat_flow
                .in_set(GameplaySet::Logic)
                .run_if(in_state(LevelPhase::Defeat)),
        ),
    );

    app.add_systems(OnEnter(Screen::Menu), reset_between_levels);
}

// --- Victory / defeat flow ---

fn any_input_just_pressed(keyboard: &ButtonInput<KeyCode>, gamepads: &Query<&Gamepad>) -> bool {
    keyboard.get_just_pressed().next().is_some()
        || gamepads.iter().any(|g| g.get_just_pressed().next().is_some())
}

/// Marks the scoreboard from the current `Screen::Level(id)` (no level can
/// mark the wrong id), shows the victory overlay, and returns to the menu on
/// any key or gamepad button.
fn victory_flow(
    mut commands: Commands,
    screen: Res<State<Screen>>,
    text: Option<Res<VictoryText>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut scoreboard: ResMut<Scoreboard>,
    mut next_screen: ResMut<NextState<Screen>>,
    overlay_q: Query<Entity, With<OverlayScreen>>,
) {
    if overlay_q.is_empty() {
        if let Screen::Level(id) = *screen.get() {
            scoreboard.set_solved(id);
        }
        let default_text = VictoryText::default();
        let t = text.as_deref().unwrap_or(&default_text);
        shared_ui::spawn_victory_overlay(
            &mut commands,
            &t.title,
            t.subtitle.as_deref(),
            t.subtitle_font_size,
            "Press any key to continue",
            (),
        );
        // Give the overlay a frame on screen before accepting a dismissal.
        return;
    }
    if any_input_just_pressed(&keyboard, &gamepads) {
        for entity in &overlay_q {
            commands.entity(entity).despawn_recursive();
        }
        next_screen.set(Screen::Menu);
    }
}

/// Shows the defeat overlay and re-enters `DefeatText::retry_to` on any key.
/// Level-specific retry resets hook `OnTransition { exited: Defeat,
/// entered: retry_to }`, gated on their own screen.
fn defeat_flow(
    mut commands: Commands,
    text: Option<Res<DefeatText>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut next_phase: ResMut<NextState<LevelPhase>>,
    overlay_q: Query<Entity, With<OverlayScreen>>,
) {
    let default_text = DefeatText::default();
    let t = text.as_deref().unwrap_or(&default_text);
    if overlay_q.is_empty() {
        shared_ui::spawn_defeat_overlay(
            &mut commands,
            &t.title,
            t.title_font_size,
            t.subtitle.as_deref(),
            t.subtitle_font_size,
            "Press any key to retry",
            t.background,
            (),
        );
        return;
    }
    if any_input_just_pressed(&keyboard, &gamepads) {
        for entity in &overlay_q {
            commands.entity(entity).despawn_recursive();
        }
        next_phase.set(t.retry_to);
    }
}

// --- Between-level sweep ---

/// Runs on every return to the menu. Structurally removes every global a
/// level could have leaked — resources read by *shared* systems change
/// physics in whatever level is entered next (level 5 once leaked a
/// 2x-speed `PowerUpState` this way), so their cleanup must not depend on
/// each level remembering an `OnExit` line. Also resets pause/orbit and
/// despawns any surviving overlay.
fn reset_between_levels(
    mut commands: Commands,
    mut game_paused: ResMut<GamePaused>,
    mut camera_orbit: ResMut<shared_ui::CameraOrbit>,
    pause_q: Query<Entity, With<PauseOverlay>>,
    overlay_q: Query<Entity, With<OverlayScreen>>,
) {
    game_paused.0 = false;
    camera_orbit.yaw = 0.0;
    camera_orbit.pitch = 0.0;
    camera_orbit.zoom = 1.0;
    for entity in pause_q.iter().chain(overlay_q.iter()) {
        commands.entity(entity).despawn_recursive();
    }
    // Globals read by shared systems (removing an absent resource is a no-op).
    commands.remove_resource::<TerrainConfig>();
    commands.remove_resource::<GroundYOverride>();
    commands.remove_resource::<GravityOverride>();
    commands.remove_resource::<PowerUpState>();
    // Kit-owned per-level configuration.
    commands.remove_resource::<VictoryText>();
    commands.remove_resource::<DefeatText>();
    commands.remove_resource::<TextInputActive>();
}

// --- Entity cleanup ---

/// Despawn everything a level tagged with its marker component `M`.
///
/// Register it in the level's `OnExit`:
///
/// ```ignore
/// .add_systems(OnExit(SCREEN), level_kit::despawn_level::<FooEntity>)
/// ```
///
/// Entities whose parent also carries `M` are skipped: the parent's
/// `despawn_recursive` already takes them with it, and despawning them a
/// second time is what produces Bevy's `B0003 … doesn't exist in this World`
/// warning spam on level exit. A marked child of an *un*marked parent is still
/// despawned, so nothing leaks either way.
pub fn despawn_level<M: Component>(
    mut commands: Commands,
    roots: Query<(Entity, Option<&Parent>), With<M>>,
    marked: Query<(), With<M>>,
) {
    for (entity, parent) in &roots {
        if let Some(parent) = parent {
            if marked.contains(parent.get()) {
                continue;
            }
        }
        commands.entity(entity).despawn_recursive();
    }
}
