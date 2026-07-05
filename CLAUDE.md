# Engineering notes for this codebase

## Terrain collision lives in `src/terrain.rs` — don't fork it

All player-vs-terrain collision (surface snapping, wall pushout, ceilings)
is the single shared system `terrain_collision` in `src/terrain.rs`,
parameterized per level by the `TerrainConfig` resource (inserted on level
enter, removed on exit; without it the system is a no-op). The shared
module also owns the `TerrainSurface`, `SolidBlock` and `WaterSlideSegment`
components. When a new level needs terrain collision, insert a
`TerrainConfig` and register the shared system after `player_movement` —
do not write a new per-level collision system. If the player is under
scripted motion (zipline, cutscene), insert `TerrainPhysicsExempt` on the
player for the duration instead of adding query filters to the shared
system.

## Player vs world collision: always sweep, never point-sample

Per-frame collision in this game runs after `player_movement` updates the
transform. The naive form — "find the highest surface at the player's
current XZ that is at/below `player.y + tolerance`" — silently fails in
two cases:

1. **Horizontal sweep skip.** When `velocity_xz * dt` exceeds the smallest
   collider footprint along the motion axis (Level 101 slide steps are
   2 units wide; Level 102 cells are `CELL` units), the player is over a
   *different* surface each frame and never satisfies the snap condition for
   the surfaces they pass over. The visible result is "player keeps falling
   past surfaces."
2. **Vertical sweep skip.** A surface that lay between `prev_y` and
   `current_y` during the frame is not at `current_y ± tolerance` and gets
   missed unless `tolerance` is grown to `|vy| * dt + slack`.

Rules when adding or modifying any collision system that compares the player
against surface entities:

- Compute the previous-frame position from the velocity that was just
  applied: `prev_pos = transform.translation - velocity * dt`. Consider any
  surface whose XZ overlaps **either** `prev_pos.xz` or `current_pos.xz`,
  not just current.
- A surface should catch the player if it sat anywhere in the range
  `[min(prev_y, current_y), max(prev_y, current_y)]` during the frame and
  the player is moving down (`vy <= 0`). Snap to the highest such surface.
- Keep the existing point-tolerance check — it's correct for the in-cell
  vertical-fall case and is cheaper than the sweep test.
- Never trigger snap-up when `velocity.y > 0` (the player is jumping). The
  swept check is gated on `vy <= 0` for the same reason.
- **Add step-down snapping** for grounded walk-offs. When the player was
  statically grounded last frame (`!was_airborne && vy.abs() < 0.01`) and
  the highest surface at the new XZ is within ~1.5 units below them, snap
  immediately instead of ungrounding. Without this, fast horizontal motion
  lets the player float off the edge of one cell and tunnel past several
  successive cells before gravity drops them far enough to be caught — the
  swept test alone misses this because the missed surfaces lie at
  intermediate XZs that are neither the previous nor the current cell.
  Keep step-down gated to surfaces above the void floor so the player
  doesn't snap through into nothing.
- **Ease only grounded steps; never ease a tunneling catch.** Grounded
  step-up/step-down transitions move toward the surface at
  `TerrainConfig::step_ease_rate` (15 u/s) so walking over uneven ground is
  smooth instead of a per-cell teleport. Airborne landings, swept crossings
  and phased-below rescues must still snap instantly — easing those lets
  the player visibly pass through geometry. Do not add a second, visual-only
  smoothing layer on top (the old `animate_player` mesh-offset lag caused
  the player to hover above steps whenever its rate diverged from the
  physics rate); the physics ease is the single source of smoothing.

Reference implementation: `terrain_collision` in `src/terrain.rs`
(`find_support` / `resolve_vertical` are the unit-tested pure core).

## Camera occlusion is opt-in via `CameraOccluder`

`follow_camera_system` pulls the camera in along the player→camera
sightline so it never clips inside geometry — but only against
`SolidBlock`s tagged with the `CameraOccluder` marker (`src/terrain.rs`).
Tag walls and tall opaque structures; do NOT tag walk-on/ride-on platforms
(stairs, slide segments, low tables) or invisible movement barriers, or the
camera will twitch every time the sightline grazes something the player is
standing on.

## Visible mesh and collision surface must agree on the top

If you ever clamp a visual dimension to keep geometry from collapsing
(e.g. `col_h.max(MIN)` for a column that would otherwise be 0), apply the
exact same clamp when populating the matching `TerrainSurface.y`. Mesh and
collision must derive from the same value:

```rust
let col_h = (height - Y_BASE).max(MIN_VISUAL);
let visual_top = Y_BASE + col_h;             // single source of truth
mesh.translation.y = visual_top - col_h / 2.0;
mesh.scale.y = col_h;
TerrainSurface { y: visual_top, .. };
```

If they diverge, the player will visually fall through into the gap below
the mesh top — and on levels that also override the absolute ground floor
(see next section) they'll come to rest on an invisible surface.

## Don't put `GroundYOverride` below the lowest visible surface

`GroundYOverride` is a hard floor inside `player_movement`. If it sits more
than ~0.2 units below the lowest spawned `TerrainSurface` / `SolidBlock` /
visible mesh, a player who fails terrain collision (for any reason) will
rest on the override at a y the player can never see geometry for, and
visually appears to be inside or under the world. Either keep the override
inside the visible terrain bounds, or guarantee that every `(x, z)` reachable
by `MovementBounds` is covered by a surface whose `y` is at or above the
override.
