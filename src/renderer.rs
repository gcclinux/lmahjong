//! Renderer module.
//!
//! Handles SDL2 window creation, tile rendering with depth effects,
//! UI overlays, animations, and layout scaling.

use std::time::Instant;

use sdl2::image::{LoadTexture, LoadSurface};
use sdl2::pixels::Color;
use sdl2::rect::{Point, Rect};
use sdl2::render::{Canvas, Texture, TextureCreator};
use sdl2::ttf::Sdl2TtfContext;
use sdl2::video::{Window, WindowContext};

use crate::board::TilePosition;
use crate::game_state::{Animation, GameState};
use crate::storage::{Leaderboard, ShuffleState, TrophyState};

/// Number of distinct tile face images per style.
const TILE_FACE_COUNT: usize = 50;

/// Total number of tile face textures (penguins + dogs + space).
const TOTAL_FACE_COUNT: usize = 150;

/// Default window width in pixels.
#[cfg(target_os = "macos")]
const DEFAULT_WIDTH: u32 = 1900;

/// Default window width in pixels.
#[cfg(target_os = "windows")]
const DEFAULT_WIDTH: u32 = 1280;

/// Default window width in pixels.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const DEFAULT_WIDTH: u32 = 1920;

/// Default window height in pixels.
#[cfg(target_os = "macos")]
const DEFAULT_HEIGHT: u32 = 900;

/// Default window height in pixels.
#[cfg(target_os = "windows")]
const DEFAULT_HEIGHT: u32 = 720;

/// Default window height in pixels.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const DEFAULT_HEIGHT: u32 = 1080;

/// Minimum window width in pixels.
#[cfg(target_os = "macos")]
const MIN_WIDTH: u32 = 800;

/// Minimum window width in pixels.
#[cfg(target_os = "windows")]
const MIN_WIDTH: u32 = 800;

/// Minimum window width in pixels.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const MIN_WIDTH: u32 = 800;

/// Minimum window height in pixels.
#[cfg(target_os = "macos")]
const MIN_HEIGHT: u32 = 600;

/// Minimum window height in pixels.
#[cfg(target_os = "windows")]
const MIN_HEIGHT: u32 = 600;

/// Minimum window height in pixels.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const MIN_HEIGHT: u32 = 600;

/// Resolves the assets directory path at runtime.
/// Checks in order:
/// 1. `$SNAP/assets` (Snap package)
/// 2. `./assets` (development / cargo run)
/// 3. Relative to the executable:
///    a. `<exe_dir>/assets`
///    b. `<exe_dir>/../../assets` (cargo target/debug or target/release)
///    c. `<exe_dir>/../Resources/assets` (macOS .app bundle)
///    d. `<exe_dir>/../share/xmahjong/assets` (FHS layout)
/// 4. `/usr/share/xmahjong/assets` (system install via .deb/.rpm)
/// 5. `/usr/local/share/xmahjong/assets` (manual install)
/// Falls back to "assets" (original behavior) if none found.
fn assets_path() -> String {
    // Snap environment
    if let Ok(snap) = std::env::var("SNAP") {
        let p = format!("{}/assets", snap);
        if std::path::Path::new(&p).is_dir() {
            return p;
        }
    }

    // Current working directory
    if std::path::Path::new("assets").is_dir() {
        return "assets".to_string();
    }

    // Relative to executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("assets");
            if p.is_dir() {
                return p.to_string_lossy().into_owned();
            }
            // Check ../../assets (covers target/debug/ or target/release/ → project root)
            let p = dir.join("../../assets");
            if p.is_dir() {
                return p.canonicalize()
                    .unwrap_or(p)
                    .to_string_lossy()
                    .into_owned();
            }
            // macOS .app bundle: Contents/MacOS/../Resources/assets
            let p = dir.join("../Resources/assets");
            if p.is_dir() {
                return p.canonicalize()
                    .unwrap_or(p)
                    .to_string_lossy()
                    .into_owned();
            }
            // Also check ../share/xmahjong/assets (FHS layout: /usr/bin/../share/...)
            let p = dir.join("../share/xmahjong/assets");
            if p.is_dir() {
                return p.canonicalize()
                    .unwrap_or(p)
                    .to_string_lossy()
                    .into_owned();
            }
        }
    }

    // Standard system paths
    for prefix in &["/usr/share/xmahjong/assets", "/usr/local/share/xmahjong/assets"] {
        if std::path::Path::new(prefix).is_dir() {
            return prefix.to_string();
        }
    }

    // Fallback
    "assets".to_string()
}

/// Base thickness in pixels at 1920×1080 reference resolution.
#[allow(dead_code)]
const BASE_THICKNESS_PX: f32 = 7.0;

/// Reference tile_width at 1920×1080 for scaling calculations.
/// At 1920×1080: tile_width = 1920.0 / 28.0 ≈ 68.57
#[allow(dead_code)]
const REFERENCE_TILE_WIDTH: f32 = 1920.0 / 28.0;

/// Brightness factor for the right side face (darker, further from light).
#[allow(dead_code)]
const RIGHT_FACE_BRIGHTNESS: f32 = 0.70;

/// Brightness factor for the bottom side face (slightly lighter than right).
#[allow(dead_code)]
const BOTTOM_FACE_BRIGHTNESS: f32 = 0.80;

/// Gradient brightness delta across side face width/height.
#[allow(dead_code)]
const SIDE_FACE_GRADIENT_DELTA: f32 = 0.07;

/// Base shadow offset in pixels at 1920×1080 resolution.
const BASE_SHADOW_OFFSET_PX: f32 = 3.0;

/// Shadow alpha value (0-255 scale).
const SHADOW_ALPHA: u8 = 80;

/// Duration of tile removal animation in milliseconds.
#[allow(dead_code)]
const REMOVAL_DURATION_MS: u32 = 300;

/// Duration of tile mismatch flash animation in milliseconds.
#[allow(dead_code)]
const MISMATCH_DURATION_MS: u32 = 500;

/// Duration of shuffle animation in milliseconds.
#[allow(dead_code)]
const SHUFFLE_DURATION_MS: u32 = 500;

/// Duration of hint pulse cycle in milliseconds.
const HINT_PULSE_CYCLE_MS: u32 = 1000;

/// The natural width of the Turtle layout in grid units.
/// Max col (26) + tile width (2) = 28.
pub const LAYOUT_GRID_WIDTH: f32 = 28.0;

/// The natural height of the Turtle layout in grid units.
/// Max row (12) + tile height (2) = 14.
pub const LAYOUT_GRID_HEIGHT: f32 = 14.0;

/// Layout scaling metrics computed for a given window size.
///
/// These describe how to map tile grid coordinates to screen pixel coordinates,
/// maintaining the layout's aspect ratio and centering within the window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutMetrics {
    /// Horizontal offset (in pixels) from the left edge of the window to the layout area.
    pub offset_x: f32,
    /// Vertical offset (in pixels) from the top edge of the window to the layout area.
    pub offset_y: f32,
    /// Width of one grid unit in pixels.
    pub tile_width: f32,
    /// Height of one grid unit in pixels.
    pub tile_height: f32,
    /// Total width of the layout area in pixels.
    pub layout_w: f32,
    /// Total height of the layout area in pixels.
    pub layout_h: f32,
}

/// Height of the HUD bar in pixels (timer, score, shuffle display).
pub const HUD_BAR_HEIGHT: u32 = 40;

/// Computes the layout rectangle that fits within the window while maintaining aspect ratio.
///
/// The layout is scaled to be as large as possible without exceeding the window bounds,
/// then centered. Margins are filled with the background.
/// On macOS, the layout is shifted down to avoid overlapping the HUD bar.
///
/// # Arguments
/// * `window_width` - Current window width in pixels
/// * `window_height` - Current window height in pixels
///
/// # Returns
/// A `LayoutMetrics` struct containing offset, scale, and dimension information.
pub fn compute_layout_rect(window_width: u32, window_height: u32) -> LayoutMetrics {
    let aspect_ratio = LAYOUT_GRID_WIDTH / LAYOUT_GRID_HEIGHT;

    // Reserve space for the HUD bar at the top
    let available_height = window_height.saturating_sub(HUD_BAR_HEIGHT);

    let window_aspect = window_width as f32 / available_height as f32;

    let (layout_w, layout_h) = if window_aspect > aspect_ratio {
        // Window is wider than layout — height-constrained
        let h = available_height as f32;
        let w = h * aspect_ratio;
        (w, h)
    } else {
        // Window is taller than layout — width-constrained
        let w = window_width as f32;
        let h = w / aspect_ratio;
        (w, h)
    };

    let offset_x = (window_width as f32 - layout_w) / 2.0;

    // Position layout below the HUD bar, centered in the remaining space
    let offset_y = HUD_BAR_HEIGHT as f32 + (available_height as f32 - layout_h) / 2.0;

    let tile_width = layout_w / LAYOUT_GRID_WIDTH;
    let tile_height = layout_h / LAYOUT_GRID_HEIGHT;

    LayoutMetrics {
        offset_x,
        offset_y,
        tile_width,
        tile_height,
        layout_w,
        layout_h,
    }
}

/// Computes the current tile thickness in pixels based on window size.
///
/// Thickness scales proportionally to the tile width relative to the reference 1920×1080 resolution.
/// The result is always at least 1 pixel, even for extremely small windows.
///
/// # Arguments
/// * `metrics` - Precomputed layout scaling metrics for the current window size
///
/// # Returns
/// The thickness in pixels (minimum 1).
pub fn compute_thickness(metrics: &LayoutMetrics) -> u32 {
    let raw = BASE_THICKNESS_PX * metrics.tile_width / REFERENCE_TILE_WIDTH;
    (raw.round() as u32).max(1)
}

/// Computes the screen rectangle for a tile at the given position using the layout metrics.
///
/// Each tile occupies a 2×2 area in grid space. Higher layers are shifted slightly
/// up-left to create a depth/stacking effect (thickness per layer for both X and Y).
///
/// # Arguments
/// * `pos` - The tile's position in grid coordinates (layer, row, col)
/// * `metrics` - Precomputed layout scaling metrics
///
/// # Returns
/// An SDL2 `Rect` representing the tile's screen position and size.
pub fn tile_screen_rect(pos: &TilePosition, metrics: &LayoutMetrics) -> Rect {
    let thickness = compute_thickness(metrics) as i32;
    let layer_offset = -(pos.layer as i32 * thickness);

    let x = metrics.offset_x + (pos.col as f32 * metrics.tile_width) + layer_offset as f32;
    let y = metrics.offset_y + (pos.row as f32 * metrics.tile_height) + layer_offset as f32;

    // Each tile occupies 2 grid units in width and height
    let w = (2.0 * metrics.tile_width) as u32;
    let h = (2.0 * metrics.tile_height) as u32;

    Rect::new(x.floor() as i32, y.floor() as i32, w, h)
}

/// Given screen coordinates, finds which tile position was clicked.
///
/// Checks from the top layer (4) down to the bottom (0) because higher tiles
/// visually occlude lower ones. Only considers tiles that are currently present
/// on the board (not removed).
///
/// # Arguments
/// * `x` - Screen x coordinate (e.g., from mouse click)
/// * `y` - Screen y coordinate (e.g., from mouse click)
/// * `state` - The current game state (to check tile presence)
/// * `metrics` - Precomputed layout scaling metrics
///
/// # Returns
/// `Some(index)` of the topmost tile at the click position, or `None` if no tile was hit.
pub fn hit_test(x: i32, y: i32, state: &GameState, metrics: &LayoutMetrics) -> Option<usize> {
    // Iterate from highest layer to lowest for correct occlusion
    for layer in (0..=4u8).rev() {
        for (idx, pos) in state.board.layout.positions.iter().enumerate() {
            if pos.layer != layer {
                continue;
            }
            // Skip positions where the tile has been removed
            if state.board.tiles[idx].is_none() {
                continue;
            }
            let rect = tile_screen_rect(pos, metrics);
            if rect.contains_point((x, y)) {
                return Some(idx);
            }
        }
    }
    None
}

/// Visual highlight state for a tile being rendered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TileHighlight {
    /// No special highlight.
    None,
    /// Gold border indicating the tile is selected.
    Selected,
    /// Pulsing glow for hint, with phase 0.0–1.0.
    HintGlow(f32),
    /// Red flash indicating a mismatched pair.
    MismatchFlash,
    /// Fade-out animation with alpha 0.0–1.0 (0.0 = fully transparent).
    Removing(f32),
}

/// Holds pre-rendered placeholder textures for tiles when real assets are missing.
/// Each placeholder is a colored rectangle with a unique color derived from its face ID.
pub struct PlaceholderTiles {
    /// Colors used for each face ID when textures are unavailable.
    colors: Vec<Color>,
}

impl PlaceholderTiles {
    fn new() -> Self {
        let colors: Vec<Color> = (0..TOTAL_FACE_COUNT)
            .map(|i| {
                // Generate distinct colors using HSV-like distribution
                let hue = (i as f32 / TOTAL_FACE_COUNT as f32) * 360.0;
                let (r, g, b) = hsv_to_rgb(hue, 0.7, 0.9);
                Color::RGB(r, g, b)
            })
            .collect();
        Self { colors }
    }

    /// Returns the placeholder color for a given face ID.
    fn color_for(&self, face_id: u8) -> Color {
        self.colors
            .get(face_id as usize)
            .copied()
            .unwrap_or(Color::RGB(128, 128, 128))
    }
}

/// UI texture set for buttons and overlays.
pub struct UiTextures {
    /// Whether real UI textures were loaded successfully.
    pub loaded: bool,
}

/// The main renderer for xMahjong.
///
/// Manages the SDL2 window, canvas, loaded textures, fonts, and placeholder assets.
///
/// IMPORTANT: Field order matters for drop safety. `tile_textures` must be declared
/// before `texture_creator` so it is dropped first (Rust drops in declaration order).
pub struct Renderer {
    /// The SDL2 hardware-accelerated canvas for drawing.
    pub canvas: Canvas<Window>,
    /// TTF context for font rendering.
    pub ttf_context: Sdl2TtfContext,
    /// Loaded tile face textures (None if the asset file was missing).
    /// SAFETY: These textures reference texture_creator and must be dropped first.
    pub tile_textures: Vec<Option<Texture<'static>>>,
    /// Background texture (None if not loaded).
    /// SAFETY: References texture_creator, must be dropped before it.
    pub background_texture: Option<Texture<'static>>,
    /// Trophy textures.
    /// SAFETY: These textures reference texture_creator and must be dropped first.
    pub trophy_no_shuffle: Option<Texture<'static>>,
    pub trophy_no_undo: Option<Texture<'static>>,
    pub trophy_no_hint: Option<Texture<'static>>,
    /// Empty trophy/coin used as base for numbered repeatable achievements.
    pub trophy_empty: Option<Texture<'static>>,
    // Streak trophies
    pub trophy_streak_zero: Option<Texture<'static>>,
    pub trophy_streak_one_day: Option<Texture<'static>>,
    pub trophy_streak_two_days: Option<Texture<'static>>,
    pub trophy_streak_seven_days: Option<Texture<'static>>,
    pub trophy_streak_fourteen_days: Option<Texture<'static>>,
    pub trophy_streak_thirty_days: Option<Texture<'static>>,
    pub trophy_streak_sixty_days: Option<Texture<'static>>,
    pub trophy_streak_three_months: Option<Texture<'static>>,
    pub trophy_streak_six_months: Option<Texture<'static>>,
    pub trophy_streak_nine_months: Option<Texture<'static>>,
    pub trophy_streak_one_year: Option<Texture<'static>>,
    /// Texture creator bound to the window context (for creating textures at runtime).
    pub texture_creator: TextureCreator<WindowContext>,
    /// Whether the tile back texture was loaded.
    pub tile_back_loaded: bool,
    /// Whether the background texture was loaded.
    pub background_loaded: bool,
    /// UI textures state.
    pub ui_textures: UiTextures,
    /// Placeholder tile colors for when textures are missing.
    pub placeholders: PlaceholderTiles,
    /// Background color used when no background texture is available.
    pub background_color: Color,
    /// Tile back color used when no tile back texture is available.
    pub tile_back_color: Color,
}

impl Renderer {
    /// Creates a new Renderer with an SDL2 window and hardware-accelerated canvas.
    ///
    /// Initializes:
    /// - SDL2 video subsystem
    /// - Window (1024×768, resizable, minimum 800×600)
    /// - Hardware-accelerated canvas
    /// - SDL2_ttf for text rendering
    /// - Asset loading with graceful fallback for missing files
    ///
    /// # Arguments
    /// * `sdl_context` - Reference to the initialized SDL2 context
    ///
    /// # Returns
    /// * `Ok(Renderer)` on success
    /// * `Err(String)` if window creation or canvas creation fails
    pub fn new(sdl_context: &sdl2::Sdl) -> Result<Self, String> {
        // Initialize video subsystem
        let video_subsystem = sdl_context.video()?;

        // Detect screen area and adjust window size
        let (initial_width, initial_height, min_width, min_height) = {
            #[cfg(target_os = "macos")]
            let (screen_w, screen_h) = {
                // On macOS, use usable bounds (excludes menu bar and dock)
                let usable = video_subsystem.display_usable_bounds(0)
                    .unwrap_or(sdl2::rect::Rect::new(0, 0, DEFAULT_WIDTH, DEFAULT_HEIGHT));
                (usable.width(), usable.height())
            };

            #[cfg(target_os = "windows")]
            let (screen_w, screen_h) = {
                // On Windows, use usable bounds so the taskbar is not covered by default
                let usable = video_subsystem.display_usable_bounds(0)
                    .unwrap_or(sdl2::rect::Rect::new(0, 0, DEFAULT_WIDTH, DEFAULT_HEIGHT));
                (usable.width(), usable.height())
            };

            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            let (screen_w, screen_h) = {
                let display_mode = video_subsystem.desktop_display_mode(0)
                    .unwrap_or(sdl2::video::DisplayMode::new(
                        sdl2::pixels::PixelFormatEnum::Unknown,
                        DEFAULT_WIDTH as i32,
                        DEFAULT_HEIGHT as i32,
                        60
                    ));
                (display_mode.w as u32, display_mode.h as u32)
            };

            // Use desired size or available screen area, whichever is smaller
            let w = DEFAULT_WIDTH.min(screen_w);
            let h = DEFAULT_HEIGHT.min(screen_h);
            let mw = MIN_WIDTH.min(screen_w);
            let mh = MIN_HEIGHT.min(screen_h);
            (w, h, mw, mh)
        };

        // Create window with detected size, resizable flag
        let mut window = video_subsystem
            .window("xMahjong", initial_width, initial_height)
            .resizable()
            .position_centered()
            .build()
            .map_err(|e| format!("Failed to create window: {}", e))?;

        // Set minimum window size (capped to screen resolution)
        window.set_minimum_size(min_width, min_height)
            .map_err(|e| format!("Failed to set minimum window size: {}", e))?;

        // Attempt to set window icon (Tux icon)
        Self::set_window_icon(&mut window);

        // Create hardware-accelerated canvas
        let canvas = window
            .into_canvas()
            .accelerated()
            .present_vsync()
            .build()
            .map_err(|e| format!("Failed to create canvas: {}", e))?;

        // Initialize SDL2_ttf
        let ttf_context = sdl2::ttf::init()
            .map_err(|e| format!("Failed to initialize SDL2_ttf: {}", e))?;

        // Create texture creator
        let texture_creator = canvas.texture_creator();

        // Resolve assets directory
        let base = assets_path();

        // Load assets with graceful fallback
        let tile_textures = Self::load_tile_textures(&texture_creator, &base);
        let tile_back_loaded = Self::load_tile_back(&texture_creator, &base);
        let (background_loaded, background_texture) = Self::load_background(&texture_creator, &base);
        let ui_textures = Self::load_ui_textures(&texture_creator, &base);

        let trophy_no_shuffle = match texture_creator.load_texture(&format!("{}/trophies/no-shuffle.png", base)) {
            Ok(texture) => Some(unsafe { std::mem::transmute(texture) }),
            Err(_) => None,
        };
        let trophy_no_undo = match texture_creator.load_texture(&format!("{}/trophies/no-undo.png", base)) {
            Ok(texture) => Some(unsafe { std::mem::transmute(texture) }),
            Err(_) => None,
        };
        let trophy_no_hint = match texture_creator.load_texture(&format!("{}/trophies/no-hint.png", base)) {
            Ok(texture) => Some(unsafe { std::mem::transmute(texture) }),
            Err(_) => None,
        };
        let trophy_empty = match texture_creator.load_texture(&format!("{}/trophies/empty.png", base)) {
            Ok(texture) => Some(unsafe { std::mem::transmute(texture) }),
            Err(_) => None,
        };

        // Load Streak Trophies
        let trophy_streak_zero = match texture_creator.load_texture(&format!("{}/trophies/zero.png", base)) {
            Ok(texture) => Some(unsafe { std::mem::transmute(texture) }),
            Err(_) => None,
        };
        let trophy_streak_one_day = match texture_creator.load_texture(&format!("{}/trophies/one_day.png", base)) {
            Ok(texture) => Some(unsafe { std::mem::transmute(texture) }),
            Err(_) => None,
        };
        let trophy_streak_two_days = match texture_creator.load_texture(&format!("{}/trophies/two_days.png", base)) {
            Ok(texture) => Some(unsafe { std::mem::transmute(texture) }),
            Err(_) => None,
        };
        let trophy_streak_seven_days = match texture_creator.load_texture(&format!("{}/trophies/seven_days.png", base)) {
            Ok(texture) => Some(unsafe { std::mem::transmute(texture) }),
            Err(_) => None,
        };
        let trophy_streak_fourteen_days = match texture_creator.load_texture(&format!("{}/trophies/fourteen_days.png", base)) {
            Ok(texture) => Some(unsafe { std::mem::transmute(texture) }),
            Err(_) => None,
        };
        let trophy_streak_thirty_days = match texture_creator.load_texture(&format!("{}/trophies/thirty_days.png", base)) {
            Ok(texture) => Some(unsafe { std::mem::transmute(texture) }),
            Err(_) => None,
        };
        let trophy_streak_sixty_days = match texture_creator.load_texture(&format!("{}/trophies/sixty_days.png", base)) {
            Ok(texture) => Some(unsafe { std::mem::transmute(texture) }),
            Err(_) => None,
        };
        let trophy_streak_three_months = match texture_creator.load_texture(&format!("{}/trophies/three_months.png", base)) {
            Ok(texture) => Some(unsafe { std::mem::transmute(texture) }),
            Err(_) => None,
        };
        let trophy_streak_six_months = match texture_creator.load_texture(&format!("{}/trophies/six_months.png", base)) {
            Ok(texture) => Some(unsafe { std::mem::transmute(texture) }),
            Err(_) => None,
        };
        let trophy_streak_nine_months = match texture_creator.load_texture(&format!("{}/trophies/nine_months.png", base)) {
            Ok(texture) => Some(unsafe { std::mem::transmute(texture) }),
            Err(_) => None,
        };
        let trophy_streak_one_year = match texture_creator.load_texture(&format!("{}/trophies/one_year.png", base)) {
            Ok(texture) => Some(unsafe { std::mem::transmute(texture) }),
            Err(_) => None,
        };

        Ok(Self {
            canvas,
            texture_creator,
            ttf_context,
            tile_textures,
            background_texture,
            trophy_no_shuffle,
            trophy_no_undo,
            trophy_no_hint,
            trophy_empty,
            trophy_streak_zero,
            trophy_streak_one_day,
            trophy_streak_two_days,
            trophy_streak_seven_days,
            trophy_streak_fourteen_days,
            trophy_streak_thirty_days,
            trophy_streak_sixty_days,
            trophy_streak_three_months,
            trophy_streak_six_months,
            trophy_streak_nine_months,
            trophy_streak_one_year,
            tile_back_loaded,
            background_loaded,
            ui_textures,
            placeholders: PlaceholderTiles::new(),
            background_color: Color::RGB(34, 85, 34),  // Dark green felt
            tile_back_color: Color::RGB(240, 230, 200), // Ivory tile back
        })
    }

    /// Attempts to set the window icon to a Tux image.
    /// Logs a warning if the icon file is not found.
    fn set_window_icon(window: &mut Window) {
        let base = assets_path();
        let icon_path = format!("{}/icon.png", base);
        match sdl2::surface::Surface::from_file(&icon_path) {
            Ok(icon_surface) => {
                window.set_icon(&icon_surface);
            }
            Err(_) => {
                eprintln!(
                    "[xMahjong] Warning: Window icon not found at '{}'. Using default icon.",
                    icon_path
                );
            }
        }
    }

    /// Attempts to load tile face textures from the assets directory.
    /// Returns a Vec of Option<Texture> - loaded textures or None for missing files.
    /// Indices 0-49 are penguin tiles (assets/tiles/), indices 50-99 are dog tiles (assets/dogs/).
    /// Missing textures are logged as warnings and will use placeholder colors.
    ///
    /// SAFETY: The returned textures have their lifetime erased to 'static.
    /// The caller must ensure the TextureCreator outlives these textures.
    fn load_tile_textures(texture_creator: &TextureCreator<WindowContext>, base: &str) -> Vec<Option<Texture<'static>>> {
        let mut textures = Vec::with_capacity(TOTAL_FACE_COUNT);

        // Load penguin tiles (face IDs 0-49)
        for i in 0..TILE_FACE_COUNT {
            let path = format!("{}/tiles/face_{:02}.png", base, i);
            match texture_creator.load_texture(&path) {
                Ok(texture) => {
                    // SAFETY: The texture_creator is stored in the same struct and
                    // outlives this Vec (dropped after it due to field order).
                    let texture: Texture<'static> = unsafe { std::mem::transmute(texture) };
                    textures.push(Some(texture));
                }
                Err(e) => {
                    textures.push(None);
                    eprintln!(
                        "[xMahjong] Warning: Tile texture not found: '{}'. Using placeholder. ({})",
                        path, e
                    );
                }
            }
        }

        // Load dog tiles (face IDs 50-99)
        for i in 0..TILE_FACE_COUNT {
            let path = format!("{}/dogs/face_{:02}.png", base, i);
            match texture_creator.load_texture(&path) {
                Ok(texture) => {
                    let texture: Texture<'static> = unsafe { std::mem::transmute(texture) };
                    textures.push(Some(texture));
                }
                Err(_) => {
                    textures.push(None);
                }
            }
        }

        // Load space tiles (face IDs 100-149)
        for i in 0..TILE_FACE_COUNT {
            let path = format!("{}/space/face_{:02}.png", base, i);
            match texture_creator.load_texture(&path) {
                Ok(texture) => {
                    let texture: Texture<'static> = unsafe { std::mem::transmute(texture) };
                    textures.push(Some(texture));
                }
                Err(e) => {
                    textures.push(None);
                    eprintln!(
                        "[xMahjong] Warning: Space tile texture not found: '{}'. Using placeholder. ({})",
                        path, e
                    );
                }
            }
        }

        textures
    }

    /// Attempts to load the tile back texture.
    fn load_tile_back(_texture_creator: &TextureCreator<WindowContext>, base: &str) -> bool {
        let path = format!("{}/tiles/tile_back.png", base);
        if std::path::Path::new(&path).exists() {
            true
        } else {
            eprintln!(
                "[xMahjong] Warning: Tile back texture not found: '{}'. Using placeholder color.",
                path
            );
            false
        }
    }

    /// Attempts to load the background texture.
    fn load_background(texture_creator: &TextureCreator<WindowContext>, base: &str) -> (bool, Option<Texture<'static>>) {
        let path = format!("{}/background.png", base);
        match texture_creator.load_texture(&path) {
            Ok(texture) => {
                let texture: Texture<'static> = unsafe { std::mem::transmute(texture) };
                (true, Some(texture))
            }
            Err(_) => {
                eprintln!(
                    "[xMahjong] Warning: Background texture not found: '{}'. Using solid color.",
                    path
                );
                (false, None)
            }
        }
    }

    /// Attempts to load UI textures (buttons, overlays).
    fn load_ui_textures(_texture_creator: &TextureCreator<WindowContext>, base: &str) -> UiTextures {
        let ui_path = format!("{}/ui", base);
        if std::path::Path::new(&ui_path).exists() {
            UiTextures { loaded: true }
        } else {
            eprintln!(
                "[xMahjong] Warning: UI textures directory not found: '{}'. Using fallback rendering.",
                ui_path
            );
            UiTextures { loaded: false }
        }
    }

    /// Returns the current window size as (width, height).
    pub fn window_size(&self) -> (u32, u32) {
        self.canvas.output_size().unwrap_or((DEFAULT_WIDTH, DEFAULT_HEIGHT))
    }

    /// Clears the canvas with the background color or texture.
    pub fn clear(&mut self) {
        self.canvas.set_draw_color(self.background_color);
        self.canvas.clear();
        // Draw background texture stretched to fill the window
        if let Some(ref texture) = self.background_texture {
            let (w, h) = self.window_size();
            let dest = Rect::new(0, 0, w, h);
            self.canvas.copy(texture, None, dest).ok();
        }
    }

    /// Presents the rendered frame to the screen.
    pub fn present(&mut self) {
        self.canvas.present();
    }

    /// Draws a placeholder tile at the given screen rectangle.
    ///
    /// Renders the tile back (ivory rectangle with border) and the face
    /// (colored inner rectangle) using placeholder colors.
    pub fn draw_placeholder_tile(&mut self, face_id: u8, dest: Rect, selected: bool) {
        // Draw tile back (slightly larger for border effect)
        let back_color = if selected {
            Color::RGB(255, 215, 0) // Gold highlight for selection
        } else {
            self.tile_back_color
        };
        self.canvas.set_draw_color(back_color);
        self.canvas.fill_rect(dest).ok();

        // Draw tile border
        self.canvas.set_draw_color(Color::RGB(80, 80, 80));
        self.canvas.draw_rect(dest).ok();

        // Draw face color (inner area)
        let face_color = self.placeholders.color_for(face_id);
        let inner = Rect::new(
            dest.x() + 3,
            dest.y() + 3,
            dest.width().saturating_sub(6),
            dest.height().saturating_sub(6),
        );
        self.canvas.set_draw_color(face_color);
        self.canvas.fill_rect(inner).ok();

        // Draw inner border for depth
        self.canvas.set_draw_color(Color::RGB(60, 60, 60));
        self.canvas.draw_rect(inner).ok();
    }

    /// Renders all tiles on the board with depth effect (bottom-to-top ordering).
    ///
    /// Tiles are sorted by layer so that lower layers are drawn first and higher
    /// layers paint over them. Each layer is offset by a few pixels to simulate depth.
    /// Animation states (removal, mismatch, hint glow, shuffle) are detected from
    /// `state.animations` and applied as highlight effects.
    pub fn render_board(&mut self, state: &GameState, _layout_rect: Rect) {
        let now = Instant::now();

        // Determine if shuffle animation is active (dims/hides tiles during shuffle)
        let shuffle_progress = self.get_shuffle_progress(state, now);

        // Define a structure to represent a tile to render, allowing us to include removing tiles
        struct RenderTile<'a> {
            _idx: usize,
            pos: &'a TilePosition,
            face_id: u8,
            highlight: TileHighlight,
        }

        let layout = state.board.layout;
        let mut render_tiles = Vec::new();

        // Collect active tiles
        for (idx, pos) in layout.positions.iter().enumerate() {
            if let Some(tile) = state.board.tiles[idx] {
                let highlight = self.determine_highlight(state, idx, now);
                render_tiles.push(RenderTile {
                    _idx: idx,
                    pos,
                    face_id: tile.face_id,
                    highlight,
                });
            }
        }

        // Collect removing tiles from animations
        for anim in &state.animations {
            if let Animation::TileRemoval { positions, face_id, start_time, duration_ms } = anim {
                let elapsed_ms = now.duration_since(*start_time).as_millis() as u32;
                if elapsed_ms < *duration_ms {
                    let progress = (elapsed_ms as f32 / *duration_ms as f32).min(1.0);
                    let alpha = 1.0 - progress;

                    for &pos_idx in &[positions.0, positions.1] {
                        let pos = &layout.positions[pos_idx];
                        render_tiles.push(RenderTile {
                            _idx: pos_idx,
                            pos,
                            face_id: *face_id,
                            highlight: TileHighlight::Removing(alpha),
                        });
                    }
                }
            }
        }

        // Sort by layer ascending so lower layers are drawn first
        render_tiles.sort_by_key(|rt| rt.pos.layer);

        // Use the same LayoutMetrics as hit_test for consistent positioning
        let (win_w, win_h) = self.window_size();
        let metrics = compute_layout_rect(win_w, win_h);
        let thickness = compute_thickness(&metrics);

        for rt in &render_tiles {
            // Calculate screen rectangle using the same function as hit_test
            let dest = tile_screen_rect(rt.pos, &metrics);

            // If shuffle animation is active, skip rendering individual tile effects
            // and instead render with a shuffle visual
            if let Some(progress) = shuffle_progress {
                self.render_tile_with_shuffle(rt.face_id, dest, rt.pos.layer, progress, thickness, &metrics);
            } else {
                self.render_tile_3d(rt.face_id, dest, rt.pos.layer, rt.highlight, thickness, &metrics);
            }
        }

        // Render colorful match celebration lightnings
        self.render_lightnings(state, now);
    }

    /// Renders a single tile as a 3D block with side faces, shadow, and top face.
    ///
    /// Draw order: shadow → corner junction → right side face → bottom side face → top face.
    /// This ensures the top face content is never occluded by side faces (Requirement 1.7).
    ///
    /// # Arguments
    /// * `face_id` - The tile face texture index
    /// * `dest` - The top face rectangle (from `tile_screen_rect`)
    /// * `layer` - The tile's layer (0–4)
    /// * `highlight` - The current highlight state
    /// * `thickness` - Pre-computed thickness in pixels
    /// * `metrics` - Layout metrics for shadow offset scaling
    fn render_tile_3d(
        &mut self,
        face_id: u8,
        dest: Rect,
        layer: u8,
        highlight: TileHighlight,
        thickness: u32,
        metrics: &LayoutMetrics,
    ) {
        // Early return if fully transparent during removal animation
        if let TileHighlight::Removing(alpha) = highlight {
            if alpha <= 0.0 {
                return;
            }
        }

        // Compute alpha as u8: 255 normally, or scaled for Removing state
        let alpha_u8: u8 = if let TileHighlight::Removing(alpha) = highlight {
            (alpha * 255.0) as u8
        } else {
            255
        };

        // Get highlight-aware base color for side faces
        let side_base_color = side_face_base_color(&highlight, self.tile_back_color);

        // 1. Draw shadow (behind everything)
        self.draw_shadow(dest, thickness, layer, metrics, alpha_u8);

        // 2. Draw corner junction (at bottom-right intersection of side faces)
        self.draw_corner_junction(dest, thickness, side_base_color, alpha_u8);

        // 3. Draw right side face with brightness reduction
        self.draw_right_side_face(
            dest,
            thickness,
            shade_color(side_base_color, RIGHT_FACE_BRIGHTNESS),
            alpha_u8,
        );

        // 4. Draw bottom side face with brightness reduction
        self.draw_bottom_side_face(
            dest,
            thickness,
            shade_color(side_base_color, BOTTOM_FACE_BRIGHTNESS),
            alpha_u8,
        );

        // 5. Draw top face (reuses existing render_tile logic for back color, border,
        //    texture/placeholder, and inner border)

        // Determine tile back color based on highlight
        let (back_color, border_color) = match highlight {
            TileHighlight::None => (self.tile_back_color, Color::RGB(80, 80, 80)),
            TileHighlight::Selected => (
                Color::RGB(255, 215, 0), // Gold
                Color::RGB(218, 165, 32), // Darker gold border
            ),
            TileHighlight::HintGlow(phase) => {
                // Pulsing glow: interpolate between normal and bright cyan
                let intensity = ((phase * std::f32::consts::PI * 2.0).sin() + 1.0) / 2.0;
                let r = (240.0 + intensity * 15.0) as u8;
                let g = (230.0 + intensity * 25.0) as u8;
                let b = (200.0 + intensity * 55.0) as u8;
                let br = (80.0 + intensity * 100.0) as u8;
                let bg = (80.0 + intensity * 180.0) as u8;
                let bb = (80.0 + intensity * 175.0) as u8;
                (Color::RGB(r, g, b), Color::RGB(br, bg, bb))
            }
            TileHighlight::MismatchFlash => (
                Color::RGB(255, 100, 100), // Red flash
                Color::RGB(200, 0, 0),     // Dark red border
            ),
            TileHighlight::Removing(_) => {
                let back = self.tile_back_color;
                (
                    Color::RGBA(back.r, back.g, back.b, alpha_u8),
                    Color::RGBA(80, 80, 80, alpha_u8),
                )
            }
        };

        // Draw tile back/body (top face)
        self.canvas.set_draw_color(back_color);
        self.canvas.fill_rect(dest).ok();

        // Draw border
        self.canvas.set_draw_color(border_color);
        self.canvas.draw_rect(dest).ok();

        // For selected/hint/mismatch: draw an extra inner border for emphasis
        match highlight {
            TileHighlight::Selected | TileHighlight::HintGlow(_) | TileHighlight::MismatchFlash => {
                let inner_border = Rect::new(
                    dest.x() + 1,
                    dest.y() + 1,
                    dest.width().saturating_sub(2),
                    dest.height().saturating_sub(2),
                );
                self.canvas.draw_rect(inner_border).ok();
            }
            _ => {}
        }

        // Draw face: use loaded texture if available, otherwise placeholder color
        let inner = Rect::new(
            dest.x() + 3,
            dest.y() + 3,
            dest.width().saturating_sub(6),
            dest.height().saturating_sub(6),
        );

        if let Some(Some(texture)) = self.tile_textures.get(face_id as usize) {
            // Draw the actual tile texture
            if let TileHighlight::Removing(_) = highlight {
                // Alpha fade is visual from the back color change
            }
            self.canvas.copy(texture, None, inner).ok();
        } else {
            // Fallback to placeholder color
            let face_color = self.placeholders.color_for(face_id);
            let face_draw_color = if let TileHighlight::Removing(_) = highlight {
                Color::RGBA(face_color.r, face_color.g, face_color.b, alpha_u8)
            } else {
                face_color
            };
            self.canvas.set_draw_color(face_draw_color);
            self.canvas.fill_rect(inner).ok();
        }

        // Draw inner border for depth
        let inner_border_color = if let TileHighlight::Removing(_) = highlight {
            Color::RGBA(60, 60, 60, alpha_u8)
        } else {
            Color::RGB(60, 60, 60)
        };
        self.canvas.set_draw_color(inner_border_color);
        self.canvas.draw_rect(inner).ok();

        // Draw yellow fog overlay on hinted tiles for high visibility
        if let TileHighlight::HintGlow(phase) = highlight {
            let intensity = ((phase * std::f32::consts::PI * 2.0).sin() + 1.0) / 2.0;
            // Pulsing alpha between 50 and 90 for a visible yellow tint
            let fog_alpha = (50.0 + intensity * 40.0) as u8;
            self.canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
            self.canvas.set_draw_color(Color::RGBA(255, 220, 0, fog_alpha));
            self.canvas.fill_rect(dest).ok();
            self.canvas.set_blend_mode(sdl2::render::BlendMode::None);
        }

        // Draw yellow fog overlay on selected tile for visibility
        if let TileHighlight::Selected = highlight {
            self.canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
            self.canvas.set_draw_color(Color::RGBA(255, 220, 0, 70));
            self.canvas.fill_rect(dest).ok();
            self.canvas.set_blend_mode(sdl2::render::BlendMode::None);
        }
    }

    /// Determines the highlight state for a tile at the given position index.
    fn determine_highlight(&self, state: &GameState, pos_idx: usize, now: Instant) -> TileHighlight {
        // Check for active removal animation on this position
        for anim in &state.animations {
            match anim {
                Animation::TileRemoval {
                    positions,
                    start_time,
                    duration_ms,
                    ..
                } => {
                    if positions.0 == pos_idx || positions.1 == pos_idx {
                        let elapsed_ms = now.duration_since(*start_time).as_millis() as u32;
                        let progress = (elapsed_ms as f32 / *duration_ms as f32).min(1.0);
                        return TileHighlight::Removing(1.0 - progress);
                    }
                }
                Animation::Lightning { .. } => {}
                Animation::TileMismatch {
                    positions,
                    start_time,
                    duration_ms,
                } => {
                    if positions.0 == pos_idx || positions.1 == pos_idx {
                        let elapsed_ms = now.duration_since(*start_time).as_millis() as u32;
                        if elapsed_ms < *duration_ms {
                            return TileHighlight::MismatchFlash;
                        }
                    }
                }
                Animation::HintPulse {
                    positions,
                    start_time,
                } => {
                    if positions.0 == pos_idx || positions.1 == pos_idx {
                        let elapsed_ms = now.duration_since(*start_time).as_millis() as u32;
                        let phase = (elapsed_ms % HINT_PULSE_CYCLE_MS) as f32
                            / HINT_PULSE_CYCLE_MS as f32;
                        return TileHighlight::HintGlow(phase);
                    }
                }
                Animation::Shuffle { .. } => {
                    // Shuffle animation is handled at the board level, not per-tile
                }
            }
        }

        // Check hint state (non-animation based hint display)
        if let Some(ref hint) = state.hint {
            if hint.position_a == pos_idx || hint.position_b == pos_idx {
                let elapsed_ms = now.duration_since(hint.activated_at).as_millis() as u32;
                let phase = (elapsed_ms % HINT_PULSE_CYCLE_MS) as f32 / HINT_PULSE_CYCLE_MS as f32;
                return TileHighlight::HintGlow(phase);
            }
        }

        // Check selection state
        if state.selection == Some(pos_idx) {
            return TileHighlight::Selected;
        }

        TileHighlight::None
    }

    /// Renders a magnified (zoomed) view of a single tile face, centered on screen.
    ///
    /// Used for long-press magnification on mobile devices where tiles can be small.
    /// Draws a large version of the tile face (3x normal size) with a semi-transparent
    /// backdrop and a rounded border, overlaid on top of the game board.
    ///
    /// # Arguments
    /// * `face_id` - The tile face texture index to magnify
    pub fn render_magnified_tile(&mut self, face_id: u8) {
        let (win_w, win_h) = self.window_size();

        // Draw semi-transparent dark overlay behind the magnified tile
        self.canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
        self.canvas.set_draw_color(Color::RGBA(0, 0, 0, 140));
        self.canvas.fill_rect(Rect::new(0, 0, win_w, win_h)).ok();

        // Calculate magnified tile size: ~3x normal tile size, capped to window
        let metrics = compute_layout_rect(win_w, win_h);
        let normal_w = (2.0 * metrics.tile_width) as u32;
        let normal_h = (2.0 * metrics.tile_height) as u32;

        // Scale up by 3x but cap at 80% of window dimensions
        let mag_w = (normal_w * 3).min((win_w as f32 * 0.8) as u32);
        let mag_h = (normal_h * 3).min((win_h as f32 * 0.8) as u32);

        // Center on screen
        let mag_x = (win_w as i32 - mag_w as i32) / 2;
        let mag_y = (win_h as i32 - mag_h as i32) / 2;

        let mag_rect = Rect::new(mag_x, mag_y, mag_w, mag_h);

        // Draw tile background (ivory card)
        self.canvas.set_blend_mode(sdl2::render::BlendMode::None);
        self.canvas.set_draw_color(self.tile_back_color);
        self.canvas.fill_rect(mag_rect).ok();

        // Draw outer border (dark)
        self.canvas.set_draw_color(Color::RGB(60, 60, 60));
        self.canvas.draw_rect(mag_rect).ok();
        // Double border for emphasis
        let outer2 = Rect::new(mag_x - 1, mag_y - 1, mag_w + 2, mag_h + 2);
        self.canvas.draw_rect(outer2).ok();
        let outer3 = Rect::new(mag_x - 2, mag_y - 2, mag_w + 4, mag_h + 4);
        self.canvas.set_draw_color(Color::RGB(255, 215, 0)); // Gold border
        self.canvas.draw_rect(outer3).ok();
        let outer4 = Rect::new(mag_x - 3, mag_y - 3, mag_w + 6, mag_h + 6);
        self.canvas.draw_rect(outer4).ok();

        // Draw the face texture (or placeholder) within the magnified area with padding
        let padding = 6;
        let inner = Rect::new(
            mag_x + padding,
            mag_y + padding,
            mag_w.saturating_sub(padding as u32 * 2),
            mag_h.saturating_sub(padding as u32 * 2),
        );

        if let Some(Some(texture)) = self.tile_textures.get(face_id as usize) {
            self.canvas.copy(texture, None, inner).ok();
        } else {
            // Fallback to placeholder color
            let face_color = self.placeholders.color_for(face_id);
            self.canvas.set_draw_color(face_color);
            self.canvas.fill_rect(inner).ok();
        }

        // Draw inner border
        self.canvas.set_draw_color(Color::RGB(60, 60, 60));
        self.canvas.draw_rect(inner).ok();

        // Draw "magnifying glass" hint text at the bottom
        // (Simple indicator so the user knows this is a zoom view)
        self.canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
        let label_h: u32 = 20;
        let label_y = mag_y + mag_h as i32 + 8;
        let label_w: u32 = 140;
        let label_x = (win_w as i32 - label_w as i32) / 2;
        self.canvas.set_draw_color(Color::RGBA(0, 0, 0, 180));
        self.canvas.fill_rect(Rect::new(label_x - 4, label_y - 2, label_w + 8, label_h + 4)).ok();
        self.canvas.set_draw_color(Color::RGBA(255, 255, 255, 220));
        // Draw a small "🔍 Hold to zoom" text area (just the background; actual text
        // rendering would need TTF which is heavy — the visual overlay is self-explanatory)
        self.canvas.set_blend_mode(sdl2::render::BlendMode::None);
    }

    /// Returns the shuffle animation progress (0.0–1.0) if a shuffle animation is active.
    fn get_shuffle_progress(&self, state: &GameState, now: Instant) -> Option<f32> {
        for anim in &state.animations {
            if let Animation::Shuffle {
                start_time,
                duration_ms,
            } = anim
            {
                let elapsed_ms = now.duration_since(*start_time).as_millis() as u32;
                if elapsed_ms < *duration_ms {
                    return Some(elapsed_ms as f32 / *duration_ms as f32);
                }
            }
        }
        None
    }

    /// Renders a tile during shuffle animation with a visual shuffle effect.
    /// The tile briefly flashes/fades based on shuffle progress.
    /// Draws the full 3D tile block (shadow, side faces, top face) with uniform alpha.
    fn render_tile_with_shuffle(
        &mut self,
        face_id: u8,
        dest: Rect,
        layer: u8,
        progress: f32,
        thickness: u32,
        metrics: &LayoutMetrics,
    ) {
        // During shuffle: tiles fade out in first half, fade in with new faces in second half
        let alpha = if progress < 0.5 {
            // Fading out: 1.0 -> 0.0 over first half
            1.0 - (progress * 2.0)
        } else {
            // Fading in: 0.0 -> 1.0 over second half
            (progress - 0.5) * 2.0
        };

        let a = (alpha * 255.0) as u8;

        // 1. Draw shadow (layer-aware, scaled offset)
        self.draw_shadow(dest, thickness, layer, metrics, a);

        // 2. Draw 3D side faces (no highlight during shuffle, use tile_back_color)
        let side_base = self.tile_back_color;
        self.draw_corner_junction(dest, thickness, side_base, a);
        self.draw_right_side_face(
            dest,
            thickness,
            shade_color(side_base, RIGHT_FACE_BRIGHTNESS),
            a,
        );
        self.draw_bottom_side_face(
            dest,
            thickness,
            shade_color(side_base, BOTTOM_FACE_BRIGHTNESS),
            a,
        );

        // 3. Draw top face with reduced alpha
        let back = self.tile_back_color;
        self.canvas.set_draw_color(Color::RGBA(back.r, back.g, back.b, a));
        self.canvas.fill_rect(dest).ok();

        self.canvas.set_draw_color(Color::RGBA(80, 80, 80, a));
        self.canvas.draw_rect(dest).ok();

        let inner = Rect::new(
            dest.x() + 3,
            dest.y() + 3,
            dest.width().saturating_sub(6),
            dest.height().saturating_sub(6),
        );

        if let Some(Some(texture)) = self.tile_textures.get(face_id as usize) {
            // Draw texture with alpha modulation for shuffle fade
            // Note: SDL2 texture alpha mod would require mutable access to texture
            self.canvas.copy(texture, None, inner).ok();
        } else {
            let face_color = self.placeholders.color_for(face_id);
            self.canvas.set_draw_color(Color::RGBA(face_color.r, face_color.g, face_color.b, a));
            self.canvas.fill_rect(inner).ok();
        }

        self.canvas.set_draw_color(Color::RGBA(60, 60, 60, a));
        self.canvas.draw_rect(inner).ok();
    }

    /// Computes tile dimensions and offset based on the layout and available screen area.
    ///
    /// Returns (tile_width, tile_height, x_offset, y_offset) where offsets position
    /// the layout within the given layout_rect.
    #[allow(dead_code)]
    fn compute_tile_geometry(
        &self,
        layout: &crate::board::Layout,
        layout_rect: Rect,
    ) -> (u32, u32, i32, i32) {
        // Find the extent of the layout in grid units
        let max_col = layout
            .positions
            .iter()
            .map(|p| p.col as u32)
            .max()
            .unwrap_or(0)
            + 2; // +2 because each tile occupies 2 grid units
        let max_row = layout
            .positions
            .iter()
            .map(|p| p.row as u32)
            .max()
            .unwrap_or(0)
            + 2;

        // Each tile occupies 2x2 grid cells, so the tile is half the cell width
        // Calculate tile size to fit within layout_rect
        let available_w = layout_rect.width();
        let available_h = layout_rect.height();

        // Tile width = available width / (max_col / 2) since tiles are 2 grid units wide
        // But we position tiles at col * (tile_w / 2), so:
        // total_width = max_col * (tile_w / 2) + tile_w
        // Solve: tile_w = available_w / (max_col/2 + 1)
        let tile_w = (available_w * 2) / (max_col + 2);
        let tile_h = (available_h * 2) / (max_row + 2);

        // Use the smaller dimension to maintain aspect ratio (tiles are roughly 4:5)
        let tile_w = tile_w.min(tile_h * 4 / 5);
        let tile_h = tile_h.min(tile_w * 5 / 4);

        // Center the layout within layout_rect
        let total_w = max_col * (tile_w / 2) + tile_w;
        let total_h = max_row * (tile_h / 2) + tile_h;
        let x_offset = layout_rect.x() + (available_w as i32 - total_w as i32) / 2;
        let y_offset = layout_rect.y() + (available_h as i32 - total_h as i32) / 2;

        (tile_w, tile_h, x_offset, y_offset)
    }

    /// Loads a font from the assets directory at the given point size.
    /// Returns None if the font file is not found.
    pub fn load_font(&self, point_size: u16) -> Option<sdl2::ttf::Font<'_, 'static>> {
        let base = assets_path();
        let font_path = format!("{}/fonts/default.ttf", base);
        match self.ttf_context.load_font(&font_path, point_size) {
            Ok(font) => Some(font),
            Err(e) => {
                eprintln!(
                    "[xMahjong] Warning: Could not load font '{}': {}. Text rendering disabled.",
                    font_path, e
                );
                None
            }
        }
    }

    // ─── UI Overlay Rendering ────────────────────────────────────────────────────

    /// Renders a semi-transparent dark overlay covering the entire window.
    /// Used as backdrop for menus, dialogs, and notifications.
    fn draw_overlay_backdrop(&mut self) {
        let (w, h) = self.window_size();
        self.canvas.set_draw_color(Color::RGBA(0, 0, 0, 180));
        self.canvas.fill_rect(Rect::new(0, 0, w, h)).ok();
    }

    /// Draws a centered dialog box with the given dimensions.
    /// Returns the Rect of the dialog for positioning child elements.
    fn draw_dialog_box(&mut self, width: u32, height: u32) -> Rect {
        let (win_w, win_h) = self.window_size();
        let x = (win_w.saturating_sub(width)) / 2;
        let y = (win_h.saturating_sub(height)) / 2;
        let dialog = Rect::new(x as i32, y as i32, width, height);

        // Dialog background
        self.canvas.set_draw_color(Color::RGB(45, 45, 60));
        self.canvas.fill_rect(dialog).ok();

        // Dialog border
        self.canvas.set_draw_color(Color::RGB(100, 140, 180));
        self.canvas.draw_rect(dialog).ok();

        // Inner border for depth
        let inner = Rect::new(
            dialog.x() + 2,
            dialog.y() + 2,
            dialog.width().saturating_sub(4),
            dialog.height().saturating_sub(4),
        );
        self.canvas.set_draw_color(Color::RGB(70, 90, 120));
        self.canvas.draw_rect(inner).ok();

        dialog
    }

    /// Draws a placeholder button (colored rectangle) at the given position.
    /// The color distinguishes button types. Returns the button Rect for hit-testing.
    #[allow(dead_code)]
    fn draw_button(&mut self, x: i32, y: i32, width: u32, height: u32, color: Color) -> Rect {
        let btn = Rect::new(x, y, width, height);

        // Button fill
        self.canvas.set_draw_color(color);
        self.canvas.fill_rect(btn).ok();

        // Button border (lighter for 3D effect)
        self.canvas.set_draw_color(Color::RGB(
            color.r.saturating_add(40),
            color.g.saturating_add(40),
            color.b.saturating_add(40),
        ));
        self.canvas.draw_rect(btn).ok();

        // Text area placeholder (slightly lighter inner rectangle to represent label)
        let text_area = Rect::new(
            x + 8,
            y + 4,
            width.saturating_sub(16),
            height.saturating_sub(8),
        );
        self.canvas.set_draw_color(Color::RGBA(255, 255, 255, 60));
        self.canvas.fill_rect(text_area).ok();

        btn
    }

    /// Draws text using a simple built-in bitmap font (5×7 pixel characters).
    /// Each character is scaled by `scale` factor. Color is specified by `color`.
    /// This works without any TTF font file.
    fn draw_bitmap_text(&mut self, text: &str, x: i32, y: i32, scale: u32, color: Color) {
        self.canvas.set_draw_color(color);
        let mut cursor_x = x;
        for ch in text.chars() {
            if let Some(glyph) = bitmap_glyph(ch) {
                for (row_idx, &row_bits) in glyph.iter().enumerate() {
                    for col in 0..5u32 {
                        if row_bits & (1 << (4 - col)) != 0 {
                            let px = cursor_x + (col * scale) as i32;
                            let py = y + (row_idx as u32 * scale) as i32;
                            self.canvas.fill_rect(Rect::new(px, py, scale, scale)).ok();
                        }
                    }
                }
            }
            cursor_x += (6 * scale) as i32; // 5px char + 1px spacing
        }
    }

    /// Draws a labeled button with readable bitmap text.
    fn draw_labeled_button(&mut self, x: i32, y: i32, width: u32, height: u32, color: Color, label: &str) -> Rect {
        let btn = Rect::new(x, y, width, height);

        // Button fill
        self.canvas.set_draw_color(color);
        self.canvas.fill_rect(btn).ok();

        // Button border (lighter for 3D effect)
        self.canvas.set_draw_color(Color::RGB(
            color.r.saturating_add(40),
            color.g.saturating_add(40),
            color.b.saturating_add(40),
        ));
        self.canvas.draw_rect(btn).ok();

        // Draw the label text centered within the button
        let text_scale = 2u32;
        let text_w = label.len() as i32 * 6 * text_scale as i32;
        let text_h = 7 * text_scale as i32;
        let tx = x + (width as i32 - text_w) / 2;
        let ty = y + (height as i32 - text_h) / 2;
        self.draw_bitmap_text(label, tx, ty, text_scale, Color::RGB(255, 255, 255));

        btn
    }

    /// Renders the HUD overlay: timer display (MM:SS), score, level, and shuffles remaining.
    ///
    /// The HUD is drawn as a top bar with four sections:
    /// - Left: Timer display
    /// - Center-left: Score
    /// - Center-right: Level
    /// - Right: Shuffle count remaining
    ///
    /// Without fonts, these are rendered as colored rectangles with
    /// distinguishable colors to indicate each element's purpose.
    pub fn render_hud(&mut self, state: &GameState) {
        let (win_w, _win_h) = self.window_size();

        // HUD background bar at the top
        let hud_height: u32 = 40;
        let hud_rect = Rect::new(0, 0, win_w, hud_height);
        self.canvas.set_draw_color(Color::RGBA(20, 20, 30, 200));
        self.canvas.fill_rect(hud_rect).ok();

        // Bottom border of HUD
        self.canvas.set_draw_color(Color::RGB(80, 120, 160));
        self.canvas.draw_line(
            sdl2::rect::Point::new(0, hud_height as i32),
            sdl2::rect::Point::new(win_w as i32, hud_height as i32),
        ).ok();

        // Divide the bar into 5 sections
        let section_w = win_w as i32 / 5;

        // Timer display (section 1 — left) — total across all levels
        let total_ms = state.base_time_ms + state.timer.elapsed_ms
            + state.timer.last_tick.map(|t| t.elapsed().as_millis() as u64).unwrap_or(0);
        let total_secs = (total_ms / 1000) as u32;
        let minutes = total_secs / 60;
        let seconds = total_secs % 60;
        let timer_text = format!("{:02}:{:02}", minutes, seconds);
        let timer_w = timer_text.len() as i32 * 12;
        let timer_x = (section_w - timer_w) / 2;
        self.draw_bitmap_text(&timer_text, timer_x, 12, 2, Color::RGB(100, 220, 100));

        // Score display (section 2) — total across all levels
        let total_score = state.base_score + state.score.live_score();
        let score_text = format!("SCORE {}", total_score);
        let score_w = score_text.len() as i32 * 12;
        let score_x = section_w + (section_w - score_w) / 2;
        self.draw_bitmap_text(&score_text, score_x, 12, 2, Color::RGB(255, 215, 0));

        // Level display (section 3 — center)
        let level_text = format!("LEVEL {}", state.level);
        let level_w = level_text.len() as i32 * 12;
        let level_x = section_w * 2 + (section_w - level_w) / 2;
        self.draw_bitmap_text(&level_text, level_x, 12, 2, Color::RGB(200, 150, 255));

        // Hints used (section 4) — total across all levels
        let total_hints = state.base_hints + state.score.hints_used;
        let hints_text = format!("HINTS {}", total_hints);
        let hints_w = hints_text.len() as i32 * 12;
        let hints_x = section_w * 3 + (section_w - hints_w) / 2;
        self.draw_bitmap_text(&hints_text, hints_x, 12, 2, Color::RGB(0, 200, 200));

        // Shuffles remaining (section 5 — right)
        let shuffle_text = format!("SHUFFLE {}", state.shuffles_remaining);
        let shuffle_w = shuffle_text.len() as i32 * 12;
        let shuffle_x = section_w * 4 + (section_w - shuffle_w) / 2;
        self.draw_bitmap_text(&shuffle_text, shuffle_x, 12, 2, Color::RGB(100, 150, 255));
    }

    /// Renders a small, clickable "MENU" button in the bottom-left corner of the screen.
    ///
    /// This provides visual discoverability for the pause menu (ESC key).
    /// The button is rendered as a compact pill with an "=" hamburger icon,
    /// the word "MENU", and a subtle "ESC" hint — all in a style consistent
    /// with the game's pixel-art aesthetic.
    ///
    /// On Linux the button is rendered larger for better readability at high DPI.
    ///
    /// Returns the bounding Rect of the button for hit-testing.
    pub fn render_menu_button(&mut self) -> Rect {
        let (_win_w, win_h) = self.window_size();

        // On Linux, use a larger button and font scale for readability
        #[cfg(target_os = "linux")]
        let (btn_w, btn_h, text_scale, icon_scale): (u32, u32, u32, i32) = (120, 36, 2, 2);
        #[cfg(not(target_os = "linux"))]
        let (btn_w, btn_h, text_scale, icon_scale): (u32, u32, u32, i32) = (80, 26, 1, 1);

        let btn_x: i32 = 10;
        let btn_y: i32 = win_h as i32 - btn_h as i32 - 10;

        let btn_rect = Rect::new(btn_x, btn_y, btn_w, btn_h);

        // Semi-transparent dark background
        self.canvas.set_draw_color(Color::RGBA(25, 30, 40, 200));
        self.canvas.fill_rect(btn_rect).ok();

        // Subtle border (teal accent to match game palette)
        self.canvas.set_draw_color(Color::RGB(60, 140, 140));
        self.canvas.draw_rect(btn_rect).ok();

        // Draw hamburger icon (three horizontal lines) at left side
        let icon_x = btn_x + 6;
        let icon_center_y = btn_y + btn_h as i32 / 2;
        let line_w: u32 = (8 * icon_scale) as u32;
        let line_h: u32 = (2 * icon_scale) as u32;
        self.canvas.set_draw_color(Color::RGB(150, 220, 220));
        self.canvas.fill_rect(Rect::new(icon_x, icon_center_y - 6 * icon_scale, line_w, line_h)).ok();
        self.canvas.fill_rect(Rect::new(icon_x, icon_center_y - 1 * icon_scale, line_w, line_h)).ok();
        self.canvas.fill_rect(Rect::new(icon_x, icon_center_y + 4 * icon_scale, line_w, line_h)).ok();

        // "ESC" text (muted color as a keyboard hint)
        let text_y = btn_y + (btn_h as i32 - 8 * text_scale as i32) / 2;
        let esc_x = btn_x + 8 + (line_w as i32) + 4;
        self.draw_bitmap_text("ESC", esc_x, text_y, text_scale, Color::RGB(120, 180, 180));

        // Separator dot
        let esc_text_w = 3 * 6 * text_scale as i32; // "ESC" = 3 chars
        let dot_x = esc_x + esc_text_w + 2;
        let dot_size = (2 * icon_scale) as u32;
        self.canvas.set_draw_color(Color::RGB(80, 130, 130));
        self.canvas.fill_rect(Rect::new(dot_x, icon_center_y - icon_scale, dot_size, dot_size)).ok();

        // "MENU" text (brighter)
        let menu_x = dot_x + dot_size as i32 + 4;
        self.draw_bitmap_text("MENU", menu_x, text_y, text_scale, Color::RGB(200, 240, 240));

        btn_rect
    }

    /// Renders the pause menu overlay with game options.
    ///
    /// Menu items (rendered as placeholder buttons):
    /// - New Game (green)
    /// - Undo (blue)
    /// - Hint (cyan)
    /// - Shuffle (purple)
    /// - Shortcuts (green)
    /// - Leaderboard (blue)
    /// - Save + Quit (orange)
    /// - Difficulty toggle (teal)
    pub fn render_menu(&mut self, selected: usize, difficulty: &str) {
        self.draw_overlay_backdrop();

        let dialog = self.draw_dialog_box(300, 590);

        // Title
        self.draw_bitmap_text(
            "PAUSED",
            dialog.x() + 100,
            dialog.y() + 18,
            3,
            Color::RGB(200, 220, 255),
        );

        let btn_w: u32 = 220;
        let btn_h: u32 = 40;
        let btn_x = dialog.x() + ((dialog.width() - btn_w) / 2) as i32;
        let start_y = dialog.y() + 60;
        let spacing: i32 = 48;

        let difficulty_label = format!("MODE: {}", difficulty);

        let buttons: Vec<(Color, &str)> = vec![
            (Color::RGB(50, 140, 70), "NEW GAME"),
            (Color::RGB(50, 100, 180), "UNDO"),
            (Color::RGB(50, 160, 170), "HINT"),
            (Color::RGB(120, 60, 160), "SHUFFLE"),
            (Color::RGB(100, 140, 100), "SHORTCUTS"),
            (Color::RGB(50, 100, 180), "ACHIEVEMENTS"),
            (Color::RGB(0, 130, 130), &difficulty_label),
            (Color::RGB(80, 120, 180), "ABOUT"),
            (Color::RGB(160, 100, 180), "SWITCH USER"),
            (Color::RGB(200, 130, 50), "SAVE + QUIT"),
        ];

        for (i, (color, label)) in buttons.iter().enumerate() {
            let y = start_y + spacing * i as i32;
            self.draw_labeled_button(btn_x, y, btn_w, btn_h, *color, label);

            if i == selected {
                // Draw a bright white selection border around the highlighted button
                let sel_rect = Rect::new(btn_x - 2, y - 2, btn_w + 4, btn_h + 4);
                self.canvas.set_draw_color(Color::RGB(255, 255, 255));
                self.canvas.draw_rect(sel_rect).ok();
                let sel_rect_inner = Rect::new(btn_x - 1, y - 1, btn_w + 2, btn_h + 2);
                self.canvas.draw_rect(sel_rect_inner).ok();
            }
        }

        // Shortcut hints at bottom
        self.draw_bitmap_text(
            "ESC RESUME  CTRL+S SAVE",
            dialog.x() + 20,
            dialog.y() + 562,
            1,
            Color::RGB(120, 120, 140),
        );
    }

    /// Renders the shortcuts popup showing all keyboard shortcuts.
    pub fn render_shortcuts(&mut self) {
        self.draw_overlay_backdrop();

        let dialog = self.draw_dialog_box(420, 420);

        // Title
        self.draw_bitmap_text(
            "SHORTCUTS",
            dialog.x() + 140,
            dialog.y() + 16,
            3,
            Color::RGB(255, 215, 0),
        );

        let x_key = dialog.x() + 20;
        let x_action = dialog.x() + 200;
        let mut y = dialog.y() + 58;
        let line_h: i32 = 24;
        let key_color = Color::RGB(150, 220, 255);
        let action_color = Color::RGB(200, 200, 210);

        let shortcuts: &[(&str, &str)] = &[
            ("LEFT CLICK", "SELECT TILE"),
            ("CTRL+S", "SAVE GAME"),
            ("CTRL+Q", "SAVE + QUIT"),
            ("CTRL+N", "NEW GAME"),
            ("CTRL+R", "RESUME"),
            ("CTRL+P", "PAUSE"),
            ("CTRL+M", "TOGGLE MUTE"),
            ("SHIFT+S", "SHUFFLE"),
            ("SHIFT+U", "UNDO"),
            ("SHIFT+H", "HINT"),
            ("ESCAPE", "PAUSE / RESUME"),
            ("UP/DOWN", "NAVIGATE MENU"),
            ("ENTER", "SELECT MENU ITEM"),
        ];

        for (key, action) in shortcuts {
            self.draw_bitmap_text(key, x_key, y, 2, key_color);
            self.draw_bitmap_text(action, x_action, y, 2, action_color);
            y += line_h;
        }

        // Back button
        let btn_w: u32 = 180;
        let btn_h: u32 = 40;
        let btn_x = dialog.x() + ((dialog.width() - btn_w) / 2) as i32;
        let btn_y = dialog.y() + 420 - 55;
        self.draw_labeled_button(btn_x, btn_y, btn_w, btn_h, Color::RGB(100, 100, 100), "BACK");
    }

    /// Renders the victory overlay showing final time and score.
    ///
    /// Displays:
    /// - Victory message
    /// - Final time (MM:SS format)
    /// - Final score
    /// - New Game button
    /// - Leaderboard button
    ///
    /// # Arguments
    /// * `time` - Formatted time string (e.g., "05:23")
    /// * `score` - Final score value
    pub fn render_victory(&mut self, time: &str, score: u32, level: u32, selected: usize) {
        self.draw_overlay_backdrop();

        let dialog_h: u32 = if level < 50 { 360 } else { 300 };
        let dialog = self.draw_dialog_box(350, dialog_h);

        // "VICTORY!" title
        self.draw_bitmap_text(
            "VICTORY!",
            dialog.x() + 110,
            dialog.y() + 24,
            3,
            Color::RGB(255, 215, 0),
        );

        // Time display
        let time_label = format!("TIME  {}", time);
        self.draw_bitmap_text(
            &time_label,
            dialog.x() + 100,
            dialog.y() + 75,
            2,
            Color::RGB(100, 220, 100),
        );

        // Score display
        let score_label = format!("SCORE {}", score);
        self.draw_bitmap_text(
            &score_label,
            dialog.x() + 100,
            dialog.y() + 110,
            2,
            Color::RGB(255, 200, 50),
        );

        let btn_w: u32 = 220;
        let btn_h: u32 = 44;
        let btn_x = dialog.x() + ((dialog.width() - btn_w) / 2) as i32;

        if level < 50 {
            let buttons: &[(i32, Color, &str)] = &[
                (155, Color::RGB(120, 60, 180), "NEXT LEVEL"),
                (215, Color::RGB(50, 140, 70), "NEW GAME"),
                (275, Color::RGB(50, 100, 180), "ACHIEVEMENTS"),
            ];

            for (i, (y_off, color, label)) in buttons.iter().enumerate() {
                let y = dialog.y() + y_off;
                self.draw_labeled_button(btn_x, y, btn_w, btn_h, *color, label);
                if i == selected {
                    let sel_rect = Rect::new(btn_x - 2, y - 2, btn_w + 4, btn_h + 4);
                    self.canvas.set_draw_color(Color::RGB(255, 255, 255));
                    self.canvas.draw_rect(sel_rect).ok();
                    let sel_rect_inner = Rect::new(btn_x - 1, y - 1, btn_w + 2, btn_h + 2);
                    self.canvas.draw_rect(sel_rect_inner).ok();
                }
            }
        } else {
            let buttons: &[(i32, Color, &str)] = &[
                (160, Color::RGB(50, 140, 70), "NEW GAME"),
                (220, Color::RGB(50, 100, 180), "ACHIEVEMENTS"),
            ];

            for (i, (y_off, color, label)) in buttons.iter().enumerate() {
                let y = dialog.y() + y_off;
                self.draw_labeled_button(btn_x, y, btn_w, btn_h, *color, label);
                if i == selected {
                    let sel_rect = Rect::new(btn_x - 2, y - 2, btn_w + 4, btn_h + 4);
                    self.canvas.set_draw_color(Color::RGB(255, 255, 255));
                    self.canvas.draw_rect(sel_rect).ok();
                    let sel_rect_inner = Rect::new(btn_x - 1, y - 1, btn_w + 2, btn_h + 2);
                    self.canvas.draw_rect(sel_rect_inner).ok();
                }
            }
        }
    }

    /// Renders the achievements view with 3D wooden plank shelves and numbered trophies.
    pub fn render_leaderboard(&mut self, user_name: &str) {
        self.draw_overlay_backdrop();

        let leaderboard = Leaderboard::load(user_name);
        let shuffle_state = ShuffleState::load(user_name);
        let trophy_state = TrophyState::load(user_name);
        let current_streak = shuffle_state.consecutive_days;
        
        let dialog_w: u32 = 900;
        let dialog_h: u32 = 620;
        let dialog = self.draw_dialog_box(dialog_w, dialog_h);

        // Futuristic neon turquoise title (centered, scale 4)
        let title = "ACHIEVEMENTS";
        let title_w = title.len() as i32 * 6 * 4;
        self.draw_bitmap_text(
            title,
            dialog.x() + (dialog.width() as i32 - title_w) / 2,
            dialog.y() + 20,
            4,
            Color::RGB(0, 255, 220),
        );

        // Futuristic divider grid line under the title
        let line_y = dialog.y() + 65;
        self.canvas.set_draw_color(Color::RGB(0, 180, 255));
        self.canvas.draw_line(
            sdl2::rect::Point::new(dialog.x() + 20, line_y),
            sdl2::rect::Point::new(dialog.x() + dialog.width() as i32 - 20, line_y)
        ).ok();

        // Left Panel: Last Match Stats
        let stats_panel = Rect::new(dialog.x() + 30, dialog.y() + 85, 340, 450);
        self.canvas.set_draw_color(Color::RGB(50, 80, 120));
        self.canvas.draw_rect(stats_panel).ok();
        let stats_panel_inner = Rect::new(stats_panel.x() + 2, stats_panel.y() + 2, stats_panel.width() - 4, stats_panel.height() - 4);
        self.canvas.set_draw_color(Color::RGB(25, 30, 45));
        self.canvas.fill_rect(stats_panel_inner).ok();

        // Right Panel: Trophy Cabinet
        let cabinet_panel = Rect::new(dialog.x() + 400, dialog.y() + 85, 470, 450);
        self.canvas.set_draw_color(Color::RGB(50, 80, 120));
        self.canvas.draw_rect(cabinet_panel).ok();
        let cabinet_panel_inner = Rect::new(cabinet_panel.x() + 2, cabinet_panel.y() + 2, cabinet_panel.width() - 4, cabinet_panel.height() - 4);
        self.canvas.set_draw_color(Color::RGB(25, 30, 45));
        self.canvas.fill_rect(cabinet_panel_inner).ok();

        // Cabinet Shelves with 3D plank effect
        let shelf_w: u32 = cabinet_panel.width() - 40;
        let shelf_h: u32 = 8;
        let shelf_x = cabinet_panel.x() + 20;
        let top_shelf_y = cabinet_panel.y() + 185;
        let bottom_shelf_y = cabinet_panel.y() + 365;

        // 3D plank: top shelf
        self.canvas.set_draw_color(Color::RGB(180, 120, 60));
        self.canvas.fill_rect(Rect::new(shelf_x, top_shelf_y, shelf_w, shelf_h)).ok();
        self.canvas.set_draw_color(Color::RGB(140, 85, 40));
        self.canvas.fill_rect(Rect::new(shelf_x, top_shelf_y + shelf_h as i32, shelf_w, 8)).ok();
        self.canvas.set_draw_color(Color::RGB(220, 160, 90));
        self.canvas.draw_line(
            sdl2::rect::Point::new(shelf_x, top_shelf_y),
            sdl2::rect::Point::new(shelf_x + shelf_w as i32 - 1, top_shelf_y)
        ).ok();

        // 3D plank: bottom shelf
        self.canvas.set_draw_color(Color::RGB(180, 120, 60));
        self.canvas.fill_rect(Rect::new(shelf_x, bottom_shelf_y, shelf_w, shelf_h)).ok();
        self.canvas.set_draw_color(Color::RGB(140, 85, 40));
        self.canvas.fill_rect(Rect::new(shelf_x, bottom_shelf_y + shelf_h as i32, shelf_w, 8)).ok();
        self.canvas.set_draw_color(Color::RGB(220, 160, 90));
        self.canvas.draw_line(
            sdl2::rect::Point::new(shelf_x, bottom_shelf_y),
            sdl2::rect::Point::new(shelf_x + shelf_w as i32 - 1, bottom_shelf_y)
        ).ok();

        let has_last_match = !leaderboard.entries.is_empty();
        
        let (_hints_used, _shuffles_used, _undos_used, streak_days) = if has_last_match {
            let entry = &leaderboard.entries[0];
            let label_x = stats_panel.x() + 20;
            let val_x = stats_panel.x() + 180;
            
            // Draw Left Panel text
            self.draw_bitmap_text("LAST MATCH STATS", stats_panel.x() + 20, stats_panel.y() + 20, 2, Color::RGB(0, 180, 255));
            
            self.draw_bitmap_text("PLAYER:", label_x, stats_panel.y() + 55, 2, Color::RGB(200, 200, 200));
            self.draw_bitmap_text(&entry.name, val_x, stats_panel.y() + 55, 2, Color::RGB(255, 255, 255));

            self.draw_bitmap_text("SCORE:", label_x, stats_panel.y() + 80, 2, Color::RGB(200, 200, 200));
            self.draw_bitmap_text(&format!("{}", entry.score), val_x, stats_panel.y() + 80, 2, Color::RGB(0, 255, 150));
            
            let minutes = entry.time_seconds / 60;
            let seconds = entry.time_seconds % 60;
            self.draw_bitmap_text("TIME:", label_x, stats_panel.y() + 105, 2, Color::RGB(200, 200, 200));
            self.draw_bitmap_text(&format!("{:02}:{:02}", minutes, seconds), val_x, stats_panel.y() + 105, 2, Color::RGB(255, 255, 255));
            
            let diff_label = match entry.difficulty.as_str() {
                "normal" => "NORM",
                _ => "EASY",
            };
            let diff_color = match entry.difficulty.as_str() {
                "normal" => Color::RGB(255, 120, 100),
                _ => Color::RGB(100, 220, 100),
            };
            self.draw_bitmap_text("MODE:", label_x, stats_panel.y() + 130, 2, Color::RGB(200, 200, 200));
            self.draw_bitmap_text(diff_label, val_x, stats_panel.y() + 130, 2, diff_color);

            self.draw_bitmap_text("HINTS:", label_x, stats_panel.y() + 155, 2, Color::RGB(200, 200, 200));
            self.draw_bitmap_text(&format!("{}", entry.hints_used), val_x, stats_panel.y() + 155, 2, Color::RGB(255, 255, 255));

            self.draw_bitmap_text("SHUFFLES:", label_x, stats_panel.y() + 180, 2, Color::RGB(200, 200, 200));
            self.draw_bitmap_text(&format!("{}", entry.shuffles_used), val_x, stats_panel.y() + 180, 2, Color::RGB(255, 255, 255));

            self.draw_bitmap_text("UNDOS:", label_x, stats_panel.y() + 205, 2, Color::RGB(200, 200, 200));
            self.draw_bitmap_text(&format!("{}", entry.undos_used), val_x, stats_panel.y() + 205, 2, Color::RGB(255, 255, 255));

            // Trophy counters section
            self.draw_bitmap_text("TROPHY COUNTERS", label_x, stats_panel.y() + 245, 2, Color::RGB(0, 180, 255));

            self.draw_bitmap_text("STREAK:", label_x, stats_panel.y() + 275, 2, Color::RGB(200, 200, 200));
            self.draw_bitmap_text(&format!("{} DAYS", current_streak), val_x, stats_panel.y() + 275, 2, Color::RGB(255, 200, 0));

            self.draw_bitmap_text("PERFECT:", label_x, stats_panel.y() + 300, 2, Color::RGB(200, 200, 200));
            let pc_color = if trophy_state.perfect_combo_count > 0 { Color::RGB(255, 215, 0) } else { Color::RGB(100, 100, 100) };
            self.draw_bitmap_text(&format!("{}", trophy_state.perfect_combo_count), val_x, stats_panel.y() + 300, 2, pc_color);

            self.draw_bitmap_text("RAPID:", label_x, stats_panel.y() + 325, 2, Color::RGB(200, 200, 200));
            let rc_color = if trophy_state.rapid_clear_count > 0 { Color::RGB(255, 215, 0) } else { Color::RGB(100, 100, 100) };
            self.draw_bitmap_text(&format!("{}", trophy_state.rapid_clear_count), val_x, stats_panel.y() + 325, 2, rc_color);

            self.draw_bitmap_text("NO HINTS:", label_x, stats_panel.y() + 350, 2, Color::RGB(200, 200, 200));
            let nh_color = if trophy_state.no_hints_count > 0 { Color::RGB(255, 215, 0) } else { Color::RGB(100, 100, 100) };
            self.draw_bitmap_text(&format!("{}", trophy_state.no_hints_count), val_x, stats_panel.y() + 350, 2, nh_color);

            self.draw_bitmap_text("NO SHUFFLES:", label_x, stats_panel.y() + 375, 2, Color::RGB(200, 200, 200));
            let ns_color = if trophy_state.no_shuffles_count > 0 { Color::RGB(255, 215, 0) } else { Color::RGB(100, 100, 100) };
            self.draw_bitmap_text(&format!("{}", trophy_state.no_shuffles_count), val_x, stats_panel.y() + 375, 2, ns_color);

            self.draw_bitmap_text("NO UNDOS:", label_x, stats_panel.y() + 400, 2, Color::RGB(200, 200, 200));
            let nu_color = if trophy_state.no_undos_count > 0 { Color::RGB(255, 215, 0) } else { Color::RGB(100, 100, 100) };
            self.draw_bitmap_text(&format!("{}", trophy_state.no_undos_count), val_x, stats_panel.y() + 400, 2, nu_color);

            (entry.hints_used, entry.shuffles_used, entry.undos_used, current_streak)
        } else {
            let label_x = stats_panel.x() + 20;
            let val_x = stats_panel.x() + 180;

            self.draw_bitmap_text("LAST MATCH STATS", stats_panel.x() + 20, stats_panel.y() + 20, 2, Color::RGB(0, 180, 255));
            self.draw_bitmap_text("NO MATCHES YET", stats_panel.x() + 20, stats_panel.y() + 75, 2, Color::RGB(150, 150, 150));
            self.draw_bitmap_text("WIN A GAME", stats_panel.x() + 20, stats_panel.y() + 120, 2, Color::RGB(255, 255, 255));
            self.draw_bitmap_text("TO EARN TROPHIES!", stats_panel.x() + 20, stats_panel.y() + 145, 2, Color::RGB(255, 255, 255));
            
            // Trophy counters section
            self.draw_bitmap_text("TROPHY COUNTERS", label_x, stats_panel.y() + 245, 2, Color::RGB(0, 180, 255));

            self.draw_bitmap_text("STREAK:", label_x, stats_panel.y() + 275, 2, Color::RGB(200, 200, 200));
            self.draw_bitmap_text(&format!("{} DAYS", current_streak), val_x, stats_panel.y() + 275, 2, Color::RGB(255, 200, 0));

            self.draw_bitmap_text("PERFECT:", label_x, stats_panel.y() + 300, 2, Color::RGB(200, 200, 200));
            self.draw_bitmap_text("0", val_x, stats_panel.y() + 300, 2, Color::RGB(100, 100, 100));

            self.draw_bitmap_text("RAPID:", label_x, stats_panel.y() + 325, 2, Color::RGB(200, 200, 200));
            self.draw_bitmap_text("0", val_x, stats_panel.y() + 325, 2, Color::RGB(100, 100, 100));

            self.draw_bitmap_text("NO HINTS:", label_x, stats_panel.y() + 350, 2, Color::RGB(200, 200, 200));
            self.draw_bitmap_text("0", val_x, stats_panel.y() + 350, 2, Color::RGB(100, 100, 100));

            self.draw_bitmap_text("NO SHUFFLES:", label_x, stats_panel.y() + 375, 2, Color::RGB(200, 200, 200));
            self.draw_bitmap_text("0", val_x, stats_panel.y() + 375, 2, Color::RGB(100, 100, 100));

            self.draw_bitmap_text("NO UNDOS:", label_x, stats_panel.y() + 400, 2, Color::RGB(200, 200, 200));
            self.draw_bitmap_text("0", val_x, stats_panel.y() + 400, 2, Color::RGB(100, 100, 100));

            (1, 1, 1, current_streak) // default locked values
        };

        // Render Trophies in the Cabinet
        self.draw_bitmap_text("TROPHY SHELF", cabinet_panel.x() + 20, cabinet_panel.y() + 20, 2, Color::RGB(0, 180, 255));

        // Top Shelf: Perfect Combo + Daily Streak + Rapid Clear
        let trophy_size = 100;
        let top_trophy_y = top_shelf_y - trophy_size as i32;
        let t_combo_x = cabinet_panel.x() + 40;
        let t_streak_x = cabinet_panel.x() + (cabinet_panel.width() as i32 - trophy_size as i32) / 2;
        let t_rapid_x = cabinet_panel.x() + cabinet_panel.width() as i32 - 40 - trophy_size as i32;

        // Perfect Combo trophy (uses empty.png with number overlay)
        if trophy_state.perfect_combo_count > 0 {
            if let Some(ref mut tex) = self.trophy_empty {
                let _ = tex.set_alpha_mod(255);
                let _ = tex.set_color_mod(255, 255, 255);
                let dest = Rect::new(t_combo_x, top_trophy_y, trophy_size as u32, trophy_size as u32);
                self.canvas.copy(tex, None, dest).ok();
            }
            let count_str = format!("{}", trophy_state.perfect_combo_count);
            let num_scale: u32 = 5;
            let count_w = count_str.len() as i32 * 6 * num_scale as i32;
            let count_h = 7 * num_scale as i32;
            let num_x = t_combo_x + (trophy_size - count_w) / 2 + 4;
            let num_y = top_trophy_y + (trophy_size - count_h) / 2 + 2;
            // Bold outline (draw at all surrounding offsets for thickness)
            let outline_color = Color::RGB(40, 30, 0);
            for dx in -2..=2i32 {
                for dy in -1..=3i32 {
                    if dx != 0 || dy != 0 {
                        self.draw_bitmap_text(&count_str, num_x + dx, num_y + dy, num_scale, outline_color);
                    }
                }
            }
            // Mid shadow layer
            self.draw_bitmap_text(&count_str, num_x + 1, num_y + 1, num_scale, Color::RGB(100, 80, 0));
            // Main number
            self.draw_bitmap_text(&count_str, num_x, num_y, num_scale, Color::RGB(255, 255, 255));
        } else {
            if let Some(ref mut tex) = self.trophy_streak_zero {
                let _ = tex.set_alpha_mod(100);
                let _ = tex.set_color_mod(80, 80, 80);
                let dest = Rect::new(t_combo_x, top_trophy_y, trophy_size as u32, trophy_size as u32);
                self.canvas.copy(tex, None, dest).ok();
            }
        }
        let combo_color = if trophy_state.perfect_combo_count > 0 { Color::RGB(255, 215, 0) } else { Color::RGB(100, 110, 130) };
        self.draw_bitmap_text("PERFECT", t_combo_x + 22, top_shelf_y + 18, 1, combo_color);

        // Daily Play Streak Milestone Trophy (centered)
        let t_streak_y = top_shelf_y - trophy_size as i32;

        let streak_tex = if streak_days >= 365 {
            &mut self.trophy_streak_one_year
        } else if streak_days >= 270 {
            &mut self.trophy_streak_nine_months
        } else if streak_days >= 180 {
            &mut self.trophy_streak_six_months
        } else if streak_days >= 90 {
            &mut self.trophy_streak_three_months
        } else if streak_days >= 60 {
            &mut self.trophy_streak_sixty_days
        } else if streak_days >= 30 {
            &mut self.trophy_streak_thirty_days
        } else if streak_days >= 14 {
            &mut self.trophy_streak_fourteen_days
        } else if streak_days >= 7 {
            &mut self.trophy_streak_seven_days
        } else if streak_days >= 2 {
            &mut self.trophy_streak_two_days
        } else if streak_days >= 1 {
            &mut self.trophy_streak_one_day
        } else {
            &mut self.trophy_streak_zero
        };

        if let Some(ref mut tex) = streak_tex {
            let _ = tex.set_alpha_mod(255);
            let _ = tex.set_color_mod(255, 255, 255);
            let dest = Rect::new(t_streak_x, t_streak_y, trophy_size as u32, trophy_size as u32);
            self.canvas.copy(tex, None, dest).ok();
        }
        self.draw_bitmap_text("STREAK", t_streak_x + 28, top_shelf_y + 18, 1, Color::RGB(255, 200, 0));

        // Rapid Clear trophy (uses empty.png with number overlay)
        if trophy_state.rapid_clear_count > 0 {
            if let Some(ref mut tex) = self.trophy_empty {
                let _ = tex.set_alpha_mod(255);
                let _ = tex.set_color_mod(255, 255, 255);
                let dest = Rect::new(t_rapid_x, top_trophy_y, trophy_size as u32, trophy_size as u32);
                self.canvas.copy(tex, None, dest).ok();
            }
            let count_str = format!("{}", trophy_state.rapid_clear_count);
            let num_scale: u32 = 5;
            let count_w = count_str.len() as i32 * 6 * num_scale as i32;
            let count_h = 7 * num_scale as i32;
            let num_x = t_rapid_x + (trophy_size - count_w) / 2 + 4;
            let num_y = top_trophy_y + (trophy_size - count_h) / 2 + 2;
            // Bold outline (draw at all surrounding offsets for thickness)
            let outline_color = Color::RGB(40, 30, 0);
            for dx in -2..=2i32 {
                for dy in -1..=3i32 {
                    if dx != 0 || dy != 0 {
                        self.draw_bitmap_text(&count_str, num_x + dx, num_y + dy, num_scale, outline_color);
                    }
                }
            }
            // Mid shadow layer
            self.draw_bitmap_text(&count_str, num_x + 1, num_y + 1, num_scale, Color::RGB(100, 80, 0));
            // Main number
            self.draw_bitmap_text(&count_str, num_x, num_y, num_scale, Color::RGB(255, 255, 255));
        } else {
            if let Some(ref mut tex) = self.trophy_streak_zero {
                let _ = tex.set_alpha_mod(100);
                let _ = tex.set_color_mod(80, 80, 80);
                let dest = Rect::new(t_rapid_x, top_trophy_y, trophy_size as u32, trophy_size as u32);
                self.canvas.copy(tex, None, dest).ok();
            }
        }
        let rapid_color = if trophy_state.rapid_clear_count > 0 { Color::RGB(255, 215, 0) } else { Color::RGB(100, 110, 130) };
        self.draw_bitmap_text("RAPID", t_rapid_x + 28, top_shelf_y + 18, 1, rapid_color);

        // Bottom Shelf: Run Trophies
        let bottom_trophy_y = bottom_shelf_y - trophy_size as i32;
        let t1_x = cabinet_panel.x() + 40;
        let t2_x = cabinet_panel.x() + (cabinet_panel.width() as i32 - trophy_size as i32) / 2;
        let t3_x = cabinet_panel.x() + cabinet_panel.width() as i32 - 40 - trophy_size as i32;

        // Trophy 1: NO HINTS (cumulative counter)
        let nh_count = trophy_state.no_hints_count;
        if nh_count > 0 {
            if let Some(ref mut tex) = self.trophy_empty {
                let _ = tex.set_alpha_mod(255);
                let _ = tex.set_color_mod(255, 255, 255);
                let dest = Rect::new(t1_x, bottom_trophy_y, trophy_size as u32, trophy_size as u32);
                self.canvas.copy(tex, None, dest).ok();
            }
            let count_str = format!("{}", nh_count);
            let num_scale: u32 = 5;
            let count_w = count_str.len() as i32 * 6 * num_scale as i32;
            let count_h = 7 * num_scale as i32;
            let num_x = t1_x + (trophy_size - count_w) / 2 + 4;
            let num_y = bottom_trophy_y + (trophy_size - count_h) / 2 + 2;
            let outline_color = Color::RGB(40, 30, 0);
            for dx in -2..=2i32 {
                for dy in -1..=3i32 {
                    if dx != 0 || dy != 0 {
                        self.draw_bitmap_text(&count_str, num_x + dx, num_y + dy, num_scale, outline_color);
                    }
                }
            }
            self.draw_bitmap_text(&count_str, num_x + 1, num_y + 1, num_scale, Color::RGB(100, 80, 0));
            self.draw_bitmap_text(&count_str, num_x, num_y, num_scale, Color::RGB(255, 255, 255));
        } else {
            if let Some(ref mut tex) = self.trophy_streak_zero {
                let _ = tex.set_alpha_mod(100);
                let _ = tex.set_color_mod(80, 80, 80);
                let dest = Rect::new(t1_x, bottom_trophy_y, trophy_size as u32, trophy_size as u32);
                self.canvas.copy(tex, None, dest).ok();
            }
        }
        let t1_color = if nh_count > 0 { Color::RGB(255, 215, 0) } else { Color::RGB(100, 110, 130) };
        self.draw_bitmap_text("NO HINTS", t1_x + 20, bottom_shelf_y + 18, 1, t1_color);

        // Trophy 2: NO SHUFFLES (cumulative counter)
        let ns_count = trophy_state.no_shuffles_count;
        if ns_count > 0 {
            if let Some(ref mut tex) = self.trophy_empty {
                let _ = tex.set_alpha_mod(255);
                let _ = tex.set_color_mod(255, 255, 255);
                let dest = Rect::new(t2_x, bottom_trophy_y, trophy_size as u32, trophy_size as u32);
                self.canvas.copy(tex, None, dest).ok();
            }
            let count_str = format!("{}", ns_count);
            let num_scale: u32 = 5;
            let count_w = count_str.len() as i32 * 6 * num_scale as i32;
            let count_h = 7 * num_scale as i32;
            let num_x = t2_x + (trophy_size - count_w) / 2 + 4;
            let num_y = bottom_trophy_y + (trophy_size - count_h) / 2 + 2;
            let outline_color = Color::RGB(40, 30, 0);
            for dx in -2..=2i32 {
                for dy in -1..=3i32 {
                    if dx != 0 || dy != 0 {
                        self.draw_bitmap_text(&count_str, num_x + dx, num_y + dy, num_scale, outline_color);
                    }
                }
            }
            self.draw_bitmap_text(&count_str, num_x + 1, num_y + 1, num_scale, Color::RGB(100, 80, 0));
            self.draw_bitmap_text(&count_str, num_x, num_y, num_scale, Color::RGB(255, 255, 255));
        } else {
            if let Some(ref mut tex) = self.trophy_streak_zero {
                let _ = tex.set_alpha_mod(100);
                let _ = tex.set_color_mod(80, 80, 80);
                let dest = Rect::new(t2_x, bottom_trophy_y, trophy_size as u32, trophy_size as u32);
                self.canvas.copy(tex, None, dest).ok();
            }
        }
        let t2_color = if ns_count > 0 { Color::RGB(255, 215, 0) } else { Color::RGB(100, 110, 130) };
        self.draw_bitmap_text("NO SHUFFLE", t2_x + 14, bottom_shelf_y + 18, 1, t2_color);

        // Trophy 3: NO UNDOS (cumulative counter)
        let nu_count = trophy_state.no_undos_count;
        if nu_count > 0 {
            if let Some(ref mut tex) = self.trophy_empty {
                let _ = tex.set_alpha_mod(255);
                let _ = tex.set_color_mod(255, 255, 255);
                let dest = Rect::new(t3_x, bottom_trophy_y, trophy_size as u32, trophy_size as u32);
                self.canvas.copy(tex, None, dest).ok();
            }
            let count_str = format!("{}", nu_count);
            let num_scale: u32 = 5;
            let count_w = count_str.len() as i32 * 6 * num_scale as i32;
            let count_h = 7 * num_scale as i32;
            let num_x = t3_x + (trophy_size - count_w) / 2 + 4;
            let num_y = bottom_trophy_y + (trophy_size - count_h) / 2 + 2;
            let outline_color = Color::RGB(40, 30, 0);
            for dx in -2..=2i32 {
                for dy in -1..=3i32 {
                    if dx != 0 || dy != 0 {
                        self.draw_bitmap_text(&count_str, num_x + dx, num_y + dy, num_scale, outline_color);
                    }
                }
            }
            self.draw_bitmap_text(&count_str, num_x + 1, num_y + 1, num_scale, Color::RGB(100, 80, 0));
            self.draw_bitmap_text(&count_str, num_x, num_y, num_scale, Color::RGB(255, 255, 255));
        } else {
            if let Some(ref mut tex) = self.trophy_streak_zero {
                let _ = tex.set_alpha_mod(100);
                let _ = tex.set_color_mod(80, 80, 80);
                let dest = Rect::new(t3_x, bottom_trophy_y, trophy_size as u32, trophy_size as u32);
                self.canvas.copy(tex, None, dest).ok();
            }
        }
        let t3_color = if nu_count > 0 { Color::RGB(255, 215, 0) } else { Color::RGB(100, 110, 130) };
        self.draw_bitmap_text("NO UNDO", t3_x + 22, bottom_shelf_y + 18, 1, t3_color);

        // Back button
        let btn_w: u32 = 180;
        let btn_h: u32 = 40;
        let btn_x = dialog.x() + ((dialog.width() - btn_w) / 2) as i32;
        let btn_y = dialog.y() + dialog_h as i32 - 50;
        self.draw_labeled_button(btn_x, btn_y, btn_w, btn_h, Color::RGB(80, 80, 95), "BACK");
    }

    /// Renders a beautiful daily play streak achievement popup.
    pub fn render_daily_streak_popup(&mut self, streak: u32) {
        self.draw_overlay_backdrop();

        let dialog = self.draw_dialog_box(380, 240);

        // Neon cyan title
        let title = "DAILY ACHIEVEMENT";
        let title_w = title.len() as i32 * 6 * 2;
        self.draw_bitmap_text(
            title,
            dialog.x() + (dialog.width() as i32 - title_w) / 2,
            dialog.y() + 25,
            2,
            Color::RGB(0, 255, 200),
        );

        // Subtitle
        let subtitle = "CONSECUTIVE PLAY STREAK";
        let sub_w = subtitle.len() as i32 * 6 * 1;
        self.draw_bitmap_text(
            subtitle,
            dialog.x() + (dialog.width() as i32 - sub_w) / 2,
            dialog.y() + 75,
            1,
            Color::RGB(180, 180, 220),
        );

        // Day count
        let day_text = format!("DAY {}", streak);
        let text_w = day_text.len() as i32 * 6 * 4;
        let tx = dialog.x() + (dialog.width() as i32 - text_w) / 2;
        self.draw_bitmap_text(
            &day_text,
            tx,
            dialog.y() + 115,
            4,
            Color::RGB(255, 215, 0), // Gold
        );

        // Button
        let btn_w: u32 = 180;
        let btn_h: u32 = 40;
        let btn_x = dialog.x() + ((dialog.width() - btn_w) / 2) as i32;
        let btn_y = dialog.y() + 180;
        self.draw_labeled_button(btn_x, btn_y, btn_w, btn_h, Color::RGB(0, 150, 150), "AWESOME");
    }

    /// Renders the no-moves notification with options to shuffle or start a new game.
    ///
    /// Displayed when no valid pairs remain but tiles are still on the board.
    /// Options:
    /// - Shuffle (if shuffles remaining > 0)
    /// - New Game
    pub fn render_no_moves(&mut self, selected: usize) {
        self.draw_overlay_backdrop();

        let dialog = self.draw_dialog_box(320, 220);

        // "NO MOVES!" title
        self.draw_bitmap_text(
            "NO MOVES!",
            dialog.x() + 85,
            dialog.y() + 22,
            3,
            Color::RGB(255, 120, 80),
        );

        // Explanatory text
        self.draw_bitmap_text(
            "NO VALID PAIRS REMAIN",
            dialog.x() + 40,
            dialog.y() + 68,
            2,
            Color::RGB(180, 180, 200),
        );

        let btn_w: u32 = 200;
        let btn_h: u32 = 44;
        let btn_x = dialog.x() + ((dialog.width() - btn_w) / 2) as i32;

        let buttons: &[(i32, Color, &str)] = &[
            (110, Color::RGB(120, 60, 160), "SHUFFLE"),
            (164, Color::RGB(50, 140, 70), "NEW GAME"),
        ];

        for (i, (y_off, color, label)) in buttons.iter().enumerate() {
            let y = dialog.y() + y_off;
            self.draw_labeled_button(btn_x, y, btn_w, btn_h, *color, label);
            if i == selected {
                let sel_rect = Rect::new(btn_x - 2, y - 2, btn_w + 4, btn_h + 4);
                self.canvas.set_draw_color(Color::RGB(255, 255, 255));
                self.canvas.draw_rect(sel_rect).ok();
                let sel_rect_inner = Rect::new(btn_x - 1, y - 1, btn_w + 2, btn_h + 2);
                self.canvas.draw_rect(sel_rect_inner).ok();
            }
        }
    }

    /// Renders a semi-transparent hint suggestion banner over the game board.
    ///
    /// Shown after 120 seconds of inactivity (no clicks or pair matches) while Playing.
    /// The banner is non-intrusive: a translucent pill near the bottom of the screen.
    pub fn render_hint_suggestion(&mut self) {
        let (win_w, win_h) = self.window_size();

        // Semi-transparent backdrop pill centered near the bottom
        let text = "TRY SHIFT+H FOR HINT";
        let scale: u32 = 2;
        let char_w = 6 * scale; // each bitmap char is ~5px + 1px spacing, scaled
        let text_w = text.len() as u32 * char_w;
        let padding_x: u32 = 24;
        let padding_y: u32 = 14;
        let pill_w = text_w + padding_x * 2;
        let pill_h = 8 * scale + padding_y * 2;
        let pill_x = (win_w.saturating_sub(pill_w)) / 2;
        let pill_y = win_h.saturating_sub(pill_h + 60);

        let pill_rect = Rect::new(pill_x as i32, pill_y as i32, pill_w, pill_h);

        // Draw translucent dark background
        self.canvas.set_draw_color(Color::RGBA(20, 20, 40, 160));
        self.canvas.fill_rect(pill_rect).ok();

        // Draw subtle border
        self.canvas.set_draw_color(Color::RGBA(100, 180, 255, 140));
        self.canvas.draw_rect(pill_rect).ok();

        // Draw the text centered in the pill
        let text_x = pill_x as i32 + padding_x as i32;
        let text_y = pill_y as i32 + padding_y as i32;
        self.draw_bitmap_text(text, text_x, text_y, scale, Color::RGBA(180, 220, 255, 220));
    }

    /// Renders the game over dialog when no moves and no shuffles remain.
    ///
    /// Displays final stats (score, time, hints, shuffles, level) and offers:
    /// - Save Score (to enter name for leaderboard)
    /// - New Game
    pub fn render_game_over(&mut self, score: u32, time_seconds: u32, hints_used: u32, shuffles_used: u32, level: u32, selected: usize) {
        self.draw_overlay_backdrop();

        let dialog = self.draw_dialog_box(380, 374);

        // "GAME OVER" title (red)
        self.draw_bitmap_text(
            "GAME OVER",
            dialog.x() + 105,
            dialog.y() + 20,
            3,
            Color::RGB(255, 80, 80),
        );

        // Explanatory text
        self.draw_bitmap_text(
            "NO MOVES OR SHUFFLES LEFT",
            dialog.x() + 30,
            dialog.y() + 62,
            2,
            Color::RGB(180, 180, 200),
        );

        // Stats display
        let level_text = format!("LEVEL  {}", level);
        self.draw_bitmap_text(
            &level_text,
            dialog.x() + 50,
            dialog.y() + 100,
            2,
            Color::RGB(200, 200, 220),
        );

        let score_text = format!("SCORE  {}", score);
        self.draw_bitmap_text(
            &score_text,
            dialog.x() + 50,
            dialog.y() + 124,
            2,
            Color::RGB(255, 200, 50),
        );

        let minutes = time_seconds / 60;
        let seconds = time_seconds % 60;
        let time_text = format!("TIME   {:02}:{:02}", minutes, seconds);
        self.draw_bitmap_text(
            &time_text,
            dialog.x() + 50,
            dialog.y() + 148,
            2,
            Color::RGB(100, 220, 100),
        );

        let hints_text = format!("HINTS  {}", hints_used);
        self.draw_bitmap_text(
            &hints_text,
            dialog.x() + 50,
            dialog.y() + 172,
            2,
            Color::RGB(150, 180, 255),
        );

        let shuffles_text = format!("SHUFFLES  {}", shuffles_used);
        self.draw_bitmap_text(
            &shuffles_text,
            dialog.x() + 200,
            dialog.y() + 172,
            2,
            Color::RGB(180, 130, 255),
        );

        let btn_w: u32 = 220;
        let btn_h: u32 = 44;
        let btn_x = dialog.x() + ((dialog.width() - btn_w) / 2) as i32;

        let buttons: &[(i32, Color, &str)] = &[
            (210, Color::RGB(180, 140, 30), "SAVE SCORE"),
            (264, Color::RGB(50, 140, 70), "NEW GAME"),
            (318, Color::RGB(80, 100, 180), "WAIT FOR SHUFFLE"),
        ];

        for (i, (y_off, color, label)) in buttons.iter().enumerate() {
            let y = dialog.y() + y_off;
            self.draw_labeled_button(btn_x, y, btn_w, btn_h, *color, label);
            if i == selected {
                let sel_rect = Rect::new(btn_x - 2, y - 2, btn_w + 4, btn_h + 4);
                self.canvas.set_draw_color(Color::RGB(255, 255, 255));
                self.canvas.draw_rect(sel_rect).ok();
                let sel_rect_inner = Rect::new(btn_x - 1, y - 1, btn_w + 2, btn_h + 2);
                self.canvas.draw_rect(sel_rect_inner).ok();
            }
        }
    }

    /// Renders the name entry overlay for the leaderboard.
    ///
    /// Displays:
    /// - "High Score!" title
    /// - Score and time info
    /// - Text input field showing the current name
    /// - Instructions (Enter to submit, Esc to cancel)
    ///
    /// # Arguments
    /// * `name` - The current text in the name buffer
    /// * `score` - The qualifying score
    /// * `time_seconds` - The game completion time in seconds
    pub fn render_name_entry(&mut self, name: &str, score: u32, time_seconds: u32) {
        self.draw_overlay_backdrop();

        let dialog = self.draw_dialog_box(400, 280);

        let is_startup = score == 0 && time_seconds == 0;

        if is_startup {
            // Startup Username Entry
            self.draw_bitmap_text(
                "WELCOME TO XMAHJONG",
                dialog.x() + 45,
                dialog.y() + 24,
                3,
                Color::RGB(255, 215, 0),
            );

            self.draw_bitmap_text(
                "ENTER USERNAME",
                dialog.x() + 90,
                dialog.y() + 75,
                2,
                Color::RGB(200, 200, 220),
            );
        } else {
            // "HIGH SCORE!" title (gold, large)
            self.draw_bitmap_text(
                "HIGH SCORE!",
                dialog.x() + 112,
                dialog.y() + 18,
                3,
                Color::RGB(255, 215, 0),
            );

            // Score display
            let score_text = format!("SCORE  {}", score);
            self.draw_bitmap_text(
                &score_text,
                dialog.x() + 130,
                dialog.y() + 58,
                2,
                Color::RGB(255, 200, 50),
            );

            // Time display
            let minutes = time_seconds / 60;
            let seconds = time_seconds % 60;
            let time_text = format!("TIME  {:02}:{:02}", minutes, seconds);
            self.draw_bitmap_text(
                &time_text,
                dialog.x() + 130,
                dialog.y() + 82,
                2,
                Color::RGB(100, 200, 100),
            );

            // "ENTER YOUR NAME" label
            self.draw_bitmap_text(
                "ENTER YOUR NAME",
                dialog.x() + 40,
                dialog.y() + 118,
                2,
                Color::RGB(200, 200, 220),
            );
        }

        // Text input field background
        let input_x = dialog.x() + 40;
        let input_y = dialog.y() + 148;
        let input_w: u32 = 320;
        let input_h: u32 = 36;
        let input_rect = Rect::new(input_x, input_y, input_w, input_h);

        // Input field background (dark)
        self.canvas.set_draw_color(Color::RGB(25, 25, 35));
        self.canvas.fill_rect(input_rect).ok();

        // Input field border (lighter when focused)
        self.canvas.set_draw_color(Color::RGB(100, 160, 220));
        self.canvas.draw_rect(input_rect).ok();

        // Render the typed name as bitmap text inside the input field
        if !name.is_empty() {
            // Convert to uppercase for bitmap font consistency
            let display_name: String = name.to_uppercase();
            self.draw_bitmap_text(
                &display_name,
                input_x + 6,
                input_y + 10,
                2,
                Color::RGB(220, 220, 240),
            );
        }

        // Blinking cursor (solid bar after text)
        let char_width = 12i32; // 6px * scale(2) = 12px per char
        let cursor_x = input_x + 6 + (name.chars().count() as i32 * char_width).min((input_w as i32) - 14);
        let cursor_rect = Rect::new(cursor_x, input_y + 8, 2, input_h - 16);
        self.canvas.set_draw_color(Color::RGB(200, 220, 255));
        self.canvas.fill_rect(cursor_rect).ok();

        // Instructions
        let instructions = if is_startup {
            "PRESS ENTER TO START GAME"
        } else {
            "ENTER TO SUBMIT  ESC TO SKIP"
        };
        let instr_x = if is_startup { dialog.x() + 50 } else { dialog.x() + 52 };
        self.draw_bitmap_text(
            instructions,
            instr_x,
            dialog.y() + 198,
            1,
            Color::RGB(140, 140, 160),
        );

        // Character count indicator (e.g., "3/20")
        let char_count = name.chars().count();
        let count_text = format!("{}/20", char_count);
        let count_color = if char_count == 0 {
            Color::RGB(200, 80, 80) // Red if empty
        } else {
            Color::RGB(100, 200, 100) // Green if valid
        };
        self.draw_bitmap_text(
            &count_text,
            dialog.x() + 320,
            dialog.y() + 248,
            1,
            count_color,
        );
    }

    /// Renders the update available dialog.
    ///
    /// Shows the user that a newer version is available with options:
    /// - Download (green) - opens the releases page
    /// - Not Now (gray) - dismisses the dialog
    pub fn render_update_dialog(&mut self, current_version: &str, latest_version: &str) {
        self.draw_overlay_backdrop();

        let dialog = self.draw_dialog_box(360, 200);

        // "UPDATE AVAILABLE" title
        self.draw_bitmap_text(
            "UPDATE AVAILABLE",
            dialog.x() + 68,
            dialog.y() + 20,
            3,
            Color::RGB(100, 220, 100),
        );

        // Current version line
        let current_line = format!("CURRENT: V{}", current_version);
        self.draw_bitmap_text(
            &current_line,
            dialog.x() + 40,
            dialog.y() + 64,
            2,
            Color::RGB(180, 180, 200),
        );

        // Latest version line
        let latest_line = format!("LATEST:  V{}", latest_version);
        self.draw_bitmap_text(
            &latest_line,
            dialog.x() + 40,
            dialog.y() + 90,
            2,
            Color::RGB(220, 220, 255),
        );

        let btn_w: u32 = 140;
        let btn_h: u32 = 40;
        let btn_spacing: i32 = 20;

        // Center the two buttons horizontally
        let total_btn_width = (btn_w * 2) as i32 + btn_spacing;
        let btn_start_x = dialog.x() + (dialog.width() as i32 - total_btn_width) / 2;
        let btn_y = dialog.y() + 140;

        // Download button (green)
        self.draw_labeled_button(btn_start_x, btn_y, btn_w, btn_h, Color::RGB(40, 140, 60), "DOWNLOAD");

        // Not Now button (gray)
        self.draw_labeled_button(
            btn_start_x + btn_w as i32 + btn_spacing,
            btn_y,
            btn_w,
            btn_h,
            Color::RGB(100, 100, 110),
            "NOT NOW",
        );
    }

    /// Renders the quit confirmation dialog.
    ///
    /// Asks the player to confirm quitting the current game.
    /// Options:
    /// - Yes / Confirm (red)
    /// - No / Cancel (gray)
    pub fn render_quit_confirmation(&mut self) {
        self.draw_overlay_backdrop();

        let dialog = self.draw_dialog_box(300, 180);

        // "QUIT GAME?" title
        self.draw_bitmap_text(
            "QUIT GAME?",
            dialog.x() + 90,
            dialog.y() + 24,
            3,
            Color::RGB(255, 160, 160),
        );

        // "PROGRESS WILL BE LOST"
        self.draw_bitmap_text(
            "PROGRESS WILL BE LOST",
            dialog.x() + 30,
            dialog.y() + 64,
            2,
            Color::RGB(180, 180, 200),
        );

        let btn_w: u32 = 120;
        let btn_h: u32 = 40;
        let btn_spacing: i32 = 20;

        // Center the two buttons horizontally
        let total_btn_width = (btn_w * 2) as i32 + btn_spacing;
        let btn_start_x = dialog.x() + (dialog.width() as i32 - total_btn_width) / 2;
        let btn_y = dialog.y() + 110;

        // Yes button (red) with label
        self.draw_labeled_button(btn_start_x, btn_y, btn_w, btn_h, Color::RGB(180, 50, 50), "YES");

        // No button (gray) with label
        self.draw_labeled_button(
            btn_start_x + btn_w as i32 + btn_spacing,
            btn_y,
            btn_w,
            btn_h,
            Color::RGB(100, 100, 110),
            "NO",
        );
    }

    /// Draws the shadow rectangle for a tile block at the given layer.
    /// Shadow offset = layer * BASE_SHADOW_OFFSET_PX (scaled to window size).
    /// Shadow dimensions match the full tile block footprint (top_face + thickness).
    /// Only draws for layers > 0.
    fn draw_shadow(&mut self, dest: Rect, thickness: u32, layer: u8, metrics: &LayoutMetrics, alpha: u8) {
        // Only draw shadow for layers > 0
        if layer == 0 {
            return;
        }

        // Compute shadow offset: layer * BASE_SHADOW_OFFSET_PX * (tile_width / REFERENCE_TILE_WIDTH)
        let scale = metrics.tile_width / REFERENCE_TILE_WIDTH;
        let shadow_offset = (layer as f32 * BASE_SHADOW_OFFSET_PX * scale).round() as i32;

        // Shadow dimensions: top face + thickness in both directions
        let shadow_w = dest.width() + thickness;
        let shadow_h = dest.height() + thickness;

        // Shadow position: offset from dest (the top face rect)
        let shadow_rect = Rect::new(
            dest.x() + shadow_offset,
            dest.y() + shadow_offset,
            shadow_w,
            shadow_h,
        );

        // Use SHADOW_ALPHA modulated by the passed alpha
        let final_alpha = ((SHADOW_ALPHA as u16 * alpha as u16) / 255) as u8;
        self.canvas.set_draw_color(Color::RGBA(0, 0, 0, final_alpha));
        self.canvas.fill_rect(shadow_rect).ok();
    }

    /// Draws the corner junction quadrilateral (thickness × thickness square).
    /// Position: bottom-right corner intersection of the two side faces.
    /// Uses RIGHT_FACE_BRIGHTNESS for consistent appearance with the right side face.
    fn draw_corner_junction(&mut self, dest: Rect, thickness: u32, base_color: Color, alpha: u8) {
        let shaded = shade_color(base_color, RIGHT_FACE_BRIGHTNESS);
        let color = Color::RGBA(shaded.r, shaded.g, shaded.b, alpha);

        let junction_rect = Rect::new(
            dest.x() + dest.width() as i32,
            dest.y() + dest.height() as i32,
            thickness,
            thickness,
        );

        self.canvas.set_draw_color(color);
        self.canvas.fill_rect(junction_rect).ok();

        // Draw 1px dark border on outer edges
        let border_color = shade_color(base_color, 0.25);
        let border = Color::RGBA(border_color.r, border_color.g, border_color.b, alpha);
        self.canvas.set_draw_color(border);
        self.canvas.draw_rect(junction_rect).ok();
    }

    /// Draws the right side face with gradient shading.
    ///
    /// Position: vertical strip to the right of the top face (`dest`).
    /// Width = `thickness`, Height = top face height.
    /// Gradient: left edge uses `base_color` (already shaded by RIGHT_FACE_BRIGHTNESS),
    /// right edge is further darkened by SIDE_FACE_GRADIENT_DELTA / RIGHT_FACE_BRIGHTNESS.
    /// A 1px dark border is drawn on the outer edges (right, top, bottom).
    ///
    /// # Arguments
    /// * `dest` - The top face rectangle (used to compute side face position)
    /// * `thickness` - Width of the side face in pixels
    /// * `base_color` - Already-shaded base color (has RIGHT_FACE_BRIGHTNESS applied)
    /// * `alpha` - Alpha value to apply to all drawn colors (for removal animation)
    fn draw_right_side_face(
        &mut self,
        dest: Rect,
        thickness: u32,
        base_color: Color,
        alpha: u8,
    ) {
        if thickness == 0 {
            return;
        }

        let face_x = dest.x().saturating_add(dest.width() as i32);
        let face_y = dest.y();
        let face_height = dest.height();

        // Draw gradient columns from left (lighter) to right (darker)
        let max_col = thickness.max(1) - 1;
        for col in 0..thickness {
            let factor = if max_col == 0 {
                1.0_f32
            } else {
                1.0 - (col as f32 / max_col as f32)
                    * (SIDE_FACE_GRADIENT_DELTA / RIGHT_FACE_BRIGHTNESS)
            };

            let r = (base_color.r as f32 * factor).round().clamp(0.0, 255.0) as u8;
            let g = (base_color.g as f32 * factor).round().clamp(0.0, 255.0) as u8;
            let b = (base_color.b as f32 * factor).round().clamp(0.0, 255.0) as u8;
            self.canvas.set_draw_color(Color::RGBA(r, g, b, alpha));

            let col_rect = Rect::new(
                face_x.saturating_add(col as i32),
                face_y,
                1,
                face_height,
            );
            self.canvas.fill_rect(col_rect).ok();
        }

        // Draw 1px dark border on outer edges (right, top, bottom)
        // Border color: shade_color(base_color, 0.25) ensuring luminance ≤ 30% of base
        let border_color = shade_color(base_color, 0.25);
        let border_color = Color::RGBA(border_color.r, border_color.g, border_color.b, alpha);
        self.canvas.set_draw_color(border_color);

        // Right edge (vertical line at x = face_x + thickness - 1)
        let right_edge_x = face_x.saturating_add(thickness as i32 - 1);
        let right_line = Rect::new(right_edge_x, face_y, 1, face_height);
        self.canvas.fill_rect(right_line).ok();

        // Top edge (horizontal line at y = face_y, spanning the side face width)
        let top_line = Rect::new(face_x, face_y, thickness, 1);
        self.canvas.fill_rect(top_line).ok();

        // Bottom edge (horizontal line at y = face_y + face_height - 1)
        let bottom_edge_y = face_y.saturating_add(face_height as i32 - 1);
        let bottom_line = Rect::new(face_x, bottom_edge_y, thickness, 1);
        self.canvas.fill_rect(bottom_line).ok();
    }

    /// Draws the bottom side face with gradient shading.
    ///
    /// Position: horizontal strip below the bottom edge of the top face (`dest`).
    /// Width = top face width, Height = `thickness`.
    /// Gradient: top edge uses `base_color` (already shaded by BOTTOM_FACE_BRIGHTNESS),
    /// bottom edge is further darkened by SIDE_FACE_GRADIENT_DELTA / BOTTOM_FACE_BRIGHTNESS.
    /// A 1px dark border is drawn on the outer edges (bottom, left, right).
    ///
    /// # Arguments
    /// * `dest` - The top face rectangle (used to compute side face position)
    /// * `thickness` - Height of the side face in pixels
    /// * `base_color` - Already-shaded base color (has BOTTOM_FACE_BRIGHTNESS applied)
    /// * `alpha` - Alpha value to apply to all drawn colors (for removal animation)
    fn draw_bottom_side_face(
        &mut self,
        dest: Rect,
        thickness: u32,
        base_color: Color,
        alpha: u8,
    ) {
        if thickness == 0 {
            return;
        }

        let face_x = dest.x();
        let face_y = dest.y().saturating_add(dest.height() as i32);
        let face_width = dest.width();

        // Draw gradient rows from top (lighter) to bottom (darker)
        let max_row = thickness.max(1) - 1;
        for row in 0..thickness {
            let factor = if max_row == 0 {
                1.0_f32
            } else {
                1.0 - (row as f32 / max_row as f32)
                    * (SIDE_FACE_GRADIENT_DELTA / BOTTOM_FACE_BRIGHTNESS)
            };

            let r = (base_color.r as f32 * factor).round().clamp(0.0, 255.0) as u8;
            let g = (base_color.g as f32 * factor).round().clamp(0.0, 255.0) as u8;
            let b = (base_color.b as f32 * factor).round().clamp(0.0, 255.0) as u8;
            self.canvas.set_draw_color(Color::RGBA(r, g, b, alpha));

            let row_rect = Rect::new(
                face_x,
                face_y.saturating_add(row as i32),
                face_width,
                1,
            );
            self.canvas.fill_rect(row_rect).ok();
        }

        // Draw 1px dark border on outer edges (bottom, left, right)
        // Border color: shade_color(base_color, 0.25) ensuring luminance ≤ 30% of base
        let border_color = shade_color(base_color, 0.25);
        let border_color = Color::RGBA(border_color.r, border_color.g, border_color.b, alpha);
        self.canvas.set_draw_color(border_color);

        // Bottom edge (horizontal line at y = face_y + thickness - 1)
        let bottom_edge_y = face_y.saturating_add(thickness as i32 - 1);
        let bottom_line = Rect::new(face_x, bottom_edge_y, face_width, 1);
        self.canvas.fill_rect(bottom_line).ok();

        // Left edge (vertical line at x = face_x, spanning the side face height)
        let left_line = Rect::new(face_x, face_y, 1, thickness);
        self.canvas.fill_rect(left_line).ok();

        // Right edge (vertical line at x = face_x + face_width - 1)
        let right_edge_x = face_x.saturating_add(face_width as i32 - 1);
        let right_line = Rect::new(right_edge_x, face_y, 1, thickness);
        self.canvas.fill_rect(right_line).ok();
    }

    fn draw_thick_line(&mut self, p1: Point, p2: Point, width: i32, color: Color) {
        let dx = (p2.x - p1.x) as f32;
        let dy = (p2.y - p1.y) as f32;
        let length = (dx * dx + dy * dy).sqrt();
        if length < 0.1 {
            return;
        }
        let nx = -dy / length;
        let ny = dx / length;

        self.canvas.set_draw_color(color);
        let half_w = width / 2;
        for i in -half_w..=half_w {
            let offset_x = (nx * i as f32).round() as i32;
            let offset_y = (ny * i as f32).round() as i32;
            let start = Point::new(p1.x + offset_x, p1.y + offset_y);
            let end = Point::new(p2.x + offset_x, p2.y + offset_y);
            let _ = self.canvas.draw_line(start, end);
        }
    }

    fn draw_circle(&mut self, center: Point, radius: i32) {
        if radius <= 0 {
            return;
        }
        let mut x = radius;
        let mut y = 0;
        let mut error = 1 - x;

        while x >= y {
            let pts = [
                Point::new(center.x + x, center.y + y),
                Point::new(center.x + y, center.y + x),
                Point::new(center.x - y, center.y + x),
                Point::new(center.x - x, center.y + y),
                Point::new(center.x - x, center.y - y),
                Point::new(center.x - y, center.y - x),
                Point::new(center.x + y, center.y - x),
                Point::new(center.x + x, center.y - y),
            ];
            for &pt in &pts {
                let _ = self.canvas.draw_point(pt);
            }
            y += 1;
            if error < 0 {
                error += 2 * y + 1;
            } else {
                x -= 1;
                error += 2 * (y - x) + 1;
            }
        }
    }

    fn draw_lightning_bolt(
        &mut self,
        start: Point,
        end: Point,
        _progress: f32,
        _seed: u64,
        glow_color: Color,
        core_color: Color,
        width: i32,
    ) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let num_segments = 8;
        let mut points = Vec::with_capacity(num_segments + 1);
        points.push(start);

        let dx = (end.x - start.x) as f32;
        let dy = (end.y - start.y) as f32;
        let length = (dx * dx + dy * dy).sqrt();
        if length < 5.0 {
            return;
        }

        let nx = -dy / length;
        let ny = dx / length;

        let max_displacement = (length * 0.15).min(40.0).max(8.0);

        for i in 1..num_segments {
            let t = i as f32 / num_segments as f32;
            let base_x = start.x as f32 + dx * t;
            let base_y = start.y as f32 + dy * t;

            let envelope = 4.0 * t * (1.0 - t);

            let disp = rng.gen_range(-max_displacement..max_displacement) * envelope;
            let px = (base_x + nx * disp).round() as i32;
            let py = (base_y + ny * disp).round() as i32;
            points.push(Point::new(px, py));
        }
        points.push(end);

        self.canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
        for i in 0..num_segments {
            let p_start = points[i];
            let p_end = points[i + 1];

            self.draw_thick_line(p_start, p_end, width * 3, glow_color);
            self.draw_thick_line(p_start, p_end, width, core_color);
        }
    }

    fn render_lightnings(&mut self, state: &GameState, now: Instant) {
        let (win_w, win_h) = self.window_size();
        let metrics = compute_layout_rect(win_w, win_h);

        for anim in &state.animations {
            if let Animation::Lightning { positions, start_time, duration_ms } = anim {
                let elapsed_ms = now.duration_since(*start_time).as_millis() as u32;
                if elapsed_ms >= *duration_ms {
                    continue;
                }

                let progress = elapsed_ms as f32 / *duration_ms as f32;

                let pos_a = &state.board.layout.positions[positions.0];
                let pos_b = &state.board.layout.positions[positions.1];
                let rect_a = tile_screen_rect(pos_a, &metrics);
                let rect_b = tile_screen_rect(pos_b, &metrics);

                let start_a = Point::new(
                    rect_a.x() + rect_a.width() as i32 / 2,
                    rect_a.y() + rect_a.height() as i32 / 2,
                );
                let start_b = Point::new(
                    rect_b.x() + rect_b.width() as i32 / 2,
                    rect_b.y() + rect_b.height() as i32 / 2,
                );

                // Calculate the midpoint between the two matched tiles
                let midpoint = Point::new(
                    (start_a.x + start_b.x) / 2,
                    (start_a.y + start_b.y) / 2,
                );

                let propagation = (progress / 0.35).min(1.0);

                let target_a = Point::new(
                    (start_a.x as f32 + (midpoint.x - start_a.x) as f32 * propagation) as i32,
                    (start_a.y as f32 + (midpoint.y - start_a.y) as f32 * propagation) as i32,
                );
                let target_b = Point::new(
                    (start_b.x as f32 + (midpoint.x - start_b.x) as f32 * propagation) as i32,
                    (start_b.y as f32 + (midpoint.y - start_b.y) as f32 * propagation) as i32,
                );

                let alpha_val = if progress > 0.8 {
                    ((1.0 - (progress - 0.8) / 0.2) * 255.0) as u8
                } else {
                    255
                };

                let color_pairs = [
                    (Color::RGBA(0, 255, 255, alpha_val), Color::RGBA(220, 255, 255, alpha_val)),
                    (Color::RGBA(255, 0, 255, alpha_val), Color::RGBA(255, 220, 255, alpha_val)),
                    (Color::RGBA(255, 215, 0, alpha_val), Color::RGBA(255, 255, 220, alpha_val)),
                ];

                for (glow, core) in &color_pairs {
                    self.draw_lightning_bolt(start_a, target_a, progress, 0, *glow, *core, 2);
                }

                for (glow, core) in &color_pairs {
                    self.draw_lightning_bolt(start_b, target_b, progress, 0, *glow, *core, 2);
                }

                if progress >= 0.3 {
                    let burst_progress = (progress - 0.3) / 0.7;
                    let max_radius = 45.0; // Slightly smaller burst radius since it's localized
                    let current_radius = (burst_progress * max_radius) as i32;

                    self.canvas.set_blend_mode(sdl2::render::BlendMode::Blend);

                    let ring_alpha = ((1.0 - burst_progress) * alpha_val as f32) as u8;

                    self.canvas.set_draw_color(Color::RGBA(0, 255, 255, ring_alpha / 2));
                    self.draw_circle(midpoint, current_radius);
                    self.draw_circle(midpoint, current_radius - 1);

                    self.canvas.set_draw_color(Color::RGBA(255, 0, 255, ring_alpha / 3));
                    self.draw_circle(midpoint, (current_radius * 4 / 5) as i32);

                    self.canvas.set_draw_color(Color::RGBA(255, 215, 0, ring_alpha / 4));
                    self.draw_circle(midpoint, (current_radius * 3 / 5) as i32);

                    let num_sparks = 12;
                    let spark_len = 12.0 * (1.0 - burst_progress);
                    let inner_r = current_radius as f32;
                    let outer_r = inner_r + spark_len;

                    for i in 0..num_sparks {
                        let angle = (i as f32 * 2.0 * std::f32::consts::PI) / num_sparks as f32;
                        let cos = angle.cos();
                        let sin = angle.sin();

                        let p1 = Point::new(
                            (midpoint.x as f32 + inner_r * cos) as i32,
                            (midpoint.y as f32 + inner_r * sin) as i32,
                        );
                        let p2 = Point::new(
                            (midpoint.x as f32 + outer_r * cos) as i32,
                            (midpoint.y as f32 + outer_r * sin) as i32,
                        );

                        let spark_color = match i % 3 {
                            0 => Color::RGBA(0, 255, 255, ring_alpha),
                            1 => Color::RGBA(255, 0, 255, ring_alpha),
                            _ => Color::RGBA(255, 215, 0, ring_alpha),
                        };

                        self.draw_thick_line(p1, p2, 2, spark_color);
                    }
                }
            }
        }
    }
}

/// Returns a 5×7 bitmap glyph for a given character.
/// Each element is a u8 where bits 4..0 represent pixels left-to-right.
/// Returns None for unsupported characters.
fn bitmap_glyph(ch: char) -> Option<&'static [u8; 7]> {
    match ch.to_ascii_uppercase() {
        'A' => Some(&[0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
        'B' => Some(&[0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110]),
        'C' => Some(&[0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110]),
        'D' => Some(&[0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110]),
        'E' => Some(&[0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111]),
        'F' => Some(&[0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000]),
        'G' => Some(&[0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110]),
        'H' => Some(&[0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
        'I' => Some(&[0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
        'J' => Some(&[0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100]),
        'K' => Some(&[0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001]),
        'L' => Some(&[0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111]),
        'M' => Some(&[0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001]),
        'N' => Some(&[0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001]),
        'O' => Some(&[0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
        'P' => Some(&[0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000]),
        'Q' => Some(&[0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101]),
        'R' => Some(&[0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001]),
        'S' => Some(&[0b01110, 0b10001, 0b10000, 0b01110, 0b00001, 0b10001, 0b01110]),
        'T' => Some(&[0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100]),
        'U' => Some(&[0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
        'V' => Some(&[0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100]),
        'W' => Some(&[0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001]),
        'X' => Some(&[0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001]),
        'Y' => Some(&[0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100]),
        'Z' => Some(&[0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111]),
        '0' => Some(&[0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110]),
        '1' => Some(&[0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
        '2' => Some(&[0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111]),
        '3' => Some(&[0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110]),
        '4' => Some(&[0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010]),
        '5' => Some(&[0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110]),
        '6' => Some(&[0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110]),
        '7' => Some(&[0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000]),
        '8' => Some(&[0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110]),
        '9' => Some(&[0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110]),
        ' ' => Some(&[0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000]),
        ':' => Some(&[0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000]),
        '?' => Some(&[0b01110, 0b10001, 0b00001, 0b00110, 0b00100, 0b00000, 0b00100]),
        '!' => Some(&[0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100]),
        '-' => Some(&[0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000]),
        '/' => Some(&[0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000]),
        '+' => Some(&[0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000]),
        '.' => Some(&[0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00100]),
        ',' => Some(&[0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b01000]),
        '(' => Some(&[0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010]),
        ')' => Some(&[0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000]),
        _ => None,
    }
}

/// Converts HSV color values to RGB.
/// Hue in [0, 360), Saturation in [0, 1], Value in [0, 1].
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r1, g1, b1) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    (
        ((r1 + m) * 255.0) as u8,
        ((g1 + m) * 255.0) as u8,
        ((b1 + m) * 255.0) as u8,
    )
}

/// Computes the side face color given a base color and a brightness factor.
/// Applies per-channel scaling: channel * factor, rounded to nearest integer, clamped to [0, 255].
/// The alpha channel is preserved unchanged.
pub fn shade_color(color: Color, factor: f32) -> Color {
    let r = (color.r as f32 * factor).round().clamp(0.0, 255.0) as u8;
    let g = (color.g as f32 * factor).round().clamp(0.0, 255.0) as u8;
    let b = (color.b as f32 * factor).round().clamp(0.0, 255.0) as u8;
    Color::RGBA(r, g, b, color.a)
}

/// Computes highlight-aware base color for side faces.
/// Maps TileHighlight → appropriate base color for the side faces.
/// The returned color is the "raw" base before brightness reduction is applied.
pub fn side_face_base_color(highlight: &TileHighlight, default_back: Color) -> Color {
    match highlight {
        TileHighlight::None => default_back,
        TileHighlight::Selected => Color::RGB(255, 215, 0), // Gold
        TileHighlight::HintGlow(phase) => {
            // Pulsing glow: interpolate between default_back and bright cyan/glow,
            // matching the top-face highlight logic.
            let intensity = ((phase * std::f32::consts::PI * 2.0).sin() + 1.0) / 2.0;
            let r = (default_back.r as f32 + intensity * (255.0 - default_back.r as f32) * 0.1) as u8;
            let g = (default_back.g as f32 + intensity * (255.0 - default_back.g as f32) * 0.1) as u8;
            let b = (default_back.b as f32 + intensity * (255.0 - default_back.b as f32) * 0.22) as u8;
            Color::RGB(r, g, b)
        }
        TileHighlight::MismatchFlash => Color::RGB(255, 100, 100), // Red flash
        TileHighlight::Removing(_) => default_back, // Alpha handled separately
    }
}
