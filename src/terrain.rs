//! Shared terrain collision for levels with non-flat geometry.
//!
//! One parameterized implementation of the swept player-vs-surface collision
//! described in CLAUDE.md, used by the hill (101), meadow (102) and waterpark
//! (103) levels. The invariants it upholds:
//!
//! - **Always sweep, never point-sample.** Surfaces are tested against both
//!   the previous and current XZ position (`prev = pos - velocity * dt`), and
//!   the vertical tolerance grows with `|vy| * dt` so fast motion cannot
//!   tunnel between or through surfaces.
//! - **Never snap up while jumping.** Every snap/ease path is gated on
//!   `velocity.y <= 0`.
//! - **Step-down snapping** keeps a statically-grounded player glued to the
//!   ground across small ledges instead of floating off cell edges, gated
//!   above the void floor (`floor_y`).
//! - **Ease only grounded steps; never ease a tunneling catch.** Grounded
//!   step-up/step-down transitions move at `step_ease_rate` so walking over
//!   uneven ground is smooth, but airborne landings, swept crossings and
//!   phased-below rescues snap instantly — easing those would let the player
//!   visibly pass through geometry.
//!
//! Levels opt in by inserting a [`TerrainConfig`] resource in their `OnEnter`
//! setup (and removing it on `OnExit`) and registering [`terrain_collision`]
//! after `player_movement` in their update chain. Without the resource the
//! system is a no-op, so flat levels are unaffected.

use bevy::prelude::*;

use crate::player::{Player, PlayerPhysics, SquashState};

/// Player body height used for wall-overlap and ceiling checks.
const PLAYER_BODY_H: f32 = 1.8;
/// Minimum vertical overlap before a wall pushes the player out horizontally
/// (a head barely grazing a block shouldn't cause XZ pushout).
const MIN_WALL_OVERLAP: f32 = 0.3;

// --- Components ---

/// A walkable surface top. `min`/`max` are XZ bounds, `y` is the surface
/// height the player's feet rest at. Must equal the visible mesh top
/// (CLAUDE.md: mesh and collision derive from the same value).
#[derive(Component)]
pub struct TerrainSurface {
    pub min: Vec2,
    pub max: Vec2,
    pub y: f32,
}

/// Solid wall collision box — blocks horizontal movement and jumps through
/// its underside. `min`/`max` are XZ bounds, `y_min`/`y_max` are vertical
/// bounds.
#[derive(Component)]
pub struct SolidBlock {
    pub min: Vec2,
    pub max: Vec2,
    pub y_min: f32,
    pub y_max: f32,
}

/// A water-slide segment that auto-carries the player along `direction` at
/// [`SLIDE_CARRY_SPEED`]. The per-level slide systems consume this; it lives
/// here so levels don't each redefine the type or the carry tuning.
#[derive(Component)]
pub struct WaterSlideSegment {
    pub min: Vec2,
    pub max: Vec2,
    pub y: f32,
    pub direction: Vec3,
}

/// Horizontal speed (units/s) a slide segment forces the player along at.
pub const SLIDE_CARRY_SPEED: f32 = 6.0;

impl WaterSlideSegment {
    /// True when `pos` is inside the segment's XZ bounds and within pickup
    /// tolerance of its surface height — i.e. the segment carries the player.
    pub fn carries(&self, pos: Vec3) -> bool {
        pos.x >= self.min.x
            && pos.x <= self.max.x
            && pos.z >= self.min.y
            && pos.z <= self.max.y
            && (pos.y - self.y).abs() < 1.0
    }
}

/// Marker: the player is under scripted motion (zipline, suck-up animation…);
/// terrain collision must not fight the script.
#[derive(Component)]
pub struct TerrainPhysicsExempt;

/// Opt-in marker: this [`SolidBlock`] also blocks the follow camera's
/// sightline (see `follow_camera_system`). Tag walls, not walk-on platforms.
#[derive(Component)]
pub struct CameraOccluder;

// --- Config ---

/// Column-style pushout for heightfield levels: every [`TerrainSurface`] is
/// treated as a top-anchored slab `thickness` deep (clamped to `base_y`),
/// matching the rendered column, instead of using separate [`SolidBlock`]s.
/// This keeps collision derived from the same `surf.y` the mesh uses, which
/// matters when surfaces morph at runtime.
#[derive(Clone, Copy)]
pub struct ColumnPushout {
    pub thickness: f32,
    pub base_y: f32,
}

/// Per-level tuning for [`terrain_collision`]. Insert on level enter, remove
/// on exit.
#[derive(Resource, Clone)]
pub struct TerrainConfig {
    /// Void sentinel strictly below every real surface.
    pub floor_y: f32,
    /// Max step height a grounded player walks up (tolerance slack).
    pub step_up_limit: f32,
    /// Cap on the swept tolerance `|vy| * dt + step_up_limit`.
    pub tolerance_max: f32,
    /// Max ledge drop that step-down snapping bridges (CLAUDE.md: ~1.5).
    pub step_down_limit: f32,
    /// Vertical speed (units/s) for eased grounded step transitions.
    pub step_ease_rate: f32,
    /// Player radius for horizontal pushout.
    pub pushout_margin: f32,
    /// When set, pushout also treats every surface as a slab column.
    pub column_pushout: Option<ColumnPushout>,
}

impl TerrainConfig {
    /// Standard tuning shared by every level (CLAUDE.md: ~1.5 step-down,
    /// 15 u/s ease). Only `floor_y` — the void sentinel strictly below the
    /// level's lowest surface — is inherently per-level; override other
    /// fields with struct-update syntax where a level genuinely differs.
    pub fn standard(floor_y: f32) -> Self {
        Self {
            floor_y,
            step_up_limit: 0.5,
            tolerance_max: 2.0,
            step_down_limit: 1.5,
            step_ease_rate: 15.0,
            pushout_margin: 0.3,
            column_pushout: None,
        }
    }
}

// --- Pure core (unit-testable, no ECS) ---

/// Result of scanning surfaces for support under the player.
pub struct SupportScan {
    /// Highest surface the player should land on this frame (or `floor_y`).
    pub best_y: f32,
    /// Highest surface at the current XZ regardless of tolerance (fallback
    /// for a player that phased below everything).
    pub any_surface: f32,
}

/// Scan `(min, max, y)` surfaces for the best support, per the swept rules:
/// a surface counts if its XZ bounds contain the previous **or** current
/// position, and it either sits within `tolerance` of the player (at the
/// current XZ) or was vertically crossed during the frame while descending.
pub fn find_support(
    cur: Vec3,
    prev: Vec3,
    vy: f32,
    tolerance: f32,
    floor_y: f32,
    surfaces: impl IntoIterator<Item = (Vec2, Vec2, f32)>,
) -> SupportScan {
    let mut best_y = floor_y;
    let mut any_surface = floor_y;
    for (min, max, y) in surfaces {
        let in_now = cur.x >= min.x && cur.x <= max.x && cur.z >= min.y && cur.z <= max.y;
        let in_prev = prev.x >= min.x && prev.x <= max.x && prev.z >= min.y && prev.z <= max.y;
        if !(in_now || in_prev) {
            continue;
        }
        if in_now && y > any_surface {
            any_surface = y;
        }
        // Standing / vertical phase-through check (only at current XZ).
        if in_now && y <= cur.y + tolerance && y > best_y {
            best_y = y;
        }
        // Swept check: the player crossed this surface vertically while
        // moving downward — it should catch them even if their current XZ
        // is over a lower cell.
        if vy <= 0.0 && prev.y >= y - 0.05 && cur.y < y && y > best_y {
            best_y = y;
        }
    }
    SupportScan { best_y, any_surface }
}

/// What the vertical resolution decided to do with the player.
#[derive(Debug, PartialEq)]
pub enum VerticalAction {
    /// Ground instantly at this height (landing, tunnel catch, phase rescue).
    SnapTo(f32),
    /// Ground and move toward this height at `step_ease_rate` (grounded step).
    EaseTo(f32),
    /// The player is above any support and should be falling.
    Unground,
    /// No change (e.g. rising during a jump, close to the ground).
    Keep,
}

/// Decide the vertical resolution for this frame. Pure logic mirror of the
/// snap rules in CLAUDE.md plus the easing rule (ease only grounded steps).
pub fn resolve_vertical(
    y: f32,
    prev_y: f32,
    vy: f32,
    was_airborne: bool,
    scan: &SupportScan,
    floor_y: f32,
    step_down_limit: f32,
) -> VerticalAction {
    let mut best_y = scan.best_y;
    // Fallback: nothing within tolerance but there IS a surface here — the
    // player has phased below all surfaces. Rescue them upward.
    let mut phased = false;
    if best_y <= floor_y + 0.1 && scan.any_surface > floor_y + 0.1 && vy <= 0.0 {
        best_y = scan.any_surface;
        phased = true;
    }

    let crossed = vy <= 0.0 && prev_y >= best_y && y < best_y;
    let static_grounded_last = !was_airborne && vy.abs() < 0.01;
    let step_down = static_grounded_last
        && y > best_y
        && y - best_y <= step_down_limit
        && best_y > floor_y + 0.1;

    if (y <= best_y + 0.1 || crossed || step_down) && vy <= 0.0 {
        if was_airborne || crossed || phased {
            VerticalAction::SnapTo(best_y)
        } else {
            VerticalAction::EaseTo(best_y)
        }
    } else if y > best_y + 0.2 {
        VerticalAction::Unground
    } else {
        VerticalAction::Keep
    }
}

// --- Camera occlusion support ---

impl SolidBlock {
    /// Distance along a normalized ray to this AABB's entry face, with the
    /// box inflated by `pad_xz` horizontally and `pad_y` vertically. `None`
    /// if there is no hit within `max_dist` (or the ray starts on a face and
    /// immediately leaves).
    pub fn ray_entry(
        &self,
        origin: Vec3,
        dir: Vec3,
        max_dist: f32,
        pad_xz: f32,
        pad_y: f32,
    ) -> Option<f32> {
        let min = Vec3::new(self.min.x - pad_xz, self.y_min - pad_y, self.min.y - pad_xz);
        let max = Vec3::new(self.max.x + pad_xz, self.y_max + pad_y, self.max.y + pad_xz);
        let mut t_near = f32::NEG_INFINITY;
        let mut t_far = f32::INFINITY;
        for axis in 0..3 {
            let (o, d, mn, mx) = (origin[axis], dir[axis], min[axis], max[axis]);
            if d.abs() < 1e-6 {
                if o < mn || o > mx {
                    return None;
                }
            } else {
                let t1 = (mn - o) / d;
                let t2 = (mx - o) / d;
                let (lo, hi) = if t1 < t2 { (t1, t2) } else { (t2, t1) };
                t_near = t_near.max(lo);
                t_far = t_far.min(hi);
                if t_near > t_far {
                    return None;
                }
            }
        }
        // A graze where the ray starts on a face and leaves, or a box
        // entirely behind the origin.
        if t_far <= 1e-3 {
            return None;
        }
        if t_near > max_dist {
            return None;
        }
        Some(t_near.max(0.0))
    }
}

// --- The collision system ---

/// Push the player horizontally out of the XZ box `min..max` (inflated by
/// `margin`) along the shortest axis, killing inward velocity. Returns true
/// if a push happened.
fn push_out_of_box(
    transform: &mut Transform,
    physics: &mut PlayerPhysics,
    min: Vec2,
    max: Vec2,
    margin: f32,
) -> bool {
    let px = transform.translation.x;
    let pz = transform.translation.z;
    if !(px + margin > min.x && px - margin < max.x && pz + margin > min.y && pz - margin < max.y)
    {
        return false;
    }
    let push_left = (px + margin) - min.x;
    let push_right = max.x - (px - margin);
    let push_front = (pz + margin) - min.y;
    let push_back = max.y - (pz - margin);
    let min_push = push_left.min(push_right).min(push_front).min(push_back);

    if min_push == push_left {
        transform.translation.x = min.x - margin;
        physics.velocity.x = physics.velocity.x.min(0.0);
    } else if min_push == push_right {
        transform.translation.x = max.x + margin;
        physics.velocity.x = physics.velocity.x.max(0.0);
    } else if min_push == push_front {
        transform.translation.z = min.y - margin;
        physics.velocity.z = physics.velocity.z.min(0.0);
    } else {
        transform.translation.z = max.y + margin;
        physics.velocity.z = physics.velocity.z.max(0.0);
    }
    true
}

/// Shared player-vs-terrain collision. Runs after `player_movement` (which
/// has already integrated `velocity * dt` into the transform). No-op unless
/// the level inserted a [`TerrainConfig`].
pub fn terrain_collision(
    config: Option<Res<TerrainConfig>>,
    surfaces: Query<&TerrainSurface>,
    solids: Query<&SolidBlock>,
    mut player_q: Query<
        (&mut Transform, &mut PlayerPhysics, &mut SquashState),
        (With<Player>, Without<TerrainPhysicsExempt>),
    >,
    time: Res<Time>,
) {
    let Some(cfg) = config else { return };
    let Ok((mut transform, mut physics, mut squash)) = player_q.get_single_mut() else {
        return;
    };

    let was_airborne = !physics.grounded;
    let dt = time.delta_secs();

    // Horizontal pushout (multi-iteration to handle cascading corners).
    // Blocks whose top is within step_up_limit of the feet never push — the
    // vertical logic steps the player onto them instead.
    for _ in 0..3 {
        let mut pushed = false;
        for solid in &solids {
            let py = transform.translation.y;
            if py >= solid.y_max - cfg.step_up_limit {
                continue;
            }
            let body_top = py + PLAYER_BODY_H;
            let overlap = body_top.min(solid.y_max) - py.max(solid.y_min);
            if overlap < MIN_WALL_OVERLAP {
                continue;
            }
            pushed |= push_out_of_box(
                &mut transform,
                &mut physics,
                solid.min,
                solid.max,
                cfg.pushout_margin,
            );
        }
        if let Some(col) = cfg.column_pushout {
            for surf in &surfaces {
                // Degenerate / void cells anchored at or below the floor.
                if surf.y <= col.base_y + 0.1 {
                    continue;
                }
                let py = transform.translation.y;
                // On top of the slab, or low enough to step onto.
                if py >= surf.y - cfg.step_up_limit {
                    continue;
                }
                let col_bottom = (surf.y - col.thickness).max(col.base_y);
                let body_top = py + PLAYER_BODY_H;
                let v_overlap = body_top.min(surf.y) - py.max(col_bottom);
                if v_overlap < MIN_WALL_OVERLAP {
                    continue;
                }
                pushed |= push_out_of_box(
                    &mut transform,
                    &mut physics,
                    surf.min,
                    surf.max,
                    cfg.pushout_margin,
                );
            }
        }
        if !pushed {
            break;
        }
    }

    // Ceiling collision: don't jump up through the underside of a block.
    for solid in &solids {
        let px = transform.translation.x;
        let pz = transform.translation.z;
        let py = transform.translation.y;
        let m = cfg.pushout_margin;
        if px + m > solid.min.x
            && px - m < solid.max.x
            && pz + m > solid.min.y
            && pz - m < solid.max.y
            && physics.velocity.y > 0.0
            && py + PLAYER_BODY_H > solid.y_min
            && py < solid.y_min
        {
            transform.translation.y = solid.y_min - PLAYER_BODY_H;
            physics.velocity.y = 0.0;
        }
    }

    // Vertical resolution: swept scan, then snap / ease / unground.
    let cur = transform.translation;
    let prev = cur - physics.velocity * dt;
    let vy = physics.velocity.y;
    let tolerance = if vy <= 0.0 {
        (vy.abs() * dt + cfg.step_up_limit).min(cfg.tolerance_max)
    } else {
        0.0
    };

    let scan = find_support(
        cur,
        prev,
        vy,
        tolerance,
        cfg.floor_y,
        surfaces.iter().map(|s| (s.min, s.max, s.y)),
    );

    match resolve_vertical(
        cur.y,
        prev.y,
        vy,
        was_airborne,
        &scan,
        cfg.floor_y,
        cfg.step_down_limit,
    ) {
        VerticalAction::SnapTo(y) => {
            transform.translation.y = y;
            physics.velocity.y = 0.0;
            physics.grounded = true;
            if was_airborne {
                squash.timer = 0.3;
            }
        }
        VerticalAction::EaseTo(target) => {
            physics.velocity.y = 0.0;
            physics.grounded = true;
            let d = target - transform.translation.y;
            let max_step = cfg.step_ease_rate * dt;
            transform.translation.y += d.clamp(-max_step, max_step);
        }
        VerticalAction::Unground => {
            physics.grounded = false;
        }
        VerticalAction::Keep => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLOOR: f32 = -2.0;

    fn surf(min: (f32, f32), max: (f32, f32), y: f32) -> (Vec2, Vec2, f32) {
        (Vec2::new(min.0, min.1), Vec2::new(max.0, max.1), y)
    }

    // --- find_support ---

    #[test]
    fn swept_catch_across_narrow_surface_in_one_fast_frame() {
        // 2-unit-wide platform at y=1. In one fast frame the player moved
        // from over the platform (above its top) to past it in XZ (below its
        // top). A point-sample at the current position misses it entirely;
        // the swept scan must catch the crossing via the previous position.
        let s = [surf((0.0, -1.0), (2.0, 1.0), 1.0)];
        let cur = Vec3::new(3.0, 0.5, 0.0); // outside XZ, below the top
        let prev = Vec3::new(1.0, 1.5, 0.0); // inside XZ, above the top
        let scan = find_support(cur, prev, -5.0, 0.5, FLOOR, s);
        assert_eq!(scan.best_y, 1.0, "crossed surface must catch the player");
    }

    #[test]
    fn no_snap_up_when_rising() {
        // Surface 0.3 above the player; player is jumping (vy > 0).
        let s = [surf((-5.0, -5.0), (5.0, 5.0), 1.0)];
        let cur = Vec3::new(0.0, 0.7, 0.0);
        let prev = Vec3::new(0.0, 0.5, 0.0);
        // tolerance is 0 when rising (the system computes it that way).
        let scan = find_support(cur, prev, 3.0, 0.0, FLOOR, s);
        assert_eq!(scan.best_y, FLOOR, "rising player must not acquire support above");
    }

    #[test]
    fn tolerance_admits_step_up_within_slack() {
        let s = [surf((-5.0, -5.0), (5.0, 5.0), 1.0)];
        let cur = Vec3::new(0.0, 0.6, 0.0);
        let prev = cur;
        let scan = find_support(cur, prev, 0.0, 0.5, FLOOR, s);
        assert_eq!(scan.best_y, 1.0, "surface within tolerance is support");
        let scan = find_support(cur, prev, 0.0, 0.3, FLOOR, s);
        assert_eq!(scan.best_y, FLOOR, "surface beyond tolerance is not");
    }

    #[test]
    fn any_surface_found_even_outside_tolerance() {
        // Player phased 3 units below the only surface here.
        let s = [surf((-5.0, -5.0), (5.0, 5.0), 2.0)];
        let cur = Vec3::new(0.0, -1.0, 0.0);
        let scan = find_support(cur, cur, 0.0, 0.5, FLOOR, s);
        assert_eq!(scan.best_y, FLOOR);
        assert_eq!(scan.any_surface, 2.0);
    }

    // --- resolve_vertical ---

    fn scan(best_y: f32, any_surface: f32) -> SupportScan {
        SupportScan { best_y, any_surface }
    }

    #[test]
    fn airborne_landing_snaps() {
        // Falling player at/below the surface.
        let a = resolve_vertical(0.95, 1.4, -9.0, true, &scan(1.0, 1.0), FLOOR, 1.5);
        assert_eq!(a, VerticalAction::SnapTo(1.0));
    }

    #[test]
    fn grounded_step_up_eases() {
        // Walking into a 0.4-high step: support acquired via tolerance,
        // player below it, was grounded, vy == 0.
        let a = resolve_vertical(1.0, 1.0, 0.0, false, &scan(1.4, 1.4), FLOOR, 1.5);
        assert_eq!(a, VerticalAction::EaseTo(1.4));
    }

    #[test]
    fn grounded_step_down_within_limit_eases() {
        let a = resolve_vertical(1.0, 1.0, 0.0, false, &scan(0.2, 0.2), FLOOR, 1.5);
        assert_eq!(a, VerticalAction::EaseTo(0.2));
    }

    #[test]
    fn step_down_beyond_limit_ungrounds() {
        let a = resolve_vertical(2.0, 2.0, 0.0, false, &scan(0.2, 0.2), FLOOR, 1.5);
        assert_eq!(a, VerticalAction::Unground);
    }

    #[test]
    fn step_down_never_fires_into_void() {
        // No support at all (best_y == floor): walking off the last ledge
        // must unground, not snap to the void sentinel.
        let a = resolve_vertical(0.5, 0.5, 0.0, false, &scan(FLOOR, FLOOR), FLOOR, 1.5);
        assert_eq!(a, VerticalAction::Unground);
    }

    #[test]
    fn crossed_surface_snaps_not_eases() {
        // Fast grounded slide: prev_y above the support, cur below it.
        let a = resolve_vertical(0.6, 1.2, -0.5, false, &scan(1.0, 1.0), FLOOR, 1.5);
        assert_eq!(a, VerticalAction::SnapTo(1.0));
    }

    #[test]
    fn phased_below_rescue_snaps() {
        // Nothing in tolerance, but a surface exists overhead.
        let a = resolve_vertical(-1.0, -1.0, 0.0, false, &scan(FLOOR, 2.0), FLOOR, 1.5);
        assert_eq!(a, VerticalAction::SnapTo(2.0));
    }

    #[test]
    fn rising_player_keeps_flying() {
        let a = resolve_vertical(1.5, 1.2, 5.0, true, &scan(1.0, 1.0), FLOOR, 1.5);
        assert_eq!(a, VerticalAction::Unground);
    }

    #[test]
    fn ease_never_overshoots() {
        // The system clamps the ease step to the remaining gap; emulate one
        // frame of easing here.
        let target = 1.0_f32;
        let mut y = 0.9_f32;
        let max_step = 15.0 * (1.0 / 60.0);
        y += (target - y).clamp(-max_step, max_step);
        assert!((y - target).abs() < 1e-6);
    }

    // --- ray_entry ---

    fn block(min: (f32, f32), max: (f32, f32), y_min: f32, y_max: f32) -> SolidBlock {
        SolidBlock {
            min: Vec2::new(min.0, min.1),
            max: Vec2::new(max.0, max.1),
            y_min,
            y_max,
        }
    }

    #[test]
    fn ray_clean_hit() {
        let b = block((2.0, -1.0), (4.0, 1.0), 0.0, 3.0);
        let t = b.ray_entry(Vec3::new(0.0, 1.0, 0.0), Vec3::X, 10.0, 0.0, 0.0);
        assert_eq!(t, Some(2.0));
    }

    #[test]
    fn ray_miss() {
        let b = block((2.0, -1.0), (4.0, 1.0), 0.0, 3.0);
        // Ray passes above the block.
        let t = b.ray_entry(Vec3::new(0.0, 5.0, 0.0), Vec3::X, 10.0, 0.0, 0.0);
        assert_eq!(t, None);
    }

    #[test]
    fn ray_origin_inside_returns_zero() {
        let b = block((-1.0, -1.0), (1.0, 1.0), 0.0, 2.0);
        let t = b.ray_entry(Vec3::new(0.0, 1.0, 0.0), Vec3::X, 10.0, 0.0, 0.0);
        assert_eq!(t, Some(0.0));
    }

    #[test]
    fn ray_leaving_face_it_starts_on_is_no_hit() {
        // Origin exactly on the +X face, pointing away.
        let b = block((-1.0, -1.0), (1.0, 1.0), 0.0, 2.0);
        let t = b.ray_entry(Vec3::new(1.0, 1.0, 0.0), Vec3::X, 10.0, 0.0, 0.0);
        assert_eq!(t, None);
    }

    #[test]
    fn ray_hit_beyond_max_dist_is_no_hit() {
        let b = block((5.0, -1.0), (7.0, 1.0), 0.0, 3.0);
        let t = b.ray_entry(Vec3::new(0.0, 1.0, 0.0), Vec3::X, 3.0, 0.0, 0.0);
        assert_eq!(t, None);
    }

    #[test]
    fn ray_padding_inflates_the_box() {
        let b = block((2.0, -1.0), (4.0, 1.0), 0.0, 3.0);
        let t = b.ray_entry(Vec3::new(0.0, 1.0, 0.0), Vec3::X, 10.0, 0.5, 0.0);
        assert_eq!(t, Some(1.5));
    }
}
