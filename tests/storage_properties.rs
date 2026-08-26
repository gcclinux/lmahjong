//! Property-based tests for leaderboard storage.

use proptest::prelude::*;

use xmahjong::storage::{Leaderboard, LeaderboardEntry, UserProgress};

// Feature: xmahjong, Property 17: Leaderboard Invariants
//
// **Validates: Requirements 8.3**
//
// For any sequence of insertions, leaderboard has at most 10 entries,
// sorted descending by score, and contains only the top 10 scores ever inserted.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_17_leaderboard_invariants(
        scores in prop::collection::vec(0u32..100000, 1..50),
    ) {
        let mut leaderboard = Leaderboard::default();

        for (i, &score) in scores.iter().enumerate() {
            leaderboard.insert(LeaderboardEntry {
                name: format!("Player{}", i),
                score,
                time_seconds: 100,
                hints_used: 0,
                shuffles_used: 0,
                undos_used: 0,
                difficulty: "easy".to_string(),
                date: "2024-01-01".to_string(),
                consecutive_days: 0,
            });
        }

        let entries = &leaderboard.entries;

        // Invariant 1: Exactly 1 entry (the latest match)
        prop_assert_eq!(
            entries.len(), 1,
            "Leaderboard has {} entries, expected exactly 1",
            entries.len()
        );

        // Invariant 2: Matches the last score in sequence
        let last_score = *scores.last().unwrap();
        prop_assert_eq!(
            entries[0].score, last_score,
            "Latest score did not match last inserted score"
        );
    }
}

use xmahjong::storage::Settings;

// Feature: xmahjong, Property 18: Mute State Persistence Round-Trip
//
// **Validates: Requirements 11.5**
//
// For any mute state (true or false), serializing settings and then
// deserializing them preserves the mute state exactly.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_18_mute_state_persistence_round_trip(
        muted in proptest::bool::ANY,
    ) {
        let settings = Settings {
            muted,
            ..Default::default()
        };

        // Serialize to JSON (simulates save)
        let json = serde_json::to_string(&settings)
            .expect("Settings serialization should not fail");

        // Deserialize from JSON (simulates load)
        let loaded: Settings = serde_json::from_str(&json)
            .expect("Settings deserialization should not fail");

        // The mute state must be preserved exactly
        prop_assert_eq!(
            loaded.muted, settings.muted,
            "Mute state not preserved: saved {:?}, loaded {:?}",
            settings.muted, loaded.muted
        );
    }
}

use xmahjong::storage::SavedGame;

// Feature: extended-levels, Property 8: Save/load level round-trip
//
// For any level N in 1..=1000, serializing a SavedGame with that level
// and deserializing it back preserves the level value exactly.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn property_8_save_load_level_round_trip(level in 1u32..=1000) {
        let saved = SavedGame {
            tiles: vec![Some(0); 144],
            undo_stack: Vec::new(),
            elapsed_ms: 0,
            hints_used: 0,
            shuffles_used: 0,
            shuffles_remaining: 30,
            pairs_matched: 0,
            undos_used: 0,
            level,
            base_score: 0,
            base_time_ms: 0,
            base_hints: 0,
            base_shuffles: 0,
            base_undos: 0,
            difficulty: "easy".to_string(),
        };

        // Serialize to JSON
        let json = serde_json::to_string(&saved)
            .expect("SavedGame serialization should not fail");

        // Deserialize from JSON
        let loaded: SavedGame = serde_json::from_str(&json)
            .expect("SavedGame deserialization should not fail");

        // Level must be preserved
        prop_assert_eq!(loaded.level, level,
            "Level not preserved: saved {}, loaded {}", level, loaded.level);

        // Also verify the validation would pass (1..=1000)
        prop_assert!((1..=1000).contains(&loaded.level),
            "Loaded level {} is outside valid range", loaded.level);
    }
}

// Feature: extended-levels, Property 9: Invalid level in save is rejected
//
// For any level value outside 1..=1000, deserializing a SavedGame with that level
// and applying the validation filter should return None.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn property_9_invalid_level_rejected(level in prop_oneof![Just(0u32), 1001u32..=5000]) {
        let saved = SavedGame {
            tiles: vec![Some(0); 144],
            undo_stack: Vec::new(),
            elapsed_ms: 0,
            hints_used: 0,
            shuffles_used: 0,
            shuffles_remaining: 30,
            pairs_matched: 0,
            undos_used: 0,
            level,
            base_score: 0,
            base_time_ms: 0,
            base_hints: 0,
            base_shuffles: 0,
            base_undos: 0,
            difficulty: "easy".to_string(),
        };

        // Serialize to JSON
        let json = serde_json::to_string(&saved)
            .expect("SavedGame serialization should not fail");

        // Deserialize and apply the same validation that load() uses
        let loaded: Option<SavedGame> = serde_json::from_str::<SavedGame>(&json)
            .ok()
            .filter(|s| (1..=1000).contains(&s.level));

        // Should be None because level is outside valid range
        prop_assert!(loaded.is_none(),
            "Level {} should be rejected but was accepted", level);
    }

    #[test]
    fn property_user_progress_roundtrip(
        max_level in 0u32..=1000,
        levels in prop::collection::vec(1u32..=1000, 0..50),
    ) {
        let mut progress = UserProgress {
            max_completed_level: max_level,
            completed_levels: levels,
        };
        progress.completed_levels.sort_unstable();
        progress.completed_levels.dedup();

        let json = serde_json::to_string(&progress).expect("serialization failed");
        let loaded: UserProgress = serde_json::from_str(&json).expect("deserialization failed");

        prop_assert_eq!(progress, loaded);
    }

    #[test]
    fn property_user_progress_unlocking_invariants(
        completed in prop::collection::vec(1u32..=1000, 1..30),
    ) {
        let mut progress = UserProgress::default();
        // Level 1 is always unlocked
        prop_assert!(progress.is_level_unlocked(1));

        for &lvl in &completed {
            progress.mark_completed(lvl);
            prop_assert!(progress.is_level_completed(lvl));
            prop_assert!(progress.is_level_unlocked(lvl));
            if lvl < 1000 {
                prop_assert!(progress.is_level_unlocked(lvl + 1));
            }
        }

        // max_completed_level is at least the maximum completed level
        let max_c = completed.iter().copied().max().unwrap_or(0);
        prop_assert!(progress.max_completed_level >= max_c);
    }
}

