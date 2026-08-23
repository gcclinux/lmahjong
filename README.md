# xMahjong

A Tux-themed Mahjong solitaire game for Linux, windows & MacOS built with Rust and SDL2.

## About

xMahjong is a classic tile-matching solitaire game featuring Tux penguin-themed graphics. Clear all 144 tiles from the board by matching pairs of free tiles. The game uses the traditional Turtle layout with 5 stacked layers, and every generated board is guaranteed to be solvable.

![v0.1.5](sprite/latest.png)

Example

![Initial board](sprite/board-example.png)

### Features

- 1000 Levels across 5 phases (Penguin, Dog, Space, Endgame, and Grandmaster Mixed)
- Level Select screen to browse, choose, and replay any completed or unlocked level
- 4 rich tile theme packs: Tux Penguins, Puppies, Space, and Ocean (200 unique tile face graphics)
- Classic Turtle layout with 144 tiles across 5 layers
- Guaranteed solvable boards via reverse-deal generation
- Hint system, undo (up to 10 moves), and progressive shuffle system
- Timer and scoring system with local leaderboard (top 10)
- Keyboard shortcuts for all actions
- Audio feedback with mute support
- Resizable window (min 1920×1080, adapts to screen resolution)
- Native Linux packages (.deb, .rpm, AppImage)
- Daily new free Shuffle +1

![Game Over - Shuffle](assets/shuffles.png)

## Prerequisites

### System Dependencies

You need SDL2 development libraries installed:

**Ubuntu / Debian:**

```bash
sudo apt install libsdl2-dev libsdl2-image-dev libsdl2-mixer-dev libsdl2-ttf-dev pkg-config
```

**Fedora:**

```bash
sudo dnf install SDL2-devel SDL2_image-devel SDL2_mixer-devel SDL2_ttf-devel pkg-config
```

**Arch Linux:**

```bash
sudo pacman -S sdl2 sdl2_image sdl2_mixer sdl2_ttf pkg-config
```

### Rust Toolchain

Install Rust via [rustup](https://rustup.rs/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Building

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release
```

**macOS (requires Homebrew SDL2 libraries):**
```bash
brew install sdl2 sdl2_image sdl2_mixer sdl2_ttf pkg-config
LIBRARY_PATH="/opt/homebrew/lib" cargo build --release
```

The binary is output to `target/debug/xmahjong` or `target/release/xmahjong`.

## Running

```bash
# Run directly
cargo run

# Or run the release binary
cargo run --release
```

## Running Tests

```bash
# Run all tests (unit + property-based)
cargo test

# Run only unit tests
cargo test --lib

# Run a specific property test file
cargo test --test board_properties

# Run tests with output shown
cargo test -- --nocapture
```

The project includes 19 property-based tests using `proptest` that validate correctness properties like board generation invariants, solvability, matching logic, undo/redo behavior, shuffle guarantees, and layout scaling.

## Controls

| Shortcut | Action |
|----------|--------|
| Left click | Select tile |
| Ctrl+S | Save game |
| Ctrl+Q | Save + Quit |
| Ctrl+N | New Game |
| Ctrl+R | Resume |
| Ctrl+P | Pause |
| Ctrl+M | Toggle Mute |
| Shift+S | Shuffle |
| Shift+U | Undo |
| Shift+H | Hint |
| Escape | Pause / Resume (toggle) |

## Scoring

Score starts at 0 and increases with each pair matched:

- **Base:** +10 points per pair removed
- **Streak bonus:** +2 per pair (rewards continuous play)
- **Penalties:** −5 per hint used, −10 per shuffle used
- **Time bonus** (at game end): max(0, 500 − elapsed_seconds)

Top 10 scores are saved to a local leaderboard.

## Shuffle System

Shuffles rearrange the remaining tiles on the board when no moves are available (or strategically when desired). The shuffle economy is progressive:

- **Starting shuffles:** Each new game begins with 1 shuffle
- **Level completion bonus:** +1 shuffle awarded for each level completed (carried across levels)
- **Daily bonus:** +1 shuffle granted once per day when the game is launched
- **Total potential:** Accumulate shuffles across levels (1 starting + 1 per completed level), plus daily bonuses earned
- **Wait for Shuffle:** If you run out of shuffles and moves mid-run, you can choose "Wait for Shuffle" instead of starting over. This saves your game and exits. The next day when you launch the game, the daily bonus gives you a new shuffle so you can continue where you left off.

Shuffle state persists across sessions via `shuffles.json` in the data storage directory.

## Difficulty

The game offers two difficulty levels that affect how shuffles behave:

- **Easy** (default): Shuffles are guaranteed to produce a playable board state. The game uses smart placement to ensure free tiles always have valid matching pairs after a shuffle. You will never get stuck due to a bad shuffle.
- **Normal**: Shuffles randomly redistribute face IDs across the remaining tiles without any solvability guarantee. You may end up with no valid moves after a shuffle and need to use another shuffle to continue.

### Switching Difficulty

Difficulty can be changed at any time from the pause menu (ESC → navigate to "DIFFICULTY" → press Enter or click). Mid-game you can pause, switch from EASY to NORMAL (or vice versa), resume, and keep playing. The shuffle behavior changes immediately — if you switch to NORMAL and use a shuffle on the current level, it will already be the random (non-guaranteed) shuffle. When you advance to the next level, the game uses whichever difficulty you last set.

Difficulty is saved with the game state and displayed on the leaderboard.

## Level Select & Replaying Levels

You can replay any level you have already completed (or currently unlocked):
1. Press **Escape** or click the **MENU** button in-game to open the Pause Menu.
2. Select **LEVELS** (using arrows + Enter, or mouse click).
3. Browse the 1000-level catalog with phase jump tabs (**PENGUIN**, **DOG**, **SPACE**, **ENDGAME**, **GRANDMASTER**) or page through using `< PREV` / `NEXT >` (or `PgUp` / `PgDn`).
4. Select any completed or unlocked level to start playing immediately. Completed levels are saved to `progress.json` per user profile.

## Trophies & Stats

View your career achievements, consistency streaks, and clean-clearance records:
- **Career Overview:** Tracks highest completed level out of 1000 and total career points accumulated across runs.
- **Daily Consistency Streaks:** Displays your current active day streak and all-time best streak record with daily launch gift bonuses.
- **Mastery & Clean Clearances:** Counts the total number of levels cleared with **No Hints** (💡), **No Undos** (↩️), and **No Lives/Shuffles Used** (❤️).
- **Save Stats Image:** Click the **SAVE STATS IMAGE** button to export a high-resolution screenshot of your trophy dashboard to disk (`trophies_stats_<timestamp>.bmp`) and immediately open it for convenient sharing online.
- **Close:** Click **CLOSE** or press **Escape** / **Enter** to exit and return directly to the menu screen.
- Access via the **ACHIEVEMENTS** option in the Pause Menu or from the Game Won / Game Over summary screens.


## Packaging (.deb, .rpm, AppImage)

A packaging script is included to create distribution packages:

```bash
# Build all three formats
./package.sh all

# Or build individually
./package.sh deb
./package.sh rpm
./package.sh appimage
```

Output goes to `target/package/`.

### Prerequisites for packaging

| Format | Tool needed | Install with |
|--------|-------------|--------------|
| .deb | dpkg-deb | Pre-installed on Debian/Ubuntu |
| .rpm | rpmbuild | `sudo dnf install rpm-build` |
| AppImage | wget | Pre-installed on most distros (downloads appimagetool automatically) |

### Installing the packages

**.deb (Ubuntu/Debian):**
```bash
sudo apt install ./target/package/xmahjong_0.1.0_amd64.deb
```
This automatically installs SDL2 runtime dependencies via apt.

**.rpm (Fedora/RHEL):**
```bash
sudo dnf install ./target/package/xmahjong-0.1.0-1.x86_64.rpm
```
This automatically installs SDL2 runtime dependencies via dnf.

**AppImage (any distro):**
```bash
chmod +x target/package/xmahjong-0.1.0-x86_64.AppImage
./target/package/xmahjong-0.1.0-x86_64.AppImage
```
AppImages bundle SDL2 libraries inside, so no system dependencies are needed.

**macOS (.app bundle):**
```bash
cp -R xMahjong-0.1.0-aarch64.app /Applications/
open /Applications/xMahjong-0.1.0-aarch64.app
```
Copy the `.app` to your Applications folder and launch it. The app bundle includes all SDL2 libraries — no Homebrew or other dependencies required.

**macOS (.dmg disk image):**
```bash
open xMahjong-0.1.0-arm64.dmg
```
Double-click the `.dmg` to mount it, then drag `xMahjong.app` into the Applications folder. No additional dependencies are needed.

**macOS Gatekeeper notice:**
Since the app is ad-hoc signed (not notarized with Apple), macOS may block it with a "cannot verify" warning. To allow it to run, remove the quarantine attribute:
```bash
xattr -cr /Applications/xMahjong.app
```

## Assets

![Tux Penguin Sprite Sheet](sprite/tux-sprite-xmahjong.png)
![Puppy Sprite Sheet](sprite/puppy-sprite-xmahjong.png)
![Space Sprite Sheet](sprite/space-sprite-xmahjong.png)

## Data Storage

`xmahjong` supports multi-user profiles. When starting the game, players are prompted to enter their username. All settings, levels, and progress details are stored within the user-specific directory:

| Platform | Path |
|----------|------|
| **Linux** | `~/.local/share/xmahjong/<user_name>/` |
| **macOS** | `~/Library/Application Support/xmahjong/<user_name>/` |
| **Windows** | `%APPDATA%\xmahjong\<user_name>\` (e.g. `C:\Users\<user>\AppData\Roaming\xmahjong\<user_name>\`) |

Entering a different username at game startup creates and manages a separate set of user stats and saves, enabling multiple players to share the game. You can also switch users at any time via the **SWITCH USER** option in the Pause menu (ESC). Level completions and scores are automatically saved to `<user_name>`'s profile without prompting for username input after every level.

### Files (per username folder)

| File | Description |
|------|-------------|
| `leaderboard.json` | Top 10 scores with player name, score, time, hints used, shuffles used, undos used, and date |
| `settings.json` | Persistent settings (muted state) |
| `savegame.json` | In-progress game state for resuming later (deleted after loading) |
| `progress.json` | Persistent level completion and unlocked level tracking |
| `shuffles.json` | Shuffle state: tracks last daily bonus date for the +1 daily shuffle reward |
| `trophies.json` | Persistent trophy achievement progress |

### Menu

- Pause menu (render_menu) — ✅ Has keyboard nav & Switch User option
- Victory (render_victory) — ✅ Has keyboard nav
- Leaderboard (render_leaderboard) — ✅ Has Enter/Escape support
- Shortcuts (render_shortcuts) — ✅ Has Enter/Escape support
- No Moves (render_no_moves) — ✅ Has Shuffle + New Game buttons
- Game Over (render_game_over) — ✅ Has Save Score + New Game buttons
- Quit Confirmation (render_quit_confirmation) — ✅ Has Yes/No buttons
- Name Entry (render_name_entry) — ✅ Startup username prompt & text input

### Graphify

- Manually Update .gitignore
```
add graphify-out/cache
```

#### Windows
- PS> winget install astral-sh.uv
- PS> uv tool install graphifyy
- PS> graphify install --platform antigravity / kiro
- PS> graphify hook install

#### Linux
- $ sudo snap install astral-uv --classic
- $ uv tool install graphifyy
- $ graphify install --platform antigravity / kiro
- $ graphify hook install


## License

This project is licensed under the [GNU General Public License v3.0](LICENSE).
