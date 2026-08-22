# Level Design

xMahjong features **1000 levels** split into five distinct progression phases: **Penguin Phase** (1–10), **Dog Phase** (11–20), **Space Phase** (21–50), **Endgame Phase** (51–100), and the ultimate **Grandmaster Mixed Phase** (101–1000).

---

## Theme Packs & Asset Overview

The game includes **200 unique tile face graphics** divided across 4 distinct theme packs:

| Theme | Asset Directory | Face ID Range | Unique Faces | Description |
|-------|-----------------|---------------|--------------|-------------|
| **Penguins** | `assets/tiles/` | `0 – 49` | 50 | Classic Tux Penguin faces and characters |
| **Dogs** | `assets/dogs/` | `50 – 99` | 50 | 5 distinct puppy and dog breed styles |
| **Space** | `assets/space/` | `100 – 149` | 50 | Cosmic sci-fi, aliens, rockets, and galaxies |
| **Ocean** | `assets/ocean/` | `150 – 199` | 50 | Vibrant marine life, sea creatures, corals, and ocean wonders |

---

## 1. Penguin Phase (Levels 1–10)

Levels 1–10 introduce the core game mechanics using only Penguin tile faces from `assets/tiles/` (IDs 0–49). The board starts small and expands by 12 tiles (3 pairs / 6 faces) each level.

| Level | Tiles | Pairs | Faces Used | Face Pool Size | Theme |
|-------|-------|-------|------------|----------------|-------|
| 1 | 36 | 18 | 9 | 50 | Penguins only (`assets/tiles/`) |
| 2 | 48 | 24 | 12 | 50 | Penguins only (`assets/tiles/`) |
| 3 | 60 | 30 | 15 | 50 | Penguins only (`assets/tiles/`) |
| 4 | 72 | 36 | 18 | 50 | Penguins only (`assets/tiles/`) |
| 5 | 84 | 42 | 21 | 50 | Penguins only (`assets/tiles/`) |
| 6 | 96 | 48 | 24 | 50 | Penguins only (`assets/tiles/`) |
| 7 | 108 | 54 | 27 | 50 | Penguins only (`assets/tiles/`) |
| 8 | 120 | 60 | 30 | 50 | Penguins only (`assets/tiles/`) |
| 9 | 132 | 66 | 33 | 50 | Penguins only (`assets/tiles/`) |
| 10 | 144 | 72 | 36 | 50 | Penguins only (Full Board) |

---

## 2. Dog Phase (Levels 11–20)

Levels 11–20 repeat the 36-to-144 tile count ramp, mixing in Dog tile faces from `assets/dogs/` (IDs 50–99). Each level introduces one new dog style (10 faces per style) until all 5 styles are active.

| Level | Tiles | Pairs | Dog Styles Active | Face Pool Size | Theme |
|-------|-------|-------|-------------------|----------------|-------|
| 11 | 36 | 18 | 1 (IDs 50–59) | 60 | Penguins + Dogs |
| 12 | 48 | 24 | 2 (IDs 50–69) | 70 | Penguins + Dogs |
| 13 | 60 | 30 | 3 (IDs 50–79) | 80 | Penguins + Dogs |
| 14 | 72 | 36 | 4 (IDs 50–89) | 90 | Penguins + Dogs |
| 15 | 84 | 42 | 5 (IDs 50–99) | 100 | Penguins + Dogs |
| 16 | 96 | 48 | 5 (IDs 50–99) | 100 | Penguins + Dogs |
| 17 | 108 | 54 | 5 (IDs 50–99) | 100 | Penguins + Dogs |
| 18 | 120 | 60 | 5 (IDs 50–99) | 100 | Penguins + Dogs |
| 19 | 132 | 66 | 5 (IDs 50–99) | 100 | Penguins + Dogs |
| 20 | 144 | 72 | 5 (IDs 50–99) | 100 | Penguins + Dogs (Full Board) |

---

## 3. Space Phase (Levels 21–50)

Levels 21–50 combine tiles from Penguins (IDs 0–49), Dogs (IDs 50–99), and Space (IDs 100–149, `assets/space/`). The face pool grows linearly from 100 to 200 using the formula `100 + ((level - 21) * 100) / 29`, distributed evenly across the three packs.

| Level | Tiles | Pairs | Face Pool Size | Penguin | Dog | Space | Theme |
|-------|-------|-------|----------------|---------|-----|-------|-------|
| 21 | 36 | 18 | 100 | 33 | 33 | 34 | Penguins + Dogs + Space |
| 22 | 48 | 24 | 103 | 34 | 34 | 35 | Penguins + Dogs + Space |
| 23 | 60 | 30 | 106 | 35 | 35 | 36 | Penguins + Dogs + Space |
| 24 | 72 | 36 | 110 | 36 | 36 | 38 | Penguins + Dogs + Space |
| 25 | 84 | 42 | 113 | 37 | 37 | 39 | Penguins + Dogs + Space |
| 26 | 96 | 48 | 117 | 39 | 39 | 39 | Penguins + Dogs + Space |
| 27 | 108 | 54 | 120 | 40 | 40 | 40 | Penguins + Dogs + Space |
| 28 | 120 | 60 | 124 | 41 | 41 | 42 | Penguins + Dogs + Space |
| 29 | 132 | 66 | 127 | 42 | 42 | 43 | Penguins + Dogs + Space |
| 30 | 144 | 72 | 131 | 43 | 43 | 45 | Penguins + Dogs + Space (Full Board) |
| 31–40 | 36–144 | 18–72 | 134–165 | 44–55 | 44–55 | 46–55 | Penguins + Dogs + Space |
| 41–50 | 36–144 | 18–72 | 168–200 | 56–66 | 56–66 | 56–68 | Penguins + Dogs + Space (Full Board at 50) |

---

## 4. Endgame Phase (Levels 51–100)

Levels 51–100 maintain fixed maximum parameters: 144 tiles, 72 pairs, and a 200-entry face pool drawn from Penguins (66 entries), Dogs (66 entries), and Space (68 entries). Each board uses a clock-derived seed for limitless procedural variation.

| Parameter | Value |
|-----------|-------|
| Level Range | 51–100 |
| Tiles | 144 (Full Turtle Board) |
| Pairs | 72 |
| Faces Used per Board | 36 |
| Face Pool Size | 200 |
| Themes | Penguins (`assets/tiles/`) + Dogs (`assets/dogs/`) + Space (`assets/space/`) |

---

## 5. Grandmaster Mixed Phase (Levels 101–1000)

Levels 101 through 1000 represent the **Grandmaster Mixed Phase**. This mode harnesses all **4 complete theme packs**—**Penguins**, **Dogs**, **Space**, and the new **Ocean** pack (`assets/ocean/`)—into a full, perfectly balanced **25% distribution** across the entire 200-entry pool.

### Face Pool Composition (Levels 101–1000)

```
Total Pool: 200 Face Entries
┌───────────────────────────┬───────────────────────────┐
│ Penguin Pack (IDs 0–49)   │ Dog Pack (IDs 50–99)      │
│ 50 Entries (25.0%)        │ 50 Entries (25.0%)        │
│ assets/tiles/             │ assets/dogs/              │
├───────────────────────────┼───────────────────────────┤
│ Space Pack (IDs 100–149)  │ Ocean Pack (IDs 150–199)  │
│ 50 Entries (25.0%)        │ 50 Entries (25.0%)        │
│ assets/space/             │ assets/ocean/             │
└───────────────────────────┴───────────────────────────┘
```

### Grandmaster Phase Milestones & Ranges (Levels 101–1000)

| Level Milestone Range | Tiles | Pairs | Active Asset Packs | Difficulty & Experience |
|-----------------------|-------|-------|--------------------|-------------------------|
| **101 – 200** (Grandmaster Novice) | 144 | 72 | Penguins, Dogs, Space, Ocean | Introduction to full 4-pack visual diversity; 36 faces chosen from 200 pool including `assets/ocean/` |
| **201 – 400** (Grandmaster Adept) | 144 | 72 | Penguins, Dogs, Space, Ocean | Complex multi-layer tile discrimination across marine, celestial, canine, and penguin art |
| **401 – 600** (Grandmaster Expert) | 144 | 72 | Penguins, Dogs, Space, Ocean | High-speed pattern recognition under clock-derived procedural layouts |
| **601 – 800** (Grandmaster Master) | 144 | 72 | Penguins, Dogs, Space, Ocean | Strategic depth and long-range planning on 5-layer turtle layouts |
| **801 – 1000** (Supreme Grandmaster) | 144 | 72 | Penguins, Dogs, Space, Ocean | The ultimate Mahjong Solitaire endurance test culminating at Level 1000 |

### Parameters for Levels 101–1000

| Parameter | Value |
|-----------|-------|
| Level Range | 101 – 1000 (900 levels) |
| Board Layout | Classic Turtle (144 tiles, 5 layers) |
| Pairs per Board | 72 |
| Distinct Faces per Board | 36 unique faces (4 tiles per face) |
| Face Pool Size | 200 |
| **Penguin Pack entries** | 50 (IDs `0–49`, exactly 25.0%) |
| **Dog Pack entries** | 50 (IDs `50–99`, exactly 25.0%) |
| **Space Pack entries** | 50 (IDs `100–149`, exactly 25.0%) |
| **Ocean Pack entries** | 50 (IDs `150–199`, exactly 25.0%) |
| Tile Textures Used | `assets/tiles/`, `assets/dogs/`, `assets/space/`, `assets/ocean/` |

---

## How Generation Works

- **Tile Count Rule**: Tile count is always a multiple of 4 (each chosen face ID appears exactly 4 times on the board, forming 2 matchable pairs).
- **Face Selection**: For each board, the reverse-deal algorithm randomly draws 36 distinct face IDs from the level's face pool and places them in reverse-deal order to ensure 100% solvability.
- **Compact Layouts (Levels 1–9, 11–19, 21–29, 31–39, 41–49)**: For levels with fewer than 144 tiles, outer positions are removed from the outside in before placing tiles, keeping the board compact and playable.

---

## Progression & Level Select

- **Next Level**: Clearing a board presents a "NEXT LEVEL" button on the victory screen for levels 1 through 999.
- **Victory at Level 1000**: Level 1000 is the final maximum level, displaying completion awards.
- **Level Select Screen**: Accessible from the Pause Menu (<kbd>ESC</kbd> → **LEVELS**). Allows browsing and instant replay of any completed or unlocked level across all 1000 levels.
  - Quick jump tabs: **PENGUIN (1–10)**, **DOG (11–20)**, **SPACE (21–50)**, **ENDGAME (51–100)**, **GRANDMASTER (101+)**.
  - Paginated 5×5 grid with 25 levels per page (40 pages total).
  - Persistence: Progress is saved to `progress.json` per user profile.

---

## Difficulty Modes

| Difficulty | Shuffle Behavior | Solvability |
|------------|------------------|-------------|
| **Easy** (default) | Guaranteed valid arrangement. Smart placement algorithm ensures at least 5 playable pairs after every shuffle. | Always playable after shuffle |
| **Normal** | Pure random redistribution of remaining face IDs across occupied positions without retry guarantees. | May require additional shuffles if blocked |

- Difficulty can be toggled mid-game from the Pause Menu (<kbd>ESC</kbd> → **MODE: EASY/NORMAL**).
- Setting is saved to `savegame.json` and recorded in leaderboard entries.
