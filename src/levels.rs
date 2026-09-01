//! The level roster — the single place a level is declared.
//!
//! Adding a level means adding one [`LevelInfo`] row here pointing at the
//! level module's `ID` and `register` fn — and nothing else. The menu grid,
//! the keyboard/gamepad menu navigation, the scoreboard total, the level-id →
//! screen mapping, the shared [`crate::level_kit::LevelPhase`] sub-state and
//! plugin registration in `main` all read this table, so none of them can
//! drift out of sync or need a hardcoded count bumped.

use bevy::app::App;

use crate::Screen;

/// One playable level.
pub struct LevelInfo {
    /// Level id as shown in the menu and stored in the [`crate::Scoreboard`].
    /// Always the level module's own `ID` const, so the roster row and the
    /// module cannot disagree.
    pub id: u32,
    /// Title shown on the menu button.
    pub name: &'static str,
    /// Registers the level's systems on the app. Called once from `main`.
    pub register: fn(&mut App),
    /// Hidden easter-egg level: listed in the menu only after `?` is pressed.
    pub hidden: bool,
    /// The level starts in `LevelPhase::Frozen` — a scripted intro during
    /// which the sim (movement, collision, escape/pause) must not run yet,
    /// like the race's 3-2-1 countdown. Most levels start `Playing`.
    pub starts_frozen: bool,
}

impl LevelInfo {
    /// Screen state entered when the level is launched.
    pub fn screen(&self) -> Screen {
        Screen::Level(self.id)
    }
}

const fn level(
    id: u32,
    name: &'static str,
    register: fn(&mut App),
    hidden: bool,
    starts_frozen: bool,
) -> LevelInfo {
    LevelInfo { id, name, register, hidden, starts_frozen }
}

/// Every level, in menu order. Regular levels first, hidden ones after.
pub const LEVELS: &[LevelInfo] = &[
    level(crate::level1::ID, "The Password Gate", crate::level1::register, false, false),
    level(crate::level2::ID, "The Cannon Gauntlet", crate::level2::register, false, false),
    level(crate::level3::ID, "The Countdown", crate::level3::register, false, false),
    level(crate::level4::ID, "The Invisible Maze", crate::level4::register, false, false),
    level(crate::level5::ID, "The Rigged Race", crate::level5::register, false, true),
    level(crate::level101::ID, "The Hill Fortress", crate::level101::register, true, false),
    level(crate::level103::ID, "The Indoor Waterpark", crate::level103::register, true, false),
];

/// The level with this id, if any.
pub fn info(id: u32) -> Option<&'static LevelInfo> {
    LEVELS.iter().find(|l| l.id == id)
}

/// Screen the given level id launches into, if the id is on the roster.
pub fn screen_for_level(id: u32) -> Option<Screen> {
    info(id).map(|l| l.screen())
}

/// Levels the player can currently pick, in menu order. Hidden levels are
/// included only once they have been revealed.
pub fn visible(revealed: bool) -> impl Iterator<Item = &'static LevelInfo> {
    LEVELS.iter().filter(move |l| revealed || !l.hidden)
}

/// How many levels count toward the scoreboard right now — hidden levels only
/// join the denominator once revealed, so the menu never advertises them.
pub fn visible_count(revealed: bool) -> u32 {
    visible(revealed).count() as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_are_unique() {
        let ids: HashSet<u32> = LEVELS.iter().map(|l| l.id).collect();
        assert_eq!(ids.len(), LEVELS.len(), "duplicate level id in LEVELS");
    }

    #[test]
    fn screen_lookup_round_trips() {
        for l in LEVELS {
            assert_eq!(screen_for_level(l.id), Some(Screen::Level(l.id)));
        }
        assert_eq!(screen_for_level(0), None);
        assert_eq!(screen_for_level(9999), None);
    }

    #[test]
    fn hidden_levels_only_listed_once_revealed() {
        let hidden_count = LEVELS.iter().filter(|l| l.hidden).count();
        assert!(hidden_count > 0, "test is vacuous without hidden levels");
        assert_eq!(visible_count(false) as usize, LEVELS.len() - hidden_count);
        assert_eq!(visible_count(true) as usize, LEVELS.len());
        assert!(visible(false).all(|l| !l.hidden));
    }

    #[test]
    fn regular_levels_come_before_hidden_ones() {
        let first_hidden = LEVELS.iter().position(|l| l.hidden);
        if let Some(i) = first_hidden {
            assert!(LEVELS[i..].iter().all(|l| l.hidden), "menu order interleaves hidden levels");
        }
    }
}
