https://www.youtube.com/watch?v=0O1JMrXuncw

# DEFCON Game Hacking Village CTF by Guided Hacking

This video walks through the challenges of the DEFCON Game Hacking Village CTF, featuring a Unity game designed for the event. It serves as a beginner-friendly tutorial showcasing various game hacking techniques using tools like Cheat Engine, DnspyEx, Melon Loader, and Unity Explorer, all demonstrated on challenges presented at DEFCON 32 in 2024 and previewing DEFCON 33 in 2025.

# Chapter 1. Turret Mono Patch

## Disabling the Turret's Laser Fire

*   **Challenge:** A turret's laser fire kills the player on contact with a red square, even when teleporting.
*   **Solution:** Use Cheat Engine's mono features to examine Unity game classes.
    *   Locate the `Turret` class and its `fireLaser` method.
    *   Patch the beginning of the `fireLaser` function with a `ret` instruction to prevent it from executing.
    *   This allows passage through the red square without taking damage and obtaining the flag.

# Chapter 2. Infinite Health

## Maintaining Health While Collecting Items

*   **Challenge:** Touching a shield while trying to obtain a gun within it damages both the player and the shield.
*   **Solution:** Find and manipulate the player's health value.
    *   Scan for the player's starting health (e.g., 100) in Cheat Engine, using "All" for value type initially.
    *   Run into the shield to decrease health (e.g., to 90) and filter the scan.
    *   "Hold" the health value using the active button and continue running into the shield.
    *   This prevents player health loss while allowing the shield's health to deplete, enabling the player to grab the gun and the flag.

# Chapter 3. Infinite Clip

## Overcoming Ammunition Limitations

*   **Challenge:** A limited ammunition count (e.g., 10 bullets) is insufficient to defeat a target (a unicorn).
*   **Solution:** Locate and modify the ammunition count.
    *   Scan for the starting bullet count (e.g., 10) in Cheat Engine.
    *   Shoot the weapon to decrease the count and filter the scan.
    *   Multiple addresses may appear; hold all suspect values and continue shooting.
    *   This allows for unlimited shots to defeat the unicorn and obtain the flag.

# Chapter 4. Combine Health & Clip

## Integrating Previous Hacking Techniques

*   **Challenge:** This level requires combining the techniques from the previous two levels.
*   **Solution:**
    *   Apply the infinite health technique to avoid damage from the shield.
    *   Apply the infinite clip technique to have unlimited ammunition.
    *   This allows the player to run under the shield and defeat the unicorn without repercussions, obtaining the flag.

# Chapter 5. Z-Axis Freeze

## Navigating Vertical Obstacles

*   **Challenge:** The player is dropped down when trying to grab a flag and needs to reach a platform.
*   **Solution:** Find and manipulate the player's Z-axis (vertical position) value.
    *   Use a staircase to move up and down.
    *   In Cheat Engine, scan for a floating-point value of unknown initial value.
    *   Use "Increased" or "Decreased" value scans as the player moves vertically, interspersed with "Unchanged" scans.
    *   Continue filtering until a small list of values remains.
    *   Add these values to the address list and test each one by manipulating it to see if it affects vertical position.
    *   Once identified, adjust the value to move the player to the platform and obtain the flag.

# Chapter 6. Fish Bypass

## Circumventing Item Checks

*   **Challenge:** A cat statue requires a fish, but the player only has an apple, and it rejects the apple.
*   **Solution:** Use Cheat Engine's mono features to bypass the item check.
    *   Examine .NET classes for anything related to "fish."
    *   Locate the `OnTriggerEnter` function within a class likely responsible for checking if the player has the fish.
    *   Analyze the assembly code for the function, specifically looking for conditional jumps.
    *   Identify a jump that leads to an early exit if the condition is not met (e.g., the player doesn't have the fish).
    *   Patch this jump instruction (e.g., replace it with `nop` or a jump to the success branch) to force the function to proceed as if the player has the fish.
    *   This allows the player to interact with the statue and receive the flag.

# Chapter 7. Win Condition Patch

## Forcing a Win State

*   **Challenge:** Hitting pumpkin targets within a time limit is required to avoid being killed by a blow dart. The shooting speed is insufficient to win.
*   **Solution:** Modify the win condition logic.
    *   Use Mono features in Cheat Engine to find the `CheckWin` function within the `WhackAMole` class.
    *   Examine the control flow graph of the `CheckWin` function.
    *   Identify the branches responsible for determining win or lose states (e.g., `eax` set to 0 for lose, 1 for win).
    *   Patch out the conditional checks that lead to the lose state, ensuring the function always proceeds to the win state.
    *   This bypasses the need to score points and allows the player to obtain the flag after the timer expires.

# Chapter 8. Loot Table Mod

## Increasing Item Drop Rates

*   **Challenge:** A chest has a very low drop rate for a specific item (the flag).
*   **Solution:** Use Melon Loader and Unity Explorer to modify the loot table.
    *   Open Unity Explorer through Melon Loader.
    *   Locate the chest object and its loot table reference.
    *   Inspect the loot table to see the available items and their properties (chance percentage, weight).
    *   Identify the game object representing the flag.
    *   Either modify the chance percentage/weight of the flag item or, more directly, replace the game object reference of another item (e.g., a fish) with the flag's game object.
    *   When interacting with the chest, it will now drop the flag.

# Chapter 9. Speed Hack

## Accelerating Player Movement

*   **Challenge:** Reach the end of a level quickly to collect coins and avoid being killed by a turret.
*   **Solution:** Increase player movement speed and disable threats.
    *   Use Unity Explorer to access the player character's movement component.
    *   Modify the "multiplayer" (likely movement multiplier) value to a higher number (e.g., 4).
    *   Additionally, locate the turret object and despawn it to remove the threat.
    *   With increased speed and no threats, the player can collect coins and reach the flag quickly.

# Chapter 10. Hidden Level Unlock

## Accessing a Secret Level

*   **Challenge:** There are 10 levels indicated in the game files, but only 9 are selectable. A hidden level needs to be accessed.
*   **Solution:** Use Melon Loader and Unity Explorer to find and load the hidden level.
    *   Inspect the game's folder structure to confirm the existence of a Level 10.
    *   Use Unity Explorer to search for the missing scene.
    *   Load the Level 10 scene.
    *   This level's logic involves DN Spy and a required key. Instead of obtaining the key, use Unity Explorer to spawn the hidden flag.
    *   Locate the "hidden flag" object in Unity Explorer and instantiate it.
    *   Find the spawned flag in the game world (behind a unicorn in this case) and collect it.

# Critique

*   **Most Liked:** The most positively received feedback centers on the clarity and effectiveness of the tutorial in demonstrating practical game hacking techniques. The use of the DEFCON CTF challenges as a framework is highly appreciated, making the content engaging and relevant. Specific mentions of the step-by-step walkthroughs for each level, especially the more complex ones like the Z-Axis freeze and loot table modification, highlight their value to viewers.
*   **General Appreciation:** Many comments express gratitude for the detailed explanations and the accessibility of the content for beginners. The choice of a Unity game for the CTF is seen as a good learning platform. The sponsorship and continued involvement of Guided Hacking with the DEFCON Game Hacking Village are also frequently praised, with viewers looking forward to future events.
*   **Tool Usage:** The breakdown of how to use specific tools like Cheat Engine (including its mono features), DnspyEx, Melon Loader, and Unity Explorer is a significant positive. Viewers find the practical application of these tools within the CTF challenges highly informative.
*   **Pacing and Structure:** The video's chapter-like structure, indicated by timestamps and clear problem statements for each level, is well-received. This organization allows viewers to follow along easily and jump to specific challenges.
*   **Minor Suggestions:** A few viewers have suggested slightly more in-depth explanations for certain assembly code snippets or more explicit detail on patching specific bytes when `nop` padding is required. However, these are generally minor points within an otherwise very positive reception.