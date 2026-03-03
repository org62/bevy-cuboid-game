# Debugger Challenges - Level Design & Progress Tracker

A Bevy 0.15 game that teaches debugging skills through 3D platformer levels. Players use a Rust debugger (breakpoints, variable inspection, value modification) to solve puzzles that are impossible through normal gameplay alone.

**Visual style**: Pastel colors, geometric 3D primitives, glowing emissive materials, floating/bobbing collectibles.
**Controls**: WASD/Arrows move, Space jump, Escape menu.
**Debugger targets**: Key functions marked `#[inline(never)]` for breakpoint-friendly debugging.

---

## Progress Tracker

| # | Level Name | File | Status |
|---|-----------|------|--------|
| 1 | The Password Gate | `level1.rs` + `password.rs` | DONE |
| 2 | The Cannon Gauntlet | `level2.rs` | DONE |
| 3 | The Countdown | `level3.rs` | TODO |
| 4 | The Invisible Maze | `level4.rs` | TODO |
| 5 | The Rigged Race | `level5.rs` | TODO |
| 6 | The Locked Chest | `level6.rs` | TODO |
| 7 | Gravity Flip | `level7.rs` | TODO |
| 8 | The Phantom Toll | `level8.rs` | TODO |
| 9 | Friendly Fire | `level9.rs` | TODO |
| 10 | The Loot Goblin | `level10.rs` | TODO |
| 11 | The Doppelganger | `level11.rs` | TODO |
| 12 | The Final Exam | `level12.rs` | TODO |
| - | Menu & infra updates | `main.rs`, `menu.rs` | TODO |

---

## Shared Infrastructure Changes

**`main.rs`**: Add 10 new `Screen` variants and their `SubStates` phase enums. Expand `Scoreboard` to track all 12 levels. Register all new level plugins.

**`menu.rs`**: Redesign to show 12 level buttons (scrollable or paginated grid). Add keyboard shortcuts for all levels.

**`player.rs`**: Add configurable gravity direction parameter (needed for Level 7). Current gravity constant `GRAVITY = -25.0` should become a resource or system parameter.

---

## Level 1: The Password Gate (DONE)

**Skill**: Setting breakpoints, inspecting string variables
**Gameplay**: Explore a pastel green area with collectible stars. Enter the restricted purple zone to trigger a password prompt. The password `"sesame"` is hardcoded in `check_password()` marked `#[inline(never)]` with byte-by-byte comparison.
**Win condition**: Enter the correct password.
**Hint**: "Use the debugger to find the password!"

---

## Level 2: The Cannon Gauntlet (DONE)

**Skill**: Inspecting function logic, understanding impossible win conditions
**Gameplay**: Dodge cannon fire in a beige arena, collect health cubes (heal 10 HP, max 100). Win requires 1000 HP -- impossible through normal play. Player must debug `check_health_victory()` and `collect_health_cube()` to find the `WIN_HP = 1000` constant and the `max 100` cap.
**Win condition**: `health.current >= 1000`
**Hint**: "The numbers don't add up... inspect check_health_victory() in the debugger."

---

## Level 3: The Countdown

**Skill**: Watching a variable change over time, modifying a continuously-updating value
**Theme**: Ancient stone temple with a ticking bomb in the center

**Gameplay**: A bomb counts down from 30 seconds. The arena has elevated platforms with 5 time-extension crystals (+3 sec each, total +15 sec -- not enough). A defuse panel sits across an impassable lava pit. The only way to win is to pause in the debugger, find the countdown value in `tick_bomb_timer()`, and either set `remaining` below `-10.0` (hidden defuse trigger) or set `defused = true` directly.

**Data structures**:
```rust
#[derive(Resource)]
struct BombTimer {
    remaining: f32,       // starts at 30.0, ticks down each frame
    defused: bool,        // win when true
}

#[inline(never)]
fn tick_bomb_timer(timer: &mut BombTimer, delta: f32) {
    if timer.defused { return; }
    timer.remaining -= delta;
    if timer.remaining <= -10.0 {
        // Hidden defuse: underflow triggers defusal
        timer.defused = true;
    }
}

#[inline(never)]
fn check_bomb_defused(timer: &BombTimer) -> bool {
    timer.defused
}
```

**Win condition**: `timer.defused == true`
**Failure**: Timer reaches 0 -- explosion, level restarts.
**Hint**: "The bomb is ticking... Can you stop time itself? Examine `tick_bomb_timer()` in the debugger."

**Visual elements**:
- Dark stone temple floor/walls with glowing orange cracks
- Central bomb: large black sphere with pulsing red emissive ring, red glow intensifies as timer drops
- Blue glowing crystals on elevated platforms (bob + rotate)
- Lava pit: orange emissive plane with particles
- HUD: large timer display (red when < 10s)

---

## Level 4: The Invisible Maze

**Skill**: Reading and writing entity position data (`Transform.translation`)
**Theme**: Foggy moonlit garden with invisible walls

**Gameplay**: A golden trophy glows on a pedestal at the far corner. Between the player and trophy is a complex maze made entirely of invisible wall colliders -- no visual geometry, just collision. Drifting semi-transparent fog cubes obscure the space. Small glowing breadcrumb orbs mark dead ends to frustrate brute-force navigation. The real solution: break into the debugger, find the player's `Transform` component, and write new `translation` values to teleport past the walls directly to the trophy position.

**Data structures**:
```rust
#[derive(Component)]
struct InvisibleWall; // marker for maze colliders

#[derive(Component)]
struct Trophy;

const TROPHY_POS: Vec3 = Vec3::new(12.0, 0.5, -10.0);

#[inline(never)]
fn check_trophy_collected(player_pos: Vec3, trophy_pos: Vec3) -> bool {
    let dx = player_pos.x - trophy_pos.x;
    let dz = player_pos.z - trophy_pos.z;
    (dx * dx + dz * dz) < 2.0
}
```

**Win condition**: Player position within distance 2.0 of trophy.
**Hint**: "The path is hidden, but your position is not. What if you could simply... be somewhere else? Look for `Transform.translation`."

**Visual elements**:
- Dark ambient lighting with blue-white moonlight
- Fog: many small semi-transparent white cubes drifting slowly
- Trophy: golden glowing sphere on stone pedestal with particle sparkles
- Breadcrumb orbs: small dim red spheres at dead ends
- Ground: dark green grass-colored plane

---

## Level 5: The Rigged Race

**Skill**: Finding and modifying floating-point speed values
**Theme**: Colorful oval racetrack with three AI opponents

**Gameplay**: Player races three AI runners around an oval track. The AI moves at speed 20.0, the player's speed is capped at 3.0 via `RacerStats.player_speed`. After 3 laps, first to cross the finish line wins. The track has ramps and lane markers. The player must find the `player_speed` float (or `ai_speed`) in the debugger and adjust it. Multiple valid solutions: boost `player_speed`, reduce `ai_speed`, or change `laps_to_win` to 1.

**Data structures**:
```rust
#[repr(C)]
#[derive(Resource)]
struct RacerStats {
    player_speed: f32,   // 3.0 -- painfully slow
    ai_speed: f32,       // 20.0 -- impossibly fast
    laps_to_win: i32,    // 3
}

#[derive(Component)]
struct AiRacer {
    lane: u8,
    progress: f32,       // 0.0..1.0 per lap
    lap: i32,
}

#[derive(Resource)]
struct PlayerRaceState {
    progress: f32,
    lap: i32,
}

#[inline(never)]
fn compute_player_race_speed(stats: &RacerStats) -> f32 {
    stats.player_speed
}

#[inline(never)]
fn compute_ai_race_speed(stats: &RacerStats) -> f32 {
    stats.ai_speed
}

#[inline(never)]
fn check_race_victory(player_lap: i32, laps_to_win: i32) -> bool {
    player_lap >= laps_to_win
}
```

**Win condition**: Player completes 3 laps before any AI racer.
**Failure**: Any AI racer completes 3 laps first.
**Hint**: "Your legs feel like lead! The race is rigged. Your speed is a float -- find where `compute_player_race_speed()` reads it."

**Visual elements**:
- Bright oval track with colored lane stripes (red, blue, yellow, green)
- AI racers: colored cube characters running around the track
- Finish line: checkered black/white arch
- Ramps/jumps on the track for fun
- HUD: lap counter, position indicator ("3rd / 4 racers")

---

## Level 6: The Locked Chest

**Skill**: Following pointers (one level of indirection via `Box`)
**Theme**: Torch-lit dungeon room with treasure chests

**Gameplay**: Five treasure chests on stone pedestals in a dungeon room. HUD shows "Keys: 0". Each chest costs 1 key to open. There are no keys anywhere in the level. The key count is stored behind a `Box<KeyRing>` -- the `Inventory` resource holds a pointer to a heap-allocated struct with padding fields around the actual `count`. The player must use the debugger to follow `inventory.keys` to the `KeyRing` allocation and set `count >= 5`. Opening all 5 chests wins.

**Data structures**:
```rust
#[repr(C)]
struct KeyRing {
    _padding_a: [u32; 4],  // decoy fields (0xDEADBEEF)
    count: i32,             // THE actual key count -- starts at 0
    _padding_b: [u32; 2],  // more decoys (0xCAFEBABE)
}

#[derive(Resource)]
struct Inventory {
    keys: Box<KeyRing>,     // pointer indirection!
}

#[derive(Resource)]
struct ChestsOpened(u32);

#[inline(never)]
fn try_open_chest(inventory: &mut Inventory) -> bool {
    let keys = &mut *inventory.keys;
    if keys.count > 0 {
        keys.count -= 1;
        true
    } else {
        false
    }
}

#[inline(never)]
fn check_all_chests_opened(opened: &ChestsOpened) -> bool {
    opened.0 >= 5
}
```

**Win condition**: All 5 chests opened (`ChestsOpened.0 >= 5`).
**Hint**: "No keys to be found... The inventory holds a pointer to your key ring. Set a breakpoint on `try_open_chest()` and inspect `inventory.keys`."

**Visual elements**:
- Dark stone walls with warm orange torch light (point lights)
- 5 wooden chests on stone pedestals (lid flip animation on open, golden particle burst)
- Key counter HUD badge (silver pill shape)
- Cobweb decorations, scattered coins on floor

---

## Level 7: Gravity Flip

**Skill**: Modifying function behavior / boolean control flow
**Theme**: Vertical tower with spiraling platforms

**Gameplay**: A tall tower with platforms spiraling upward. A golden star sits at the top (Y = 30). Every 4 seconds gravity reverses -- player and loose objects fall upward, then crash back down. The flip always resets progress. Gravity is determined by `compute_gravity_direction()` which reads `GravityState.flipped`. The player must either: (a) lock `flipped` to `false`, (b) set `flip_interval` to something huge like 9999.0, or (c) modify the resource so the function always returns normal gravity.

**Data structures**:
```rust
#[repr(C)]
#[derive(Resource)]
struct GravityState {
    flipped: bool,
    flip_timer: f32,       // counts down from flip_interval
    flip_interval: f32,    // 4.0 seconds
}

#[inline(never)]
fn compute_gravity_direction(state: &GravityState) -> f32 {
    if state.flipped { 25.0 } else { -25.0 }
}

#[inline(never)]
fn update_gravity_flip(state: &mut GravityState, dt: f32) {
    state.flip_timer -= dt;
    if state.flip_timer <= 0.0 {
        state.flipped = !state.flipped;
        state.flip_timer = state.flip_interval;
    }
}

#[inline(never)]
fn check_reached_top(player_y: f32) -> bool {
    player_y >= 30.0
}
```

**Win condition**: Player reaches Y >= 30.0.
**Hint**: "Gravity keeps betraying you! The flip is controlled by `compute_gravity_direction()`. What if gravity always went... your way?"

**Visual elements**:
- Tall cylindrical tower interior with stone walls
- Spiraling platforms in alternating colors (blue, purple, teal)
- Loose rocks and debris that also flip with gravity (visual feedback)
- Golden star at the peak with bright glow
- Warning flash (screen tint red) 1 second before each flip
- HUD: height indicator bar along left edge

---

## Level 8: The Phantom Toll

**Skill**: Stepping through an algorithm, understanding computed values
**Theme**: Arched bridge over a bottomless chasm with three ghostly toll booths

**Gameplay**: A beautiful arched bridge spans a deep chasm. Three toll booths block it at intervals. A ghostly toll keeper displays the price. Toll 1 costs 7 gold (affordable -- player starts with 10). Toll 2 costs 77. Toll 3 costs 777. The toll is computed by `compute_toll()` using obfuscated arithmetic (hex constants, bitwise AND, wrapping multiplies). The player must step through the function to understand the formula, then either: give themselves enough gold, or modify the computation's intermediate values/inputs.

**Data structures**:
```rust
#[repr(C)]
#[derive(Resource)]
struct PlayerWallet {
    gold: i32,           // starts at 10
}

#[derive(Resource)]
struct TollState {
    checkpoint: u32,     // which toll booth (0, 1, 2)
    paid: [bool; 3],
}

#[inline(never)]
fn compute_toll(checkpoint: u32) -> i32 {
    // Obfuscated: computes 7 * 10^checkpoint
    let mut base: i32 = 1;
    let mut i: u32 = 0;
    while i < checkpoint {
        base = base.wrapping_mul(10);
        i += 1;
    }
    let mask: i32 = 0x0F;
    let seed: i32 = 0x37; // 55 in decimal, 55 & 0x0F = 7
    let factor = seed & mask;
    factor.wrapping_mul(base)
}

#[inline(never)]
fn try_pay_toll(wallet: &mut PlayerWallet, cost: i32) -> bool {
    if wallet.gold >= cost {
        wallet.gold -= cost;
        true
    } else {
        false
    }
}
```

**Win condition**: All three tolls paid, player reaches far side.
**Hint**: "The toll keeper's price grows tenfold! Step through `compute_toll()` to understand the formula, then make yourself wealthy."

**Visual elements**:
- Grand stone bridge with arches, torchlit
- Bottomless dark chasm below (fog at bottom)
- Three ghostly toll keeper NPCs (semi-transparent white figures)
- Gold coin HUD counter
- Each paid toll booth turns from red barrier to green open gate

---

## Level 9: Friendly Fire

**Skill**: Shared code discrimination, conditional debugging
**Theme**: Colosseum battle arena with team combat

**Gameplay**: A Roman colosseum arena. Two blue-team allies ("Bolt" and "Spark", 100 HP each) fight two red-team enemies ("Fang" and "Claw", 500 HP each). All use the same `apply_arena_damage()` function. Allies take 5-10 damage/tick, enemies take 1-2 damage/tick. The fight is hopelessly rigged against the player's team. Simply NOPping the damage function causes a draw after 60 seconds (not accepted). The player must inspect the `Fighter.team` field when `apply_arena_damage()` is called and conditionally modify behavior -- e.g., set enemy health to 0 when team == 2, or set ally damage to 0.

**Data structures**:
```rust
#[repr(C)]
#[derive(Component)]
struct Fighter {
    health: f32,
    team: i32,             // 1 = ally (blue), 2 = enemy (red)
    name: [u8; 16],        // "Bolt\0", "Spark\0", "Fang\0", "Claw\0"
    attack_timer: f32,
    _decoy: i32,           // random noise value, changes every tick
}

#[derive(Resource)]
struct ArenaState {
    elapsed: f32,
    draw_timeout: f32,     // 60.0 -- draw declared if nobody dies
}

#[inline(never)]
fn apply_arena_damage(fighter: &mut Fighter, damage: f32) {
    // Called for ALL fighters -- allies AND enemies
    // NOPping causes a draw (nobody dies)
    fighter.health -= damage;
    if fighter.health < 0.0 { fighter.health = 0.0; }
}

#[inline(never)]
fn check_arena_victory(allies_alive: bool, enemies_alive: bool) -> i32 {
    // 0 = ongoing, 1 = player wins, -1 = player loses, 2 = draw
    match (allies_alive, enemies_alive) {
        (true, false) => 1,
        (false, _) => -1,
        _ => 0,
    }
}
```

**Win condition**: Both enemies dead, at least one ally alive (`check_arena_victory() == 1`).
**Failure**: Both allies dead, or draw after 60 seconds.
**Hint**: "The same function damages everyone! Stopping damage causes a draw. Look at the `team` field on each `Fighter` when `apply_arena_damage()` is called."

**Visual elements**:
- Sandy colosseum arena with stone walls and arched spectator seating
- Blue team NPCs (blue cubes with name labels overhead)
- Red team NPCs (red cubes with name labels overhead)
- Health bars floating above each fighter
- Combat particles (small flashes on each hit)
- HUD: team health summary, elapsed timer

---

## Level 10: The Loot Goblin

**Skill**: Inspecting array/Vec data structures, modifying weights in a probability table
**Theme**: Cozy treasure cave with a goblin and magical fountain

**Gameplay**: A goblin stands next to a glowing fountain in a cave. Walking near the goblin triggers a loot drop (3-second cooldown). Items drop as colored gems: gray pebble (weight 90.0), green gem (weight 8.0), blue gem (weight 1.9), golden key (weight 0.1). The exit door requires the golden key. Statistically ~1000 drops needed -- with 3-sec cooldown that's ~50 minutes of pure grinding. The player must find the `LootTable` resource, inspect the `entries` Vec, and either boost the golden key's weight or zero all other weights.

**Data structures**:
```rust
#[repr(C)]
#[derive(Clone)]
struct LootEntry {
    item_id: u32,          // 0=pebble, 1=green, 2=blue, 3=golden_key
    weight: f32,
    _name: [u8; 16],      // readable in debugger: "Pebble", "Green Gem", etc.
}

#[derive(Resource)]
struct LootTable {
    entries: Vec<LootEntry>,
    // Default weights: [90.0, 8.0, 1.9, 0.1]
}

#[derive(Resource)]
struct PlayerLoot {
    has_golden_key: bool,
}

#[inline(never)]
fn roll_loot(table: &LootTable, random_val: f32) -> u32 {
    let total: f32 = table.entries.iter().map(|e| e.weight).sum();
    let mut roll = random_val * total;
    for entry in &table.entries {
        roll -= entry.weight;
        if roll <= 0.0 {
            return entry.item_id;
        }
    }
    0
}

#[inline(never)]
fn check_has_key(loot: &PlayerLoot) -> bool {
    loot.has_golden_key
}
```

**Win condition**: Player has the golden key and walks to exit door.
**Hint**: "The goblin's loot is random... or is it? Inspect the `LootTable` entries to see the weights. The golden key's odds are 1 in 1000."

**Visual elements**:
- Warm cave interior with stalactites and crystal formations
- Goblin NPC: small green cube character with bouncing idle animation
- Glowing fountain: blue emissive water cylinder with sparkle particles
- Dropped gems scatter on ground with bounce physics
- Exit door: large wooden door with golden keyhole glow
- HUD: last drop result, total drops counter

---

## Level 11: The Doppelganger

**Skill**: Combining debugging with gameplay actions, identifying correct field among decoys
**Theme**: Symmetric mirror arena with a dark clone

**Gameplay**: A symmetric arena split by a translucent blue mirror wall. A dark-colored clone on the other side mirrors the player's X-movement in reverse. The goal: maneuver the clone into a glowing red trap zone on the clone's side. When the clone enters the trap, `check_clone_trapped()` fires -- but it also checks `clone.invincible`, which is `true`. The player must: (1) break on `check_clone_trapped()` to discover the `invincible` flag, (2) set it to `false` (among 7 decoy booleans that are all `false`), and (3) then physically maneuver so the clone walks into the trap (requires walking to the mirror-opposite position).

**Two-step solution**: Debug (find and flip the flag) THEN play (position yourself correctly).

**Data structures**:
```rust
#[repr(C)]
#[derive(Component)]
struct CloneData {
    mirror_axis: f32,          // X position of mirror wall
    invincible: bool,          // true -- THE field to flip
    _decoy_flags: [bool; 7],   // all false -- red herrings
    trapped: bool,
}

const TRAP_ZONE_MIN: Vec2 = Vec2::new(8.0, -3.0);
const TRAP_ZONE_MAX: Vec2 = Vec2::new(12.0, 3.0);

#[inline(never)]
fn check_clone_trapped(clone_pos: Vec3, clone: &mut CloneData) -> bool {
    let in_zone = clone_pos.x >= TRAP_ZONE_MIN.x
        && clone_pos.x <= TRAP_ZONE_MAX.x
        && clone_pos.z >= TRAP_ZONE_MIN.y
        && clone_pos.z <= TRAP_ZONE_MAX.y;
    if in_zone && !clone.invincible {
        clone.trapped = true;
        return true;
    }
    false
}

#[inline(never)]
fn mirror_player_position(player_pos: Vec3, mirror_x: f32) -> Vec3 {
    Vec3::new(2.0 * mirror_x - player_pos.x, player_pos.y, player_pos.z)
}
```

**Win condition**: `clone.trapped == true`
**Hint**: "Your shadow cannot be harmed... or can it? When the clone enters the red zone, something blocks the trap. Break on `check_clone_trapped()`."

**Visual elements**:
- Symmetric black/white tiled arena
- Translucent blue mirror wall down the center
- Clone: dark gray version of player model with glowing red eyes
- Red trap zone: glowing red floor patch on clone's side
- Power orbs on both sides (visual symmetry reinforcement)
- Clone mirrors player movement in real-time

---

## Level 12: The Final Exam

**Skill**: Synthesis of ALL techniques with NO hints
**Theme**: Grand four-room castle gauntlet

**Gameplay**: Four sequential rooms, each sealed by a door that opens on solving its puzzle. **No hint box is shown** -- the player must recognize which technique applies.

### Room 1: The Sealed Door
A door with a numeric keypad. A 6-digit code is computed at runtime (not a string literal) by `verify_access_code()`. The player must break on the function and inspect the computed `expected` local variable.
```rust
#[inline(never)]
fn verify_access_code(input: &str) -> bool {
    let mut code: u32 = 7;
    for i in 0u32..6 {
        code = code.wrapping_mul(13).wrapping_add(i * 3);
    }
    let expected = format!("{:06}", code % 1_000_000);
    input == expected
}
```

### Room 2: The Quicksand Floor
The floor pulls the player down. A `sink_rate` starts at 0.5 and doubles every 5 seconds. The exit platform is elevated. The player must find and zero the sink rate or teleport.
```rust
#[repr(C)]
#[derive(Resource)]
struct QuicksandState {
    sink_rate: f32,     // starts 0.5, doubles every 5s
    elapsed: f32,
    last_double: f32,
}

#[inline(never)]
fn apply_quicksand(sink_rate: f32, player_y: f32, dt: f32) -> f32 {
    player_y - sink_rate * dt
}
```

### Room 3: The Guarded Vault
Two guardian NPCs share a single `guardian_attack()` function. A vault door's `locked` flag is behind a `Box<VaultLock>` with padding. The player must neutralize the guardians (modify their health/damage) AND follow the pointer to unlock the vault.
```rust
#[repr(C)]
struct VaultLock {
    _pins: [u32; 3],    // decoy
    locked: bool,
}

#[derive(Resource)]
struct VaultDoor {
    lock: Box<VaultLock>,
}

#[derive(Component)]
struct Guardian {
    health: f32,
    damage_per_tick: f32,
}

#[inline(never)]
fn guardian_attack(damage: f32, target_health: &mut f32) {
    *target_health -= damage;
}
```

### Room 4: The Weighted Scale
A giant scale -- player's side must outweigh the other. Player weight is computed by `compute_player_weight()` which sums fields including a hidden `_penalty: -1000.0`. The player must step through the function, find the sabotaged field, and fix it.
```rust
#[repr(C)]
#[derive(Resource)]
struct PlayerWeight {
    base: f32,           // 10.0
    equipment: f32,      // 5.0
    _penalty: f32,       // -1000.0 (hidden sabotage!)
    bonus: f32,          // 0.0
}

#[inline(never)]
fn compute_player_weight(pw: &PlayerWeight) -> f32 {
    pw.base + pw.equipment + pw._penalty + pw.bonus
}
```

**Win condition**: All four rooms cleared.
**Hint**: NONE. Bottom-left only shows: "[Esc] Menu | WASD Move | Space Jump | You're on your own now."

**Visual elements**:
- Grand castle with progressively grander rooms
- Room 1: stone corridor with glowing keypad
- Room 2: sandy floor that visually sinks, elevated stone exit platform
- Room 3: vault room with patrolling guardian cubes, heavy metal vault door
- Room 4: grand hall with giant golden scale, stacks of weights, dramatic lighting

---

## Difficulty Progression

| Level | Difficulty | Primary Skill |
|-------|-----------|---------------|
| 1 | Beginner | Breakpoints, string inspection |
| 2 | Beginner | Integer inspection, win condition analysis |
| 3 | Beginner+ | Watching changing values, modifying floats |
| 4 | Easy | Reading/writing position data (Transform) |
| 5 | Easy | Float scanning and modification |
| 6 | Intermediate | Pointer following (Box indirection) |
| 7 | Intermediate | Modifying boolean control flow |
| 8 | Intermediate+ | Algorithm step-through, computed values |
| 9 | Hard | Shared code discrimination, team fields |
| 10 | Hard | Array/Vec data structure inspection |
| 11 | Hard+ | Multi-step: debug then play, decoy fields |
| 12 | Expert | All techniques combined, no hints |

---

## Implementation Order

Build levels in numerical order (3 through 12). Each level follows the established plugin pattern from `level2.rs`:
1. Define `LevelNPlugin` struct implementing `Plugin`
2. Add marker component for cleanup (e.g., `CountdownEntity`)
3. Define level-specific resources and components
4. Define `#[inline(never)]` challenge functions
5. Implement setup system (spawn world, player, HUD)
6. Implement gameplay systems (FixedUpdate for physics, Update for input/UI)
7. Implement victory/failure overlays
8. Implement OnExit cleanup (despawn all marked entities)
