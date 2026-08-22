//! xMahjong - A Tux-themed Mahjong solitaire game for Linux.
//!
//! Main entry point: initializes SDL2, creates all game components,
//! and runs the game loop at ~60 FPS.

use std::time::{Duration, Instant};

use sdl2::rect::Rect;

use xmahjong::audio::AudioManager;
use xmahjong::board::turtle_layout;
use xmahjong::game_state::{GameState, GameStatus, NameEntryState, ScoreTracker, Difficulty};
use xmahjong::generator::BoardGenerator;
use xmahjong::input::{GameAction, InputHandler};
use xmahjong::logic::{self, GameOverReason, HintResult, SelectionResult};
use xmahjong::renderer::{self, Renderer};
use xmahjong::storage::{Leaderboard, LeaderboardEntry, SavedGame, Settings, ShuffleState, TrophyState, UserProgress};
use xmahjong::timer::GameTimer;

/// Target frame duration for ~60 FPS (16.67ms per frame).
const FRAME_DURATION_MS: u64 = 16;

/// Duration in seconds before auto-dismissing hints.
const HINT_DISMISS_SECS: u64 = 3;

/// Current application version (read from the `release` file at compile time).
const CURRENT_VERSION: &str = env!("XMAHJONG_VERSION");

/// URL to fetch the latest version number.
const VERSION_CHECK_URL: &str =
    "https://raw.githubusercontent.com/gcclinux/xmahjong/refs/heads/main/release";

/// URL to open for downloading the latest release.
const RELEASE_DOWNLOAD_URL: &str = "https://github.com/gcclinux/xmahjong/releases/latest";

/// URL for the About page.
const ABOUT_URL: &str = "https://easysmartapps.co.uk/xmahjong";

/// Maximum level number. Level progression stops at this level.
const MAX_LEVEL: u32 = 1000;

/// State for the update-available dialog shown at startup.
struct UpdateInfo {
    latest_version: String,
}

/// Checks for a newer version by fetching the remote release file.
/// Returns Some(latest_version) if an update is available, None otherwise.
fn check_for_update() -> Option<String> {
    let response = ureq::get(VERSION_CHECK_URL)
        .timeout(std::time::Duration::from_secs(5))
        .call()
        .ok()?;

    let body = response.into_string().ok()?;
    let latest = body.trim().to_string();

    if latest.is_empty() {
        return None;
    }

    // Compare versions: only show update if remote is strictly newer
    if version_is_newer(&latest, CURRENT_VERSION) {
        Some(latest)
    } else {
        None
    }
}

/// Returns true if `latest` is a newer version than `current`.
/// Compares numeric semver components left-to-right.
fn version_is_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> {
        v.split('.')
            .filter_map(|s| s.parse::<u32>().ok())
            .collect()
    };
    let l = parse(latest);
    let c = parse(current);

    for i in 0..l.len().max(c.len()) {
        let lv = l.get(i).copied().unwrap_or(0);
        let cv = c.get(i).copied().unwrap_or(0);
        if lv > cv {
            return true;
        }
        if lv < cv {
            return false;
        }
    }
    false
}

/// Opens a URL in the user's default browser.
fn open_url(url: &str) {
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
    }
}

/// Development mode configuration parsed from CLI arguments.
/// When active, all persistence is disabled and the game starts at a specific level.
struct DevMode {
    /// Whether dev mode is active.
    enabled: bool,
    /// Starting level (1-1000). Only used when enabled is true.
    start_level: u32,
}

/// Parses command-line arguments for dev mode.
/// Supports: --dev --level N
fn parse_dev_args() -> DevMode {
    let args: Vec<String> = std::env::args().collect();
    let mut enabled = false;
    let mut start_level = 1u32;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--dev" => enabled = true,
            "--level" => {
                if i + 1 < args.len() {
                    if let Ok(lvl) = args[i + 1].parse::<u32>() {
                        if (1..=MAX_LEVEL).contains(&lvl) {
                            start_level = lvl;
                        } else {
                            eprintln!("[xMahjong] Warning: --level must be 1-{}, got {}. Using 1.", MAX_LEVEL, lvl);
                        }
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    DevMode { enabled, start_level }
}

/// Checks if a raw SDL event type ID corresponds to a known SDL2 event type.
///
/// The sdl2 crate v0.37 uses `unsafe { transmute(n) }` in `EventType::try_from`,
/// which causes undefined behavior (abort) if the value doesn't match any valid
/// `SDL_EventType` discriminant. This function acts as a pre-filter to avoid that.
fn is_known_sdl2_event_type(raw_type: u32) -> bool {
    use sdl2::sys::SDL_EventType::*;
    matches!(
        raw_type,
        x if x == SDL_FIRSTEVENT as u32
            || x == SDL_QUIT as u32
            || x == SDL_APP_TERMINATING as u32
            || x == SDL_APP_LOWMEMORY as u32
            || x == SDL_APP_WILLENTERBACKGROUND as u32
            || x == SDL_APP_DIDENTERBACKGROUND as u32
            || x == SDL_APP_WILLENTERFOREGROUND as u32
            || x == SDL_APP_DIDENTERFOREGROUND as u32
            || x == SDL_LOCALECHANGED as u32
            || x == SDL_DISPLAYEVENT as u32
            || x == SDL_WINDOWEVENT as u32
            || x == SDL_SYSWMEVENT as u32
            || x == SDL_KEYDOWN as u32
            || x == SDL_KEYUP as u32
            || x == SDL_TEXTEDITING as u32
            || x == SDL_TEXTINPUT as u32
            || x == SDL_KEYMAPCHANGED as u32
            || x == SDL_TEXTEDITING_EXT as u32
            || x == SDL_MOUSEMOTION as u32
            || x == SDL_MOUSEBUTTONDOWN as u32
            || x == SDL_MOUSEBUTTONUP as u32
            || x == SDL_MOUSEWHEEL as u32
            || x == SDL_JOYAXISMOTION as u32
            || x == SDL_JOYBALLMOTION as u32
            || x == SDL_JOYHATMOTION as u32
            || x == SDL_JOYBUTTONDOWN as u32
            || x == SDL_JOYBUTTONUP as u32
            || x == SDL_JOYDEVICEADDED as u32
            || x == SDL_JOYDEVICEREMOVED as u32
            || x == SDL_JOYBATTERYUPDATED as u32
            || x == SDL_CONTROLLERAXISMOTION as u32
            || x == SDL_CONTROLLERBUTTONDOWN as u32
            || x == SDL_CONTROLLERBUTTONUP as u32
            || x == SDL_CONTROLLERDEVICEADDED as u32
            || x == SDL_CONTROLLERDEVICEREMOVED as u32
            || x == SDL_CONTROLLERDEVICEREMAPPED as u32
            || x == SDL_CONTROLLERTOUCHPADDOWN as u32
            || x == SDL_CONTROLLERTOUCHPADMOTION as u32
            || x == SDL_CONTROLLERTOUCHPADUP as u32
            || x == SDL_CONTROLLERSENSORUPDATE as u32
            || x == SDL_FINGERDOWN as u32
            || x == SDL_FINGERUP as u32
            || x == SDL_FINGERMOTION as u32
            || x == SDL_DOLLARGESTURE as u32
            || x == SDL_DOLLARRECORD as u32
            || x == SDL_MULTIGESTURE as u32
            || x == SDL_CLIPBOARDUPDATE as u32
            || x == SDL_DROPFILE as u32
            || x == SDL_DROPTEXT as u32
            || x == SDL_DROPBEGIN as u32
            || x == SDL_DROPCOMPLETE as u32
            || x == SDL_AUDIODEVICEADDED as u32
            || x == SDL_AUDIODEVICEREMOVED as u32
            || x == SDL_SENSORUPDATE as u32
            || x == SDL_RENDER_TARGETS_RESET as u32
            || x == SDL_RENDER_DEVICE_RESET as u32
            || x == SDL_POLLSENTINEL as u32
            || x == SDL_USEREVENT as u32
            || x == SDL_LASTEVENT as u32
    )
}

fn main() {
    // Parse command-line arguments for dev mode
    let dev_mode = parse_dev_args();

    if dev_mode.enabled {
        eprintln!("[xMahjong] DEV MODE: Starting at level {}. All saving disabled.", dev_mode.start_level);
    }

    // 1. Initialize SDL2 context
    let sdl_context = sdl2::init().expect("Failed to initialize SDL2");

    // 2. Create Renderer (handles window, canvas, textures)
    let mut renderer = Renderer::new(&sdl_context).expect("Failed to create renderer");

    // In dev mode, update window title to show dev status
    if dev_mode.enabled {
        renderer.canvas.window_mut().set_title(
            &format!("[DEV] xMahjong - Level {}", dev_mode.start_level)
        ).ok();
    }

    // Enable SDL2 text input (needed for TextInput events during name entry)
    {
        let video = sdl_context.video().expect("Failed to get video subsystem for text input");
        video.text_input().start();
    }

    // 3. Create AudioManager (handles init failure gracefully)
    let mut audio = AudioManager::new();

    // 4. Active username and settings
    let mut current_user_name = String::new();
    let mut settings = Settings::default();

    // 5. Create InputHandler
    let input_handler = InputHandler::new();

    // 6. Generate initial board and create GameState
    let mut game_state = if dev_mode.enabled {
        current_user_name = "dev".to_string();
        settings = Settings::load(&current_user_name);
        audio.set_mute(settings.muted);
        create_new_game_state_for_level(dev_mode.start_level, Difficulty::Easy)
    } else {
        create_new_game_state()
    };

    let mut name_entry: Option<NameEntryState> = None;
    let mut daily_streak_achievement = None;

    if dev_mode.enabled {
        game_state.timer.start();
    } else {
        // Startup name entry prompt
        name_entry = Some(NameEntryState::new(0, 0, 0, 0, 0));
        game_state.status = GameStatus::NameEntry;
    }

    // Track whether we're showing a quit confirmation
    let mut quit_confirmation = false;

    // Track which state to return to when leaving the leaderboard view
    let mut leaderboard_return_status = GameStatus::Won;
    // Track the currently selected menu item in the pause menu (0-indexed)
    let mut pause_menu_selection: usize = 0;
    const PAUSE_MENU_ITEM_COUNT: usize = 11;
    // Track level select page (0-indexed, 25 levels per page) and chosen level (1..=1000)
    let mut level_select_page: usize = 0;
    let mut level_select_chosen: u32 = 1;
    // Track the currently selected menu item in the victory dialog (0-indexed)
    let mut victory_menu_selection: usize = 0;
    // Track the currently selected item in the No Moves dialog (0=Shuffle, 1=New Game)
    let mut lost_menu_selection: usize = 0;
    // Track the currently selected item in the Game Over dialog (0=Save Score, 1=New Game)
    let mut game_over_menu_selection: usize = 0;

    // Track inactivity: time of last user action (click, key) while Playing.
    // After 120 seconds of inactivity without matching a pair, show a hint suggestion.
    let mut last_activity_time = Instant::now();
    // Whether the hint suggestion overlay is currently visible.
    let mut show_hint_suggestion = false;
    const INACTIVITY_HINT_SECS: u64 = 60;

    // --- Tile magnification (long-press to zoom) ---
    // Tracks the time and position of a mouse/touch press for long-press detection.
    let mut press_start: Option<(Instant, i32, i32)> = None;
    // The tile index being magnified (shown as a large overlay), if any.
    let mut magnified_tile: Option<usize> = None;
    /// Duration in milliseconds a press must be held to trigger magnification.
    const MAGNIFY_HOLD_MS: u128 = 500;

    // Check for updates at startup (non-blocking: if network fails, silently skip)
    // Skip in dev mode to avoid unnecessary network calls
    let mut update_info: Option<UpdateInfo> = if dev_mode.enabled {
        None
    } else {
        check_for_update().map(|v| UpdateInfo { latest_version: v })
    };

    // SDL2 event pump
    let event_pump = sdl_context
        .event_pump()
        .expect("Failed to create SDL2 event pump");

    // 7. Main game loop
    'game_loop: loop {
        let frame_start = Instant::now();

        // --- 7a. Poll SDL2 events and process through InputHandler ---
        // NOTE: We call SDL_PollEvent via raw FFI and filter out unknown event types
        // before calling Event::from_ll(). The sdl2 crate v0.37 uses unsafe transmute
        // in EventType::try_from which causes UB/abort on event type IDs not present
        // in the compiled SDL_EventType enum (happens with SDL3 compat shims on macOS).
        loop {
            let mut raw = std::mem::MaybeUninit::<sdl2::sys::SDL_Event>::uninit();
            let has_event = unsafe { sdl2::sys::SDL_PollEvent(raw.as_mut_ptr()) == 1 };
            if !has_event {
                break;
            }
            let raw = unsafe { raw.assume_init() };
            let raw_type = unsafe { raw.type_ };

            // Only process events whose type ID matches a known SDL2 event type.
            // This prevents the sdl2 crate's unsafe transmute from creating invalid
            // enum values (which aborts the process) when SDL3 injects unknown events.
            if !is_known_sdl2_event_type(raw_type) {
                continue;
            }

            let event = sdl2::event::Event::from_ll(raw);
            // Dismiss daily streak popup on any keydown or click
            if daily_streak_achievement.is_some() {
                match event {
                    sdl2::event::Event::KeyDown { .. } | sdl2::event::Event::MouseButtonDown { .. } => {
                        daily_streak_achievement = None;
                        continue;
                    }
                    _ => {}
                }
            }

            // --- Handle name entry state input first ---
            if game_state.status == GameStatus::NameEntry {
                if let Some(ref mut entry) = name_entry {
                    match &event {
                        sdl2::event::Event::TextInput { text, .. } => {
                            for c in text.chars() {
                                entry.push_char(c);
                            }
                            continue;
                        }
                        sdl2::event::Event::KeyDown {
                            keycode: Some(keycode),
                            ..
                        } => {
                            match *keycode {
                                sdl2::keyboard::Keycode::Return
                                | sdl2::keyboard::Keycode::KpEnter => {
                                    // Submit the username if valid
                                    if entry.is_valid() {
                                        current_user_name = entry.text.trim().to_string();

                                        // Load settings for this user
                                        settings = Settings::load(&current_user_name);
                                        audio.set_mute(settings.muted);

                                        // Load saved game if exists, or create new game
                                        game_state = if !dev_mode.enabled && SavedGame::exists(&current_user_name) {
                                            match load_saved_game(&current_user_name) {
                                                Some(state) => {
                                                    SavedGame::delete(&current_user_name);
                                                    state
                                                }
                                                None => create_new_game_state(),
                                            }
                                        } else {
                                            create_new_game_state()
                                        };

                                        // Synchronize user progress with current game level
                                        if !dev_mode.enabled {
                                            UserProgress::load_and_sync(&current_user_name, game_state.level);
                                        }

                                        // Load shuffle state and apply daily bonus for user
                                        let mut shuffle_state = ShuffleState::load(&current_user_name);
                                        let today = current_date_string();
                                        let today_days = {
                                            use std::time::SystemTime;
                                            let now = SystemTime::now()
                                                .duration_since(SystemTime::UNIX_EPOCH)
                                                .unwrap_or_default()
                                                .as_secs();
                                            now / 86400
                                        };
                                        let daily_bonus = shuffle_state.claim_daily_bonus(&today);
                                        if daily_bonus {
                                            game_state.shuffles_remaining += 1;
                                            if !dev_mode.enabled {
                                                let last_days = shuffle_state.last_launch_epoch_days;
                                                if last_days == 0 {
                                                    shuffle_state.consecutive_days = 1;
                                                    daily_streak_achievement = Some(1);
                                                } else if last_days == today_days - 1 {
                                                    shuffle_state.consecutive_days += 1;
                                                    daily_streak_achievement = Some(shuffle_state.consecutive_days);
                                                } else {
                                                    shuffle_state.consecutive_days = 1;
                                                    daily_streak_achievement = Some(1);
                                                }
                                                shuffle_state.last_launch_epoch_days = today_days;
                                            }
                                        }
                                        if !dev_mode.enabled {
                                            shuffle_state.save(&current_user_name);
                                        }

                                        name_entry = None;
                                        game_state.timer.start();
                                        game_state.status = GameStatus::Playing;
                                    }
                                    continue;
                                }
                                sdl2::keyboard::Keycode::Backspace => {
                                    entry.pop_char();
                                    continue;
                                }
                                sdl2::keyboard::Keycode::Escape => {
                                    if current_user_name.is_empty() {
                                        break 'game_loop;
                                    } else {
                                        name_entry = None;
                                        game_state.status = GameStatus::Playing;
                                    }
                                    continue;
                                }
                                _ => {
                                    continue;
                                }
                            }
                        }
                        sdl2::event::Event::Quit { .. } => {
                            break 'game_loop;
                        }
                        _ => {
                            continue;
                        }
                    }
                }
            }

            // --- Handle pause menu keyboard navigation ---
            if game_state.status == GameStatus::Paused {
                if let sdl2::event::Event::KeyDown { keycode: Some(keycode), .. } = &event {
                    match *keycode {
                        sdl2::keyboard::Keycode::Up => {
                            if pause_menu_selection == 0 {
                                pause_menu_selection = PAUSE_MENU_ITEM_COUNT - 1;
                            } else {
                                pause_menu_selection -= 1;
                            }
                            continue;
                        }
                        sdl2::keyboard::Keycode::Down => {
                            pause_menu_selection = (pause_menu_selection + 1) % PAUSE_MENU_ITEM_COUNT;
                            continue;
                        }
                        sdl2::keyboard::Keycode::Return
                        | sdl2::keyboard::Keycode::KpEnter => {
                            // Reset inactivity when user interacts with pause menu
                            last_activity_time = Instant::now();
                            show_hint_suggestion = false;
                            match pause_menu_selection {
                                0 => {
                                    // NEW GAME (preserves current difficulty)
                                    let diff = game_state.difficulty;
                                    game_state = create_new_game_state_for_level(1, diff);
                                    game_state.timer.start();
                                }
                                1 => {
                                    // UNDO
                                    game_state.status = GameStatus::Playing;
                                    game_state.timer.resume();
                                    let _ = logic::undo(&mut game_state);
                                }
                                2 => {
                                    // HINT
                                    game_state.status = GameStatus::Playing;
                                    game_state.timer.resume();
                                    let result = logic::request_hint(&mut game_state);
                                    if let HintResult::NoMatchesAvailable = result {
                                        if game_state.shuffles_remaining == 0 {
                                            game_state.timer.pause();
                                            game_state.score.elapsed_seconds = game_state.timer.elapsed_seconds();
                                            game_state.status = GameStatus::GameOver;
                                        } else {
                                            game_state.status = GameStatus::Lost;
                                        }
                                    }
                                }
                                3 => {
                                    // SHUFFLE
                                    game_state.status = GameStatus::Playing;
                                    game_state.timer.resume();
                                    if logic::shuffle(&mut game_state).is_ok() {
                                        audio.play_shuffle();
                                    }
                                }
                                4 => {
                                    // LEVELS
                                    UserProgress::load_and_sync(&current_user_name, game_state.level);
                                    let start_lvl = game_state.level.min(1000);
                                    level_select_page = ((start_lvl - 1) / 25) as usize;
                                    level_select_chosen = start_lvl;
                                    game_state.status = GameStatus::LevelSelect;
                                }
                                5 => {
                                    // SHORTCUTS
                                    game_state.status = GameStatus::Shortcuts;
                                }
                                6 => {
                                    // LEADERBOARD
                                    leaderboard_return_status = GameStatus::Paused;
                                    game_state.status = GameStatus::Leaderboard;
                                }
                                7 => {
                                    // DIFFICULTY toggle
                                    game_state.difficulty = match game_state.difficulty {
                                        Difficulty::Easy => Difficulty::Normal,
                                        Difficulty::Normal => Difficulty::Easy,
                                    };
                                }
                                8 => {
                                    // ABOUT — open website in default browser
                                    let _ = open::that(ABOUT_URL);
                                }
                                9 => {
                                    // SWITCH USER
                                    if !dev_mode.enabled {
                                        save_current_game(&game_state, &current_user_name);
                                    }
                                    current_user_name.clear();
                                    name_entry = Some(NameEntryState::new(0, 0, 0, 0, 0));
                                    game_state.status = GameStatus::NameEntry;
                                    pause_menu_selection = 0;
                                }
                                10 => {
                                    // SAVE + QUIT
                                    if !dev_mode.enabled {
                                        save_current_game(&game_state, &current_user_name);
                                    }
                                    break 'game_loop;
                                }
                                _ => {}
                            }
                            continue;
                        }
                        _ => {}
                    }
                }
            }

            // --- Handle level select keyboard navigation ---
            if game_state.status == GameStatus::LevelSelect {
                if let sdl2::event::Event::KeyDown { keycode: Some(keycode), .. } = &event {
                    match *keycode {
                        sdl2::keyboard::Keycode::Left => {
                            if level_select_chosen > 1 {
                                level_select_chosen -= 1;
                            } else {
                                level_select_chosen = 1000;
                            }
                            level_select_page = ((level_select_chosen - 1) / 25) as usize;
                            continue;
                        }
                        sdl2::keyboard::Keycode::Right => {
                            if level_select_chosen < 1000 {
                                level_select_chosen += 1;
                            } else {
                                level_select_chosen = 1;
                            }
                            level_select_page = ((level_select_chosen - 1) / 25) as usize;
                            continue;
                        }
                        sdl2::keyboard::Keycode::Up => {
                            if level_select_chosen > 5 {
                                level_select_chosen -= 5;
                            }
                            level_select_page = ((level_select_chosen - 1) / 25) as usize;
                            continue;
                        }
                        sdl2::keyboard::Keycode::Down => {
                            if level_select_chosen + 5 <= 1000 {
                                level_select_chosen += 5;
                            }
                            level_select_page = ((level_select_chosen - 1) / 25) as usize;
                            continue;
                        }
                        sdl2::keyboard::Keycode::PageUp
                        | sdl2::keyboard::Keycode::LeftBracket => {
                            if level_select_page > 0 {
                                level_select_page -= 1;
                                level_select_chosen = (level_select_page * 25 + 1) as u32;
                            }
                            continue;
                        }
                        sdl2::keyboard::Keycode::PageDown
                        | sdl2::keyboard::Keycode::RightBracket => {
                            if level_select_page + 1 < 40 {
                                level_select_page += 1;
                                level_select_chosen = (level_select_page * 25 + 1) as u32;
                            }
                            continue;
                        }
                        sdl2::keyboard::Keycode::Return
                        | sdl2::keyboard::Keycode::KpEnter => {
                            let user_progress = UserProgress::load_and_sync(&current_user_name, game_state.level);
                            if user_progress.is_level_unlocked(level_select_chosen) {
                                let diff = game_state.difficulty;
                                game_state = create_new_game_state_for_level(level_select_chosen, diff);
                                game_state.timer.start();
                                game_state.status = GameStatus::Playing;
                                quit_confirmation = false;
                                last_activity_time = Instant::now();
                                show_hint_suggestion = false;
                            }
                            continue;
                        }
                        sdl2::keyboard::Keycode::Escape => {
                            game_state.status = GameStatus::Paused;
                            continue;
                        }
                        _ => {}
                    }
                }
            }

            // --- Handle leaderboard keyboard navigation ---
            if game_state.status == GameStatus::Leaderboard {
                if let sdl2::event::Event::KeyDown { keycode: Some(keycode), .. } = &event {
                    match *keycode {
                        sdl2::keyboard::Keycode::Return
                        | sdl2::keyboard::Keycode::KpEnter
                        | sdl2::keyboard::Keycode::Escape => {
                            game_state.status = leaderboard_return_status;
                            continue;
                        }
                        _ => {}
                    }
                }
            }

            // --- Handle shortcuts popup keyboard navigation ---
            if game_state.status == GameStatus::Shortcuts {
                if let sdl2::event::Event::KeyDown { keycode: Some(keycode), .. } = &event {
                    match *keycode {
                        sdl2::keyboard::Keycode::Return
                        | sdl2::keyboard::Keycode::KpEnter
                        | sdl2::keyboard::Keycode::Escape => {
                            game_state.status = GameStatus::Paused;
                            continue;
                        }
                        _ => {}
                    }
                }
            }

            // --- Handle victory menu keyboard navigation ---
            if game_state.status == GameStatus::Won {
                if let sdl2::event::Event::KeyDown { keycode: Some(keycode), .. } = &event {
                    let item_count = if game_state.level < MAX_LEVEL { 3usize } else { 2usize };
                    match *keycode {
                        sdl2::keyboard::Keycode::Up => {
                            if victory_menu_selection == 0 {
                                victory_menu_selection = item_count - 1;
                            } else {
                                victory_menu_selection -= 1;
                            }
                            continue;
                        }
                        sdl2::keyboard::Keycode::Down => {
                            victory_menu_selection = (victory_menu_selection + 1) % item_count;
                            continue;
                        }
                        sdl2::keyboard::Keycode::Return
                        | sdl2::keyboard::Keycode::KpEnter => {
                            if game_state.level < MAX_LEVEL {
                                match victory_menu_selection {
                                    0 => {
                                        // NEXT LEVEL
                                        let next_level = game_state.level + 1;
                                        let accumulated = game_state.base_score + game_state.score.calculate_score();
                                        let accumulated_time = game_state.base_time_ms + game_state.timer.elapsed_ms;
                                        let accumulated_hints = game_state.base_hints + game_state.score.hints_used;
                                        let accumulated_shuffles = game_state.base_shuffles + game_state.score.shuffles_used;
                                        let accumulated_undos = game_state.base_undos + game_state.score.undos_used;
                                        let remaining_shuffles = game_state.shuffles_remaining + 1; // +1 shuffle reward for completing level
                                        let diff = game_state.difficulty;
                                        game_state = create_new_game_state_for_level(next_level, diff);
                                        game_state.base_score = accumulated;
                                        game_state.base_time_ms = accumulated_time;
                                        game_state.base_hints = accumulated_hints;
                                        game_state.base_shuffles = accumulated_shuffles;
                                        game_state.base_undos = accumulated_undos;
                                        game_state.shuffles_remaining = remaining_shuffles;
                                        game_state.timer.start();
                                        victory_menu_selection = 0;
                                    }
                                    1 => {
                                        // NEW GAME
                                        let diff = game_state.difficulty; game_state = create_new_game_state_with_difficulty(diff);
                                        game_state.timer.start();
                                        victory_menu_selection = 0;
                                    }
                                    2 => {
                                        // LEADERBOARD
                                        leaderboard_return_status = GameStatus::Won;
                                        game_state.status = GameStatus::Leaderboard;
                                    }
                                    _ => {}
                                }
                            } else {
                                match victory_menu_selection {
                                    0 => {
                                        // NEW GAME
                                        let diff = game_state.difficulty; game_state = create_new_game_state_with_difficulty(diff);
                                        game_state.timer.start();
                                        victory_menu_selection = 0;
                                    }
                                    1 => {
                                        // LEADERBOARD
                                        leaderboard_return_status = GameStatus::Won;
                                        game_state.status = GameStatus::Leaderboard;
                                    }
                                    _ => {}
                                }
                            }
                            continue;
                        }
                        _ => {}
                    }
                }
            }

            // --- Handle No Moves (Lost) dialog keyboard navigation ---
            if game_state.status == GameStatus::Lost {
                if let sdl2::event::Event::KeyDown { keycode: Some(keycode), .. } = &event {
                    match *keycode {
                        sdl2::keyboard::Keycode::Up => {
                            if lost_menu_selection == 0 {
                                lost_menu_selection = 1;
                            } else {
                                lost_menu_selection -= 1;
                            }
                            continue;
                        }
                        sdl2::keyboard::Keycode::Down => {
                            lost_menu_selection = (lost_menu_selection + 1) % 2;
                            continue;
                        }
                        sdl2::keyboard::Keycode::Return
                        | sdl2::keyboard::Keycode::KpEnter => {
                            match lost_menu_selection {
                                0 => {
                                    // SHUFFLE
                                    game_state.status = GameStatus::Playing;
                                    if logic::shuffle(&mut game_state).is_ok() {
                                        audio.play_shuffle();
                                    }
                                }
                                1 => {
                                    // NEW GAME
                                    let diff = game_state.difficulty; game_state = create_new_game_state_with_difficulty(diff);
                                    game_state.timer.start();
                                }
                                _ => {}
                            }
                            lost_menu_selection = 0;
                            continue;
                        }
                        _ => {}
                    }
                }
            }

            // --- Handle Game Over dialog keyboard navigation ---
            if game_state.status == GameStatus::GameOver {
                if let sdl2::event::Event::KeyDown { keycode: Some(keycode), .. } = &event {
                    match *keycode {
                        sdl2::keyboard::Keycode::Up => {
                            if game_over_menu_selection == 0 {
                                game_over_menu_selection = 2;
                            } else {
                                game_over_menu_selection -= 1;
                            }
                            continue;
                        }
                        sdl2::keyboard::Keycode::Down => {
                            game_over_menu_selection = (game_over_menu_selection + 1) % 3;
                            continue;
                        }
                        sdl2::keyboard::Keycode::Return
                        | sdl2::keyboard::Keycode::KpEnter => {
                            match game_over_menu_selection {
                                0 => {
                                    // SAVE SCORE
                                    let score = game_state.base_score + game_state.score.live_score();
                                    let total_time_ms = game_state.base_time_ms + game_state.timer.elapsed_ms;
                                    let time_seconds = (total_time_ms / 1000) as u32;
                                    let hints_used = game_state.base_hints + game_state.score.hints_used;
                                    let shuffles_used = game_state.base_shuffles + game_state.score.shuffles_used;
                                    let undos_used = game_state.base_undos + game_state.score.undos_used;
                                    let shuffle_state_loaded = ShuffleState::load(&current_user_name);
                                    let lb_entry = LeaderboardEntry {
                                        name: current_user_name.clone(),
                                        score,
                                        time_seconds,
                                        hints_used,
                                        shuffles_used,
                                        undos_used,
                                        difficulty: match game_state.difficulty {
                                            Difficulty::Easy => "easy".to_string(),
                                            Difficulty::Normal => "normal".to_string(),
                                        },
                                        date: current_date_string(),
                                        consecutive_days: shuffle_state_loaded.consecutive_days,
                                    };
                                    if !dev_mode.enabled {
                                        let mut leaderboard = Leaderboard::load(&current_user_name);
                                        leaderboard.insert(lb_entry);
                                        leaderboard.save(&current_user_name);
                                    }
                                    leaderboard_return_status = GameStatus::GameOver;
                                    game_state.status = GameStatus::Leaderboard;
                                }
                                1 => {
                                    // NEW GAME
                                    let diff = game_state.difficulty; game_state = create_new_game_state_with_difficulty(diff);
                                    game_state.timer.start();
                                }
                                2 => {
                                    // WAIT FOR SHUFFLE — save game and quit
                                    if !dev_mode.enabled {
                                        save_current_game(&game_state, &current_user_name);
                                    }
                                    break 'game_loop;
                                }
                                _ => {}
                            }
                            game_over_menu_selection = 0;
                            continue;
                        }
                        _ => {}
                    }
                }
            }

            let is_paused = matches!(
                game_state.status,
                GameStatus::Paused | GameStatus::Menu
            );

            if let Some(action) = input_handler.process_event(&event, is_paused) {
                // If update dialog is showing, handle it first
                if update_info.is_some() {
                    match action {
                        GameAction::SelectTile(x, y) => {
                            // Check if click hit DOWNLOAD or NOT NOW button
                            let (win_w, win_h) = renderer.window_size();
                            let dialog_w: i32 = 360;
                            let dialog_h: i32 = 200;
                            let dialog_x = (win_w as i32 - dialog_w) / 2;
                            let dialog_y = (win_h as i32 - dialog_h) / 2;

                            let btn_w: i32 = 140;
                            let btn_h: i32 = 40;
                            let btn_spacing: i32 = 20;
                            let total_btn_width = btn_w * 2 + btn_spacing;
                            let btn_start_x = dialog_x + (dialog_w - total_btn_width) / 2;
                            let btn_y = dialog_y + 140;

                            // DOWNLOAD button area
                            if x >= btn_start_x && x < btn_start_x + btn_w
                                && y >= btn_y && y < btn_y + btn_h
                            {
                                open_url(RELEASE_DOWNLOAD_URL);
                                update_info = None;
                            }

                            // NOT NOW button area
                            let not_now_x = btn_start_x + btn_w + btn_spacing;
                            if x >= not_now_x && x < not_now_x + btn_w
                                && y >= btn_y && y < btn_y + btn_h
                            {
                                update_info = None;
                            }
                            continue;
                        }
                        GameAction::Quit => {
                            update_info = None;
                            continue;
                        }
                        _ => {
                            // Any other key dismisses the update dialog
                            update_info = None;
                            continue;
                        }
                    }
                }

                // If quit confirmation is showing, handle specially
                if quit_confirmation {
                    match action {
                        GameAction::Quit => break 'game_loop,
                        GameAction::SelectTile(x, y) => {
                            // Check if click hit YES or NO button
                            let (win_w, win_h) = renderer.window_size();
                            let dialog_w: i32 = 300;
                            let dialog_h: i32 = 180;
                            let dialog_x = (win_w as i32 - dialog_w) / 2;
                            let dialog_y = (win_h as i32 - dialog_h) / 2;

                            let btn_w: i32 = 120;
                            let btn_h: i32 = 40;
                            let btn_spacing: i32 = 20;
                            let total_btn_width = btn_w * 2 + btn_spacing;
                            let btn_start_x = dialog_x + (dialog_w - total_btn_width) / 2;
                            let btn_y = dialog_y + 110;

                            // YES button area
                            if x >= btn_start_x && x < btn_start_x + btn_w
                                && y >= btn_y && y < btn_y + btn_h
                            {
                                break 'game_loop;
                            }

                            // NO button area
                            let no_x = btn_start_x + btn_w + btn_spacing;
                            if x >= no_x && x < no_x + btn_w
                                && y >= btn_y && y < btn_y + btn_h
                            {
                                quit_confirmation = false;
                            }
                            // Click outside buttons: do nothing (dialog stays)
                            continue;
                        }
                        _ => {
                            // Escape or any key dismisses confirmation
                            quit_confirmation = false;
                            continue;
                        }
                    }
                }

                // --- 7b. Process GameAction based on current GameStatus ---

                // Reset inactivity timer on any user action while playing
                if game_state.status == GameStatus::Playing {
                    last_activity_time = Instant::now();
                    show_hint_suggestion = false;
                }

                match action {
                    GameAction::SelectTile(x, y) => {
                        // Record press start for long-press magnification detection
                        press_start = Some((Instant::now(), x, y));
                        magnified_tile = None;

                        if game_state.status == GameStatus::Playing {
                            // Check if click is on the menu button (bottom-left corner)
                            let (win_w, win_h) = renderer.window_size();
                            let menu_btn_x: i32 = 10;
                            let menu_btn_y: i32 = win_h as i32 - 26 - 10;
                            let menu_btn_w: i32 = 80;
                            let menu_btn_h: i32 = 26;
                            if x >= menu_btn_x && x < menu_btn_x + menu_btn_w
                                && y >= menu_btn_y && y < menu_btn_y + menu_btn_h
                            {
                                // Clicked menu button — open pause menu
                                game_state.status = GameStatus::Paused;
                                game_state.timer.pause();
                                pause_menu_selection = 0;
                            }
                            // Check if click is on the HUD shuffle area (rightmost 1/5th)
                            else if y >= 0 && y < 40 {
                                let section_w = win_w as i32 / 5;
                                let shuffle_section_start = section_w * 4;
                                if x >= shuffle_section_start && x < win_w as i32 {
                                    // Clicked on shuffle in HUD
                                    match logic::shuffle(&mut game_state) {
                                        Ok(()) => {
                                            audio.play_shuffle();
                                        }
                                        Err(_) => {
                                            audio.play_error();
                                        }
                                    }
                                }
                            } else {
                                let won = handle_select_tile(
                                    &mut game_state,
                                    &mut audio,
                                    &renderer,
                                    x,
                                    y,
                                    &current_user_name,
                                    dev_mode.enabled,
                                );
                                if won {
                                    // Reset victory menu selection
                                    victory_menu_selection = 0;
                                    let score = game_state.base_score + game_state.score.calculate_score();
                                    let total_time_ms = game_state.base_time_ms + game_state.timer.elapsed_ms;
                                    let time_seconds = (total_time_ms / 1000) as u32;
                                    let hints_used = game_state.base_hints + game_state.score.hints_used;
                                    let shuffles_used = game_state.base_shuffles + game_state.score.shuffles_used;
                                    let undos_used = game_state.base_undos + game_state.score.undos_used;

                                    let shuffle_state_loaded = ShuffleState::load(&current_user_name);
                                    let lb_entry = LeaderboardEntry {
                                        name: current_user_name.clone(),
                                        score,
                                        time_seconds,
                                        hints_used,
                                        shuffles_used,
                                        undos_used,
                                        difficulty: match game_state.difficulty {
                                            Difficulty::Easy => "easy".to_string(),
                                            Difficulty::Normal => "normal".to_string(),
                                        },
                                        date: current_date_string(),
                                        consecutive_days: shuffle_state_loaded.consecutive_days,
                                    };
                                    if !dev_mode.enabled {
                                        let mut leaderboard = Leaderboard::load(&current_user_name);
                                        leaderboard.insert(lb_entry);
                                        leaderboard.save(&current_user_name);
                                        let mut progress = UserProgress::load(&current_user_name);
                                        progress.mark_completed(game_state.level);
                                        progress.save(&current_user_name);
                                    }
                                    game_state.status = GameStatus::Won;
                                }
                            }
                        } else if game_state.status == GameStatus::Paused {
                            // Handle clicks on pause menu buttons
                            let (win_w, win_h) = renderer.window_size();
                            let dialog_w: u32 = 300;
                            let dialog_h: u32 = 610;
                            let dialog_x = (win_w.saturating_sub(dialog_w)) / 2;
                            let dialog_y = (win_h.saturating_sub(dialog_h)) / 2;

                            let btn_w: u32 = 220;
                            let btn_h: u32 = 36;
                            let btn_x = dialog_x as i32 + ((dialog_w - btn_w) / 2) as i32;
                            let start_y = dialog_y as i32 + 52;
                            let spacing: i32 = 44;

                            // Check which button was clicked
                            if x >= btn_x && x < btn_x + btn_w as i32 {
                                if y >= start_y && y < start_y + btn_h as i32 {
                                    // NEW GAME
                                    let diff = game_state.difficulty; game_state = create_new_game_state_with_difficulty(diff);
                                    game_state.timer.start();
                                } else if y >= start_y + spacing && y < start_y + spacing + btn_h as i32 {
                                    // UNDO
                                    game_state.status = GameStatus::Playing;
                                    game_state.timer.resume();
                                    let _ = logic::undo(&mut game_state);
                                } else if y >= start_y + spacing * 2 && y < start_y + spacing * 2 + btn_h as i32 {
                                    // HINT
                                    game_state.status = GameStatus::Playing;
                                    game_state.timer.resume();
                                    let result = logic::request_hint(&mut game_state);
                                    if let HintResult::NoMatchesAvailable = result {
                                        if game_state.shuffles_remaining == 0 {
                                            game_state.timer.pause();
                                            game_state.score.elapsed_seconds = game_state.timer.elapsed_seconds();
                                            game_state.status = GameStatus::GameOver;
                                        } else {
                                            game_state.status = GameStatus::Lost;
                                        }
                                    }
                                } else if y >= start_y + spacing * 3 && y < start_y + spacing * 3 + btn_h as i32 {
                                    // SHUFFLE
                                    game_state.status = GameStatus::Playing;
                                    game_state.timer.resume();
                                    if logic::shuffle(&mut game_state).is_ok() {
                                        audio.play_shuffle();
                                    }
                                } else if y >= start_y + spacing * 4 && y < start_y + spacing * 4 + btn_h as i32 {
                                    // LEVELS
                                    UserProgress::load_and_sync(&current_user_name, game_state.level);
                                    let start_lvl = game_state.level.min(1000);
                                    level_select_page = ((start_lvl - 1) / 25) as usize;
                                    level_select_chosen = start_lvl;
                                    game_state.status = GameStatus::LevelSelect;
                                } else if y >= start_y + spacing * 5 && y < start_y + spacing * 5 + btn_h as i32 {
                                    // SHORTCUTS
                                    game_state.status = GameStatus::Shortcuts;
                                } else if y >= start_y + spacing * 6 && y < start_y + spacing * 6 + btn_h as i32 {
                                    // LEADERBOARD / ACHIEVEMENTS
                                    leaderboard_return_status = GameStatus::Paused;
                                    game_state.status = GameStatus::Leaderboard;
                                } else if y >= start_y + spacing * 7 && y < start_y + spacing * 7 + btn_h as i32 {
                                    // DIFFICULTY toggle
                                    game_state.difficulty = match game_state.difficulty {
                                        Difficulty::Easy => Difficulty::Normal,
                                        Difficulty::Normal => Difficulty::Easy,
                                    };
                                } else if y >= start_y + spacing * 8 && y < start_y + spacing * 8 + btn_h as i32 {
                                    // ABOUT — open website in default browser
                                    let _ = open::that(ABOUT_URL);
                                } else if y >= start_y + spacing * 9 && y < start_y + spacing * 9 + btn_h as i32 {
                                    // SWITCH USER
                                    if !dev_mode.enabled {
                                        save_current_game(&game_state, &current_user_name);
                                    }
                                    current_user_name.clear();
                                    name_entry = Some(NameEntryState::new(0, 0, 0, 0, 0));
                                    game_state.status = GameStatus::NameEntry;
                                    pause_menu_selection = 0;
                                } else if y >= start_y + spacing * 10 && y < start_y + spacing * 10 + btn_h as i32 {
                                    // SAVE + QUIT
                                    if !dev_mode.enabled {
                                        save_current_game(&game_state, &current_user_name);
                                    }
                                    break 'game_loop;
                                }
                            }
                        } else if game_state.status == GameStatus::Lost {
                            // Handle clicks on the "No Moves" dialog buttons
                            let (win_w, win_h) = renderer.window_size();
                            let dialog_w: u32 = 320;
                            let dialog_h: u32 = 220;
                            let dialog_x = (win_w.saturating_sub(dialog_w)) / 2;
                            let dialog_y = (win_h.saturating_sub(dialog_h)) / 2;

                            let btn_w: u32 = 200;
                            let btn_h: u32 = 44;
                            let btn_x = dialog_x as i32 + ((dialog_w - btn_w) / 2) as i32;

                            // Shuffle button: y offset 110 from dialog top
                            let shuffle_y = dialog_y as i32 + 110;
                            if x >= btn_x && x < btn_x + btn_w as i32
                                && y >= shuffle_y && y < shuffle_y + btn_h as i32
                            {
                                // Perform shuffle and return to playing
                                game_state.status = GameStatus::Playing;
                                if logic::shuffle(&mut game_state).is_ok() {
                                    audio.play_shuffle();
                                }
                            }

                            // New Game button: y offset 164 from dialog top
                            let new_game_y = dialog_y as i32 + 164;
                            if x >= btn_x && x < btn_x + btn_w as i32
                                && y >= new_game_y && y < new_game_y + btn_h as i32
                            {
                                let diff = game_state.difficulty; game_state = create_new_game_state_with_difficulty(diff);
                                game_state.timer.start();
                            }
                        } else if game_state.status == GameStatus::GameOver {
                            // Handle clicks on the "Game Over" dialog buttons
                            let (win_w, win_h) = renderer.window_size();
                            let dialog_w: u32 = 380;
                            let dialog_h: u32 = 374;
                            let dialog_x = (win_w.saturating_sub(dialog_w)) / 2;
                            let dialog_y = (win_h.saturating_sub(dialog_h)) / 2;

                            let btn_w: u32 = 220;
                            let btn_h: u32 = 44;
                            let btn_x = dialog_x as i32 + ((dialog_w - btn_w) / 2) as i32;

                            // Save Score button: y offset 210 from dialog top
                            let save_y = dialog_y as i32 + 210;
                            if x >= btn_x && x < btn_x + btn_w as i32
                                && y >= save_y && y < save_y + btn_h as i32
                            {
                                let score = game_state.base_score + game_state.score.live_score();
                                let total_time_ms = game_state.base_time_ms + game_state.timer.elapsed_ms;
                                let time_seconds = (total_time_ms / 1000) as u32;
                                let hints_used = game_state.base_hints + game_state.score.hints_used;
                                let shuffles_used = game_state.base_shuffles + game_state.score.shuffles_used;
                                let undos_used = game_state.base_undos + game_state.score.undos_used;
                                let shuffle_state_loaded = ShuffleState::load(&current_user_name);
                                let lb_entry = LeaderboardEntry {
                                    name: current_user_name.clone(),
                                    score,
                                    time_seconds,
                                    hints_used,
                                    shuffles_used,
                                    undos_used,
                                    difficulty: match game_state.difficulty {
                                        Difficulty::Easy => "easy".to_string(),
                                        Difficulty::Normal => "normal".to_string(),
                                    },
                                    date: current_date_string(),
                                    consecutive_days: shuffle_state_loaded.consecutive_days,
                                };
                                if !dev_mode.enabled {
                                    let mut leaderboard = Leaderboard::load(&current_user_name);
                                    leaderboard.insert(lb_entry);
                                    leaderboard.save(&current_user_name);
                                }
                                leaderboard_return_status = GameStatus::GameOver;
                                game_state.status = GameStatus::Leaderboard;
                            }

                            // New Game button: y offset 264 from dialog top
                            let new_game_y = dialog_y as i32 + 264;
                            if x >= btn_x && x < btn_x + btn_w as i32
                                && y >= new_game_y && y < new_game_y + btn_h as i32
                            {
                                let diff = game_state.difficulty; game_state = create_new_game_state_with_difficulty(diff);
                                game_state.timer.start();
                            }

                            // Wait for Shuffle button: y offset 318 from dialog top
                            let wait_y = dialog_y as i32 + 318;
                            if x >= btn_x && x < btn_x + btn_w as i32
                                && y >= wait_y && y < wait_y + btn_h as i32
                            {
                                if !dev_mode.enabled {
                                    save_current_game(&game_state, &current_user_name);
                                }
                                break 'game_loop;
                            }
                        } else if game_state.status == GameStatus::Won {
                            // Handle clicks on Victory dialog buttons
                            let (win_w, win_h) = renderer.window_size();
                            let dialog_h: u32 = if game_state.level < MAX_LEVEL { 360 } else { 300 };
                            let dialog_w: u32 = 350;
                            let dialog_x = (win_w.saturating_sub(dialog_w)) / 2;
                            let dialog_y = (win_h.saturating_sub(dialog_h)) / 2;

                            let btn_w: u32 = 220;
                            let btn_h: u32 = 44;
                            let btn_x = dialog_x as i32 + ((dialog_w - btn_w) / 2) as i32;

                            if game_state.level < MAX_LEVEL {
                                // Next Level button: y offset 155
                                let next_level_y = dialog_y as i32 + 155;
                                if x >= btn_x && x < btn_x + btn_w as i32
                                    && y >= next_level_y && y < next_level_y + btn_h as i32
                                {
                                    let next_level = game_state.level + 1;
                                    let accumulated = game_state.base_score + game_state.score.calculate_score();
                                    let accumulated_time = game_state.base_time_ms + game_state.timer.elapsed_ms;
                                    let accumulated_hints = game_state.base_hints + game_state.score.hints_used;
                                    let accumulated_shuffles = game_state.base_shuffles + game_state.score.shuffles_used;
                                    let accumulated_undos = game_state.base_undos + game_state.score.undos_used;
                                    let remaining_shuffles = game_state.shuffles_remaining + 1; // +1 shuffle reward for completing level
                                    let diff = game_state.difficulty;
                                    game_state = create_new_game_state_for_level(next_level, diff);
                                    game_state.base_score = accumulated;
                                    game_state.base_time_ms = accumulated_time;
                                    game_state.base_hints = accumulated_hints;
                                    game_state.base_shuffles = accumulated_shuffles;
                                    game_state.base_undos = accumulated_undos;
                                    game_state.shuffles_remaining = remaining_shuffles;
                                    game_state.timer.start();
                                }

                                // New Game button: y offset 215
                                let new_game_y = dialog_y as i32 + 215;
                                if x >= btn_x && x < btn_x + btn_w as i32
                                    && y >= new_game_y && y < new_game_y + btn_h as i32
                                {
                                    let diff = game_state.difficulty; game_state = create_new_game_state_with_difficulty(diff);
                                    game_state.timer.start();
                                }

                                // Leaderboard button: y offset 275
                                let lb_y = dialog_y as i32 + 275;
                                if x >= btn_x && x < btn_x + btn_w as i32
                                    && y >= lb_y && y < lb_y + btn_h as i32
                                {
                                    leaderboard_return_status = GameStatus::Won;
                                    game_state.status = GameStatus::Leaderboard;
                                }
                            } else {
                                // New Game button: y offset 160
                                let new_game_y = dialog_y as i32 + 160;
                                if x >= btn_x && x < btn_x + btn_w as i32
                                    && y >= new_game_y && y < new_game_y + btn_h as i32
                                {
                                    let diff = game_state.difficulty; game_state = create_new_game_state_with_difficulty(diff);
                                    game_state.timer.start();
                                }

                                // Leaderboard button: y offset 220
                                let lb_y = dialog_y as i32 + 220;
                                if x >= btn_x && x < btn_x + btn_w as i32
                                    && y >= lb_y && y < lb_y + btn_h as i32
                                {
                                    leaderboard_return_status = GameStatus::Won;
                                    game_state.status = GameStatus::Leaderboard;
                                }
                            }
                        } else if game_state.status == GameStatus::Leaderboard {
                            // Handle clicks on Achievements dialog (Back button)
                            let (win_w, win_h) = renderer.window_size();
                            let dialog_w: u32 = 900;
                            let dialog_h: u32 = 620;
                            let dialog_x = (win_w.saturating_sub(dialog_w)) / 2;
                            let dialog_y = (win_h.saturating_sub(dialog_h)) / 2;

                            let btn_w: u32 = 180;
                            let btn_h: u32 = 40;
                            let btn_x = dialog_x as i32 + ((dialog_w - btn_w) / 2) as i32;
                            let btn_y = dialog_y as i32 + dialog_h as i32 - 50;

                            if x >= btn_x && x < btn_x + btn_w as i32
                                && y >= btn_y && y < btn_y + btn_h as i32
                            {
                                game_state.status = leaderboard_return_status;
                            }
                        } else if game_state.status == GameStatus::Shortcuts {
                            // Handle clicks on Shortcuts dialog (Back button)
                            let (win_w, win_h) = renderer.window_size();
                            let dialog_w: u32 = 420;
                            let dialog_h: u32 = 420;
                            let dialog_x = (win_w.saturating_sub(dialog_w)) / 2;
                            let dialog_y = (win_h.saturating_sub(dialog_h)) / 2;

                            let btn_w: u32 = 180;
                            let btn_h: u32 = 40;
                            let btn_x = dialog_x as i32 + ((dialog_w - btn_w) / 2) as i32;
                            let btn_y = dialog_y as i32 + dialog_h as i32 - 55;

                            if x >= btn_x && x < btn_x + btn_w as i32
                                && y >= btn_y && y < btn_y + btn_h as i32
                            {
                                game_state.status = GameStatus::Paused;
                            }
                        } else if game_state.status == GameStatus::LevelSelect {
                            // Handle clicks on Level Select dialog
                            let (win_w, win_h) = renderer.window_size();
                            let dialog_w: u32 = 720;
                            let dialog_h: u32 = 590;
                            let dialog_x = (win_w.saturating_sub(dialog_w)) / 2;
                            let dialog_y = (win_h.saturating_sub(dialog_h)) / 2;

                            // Phase jump tabs (y in dialog_y + 78 .. dialog_y + 106)
                            let tab_w: u32 = 124;
                            let tab_h: u32 = 28;
                            let tab_gap: i32 = 8;
                            let tabs_total_w = (tab_w as i32 * 5) + (tab_gap * 4);
                            let tab_start_x = dialog_x as i32 + (dialog_w as i32 - tabs_total_w) / 2;
                            let tab_y = dialog_y as i32 + 78;

                            if y >= tab_y && y < tab_y + tab_h as i32 {
                                for (i, target_level) in [1u32, 11, 21, 51, 101].iter().enumerate() {
                                    let tx = tab_start_x + i as i32 * (tab_w as i32 + tab_gap);
                                    if x >= tx && x < tx + tab_w as i32 {
                                        level_select_chosen = *target_level;
                                        level_select_page = ((*target_level - 1) / 25) as usize;
                                        break;
                                    }
                                }
                            }

                            // 5x5 Grid of levels
                            let grid_cols = 5;
                            let grid_rows = 5;
                            let cell_w: u32 = 116;
                            let cell_h: u32 = 54;
                            let cell_gap_x: i32 = 12;
                            let cell_gap_y: i32 = 10;
                            let grid_total_w = (cell_w as i32 * grid_cols) + (cell_gap_x * (grid_cols - 1));
                            let grid_start_x = dialog_x as i32 + (dialog_w as i32 - grid_total_w) / 2;
                            let grid_start_y = dialog_y as i32 + 116;

                            let page_start_level = (level_select_page * 25 + 1) as u32;

                            for row in 0..grid_rows {
                                for col in 0..grid_cols {
                                    let idx = row * grid_cols + col;
                                    let level = page_start_level + idx as u32;
                                    if level > 1000 {
                                        continue;
                                    }
                                    let cx = grid_start_x + col * (cell_w as i32 + cell_gap_x);
                                    let cy = grid_start_y + row * (cell_h as i32 + cell_gap_y);

                                    if x >= cx && x < cx + cell_w as i32
                                        && y >= cy && y < cy + cell_h as i32
                                    {
                                        level_select_chosen = level;
                                        let user_progress = UserProgress::load_and_sync(&current_user_name, game_state.level);
                                        if user_progress.is_level_unlocked(level) {
                                            let diff = game_state.difficulty;
                                            game_state = create_new_game_state_for_level(level, diff);
                                            game_state.timer.start();
                                            game_state.status = GameStatus::Playing;
                                            quit_confirmation = false;
                                            last_activity_time = Instant::now();
                                            show_hint_suggestion = false;
                                        }
                                    }
                                }
                            }

                            // Bottom Navigation: PREV button
                            let nav_y = dialog_y as i32 + 444;
                            let nav_btn_w: u32 = 130;
                            let nav_btn_h: u32 = 38;
                            let prev_x = grid_start_x;
                            if x >= prev_x && x < prev_x + nav_btn_w as i32
                                && y >= nav_y && y < nav_y + nav_btn_h as i32
                            {
                                if level_select_page > 0 {
                                    level_select_page -= 1;
                                    level_select_chosen = (level_select_page * 25 + 1) as u32;
                                }
                            }

                            // NEXT button
                            let next_x = grid_start_x + grid_total_w - nav_btn_w as i32;
                            if x >= next_x && x < next_x + nav_btn_w as i32
                                && y >= nav_y && y < nav_y + nav_btn_h as i32
                            {
                                if level_select_page + 1 < 40 {
                                    level_select_page += 1;
                                    level_select_chosen = (level_select_page * 25 + 1) as u32;
                                }
                            }

                            // BACK button
                            let back_w: u32 = 160;
                            let back_h: u32 = 40;
                            let back_x = dialog_x as i32 + (dialog_w as i32 - back_w as i32) / 2;
                            let back_y = dialog_y as i32 + 494;
                            if x >= back_x && x < back_x + back_w as i32
                                && y >= back_y && y < back_y + back_h as i32
                            {
                                game_state.status = GameStatus::Paused;
                            }
                        }
                    }

                    GameAction::NewGame => {
                        // Clear magnification state on new game
                        press_start = None;
                        magnified_tile = None;
                        let diff = game_state.difficulty; game_state = create_new_game_state_with_difficulty(diff);
                        game_state.timer.start();
                        quit_confirmation = false;
                        last_activity_time = Instant::now();
                        show_hint_suggestion = false;
                    }

                    GameAction::Undo => {
                        if game_state.status == GameStatus::Playing {
                            let _ = logic::undo(&mut game_state);
                        }
                    }

                    GameAction::Hint => {
                        if game_state.status == GameStatus::Playing {
                            let result = logic::request_hint(&mut game_state);
                            if let HintResult::NoMatchesAvailable = result {
                                // No moves available
                                if game_state.shuffles_remaining == 0 {
                                    game_state.timer.pause();
                                    game_state.score.elapsed_seconds = game_state.timer.elapsed_seconds();
                                    game_state.status = GameStatus::GameOver;
                                } else {
                                    game_state.status = GameStatus::Lost;
                                }
                            }
                        }
                    }

                    GameAction::Shuffle => {
                        if game_state.status == GameStatus::Playing
                            || game_state.status == GameStatus::Lost
                        {
                            if game_state.status == GameStatus::Lost {
                                game_state.status = GameStatus::Playing;
                            }
                            match logic::shuffle(&mut game_state) {
                                Ok(()) => {
                                    audio.play_shuffle();
                                }
                                Err(logic::ShuffleError::NoShufflesRemaining) => {
                                    // Play error sound to indicate no shuffles left
                                    audio.play_error();
                                }
                                Err(logic::ShuffleError::NoValidArrangement) => {
                                    // Extremely rare: couldn't find a valid arrangement
                                    audio.play_error();
                                }
                            }
                        }
                    }

                    GameAction::PauseMenu => {
                        if game_state.status == GameStatus::Playing {
                            game_state.status = GameStatus::Paused;
                            game_state.timer.pause();
                            pause_menu_selection = 0;
                        }
                    }

                    GameAction::Resume => {
                        if game_state.status == GameStatus::Paused {
                            game_state.status = GameStatus::Playing;
                            game_state.timer.resume();
                            last_activity_time = Instant::now();
                            show_hint_suggestion = false;
                        }
                    }

                    GameAction::Save => {
                        if !dev_mode.enabled {
                            if game_state.status == GameStatus::Playing
                                || game_state.status == GameStatus::Paused
                            {
                                save_current_game(&game_state, &current_user_name);
                            }
                        }
                    }

                    GameAction::SaveQuit => {
                        if game_state.status == GameStatus::Playing
                            || game_state.status == GameStatus::Paused
                        {
                            if !dev_mode.enabled {
                                save_current_game(&game_state, &current_user_name);
                            }
                            break 'game_loop;
                        } else {
                            break 'game_loop;
                        }
                    }

                    GameAction::ToggleMute => {
                        audio.toggle_mute();
                        settings.muted = audio.is_muted();
                        if !dev_mode.enabled {
                            settings.save(&current_user_name);
                        }
                    }

                    GameAction::ToggleFullscreen => {
                        toggle_fullscreen(&mut renderer);
                    }

                    GameAction::Quit => {
                        if game_state.status == GameStatus::Playing {
                            // Show confirmation when actively playing
                            quit_confirmation = true;
                        } else {
                            break 'game_loop;
                        }
                    }
                }
            }
        }

        // --- 7c. Update timer ---
        if game_state.status == GameStatus::Playing {
            game_state.timer.update();
        }

        // --- 7d. Expire completed animations ---
        expire_animations(&mut game_state);

        // --- 7d2. Check for long-press tile magnification ---
        // If the mouse/touch is no longer held, clear the magnification state.
        if press_start.is_some() {
            let mouse = event_pump.mouse_state();
            if !mouse.left() {
                // Button released — clear magnification
                press_start = None;
                magnified_tile = None;
            }
        }
        if let Some((press_time, px, py)) = press_start {
            if press_time.elapsed().as_millis() >= MAGNIFY_HOLD_MS && magnified_tile.is_none() {
                // Long-press threshold reached — magnify the tile under the press position
                if game_state.status == GameStatus::Playing {
                    let (win_w, win_h) = renderer.window_size();
                    let metrics = renderer::compute_layout_rect(win_w, win_h);
                    if let Some(idx) = renderer::hit_test(px, py, &game_state, &metrics) {
                        magnified_tile = Some(idx);
                    }
                }
            }
        }

        // --- 7e. Auto-dismiss hint after 3 seconds ---
        if let Some(ref hint) = game_state.hint {
            if hint.activated_at.elapsed() >= Duration::from_secs(HINT_DISMISS_SECS) {
                game_state.hint = None;
            }
        }

        // --- 7f. Render based on GameStatus ---
        renderer.clear();

        let (win_w, win_h) = renderer.window_size();
        let metrics = renderer::compute_layout_rect(win_w, win_h);
        let layout_rect = Rect::new(
            metrics.offset_x as i32,
            metrics.offset_y as i32,
            metrics.layout_w as u32,
            metrics.layout_h as u32,
        );

        match game_state.status {
            GameStatus::Playing => {
                renderer.render_board(&game_state, layout_rect);
                renderer.render_hud(&game_state);
                let _ = renderer.render_menu_button();

                // Show hint suggestion after inactivity
                if !show_hint_suggestion
                    && last_activity_time.elapsed() >= Duration::from_secs(INACTIVITY_HINT_SECS)
                {
                    show_hint_suggestion = true;
                }
                if show_hint_suggestion {
                    renderer.render_hint_suggestion();
                }
            }
            GameStatus::Paused => {
                renderer.render_board(&game_state, layout_rect);
                renderer.render_hud(&game_state);
                let diff_str = match game_state.difficulty {
                    Difficulty::Easy => "EASY",
                    Difficulty::Normal => "NORMAL",
                };
                renderer.render_menu(pause_menu_selection, diff_str);
            }
            GameStatus::Won => {
                renderer.render_board(&game_state, layout_rect);
                let time_str = game_state.timer.format_display();
                let score = game_state.base_score + game_state.score.calculate_score();
                renderer.render_victory(&time_str, score, game_state.level, victory_menu_selection);
            }
            GameStatus::Lost => {
                renderer.render_board(&game_state, layout_rect);
                renderer.render_no_moves(lost_menu_selection);
            }
            GameStatus::GameOver => {
                renderer.render_board(&game_state, layout_rect);
                let score = game_state.base_score + game_state.score.live_score();
                let total_time_ms = game_state.base_time_ms + game_state.timer.elapsed_ms;
                let time_seconds = (total_time_ms / 1000) as u32;
                let hints_used = game_state.base_hints + game_state.score.hints_used;
                let shuffles_used = game_state.base_shuffles + game_state.score.shuffles_used;
                renderer.render_game_over(score, time_seconds, hints_used, shuffles_used, game_state.level, game_over_menu_selection);
            }
            GameStatus::Menu => {
                let diff_str = match game_state.difficulty {
                    Difficulty::Easy => "EASY",
                    Difficulty::Normal => "NORMAL",
                };
                renderer.render_menu(pause_menu_selection, diff_str);
            }
            GameStatus::NameEntry => {
                renderer.render_board(&game_state, layout_rect);
                if let Some(ref entry) = name_entry {
                    renderer.render_name_entry(&entry.text, entry.score, entry.time_seconds);
                }
            }
            GameStatus::Leaderboard => {
                renderer.render_board(&game_state, layout_rect);
                renderer.render_leaderboard(&current_user_name);
            }
            GameStatus::Shortcuts => {
                renderer.render_board(&game_state, layout_rect);
                renderer.render_shortcuts();
            }
            GameStatus::LevelSelect => {
                renderer.render_board(&game_state, layout_rect);
                let user_progress = UserProgress::load_and_sync(&current_user_name, game_state.level);
                renderer.render_level_select(&current_user_name, &user_progress, level_select_page, level_select_chosen);
            }
        }

        // --- Render magnified tile overlay (long-press zoom) ---
        if let Some(tile_idx) = magnified_tile {
            if let Some(tile) = game_state.board.tiles[tile_idx] {
                renderer.render_magnified_tile(tile.face_id);
            }
        }

        if let Some(streak) = daily_streak_achievement {
            renderer.render_daily_streak_popup(streak);
        } else if quit_confirmation {
            renderer.render_quit_confirmation();
        }

        if let Some(ref info) = update_info {
            renderer.render_update_dialog(CURRENT_VERSION, &info.latest_version);
        }

        // --- 7g. Present frame ---
        renderer.present();

        // --- 7h. Cap to ~60 FPS using SDL2 delay ---
        let frame_elapsed = frame_start.elapsed();
        let target_duration = Duration::from_millis(FRAME_DURATION_MS);
        if frame_elapsed < target_duration {
            std::thread::sleep(target_duration - frame_elapsed);
        }
    }
}

/// Handles a tile selection click at screen coordinates (x, y).
/// Returns `true` if the game was just won (so the caller can check leaderboard).
fn handle_select_tile(
    state: &mut GameState,
    audio: &mut AudioManager,
    renderer: &Renderer,
    x: i32,
    y: i32,
    user_name: &str,
    dev_mode_enabled: bool,
) -> bool {
    // Hit-test to find which tile was clicked
    let (win_w, win_h) = renderer.window_size();
    let metrics = renderer::compute_layout_rect(win_w, win_h);

    let pos = match renderer::hit_test(x, y, state, &metrics) {
        Some(idx) => idx,
        None => return false, // Click didn't hit any tile
    };

    // Process tile selection through game logic
    let result = logic::select_tile(state, pos);

    match result {
        SelectionResult::Matched(pos_a, pos_b) => {
            audio.play_match();

            let face_id = state.undo_stack.last().map(|entry| entry.tile_a.face_id).unwrap_or(0);
            state.animations.push(xmahjong::game_state::Animation::TileRemoval {
                positions: (pos_a, pos_b),
                face_id,
                start_time: Instant::now(),
                duration_ms: 600,
            });
            state.animations.push(xmahjong::game_state::Animation::Lightning {
                positions: (pos_a, pos_b),
                start_time: Instant::now(),
                duration_ms: 600,
            });

            // Check for win condition after a successful match
            // (No-moves detection is deferred until the player requests a Hint)
            if let Some(GameOverReason::Won) = logic::check_game_over(state) {
                state.timer.pause();
                state.score.elapsed_seconds = state.timer.elapsed_seconds();
                state.status = GameStatus::Won;
                audio.play_victory();

                // Check and award repeatable trophies
                let mut trophies = TrophyState::load(user_name);
                trophies.check_perfect_combo(state.score.mismatches);
                let level_elapsed = state.timer.elapsed_seconds();
                trophies.check_rapid_clear(state.level, level_elapsed);
                trophies.check_no_hints(state.score.hints_used);
                trophies.check_no_shuffles(state.score.shuffles_used);
                trophies.check_no_undos(state.score.undos_used);
                if !dev_mode_enabled {
                    let mut progress = UserProgress::load(user_name);
                    progress.mark_completed(state.level);
                    progress.save(user_name);
                    trophies.save(user_name);
                }

                return true;
            }
        }
        SelectionResult::Mismatched(pos_a, pos_b) => {
            audio.play_error();
            state.score.mismatches += 1;
            state.animations.push(xmahjong::game_state::Animation::TileMismatch {
                positions: (pos_a, pos_b),
                start_time: Instant::now(),
                duration_ms: 500,
            });
        }
        SelectionResult::Selected
        | SelectionResult::Deselected
        | SelectionResult::Ignored => {}
    }

    false
}

// Level system functions are defined in the library crate for testability.
use xmahjong::levels::{tiles_for_level, face_pool_for_level};

/// Creates a new GameState with a freshly generated board for the given level and difficulty.
fn create_new_game_state_for_level(level: u32, difficulty: Difficulty) -> GameState {
    let layout = turtle_layout();
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;

    let tile_count = tiles_for_level(level);
    let face_pool = face_pool_for_level(level);
    let mut generator = BoardGenerator::new(seed);

    let board = if level <= 10 {
        // Levels 1-10: use standard generation (penguin faces only)
        if tile_count < 144 {
            generator
                .generate_with_tile_count(layout, tile_count, 10)
                .expect("Failed to generate board after 10 attempts")
        } else {
            generator
                .generate(layout, 5)
                .expect("Failed to generate board after 5 attempts")
        }
    } else {
        // Levels 11-1000: use custom face pool (penguins + dogs + space + ocean)
        generator
            .generate_with_faces(layout, tile_count, &face_pool, 10)
            .expect("Failed to generate board after 10 attempts")
    };

    GameState {
        board,
        timer: GameTimer::new(),
        score: ScoreTracker::new(),
        status: GameStatus::Playing,
        selection: None,
        hint: None,
        undo_stack: Vec::new(),
        shuffles_remaining: 1,
        level,
        base_score: 0,
        base_time_ms: 0,
        base_hints: 0,
        base_shuffles: 0,
        base_undos: 0,
        animations: Vec::new(),
        difficulty,
    }
}

/// Creates a new GameState with a freshly generated board (level 1).
fn create_new_game_state() -> GameState {
    create_new_game_state_for_level(1, Difficulty::Easy)
}

/// Creates a new GameState with a freshly generated board (level 1) with specified difficulty.
fn create_new_game_state_with_difficulty(difficulty: Difficulty) -> GameState {
    create_new_game_state_for_level(1, difficulty)
}

/// Removes completed animations from the game state.
fn expire_animations(state: &mut GameState) {
    let now = Instant::now();
    state.animations.retain(|anim| {
        match anim {
            xmahjong::game_state::Animation::TileRemoval {
                start_time,
                duration_ms,
                ..
            } => {
                let elapsed = now.duration_since(*start_time).as_millis() as u32;
                elapsed < *duration_ms
            }
            xmahjong::game_state::Animation::Lightning {
                start_time,
                duration_ms,
                ..
            } => {
                let elapsed = now.duration_since(*start_time).as_millis() as u32;
                elapsed < *duration_ms
            }
            xmahjong::game_state::Animation::TileMismatch {
                start_time,
                duration_ms,
                ..
            } => {
                let elapsed = now.duration_since(*start_time).as_millis() as u32;
                elapsed < *duration_ms
            }
            xmahjong::game_state::Animation::HintPulse { start_time, .. } => {
                // Keep hint pulse active while hint is displayed (managed by hint auto-dismiss)
                let elapsed = now.duration_since(*start_time).as_secs();
                elapsed < HINT_DISMISS_SECS
            }
            xmahjong::game_state::Animation::Shuffle {
                start_time,
                duration_ms,
                ..
            } => {
                let elapsed = now.duration_since(*start_time).as_millis() as u32;
                elapsed < *duration_ms
            }
        }
    });
}

/// Toggles fullscreen mode on the renderer's window.
/// Saves the window size and position before entering fullscreen,
/// and restores them when returning to windowed mode.
fn toggle_fullscreen(renderer: &mut Renderer) {
    use std::cell::Cell;
    use sdl2::video::FullscreenType;

    // Thread-local storage for windowed geometry (x, y, width, height).
    // Safe because the game loop is single-threaded.
    thread_local! {
        static WINDOWED_GEOMETRY: Cell<Option<(i32, i32, u32, u32)>> = const { Cell::new(None) };
    }

    let window = renderer.canvas.window_mut();
    let current = window.fullscreen_state();

    let new_state = match current {
        FullscreenType::Off => {
            // Save current windowed geometry before going fullscreen
            let (x, y) = window.position();
            let (w, h) = window.size();
            WINDOWED_GEOMETRY.with(|geo| geo.set(Some((x, y, w, h))));
            FullscreenType::Desktop
        }
        _ => FullscreenType::Off,
    };

    if let Err(e) = window.set_fullscreen(new_state) {
        eprintln!("[xMahjong] Warning: Failed to toggle fullscreen: {}", e);
        // Graceful fallback: remain in current mode, no crash
        return;
    }

    // If we just exited fullscreen, restore previous window size and position
    if new_state == FullscreenType::Off {
        let saved = WINDOWED_GEOMETRY.with(|geo| geo.take());
        if let Some((x, y, w, h)) = saved {
            let window = renderer.canvas.window_mut();
            window.set_size(w, h).ok();
            window.set_position(
                sdl2::video::WindowPos::Positioned(x),
                sdl2::video::WindowPos::Positioned(y),
            );
        }
    }
}

/// Returns the current date as an ISO 8601 date string (YYYY-MM-DD).
fn current_date_string() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Simple date calculation (no external crate needed)
    let days = now / 86400;
    let (year, month, day) = days_to_date(days);
    format!("{:04}-{:02}-{:02}", year, month, day)
}

/// Converts days since Unix epoch to (year, month, day).
fn days_to_date(days_since_epoch: u64) -> (u32, u32, u32) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days_since_epoch + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as u32, m as u32, d as u32)
}

/// Saves the current game state to disk so it can be resumed later.
fn save_current_game(state: &GameState, user_name: &str) {
    let tiles: Vec<Option<u8>> = state.board.tiles.iter().map(|t| t.map(|tile| tile.face_id)).collect();

    let undo_stack: Vec<(usize, u8, usize, u8)> = state.undo_stack.iter().map(|entry| {
        (entry.position_a, entry.tile_a.face_id, entry.position_b, entry.tile_b.face_id)
    }).collect();

    let saved = SavedGame {
        tiles,
        undo_stack,
        elapsed_ms: state.timer.elapsed_ms
            + state.timer.last_tick.map(|t| t.elapsed().as_millis() as u64).unwrap_or(0),
        hints_used: state.score.hints_used,
        shuffles_used: state.score.shuffles_used,
        shuffles_remaining: state.shuffles_remaining,
        pairs_matched: state.score.pairs_matched,
        undos_used: state.score.undos_used,
        level: state.level,
        base_score: state.base_score,
        base_time_ms: state.base_time_ms,
        base_hints: state.base_hints,
        base_shuffles: state.base_shuffles,
        base_undos: state.base_undos,
        difficulty: match state.difficulty {
            Difficulty::Easy => "easy".to_string(),
            Difficulty::Normal => "normal".to_string(),
        },
    };

    saved.save(user_name);
}

/// Loads a saved game from disk for a specific user and reconstructs the GameState.
/// Returns None if loading fails.
fn load_saved_game(user_name: &str) -> Option<GameState> {
    use xmahjong::board::{Board, Tile, turtle_layout};
    use xmahjong::logic::UndoEntry;

    let saved = SavedGame::load(user_name)?;
    let layout = turtle_layout();

    // Validate tile count matches layout
    if saved.tiles.len() != layout.positions.len() {
        eprintln!("[xMahjong] Save file has wrong tile count, ignoring.");
        return None;
    }

    // Reconstruct board
    let mut board = Board::new(layout);
    for (idx, maybe_face) in saved.tiles.iter().enumerate() {
        if let Some(face_id) = maybe_face {
            board.tiles[idx] = Some(Tile {
                face_id: *face_id,
                position: layout.positions[idx],
            });
        }
    }

    // Reconstruct undo stack
    let undo_stack: Vec<UndoEntry> = saved.undo_stack.iter().map(|&(pos_a, face_a, pos_b, face_b)| {
        UndoEntry {
            tile_a: Tile { face_id: face_a, position: layout.positions[pos_a] },
            tile_b: Tile { face_id: face_b, position: layout.positions[pos_b] },
            position_a: pos_a,
            position_b: pos_b,
        }
    }).collect();

    // Reconstruct timer (paused state, will be resumed on start)
    let mut timer = GameTimer::new();
    timer.elapsed_ms = saved.elapsed_ms;
    // Timer is stopped; it will resume when game loop starts

    let difficulty = match saved.difficulty.as_str() {
        "normal" => Difficulty::Normal,
        _ => Difficulty::Easy,
    };

    let state = GameState {
        board,
        timer,
        score: ScoreTracker {
            hints_used: saved.hints_used,
            shuffles_used: saved.shuffles_used,
            undos_used: saved.undos_used,
            elapsed_seconds: 0, // Only used at game end
            pairs_matched: saved.pairs_matched,
            mismatches: 0,
        },
        status: GameStatus::Playing,
        selection: None,
        hint: None,
        undo_stack,
        shuffles_remaining: saved.shuffles_remaining,
        level: saved.level,
        base_score: saved.base_score,
        base_time_ms: saved.base_time_ms,
        base_hints: saved.base_hints,
        base_shuffles: saved.base_shuffles,
        base_undos: saved.base_undos,
        animations: Vec::new(),
        difficulty,
    };

    Some(state)
}
