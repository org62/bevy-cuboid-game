# Engineering notes for this codebase

## Adding a level: one module + one roster row

Levels are declared in exactly one place — the `LEVELS` table in
`src/levels.rs` (id, title, `register` fn, hidden?, starts_frozen?). The menu
grid, keyboard/gamepad navigation, the `Solved: n / m` line, the level-id →
screen mapping, the shared `LevelPhase` sub-state and plugin registration in
`main` all read that table. There is no per-level `Screen` variant (`Screen`
is `Menu | Level(u32)`), no per-level phase enum, and nothing to add in
`main.rs`. Adding a level means:

1. A row in `levels::LEVELS` pointing at the module's `ID` and `register`.
2. A level module shaped like:

```rust
pub const ID: u32 = 7;
const SCREEN: Screen = Screen::Level(ID);

pub fn register(app: &mut App) {
    app.add_systems(OnEnter(SCREEN), setup_foo)
        .add_systems(Update, foo_logic.in_set(GameplaySet::Logic)
            .run_if(level_kit::in_phase(SCREEN, LevelPhase::Playing)))
        .add_systems(OnExit(SCREEN), level_kit::despawn_level::<FooEntity>);
}
```

`level_kit::install` (called once in `main`) already registers orbit input,
`player_movement`, `terrain_collision`, `animate_player`,
`follow_camera_system`, `escape_to_menu`, `toggle_pause`, the hint/tutorial
hotkeys and the victory/defeat flow, each in its `GameplaySet` and gated on
the shared `LevelPhase`. Do not hand-wire any of those per level.

**`LevelPhase` is the shared per-level state machine** (a sub-state existing
on any `Screen::Level(_)`): `Frozen | Playing | Victory | Defeat`. The sim
runs only in `Playing`; camera and player animation run in every phase so the
scene stays alive behind overlays. `Frozen` is for level-driven freezes — the
race countdown, the password prompt; a level that opens frozen sets
`starts_frozen` in its roster row (and must opt `escape_to_menu`/`toggle_pause`
back in for `Frozen` if it wants them there, as level 5 does).

**Winning and losing are the shared flow, not per-level handlers.** Insert
`level_kit::VictoryText` / `DefeatText` in setup for the wording, then set
`NextState<LevelPhase>` to `Victory` or `Defeat`. The kit marks the scoreboard
from the current `Screen::Level(id)` (a level cannot mark the wrong id),
spawns the overlay, and on any key returns to the menu (victory) or re-enters
`DefeatText::retry_to` (defeat; `Playing` for most levels, `Frozen` for the
race's countdown). Level-specific reactions hook the shared state, always
gated on their own screen:

```rust
.add_systems(OnEnter(LevelPhase::Victory), reveal_walls.run_if(in_state(SCREEN)))
.add_systems(OnTransition { exited: LevelPhase::Defeat, entered: LevelPhase::Playing },
             reset_after_death.run_if(in_state(SCREEN)))
```

The `run_if(in_state(SCREEN))` on those hooks is not optional: `LevelPhase`
is shared, so an ungated hook fires on every level's victory/retry.

**The frame order is `GameplaySet`, not `.after(...)`.** `Input → Movement →
Collision → Scripted → Camera → Logic`, chained once by `level_kit::install`.
Put each new system in the set that matches what it *does*:

- moves the player under a script (slide, zipline, teleport) → `Scripted`;
- only reads the final player transform (a bespoke camera) → `Camera`;
- everything else (goal checks, HUD, pickups, timers) → `Logic`.

This exists because `.after(PlayerMovementSet)` leaves the follow camera
*unordered* against `terrain_collision`: Bevy may run the camera first, framing
the pre-collision player position — the raw ballistic one that dips into
geometry — which reads as camera judder on stairs and hills. Level 101 had
exactly that bug. A system placed in the right set cannot reintroduce it.

A level that drives its own camera (level 5's race chase cam) just spawns a
camera *without* a `FollowCamera` component — the shared follow system no-ops
— and registers its own camera system in `GameplaySet::Camera`. The kit also
disables orbit-look input when no `FollowCamera` exists: movement is
orbit-relative, so a live orbit under a fixed camera would invisibly rotate
the player's movement frame while the view stays put (mouse motion during the
race made players — and the test bot — veer and drive in circles). Don't
re-register `update_camera_orbit` on such a level.

**Global resources are swept centrally; you cannot leak them.** On every
return to the menu, `level_kit::reset_between_levels` removes
`TerrainConfig`, `GroundYOverride`, `GravityOverride`, `PowerUpState`,
`VictoryText`, `DefeatText` and `TextInputActive`, despawns stray overlays
and resets pause/orbit. (These are the resources read by *shared* systems —
leaking one used to change physics in every level entered afterwards; level 5
once leaked a 2x-speed `PowerUpState`.) If a new shared-system resource is
added, add it to that sweep. Level-private resources are re-inserted on entry
so leaking them is only untidy, but still remove them in the level's `OnExit`.

`level_kit::despawn_level::<Marker>` is the standard entity cleanup. It skips
entities whose parent carries the same marker (the parent's recursive despawn
already took them) — despawning them twice is what produced the `B0003 …
doesn't exist in this World` warning spam on level exit.

## Frame pacing: lock the sim to the refresh grid, and the camera is rigid

Under Fifo vsync the display scans out exactly one frame per refresh, but
the CPU-side frame loop returns at wildly uneven times — measured on real
hardware, a trivial scene's raw `Time::delta` bursts in a 3-frame cycle
(~1ms / 15ms / 33ms, swapchain queue jitter) while the screen still
updates every 16.7ms. Integrating motion with those raw deltas reads as
game-wide judder, *worst when the camera rotates* (the whole screen is in
parallax motion). `src/frame_pacing.rs` fixes this ONCE, centrally, by
rewriting the default `Time` each frame.

The correct model — and the one `pace_locked` implements — is to advance
the sim by exactly ONE refresh interval per presented frame (interval
anchored to the OS-reported monitor rate), carrying a bounded debt term so
genuine frame drops are still repaid and sim time tracks wall clock
long-term. Do NOT go back to rounding the raw delta to the nearest whole
refresh (the old `snap_to_grid`): the raw delta is a CPU-loop-timing
artifact that does not track scanout, so rounding it reproduces the same
0 / 1 / 2-interval cadence as the burst and bakes the judder right back in.
Empirically, locking dropped the per-frame sim-step deviation from ~13.8ms
(snapping) to ~2.6ms. Rules:

- Do not smooth, clamp, or average `time.delta_secs()` again inside any
  system — double smoothing recreates the judder.
- The follow camera is rigidly anchored to the player by design. Do not
  add positional smoothing/lerp layers to it (or to the player mesh):
  every such layer re-integrates frame-time jitter and shows it as
  judder. This was field-tested on real hardware; rigid won.
- `desired_maximum_frame_latency` stays at 1 and present mode stays Fifo;
  measured on real hardware this gives the tightest frame-loop pacing of
  the Fifo latency options (raising it widens the raw-delta swing).
- Rare zero- or double-length sim steps are legal (the debt term repaying
  a drop or a burst frame). Never divide by `dt`.
- F3 toggles the frame-pacing overlay (raw vs sim frame times). It is the
  first stop for any "choppy" report: with locking, `raw` stays spiky but
  `sim` should be nearly flat (spikes near 0) with `drift` bounded within
  about one refresh interval. `sim avg` far from `raw avg` means the game
  is running slow/fast — an estimator or pacing bug, not a feel issue.

## Raw mouse motion is normalized once, in `src/raw_mouse.rs`

`MouseMotion` is winit's raw device motion. On a local mouse it is a relative
delta in mouse counts; on an **absolute** pointer — RDP's virtual mouse, VM
guest pointers, some streaming clients — it is the pointer's *position*
normalized over `0..=65535`, and winit 0.30 forwards it unchanged because
`MOUSE_MOVE_RELATIVE` is `0`, making its `has_flag(usFlags, MOUSE_MOVE_RELATIVE)`
test `x & 0 == 0` — always true. Measured in a live RDP session (2560x1440,
`usFlags = 0x0003`), every event carried `lLastX ~32500, lLastY ~31900`: a
constant, same-signed, enormous "delta" that whips the camera around.

- **Read `RawMouse::delta`, never `MouseMotion`.** `accumulate_raw_mouse`
  (PreUpdate, after `InputSystem`) differentiates absolute streams back into
  motion and passes relative ones through untouched.
- Scaling cannot fix this — a constant offset stays constant. Only
  differencing works. Do not re-add a sensitivity/scale "fix" for it.
- Mode detection is streak-based (3 corroborating reports) and a suspected
  position is never forwarded as motion, latched or not: leaking one is a whip
  across the whole screen, dropping a few is invisible.
- Absolute positions are never negative, so a negative component is proof of a
  relative device and a large same-signed one proof of an absolute device.
  Small positive values (pointer near the desktop origin) prove nothing — leave
  the current mode alone.
- Steps larger than `MAX_ABS_STEP` are warps (reconnect, monitor hop, focus
  return), not hand motion; drop them but keep the new origin.
- Absolute reports carry `MOUSE_VIRTUAL_DESKTOP`, so they normalize over the
  monitor bounding box, not the primary screen — hence `virtual_desktop_size`.
- F3's overlay shows `mouse relative|absolute (rdp/vm)` plus the frame delta.
  First stop for any "mouse is too fast/slow/spinning" report.
- Inherent limitation, not a bug: over RDP the client sends absolute positions,
  so turning stops when the user's *local* pointer reaches their screen edge.
  Nothing server-side can warp a physical pointer back.

## Terrain collision lives in `src/terrain.rs` — don't fork it

All player-vs-terrain collision (surface snapping, wall pushout, ceilings)
is the single shared system `terrain_collision` in `src/terrain.rs`,
parameterized per level by the `TerrainConfig` resource (inserted on level
enter, swept centrally on menu return; without it the system is a no-op). The shared
module also owns the `TerrainSurface`, `SolidBlock` and `WaterSlideSegment`
components. A new level gets terrain collision by inserting a
`TerrainConfig` and nothing else — `level_kit::install` already
registers the shared system in `GameplaySet::Collision`. Do not write a
per-level collision system. `TerrainDiag` records what the last resolution
did (`SnapTo` / `EaseTo` / `Unground`) with the `dy` and `dt` it used;
assert against that record rather than differencing the transform across
frames, because a swept catch legitimately moves the player further in one
frame than `step_ease_rate` allows — a transform diff cannot tell the two
apart and produces intermittent false failures. If the player is under
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
   2 units wide), the player is over a *different* surface each frame and
   never satisfies the snap condition for the surfaces they pass over. The
   visible result is "player keeps falling past surfaces."
2. **Vertical sweep skip.** A surface that lay between `prev_y` and
   `current_y` during the frame is not at `current_y ± tolerance` and gets
   missed. Catch it with the *swept* test (`prev_y >= surface && current_y <
   surface`), **not** by growing the point tolerance — see the next section.

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
- **Never lift a player onto a surface their feet did not reach.** Support
  *above* the feet is granted only to a grounded walker stepping up, and only
  by `step_up_limit` (`terrain::step_up_tolerance`). Two ways this went wrong,
  both of which read to the player as "I jumped short and got dragged up onto
  the tier above", and both of which also re-grounded them in mid-air so they
  could jump again from nothing:
  - the tolerance used to be `|vy| * dt + step_up_limit` for *any* descending
    player, magneting a fast faller up to ~0.8 units and snapping a jump onto
    a platform at its apex (`vy ≈ 0` → the full slack);
  - the phase-through rescue used to fire on every level. It is gated on
    `ColumnPushout` now: in a heightfield, being under the surface at your XZ
    means you are inside a solid column and must be pushed out, but on a
    platform level that airspace is exactly where a short jump leaves you.
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
  and (heightfield-only) phase rescues must still snap instantly — easing those lets
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
