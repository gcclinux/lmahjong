//! Level system functions for determining tile count and face pool per level.
//!
//! Extracted from main.rs for testability.

/// Returns the number of tiles to place for a given level (1-1000).
/// Levels 1-50: existing 10-level cycle (36, 48, 60, 72, 84, 96, 108, 120, 132, 144).
/// Levels 51-1000: always 144.
/// All values are multiples of 4 (required for face pairing).
pub fn tiles_for_level(level: u32) -> usize {
    if level >= 51 {
        return 144;
    }
    let effective = ((level - 1) % 10) + 1;
    match effective {
        1 => 36,
        2 => 48,
        3 => 60,
        4 => 72,
        5 => 84,
        6 => 96,
        7 => 108,
        8 => 120,
        9 => 132,
        _ => 144,
    }
}

/// Returns the face pool for a given level (1-1000).
/// - Levels 1-10: only penguin faces (0-49)
/// - Levels 11-20: penguin faces (0-49) mixed with an increasing number of dog faces (50-99)
///   Level 11: 1 dog style added, Level 12: 2 dog styles, ... Level 15+: 5 dog styles
/// - Levels 21-50: penguin (0-49) + dog (50-99) + space (100-149) faces
///   Pool grows linearly from 100 to 200
///   Distribution: floor(pool_size/3) penguin, floor(pool_size/3) dog, remainder space
/// - Levels 51-100: penguin (0-49) + dog (50-99) + space (100-149) faces
///   Pool fixed at 200 (same as level 50: 66 penguin, 66 dog, 68 space)
/// - Levels 101-1000: Grandmaster Mixed Phase
///   Uses all 4 image packs: penguin (0-49), dog (50-99), space (100-149), ocean (150-199)
///   Equal 25% distribution: 50 penguin, 50 dog, 50 space, 50 ocean (pool size: 200)
pub fn face_pool_for_level(level: u32) -> Vec<u8> {
    if level <= 10 {
        // Pure penguin faces
        (0u8..50).collect()
    } else if level <= 20 {
        // Mix penguin + dog faces
        // Add 1 dog style per level from 11 to 15, then stay at 5 dog styles for 16-20
        let dog_styles = ((level - 10) as usize).min(5);
        let mut pool: Vec<u8> = (0u8..50).collect(); // all penguins
        // Add dog faces: each "style" adds 10 dog face IDs
        // Style 1: faces 50-59, Style 2: 60-69, etc.
        for s in 0..dog_styles {
            let start = 50 + (s as u8 * 10);
            let end = start + 10;
            for face_id in start..end {
                pool.push(face_id);
            }
        }
        pool
    } else if level <= 100 {
        // Space Phase (21-50) & Endgame Phase (51-100)
        // Pool size: linear interpolation from 100 (level 21) to 200 (level 50)
        // For levels 51-100, clamp to level 50's formula output (pool_size = 200)
        let effective_level = level.min(50);
        let pool_size = 100 + ((effective_level - 21) as usize * 100) / 29;
        let per_set = pool_size / 3;
        let remainder = pool_size - per_set * 2; // space gets remainder

        let mut pool = Vec::with_capacity(pool_size);
        // Penguin faces: per_set IDs wrapping within 0-49
        for i in 0..per_set {
            pool.push((i % 50) as u8);
        }
        // Dog faces: per_set IDs wrapping within 50-99
        for i in 0..per_set {
            pool.push(50 + (i % 50) as u8);
        }
        // Space faces: remainder IDs wrapping within 100-149
        for i in 0..remainder {
            pool.push(100 + (i % 50) as u8);
        }
        pool
    } else {
        // Grandmaster Mixed Phase: levels 101-1000
        // Equal 25% distribution across all 4 packs:
        // Penguin (0-49), Dog (50-99), Space (100-149), Ocean (150-199)
        let pool_size = 200;
        let per_set = pool_size / 4; // 50
        let remainder = pool_size - per_set * 4; // 0

        let mut pool = Vec::with_capacity(pool_size);
        // Penguin faces: 50 IDs (0-49)
        for i in 0..per_set {
            pool.push((i % 50) as u8);
        }
        // Dog faces: 50 IDs (50-99)
        for i in 0..per_set {
            pool.push(50 + (i % 50) as u8);
        }
        // Space faces: 50 IDs (100-149)
        for i in 0..per_set {
            pool.push(100 + (i % 50) as u8);
        }
        // Ocean faces: 50 IDs (150-199)
        for i in 0..(per_set + remainder) {
            pool.push(150 + (i % 50) as u8);
        }
        pool
    }
}
