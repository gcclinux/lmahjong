//! Storage module.
//!
//! Handles persistence of leaderboard scores and game settings
//! to JSON files on disk.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Sanitizes a username string so it can be safely used as a directory name.
pub fn sanitize_user_name(user_name: &str) -> String {
    let trimmed = user_name.trim();
    if trimmed.is_empty() {
        return "default".to_string();
    }
    let sanitized: String = trimmed
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    if sanitized.is_empty() {
        "default".to_string()
    } else {
        sanitized
    }
}

/// Returns the base storage directory path without user subfolder.
/// Checks `$SNAP_USER_DATA` first (Snap packages), then uses the platform-appropriate location:
/// - macOS:   `~/Library/Application Support/xmahjong/`
/// - Windows: `%APPDATA%\xmahjong\`
/// - Linux:   `~/.local/share/xmahjong/`
pub fn base_storage_dir() -> PathBuf {
    if let Ok(snap_dir) = std::env::var("SNAP_USER_DATA") {
        PathBuf::from(snap_dir)
    } else {
        dirs_fallback()
    }
}

/// Returns the storage directory path for a specific username.
/// E.g. `~/.local/share/xmahjong/<user_name>/`
pub fn storage_dir_for_user(user_name: &str) -> PathBuf {
    let safe_name = sanitize_user_name(user_name);
    base_storage_dir().join(safe_name)
}

/// Returns the resolved storage directory for a user, checking for existing case-insensitive directories.
pub fn resolve_user_dir(user_name: &str) -> PathBuf {
    let exact = storage_dir_for_user(user_name);
    if exact.exists() {
        return exact;
    }
    let base = base_storage_dir();
    if let Ok(entries) = fs::read_dir(&base) {
        let lower = user_name.to_lowercase();
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.to_lowercase() == lower {
                        return entry.path();
                    }
                }
            }
        }
    }
    exact
}

/// Platform-appropriate storage directory fallback.
fn dirs_fallback() -> PathBuf {
    if cfg!(target_os = "macos") {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("xmahjong")
    } else if cfg!(target_os = "windows") {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(appdata).join("xmahjong")
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("xmahjong")
    }
}

/// A single leaderboard entry recording a completed game.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LeaderboardEntry {
    /// Player name (1-20 characters).
    pub name: String,
    /// Final game score.
    pub score: u32,
    /// Time to complete in seconds.
    pub time_seconds: u32,
    /// Total number of hints used across all levels.
    #[serde(default)]
    pub hints_used: u32,
    /// Total number of shuffles used across all levels.
    #[serde(default)]
    pub shuffles_used: u32,
    /// Total number of undos used across all levels.
    #[serde(default)]
    pub undos_used: u32,
    /// Difficulty level ("easy" or "normal").
    #[serde(default = "default_difficulty_str")]
    pub difficulty: String,
    /// Date of completion in ISO 8601 format.
    pub date: String,
    /// Number of consecutive days played when score was achieved.
    #[serde(default)]
    pub consecutive_days: u32,
}

impl LeaderboardEntry {
    /// Validates the entry's name. Returns true if name is 1-20 characters.
    pub fn is_valid_name(name: &str) -> bool {
        let len = name.chars().count();
        (1..=20).contains(&len)
    }

    /// Dynamically calculates achievements based on the entry's stats.
    pub fn get_achievements(&self) -> Vec<String> {
        let mut achs = Vec::new();
        if self.hints_used == 0 {
            achs.push("NO-HNT".to_string());
        }
        if self.shuffles_used == 0 {
            achs.push("NO-SHF".to_string());
        }
        if self.undos_used == 0 {
            achs.push("NO-UND".to_string());
        }
        if self.time_seconds > 0 && self.time_seconds < 180 {
            achs.push("SPEEDY".to_string());
        }
        if self.consecutive_days > 1 {
            achs.push(format!("STRK:{}", self.consecutive_days));
        }
        achs
    }
}

/// Leaderboard holding up to 10 entries sorted descending by score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Leaderboard {
    pub entries: Vec<LeaderboardEntry>,
}

impl Default for Leaderboard {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl Leaderboard {
    /// Loads the leaderboard from disk for a specific user.
    /// Returns a default (empty) leaderboard on any read or parse error.
    pub fn load(user_name: &str) -> Self {
        let path = resolve_user_dir(user_name).join("leaderboard.json");
        match fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Saves the leaderboard to disk for a specific user.
    /// Creates directories as needed. Logs errors to stderr but does not crash.
    pub fn save(&self, user_name: &str) {
        let dir = storage_dir_for_user(user_name);
        if let Err(e) = fs::create_dir_all(&dir) {
            eprintln!("xmahjong: failed to create storage directory {:?}: {}", dir, e);
            return;
        }
        let path = dir.join("leaderboard.json");
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = fs::write(&path, json) {
                    eprintln!("xmahjong: failed to write leaderboard to {:?}: {}", path, e);
                }
            }
            Err(e) => {
                eprintln!("xmahjong: failed to serialize leaderboard: {}", e);
            }
        }
    }

    /// Returns true if the given score qualifies for the achievement board (always true now).
    pub fn qualifies(&self, _score: u32) -> bool {
        true
    }

    /// Inserts a new entry, replacing any previous entry to only store the last match.
    pub fn insert(&mut self, entry: LeaderboardEntry) {
        self.entries.clear();
        self.entries.push(entry);
    }
}

/// Persistent trophy achievement state.
///
/// Stored as `trophies.json` in the storage directory.
/// Tracks cumulative counts for repeatable achievements:
/// - Perfect Combo: complete a level with zero mismatches (no wrong tile pair selections)
/// - Rapid Clear: complete a level within the time threshold for its difficulty tier
///
/// Rapid Clear thresholds (seconds):
///   Levels  1-10 (Easy):   120s
///   Levels 11-20 (Medium): 180s
///   Levels 21-30 (Hard):   240s
///   Levels 31-40 (Expert): 300s
///   Levels 41-50+:         360s
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrophyState {
    /// Number of times a Perfect Combo was achieved (0 mismatches in a completed level).
    #[serde(default)]
    pub perfect_combo_count: u32,
    /// Number of times a Rapid Clear was achieved (level completed within time threshold).
    #[serde(default)]
    pub rapid_clear_count: u32,
    /// Number of levels cleared without using any hints.
    #[serde(default)]
    pub no_hints_count: u32,
    /// Number of levels cleared without using any shuffles.
    #[serde(default)]
    pub no_shuffles_count: u32,
    /// Number of levels cleared without using any undos.
    #[serde(default)]
    pub no_undos_count: u32,
    /// Total career score accumulated across all completed levels.
    #[serde(default)]
    pub total_career_score: u64,
}

impl Default for TrophyState {
    fn default() -> Self {
        Self {
            perfect_combo_count: 0,
            rapid_clear_count: 0,
            no_hints_count: 0,
            no_shuffles_count: 0,
            no_undos_count: 0,
            total_career_score: 0,
        }
    }
}

impl TrophyState {
    /// Loads the trophy state from disk for a specific user.
    /// Returns default state on any read or parse error.
    pub fn load(user_name: &str) -> Self {
        let path = resolve_user_dir(user_name).join("trophies.json");
        match fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Saves the trophy state to disk for a specific user.
    pub fn save(&self, user_name: &str) {
        let dir = storage_dir_for_user(user_name);
        if let Err(e) = fs::create_dir_all(&dir) {
            eprintln!("xmahjong: failed to create storage directory {:?}: {}", dir, e);
            return;
        }
        let path = dir.join("trophies.json");
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = fs::write(&path, json) {
                    eprintln!("xmahjong: failed to write trophy state to {:?}: {}", path, e);
                }
            }
            Err(e) => {
                eprintln!("xmahjong: failed to serialize trophy state: {}", e);
            }
        }
    }

    /// Returns the rapid clear time threshold in seconds for a given level.
    /// Lower levels have tighter thresholds (easier boards = less time allowed).
    pub fn rapid_clear_threshold(level: u32) -> u32 {
        match level {
            1..=10 => 120,   // Easy boards: 2 minutes
            11..=20 => 180,  // Medium boards: 3 minutes
            21..=30 => 240,  // Hard boards: 4 minutes
            31..=40 => 300,  // Expert boards: 5 minutes
            _ => 360,        // Master boards: 6 minutes
        }
    }

    /// Checks if a Perfect Combo was achieved (no mismatches in the level).
    /// If so, increments the counter and returns true.
    pub fn check_perfect_combo(&mut self, mismatches: u32) -> bool {
        if mismatches == 0 {
            self.perfect_combo_count += 1;
            true
        } else {
            false
        }
    }

    /// Checks if a Rapid Clear was achieved (level completed within threshold).
    /// If so, increments the counter and returns true.
    pub fn check_rapid_clear(&mut self, level: u32, elapsed_seconds: u32) -> bool {
        let threshold = Self::rapid_clear_threshold(level);
        if elapsed_seconds > 0 && elapsed_seconds <= threshold {
            self.rapid_clear_count += 1;
            true
        } else {
            false
        }
    }

    /// Checks if a level was cleared without hints. Increments counter if so.
    pub fn check_no_hints(&mut self, hints_used: u32) -> bool {
        if hints_used == 0 {
            self.no_hints_count += 1;
            true
        } else {
            false
        }
    }

    /// Checks if a level was cleared without shuffles. Increments counter if so.
    pub fn check_no_shuffles(&mut self, shuffles_used: u32) -> bool {
        if shuffles_used == 0 {
            self.no_shuffles_count += 1;
            true
        } else {
            false
        }
    }

    /// Checks if a level was cleared without undos. Increments counter if so.
    pub fn check_no_undos(&mut self, undos_used: u32) -> bool {
        if undos_used == 0 {
            self.no_undos_count += 1;
            true
        } else {
            false
        }
    }
}

/// Game settings that persist across sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub muted: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { muted: false }
    }
}

impl Settings {
    /// Loads user settings from disk. Returns default settings on any read/parse error.
    pub fn load(user_name: &str) -> Self {
        let path = resolve_user_dir(user_name).join("settings.json");
        match fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Saves settings to disk for a specific user.
    /// Creates directories as needed. Logs errors to stderr but does not crash.
    pub fn save(&self, user_name: &str) {
        let dir = storage_dir_for_user(user_name);
        if let Err(e) = fs::create_dir_all(&dir) {
            eprintln!("xmahjong: failed to create storage directory {:?}: {}", dir, e);
            return;
        }
        let path = dir.join("settings.json");
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = fs::write(&path, json) {
                    eprintln!("xmahjong: failed to write settings to {:?}: {}", path, e);
                }
            }
            Err(e) => {
                eprintln!("xmahjong: failed to serialize settings: {}", e);
            }
        }
    }
}

/// Persistent shuffle state tracking daily bonus.
///
/// Stored as `shuffles.json` in the storage directory.
/// Tracks the last date the game was launched (for +1 daily bonus).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShuffleState {
    /// The last date (ISO 8601 YYYY-MM-DD) the user launched the game and received a daily bonus.
    pub last_bonus_date: String,
    /// Days since unix epoch of the last launch.
    #[serde(default)]
    pub last_launch_epoch_days: u64,
    /// Number of consecutive days launched.
    #[serde(default)]
    pub consecutive_days: u32,
    /// Maximum streak record achieved.
    #[serde(default)]
    pub best_streak: u32,
}

impl Default for ShuffleState {
    fn default() -> Self {
        Self {
            last_bonus_date: String::new(),
            last_launch_epoch_days: 0,
            consecutive_days: 0,
            best_streak: 0,
        }
    }
}

impl ShuffleState {
    /// Loads shuffle state from disk for a specific user.
    /// Returns default state (1 shuffle) if no file exists or if it's corrupt.
    pub fn load(user_name: &str) -> Self {
        let path = resolve_user_dir(user_name).join("shuffles.json");
        let mut state: Self = match fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        };
        if state.consecutive_days > state.best_streak {
            state.best_streak = state.consecutive_days;
        }
        state
    }

    /// Saves the shuffle state to disk for a specific user.
    pub fn save(&self, user_name: &str) {
        let dir = storage_dir_for_user(user_name);
        if let Err(e) = fs::create_dir_all(&dir) {
            eprintln!("xmahjong: failed to create storage directory {:?}: {}", dir, e);
            return;
        }
        let path = dir.join("shuffles.json");
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = fs::write(&path, json) {
                    eprintln!("xmahjong: failed to write shuffle state to {:?}: {}", path, e);
                }
            }
            Err(e) => {
                eprintln!("xmahjong: failed to serialize shuffle state: {}", e);
            }
        }
    }

    /// Checks if a daily bonus should be applied for the given date.
    /// Returns true if today is different from the last bonus date (bonus should be given).
    /// Updates the last_bonus_date to today.
    pub fn claim_daily_bonus(&mut self, today: &str) -> bool {
        if self.last_bonus_date != today {
            self.last_bonus_date = today.to_string();
            true
        } else {
            false
        }
    }
}

/// Represents a saved game state that can be resumed later.
///
/// Stores only the minimal data needed to reconstruct the game:
/// board tile face IDs (None for removed), undo stack, timer, and score tracker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedGame {
    /// For each of the 144 positions: `Some(face_id)` if a tile is present, `None` if removed.
    pub tiles: Vec<Option<u8>>,
    /// Undo stack: each entry records (pos_a, face_a, pos_b, face_b).
    pub undo_stack: Vec<(usize, u8, usize, u8)>,
    /// Elapsed time in milliseconds at the time of save.
    pub elapsed_ms: u64,
    /// Number of hints used.
    pub hints_used: u32,
    /// Number of shuffles used.
    pub shuffles_used: u32,
    /// Number of shuffles remaining.
    pub shuffles_remaining: u32,
    /// Number of pairs matched so far.
    pub pairs_matched: u32,
    /// Number of undos used this level.
    #[serde(default)]
    pub undos_used: u32,
    /// Current level.
    #[serde(default = "default_level")]
    pub level: u32,
    /// Accumulated score from previous levels.
    #[serde(default)]
    pub base_score: u32,
    /// Accumulated time in milliseconds from previous levels.
    #[serde(default)]
    pub base_time_ms: u64,
    /// Accumulated hints used from previous levels.
    #[serde(default)]
    pub base_hints: u32,
    /// Accumulated shuffles used from previous levels.
    #[serde(default)]
    pub base_shuffles: u32,
    /// Accumulated undos used from previous levels.
    #[serde(default)]
    pub base_undos: u32,
    /// Difficulty level for this session ("easy" or "normal").
    #[serde(default = "default_difficulty_str")]
    pub difficulty: String,
}

/// Default level value for backwards compatibility with old save files.
fn default_level() -> u32 {
    1
}

/// Default difficulty value for backwards compatibility with old save files.
fn default_difficulty_str() -> String {
    "easy".to_string()
}

impl SavedGame {
    /// Loads a saved game from disk for a specific user. Returns None if no save exists, is corrupt,
    /// or contains a level outside the valid range (1-1000).
    pub fn load(user_name: &str) -> Option<Self> {
        let path = resolve_user_dir(user_name).join("savegame.json");
        match fs::read_to_string(&path) {
            Ok(contents) => {
                let saved: Option<Self> = serde_json::from_str(&contents).ok();
                saved.filter(|s| (1..=1000).contains(&s.level))
            }
            Err(_) => None,
        }
    }

    /// Saves the game state to disk for a specific user.
    pub fn save(&self, user_name: &str) {
        let dir = storage_dir_for_user(user_name);
        if let Err(e) = fs::create_dir_all(&dir) {
            eprintln!("xmahjong: failed to create storage directory {:?}: {}", dir, e);
            return;
        }
        let path = dir.join("savegame.json");
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = fs::write(&path, json) {
                    eprintln!("xmahjong: failed to write savegame to {:?}: {}", path, e);
                }
            }
            Err(e) => {
                eprintln!("xmahjong: failed to serialize savegame: {}", e);
            }
        }
    }

    /// Deletes the saved game file for a specific user (e.g., after successfully loading it).
    pub fn delete(user_name: &str) {
        let path = storage_dir_for_user(user_name).join("savegame.json");
        let _ = fs::remove_file(&path);
        let resolved = resolve_user_dir(user_name).join("savegame.json");
        if resolved != path {
            let _ = fs::remove_file(&resolved);
        }
    }

    /// Returns true if a saved game file exists on disk for a specific user.
    pub fn exists(user_name: &str) -> bool {
        resolve_user_dir(user_name).join("savegame.json").exists()
    }
}

/// Persistent level completion progress tracking for a user.
///
/// Stored as `progress.json` in the user's storage directory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserProgress {
    /// Highest level completed by the user (0 if none completed yet).
    #[serde(default)]
    pub max_completed_level: u32,
    /// Specific completed level numbers.
    #[serde(default)]
    pub completed_levels: Vec<u32>,
}

impl Default for UserProgress {
    fn default() -> Self {
        Self {
            max_completed_level: 0,
            completed_levels: Vec::new(),
        }
    }
}

impl UserProgress {
    /// Loads the user's level progress from disk.
    /// Includes backwards compatibility checks with savegame.json and existing user directories.
    pub fn load(user_name: &str) -> Self {
        let dir = resolve_user_dir(user_name);
        let path = dir.join("progress.json");
        let mut progress: Self = match fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        };

        // Backwards compatibility check: if max_completed_level is 0, check savegame.json
        if progress.max_completed_level == 0 {
            if let Some(save) = SavedGame::load(user_name) {
                if save.level > 1 {
                    progress.max_completed_level = save.level - 1;
                    progress.completed_levels = (1..save.level).collect();
                }
            }
        }

        // Also check if any case-insensitive directory match has progress or saved games
        if progress.max_completed_level == 0 {
            let base = base_storage_dir();
            if let Ok(entries) = fs::read_dir(&base) {
                let lower = user_name.to_lowercase();
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        if let Some(name) = entry.file_name().to_str() {
                            if name.to_lowercase() == lower {
                                let other_progress_path = entry.path().join("progress.json");
                                if let Ok(c) = fs::read_to_string(&other_progress_path) {
                                    if let Ok(p) = serde_json::from_str::<UserProgress>(&c) {
                                        if p.max_completed_level > progress.max_completed_level {
                                            progress = p;
                                        }
                                    }
                                }
                                let other_save_path = entry.path().join("savegame.json");
                                if let Ok(c) = fs::read_to_string(&other_save_path) {
                                    if let Ok(save) = serde_json::from_str::<SavedGame>(&c) {
                                        if save.level > 1 && save.level - 1 > progress.max_completed_level {
                                            progress.max_completed_level = save.level - 1;
                                            progress.completed_levels = (1..save.level).collect();
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if progress.max_completed_level > 0 && progress.completed_levels.is_empty() {
            progress.completed_levels = (1..=progress.max_completed_level).collect();
        }

        progress
    }

    /// Loads user progress and synchronizes with current game level in memory.
    /// If current_game_level is greater than 1 (e.g. level 13), marks levels 1..current_game_level (1..12) as completed.
    pub fn load_and_sync(user_name: &str, current_game_level: u32) -> Self {
        let mut progress = Self::load(user_name);
        let mut changed = false;

        if current_game_level > 1 {
            let completed_up_to = (current_game_level - 1).min(1000);
            if completed_up_to > progress.max_completed_level {
                progress.max_completed_level = completed_up_to;
                changed = true;
            }
            for lvl in 1..=completed_up_to {
                if !progress.completed_levels.contains(&lvl) {
                    progress.completed_levels.push(lvl);
                    changed = true;
                }
            }
        }

        if progress.max_completed_level > 0 {
            for lvl in 1..=progress.max_completed_level {
                if !progress.completed_levels.contains(&lvl) {
                    progress.completed_levels.push(lvl);
                    changed = true;
                }
            }
        }

        if changed {
            progress.completed_levels.sort_unstable();
            progress.completed_levels.dedup();
            progress.save(user_name);
        }

        progress
    }

    /// Saves the user's level progress to disk.
    pub fn save(&self, user_name: &str) {
        let dir = storage_dir_for_user(user_name);
        if let Err(e) = fs::create_dir_all(&dir) {
            eprintln!("xmahjong: failed to create storage directory {:?}: {}", dir, e);
            return;
        }
        let path = dir.join("progress.json");
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = fs::write(&path, json) {
                    eprintln!("xmahjong: failed to write progress to {:?}: {}", path, e);
                }
            }
            Err(e) => {
                eprintln!("xmahjong: failed to serialize progress: {}", e);
            }
        }
    }

    /// Marks a level as completed and updates max_completed_level.
    /// Returns true if progress was updated.
    pub fn mark_completed(&mut self, level: u32) -> bool {
        let mut changed = false;
        if !self.completed_levels.contains(&level) {
            self.completed_levels.push(level);
            self.completed_levels.sort_unstable();
            changed = true;
        }
        if level > self.max_completed_level {
            self.max_completed_level = level;
            changed = true;
        }
        changed
    }

    /// Checks whether a given level is unlocked / available to play.
    /// Level 1 is always unlocked. Any completed level or the next playable level is unlocked.
    pub fn is_level_unlocked(&self, level: u32) -> bool {
        if level == 1 {
            return true;
        }
        if level <= self.max_completed_level + 1 && level <= 1000 {
            return true;
        }
        self.completed_levels.contains(&level)
    }

    /// Checks whether a given level is completed.
    pub fn is_level_completed(&self, level: u32) -> bool {
        self.completed_levels.contains(&level) || (self.max_completed_level > 0 && level <= self.max_completed_level)
    }
}

/// Summary profile for an existing player discovered in local storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserProfile {
    /// Canonical display name of the player.
    pub name: String,
    /// Whether an in-progress saved game exists.
    pub has_save: bool,
    /// Level number of the saved game (if any, default 1).
    pub save_level: u32,
    /// Highest level completed by the player (0 if none completed yet).
    pub max_completed_level: u32,
    /// Consecutive launch day streak.
    pub streak: u32,
    /// Best recorded leaderboard score.
    pub best_score: u32,
    /// Last played or modified time in epoch seconds (for sorting most recent first).
    pub last_played_epoch_secs: u64,
}

/// Scans local storage for all existing player profiles.
/// Returns a list of player profiles sorted by most recently active first.
pub fn list_user_profiles() -> Vec<UserProfile> {
    let base = base_storage_dir();
    let entries = match fs::read_dir(&base) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    use std::collections::HashMap;
    let mut profile_map: HashMap<String, UserProfile> = HashMap::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let file_name = match entry.file_name().into_string() {
            Ok(name) => name,
            Err(_) => continue,
        };

        // Filter out hidden directories, empty, or scratch
        if file_name.is_empty() || file_name.starts_with('.') || file_name == "scratch" {
            continue;
        }

        // Determine the latest modification time among the directory and its JSON files
        let mut latest_mtime: u64 = fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Check savegame.json
        let save_path = path.join("savegame.json");
        let (has_save, save_level) = if save_path.exists() {
            if let Ok(m) = fs::metadata(&save_path).and_then(|m| m.modified()) {
                if let Ok(d) = m.duration_since(std::time::UNIX_EPOCH) {
                    latest_mtime = latest_mtime.max(d.as_secs());
                }
            }
            if let Ok(c) = fs::read_to_string(&save_path) {
                if let Ok(s) = serde_json::from_str::<SavedGame>(&c) {
                    (true, s.level)
                } else {
                    (false, 1)
                }
            } else {
                (false, 1)
            }
        } else {
            (false, 1)
        };

        // Check progress.json
        let progress_path = path.join("progress.json");
        let max_completed_level = if progress_path.exists() {
            if let Ok(m) = fs::metadata(&progress_path).and_then(|m| m.modified()) {
                if let Ok(d) = m.duration_since(std::time::UNIX_EPOCH) {
                    latest_mtime = latest_mtime.max(d.as_secs());
                }
            }
            if let Ok(c) = fs::read_to_string(&progress_path) {
                if let Ok(p) = serde_json::from_str::<UserProgress>(&c) {
                    p.max_completed_level
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            0
        };

        // Check shuffles.json (for streak)
        let shuffles_path = path.join("shuffles.json");
        let streak = if shuffles_path.exists() {
            if let Ok(m) = fs::metadata(&shuffles_path).and_then(|m| m.modified()) {
                if let Ok(d) = m.duration_since(std::time::UNIX_EPOCH) {
                    latest_mtime = latest_mtime.max(d.as_secs());
                }
            }
            if let Ok(c) = fs::read_to_string(&shuffles_path) {
                if let Ok(s) = serde_json::from_str::<ShuffleState>(&c) {
                    s.best_streak.max(s.consecutive_days)
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            0
        };

        // Check leaderboard.json (for best score)
        let lb_path = path.join("leaderboard.json");
        let best_score = if lb_path.exists() {
            if let Ok(m) = fs::metadata(&lb_path).and_then(|m| m.modified()) {
                if let Ok(d) = m.duration_since(std::time::UNIX_EPOCH) {
                    latest_mtime = latest_mtime.max(d.as_secs());
                }
            }
            if let Ok(c) = fs::read_to_string(&lb_path) {
                if let Ok(lb) = serde_json::from_str::<Leaderboard>(&c) {
                    lb.entries.iter().map(|e| e.score).max().unwrap_or(0)
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            0
        };

        // Also check trophies.json and settings.json timestamps
        for aux_file in &["trophies.json", "settings.json"] {
            let aux_path = path.join(aux_file);
            if let Ok(m) = fs::metadata(&aux_path).and_then(|m| m.modified()) {
                if let Ok(d) = m.duration_since(std::time::UNIX_EPOCH) {
                    latest_mtime = latest_mtime.max(d.as_secs());
                }
            }
        }

        let key = file_name.to_lowercase();
        let profile = UserProfile {
            name: file_name,
            has_save,
            save_level,
            max_completed_level,
            streak,
            best_score,
            last_played_epoch_secs: latest_mtime,
        };

        match profile_map.get_mut(&key) {
            Some(existing) => {
                // Merge info if this dir is more recent or has better stats
                if profile.last_played_epoch_secs > existing.last_played_epoch_secs {
                    existing.name = profile.name;
                    existing.last_played_epoch_secs = profile.last_played_epoch_secs;
                }
                if profile.has_save {
                    existing.has_save = true;
                    existing.save_level = profile.save_level.max(existing.save_level);
                }
                existing.max_completed_level = existing.max_completed_level.max(profile.max_completed_level);
                existing.streak = existing.streak.max(profile.streak);
                existing.best_score = existing.best_score.max(profile.best_score);
            }
            None => {
                profile_map.insert(key, profile);
            }
        }
    }

    let mut profiles: Vec<UserProfile> = profile_map.into_values().collect();
    // Sort descending by last played time, then alphabetically by name
    profiles.sort_by(|a, b| {
        b.last_played_epoch_secs
            .cmp(&a.last_played_epoch_secs)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    profiles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_only_stores_the_last_entry() {
        let mut lb = Leaderboard::default();
        lb.insert(LeaderboardEntry {
            name: "Alice".to_string(),
            score: 500,
            time_seconds: 300,
            hints_used: 0,
            shuffles_used: 0,
            undos_used: 0,
            difficulty: "easy".to_string(),
            date: "2024-01-01".to_string(),
            consecutive_days: 0,
        });
        assert_eq!(lb.entries.len(), 1);
        assert_eq!(lb.entries[0].name, "Alice");

        lb.insert(LeaderboardEntry {
            name: "Bob".to_string(),
            score: 800,
            time_seconds: 200,
            hints_used: 0,
            shuffles_used: 0,
            undos_used: 0,
            difficulty: "easy".to_string(),
            date: "2024-01-02".to_string(),
            consecutive_days: 0,
        });

        assert_eq!(lb.entries.len(), 1);
        assert_eq!(lb.entries[0].name, "Bob");
        assert_eq!(lb.entries[0].score, 800);
    }

    #[test]
    fn qualifies_always_returns_true() {
        let mut lb = Leaderboard::default();
        assert!(lb.qualifies(0));
        assert!(lb.qualifies(1000));
        lb.insert(LeaderboardEntry {
            name: "P".to_string(),
            score: 500,
            time_seconds: 100,
            hints_used: 0,
            shuffles_used: 0,
            undos_used: 0,
            difficulty: "easy".to_string(),
            date: "2024-01-01".to_string(),
            consecutive_days: 0,
        });
        assert!(lb.qualifies(10));
    }

    #[test]
    fn name_validation_1_to_20_chars() {
        // Empty name is invalid
        assert!(!LeaderboardEntry::is_valid_name(""));

        // 1 character is valid
        assert!(LeaderboardEntry::is_valid_name("A"));

        // 20 characters is valid
        assert!(LeaderboardEntry::is_valid_name("12345678901234567890"));

        // 21 characters is invalid
        assert!(!LeaderboardEntry::is_valid_name("123456789012345678901"));
    }

    #[test]
    fn settings_default_mute_is_false() {
        let settings = Settings::default();
        assert!(!settings.muted);
    }

    #[test]
    fn test_sanitize_user_name() {
        assert_eq!(sanitize_user_name("  "), "default");
        assert_eq!(sanitize_user_name("Alice"), "Alice");
        assert_eq!(sanitize_user_name("Bob/Smith"), "Bob_Smith");
        assert_eq!(sanitize_user_name("User:1*"), "User_1_");
    }

    #[test]
    fn test_storage_dir_for_user() {
        let dir = storage_dir_for_user("Alice");
        assert!(dir.ends_with("Alice"));
    }

    #[test]
    fn test_user_progress_load_and_sync() {
        let test_user = "test_sync_user_123";
        let progress = UserProgress::load_and_sync(test_user, 13);
        assert_eq!(progress.max_completed_level, 12);
        assert_eq!(progress.completed_levels.len(), 12);
        for lvl in 1..=12 {
            assert!(progress.is_level_completed(lvl));
            assert!(progress.is_level_unlocked(lvl));
        }
        assert!(progress.is_level_unlocked(13));
        assert!(!progress.is_level_completed(13));
        assert!(!progress.is_level_unlocked(14));

        // Clean up test file
        let path = storage_dir_for_user(test_user).join("progress.json");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(storage_dir_for_user(test_user));
    }

    #[test]
    fn test_list_user_profiles() {
        let test_user = "test_profile_user_abc";
        let dir = storage_dir_for_user(test_user);
        let _ = fs::create_dir_all(&dir);

        let mut progress = UserProgress::default();
        progress.mark_completed(5);
        progress.save(test_user);

        let profiles = list_user_profiles();
        let found = profiles.iter().find(|p| p.name.to_lowercase() == test_user.to_lowercase());
        assert!(found.is_some());
        let p = found.unwrap();
        assert_eq!(p.max_completed_level, 5);

        // Clean up
        let _ = fs::remove_file(dir.join("progress.json"));
        let _ = fs::remove_dir(&dir);
    }
}

