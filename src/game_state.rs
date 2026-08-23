//! Game state module.
//!
//! Defines the central game state structure and game status enum,
//! holding all data needed to represent the current state of a game session.

use std::time::Instant;

use crate::board::Board;
use crate::logic::UndoEntry;
use crate::timer::GameTimer;

/// Difficulty level that affects shuffle behavior.
///
/// - **Easy**: Shuffles are guaranteed to produce a solvable board state.
///   Uses smart placement to ensure free tiles always have valid pairs.
/// - **Normal**: Shuffles randomly redistribute face IDs without guaranteeing
///   solvability. The player may need to use additional shuffles if they get stuck.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Difficulty {
    /// Guaranteed solvable shuffles (smart placement fallback).
    Easy,
    /// Pure random shuffles — may require reshuffling due to dead ends.
    Normal,
}

/// Central game state holding all data for the current session.
pub struct GameState {
    /// The board with tile positions and occupancy.
    pub board: Board,
    /// Elapsed time tracker with pause support.
    pub timer: GameTimer,
    /// Score tracking (hints, shuffles, time).
    pub score: ScoreTracker,
    /// Current game phase.
    pub status: GameStatus,
    /// Currently selected tile position index, if any.
    pub selection: Option<usize>,
    /// Active hint highlight state, if any.
    pub hint: Option<HintState>,
    /// Stack of moves available for undo (max 10).
    pub undo_stack: Vec<UndoEntry>,
    /// Number of shuffles remaining (starts at 1, +1 per completed level, +1 daily bonus).
    pub shuffles_remaining: u32,
    /// Current level (starts at 1).
    pub level: u32,
    /// Accumulated score from previous levels (carried forward on level advance).
    pub base_score: u32,
    /// Accumulated time in milliseconds from previous levels.
    pub base_time_ms: u64,
    /// Accumulated hints used from previous levels.
    pub base_hints: u32,
    /// Accumulated shuffles used from previous levels.
    pub base_shuffles: u32,
    /// Accumulated undos used from previous levels.
    pub base_undos: u32,
    /// Active animations being played.
    pub animations: Vec<Animation>,
    /// Difficulty level for this session (affects shuffle behavior).
    pub difficulty: Difficulty,
}

/// The current phase/status of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameStatus {
    /// Game is actively being played.
    Playing,
    /// Game is paused (timer stopped, input disabled).
    Paused,
    /// Player has cleared all tiles.
    Won,
    /// No valid moves remain (but shuffles may still be available).
    Lost,
    /// No valid moves AND no shuffles remaining — true game over.
    GameOver,
    /// Main menu is displayed.
    Menu,
    /// Player is entering their name for the leaderboard.
    NameEntry,
    /// Leaderboard view is displayed.
    Leaderboard,
    /// Shortcuts popup is displayed.
    Shortcuts,
    /// Level selection screen is displayed.
    LevelSelect,
}

/// Tracks score-relevant statistics for the current game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScoreTracker {
    /// Number of hints used this game.
    pub hints_used: u32,
    /// Number of shuffles used this game.
    pub shuffles_used: u32,
    /// Number of undos used this game.
    pub undos_used: u32,
    /// Elapsed seconds at game completion (snapshot for scoring).
    pub elapsed_seconds: u32,
    /// Number of pairs matched so far.
    pub pairs_matched: u32,
    /// Number of mismatched (wrong) tile pair attempts this level.
    pub mismatches: u32,
}

impl ScoreTracker {
    /// Creates a new score tracker with zero values.
    pub fn new() -> Self {
        Self {
            hints_used: 0,
            shuffles_used: 0,
            undos_used: 0,
            elapsed_seconds: 0,
            pairs_matched: 0,
            mismatches: 0,
        }
    }

    /// Calculates the final score.
    ///
    /// Score increases with each pair matched:
    /// - Base: 10 points per pair
    /// - Streak bonus: pairs_matched * 2 (rewards continuous play)
    /// - Penalties: -5 per hint used, -10 per shuffle used
    /// - Time bonus at game completion: max(0, 500 - elapsed_seconds)
    ///
    /// During gameplay (elapsed_seconds == 0), only pair/penalty scores are shown.
    pub fn calculate_score(&self) -> u32 {
        let base = self.pairs_matched * 10;
        let streak = self.pairs_matched * 2;
        let raw = base + streak;
        let penalty = self.hints_used * 5 + self.shuffles_used * 10;
        let subtotal = raw.saturating_sub(penalty);

        // Time bonus only applies at game end (when elapsed_seconds is snapshotted)
        if self.elapsed_seconds > 0 {
            let time_bonus = 500u32.saturating_sub(self.elapsed_seconds);
            subtotal + time_bonus
        } else {
            subtotal
        }
    }

    /// Calculates the live score during gameplay (no time bonus yet).
    pub fn live_score(&self) -> u32 {
        let base = self.pairs_matched * 10;
        let streak = self.pairs_matched * 2;
        let raw = base + streak;
        let penalty = self.hints_used * 5 + self.shuffles_used * 10;
        raw.saturating_sub(penalty)
    }
}

impl Default for ScoreTracker {
    fn default() -> Self {
        Self::new()
    }
}

use crate::storage::{self, UserProfile};

/// State for the leaderboard name entry / player selection flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameEntryState {
    /// Characters typed so far for a new player.
    pub text: String,
    /// Discovered existing player profiles.
    pub profiles: Vec<UserProfile>,
    /// Selected index: 0..profiles.len() for existing profiles, profiles.len() for new player input.
    pub selected_index: usize,
    /// Current page for existing profiles pagination.
    pub page: usize,
    /// The score that qualified for the leaderboard.
    pub score: u32,
    /// The total elapsed time in seconds at game completion (across all levels).
    pub time_seconds: u32,
    /// Total hints used across all levels.
    pub hints_used: u32,
    /// Total shuffles used across all levels.
    pub shuffles_used: u32,
    /// Total undos used across all levels.
    pub undos_used: u32,
}

impl NameEntryState {
    /// Number of profiles displayed per page.
    pub const PROFILES_PER_PAGE: usize = 3;

    /// Creates a new name entry state with the given score, time, and cumulative stats.
    pub fn new(score: u32, time_seconds: u32, hints_used: u32, shuffles_used: u32, undos_used: u32) -> Self {
        let profiles = storage::list_user_profiles();
        let selected_index = if profiles.is_empty() { 0 } else { 0 };
        Self {
            text: String::new(),
            profiles,
            selected_index,
            page: 0,
            score,
            time_seconds,
            hints_used,
            shuffles_used,
            undos_used,
        }
    }

    /// Creates a new name entry state with an explicit list of profiles (useful for testing).
    pub fn with_profiles(
        profiles: Vec<UserProfile>,
        score: u32,
        time_seconds: u32,
        hints_used: u32,
        shuffles_used: u32,
        undos_used: u32,
    ) -> Self {
        Self {
            text: String::new(),
            profiles,
            selected_index: 0,
            page: 0,
            score,
            time_seconds,
            hints_used,
            shuffles_used,
            undos_used,
        }
    }

    /// Total pages for profile list.
    pub fn total_pages(&self) -> usize {
        if self.profiles.is_empty() {
            1
        } else {
            (self.profiles.len() + Self::PROFILES_PER_PAGE - 1) / Self::PROFILES_PER_PAGE
        }
    }

    /// Returns start index of profiles for the current page.
    pub fn page_start_index(&self) -> usize {
        self.page * Self::PROFILES_PER_PAGE
    }

    /// Returns end index (exclusive) of profiles for the current page.
    pub fn page_end_index(&self) -> usize {
        (self.page_start_index() + Self::PROFILES_PER_PAGE).min(self.profiles.len())
    }

    /// Returns true if the "New Player" text input field is currently selected.
    pub fn is_new_player_selected(&self) -> bool {
        self.selected_index >= self.profiles.len()
    }

    /// Returns the currently selected profile, if an existing profile is selected.
    pub fn selected_profile(&self) -> Option<&UserProfile> {
        self.profiles.get(self.selected_index)
    }

    /// Moves selection to the previous item (or previous page).
    pub fn select_prev(&mut self) {
        if self.profiles.is_empty() {
            self.selected_index = 0;
            return;
        }

        if self.is_new_player_selected() {
            // Move from New Player field to the last visible item on current page
            let end = self.page_end_index();
            if end > 0 {
                self.selected_index = end - 1;
            }
        } else if self.selected_index == self.page_start_index() {
            if self.page > 0 {
                self.page -= 1;
                self.selected_index = self.page_end_index().saturating_sub(1);
            } else {
                // Wrap to New Player field
                self.selected_index = self.profiles.len();
            }
        } else {
            self.selected_index = self.selected_index.saturating_sub(1);
        }
    }

    /// Moves selection to the next item (or next page / New Player field).
    pub fn select_next(&mut self) {
        if self.profiles.is_empty() {
            self.selected_index = 0;
            return;
        }

        if self.is_new_player_selected() {
            // Move from New Player field to first item on page 0
            self.page = 0;
            self.selected_index = 0;
        } else if self.selected_index + 1 >= self.page_end_index() {
            if self.page + 1 < self.total_pages() {
                self.page += 1;
                self.selected_index = self.page_start_index();
            } else {
                // Move to New Player field
                self.selected_index = self.profiles.len();
            }
        } else {
            self.selected_index += 1;
        }
    }

    /// Toggles focus between the existing player list and the new player field.
    pub fn toggle_field(&mut self) {
        if self.is_new_player_selected() {
            if !self.profiles.is_empty() {
                self.selected_index = self.page_start_index();
            }
        } else {
            self.selected_index = self.profiles.len();
        }
    }

    /// Switches to the next page of profiles.
    pub fn next_page(&mut self) {
        if self.page + 1 < self.total_pages() {
            self.page += 1;
            self.selected_index = self.page_start_index();
        }
    }

    /// Switches to the previous page of profiles.
    pub fn prev_page(&mut self) {
        if self.page > 0 {
            self.page -= 1;
            self.selected_index = self.page_start_index();
        }
    }

    /// Selects a specific profile index.
    pub fn select_profile_at_index(&mut self, idx: usize) {
        if idx < self.profiles.len() {
            self.selected_index = idx;
            self.page = idx / Self::PROFILES_PER_PAGE;
        }
    }

    /// Selects the new player input field.
    pub fn select_new_player(&mut self) {
        self.selected_index = self.profiles.len();
    }

    /// Appends a character to the name buffer if it won't exceed 20 characters.
    /// Auto-focuses the New Player input field.
    /// Returns true if the character was added.
    pub fn push_char(&mut self, c: char) -> bool {
        self.selected_index = self.profiles.len();
        if self.text.chars().count() < 20 {
            self.text.push(c);
            true
        } else {
            false
        }
    }

    /// Removes the last character from the name buffer.
    /// Auto-focuses the New Player input field.
    /// Returns true if a character was removed.
    pub fn pop_char(&mut self) -> bool {
        self.selected_index = self.profiles.len();
        self.text.pop().is_some()
    }

    /// Returns true if the current text is a valid name (1-20 characters).
    pub fn is_valid(&self) -> bool {
        let len = self.text.trim().chars().count();
        (1..=20).contains(&len)
    }
}

/// State of an active hint highlight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HintState {
    /// First tile position of the hinted pair.
    pub position_a: usize,
    /// Second tile position of the hinted pair.
    pub position_b: usize,
    /// When the hint was activated (for auto-dismiss after 3 seconds).
    pub activated_at: Instant,
}

/// Animations that can be playing on the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Animation {
    /// Fade-out animation when a matched pair is removed.
    TileRemoval {
        positions: (usize, usize),
        face_id: u8,
        start_time: Instant,
        duration_ms: u32,
    },
    /// Colorful lightning animation converging in the middle.
    Lightning {
        positions: (usize, usize),
        start_time: Instant,
        duration_ms: u32,
    },
    /// Red flash animation for mismatched pair.
    TileMismatch {
        positions: (usize, usize),
        start_time: Instant,
        duration_ms: u32,
    },
    /// Pulsing glow on hinted tiles.
    HintPulse {
        positions: (usize, usize),
        start_time: Instant,
    },
    /// Shuffle animation when tiles are rearranged.
    Shuffle {
        start_time: Instant,
        duration_ms: u32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_perfect_game() {
        // 72 pairs matched, no penalties, completed in 100 seconds
        let tracker = ScoreTracker {
            hints_used: 0,
            shuffles_used: 0,
            undos_used: 0,
            elapsed_seconds: 100,
            pairs_matched: 72,
            mismatches: 0,
        };
        // base: 72*10=720, streak: 72*2=144, penalty: 0, time_bonus: 500-100=400
        // total: 720+144+400 = 1264
        assert_eq!(tracker.calculate_score(), 1264);
    }

    #[test]
    fn score_with_penalties() {
        let tracker = ScoreTracker {
            hints_used: 2,
            shuffles_used: 1,
            undos_used: 0,
            elapsed_seconds: 120,
            pairs_matched: 72,
            mismatches: 0,
        };
        // base: 720, streak: 144, penalty: 2*5+1*10=20, time_bonus: 500-120=380
        // total: 720+144-20+380 = 1224
        assert_eq!(tracker.calculate_score(), 1224);
    }

    #[test]
    fn score_never_negative() {
        let tracker = ScoreTracker {
            hints_used: 100,
            shuffles_used: 100,
            undos_used: 0,
            elapsed_seconds: 9999,
            pairs_matched: 0,
            mismatches: 0,
        };
        // base: 0, streak: 0, penalty: 100*5+100*10=1500 (saturates to 0), time_bonus: 0
        assert_eq!(tracker.calculate_score(), 0);
    }

    #[test]
    fn score_starts_at_zero() {
        let tracker = ScoreTracker {
            hints_used: 0,
            shuffles_used: 0,
            undos_used: 0,
            elapsed_seconds: 0,
            pairs_matched: 0,
            mismatches: 0,
        };
        // No pairs matched, no time bonus (elapsed_seconds == 0 means in-game)
        assert_eq!(tracker.calculate_score(), 0);
    }

    #[test]
    fn score_increases_with_pairs() {
        let tracker = ScoreTracker {
            hints_used: 0,
            shuffles_used: 0,
            undos_used: 0,
            elapsed_seconds: 0,
            pairs_matched: 5,
            mismatches: 0,
        };
        // base: 5*10=50, streak: 5*2=10, no time bonus during game
        assert_eq!(tracker.live_score(), 60);
    }

    #[test]
    fn name_entry_new_creates_empty_state() {
        let entry = NameEntryState::new(500, 120, 0, 0, 0);
        assert_eq!(entry.text, "");
        assert_eq!(entry.score, 500);
        assert_eq!(entry.time_seconds, 120);
        assert!(!entry.is_valid()); // Empty name is invalid
    }

    #[test]
    fn name_entry_push_char_adds_characters() {
        let mut entry = NameEntryState::new(500, 120, 0, 0, 0);
        assert!(entry.push_char('A'));
        assert!(entry.push_char('l'));
        assert!(entry.push_char('i'));
        assert_eq!(entry.text, "Ali");
        assert!(entry.is_valid());
    }

    #[test]
    fn name_entry_push_char_rejects_beyond_20() {
        let mut entry = NameEntryState::new(500, 120, 0, 0, 0);
        for c in "12345678901234567890".chars() {
            assert!(entry.push_char(c));
        }
        assert_eq!(entry.text.chars().count(), 20);
        assert!(entry.is_valid());

        // 21st character should be rejected
        assert!(!entry.push_char('X'));
        assert_eq!(entry.text.chars().count(), 20);
    }

    #[test]
    fn name_entry_pop_char_removes_last() {
        let mut entry = NameEntryState::new(500, 120, 0, 0, 0);
        entry.push_char('H');
        entry.push_char('i');
        assert!(entry.pop_char());
        assert_eq!(entry.text, "H");
        assert!(entry.pop_char());
        assert_eq!(entry.text, "");
        // Popping empty string returns false
        assert!(!entry.pop_char());
    }

    #[test]
    fn name_entry_is_valid_checks_1_to_20_chars() {
        let mut entry = NameEntryState::new(500, 120, 0, 0, 0);
        assert!(!entry.is_valid()); // 0 chars: invalid

        entry.push_char('A');
        assert!(entry.is_valid()); // 1 char: valid

        for c in "BCDEFGHIJKLMNOPQRST".chars() {
            entry.push_char(c);
        }
        assert_eq!(entry.text.chars().count(), 20);
        assert!(entry.is_valid()); // 20 chars: valid
    }

    #[test]
    fn test_name_entry_navigation() {
        let profiles = vec![
            UserProfile {
                name: "Alice".to_string(),
                has_save: true,
                save_level: 2,
                max_completed_level: 1,
                streak: 3,
                best_score: 500,
                last_played_epoch_secs: 100,
            },
            UserProfile {
                name: "Bob".to_string(),
                has_save: false,
                save_level: 1,
                max_completed_level: 5,
                streak: 1,
                best_score: 800,
                last_played_epoch_secs: 90,
            },
        ];

        let mut entry = NameEntryState::with_profiles(profiles, 0, 0, 0, 0, 0);
        assert_eq!(entry.selected_index, 0);
        assert_eq!(entry.selected_profile().unwrap().name, "Alice");
        assert!(!entry.is_new_player_selected());

        // Select next -> Bob
        entry.select_next();
        assert_eq!(entry.selected_index, 1);
        assert_eq!(entry.selected_profile().unwrap().name, "Bob");

        // Select next -> New Player input (index 2)
        entry.select_next();
        assert_eq!(entry.selected_index, 2);
        assert!(entry.is_new_player_selected());

        // Select next wraps to 0
        entry.select_next();
        assert_eq!(entry.selected_index, 0);

        // Select prev wraps to New Player
        entry.select_prev();
        assert_eq!(entry.selected_index, 2);
        assert!(entry.is_new_player_selected());

        // Toggle field toggles between 0 and 2
        entry.toggle_field();
        assert_eq!(entry.selected_index, 0);
        entry.toggle_field();
        assert_eq!(entry.selected_index, 2);
    }
}
