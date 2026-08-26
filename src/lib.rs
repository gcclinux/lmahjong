//! xMahjong library crate.
//!
//! Exports all game modules publicly for testability.

/// Board state management, tile positions, layout, and free-tile detection.
pub mod board;

/// Solvable board generation using reverse-deal algorithm.
pub mod generator;

/// Game logic: tile selection, matching, undo, shuffle, and state transitions.
pub mod logic;

/// SDL2 rendering: tiles, UI overlays, animations, and window management.
pub mod renderer;

/// Input handling: SDL2 event processing and keyboard shortcut mapping.
pub mod input;

/// Audio management: sound effects and mute control via SDL2_mixer.
pub mod audio;

/// Persistence: leaderboard and settings storage.
pub mod storage;

/// Game timer with pause/resume support.
pub mod timer;

/// Central game state structure and status enum.
pub mod game_state;

/// Level system: tile count and face pool computation per level.
pub mod levels;

/// Internationalization: multi-language translations and localization.
pub mod i18n;
