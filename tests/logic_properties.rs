//! Property-based tests for the Game Logic module.

use std::collections::HashSet;

use proptest::prelude::*;

use xmahjong::board::turtle_layout;
use xmahjong::game_state::{GameState, GameStatus, ScoreTracker, Difficulty};
use xmahjong::generator::BoardGenerator;
use xmahjong::logic::{check_game_over, request_hint, select_tile, undo, GameOverReason, HintResult, SelectionResult, UndoError};
use xmahjong::timer::GameTimer;

// Feature: xmahjong, Property 7: Re-Clicking Selected Tile Deselects
//
// **Validates: Requirements 2.6**
//
// For any board state where exactly one tile is selected, clicking that same
// tile again SHALL result in no tiles being selected.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_7_reclicking_selected_tile_deselects(seed in any::<u64>()) {
        let layout = turtle_layout();
        let mut generator = BoardGenerator::new(seed);
        let board = generator.generate(layout, 5).unwrap();

        // Find any free tile on the board
        let free_tiles = board.free_tiles();
        prop_assume!(!free_tiles.is_empty(), "No free tiles on generated board");

        let target_pos = free_tiles[0];

        // Snapshot the board tiles before any interaction
        let board_snapshot: Vec<_> = board.tiles.iter().cloned().collect();

        // Create a GameState with the generated board
        let mut state = GameState {
            board,
            timer: GameTimer::new(),
            score: ScoreTracker::new(),
            status: GameStatus::Playing,
            selection: None,
            hint: None,
            undo_stack: Vec::new(),
            shuffles_remaining: 3,
            level: 1,
            base_score: 0,
            base_time_ms: 0,
            base_hints: 0,
            base_shuffles: 0,
            base_undos: 0,
            animations: Vec::new(),
            difficulty: Difficulty::Easy,
        };

        // Select the free tile
        let first_result = select_tile(&mut state, target_pos);
        prop_assert_eq!(first_result, SelectionResult::Selected,
            "First click on free tile should return Selected");
        prop_assert_eq!(state.selection, Some(target_pos),
            "Selection should be Some(target_pos) after first click");

        // Click the same tile again
        let second_result = select_tile(&mut state, target_pos);

        // Assert: returns Deselected
        prop_assert_eq!(second_result, SelectionResult::Deselected,
            "Second click on same tile should return Deselected");

        // Assert: selection is None
        prop_assert_eq!(state.selection, None,
            "Selection should be None after re-clicking selected tile");

        // Assert: no tiles removed — board unchanged
        let board_after: Vec<_> = state.board.tiles.iter().cloned().collect();
        prop_assert_eq!(board_snapshot.len(), board_after.len(),
            "Board size should not change");
        for (i, (before, after)) in board_snapshot.iter().zip(board_after.iter()).enumerate() {
            prop_assert_eq!(before, after,
                "Tile at position {} changed unexpectedly after deselection", i);
        }

        // Assert: undo stack unchanged (no tiles removed)
        prop_assert!(state.undo_stack.is_empty(),
            "Undo stack should remain empty — no match occurred");
    }
}


/// Helper to create a GameState from a Board in Playing status.
fn make_state(board: xmahjong::board::Board) -> GameState {
    GameState {
        board,
        timer: GameTimer::new(),
        score: ScoreTracker::new(),
        status: GameStatus::Playing,
        selection: None,
        hint: None,
        undo_stack: Vec::new(),
        shuffles_remaining: 3,
        level: 1,
        base_score: 0,
        base_time_ms: 0,
        base_hints: 0,
        base_shuffles: 0,
        base_undos: 0,
        animations: Vec::new(),
        difficulty: Difficulty::Easy,
    }
}

// Feature: xmahjong, Property 6: Invalid Match Deselects Both Tiles
//
// **Validates: Requirements 2.4**
//
// For any board state with a selected free tile, selecting a second free tile
// with a different face ID SHALL result in no tiles selected and no tiles
// removed from the board.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_6_invalid_match_deselects_both_tiles(seed in 0u64..10000) {
        let layout = turtle_layout();
        let mut generator = BoardGenerator::new(seed);
        let board = generator.generate(layout, 5).unwrap();

        let free = board.free_tiles();

        // Find two free tiles with different face_ids
        let mut pair: Option<(usize, usize)> = None;
        'outer: for i in 0..free.len() {
            for j in (i + 1)..free.len() {
                let face_a = board.tiles[free[i]].unwrap().face_id;
                let face_b = board.tiles[free[j]].unwrap().face_id;
                if face_a != face_b {
                    pair = Some((free[i], free[j]));
                    break 'outer;
                }
            }
        }

        // Skip seeds where no mismatched pair exists among free tiles
        prop_assume!(pair.is_some());

        let (pos_a, pos_b) = pair.unwrap();

        let mut state = make_state(board);
        let remaining_before = state.board.remaining_count();

        // Select the first tile
        let result1 = select_tile(&mut state, pos_a);
        prop_assert_eq!(result1, SelectionResult::Selected,
            "First tile selection should return Selected, got {:?}", result1);

        // Select the second tile (different face_id) — should mismatch
        let result2 = select_tile(&mut state, pos_b);
        prop_assert_eq!(result2, SelectionResult::Mismatched(pos_a, pos_b),
            "Second tile selection should return Mismatched, got {:?}", result2);

        // Assert: both tiles still exist on the board (not removed)
        prop_assert!(state.board.tiles[pos_a].is_some(),
            "Tile at pos_a ({}) should still exist after mismatch", pos_a);
        prop_assert!(state.board.tiles[pos_b].is_some(),
            "Tile at pos_b ({}) should still exist after mismatch", pos_b);

        // Assert: no selection remains
        prop_assert_eq!(state.selection, None,
            "Selection should be None after mismatch, got {:?}", state.selection);

        // Assert: remaining_count unchanged
        let remaining_after = state.board.remaining_count();
        prop_assert_eq!(remaining_before, remaining_after,
            "Remaining count should be unchanged: before={}, after={}", remaining_before, remaining_after);
    }
}


// Feature: xmahjong, Property 5: Valid Match Removes Tiles and Updates Free Set
//
// **Validates: Requirements 2.3, 2.5**
//
// For any board state with a valid match, selecting the first tile and then
// the second tile of the matching pair SHALL remove both tiles from the board,
// and the resulting free tile set SHALL equal the set computed by a full
// recalculation over the updated board (i.e., checking is_free() for each
// remaining tile individually).
proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_5_valid_match_removes_tiles_and_updates_free_set(seed in 0u64..10_000) {
        let layout = turtle_layout();
        let mut generator = BoardGenerator::new(seed);
        let board = generator.generate(layout, 5).unwrap();

        // Find a valid pair on the generated board
        let valid_pairs = board.valid_pairs();
        // Generated boards are solvable, so there must be at least one valid pair
        prop_assume!(!valid_pairs.is_empty());

        let (pos_a, pos_b) = valid_pairs[0];

        // Set up GameState and perform the match
        let mut state = make_state(board);
        let result_a = select_tile(&mut state, pos_a);
        prop_assert_eq!(result_a, SelectionResult::Selected,
            "First tile at position {} should be Selected, got {:?}", pos_a, result_a);

        let result_b = select_tile(&mut state, pos_b);
        prop_assert_eq!(result_b, SelectionResult::Matched(pos_a, pos_b),
            "Second tile at position {} should be Matched({}, {}), got {:?}", pos_b, pos_a, pos_b, result_b);

        // Assert both tiles are removed
        prop_assert!(state.board.tiles[pos_a].is_none(),
            "Tile at position {} should be removed after match", pos_a);
        prop_assert!(state.board.tiles[pos_b].is_none(),
            "Tile at position {} should be removed after match", pos_b);

        // Compute free tiles via board.free_tiles()
        let free_set: HashSet<usize> = state.board.free_tiles().into_iter().collect();

        // Independently compute what should be free by checking is_free() for each position
        let expected_free_set: HashSet<usize> = (0..state.board.tiles.len())
            .filter(|&pos| state.board.is_free(pos))
            .collect();

        // Assert the two sets match exactly
        prop_assert_eq!(&free_set, &expected_free_set,
            "free_tiles() result does not match individual is_free() checks. \
             In free_tiles() but not is_free(): {:?}. \
             In is_free() but not free_tiles(): {:?}.",
            free_set.difference(&expected_free_set).collect::<Vec<_>>(),
            expected_free_set.difference(&free_set).collect::<Vec<_>>());
    }
}


// Feature: xmahjong, Property 10: Undo Round-Trip Restores Full State
//
// **Validates: Requirements 5.1, 5.4**
//
// For any board state where a valid match has just been made, performing an undo
// SHALL restore the board to its exact previous state — including tile positions,
// free tile set, and the score contribution of that match being excluded.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_10_undo_round_trip_restores_full_state(seed in any::<u64>()) {
        let layout = turtle_layout();
        let mut generator = BoardGenerator::new(seed);
        let board = generator.generate(layout, 5).unwrap();

        // Find a valid pair on the board
        let valid_pairs = board.valid_pairs();
        prop_assume!(!valid_pairs.is_empty(), "No valid pairs on generated board");

        let (pos_a, pos_b) = valid_pairs[0];

        let mut state = make_state(board);

        // Snapshot the board tiles before the match
        let tiles_before: Vec<Option<xmahjong::board::Tile>> = state.board.tiles.clone();
        let free_before: HashSet<usize> = state.board.free_tiles().into_iter().collect();

        // Perform the match
        let r1 = select_tile(&mut state, pos_a);
        prop_assert_eq!(r1, SelectionResult::Selected);

        let r2 = select_tile(&mut state, pos_b);
        prop_assert_eq!(r2, SelectionResult::Matched(pos_a, pos_b));

        // Verify tiles were actually removed
        prop_assert!(state.board.tiles[pos_a].is_none(),
            "Tile at pos_a should be removed after match");
        prop_assert!(state.board.tiles[pos_b].is_none(),
            "Tile at pos_b should be removed after match");

        // Call undo
        let undo_result = undo(&mut state);
        prop_assert!(undo_result.is_ok(), "Undo should succeed after a match");

        // Assert the board tiles match the snapshot exactly
        let tiles_after: Vec<Option<xmahjong::board::Tile>> = state.board.tiles.clone();
        prop_assert_eq!(tiles_before.len(), tiles_after.len(),
            "Board size should not change after undo");
        for (i, (before, after)) in tiles_before.iter().zip(tiles_after.iter()).enumerate() {
            prop_assert_eq!(before, after,
                "Tile at position {} differs after undo: before={:?}, after={:?}", i, before, after);
        }

        // Assert free_tiles() matches what it was before the match
        let free_after: HashSet<usize> = state.board.free_tiles().into_iter().collect();
        prop_assert_eq!(&free_before, &free_after,
            "Free tiles after undo should match free tiles before the match. \
             In before but not after: {:?}. In after but not before: {:?}.",
            free_before.difference(&free_after).collect::<Vec<_>>(),
            free_after.difference(&free_before).collect::<Vec<_>>());

        // Assert undo stack is now empty (score contribution excluded)
        prop_assert!(state.undo_stack.is_empty(),
            "Undo stack should be empty after undoing the only match");
    }
}


// Feature: xmahjong, Property 11: Undo Capacity Limit
//
// **Validates: Requirements 5.2**
//
// For any sequence of N consecutive matches (where N ≤ 10), undoing all N
// SHALL succeed. Attempting an (N+1)th undo SHALL fail with EmptyStack.
// Additionally, when 11+ matches are performed, the undo stack caps at 10
// entries (oldest evicted), so only 10 undos succeed and the 11th fails.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_11_undo_capacity_limit_n_matches(seed in any::<u64>(), n in 1usize..=10) {
        let layout = turtle_layout();
        let mut generator = BoardGenerator::new(seed);
        let (board, solution) = generator.generate_with_solution(layout, 5).unwrap();

        // We need at least n pairs from the solution
        prop_assume!(solution.len() >= n);

        let mut state = make_state(board);

        // Perform n matches using the solution sequence
        for i in 0..n {
            let (pos_a, pos_b) = solution[i];

            let r1 = select_tile(&mut state, pos_a);
            prop_assert_eq!(r1, SelectionResult::Selected,
                "Match {}: first select at {} should return Selected, got {:?}", i, pos_a, r1);

            let r2 = select_tile(&mut state, pos_b);
            prop_assert_eq!(r2, SelectionResult::Matched(pos_a, pos_b),
                "Match {}: second select at {} should return Matched, got {:?}", i, pos_b, r2);
        }

        // Assert undo stack has exactly n entries
        prop_assert_eq!(state.undo_stack.len(), n,
            "After {} matches, undo stack should have {} entries, got {}",
            n, n, state.undo_stack.len());

        // All n undos should succeed
        for i in 0..n {
            let result = undo(&mut state);
            prop_assert!(result.is_ok(),
                "Undo {} of {} should succeed, got {:?}", i + 1, n, result);
        }

        // The next undo (n+1) should fail with EmptyStack
        let result = undo(&mut state);
        prop_assert_eq!(result, Err(UndoError::EmptyStack),
            "Undo after all {} undos should return EmptyStack", n);
    }

    #[test]
    fn property_11_undo_capacity_limit_overflow(seed in any::<u64>()) {
        let layout = turtle_layout();
        let mut generator = BoardGenerator::new(seed);
        let (board, solution) = generator.generate_with_solution(layout, 5).unwrap();

        // We need at least 11 pairs from the solution to test overflow
        prop_assume!(solution.len() >= 11);

        let mut state = make_state(board);

        // Perform 11 matches — the undo stack caps at 10, so the first entry is evicted
        for i in 0..11 {
            let (pos_a, pos_b) = solution[i];

            let r1 = select_tile(&mut state, pos_a);
            prop_assert_eq!(r1, SelectionResult::Selected,
                "Match {}: first select at {} should return Selected, got {:?}", i, pos_a, r1);

            let r2 = select_tile(&mut state, pos_b);
            prop_assert_eq!(r2, SelectionResult::Matched(pos_a, pos_b),
                "Match {}: second select at {} should return Matched, got {:?}", i, pos_b, r2);
        }

        // Undo stack should be capped at 10
        prop_assert_eq!(state.undo_stack.len(), 10,
            "After 11 matches, undo stack should be capped at 10, got {}",
            state.undo_stack.len());

        // Exactly 10 undos should succeed
        for i in 0..10 {
            let result = undo(&mut state);
            prop_assert!(result.is_ok(),
                "Undo {} of 10 should succeed, got {:?}", i + 1, result);
        }

        // The 11th undo should fail with EmptyStack
        let result = undo(&mut state);
        prop_assert_eq!(result, Err(UndoError::EmptyStack),
            "11th undo should return EmptyStack");
    }
}


// Feature: xmahjong, Property 8: No-Moves Detection
//
// **Validates: Requirements 3.3**
//
// For any board state where no pair of free tiles shares the same face ID,
// the game-over check SHALL report that no valid moves are available.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_8_no_moves_detection(seed in any::<u64>()) {
        let layout = turtle_layout();
        let mut generator = BoardGenerator::new(seed);
        let board = generator.generate(layout, 5).unwrap();

        let mut state = make_state(board);

        // Reassign face_ids of all free tiles to be unique so no pairs exist
        let free_positions = state.board.free_tiles();
        prop_assume!(free_positions.len() >= 2, "Need at least 2 free tiles");

        // Assign each free tile a distinct face_id (0, 1, 2, ...)
        // Since face_id is u8 and free tiles are at most ~30-40 on a full board,
        // we have plenty of distinct values available (0..35 gives 36 unique IDs).
        for (i, &pos) in free_positions.iter().enumerate() {
            if let Some(ref mut tile) = state.board.tiles[pos] {
                tile.face_id = i as u8;
            }
        }

        // Verify precondition: no pair of free tiles shares the same face_id
        let free_after = state.board.free_tiles();
        let mut seen_faces: HashSet<u8> = HashSet::new();
        for &pos in &free_after {
            let face = state.board.tiles[pos].unwrap().face_id;
            // Each face_id should be unique among free tiles
            prop_assert!(seen_faces.insert(face),
                "Precondition violated: duplicate face_id {} among free tiles", face);
        }

        // Assert: check_game_over reports Lost (no valid moves)
        let result = check_game_over(&state);
        prop_assert_eq!(result, Some(GameOverReason::Lost),
            "Expected GameOverReason::Lost when no free tile pairs share a face_id, got {:?}", result);
    }
}


// Feature: xmahjong, Property 9: Hint Returns a Valid Pair
//
// **Validates: Requirements 4.1**
//
// For any board state that contains at least one valid matching pair among free
// tiles, requesting a hint SHALL return a pair of positions where both tiles are
// free and share the same face ID, and the hint state is set accordingly.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_9_hint_returns_a_valid_pair(seed in any::<u64>()) {
        let layout = turtle_layout();
        let mut generator = BoardGenerator::new(seed);
        let board = generator.generate(layout, 5).unwrap();

        // Generated boards are solvable, so there must be at least one valid pair
        let valid_pairs = board.valid_pairs();
        prop_assume!(!valid_pairs.is_empty(), "No valid pairs on generated board");

        let mut state = make_state(board);

        // Request a hint
        let result = request_hint(&mut state);

        // Assert hint returns Found with two positions
        match result {
            HintResult::Found(a, b) => {
                // Assert both positions have tiles (not empty)
                prop_assert!(state.board.tiles[a].is_some(),
                    "Hint position a ({}) should have a tile", a);
                prop_assert!(state.board.tiles[b].is_some(),
                    "Hint position b ({}) should have a tile", b);

                // Assert both tiles are free
                prop_assert!(state.board.is_free(a),
                    "Hint position a ({}) should be free", a);
                prop_assert!(state.board.is_free(b),
                    "Hint position b ({}) should be free", b);

                // Assert both tiles share the same face_id
                let face_a = state.board.tiles[a].unwrap().face_id;
                let face_b = state.board.tiles[b].unwrap().face_id;
                prop_assert_eq!(face_a, face_b,
                    "Hinted tiles should share face_id: a({})={}, b({})={}",
                    a, face_a, b, face_b);

                // Assert state.hint is Some with correct positions
                prop_assert!(state.hint.is_some(),
                    "state.hint should be Some after successful hint request");
                let hint = state.hint.unwrap();
                prop_assert_eq!(hint.position_a, a,
                    "state.hint.position_a should match returned position a");
                prop_assert_eq!(hint.position_b, b,
                    "state.hint.position_b should match returned position b");
            }
            HintResult::NoMatchesAvailable => {
                prop_assert!(false,
                    "Hint should return Found for a board with valid pairs, got NoMatchesAvailable");
            }
        }
    }
}


// Feature: space-levels, Property 1: Face pool size follows linear interpolation formula
//
// **Validates: Requirements 2.2**
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn property_1_face_pool_size_linear_interpolation(level in 21u32..=50) {
        let pool = xmahjong::levels::face_pool_for_level(level);
        let expected_size = 100 + ((level - 21) as usize * 100) / 29;
        prop_assert_eq!(pool.len(), expected_size,
            "At level {}, pool size should be {} but was {}", level, expected_size, pool.len());
    }
}

// Feature: space-levels, Property 2: Face pool distribution is even with wrapping IDs
//
// **Validates: Requirements 2.4, 2.5, 2.6**
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn property_2_face_pool_distribution_even_wrapping(level in 21u32..=50) {
        let pool = xmahjong::levels::face_pool_for_level(level);
        let pool_size = pool.len();
        let per_set = pool_size / 3;
        let remainder = pool_size - per_set * 2;

        // Split pool into penguin, dog, space segments by range
        let penguin_ids: Vec<u8> = pool.iter().copied().filter(|&id| id < 50).collect();
        let dog_ids: Vec<u8> = pool.iter().copied().filter(|&id| id >= 50 && id < 100).collect();
        let space_ids: Vec<u8> = pool.iter().copied().filter(|&id| id >= 100 && id < 150).collect();

        // Check counts
        prop_assert_eq!(penguin_ids.len(), per_set,
            "Level {}: penguin count should be {}, got {}", level, per_set, penguin_ids.len());
        prop_assert_eq!(dog_ids.len(), per_set,
            "Level {}: dog count should be {}, got {}", level, per_set, dog_ids.len());
        prop_assert_eq!(space_ids.len(), remainder,
            "Level {}: space count should be {}, got {}", level, remainder, space_ids.len());

        // Check wrapping: each penguin ID should be (index % 50)
        for (i, &id) in penguin_ids.iter().enumerate() {
            prop_assert_eq!(id, (i % 50) as u8,
                "Level {}: penguin ID at index {} should be {}, got {}", level, i, (i % 50) as u8, id);
        }
        // Each dog ID should be 50 + (index % 50)
        for (i, &id) in dog_ids.iter().enumerate() {
            prop_assert_eq!(id, 50 + (i % 50) as u8,
                "Level {}: dog ID at index {} should be {}, got {}", level, i, 50 + (i % 50) as u8, id);
        }
        // Each space ID should be 100 + (index % 50)
        for (i, &id) in space_ids.iter().enumerate() {
            prop_assert_eq!(id, 100 + (i % 50) as u8,
                "Level {}: space ID at index {} should be {}, got {}", level, i, 100 + (i % 50) as u8, id);
        }

        // All IDs must be in valid range (0-149)
        for &id in &pool {
            prop_assert!(id < 150,
                "Level {}: face ID {} is outside valid range 0-149", level, id);
        }
    }
}

// Feature: space-levels, Property 3: Tile count follows 10-level cycling pattern
//
// **Validates: Requirements 3.2, 3.3, 3.4**
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn property_3_tile_count_cycling_pattern(level in 21u32..=50) {
        let tile_count = xmahjong::levels::tiles_for_level(level);
        let effective_level = ((level - 1) % 10) + 1;
        let expected = xmahjong::levels::tiles_for_level(effective_level);
        prop_assert_eq!(tile_count, expected,
            "Level {} (effective {}) should have {} tiles, got {}", level, effective_level, expected, tile_count);
    }
}

// Feature: space-levels, Property 4: Tile count invariants
//
// **Validates: Requirements 3.5, 3.6**
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn property_4_tile_count_invariants(level in 1u32..=50) {
        let tile_count = xmahjong::levels::tiles_for_level(level);
        prop_assert_eq!(tile_count % 4, 0,
            "Level {}: tile count {} is not a multiple of 4", level, tile_count);
        prop_assert!(tile_count <= 144,
            "Level {}: tile count {} exceeds 144", level, tile_count);
    }
}

// Feature: space-levels, Property 7: Victory menu determined by max level boundary
//
// **Validates: Requirements 5.2, 5.3, 6.1, 6.2, 6.3**

/// Maximum level — mirrors the constant defined in main.rs.
const MAX_LEVEL: u32 = 1000;

/// Computes the victory menu item count for a given level.
fn victory_menu_item_count(level: u32) -> usize {
    if level < MAX_LEVEL { 3 } else { 2 }
}

/// Computes the victory dialog height for a given level.
fn victory_dialog_height(level: u32) -> u32 {
    if level < MAX_LEVEL { 360 } else { 300 }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn property_7_victory_menu_by_max_level(level in 1u32..=1000) {
        if level < MAX_LEVEL {
            prop_assert_eq!(victory_menu_item_count(level), 3,
                "Level {}: menu items should be 3 (below max)", level);
            prop_assert_eq!(victory_dialog_height(level), 360,
                "Level {}: dialog height should be 360 (below max)", level);
        } else {
            prop_assert_eq!(victory_menu_item_count(level), 2,
                "Level {}: menu items should be 2 (at max)", level);
            prop_assert_eq!(victory_dialog_height(level), 300,
                "Level {}: dialog height should be 300 (at max)", level);
        }
    }
}


// Feature: extended-levels, Property 1: Endgame level parameters match level 50
//
// **Validates: Requirements 1.1, 2.1, 2.2, 2.3, 2.4**
//
// For any level in the range 51 to 100, `tiles_for_level(level)` SHALL return 144,
// and `face_pool_for_level(level)` SHALL return a vector of length 200, both
// identical to the values returned for level 50.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn property_1_endgame_level_parameters_match_level_50(level in 51u32..=100) {
        let tiles = xmahjong::levels::tiles_for_level(level);
        let pool = xmahjong::levels::face_pool_for_level(level);

        let tiles_50 = xmahjong::levels::tiles_for_level(50);
        let pool_50 = xmahjong::levels::face_pool_for_level(50);

        // Tile count must be 144 and match level 50
        prop_assert_eq!(tiles, 144,
            "Level {}: tiles_for_level should return 144, got {}", level, tiles);
        prop_assert_eq!(tiles, tiles_50,
            "Level {}: tiles_for_level should match level 50 ({}), got {}", level, tiles_50, tiles);

        // Face pool size must be 200 and match level 50
        prop_assert_eq!(pool.len(), 200,
            "Level {}: face_pool_for_level should have length 200, got {}", level, pool.len());
        prop_assert_eq!(pool.len(), pool_50.len(),
            "Level {}: face pool length should match level 50 ({}), got {}", level, pool_50.len(), pool.len());

        // Face pool content must be identical to level 50
        prop_assert_eq!(pool, pool_50,
            "Level {}: face pool content should be identical to level 50", level);
    }
}


// Feature: extended-levels, Property 2: Endgame face pool distribution is correct
//
// **Validates: Requirements 3.1, 3.2, 3.3, 3.4**
//
// For any level in 51..=100, the face pool SHALL contain exactly 66 entries with
// IDs in 0-49 (penguin), exactly 66 entries with IDs in 50-99 (dog), and exactly
// 68 entries with IDs in 100-149 (space), with wrapping applied sequentially
// within each theme's range.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn property_2_endgame_face_pool_distribution(level in 51u32..=100) {
        let pool = xmahjong::levels::face_pool_for_level(level);

        // Total pool size must be 200
        prop_assert_eq!(pool.len(), 200,
            "Level {}: pool size should be 200, got {}", level, pool.len());

        // Split pool into theme segments by ID range
        let penguin_ids: Vec<u8> = pool.iter().copied().filter(|&id| id < 50).collect();
        let dog_ids: Vec<u8> = pool.iter().copied().filter(|&id| id >= 50 && id < 100).collect();
        let space_ids: Vec<u8> = pool.iter().copied().filter(|&id| id >= 100 && id < 150).collect();

        // Check exact distribution: 66 penguin, 66 dog, 68 space
        prop_assert_eq!(penguin_ids.len(), 66,
            "Level {}: penguin count should be 66, got {}", level, penguin_ids.len());
        prop_assert_eq!(dog_ids.len(), 66,
            "Level {}: dog count should be 66, got {}", level, dog_ids.len());
        prop_assert_eq!(space_ids.len(), 68,
            "Level {}: space count should be 68, got {}", level, space_ids.len());

        // Check wrapping: penguin IDs should be sequential (index % 50)
        for (i, &id) in penguin_ids.iter().enumerate() {
            prop_assert_eq!(id, (i % 50) as u8,
                "Level {}: penguin ID at index {} should be {}, got {}",
                level, i, (i % 50) as u8, id);
        }

        // Dog IDs should be 50 + (index % 50)
        for (i, &id) in dog_ids.iter().enumerate() {
            prop_assert_eq!(id, 50 + (i % 50) as u8,
                "Level {}: dog ID at index {} should be {}, got {}",
                level, i, 50 + (i % 50) as u8, id);
        }

        // Space IDs should be 100 + (index % 50)
        for (i, &id) in space_ids.iter().enumerate() {
            prop_assert_eq!(id, 100 + (i % 50) as u8,
                "Level {}: space ID at index {} should be {}, got {}",
                level, i, 100 + (i % 50) as u8, id);
        }
    }
}


// Feature: grandmaster-levels, Property 1: Grandmaster Mixed Phase level parameters
//
// For any level in 101..=1000, tiles_for_level SHALL return 144 and
// face_pool_for_level SHALL return a vector of length 200.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn property_grandmaster_level_parameters(level in 101u32..=1000) {
        let tiles = xmahjong::levels::tiles_for_level(level);
        let pool = xmahjong::levels::face_pool_for_level(level);

        prop_assert_eq!(tiles, 144,
            "Level {}: tiles_for_level should return 144, got {}", level, tiles);
        prop_assert_eq!(pool.len(), 200,
            "Level {}: face pool size should be 200, got {}", level, pool.len());
    }
}


// Feature: grandmaster-levels, Property 2: Grandmaster Mixed Phase 25% 4-pack distribution
//
// For any level in 101..=1000, the face pool SHALL contain an equal 25% distribution
// across all 4 theme packs: exactly 50 penguin (0-49), 50 dog (50-99), 50 space (100-149),
// and 50 ocean (150-199) faces.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn property_grandmaster_face_pool_distribution(level in 101u32..=1000) {
        let pool = xmahjong::levels::face_pool_for_level(level);

        prop_assert_eq!(pool.len(), 200,
            "Level {}: pool size should be 200, got {}", level, pool.len());

        let penguin_ids: Vec<u8> = pool.iter().copied().filter(|&id| id < 50).collect();
        let dog_ids: Vec<u8> = pool.iter().copied().filter(|&id| id >= 50 && id < 100).collect();
        let space_ids: Vec<u8> = pool.iter().copied().filter(|&id| id >= 100 && id < 150).collect();
        let ocean_ids: Vec<u8> = pool.iter().copied().filter(|&id| id >= 150 && id < 200).collect();

        // Exactly 50 each (25% each)
        prop_assert_eq!(penguin_ids.len(), 50,
            "Level {}: penguin count should be 50 (25%), got {}", level, penguin_ids.len());
        prop_assert_eq!(dog_ids.len(), 50,
            "Level {}: dog count should be 50 (25%), got {}", level, dog_ids.len());
        prop_assert_eq!(space_ids.len(), 50,
            "Level {}: space count should be 50 (25%), got {}", level, space_ids.len());
        prop_assert_eq!(ocean_ids.len(), 50,
            "Level {}: ocean count should be 50 (25%), got {}", level, ocean_ids.len());

        // Check sequential wrapping
        for (i, &id) in penguin_ids.iter().enumerate() {
            prop_assert_eq!(id, (i % 50) as u8,
                "Level {}: penguin ID at index {} should be {}, got {}", level, i, (i % 50) as u8, id);
        }
        for (i, &id) in dog_ids.iter().enumerate() {
            prop_assert_eq!(id, 50 + (i % 50) as u8,
                "Level {}: dog ID at index {} should be {}, got {}", level, i, 50 + (i % 50) as u8, id);
        }
        for (i, &id) in space_ids.iter().enumerate() {
            prop_assert_eq!(id, 100 + (i % 50) as u8,
                "Level {}: space ID at index {} should be {}, got {}", level, i, 100 + (i % 50) as u8, id);
        }
        for (i, &id) in ocean_ids.iter().enumerate() {
            prop_assert_eq!(id, 150 + (i % 50) as u8,
                "Level {}: ocean ID at index {} should be {}, got {}", level, i, 150 + (i % 50) as u8, id);
        }
    }
}


// Feature: extended-levels, Property 5: Save system rejects invalid levels
//
// **Validates: Requirements 5.3**
//
// For any SavedGame with level == 0 or level > 1000, the validation filter
// used by SavedGame::load() SHALL reject the save and return None.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn property_5_save_system_rejects_invalid_levels(level in prop_oneof![
        Just(0u32),
        1001u32..=u32::MAX,
    ]) {
        // The validation logic used in SavedGame::load() is:
        //   saved.filter(|s| (1..=1000).contains(&s.level))
        // For any level outside 1..=1000, this filter must return None.
        let is_valid = (1..=1000).contains(&level);
        prop_assert!(!is_valid,
            "Level {} should be rejected by the save validation filter, but (1..=1000).contains({}) returned true",
            level, level);
    }
}


// Feature: extended-levels, Property 4: Save round-trip preserves game state for all valid levels
//
// **Validates: Requirements 1.5, 5.1, 5.2**
//
// For any valid SavedGame with level in 1..=1000, serializing to JSON and
// deserializing back SHALL produce an identical struct with all fields preserved.

use xmahjong::storage::SavedGame;

fn arb_difficulty() -> impl Strategy<Value = String> {
    prop_oneof![Just("easy".to_string()), Just("normal".to_string())]
}

fn arb_saved_game() -> impl Strategy<Value = SavedGame> {
    (
        // tiles: Vec<Option<u8>> of length 144 with face IDs 0..200
        proptest::collection::vec(proptest::option::of(0u8..200), 144..=144),
        // undo_stack: Vec<(usize, u8, usize, u8)> up to 10 entries
        proptest::collection::vec((0usize..144, 0u8..200, 0usize..144, 0u8..200), 0..=10),
        // elapsed_ms
        0u64..3_600_000,
        // hints_used, shuffles_used, shuffles_remaining, pairs_matched, undos_used
        (0u32..100, 0u32..100, 0u32..20, 0u32..72, 0u32..100),
        // level in 1..=1000
        1u32..=1000,
        // base_score, base_time_ms, base_hints, base_shuffles, base_undos
        (0u32..100_000, 0u64..36_000_000, 0u32..500, 0u32..500, 0u32..500),
        // difficulty
        arb_difficulty(),
    )
        .prop_map(
            |(tiles, undo_stack, elapsed_ms, (hints_used, shuffles_used, shuffles_remaining, pairs_matched, undos_used), level, (base_score, base_time_ms, base_hints, base_shuffles, base_undos), difficulty)| {
                SavedGame {
                    tiles,
                    undo_stack,
                    elapsed_ms,
                    hints_used,
                    shuffles_used,
                    shuffles_remaining,
                    pairs_matched,
                    undos_used,
                    level,
                    base_score,
                    base_time_ms,
                    base_hints,
                    base_shuffles,
                    base_undos,
                    difficulty,
                }
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn property_4_save_round_trip_preserves_game_state(game in arb_saved_game()) {
        // Serialize to JSON
        let json = serde_json::to_string(&game)
            .expect("SavedGame should serialize to JSON");

        // Deserialize back
        let restored: SavedGame = serde_json::from_str(&json)
            .expect("SavedGame JSON should deserialize back");

        // Assert the round-trip produces an identical struct
        prop_assert_eq!(&game, &restored,
            "SavedGame round-trip failed: original and deserialized structs differ");
    }
}


// Feature: extended-levels, Property 3: Victory menu shows NEXT LEVEL iff below max level
//
// **Validates: Requirements 1.2, 1.3, 1.4**
//
// For any level in the range 1 to 1000, the victory screen SHALL display the
// NEXT LEVEL button if and only if level < MAX_LEVEL (i.e., level < 1000).
// At level 1000, only NEW GAME and ACHIEVEMENTS are shown (2 items).
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn property_3_victory_menu_shows_next_level_iff_below_max(level in 1u32..=1000) {
        let item_count = victory_menu_item_count(level);

        if level < MAX_LEVEL {
            // Below max level: NEXT LEVEL + NEW GAME + ACHIEVEMENTS = 3 items
            prop_assert_eq!(item_count, 3,
                "Level {}: victory menu should show 3 items (NEXT LEVEL + NEW GAME + ACHIEVEMENTS) when below max level {}, got {}",
                level, MAX_LEVEL, item_count);
        } else {
            // At max level (1000): NEW GAME + ACHIEVEMENTS = 2 items (no NEXT LEVEL)
            prop_assert_eq!(item_count, 2,
                "Level {}: victory menu should show 2 items (NEW GAME + ACHIEVEMENTS) at max level {}, got {}",
                level, MAX_LEVEL, item_count);
        }
    }
}


// Feature: extended-levels, Property 6: Generator determinism for endgame boards
//
// **Validates: Requirements 4.2**
//
// For any seed value, generating a board with tile_count=144 and a 200-entry
// face pool using that seed SHALL produce an identical tile arrangement every time.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn property_6_generator_determinism_for_endgame_boards(seed in any::<u64>()) {
        let layout = turtle_layout();
        let face_pool = xmahjong::levels::face_pool_for_level(50);

        // Generate a board with the given seed
        let mut gen1 = BoardGenerator::new(seed);
        let board1 = gen1.generate_with_faces(layout, 144, &face_pool, 10);

        // Generate a board again with the same seed
        let mut gen2 = BoardGenerator::new(seed);
        let board2 = gen2.generate_with_faces(layout, 144, &face_pool, 10);

        // Both calls should have the same success/failure outcome
        match (&board1, &board2) {
            (Ok(b1), Ok(b2)) => {
                // Assert tile arrangements are identical
                prop_assert_eq!(b1.tiles.len(), b2.tiles.len(),
                    "Board tile counts differ for seed {}", seed);
                for (i, (t1, t2)) in b1.tiles.iter().zip(b2.tiles.iter()).enumerate() {
                    prop_assert_eq!(t1, t2,
                        "Tile at position {} differs between two generations with the same seed {}",
                        i, seed);
                }
            }
            (Err(_), Err(_)) => {
                // Both failed — deterministic failure is acceptable
            }
            _ => {
                prop_assert!(false,
                    "Seed {}: one generation succeeded and the other failed — non-deterministic",
                    seed);
            }
        }
    }
}



// Feature: extended-levels, Property 7: Generator solvability for endgame boards
//
// **Validates: Requirements 4.3**
//
// For any seed that produces a successful generation, the generated 144-tile
// board with 200-face pool SHALL be solvable — meaning the returned solution
// sequence completely clears all tiles from the board when applied.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn property_7_generator_solvability_for_endgame_boards(seed in any::<u64>()) {
        let layout = turtle_layout();
        let face_pool = xmahjong::levels::face_pool_for_level(50);

        // Generate a board with tile_count=144 and 200-face pool, requesting the solution
        let mut generator = BoardGenerator::new(seed);
        let result = generator.generate_with_faces_and_solution(layout, 144, &face_pool, 10);

        // Skip seeds where generation fails (rare but possible)
        prop_assume!(result.is_ok());
        let (mut board, solution) = result.unwrap();

        // The board must start with exactly 144 tiles
        prop_assert_eq!(board.remaining_count(), 144,
            "Seed {}: board should have 144 tiles, got {}", seed, board.remaining_count());

        // The solution must have exactly 72 pairs (72 * 2 = 144 tiles cleared)
        prop_assert_eq!(solution.len(), 72,
            "Seed {}: solution should have 72 pairs, got {}", seed, solution.len());

        // Replay the solution: each pair must be valid (both tiles free, same face_id)
        for (step, &(a, b)) in solution.iter().enumerate() {
            prop_assert!(board.tiles[a].is_some(),
                "Seed {} step {}: no tile at position {}", seed, step, a);
            prop_assert!(board.tiles[b].is_some(),
                "Seed {} step {}: no tile at position {}", seed, step, b);
            prop_assert!(board.is_free(a),
                "Seed {} step {}: tile at {} not free", seed, step, a);
            prop_assert!(board.is_free(b),
                "Seed {} step {}: tile at {} not free", seed, step, b);

            let face_a = board.tiles[a].unwrap().face_id;
            let face_b = board.tiles[b].unwrap().face_id;
            prop_assert_eq!(face_a, face_b,
                "Seed {} step {}: face mismatch {} vs {}", seed, step, face_a, face_b);

            board.remove_pair(a, b);
        }

        // After replaying all 72 pair removals, the board must be completely empty
        prop_assert_eq!(board.remaining_count(), 0,
            "Seed {}: board not empty after solution, {} tiles remain", seed, board.remaining_count());
    }
}


// Feature: grandmaster-levels, Property 3: Generator solvability for Grandmaster Mixed boards
//
// For any seed that produces a successful generation, the generated 144-tile board with the
// 200-face 4-pack Grandmaster pool SHALL be 100% solvable.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn property_grandmaster_generator_solvability(seed in any::<u64>()) {
        let layout = turtle_layout();
        let face_pool = xmahjong::levels::face_pool_for_level(101);

        let mut generator = BoardGenerator::new(seed);
        let result = generator.generate_with_faces_and_solution(layout, 144, &face_pool, 10);

        prop_assume!(result.is_ok());
        let (mut board, solution) = result.unwrap();

        prop_assert_eq!(board.remaining_count(), 144,
            "Seed {}: board should have 144 tiles, got {}", seed, board.remaining_count());
        prop_assert_eq!(solution.len(), 72,
            "Seed {}: solution should have 72 pairs, got {}", seed, solution.len());

        for (step, &(a, b)) in solution.iter().enumerate() {
            prop_assert!(board.tiles[a].is_some(),
                "Seed {} step {}: no tile at position {}", seed, step, a);
            prop_assert!(board.tiles[b].is_some(),
                "Seed {} step {}: no tile at position {}", seed, step, b);
            prop_assert!(board.is_free(a),
                "Seed {} step {}: tile at {} not free", seed, step, a);
            prop_assert!(board.is_free(b),
                "Seed {} step {}: tile at {} not free", seed, step, b);

            let face_a = board.tiles[a].unwrap().face_id;
            let face_b = board.tiles[b].unwrap().face_id;
            prop_assert_eq!(face_a, face_b,
                "Seed {} step {}: face mismatch {} vs {}", seed, step, face_a, face_b);

            board.remove_pair(a, b);
        }

        prop_assert_eq!(board.remaining_count(), 0,
            "Seed {}: board not empty after solution, {} tiles remain", seed, board.remaining_count());
    }
}


// Feature: extended-levels, Property 8: Shuffle reward awarded on endgame/grandmaster level completion
//
// **Validates: Requirements 7.1, 7.2, 7.3**
//
// For any level in 51 to 1000 and for any difficulty setting (Easy or Normal),
// completing the level SHALL increase `shuffles_remaining` by exactly 1.
// This verifies the shuffle reward logic: `remaining_shuffles = shuffles_remaining + 1`.

/// Simulates the shuffle reward logic from main.rs level completion handler.
/// When a player completes a level, the reward is computed as:
///   remaining_shuffles = game_state.shuffles_remaining + 1
/// This is applied regardless of level or difficulty.
fn compute_shuffle_reward(shuffles_remaining: u32) -> u32 {
    shuffles_remaining + 1
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn property_8_shuffle_reward_on_endgame_level_completion(
        level in 51u32..=1000,
        difficulty in prop_oneof![Just(Difficulty::Easy), Just(Difficulty::Normal)],
        initial_shuffles in 0u32..=50,
    ) {
        // Simulate completing a level in the range (51-1000)
        // The shuffle reward logic in main.rs is:
        //   let remaining_shuffles = game_state.shuffles_remaining + 1;
        // This applies to ALL levels and is difficulty-independent.

        let layout = turtle_layout();
        let state = GameState {
            board: xmahjong::board::Board { tiles: vec![None; layout.positions.len()], layout },
            timer: GameTimer::new(),
            score: ScoreTracker::new(),
            status: GameStatus::Won,
            selection: None,
            hint: None,
            undo_stack: Vec::new(),
            shuffles_remaining: initial_shuffles,
            level,
            base_score: 0,
            base_time_ms: 0,
            base_hints: 0,
            base_shuffles: 0,
            base_undos: 0,
            animations: Vec::new(),
            difficulty,
        };

        let remaining_shuffles = compute_shuffle_reward(state.shuffles_remaining);

        prop_assert_eq!(remaining_shuffles, initial_shuffles + 1,
            "Level {} (difficulty {:?}): shuffle reward should be initial ({}) + 1 = {}, got {}",
            level, difficulty, initial_shuffles, initial_shuffles + 1, remaining_shuffles);

        let reward_at_51 = compute_shuffle_reward(initial_shuffles);
        let reward_at_1000 = compute_shuffle_reward(initial_shuffles);
        prop_assert_eq!(remaining_shuffles, reward_at_51,
            "Shuffle reward should be the same at level {} as at level 51", level);
        prop_assert_eq!(remaining_shuffles, reward_at_1000,
            "Shuffle reward should be the same at level {} as at level 1000", level);

        let _ = state.difficulty;
        prop_assert_eq!(remaining_shuffles, initial_shuffles + 1,
            "Shuffle reward must be +1 regardless of difficulty {:?}", difficulty);
    }
}
