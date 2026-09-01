use bevy::prelude::*;

use crate::level_kit::{self, DefeatText, GameplaySet, LevelPhase, VictoryText};
use crate::player::{spawn_player, MovementBounds, Player, PlayerPhysics};
use crate::shared_ui;
use crate::{GamePaused, Screen};

pub const ID: u32 = 2;
const SCREEN: Screen = Screen::Level(ID);

/// Long-form walkthrough shown in the tutorial modal (opened with T).
const HEALTH_TUTORIAL: &str = "\
You start at 10 HP and items heal only up to 99, but winning needs 100 - so you must set your health in memory.

Approach 1 - memory search (health is an f32):
1) Read your current HP from the HUD (e.g. 10).
2) Search memory for that number as a 4-byte float.
3) Take a hit (-1) or grab an item (+1), then search again for the new value. Repeat until one address is left.
4) That address is PlayerHealth.current. Set it to 100. You win the instant it reaches 100.

Approach 2 - breakpoint on the check:
1) Break on check_health_victory (or apply_cannon_damage).
2) Inspect the PlayerHealth argument - current is an f32.
3) Set current = 100.0 and continue.";

pub fn register(app: &mut App) {
    app.add_systems(OnEnter(SCREEN), setup_cannon_arena)
        .add_systems(
            Update,
            cannon_playing_update
                .in_set(GameplaySet::Logic)
                .run_if(level_kit::in_phase(SCREEN, LevelPhase::Playing)),
        )
        .add_systems(
            Update,
            cannon_visual_update
                .in_set(GameplaySet::Logic)
                .run_if(in_state(SCREEN)),
        )
        // On death the incoming volley despawns with the overlay up; on
        // victory it despawns so the frozen scene isn't mid-barrage.
        .add_systems(
            OnEnter(LevelPhase::Defeat),
            clear_projectiles.run_if(in_state(SCREEN)),
        )
        .add_systems(
            OnEnter(LevelPhase::Victory),
            clear_projectiles.run_if(in_state(SCREEN)),
        )
        .add_systems(
            OnTransition { exited: LevelPhase::Defeat, entered: LevelPhase::Playing },
            reset_after_death.run_if(in_state(SCREEN)),
        )
        .add_systems(OnExit(SCREEN), level_kit::despawn_level::<CannonEntity>);
}

// --- Components ---

#[derive(Component, Clone, Copy)]
struct CannonEntity;

#[derive(Component)]
struct CannonPivot;

#[derive(Component)]
struct CannonProjectile {
    velocity: Vec3,
    lifetime: Timer,
}

#[derive(Component)]
struct HealthCube {
    respawn_timer: Option<Timer>,
}

#[derive(Component)]
struct HealthHudText;

#[derive(Component)]
struct HurtFlash;

// --- Resources ---

#[repr(C)]
#[derive(Resource)]
pub struct PlayerHealth {
    pub current: f32,
}

impl Default for PlayerHealth {
    fn default() -> Self {
        Self { current: START_HP }
    }
}

#[derive(Resource)]
struct FireTimer {
    timer: Timer,
}

#[derive(Resource, Default)]
struct HurtFlashState {
    timer: f32,
}

#[derive(Resource)]
struct ProjectileAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

// --- Constants ---

const ARENA_MIN: Vec2 = Vec2::new(-8.0, -6.0);
const ARENA_MAX: Vec2 = Vec2::new(8.0, 6.0);
const CANNON_POS: Vec3 = Vec3::new(0.0, 0.0, -3.0);
const START_HP: f32 = 10.0;
const DAMAGE_PER_HIT: f32 = 1.0;
const HEAL_AMOUNT: f32 = 1.0;
// Healing from items caps here (one below the win threshold), so the goal is
// unreachable through normal play â€” it needs the debugger.
const MAX_HEAL_HP: f32 = 99.0;
const WIN_HP: f32 = 100.0;
const PLAYER_SPAWN: Vec3 = Vec3::new(0.0, 0.0, 4.0);
const PROJECTILE_SPEED: f32 = 8.0;
const FIRE_INTERVAL: f32 = 2.0;

// --- Debugger-target functions ---

#[inline(never)]
fn check_health_victory(health: &PlayerHealth) -> bool {
    health.current >= WIN_HP
}

#[inline(never)]
fn apply_cannon_damage(health: &mut PlayerHealth, damage: f32) {
    health.current -= damage;
    if health.current < 0.0 {
        health.current = 0.0;
    }
}

#[inline(never)]
fn collect_health_cube(health: &mut PlayerHealth, heal_amount: f32) {
    health.current += heal_amount;
    if health.current > MAX_HEAL_HP {
        health.current = MAX_HEAL_HP;
    }
}

// --- Setup ---

fn setup_cannon_arena(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(ClearColor(Color::srgb(0.45, 0.55, 0.65)));
    commands.insert_resource(VictoryText::with_subtitle("VICTORY!", "HP reached 100!"));
    commands.insert_resource(DefeatText::new("YOU DIED"));
    commands.insert_resource(PlayerHealth::default());
    commands.insert_resource(FireTimer {
        timer: Timer::from_seconds(FIRE_INTERVAL, TimerMode::Repeating),
    });
    commands.insert_resource(HurtFlashState::default());

    let proj_mesh = meshes.add(Sphere::new(0.15));
    let proj_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.3, 0.1),
        emissive: LinearRgba::new(2.0, 0.5, 0.1, 1.0),
        ..default()
    });
    commands.insert_resource(ProjectileAssets {
        mesh: proj_mesh,
        material: proj_mat,
    });

    // Ground
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(16.0, 12.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.76, 0.7, 0.5),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
        CannonEntity,
    ));

    // Cannon base
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.2, 0.8, 1.2))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.25, 0.25, 0.28),
            ..default()
        })),
        Transform::from_xyz(CANNON_POS.x, 0.4, CANNON_POS.z),
        CannonEntity,
    ));

    // Cannon pivot + barrel
    let barrel_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.3, 0.33),
        ..default()
    });
    commands
        .spawn((
            Transform::from_xyz(CANNON_POS.x, 0.9, CANNON_POS.z),
            Visibility::default(),
            CannonPivot,
            CannonEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.2, 1.2))),
                MeshMaterial3d(barrel_mat),
                Transform::from_xyz(0.0, 0.0, -0.6)
                    .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            ));
        });

    // Player
    let player = spawn_player(&mut commands, &mut meshes, &mut materials);
    commands.entity(player).insert((
        CannonEntity,
        Transform::from_xyz(PLAYER_SPAWN.x, 0.0, PLAYER_SPAWN.z)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        MovementBounds {
            rects: vec![(ARENA_MIN, ARENA_MAX)],
        },
    ));

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 8.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        shared_ui::FollowCamera {
            offset: Vec3::new(0.0, 8.0, 10.0),
            
            look_offset: Vec3::Y,
        },
        CannonEntity,
    ));

    // Light
    shared_ui::setup_level_lighting(
        &mut commands,
        10000.0,
        (-std::f32::consts::FRAC_PI_4, std::f32::consts::FRAC_PI_6, 0.0),
        Color::srgb(0.9, 0.85, 0.8),
        350.0,
        CannonEntity,
    );

    // Health cubes (green)
    spawn_health_cubes(&mut commands, &mut meshes, &mut materials);

    // Hurt flash overlay (starts transparent)
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            position_type: PositionType::Absolute,
            ..default()
        },
        BackgroundColor(Color::srgba(0.8, 0.0, 0.0, 0.0)),
        GlobalZIndex(5),
        HurtFlash,
        CannonEntity,
    ));

    // HUD - health badge
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                left: Val::Px(16.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.9, 0.2, 0.15, 0.85)),
            BorderRadius::all(Val::Px(12.0)),
            CannonEntity,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("HP: 10"),
                TextFont { font_size: 22.0, ..default() },
                TextColor(Color::srgb(1.0, 1.0, 1.0)),
                HealthHudText,
            ));
        });

    // HUD - controls hint
    shared_ui::spawn_controls_hint(
        &mut commands,
        "Reach 100 health",
        CannonEntity,
    );

    // HUD - hint box + tutorial modal (hidden; H reveals the hint, T the tutorial)
    shared_ui::spawn_hint_box_with_tutorial(
        &mut commands,
        "Find the memory cell that holds your health and set it to 100.",
        320.0,
        CannonEntity,
    );
    shared_ui::spawn_hint_modal(
        &mut commands,
        "Health - Full Solution",
        HEALTH_TUTORIAL,
        CannonEntity,
    );
}

fn spawn_health_cubes(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let cube_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.9, 0.2),
        emissive: LinearRgba::new(0.2, 1.0, 0.2, 1.0),
        ..default()
    });
    let cube_mesh = meshes.add(Cuboid::new(0.4, 0.4, 0.4));

    let positions: [(f32, f32); 4] = [
        (-5.0, -4.0), (5.0, -4.0), (-5.0, 4.0), (5.0, 4.0),
    ];

    for &(x, z) in &positions {
        commands.spawn((
            Mesh3d(cube_mesh.clone()),
            MeshMaterial3d(cube_mat.clone()),
            Transform::from_xyz(x, 1.0, z)
                .with_rotation(Quat::from_euler(EulerRot::XYZ, 0.785, 0.785, 0.0)),
            HealthCube { respawn_timer: None },
            Visibility::default(),
            CannonEntity,
        ));
    }
}

// --- Single combined gameplay system (runs while Playing) ---

#[allow(clippy::too_many_arguments)]
fn cannon_playing_update(
    mut commands: Commands,
    time: Res<Time>,
    mut health: ResMut<PlayerHealth>,
    mut fire_timer: ResMut<FireTimer>,
    proj_assets: Res<ProjectileAssets>,
    mut next_phase: ResMut<NextState<LevelPhase>>,
    player_q: Query<
        &Transform,
        (With<Player>, Without<CannonPivot>, Without<CannonProjectile>, Without<HealthCube>),
    >,
    mut pivot_q: Query<
        (&mut Transform, &GlobalTransform),
        (With<CannonPivot>, Without<Player>, Without<CannonProjectile>, Without<HealthCube>),
    >,
    mut proj_q: Query<
        (Entity, &mut Transform, &mut CannonProjectile),
        (Without<Player>, Without<CannonPivot>, Without<HealthCube>),
    >,
    mut cube_q: Query<
        (&Transform, &mut HealthCube, &mut Visibility),
        (Without<Player>, Without<CannonPivot>, Without<CannonProjectile>),
    >,
    game_paused: Res<GamePaused>,
    mut hurt_flash: ResMut<HurtFlashState>,
) {
    if game_paused.0 {
        return;
    }
    let dt = time.delta_secs();

    let Ok(player_transform) = player_q.get_single() else {
        return;
    };
    let player_pos = player_transform.translation;

    // === Cannon aim ===
    if let Ok((mut pivot_t, _)) = pivot_q.get_single_mut() {
        let target = Vec3::new(player_pos.x, pivot_t.translation.y, player_pos.z);
        let diff = target - pivot_t.translation;
        if diff.length_squared() > 0.5 {
            pivot_t.look_at(target, Vec3::Y);
        }
    }

    // === Cannon fire ===
    fire_timer.timer.tick(time.delta());
    if fire_timer.timer.just_finished() {
        if let Ok((_, pivot_gt)) = pivot_q.get_single_mut() {
            let tip = pivot_gt.transform_point(Vec3::new(0.0, 0.0, -1.2));
            let target = player_pos + Vec3::Y;
            let direction = (target - tip).normalize_or_zero();
            let velocity = direction * PROJECTILE_SPEED;

            commands.spawn((
                Mesh3d(proj_assets.mesh.clone()),
                MeshMaterial3d(proj_assets.material.clone()),
                Transform::from_translation(tip),
                CannonProjectile {
                    velocity,
                    lifetime: Timer::from_seconds(4.0, TimerMode::Once),
                },
                CannonEntity,
            ));
        }
    }

    // === Projectile update ===
    let player_center = player_pos + Vec3::Y;
    for (entity, mut transform, mut proj) in &mut proj_q {
        proj.lifetime.tick(time.delta());
        if proj.lifetime.finished() {
            commands.entity(entity).despawn_recursive();
            continue;
        }
        transform.translation += proj.velocity * dt;
        if transform.translation.distance_squared(player_center) < 1.0 {
            apply_cannon_damage(&mut health, DAMAGE_PER_HIT);
            hurt_flash.timer = 0.3;
            commands.entity(entity).despawn_recursive();
        }
    }

    // === Cube collection + respawn ===
    for (cube_transform, mut cube, mut vis) in &mut cube_q {
        // Respawn timer
        if let Some(ref mut timer) = cube.respawn_timer {
            timer.tick(time.delta());
            if timer.finished() {
                *vis = Visibility::Inherited;
                cube.respawn_timer = None;
            }
            continue;
        }

        if *vis == Visibility::Hidden {
            continue;
        }

        // Proximity check for collection
        let dx = player_pos.x - cube_transform.translation.x;
        let dz = player_pos.z - cube_transform.translation.z;
        if dx * dx + dz * dz < 1.5 * 1.5 {
            collect_health_cube(&mut health, HEAL_AMOUNT);
            *vis = Visibility::Hidden;
            cube.respawn_timer = Some(Timer::from_seconds(15.0, TimerMode::Once));
        }
    }

    // === Victory / death check ===
    if check_health_victory(&health) {
        next_phase.set(LevelPhase::Victory);
    } else if health.current <= 0.0 {
        next_phase.set(LevelPhase::Defeat);
    }
}

// --- Single combined visual system (runs for the whole screen) ---

fn cannon_visual_update(
    time: Res<Time>,
    health: Res<PlayerHealth>,
    mut cube_q: Query<&mut Transform, With<HealthCube>>,
    mut text_q: Query<&mut Text, With<HealthHudText>>,
    mut hurt_flash: ResMut<HurtFlashState>,
    mut flash_q: Query<&mut BackgroundColor, With<HurtFlash>>,
) {
    let dt = time.delta_secs();
    let elapsed = time.elapsed_secs();

    // Hurt flash fade
    if hurt_flash.timer > 0.0 {
        hurt_flash.timer = (hurt_flash.timer - dt).max(0.0);
        let alpha = (hurt_flash.timer / 0.3).clamp(0.0, 1.0) * 0.45;
        if let Ok(mut bg) = flash_q.get_single_mut() {
            *bg = BackgroundColor(Color::srgba(0.8, 0.0, 0.0, alpha));
        }
    }

    // Cube animation
    for mut t in &mut cube_q {
        t.rotate_y(2.0 * dt);
        t.translation.y = 1.0 + ((elapsed * 2.0) + t.translation.x * 0.5).sin() * 0.2;
    }

    // HUD
    if let Ok(mut text) = text_q.get_single_mut() {
        **text = format!("HP: {:.0}", health.current);
    }
}

// --- Defeat / victory hooks (overlays and dismissal are the shared flow) ---

fn clear_projectiles(mut commands: Commands, projectile_q: Query<Entity, With<CannonProjectile>>) {
    for entity in &projectile_q {
        commands.entity(entity).despawn_recursive();
    }
}

fn reset_after_death(
    mut health: ResMut<PlayerHealth>,
    mut player_query: Query<(&mut Transform, &mut PlayerPhysics), With<Player>>,
) {
    health.current = START_HP;
    if let Ok((mut transform, mut physics)) = player_query.get_single_mut() {
        transform.translation = PLAYER_SPAWN;
        physics.velocity = Vec3::ZERO;
        physics.grounded = true;
        physics.facing = Quat::from_rotation_y(std::f32::consts::PI);
    }
}

// --- Cleanup ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_victory_requires_100() {
        assert!(!check_health_victory(&PlayerHealth { current: 10.0 }));
        assert!(!check_health_victory(&PlayerHealth { current: 99.0 }));
        assert!(check_health_victory(&PlayerHealth { current: 100.0 }));
        assert!(check_health_victory(&PlayerHealth { current: 200.0 }));
    }

    #[test]
    fn cannon_damage_reduces_health() {
        let mut hp = PlayerHealth { current: START_HP };
        apply_cannon_damage(&mut hp, DAMAGE_PER_HIT);
        assert_eq!(hp.current, 9.0);
    }

    #[test]
    fn cannon_damage_floors_at_zero() {
        let mut hp = PlayerHealth { current: 5.0 };
        apply_cannon_damage(&mut hp, 20.0);
        assert_eq!(hp.current, 0.0);
    }

    #[test]
    fn health_cube_heals_capped_at_99() {
        let mut hp = PlayerHealth { current: 50.0 };
        collect_health_cube(&mut hp, HEAL_AMOUNT);
        assert_eq!(hp.current, 51.0);

        // At the cap, further items add nothing.
        let mut hp2 = PlayerHealth { current: 99.0 };
        collect_health_cube(&mut hp2, HEAL_AMOUNT);
        assert_eq!(hp2.current, MAX_HEAL_HP);
    }

    #[test]
    fn win_impossible_through_normal_play() {
        // Items cap health at 99, but winning requires 100.
        let mut hp = PlayerHealth { current: 0.0 };
        for _ in 0..200 {
            collect_health_cube(&mut hp, HEAL_AMOUNT);
        }
        assert_eq!(hp.current, MAX_HEAL_HP);
        assert!(!check_health_victory(&hp));
    }

    #[test]
    fn debugger_scenario_set_health_to_100() {
        // Simulates: player sets breakpoint on check_health_victory,
        // modifies health.current to 100.
        let mut hp = PlayerHealth { current: START_HP };
        hp.current = WIN_HP; // debugger sets this
        assert!(check_health_victory(&hp));
    }
}
