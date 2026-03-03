# Cheat Engine Tutorial: Reimplementation Specification

The Cheat Engine Tutorial is a built-in training application that teaches memory scanning, code injection, and pointer resolution through 9 progressive steps. This post documents the **exact behavior and data structures** of each step, extracted from the [source code](https://github.com/cheat-engine/cheat-engine/tree/master/Cheat%20Engine/Tutorial), so anyone can reimplement equivalent challenges in C, C++, Rust, or any other language.

---

## Step 1: Welcome & Password Gate

**Goal**: Attach Cheat Engine to the tutorial process.

**Implementation**: Just a welcome screen with a "Next" button and a password field. No memory scanning challenge.

**Password system** (for skipping steps):

```
Step 2: 090453
Step 3: 419482
Step 4: 890124
Step 5: 888899
Step 6: 098712
Step 7: 013370
Step 8: 525927
Step 9: 31337157  (8 digits, not 6)
```

**Spec**: Display tutorial text, "Next" button, password field. Character-by-character comparison (not string compare — each digit checked individually). Close confirmation dialog on exit.

---

## Step 2: Exact Value Scanning

**Goal**: Find a 4-byte integer health value in memory and change it to 1000.

**Data layout**:
```c
struct Step2 {
    // ... form fields ...
    int32_t health;  // private field, initialized to 100
};
```

**Behavior**:
- `health` starts at `100`
- "Hit me" button: `health -= (1 + random(5))` — decreases by 1–5
- If `health < 0`: show message "Aw, you're dead! Let me revive you", reset `health = 100`
- **Win condition**: Timer polls every ~100ms, if `health == 1000` → enable "Next" button
- Display: Label shows `health` as decimal string

**Reimplementation notes**:
- The health value is a **direct field** on the form/window object (not heap-allocated, not behind a pointer)
- Value type: 4-byte signed integer
- Scan type needed: Exact Value

---

## Step 3: Unknown Initial Value Scanning

**Goal**: Find a health value with unknown starting value, change it to 5000.

**Data layout**:
```c
struct Step3 {
    int32_t health;  // initialized to random(500) — unknown to user
    // displayed as a progress bar, not a number
};
```

**Behavior**:
- `health` starts at `random(500)` — value is NOT displayed as a number, only as a progress bar
- Progress bar: `min = -2000`, `max = -2000 + health`, `position = -2000 + health` (obfuscates the visual range)
- "Hit me" button: `loss = 1 + random(10)`, `health -= loss`. Loss is shown briefly (1 second, then hidden)
- If `health < 0`: show message, reset to `random(500)` with new progress bar range
- **Win condition**: `health == 5000` → enable "Next"

**Reimplementation notes**:
- The user cannot see the exact value — must use "Unknown Initial Value" scan, then "Decreased Value" scan after each hit
- The loss amount is briefly visible (1-second timer), but the user doesn't need it
- Value type: 4-byte integer
- Scan type needed: Unknown Initial → Decreased Value (repeated)

---

## Step 4: Floating Point Values

**Goal**: Find health (float) AND ammo (double), set both to >= 5000.

**Data layout**:
```c
struct Step4 {
    float   health;  // 32-bit float, initialized to 100.0
    double  ammo;    // 64-bit double, initialized to 100.0
};
```

**Behavior**:
- Health starts at `100.0` (float)
- Ammo starts at `100.0` (double)
- "Hit me" button: `health -= random(4) + (1 + random(10) / i)` where `i = 1 + random(10)`. Result is float arithmetic.
- "Shoot" button: `ammo -= 0.5` (constant decrement)
- If `health <= 0`: reset to `4000.0` (!). If `ammo <= 0`: reset to `100.0`
- **Win condition**: `health >= 5000.0 AND ammo >= 5000.0`
- Display: Both shown with 4 significant digits

**Reimplementation notes**:
- Health is `float` (4 bytes), ammo is `double` (8 bytes) — different scan types
- The hint says "disable Fast Scan for double" — because double alignment may not be 8-byte aligned
- Value types: Single (4-byte float) and Double (8-byte float)

---

## Step 5: Code Finder (Find What Writes)

**Goal**: Find the instruction that writes to a value, NOP it out, then verify the value no longer changes.

**Data layout**:
```c
// Heap-allocated pointer — address changes each run
int32_t* value;  // allocated via multiple getmem() calls, only last kept

// Initialization:
for (int k = 1 + random(10); k > 0; k--)
    value = malloc(4);  // intentional leaks to randomize address
*value = 100;
```

**Behavior**:
- `value` is heap-allocated with intentional dummy allocations before it (1+random(10) `malloc` calls, only last one kept)
- "Change value" button:
  ```
  old = *value
  new = random(1000)       // keep generating until new != old
  *value = new
  if (*value == old)       // check if the write was NOPped
      enable "Next" button
  ```
- **Win condition**: After clicking "Change value", if `*value` still equals `old` (meaning the write instruction was replaced with NOPs), the "Next" button enables

**Reimplementation notes**:
- The value is behind a **single pointer** (heap allocation) — address changes each run
- The write instruction in compiled code will be something like `mov [reg], eax`
- After the write, local variables are zeroed with optimizer barriers to prevent the compiler from removing the dead write
- The check `*value == old` happens AFTER the write attempt — if the instruction was NOPped, the value stays the same

---

## Step 6: Pointers

**Goal**: Find the base pointer to a heap-allocated value, create a pointer entry that survives reallocation, freeze it at 5000.

**Data layout**:
```c
// Global pointer variable (static address in .data/.bss)
int32_t* ptr;  // heap-allocated, changes on "Change pointer"

// Initialization:
for (int k = 1 + random(10); k > 0; k--)
    ptr = malloc(4);  // intentional leaks
*ptr = 100;
```

**Behavior**:
- "Change value": same as Step 5 — assigns random value, checks if write was NOPped
- "Change pointer" button (the hard part):
  ```
  old_ptr = ptr
  ptr = malloc(4)        // NEW allocation — address changes!
  *ptr = random(1000)
  countdown(3 seconds)
  if (*ptr == 5000 AND ptr != old_ptr)
      enable "Next"
  ```
- **Win condition**: After pointer reallocation + 3-second countdown, `*ptr == 5000` AND the pointer actually changed (not just freezing the old address)
- Anti-freeze check: `if (ptr == old_ptr)` → show error "freezing the pointer is not really a functional solution"

**Reimplementation notes**:
- `ptr` is a global variable at a **static address** (green in CE)
- The value it points to is heap-allocated and changes on "Change pointer"
- Pointer chain: `static_addr → heap_addr → value` (1-level pointer)

---

## Step 7: Code Injection

**Goal**: Inject code so that clicking "Hit me" INCREASES health by 2 instead of decreasing by 1.

**Data layout**:
```c
struct Step7 {
    int32_t health;  // direct field, initialized to 100
};
```

**Behavior**:
- "Hit me" button:
  ```c
  int old_health = health;
  health--;                        // the instruction to find & replace
  if (health == old_health + 2)    // check: did it increase by 2?
      enable "Next"
  ```
- If `health < 0`: reset to 100
- **Win condition**: `health == old_health + 2` after the button click

**Reimplementation notes**:
- The compiled code will have a `dec [address]` or `sub [address], 1` instruction
- The user must inject code that does `add [address], 2` instead of `dec`
- The check is `health == old_health + 2` — so the net result must be +2
- Solutions: replace `dec` with `add 2` (net: +2), or keep `dec` and add `add 3` before it, etc.

---

## Step 8: Multi-Level Pointers (4 levels deep)

**Goal**: Navigate a 4-level pointer chain, create a pointer entry that survives full reallocation, freeze at 5000 within 3 seconds.

**Data layout**:
```c
// 4-level pointer chain with decoy fields at each level
// Each level has different pointer offset!

struct Level4 {
    int32_t a, b, c, d, e, f;   // 6 decoy fields
    int32_t health;               // THE target value
};
// health offset from Level4 base: 6 * sizeof(int32_t) = 0x18

struct Level3 {
    Level4* p;                    // pointer at OFFSET 0x00
    int32_t a, b, c, d, e, f;   // 6 decoy fields after
};
// p offset from Level3 base: 0x00

struct Level2 {
    int32_t a, b, c, d, e;      // 5 decoy fields BEFORE pointer
    Level3* p;                    // pointer at OFFSET 0x14
    int32_t f;                   // 1 decoy field after
};
// p offset from Level2 base: 5 * sizeof(int32_t) = 0x14

struct Level1 {
    int32_t a, b, c;            // 3 decoy fields BEFORE pointer
    Level2* p;                   // pointer at OFFSET 0x0C
    int32_t d, e, f;            // 3 decoy fields after
};
// p offset from Level1 base: 3 * sizeof(int32_t) = 0x0C

// Global (static address):
Level1* base_pointer;
```

**Full pointer chain** (32-bit offsets):
```
base_pointer                    [static address, green in CE]
  → +0x0C → Level1.p           [→ Level2 address]
    → +0x14 → Level2.p         [→ Level3 address]  
      → +0x00 → Level3.p       [→ Level4 address]
        → +0x18 → Level4.health [the target value]
```

**Behavior**:
- On creation AND on "Change Register": ALL levels freed (zeromem'd + freed) and reallocated with `malloc(sizeof_level + random(128))` extra bytes
- All decoy fields (`a`–`f`) filled with `random(99999)` at each level
- `health = random(4000)` after each reallocation
- "Change value": only randomizes `health` to `random(4000)` (no reallocation)
- "Change Register": full reallocation + 3-second countdown
- **Win condition**: `health == 5000` after countdown

**Anti-unrandomizer**: At each pointer dereference, checks if `a==b==c==d==e==f`. If so, shows "Unrandomizer detected" and aborts. This prevents trivially zeroing the decoy fields.

**Reimplementation notes**:
- Pointer offsets are DIFFERENT at each level — this is intentional
- Each allocation has `+ random(128)` extra bytes to prevent size-based identification
- All decoy fields re-randomized on "Change Register"
- The base pointer is a global variable (static/green address)
- For 64-bit: pointer sizes become 8 bytes, adjust struct layouts and offsets accordingly

---

## Step 9: Shared Code (Distinguish Player vs Enemy)

**Goal**: Make your team win a simulated battle without freezing health. The same damage code is shared between all players.

**Data layout**:
```c
class TPlayer {
public:
    // vtable pointer at offset 0x00 (implicit in C++)
    float    health;                          // first field after vtable
    uint32_t boguscrap;                       // random decoy value
    int32_t  unrelatedrandomlychangingthing;  // changes every Hit()
    int32_t  team;                            // 1 = YOUR team, 2 = ENEMY
    char     name[64];                        // "Dave", "Eric", "HAL", "Skynet"
    TPlayer* teammate;                        // p1↔p2, p3↔p4
    void*    healthlabel;                     // UI reference (not useful for scanning)
    uint8_t  wasteofspace[99124];             // ~97KB padding per player!

    void Hit(int damage);  // SHARED function — same code for ALL players
};
```

**Player setup**:
```
Player 1: name="Dave",   team=1, health=100.0,  your team
Player 2: name="Eric",   team=1, health=100.0,  your team
Player 3: name="HAL",    team=2, health=500.0,  enemy (more health!)
Player 4: name="Skynet", team=2, health=500.0,  enemy (more health!)
```

Players are heap-allocated with `malloc(1 + random(90000))` spacers between them to randomize addresses.

**The shared Hit() function**:
```c
void TPlayer::Hit(int damage) {
    if (health == 0.0f) {
        show_message("This player is already dead");
        return;
    }
    float x = max(0.0f, health - (float)damage);
    health = x;                    // ← THIS is the shared write instruction
                                   //   Same code writes ALL players' health
    if (health == 0.0f)
        set_label("DEAD");
    else
        set_label("Health: " + to_string(health));

    unrelatedrandomlychangingthing = random(5000000);
}
```

**Autoplay damage rates** (timer tick):
```
if p1.health > 0: p1.Hit(2 + random(5))    // your team: 2-6 damage per tick
if p2.health > 0: p2.Hit(2 + random(5))
if p3.health > 0: p3.Hit(1 + random(1))    // enemies: only 1 damage per tick
if p4.health > 0: p4.Hit(1 + random(1))
```

**Win/loss conditions**:
```
if (p3.health <= 0 AND p4.health <= 0):     // both enemies dead
    if (p1.health > 0 OR p2.health > 0):    // at least one ally alive
        YOU WIN → enable "Next"
if (p1.health <= 0 AND p2.health <= 0):     // both allies dead
    FAILURE → show message
```

**Reimplementation notes**:
- `health` is a **float** (the hint says so)
- ALL 4 players use the SAME `Hit()` function — NOPping the write kills everyone
- The `team` field (int32, value 1 or 2) is the key discriminator
- The `unrelatedrandomlychangingthing` changes every hit — decoy for "find what writes" noise
- The ~97KB `wasteofspace` padding means each player object is ~97KB — affects relative address math between players
- **Solutions** (all require injected conditional code):
  1. Check `team` field relative to `health` pointer: if `[ecx+offset_of_team] == 1`, skip the damage write (protect allies)
  2. Check `team` field: if `[ecx+offset_of_team] == 2`, multiply damage (one-hit-kill enemies)
  3. Check the `this` pointer against known player addresses
  4. Check `name` field for specific strings
- Freezing health is NOT accepted as a solution — you must write conditional assembly

---

## General Reimplementation Notes

1. **Memory layout matters**: Struct field ordering and sizes determine the offsets that scanning exercises depend on. Use `#pragma pack(1)` or equivalent if needed to match expected layouts.

2. **Heap allocation randomization**: Steps 5, 6, 8, 9 all use multiple dummy `malloc` calls (loop count = `1 + random(10)`, only last result kept) to randomize heap addresses.

3. **Timer-based win checks**: Most steps poll every ~100ms to check the win condition. The user can write the value at any time between polls.

4. **The "Hit me" pattern**: Store `old_value` before modification, modify, then check `new_value` against expected. This detects both value changes (Steps 2-4) and code modifications (Steps 5, 7).

5. **32-bit vs 64-bit**: The original is 32-bit (pointer sizes = 4 bytes). If reimplementing as 64-bit, adjust all struct offsets for 8-byte pointers and document the changes.

6. **Anti-cheat checks**: Step 8's unrandomizer detection (all decoy fields equal). Step 6's anti-freeze check (pointer address comparison before/after reallocation).

7. **Optimizer barriers**: Steps 5 and 6 use `{$O+} j:=0; {$O-}` to prevent the compiler from optimizing away the write-then-check pattern. In C/C++, use `volatile` or compiler barriers.

8. **Display password in step header**: Each step shows its password (e.g., "Step 2: Exact Value scanning (PW=090453)").

9. **Close confirmation**: Every step has a step-specific snarky quit message. The Step 1 message says: "First step too hard? Better give up now."