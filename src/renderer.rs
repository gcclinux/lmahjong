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
use crate::game_state::{Animation, GameState, NameEntryState};
use crate::i18n::{self, Language};
use crate::storage::{Leaderboard, ShuffleState, TrophyState, UserProgress};

/// Number of distinct tile face images per style.
const TILE_FACE_COUNT: usize = 50;

/// Total number of tile face textures (penguins + dogs + space + ocean).
const TOTAL_FACE_COUNT: usize = 200;

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

/// Returns the bounding Rect of the HUD mute button for hit-testing.
pub fn hud_mute_button_rect(win_w: u32) -> Rect {
    let btn_w = 34;
    let btn_h = 26;
    let btn_x = win_w as i32 - btn_w - 14;
    let btn_y = 7;
    Rect::new(btn_x, btn_y, btn_w as u32, btn_h as u32)
}

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

/// Icon types rendered inside HUD stat pills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudIcon {
    None,
    Tile,
    Heart,
    Trophy,
    Lightbulb,
    Clock,
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

        Ok(Self {
            canvas,
            texture_creator,
            ttf_context,
            tile_textures,
            background_texture,
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

        // Load ocean tiles (face IDs 150-199)
        for i in 0..TILE_FACE_COUNT {
            let path = format!("{}/ocean/face_{:02}.png", base, i);
            match texture_creator.load_texture(&path) {
                Ok(texture) => {
                    let texture: Texture<'static> = unsafe { std::mem::transmute(texture) };
                    textures.push(Some(texture));
                }
                Err(e) => {
                    textures.push(None);
                    eprintln!(
                        "[xMahjong] Warning: Ocean tile texture not found: '{}'. Using placeholder. ({})",
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

    /// Loads the best matching font for the given text (Tamil, Japanese, Chinese, or Latin)
    /// from the assets directory or system paths at the given point size.
    pub fn load_font_for_text(&self, text: &str, point_size: u16) -> Option<sdl2::ttf::Font<'_, 'static>> {
        let is_tamil = text.chars().any(|ch| ('\u{0B80}'..='\u{0BFF}').contains(&ch));
        let is_japanese = text.chars().any(|ch| ('\u{3040}'..='\u{309F}').contains(&ch) || ('\u{30A0}'..='\u{30FF}').contains(&ch));
        let is_cjk = text.chars().any(|ch| {
            ('\u{4E00}'..='\u{9FFF}').contains(&ch)
                || ('\u{3400}'..='\u{4DBF}').contains(&ch)
                || ('\u{F900}'..='\u{FAFF}').contains(&ch)
                || ('\u{3000}'..='\u{303F}').contains(&ch)
                || ('\u{FF00}'..='\u{FFEF}').contains(&ch)
        });

        let mut candidate_paths = Vec::new();
        let base = assets_path();
        candidate_paths.push(format!("{}/fonts/default.ttf", base));

        if is_tamil {
            candidate_paths.push("C:\\Windows\\Fonts\\Nirmala.ttc".to_string());
            candidate_paths.push("C:\\Windows\\Fonts\\NirmalaB.ttc".to_string());
            candidate_paths.push("C:\\Windows\\Fonts\\latha.ttf".to_string());
            candidate_paths.push("/usr/share/fonts/truetype/noto/NotoSansTamil-Regular.ttf".to_string());
            candidate_paths.push("/usr/share/fonts/opentype/noto/NotoSansTamil-Regular.otf".to_string());
            candidate_paths.push("/usr/share/fonts/truetype/lohit-tamil/Lohit-Tamil.ttf".to_string());
            candidate_paths.push("/System/Library/Fonts/Supplemental/Tamil Sangam MN.ttc".to_string());
            candidate_paths.push("/System/Library/Fonts/Supplemental/Tamil MN.ttc".to_string());
            candidate_paths.push("/System/Library/Fonts/Tamil MN.ttc".to_string());
        } else if is_japanese {
            candidate_paths.push("C:\\Windows\\Fonts\\msgothic.ttc".to_string());
            candidate_paths.push("C:\\Windows\\Fonts\\YuGothR.ttc".to_string());
            candidate_paths.push("C:\\Windows\\Fonts\\YuGothM.ttc".to_string());
            candidate_paths.push("C:\\Windows\\Fonts\\meiryo.ttc".to_string());
            candidate_paths.push("C:\\Windows\\Fonts\\msyh.ttc".to_string());
            candidate_paths.push("/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc".to_string());
            candidate_paths.push("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc".to_string());
            candidate_paths.push("/usr/share/fonts/truetype/takao-gothic/TakaoPGothic.ttf".to_string());
            candidate_paths.push("/System/Library/Fonts/Hiragino Sans GB.ttc".to_string());
        } else if is_cjk {
            candidate_paths.push("C:\\Windows\\Fonts\\msyh.ttc".to_string());
            candidate_paths.push("C:\\Windows\\Fonts\\msyhbd.ttc".to_string());
            candidate_paths.push("C:\\Windows\\Fonts\\simsun.ttc".to_string());
            candidate_paths.push("C:\\Windows\\Fonts\\simhei.ttf".to_string());
            candidate_paths.push("C:\\Windows\\Fonts\\msgothic.ttc".to_string());
            candidate_paths.push("/usr/share/fonts/truetype/noto/NotoSansSC-Regular.otf".to_string());
            candidate_paths.push("/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc".to_string());
            candidate_paths.push("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc".to_string());
            candidate_paths.push("/usr/share/fonts/truetype/wqy/wqy-microhei.ttc".to_string());
            candidate_paths.push("/System/Library/Fonts/PingFang.ttc".to_string());
            candidate_paths.push("/System/Library/Fonts/Hiragino Sans GB.ttc".to_string());
        }

        // Fallbacks
        candidate_paths.push("C:\\Windows\\Fonts\\msyh.ttc".to_string());
        candidate_paths.push("C:\\Windows\\Fonts\\msgothic.ttc".to_string());
        candidate_paths.push("C:\\Windows\\Fonts\\Nirmala.ttc".to_string());
        candidate_paths.push("C:\\Windows\\Fonts\\segoeui.ttf".to_string());
        candidate_paths.push("C:\\Windows\\Fonts\\arial.ttf".to_string());
        candidate_paths.push("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".to_string());
        candidate_paths.push("/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf".to_string());
        candidate_paths.push("/usr/share/fonts/TTF/DejaVuSans.ttf".to_string());
        candidate_paths.push("/System/Library/Fonts/SFNS.ttf".to_string());
        candidate_paths.push("/System/Library/Fonts/Supplemental/Arial.ttf".to_string());
        candidate_paths.push("/Library/Fonts/Arial.ttf".to_string());
        candidate_paths.push("/Library/Fonts/Arial Unicode.ttf".to_string());

        for path in &candidate_paths {
            if std::path::Path::new(path).exists() {
                if let Ok(font) = self.ttf_context.load_font(path, point_size) {
                    return Some(font);
                }
            }
        }
        None
    }

    /// Loads a font from the assets directory or system paths at the given point size.
    /// Returns None if no font file is found.
    pub fn load_font(&self, point_size: u16) -> Option<sdl2::ttf::Font<'_, 'static>> {
        self.load_font_for_text("", point_size)
    }

    /// Returns the pixel width of the text at the given scale factor.
    pub fn text_width(&self, text: &str, scale: u32) -> u32 {
        let has_non_bitmap = text.chars().any(|ch| bitmap_glyph(ch).is_none());
        if has_non_bitmap {
            let pt_size = (7 * scale).max(11) as u16;
            if let Some(font) = self.load_font_for_text(text, pt_size) {
                if let Ok((w, _)) = font.size_of(text) {
                    return w;
                }
            }
        }
        text.chars().count() as u32 * 6 * scale
    }

    /// Draws text using TTF font if non-bitmap characters are present, otherwise uses bitmap font.
    pub fn draw_text_rendered(&mut self, text: &str, x: i32, y: i32, scale: u32, color: Color) {
        if text.is_empty() {
            return;
        }
        let has_non_bitmap = text.chars().any(|ch| bitmap_glyph(ch).is_none());
        if has_non_bitmap {
            let pt_size = (7 * scale).max(11) as u16;
            let surface = if let Some(font) = self.load_font_for_text(text, pt_size) {
                font.render(text).blended(color).ok()
            } else {
                None
            };

            if let Some(surface) = surface {
                if let Ok(texture) = self.texture_creator.create_texture_from_surface(&surface) {
                    let w = surface.width();
                    let h = surface.height();
                    let dest = Rect::new(x, y, w, h);
                    if self.canvas.copy(&texture, None, dest).is_ok() {
                        return;
                    }
                }
            }
        }
        self.draw_bitmap_text(text, x, y, scale, color);
    }

    /// Draws text horizontally centered around `center_x`.
    pub fn draw_text_centered(&mut self, text: &str, center_x: i32, y: i32, scale: u32, color: Color) {
        let w = self.text_width(text, scale);
        let x = center_x - (w as i32) / 2;
        self.draw_text_rendered(text, x, y, scale, color);
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

    /// Draws a labeled button with readable text, auto-fitting the label nicely.
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
        let text_scale = if self.text_width(label, 2) <= width.saturating_sub(12) {
            2u32
        } else {
            1u32
        };
        let text_w = self.text_width(label, text_scale);
        let text_h = (7 * text_scale) as i32;
        let tx = x + (width as i32 - text_w as i32) / 2;
        let ty = y + (height as i32 - text_h) / 2;
        self.draw_text_rendered(label, tx, ty, text_scale, Color::RGB(255, 255, 255));

        btn
    }

    /// Draws a crisp pixel-art Mahjong tile icon at (x, y) with dimensions ~12x14.
    pub fn draw_icon_tile(&mut self, x: i32, y: i32) {
        // Tile outer border / drop shadow
        self.canvas.set_draw_color(Color::RGB(30, 40, 55));
        self.canvas.fill_rect(Rect::new(x, y, 12, 14)).ok();

        // Tile ivory body
        self.canvas.set_draw_color(Color::RGB(245, 245, 240));
        self.canvas.fill_rect(Rect::new(x + 1, y + 1, 10, 12)).ok();

        // Top-left highlight
        self.canvas.set_draw_color(Color::RGB(255, 255, 255));
        self.canvas.draw_line(Point::new(x + 1, y + 1), Point::new(x + 10, y + 1)).ok();
        self.canvas.draw_line(Point::new(x + 1, y + 1), Point::new(x + 1, y + 12)).ok();

        // Bottom-right inner shadow
        self.canvas.set_draw_color(Color::RGB(190, 195, 200));
        self.canvas.draw_line(Point::new(x + 10, y + 2), Point::new(x + 10, y + 12)).ok();
        self.canvas.draw_line(Point::new(x + 2, y + 12), Point::new(x + 10, y + 12)).ok();

        // Red Dragon character '中'
        self.canvas.set_draw_color(Color::RGB(220, 30, 50));
        self.canvas.fill_rect(Rect::new(x + 5, y + 3, 2, 8)).ok();
        self.canvas.draw_rect(Rect::new(x + 3, y + 4, 6, 5)).ok();
    }

    /// Draws a crisp pixel-art red heart icon at (x, y) with dimensions 12x11.
    pub fn draw_icon_heart(&mut self, x: i32, y: i32) {
        let pattern: [&str; 10] = [
            " .XX...XX. ",
            "XXXX.XXXXX",
            "XXXXXXXXXX",
            "XXXXXXXXXX",
            "XXXXXXXXXX",
            " XXXXXXXX ",
            "  XXXXXX  ",
            "   XXXX   ",
            "    XX    ",
            "    ..    ",
        ];
        for (r, row) in pattern.iter().enumerate() {
            for (c, ch) in row.chars().enumerate() {
                if ch == 'X' {
                    let col = if r <= 2 && c <= 4 {
                        Color::RGB(255, 120, 140) // highlight
                    } else if r >= 7 {
                        Color::RGB(200, 20, 45) // shadow
                    } else {
                        Color::RGB(245, 40, 70) // primary red
                    };
                    self.canvas.set_draw_color(col);
                    self.canvas.draw_point(Point::new(x + c as i32, y + r as i32)).ok();
                } else if ch == '.' {
                    self.canvas.set_draw_color(Color::RGB(180, 20, 40));
                    self.canvas.draw_point(Point::new(x + c as i32, y + r as i32)).ok();
                }
            }
        }
    }

    /// Draws a crisp pixel-art golden trophy icon at (x, y) with dimensions 13x12.
    pub fn draw_icon_trophy(&mut self, x: i32, y: i32) {
        let gold_bright = Color::RGB(255, 240, 120);
        let gold_mid = Color::RGB(255, 205, 30);
        let gold_dark = Color::RGB(190, 140, 15);

        // Cup rim
        self.canvas.set_draw_color(gold_bright);
        self.canvas.fill_rect(Rect::new(x + 2, y, 9, 2)).ok();

        // Cup body
        self.canvas.set_draw_color(gold_mid);
        self.canvas.fill_rect(Rect::new(x + 3, y + 2, 7, 3)).ok();
        self.canvas.fill_rect(Rect::new(x + 4, y + 5, 5, 2)).ok();

        // Cup highlight & shadow
        self.canvas.set_draw_color(gold_bright);
        self.canvas.draw_point(Point::new(x + 4, y + 2)).ok();
        self.canvas.draw_point(Point::new(x + 4, y + 3)).ok();
        self.canvas.set_draw_color(gold_dark);
        self.canvas.draw_point(Point::new(x + 8, y + 3)).ok();
        self.canvas.draw_point(Point::new(x + 7, y + 5)).ok();

        // Handles (left & right wings)
        self.canvas.set_draw_color(gold_mid);
        self.canvas.draw_line(Point::new(x + 1, y + 1), Point::new(x + 1, y + 4)).ok();
        self.canvas.draw_point(Point::new(x + 2, y + 1)).ok();
        self.canvas.draw_point(Point::new(x + 2, y + 4)).ok();

        self.canvas.draw_line(Point::new(x + 11, y + 1), Point::new(x + 11, y + 4)).ok();
        self.canvas.draw_point(Point::new(x + 10, y + 1)).ok();
        self.canvas.draw_point(Point::new(x + 10, y + 4)).ok();

        // Stem
        self.canvas.set_draw_color(gold_dark);
        self.canvas.fill_rect(Rect::new(x + 5, y + 7, 3, 2)).ok();

        // Base
        self.canvas.set_draw_color(gold_bright);
        self.canvas.fill_rect(Rect::new(x + 4, y + 9, 5, 1)).ok();
        self.canvas.set_draw_color(gold_mid);
        self.canvas.fill_rect(Rect::new(x + 3, y + 10, 7, 2)).ok();
    }

    /// Draws a crisp pixel-art yellow lightbulb icon at (x, y) with dimensions 11x13.
    pub fn draw_icon_lightbulb(&mut self, x: i32, y: i32) {
        let bulb_glow = Color::RGB(255, 255, 180);
        let bulb_yellow = Color::RGB(255, 215, 30);
        let bulb_dark = Color::RGB(210, 165, 20);
        let base_metal = Color::RGB(160, 175, 195);
        let base_dark = Color::RGB(110, 120, 135);

        // Glass top
        self.canvas.set_draw_color(bulb_yellow);
        self.canvas.fill_rect(Rect::new(x + 3, y, 5, 2)).ok();
        self.canvas.fill_rect(Rect::new(x + 1, y + 2, 9, 4)).ok();
        self.canvas.fill_rect(Rect::new(x + 2, y + 6, 7, 2)).ok();
        self.canvas.fill_rect(Rect::new(x + 3, y + 8, 5, 1)).ok();

        // Glass highlight
        self.canvas.set_draw_color(bulb_glow);
        self.canvas.fill_rect(Rect::new(x + 3, y + 2, 2, 3)).ok();

        // Glass shadow
        self.canvas.set_draw_color(bulb_dark);
        self.canvas.draw_line(Point::new(x + 8, y + 3), Point::new(x + 8, y + 6)).ok();

        // Screw base
        self.canvas.set_draw_color(base_metal);
        self.canvas.fill_rect(Rect::new(x + 3, y + 9, 5, 1)).ok();
        self.canvas.fill_rect(Rect::new(x + 3, y + 11, 5, 1)).ok();
        self.canvas.set_draw_color(base_dark);
        self.canvas.fill_rect(Rect::new(x + 4, y + 10, 3, 1)).ok();
        self.canvas.fill_rect(Rect::new(x + 4, y + 12, 3, 1)).ok();
    }

    /// Draws a crisp pixel-art clock icon at (x, y) with dimensions 12x12.
    pub fn draw_icon_clock(&mut self, x: i32, y: i32) {
        let clock_ring = Color::RGB(120, 210, 255);
        let clock_face = Color::RGB(20, 30, 50);
        let clock_hands = Color::RGB(255, 255, 255);

        // Circular background
        self.canvas.set_draw_color(clock_face);
        self.canvas.fill_rect(Rect::new(x + 2, y + 2, 8, 8)).ok();

        // Circular ring outline
        self.canvas.set_draw_color(clock_ring);
        self.canvas.draw_line(Point::new(x + 4, y + 1), Point::new(x + 7, y + 1)).ok();
        self.canvas.draw_line(Point::new(x + 4, y + 10), Point::new(x + 7, y + 10)).ok();
        self.canvas.draw_line(Point::new(x + 1, y + 4), Point::new(x + 1, y + 7)).ok();
        self.canvas.draw_line(Point::new(x + 10, y + 4), Point::new(x + 10, y + 7)).ok();
        self.canvas.draw_point(Point::new(x + 2, y + 2)).ok();
        self.canvas.draw_point(Point::new(x + 3, y + 2)).ok();
        self.canvas.draw_point(Point::new(x + 2, y + 3)).ok();
        self.canvas.draw_point(Point::new(x + 9, y + 2)).ok();
        self.canvas.draw_point(Point::new(x + 8, y + 2)).ok();
        self.canvas.draw_point(Point::new(x + 9, y + 3)).ok();
        self.canvas.draw_point(Point::new(x + 2, y + 9)).ok();
        self.canvas.draw_point(Point::new(x + 3, y + 9)).ok();
        self.canvas.draw_point(Point::new(x + 2, y + 8)).ok();
        self.canvas.draw_point(Point::new(x + 9, y + 9)).ok();
        self.canvas.draw_point(Point::new(x + 8, y + 9)).ok();
        self.canvas.draw_point(Point::new(x + 9, y + 8)).ok();

        // Top button (stopwatch style)
        self.canvas.draw_line(Point::new(x + 5, y), Point::new(x + 6, y)).ok();

        // Center hub & hands
        self.canvas.set_draw_color(clock_hands);
        self.canvas.draw_point(Point::new(x + 5, y + 5)).ok();
        self.canvas.draw_point(Point::new(x + 4, y + 4)).ok();
        self.canvas.draw_line(Point::new(x + 6, y + 5), Point::new(x + 8, y + 5)).ok();
    }

    /// Draws a crisp pixel-art speaker icon (with sound waves or mute slash).
    pub fn draw_icon_speaker(&mut self, x: i32, y: i32, muted: bool) {
        let color = if muted {
            Color::RGB(160, 165, 180)
        } else {
            Color::RGB(100, 220, 255)
        };

        self.canvas.set_draw_color(color);
        // Speaker base rectangle
        self.canvas.fill_rect(Rect::new(x, y + 3, 3, 6)).ok();

        // Speaker cone
        self.canvas.draw_line(Point::new(x + 3, y + 3), Point::new(x + 6, y + 1)).ok();
        self.canvas.draw_line(Point::new(x + 3, y + 8), Point::new(x + 6, y + 10)).ok();
        self.canvas.draw_line(Point::new(x + 6, y + 1), Point::new(x + 6, y + 10)).ok();
        self.canvas.fill_rect(Rect::new(x + 4, y + 2, 2, 8)).ok();

        if !muted {
            // Sound waves radiating to the right
            self.canvas.draw_point(Point::new(x + 8, y + 3)).ok();
            self.canvas.draw_line(Point::new(x + 9, y + 4), Point::new(x + 9, y + 7)).ok();
            self.canvas.draw_point(Point::new(x + 8, y + 8)).ok();

            self.canvas.draw_point(Point::new(x + 11, y + 2)).ok();
            self.canvas.draw_line(Point::new(x + 12, y + 3), Point::new(x + 12, y + 8)).ok();
            self.canvas.draw_point(Point::new(x + 11, y + 9)).ok();
        } else {
            // Red diagonal mute slash across right side
            self.canvas.set_draw_color(Color::RGB(255, 60, 80));
            self.canvas.draw_line(Point::new(x + 8, y + 2), Point::new(x + 13, y + 9)).ok();
            self.canvas.draw_line(Point::new(x + 13, y + 2), Point::new(x + 8, y + 9)).ok();
        }
    }

    /// Renders a stat pill container in the HUD and returns its width in pixels.
    fn draw_hud_stat_pill(
        &mut self,
        x: i32,
        y: i32,
        icon: HudIcon,
        label: &str,
        value: &str,
        label_color: Color,
        value_color: Color,
    ) -> i32 {
        let text_scale = 1u32;
        let char_w = 6 * text_scale as i32;
        let char_h = 7 * text_scale as i32;
        let icon_w = match icon {
            HudIcon::None => 0,
            HudIcon::Tile => 12,
            HudIcon::Heart => 12,
            HudIcon::Trophy => 13,
            HudIcon::Lightbulb => 11,
            HudIcon::Clock => 12,
        };

        let icon_gap = if icon != HudIcon::None { 7 } else { 0 };
        let label_w = label.len() as i32 * char_w;
        let value_w = value.len() as i32 * char_w;
        let value_gap = if !label.is_empty() { 5 } else { 0 };

        let padding_x = 9;
        let pill_w = padding_x * 2 + icon_w + icon_gap + label_w + value_gap + value_w;
        let pill_h = 26;
        let pill_rect = Rect::new(x, y, pill_w as u32, pill_h as u32);

        // Pill background
        self.canvas.set_draw_color(Color::RGBA(26, 32, 50, 220));
        self.canvas.fill_rect(pill_rect).ok();

        // Subtle border
        self.canvas.set_draw_color(Color::RGB(50, 68, 98));
        self.canvas.draw_rect(pill_rect).ok();

        let mut cur_x = x + padding_x;
        let center_y = y + (pill_h as i32 - 12) / 2;

        // Draw icon
        match icon {
            HudIcon::None => {}
            HudIcon::Tile => {
                self.draw_icon_tile(cur_x, center_y - 1);
            }
            HudIcon::Heart => {
                self.draw_icon_heart(cur_x, center_y);
            }
            HudIcon::Trophy => {
                self.draw_icon_trophy(cur_x, center_y);
            }
            HudIcon::Lightbulb => {
                self.draw_icon_lightbulb(cur_x, center_y - 1);
            }
            HudIcon::Clock => {
                self.draw_icon_clock(cur_x, center_y);
            }
        }
        if icon != HudIcon::None {
            cur_x += icon_w + icon_gap;
        }

        let text_y = y + (pill_h as i32 - char_h) / 2;

        // Draw label
        if !label.is_empty() {
            self.draw_bitmap_text(label, cur_x, text_y, text_scale, label_color);
            cur_x += label_w + value_gap;
        }

        // Draw value
        self.draw_bitmap_text(value, cur_x, text_y, text_scale, value_color);

        pill_w
    }

    /// Renders the HUD overlay: branding, phase badge, level, score, lives, hints, tiles, timer, and mute toggle button.
    pub fn render_hud(&mut self, state: &GameState, muted: bool, lang: Language) {
        let (win_w, _win_h) = self.window_size();

        // HUD background bar at the top
        let hud_height: u32 = 40;
        let hud_rect = Rect::new(0, 0, win_w, hud_height);
        self.canvas.set_draw_color(Color::RGBA(18, 22, 36, 240));
        self.canvas.fill_rect(hud_rect).ok();

        // Bottom border of HUD
        self.canvas.set_draw_color(Color::RGB(45, 60, 90));
        self.canvas.draw_line(
            Point::new(0, hud_height as i32),
            Point::new(win_w as i32, hud_height as i32),
        ).ok();

        // 1. Left Section: Logo + "xMahjong" + Phase Badge
        let brand_x = 16;
        self.draw_icon_tile(brand_x, 13);
        self.draw_bitmap_text("xMahjong", brand_x + 18, 13, 2, Color::RGB(0, 215, 255));

        let brand_end_x = brand_x + 18 + (8 * 12); // ~114px

        // Phase badge
        let phase_name = if state.level <= 10 {
            i18n::t(lang, "phase_penguin")
        } else if state.level <= 20 {
            i18n::t(lang, "phase_dog")
        } else if state.level <= 50 {
            i18n::t(lang, "phase_space")
        } else if state.level <= 100 {
            i18n::t(lang, "phase_endgame")
        } else {
            i18n::t(lang, "phase_grandmaster")
        };

        let mut left_section_end = brand_end_x;
        if win_w >= 940 {
            let char_count = phase_name.chars().count() as i32;
            let badge_w = (char_count * 6) + 16;
            let badge_x = brand_end_x + 12;
            let badge_y = 10;
            let badge_h = 20;
            let badge_rect = Rect::new(badge_x, badge_y, badge_w as u32, badge_h as u32);

            self.canvas.set_draw_color(Color::RGBA(15, 65, 95, 180));
            self.canvas.fill_rect(badge_rect).ok();
            self.canvas.set_draw_color(Color::RGB(30, 140, 190));
            self.canvas.draw_rect(badge_rect).ok();

            self.draw_text_rendered(phase_name, badge_x + 8, badge_y + 6, 1, Color::RGB(100, 225, 255));
            left_section_end = badge_x + badge_w;
        }

        // 2. Mute Button on Far Right
        let mute_rect = Self::hud_mute_button_rect(win_w);
        self.canvas.set_draw_color(Color::RGBA(26, 32, 50, 220));
        self.canvas.fill_rect(mute_rect).ok();

        if muted {
            self.canvas.set_draw_color(Color::RGB(140, 50, 65));
        } else {
            self.canvas.set_draw_color(Color::RGB(50, 68, 98));
        }
        self.canvas.draw_rect(mute_rect).ok();
        self.draw_icon_speaker(mute_rect.x() + 9, mute_rect.y() + 7, muted);

        // 3. Compute data for stats pills
        let total_score = state.base_score + state.score.live_score();
        let lives = state.shuffles_remaining;
        let total_hints = state.base_hints + state.score.hints_used;
        let remaining_tiles = state.board.tiles.iter().filter(|t| t.is_some()).count();
        let total_tiles = crate::levels::tiles_for_level(state.level);

        let total_ms = state.base_time_ms + state.timer.elapsed_ms
            + state.timer.last_tick.map(|t| t.elapsed().as_millis() as u64).unwrap_or(0);
        let total_secs = (total_ms / 1000) as u32;
        let minutes = total_secs / 60;
        let seconds = total_secs % 60;
        let timer_text = format!("{:02}:{:02}", minutes, seconds);

        let label_col = Color::RGB(170, 190, 215);

        let label_level = format!("{}:", i18n::t(lang, "level"));
        let label_score = format!("{}:", i18n::t(lang, "score"));
        let label_lives = format!("{}:", i18n::t(lang, "lives"));
        let label_hints = format!("{}:", i18n::t(lang, "hints"));
        let label_tiles = format!("{}:", i18n::t(lang, "tiles"));
        let label_time = format!("{}:", i18n::t(lang, "time"));

        let pills: [(HudIcon, &str, String, Color, Color); 6] = [
            (HudIcon::None, &label_level, format!("{}", state.level), label_col, Color::RGB(140, 210, 255)),
            (HudIcon::Trophy, &label_score, format!("{}", total_score), label_col, Color::RGB(255, 215, 0)),
            (HudIcon::Heart, &label_lives, format!("{}", lives), label_col, Color::RGB(80, 240, 130)),
            (HudIcon::Lightbulb, &label_hints, format!("{}", total_hints), label_col, Color::RGB(0, 215, 255)),
            (HudIcon::Tile, &label_tiles, format!("{}/{}", remaining_tiles, total_tiles), label_col, Color::RGB(0, 215, 255)),
            (HudIcon::Clock, &label_time, timer_text, label_col, Color::RGB(100, 230, 255)),
        ];

        // Helper to compute pill width
        let calc_pill_w = |icon: HudIcon, label: &str, value: &str| -> i32 {
            let icon_w = match icon {
                HudIcon::None => 0,
                HudIcon::Tile => 12,
                HudIcon::Heart => 12,
                HudIcon::Trophy => 13,
                HudIcon::Lightbulb => 11,
                HudIcon::Clock => 12,
            };
            let icon_gap = if icon != HudIcon::None { 7 } else { 0 };
            let label_w = label.chars().count() as i32 * 6;
            let value_w = value.chars().count() as i32 * 6;
            let value_gap = if !label.is_empty() { 5 } else { 0 };
            18 + icon_w + icon_gap + label_w + value_gap + value_w
        };

        let pill_widths: Vec<i32> = pills.iter().map(|(icon, lbl, val, _, _)| calc_pill_w(*icon, lbl, val)).collect();
        let gap: i32 = if win_w >= 1200 { 10 } else { 6 };
        let total_pills_w: i32 = pill_widths.iter().sum::<i32>() + gap * (pills.len() as i32 - 1);

        let mute_btn_space = 34 + 10;
        let start_x = (win_w as i32 - mute_btn_space - total_pills_w - 14).max(left_section_end + 12);
        let pill_y = 7;

        let mut current_x = start_x;
        for (icon, label, value, l_col, v_col) in &pills {
            let w = self.draw_hud_stat_pill(current_x, pill_y, *icon, label, value, *l_col, *v_col);
            current_x += w + gap;
        }
    }

    /// Returns the bounding Rect of the HUD mute button for hit-testing.
    pub fn hud_mute_button_rect(win_w: u32) -> Rect {
        let btn_w = 34;
        let btn_h = 26;
        let btn_x = win_w as i32 - btn_w - 14;
        let btn_y = 7;
        Rect::new(btn_x, btn_y, btn_w as u32, btn_h as u32)
    }

    /// Renders a small, clickable "MENU" button in the bottom-left corner of the screen.
    pub fn render_menu_button(&mut self, lang: Language) -> Rect {
        let (_win_w, win_h) = self.window_size();

        // On Linux, use a larger button and font scale for readability
        #[cfg(target_os = "linux")]
        let (btn_w, btn_h, text_scale, icon_scale): (u32, u32, u32, i32) = (130, 36, 2, 2);
        #[cfg(not(target_os = "linux"))]
        let (btn_w, btn_h, text_scale, icon_scale): (u32, u32, u32, i32) = (90, 26, 1, 1);

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

        // Menu text (localized)
        let menu_x = dot_x + dot_size as i32 + 4;
        let menu_label = i18n::t(lang, "menu");
        self.draw_text_rendered(menu_label, menu_x, text_y, text_scale, Color::RGB(200, 240, 240));

        btn_rect
    }

    /// Renders the pause menu overlay with game options including dynamic language switcher.
    pub fn render_menu(&mut self, selected: usize, difficulty: &str, lang: Language) {
        self.draw_overlay_backdrop();

        let dialog = self.draw_dialog_box(340, 650);

        // Title
        let paused_title = i18n::t(lang, "paused");
        let center_x = dialog.x() + dialog.width() as i32 / 2;
        self.draw_text_centered(
            paused_title,
            center_x,
            dialog.y() + 16,
            3,
            Color::RGB(200, 220, 255),
        );

        let btn_w: u32 = 260;
        let btn_h: u32 = 36;
        let btn_x = dialog.x() + ((dialog.width() - btn_w) / 2) as i32;
        let start_y = dialog.y() + 50;
        let spacing: i32 = 44;

        let diff_display = match difficulty {
            "easy" | "EASY" => i18n::t(lang, "theme_auto"),
            _ => difficulty,
        };
        let difficulty_label = i18n::t_param(lang, "mode_label", &[("difficulty", diff_display)]);
        let language_label = i18n::t_param(lang, "language_label", &[("lang", lang.display_label().as_str())]);

        let new_game_str = i18n::t(lang, "new_game");
        let undo_str = i18n::t(lang, "undo");
        let hint_str = i18n::t(lang, "hint");
        let shuffle_str = i18n::t(lang, "shuffle");
        let levels_str = i18n::t(lang, "levels");
        let shortcuts_str = i18n::t(lang, "shortcuts");
        let achievements_str = i18n::t(lang, "achievements");
        let about_str = i18n::t(lang, "about");
        let switch_user_str = i18n::t(lang, "switch_user");
        let save_quit_str = i18n::t(lang, "save_quit");

        let buttons: Vec<(Color, &str)> = vec![
            (Color::RGB(50, 140, 70), new_game_str),
            (Color::RGB(50, 100, 180), undo_str),
            (Color::RGB(50, 160, 170), hint_str),
            (Color::RGB(120, 60, 160), shuffle_str),
            (Color::RGB(140, 90, 170), levels_str),
            (Color::RGB(100, 140, 100), shortcuts_str),
            (Color::RGB(50, 100, 180), achievements_str),
            (Color::RGB(0, 130, 130), &difficulty_label),
            (Color::RGB(30, 120, 150), &language_label),
            (Color::RGB(80, 120, 180), about_str),
            (Color::RGB(160, 100, 180), switch_user_str),
            (Color::RGB(200, 130, 50), save_quit_str),
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
        let bottom_hint = i18n::t(lang, "esc_resume_ctrl_s_save");
        self.draw_text_centered(
            bottom_hint,
            center_x,
            dialog.y() + 620,
            1,
            Color::RGB(120, 120, 140),
        );
    }

    /// Renders the shortcuts popup showing all keyboard shortcuts.
    pub fn render_shortcuts(&mut self, lang: Language) {
        self.draw_overlay_backdrop();

        let dialog = self.draw_dialog_box(440, 430);

        // Title
        let title = i18n::t(lang, "keyboard_shortcuts");
        let center_x = dialog.x() + dialog.width() as i32 / 2;
        self.draw_text_centered(
            title,
            center_x,
            dialog.y() + 16,
            2,
            Color::RGB(255, 215, 0),
        );

        let x_key = dialog.x() + 20;
        let x_action = dialog.x() + 190;
        let mut y = dialog.y() + 54;
        let line_h: i32 = 24;
        let key_color = Color::RGB(150, 220, 255);
        let action_color = Color::RGB(200, 200, 210);

        let shortcuts: &[(&str, &str)] = &[
            ("LEFT CLICK", i18n::t(lang, "tiles")),
            ("CTRL+S", i18n::t(lang, "save_score")),
            ("CTRL+Q", i18n::t(lang, "save_quit")),
            ("CTRL+N", i18n::t(lang, "new_game")),
            ("CTRL+R", i18n::t(lang, "resume_game")),
            ("CTRL+P", i18n::t(lang, "pause_title")),
            ("CTRL+M", "MUTE"),
            ("SHIFT+S", i18n::t(lang, "shuffle")),
            ("SHIFT+U", i18n::t(lang, "undo")),
            ("SHIFT+H", i18n::t(lang, "hint")),
            ("ESCAPE", i18n::t(lang, "paused")),
            ("UP/DOWN", "NAVIGATE"),
            ("ENTER", "SELECT"),
        ];

        for (key, action) in shortcuts {
            self.draw_bitmap_text(key, x_key, y, 2, key_color);
            self.draw_text_rendered(action, x_action, y, 2, action_color);
            y += line_h;
        }

        // Back button
        let btn_w: u32 = 180;
        let btn_h: u32 = 40;
        let btn_x = dialog.x() + ((dialog.width() - btn_w) / 2) as i32;
        let btn_y = dialog.y() + 430 - 52;
        let back_str = i18n::t(lang, "back_btn");
        self.draw_labeled_button(btn_x, btn_y, btn_w, btn_h, Color::RGB(100, 100, 100), back_str);
    }

    /// Renders the victory overlay showing final time and score.
    pub fn render_victory(&mut self, time: &str, score: u32, level: u32, selected: usize, lang: Language) {
        self.draw_overlay_backdrop();

        let dialog_h: u32 = if level < 1000 { 360 } else { 300 };
        let dialog = self.draw_dialog_box(360, dialog_h);

        let lvl_str = level.to_string();
        let max_str = "1000".to_string();
        let victory_title = if level < 1000 {
            i18n::t_param(lang, "victory_title", &[("level", &lvl_str)])
        } else {
            i18n::t_param(lang, "all_levels_complete", &[("max", &max_str)])
        };
        let center_x = dialog.x() + dialog.width() as i32 / 2;

        self.draw_text_centered(
            &victory_title,
            center_x,
            dialog.y() + 24,
            2,
            Color::RGB(255, 215, 0),
        );

        // Time display
        let time_label = format!("{}: {}", i18n::t(lang, "time"), time);
        self.draw_text_rendered(
            &time_label,
            dialog.x() + 80,
            dialog.y() + 75,
            2,
            Color::RGB(100, 220, 100),
        );

        // Score display
        let score_label = format!("{}: {}", i18n::t(lang, "score"), score);
        self.draw_text_rendered(
            &score_label,
            dialog.x() + 80,
            dialog.y() + 110,
            2,
            Color::RGB(255, 200, 50),
        );

        let btn_w: u32 = 240;
        let btn_h: u32 = 44;
        let btn_x = dialog.x() + ((dialog.width() - btn_w) / 2) as i32;

        let next_lvl_str = (level + 1).to_string();
        let next_label = i18n::t_param(lang, "next_level", &[("next", &next_lvl_str)]);
        let new_game_label = i18n::t(lang, "new_game");
        let trophies_label = i18n::t(lang, "achievements");

        if level < 1000 {
            let buttons: &[(i32, Color, &str)] = &[
                (155, Color::RGB(120, 60, 180), &next_label),
                (215, Color::RGB(50, 140, 70), new_game_label),
                (275, Color::RGB(50, 100, 180), trophies_label),
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
                (160, Color::RGB(50, 140, 70), new_game_label),
                (220, Color::RGB(50, 100, 180), trophies_label),
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

    /// Draws a card container with subtle rounded corners and stylish border.
    pub fn draw_rounded_card(&mut self, rect: Rect, bg: Color, border: Color) {
        // Main fill
        self.canvas.set_draw_color(bg);
        self.canvas.fill_rect(Rect::new(rect.x() + 2, rect.y(), rect.width().saturating_sub(4), rect.height())).ok();
        self.canvas.fill_rect(Rect::new(rect.x(), rect.y() + 2, rect.width(), rect.height().saturating_sub(4))).ok();
        self.canvas.fill_rect(Rect::new(rect.x() + 1, rect.y() + 1, rect.width().saturating_sub(2), rect.height().saturating_sub(2))).ok();

        // Border outline
        self.canvas.set_draw_color(border);
        let x = rect.x();
        let y = rect.y();
        let w = rect.width() as i32;
        let h = rect.height() as i32;

        // Horizontal edges
        self.canvas.draw_line(sdl2::rect::Point::new(x + 2, y), sdl2::rect::Point::new(x + w - 3, y)).ok();
        self.canvas.draw_line(sdl2::rect::Point::new(x + 2, y + h - 1), sdl2::rect::Point::new(x + w - 3, y + h - 1)).ok();
        // Vertical edges
        self.canvas.draw_line(sdl2::rect::Point::new(x, y + 2), sdl2::rect::Point::new(x, y + h - 3)).ok();
        self.canvas.draw_line(sdl2::rect::Point::new(x + w - 1, y + 2), sdl2::rect::Point::new(x + w - 1, y + h - 3)).ok();
        // Corner diagonals
        self.canvas.draw_point(sdl2::rect::Point::new(x + 1, y + 1)).ok();
        self.canvas.draw_point(sdl2::rect::Point::new(x + w - 2, y + 1)).ok();
        self.canvas.draw_point(sdl2::rect::Point::new(x + 1, y + h - 2)).ok();
        self.canvas.draw_point(sdl2::rect::Point::new(x + w - 2, y + h - 2)).ok();
    }

    /// Draws a mini Mahjong tile with red '中'.
    pub fn draw_mini_tile_icon(&mut self, x: i32, y: i32) {
        // Tile 3D green base
        self.canvas.set_draw_color(Color::RGB(46, 125, 50));
        self.canvas.fill_rect(Rect::new(x + 1, y + 1, 14, 18)).ok();
        // Tile front face (white/cream)
        self.canvas.set_draw_color(Color::RGB(248, 250, 252));
        self.canvas.fill_rect(Rect::new(x, y, 13, 17)).ok();
        self.canvas.set_draw_color(Color::RGB(203, 213, 225));
        self.canvas.draw_rect(Rect::new(x, y, 13, 17)).ok();
        // Red '中' symbol
        self.canvas.set_draw_color(Color::RGB(225, 29, 72));
        self.canvas.draw_rect(Rect::new(x + 3, y + 4, 7, 7)).ok();
        self.canvas.draw_line(sdl2::rect::Point::new(x + 6, y + 2), sdl2::rect::Point::new(x + 6, y + 13)).ok();
    }

    /// Draws a gold trophy icon.
    pub fn draw_trophy_icon(&mut self, cx: i32, cy: i32, size: i32) {
        let gold = Color::RGB(251, 191, 36);
        let dark_gold = Color::RGB(217, 119, 6);
        let shine = Color::RGB(254, 240, 138);

        // Pedestal base
        self.canvas.set_draw_color(dark_gold);
        self.canvas.fill_rect(Rect::new(cx - size / 3, cy + size / 3, (2 * size / 3) as u32, (size / 6) as u32)).ok();
        self.canvas.set_draw_color(gold);
        self.canvas.fill_rect(Rect::new(cx - size / 4, cy + size / 5, (size / 2) as u32, (size / 8) as u32)).ok();

        // Stem
        self.canvas.set_draw_color(dark_gold);
        self.canvas.fill_rect(Rect::new(cx - 2, cy, 4, (size / 4) as u32)).ok();

        // Cup bowl
        self.canvas.set_draw_color(gold);
        self.canvas.fill_rect(Rect::new(cx - size / 3, cy - size / 2 + 2, (2 * size / 3) as u32, (size / 2) as u32)).ok();
        // Cup rim
        self.canvas.fill_rect(Rect::new(cx - size / 3 - 2, cy - size / 2, (2 * size / 3 + 4) as u32, 3)).ok();

        // Handles
        self.canvas.set_draw_color(dark_gold);
        self.canvas.draw_rect(Rect::new(cx - size / 2, cy - size / 3, (size / 6 + 1) as u32, (size / 3) as u32)).ok();
        self.canvas.draw_rect(Rect::new(cx + size / 3 - 1, cy - size / 3, (size / 6 + 1) as u32, (size / 3) as u32)).ok();

        // Highlight shine
        self.canvas.set_draw_color(shine);
        self.canvas.draw_line(sdl2::rect::Point::new(cx - 2, cy - size / 3), sdl2::rect::Point::new(cx - 2, cy - 2)).ok();
    }

    /// Draws a mountain peak icon (snowy peak with green slopes).
    pub fn draw_mountain_icon(&mut self, cx: i32, cy: i32, size: i32) {
        let half = size / 2;
        // Mountain base (slopes)
        self.canvas.set_draw_color(Color::RGB(52, 116, 85));
        for i in 0..half {
            let width = (i * 2 + 1) as u32;
            self.canvas.fill_rect(Rect::new(cx - i, cy - half + (half + i), width, 1)).ok();
        }
        // Snow peak
        self.canvas.set_draw_color(Color::RGB(241, 245, 249));
        for i in 0..half {
            let width = (i * 2 + 1) as u32;
            self.canvas.fill_rect(Rect::new(cx - i, cy - half + i, width, 1)).ok();
        }
        // Base rim
        self.canvas.set_draw_color(Color::RGB(30, 75, 55));
        self.canvas.draw_line(sdl2::rect::Point::new(cx - half, cy + half), sdl2::rect::Point::new(cx + half, cy + half)).ok();
    }

    /// Draws a glowing golden star icon.
    pub fn draw_star_icon(&mut self, cx: i32, cy: i32, radius: i32, color: Color) {
        self.canvas.set_draw_color(color);
        // Center core
        self.canvas.fill_rect(Rect::new(cx - radius / 2, cy - radius / 2, radius as u32, radius as u32)).ok();
        // Points (top, bottom, left, right)
        for r in 1..=radius {
            let w = (radius - r + 1).max(1) as u32;
            self.canvas.fill_rect(Rect::new(cx - (w as i32) / 2, cy - radius / 2 - r, w, 1)).ok();
            self.canvas.fill_rect(Rect::new(cx - (w as i32) / 2, cy + radius / 2 + r - 1, w, 1)).ok();
            self.canvas.fill_rect(Rect::new(cx - radius / 2 - r, cy - (w as i32) / 2, 1, w)).ok();
            self.canvas.fill_rect(Rect::new(cx + radius / 2 + r - 1, cy - (w as i32) / 2, 1, w)).ok();
        }
        // Diagonal spikes
        let diag = radius * 3 / 4;
        for d in 1..=diag {
            self.canvas.draw_point(sdl2::rect::Point::new(cx - d, cy - d)).ok();
            self.canvas.draw_point(sdl2::rect::Point::new(cx + d, cy - d)).ok();
            self.canvas.draw_point(sdl2::rect::Point::new(cx - d, cy + d)).ok();
            self.canvas.draw_point(sdl2::rect::Point::new(cx + d, cy + d)).ok();
        }
        // Center glint
        self.canvas.set_draw_color(Color::RGB(255, 255, 255));
        self.canvas.fill_rect(Rect::new(cx - 1, cy - 1, 2, 2)).ok();
    }

    /// Draws a flame icon (multi-layered orange/yellow/red).
    pub fn draw_flame_icon(&mut self, cx: i32, cy: i32, size: i32) {
        let half = size / 2;
        // Outer red flame
        self.canvas.set_draw_color(Color::RGB(239, 68, 68));
        for i in 0..size {
            let width = ((size - i) * 2 / 3 + 2) as u32;
            self.canvas.fill_rect(Rect::new(cx - (width as i32) / 2, cy + half - i, width, 1)).ok();
        }
        // Mid orange flame
        self.canvas.set_draw_color(Color::RGB(249, 115, 22));
        for i in 0..(size * 3 / 4) {
            let width = ((size * 3 / 4 - i) / 2 + 2) as u32;
            self.canvas.fill_rect(Rect::new(cx - (width as i32) / 2, cy + half - i, width, 1)).ok();
        }
        // Inner core yellow
        self.canvas.set_draw_color(Color::RGB(253, 224, 71));
        for i in 0..(size / 2) {
            let width = ((size / 2 - i) / 3 + 2) as u32;
            self.canvas.fill_rect(Rect::new(cx - (width as i32) / 2, cy + half - i, width, 1)).ok();
        }
    }

    /// Draws a golden royalty crown icon.
    pub fn draw_crown_icon(&mut self, cx: i32, cy: i32, size: i32) {
        let gold = Color::RGB(245, 158, 11);
        let dark_gold = Color::RGB(180, 83, 9);
        let half = size / 2;

        // Crown base
        self.canvas.set_draw_color(dark_gold);
        self.canvas.fill_rect(Rect::new(cx - half, cy + half - 4, size as u32, 4)).ok();
        self.canvas.set_draw_color(gold);
        self.canvas.fill_rect(Rect::new(cx - half + 1, cy + half - 3, (size - 2) as u32, 2)).ok();

        // 3 Crown points
        // Left point
        self.canvas.draw_line(sdl2::rect::Point::new(cx - half, cy + half - 4), sdl2::rect::Point::new(cx - half + 2, cy - half + 4)).ok();
        self.canvas.draw_line(sdl2::rect::Point::new(cx - half + 2, cy - half + 4), sdl2::rect::Point::new(cx - 3, cy + 2)).ok();
        // Center point (tallest)
        self.canvas.draw_line(sdl2::rect::Point::new(cx - 3, cy + 2), sdl2::rect::Point::new(cx, cy - half)).ok();
        self.canvas.draw_line(sdl2::rect::Point::new(cx, cy - half), sdl2::rect::Point::new(cx + 3, cy + 2)).ok();
        // Right point
        self.canvas.draw_line(sdl2::rect::Point::new(cx + 3, cy + 2), sdl2::rect::Point::new(cx + half - 2, cy - half + 4)).ok();
        self.canvas.draw_line(sdl2::rect::Point::new(cx + half - 2, cy - half + 4), sdl2::rect::Point::new(cx + half, cy + half - 4)).ok();

        // Fill body
        self.canvas.fill_rect(Rect::new(cx - half + 2, cy + 1, (size - 4) as u32, (half - 5) as u32)).ok();

        // Jewels on peak tips
        self.canvas.set_draw_color(Color::RGB(239, 68, 68)); // Ruby
        self.canvas.fill_rect(Rect::new(cx - 1, cy - half - 1, 3, 3)).ok();
        self.canvas.set_draw_color(Color::RGB(59, 130, 246)); // Sapphire
        self.canvas.fill_rect(Rect::new(cx - half + 1, cy - half + 3, 3, 3)).ok();
        self.canvas.fill_rect(Rect::new(cx + half - 3, cy - half + 3, 3, 3)).ok();
    }

    /// Draws a gift box icon 🎁.
    pub fn draw_gift_icon(&mut self, cx: i32, cy: i32, size: i32) {
        let half = size / 2;
        // Red box body
        self.canvas.set_draw_color(Color::RGB(220, 38, 38));
        self.canvas.fill_rect(Rect::new(cx - half + 1, cy - half + 4, (size - 2) as u32, (size - 4) as u32)).ok();
        // Lid
        self.canvas.set_draw_color(Color::RGB(185, 28, 28));
        self.canvas.fill_rect(Rect::new(cx - half, cy - half + 2, size as u32, 3)).ok();

        // Yellow ribbon
        self.canvas.set_draw_color(Color::RGB(250, 204, 21));
        self.canvas.fill_rect(Rect::new(cx - 1, cy - half + 2, 3, (size - 2) as u32)).ok();
        self.canvas.fill_rect(Rect::new(cx - half + 1, cy + 1, (size - 2) as u32, 2)).ok();

        // Bow on top
        self.canvas.draw_rect(Rect::new(cx - 4, cy - half - 2, 4, 4)).ok();
        self.canvas.draw_rect(Rect::new(cx + 1, cy - half - 2, 4, 4)).ok();
    }

    /// Draws a lightbulb icon 💡.
    pub fn draw_bulb_icon(&mut self, cx: i32, cy: i32, size: i32) {
        let gold = Color::RGB(250, 204, 21);
        let half = size / 2;

        // Bulb round head
        self.canvas.set_draw_color(gold);
        self.canvas.fill_rect(Rect::new(cx - half + 2, cy - half, (size - 4) as u32, (size * 2 / 3) as u32)).ok();
        self.canvas.fill_rect(Rect::new(cx - half, cy - half + 2, size as u32, (size / 2) as u32)).ok();

        // Highlight
        self.canvas.set_draw_color(Color::RGB(255, 255, 230));
        self.canvas.fill_rect(Rect::new(cx - half + 3, cy - half + 3, 2, 3)).ok();

        // Metallic screw base
        self.canvas.set_draw_color(Color::RGB(156, 163, 175));
        self.canvas.fill_rect(Rect::new(cx - 3, cy + half - 3, 6, 2)).ok();
        self.canvas.set_draw_color(Color::RGB(107, 114, 128));
        self.canvas.fill_rect(Rect::new(cx - 2, cy + half - 1, 4, 2)).ok();
    }

    /// Draws an undo counter-clockwise arrow badge ↩️.
    pub fn draw_undo_icon(&mut self, cx: i32, cy: i32, size: i32) {
        let half = size / 2;
        // Rounded badge
        self.canvas.set_draw_color(Color::RGB(59, 130, 246));
        self.canvas.fill_rect(Rect::new(cx - half, cy - half, size as u32, size as u32)).ok();

        // White curved arrow
        self.canvas.set_draw_color(Color::RGB(255, 255, 255));
        // Arrow horizontal top bar
        self.canvas.fill_rect(Rect::new(cx - half + 4, cy - 2, (size / 2 + 1) as u32, 3)).ok();
        // Downward curve
        self.canvas.fill_rect(Rect::new(cx + 1, cy - 2, 3, (half - 1) as u32)).ok();
        // Arrow head pointing left
        self.canvas.fill_rect(Rect::new(cx - half + 3, cy - 4, 2, 7)).ok();
        self.canvas.fill_rect(Rect::new(cx - half + 2, cy - 3, 2, 5)).ok();
        self.canvas.fill_rect(Rect::new(cx - half + 1, cy - 2, 2, 3)).ok();
    }

    /// Draws a heart icon ❤️.
    pub fn draw_heart_icon(&mut self, cx: i32, cy: i32, size: i32, color: Color) {
        let half = size / 2;
        self.canvas.set_draw_color(color);

        // Top two lobes
        self.canvas.fill_rect(Rect::new(cx - half + 1, cy - half + 1, (half - 1) as u32, (half + 1) as u32)).ok();
        self.canvas.fill_rect(Rect::new(cx + 1, cy - half + 1, (half - 1) as u32, (half + 1) as u32)).ok();
        // Middle fill
        self.canvas.fill_rect(Rect::new(cx - half, cy - half + 3, size as u32, (half - 1) as u32)).ok();

        // Taper down to point
        for i in 0..half {
            let width = ((half - i) * 2) as u32;
            self.canvas.fill_rect(Rect::new(cx - (half - i), cy + 2 + i, width, 1)).ok();
        }

        // Highlight glint
        self.canvas.set_draw_color(Color::RGB(255, 220, 230));
        self.canvas.fill_rect(Rect::new(cx - half + 2, cy - half + 2, 2, 2)).ok();
    }

    /// Draws a floppy disk save icon 💾.
    pub fn draw_floppy_icon(&mut self, cx: i32, cy: i32, size: i32) {
        let half = size / 2;
        // Body
        self.canvas.set_draw_color(Color::RGB(99, 102, 241));
        self.canvas.fill_rect(Rect::new(cx - half, cy - half, size as u32, size as u32)).ok();
        // Metal shutter on top
        self.canvas.set_draw_color(Color::RGB(226, 232, 240));
        self.canvas.fill_rect(Rect::new(cx - half + 2, cy - half, (size - 5) as u32, (size / 3 + 1) as u32)).ok();
        self.canvas.set_draw_color(Color::RGB(79, 70, 229));
        self.canvas.fill_rect(Rect::new(cx - 2, cy - half + 1, 3, (size / 3 - 1) as u32)).ok();

        // Label on bottom
        self.canvas.set_draw_color(Color::RGB(248, 250, 252));
        self.canvas.fill_rect(Rect::new(cx - half + 2, cy + 1, (size - 4) as u32, (half - 2) as u32)).ok();
        self.canvas.set_draw_color(Color::RGB(148, 163, 184));
        self.canvas.fill_rect(Rect::new(cx - half + 4, cy + 3, (size - 8) as u32, 2)).ok();
    }

    /// Draws a close cross icon ✖.
    pub fn draw_cross_icon(&mut self, cx: i32, cy: i32, size: i32, color: Color) {
        let half = size / 2;
        self.canvas.set_draw_color(color);
        for d in -half..=half {
            self.canvas.draw_point(sdl2::rect::Point::new(cx + d, cy + d)).ok();
            self.canvas.draw_point(sdl2::rect::Point::new(cx + d + 1, cy + d)).ok();
            self.canvas.draw_point(sdl2::rect::Point::new(cx + d, cy - d)).ok();
            self.canvas.draw_point(sdl2::rect::Point::new(cx + d + 1, cy - d)).ok();
        }
    }

    /// Draws a career overview bar chart icon 📊.
    pub fn draw_chart_icon(&mut self, cx: i32, cy: i32, size: i32) {
        let half = size / 2;
        // Bar 1 (cyan)
        self.canvas.set_draw_color(Color::RGB(56, 189, 248));
        self.canvas.fill_rect(Rect::new(cx - half, cy + 1, 3, (half - 1) as u32)).ok();
        // Bar 2 (emerald)
        self.canvas.set_draw_color(Color::RGB(52, 211, 153));
        self.canvas.fill_rect(Rect::new(cx - half + 4, cy - 2, 3, (half + 2) as u32)).ok();
        // Bar 3 (gold)
        self.canvas.set_draw_color(Color::RGB(251, 191, 36));
        self.canvas.fill_rect(Rect::new(cx - half + 8, cy - half, 3, size as u32)).ok();
    }

    /// Formats a number with comma grouping (e.g. 11965 -> "11,965").
    fn format_number_commas(num: u64) -> String {
        let s = num.to_string();
        let bytes = s.as_bytes();
        let len = bytes.len();
        let mut result = String::new();
        for (i, &b) in bytes.iter().enumerate() {
            if i > 0 && (len - i) % 3 == 0 {
                result.push(',');
            }
            result.push(b as char);
        }
        result
    }

    pub const LEADERBOARD_WIDTH: u32 = 660;
    pub const LEADERBOARD_HEIGHT: u32 = 474;
    pub const STATS_CARD_HEIGHT: u32 = 416;

    /// Computes the exact Rect of the Trophies & Stats dialog centered in the window.
    pub fn leaderboard_dialog_rect(win_w: u32, win_h: u32) -> Rect {
        let dialog_x = (win_w.saturating_sub(Self::LEADERBOARD_WIDTH)) / 2;
        let dialog_y = (win_h.saturating_sub(Self::LEADERBOARD_HEIGHT)) / 2;
        Rect::new(dialog_x as i32, dialog_y as i32, Self::LEADERBOARD_WIDTH, Self::LEADERBOARD_HEIGHT)
    }

    /// Computes the exact Rect of the Stats Card to capture for sharing (excluding action buttons).
    pub fn leaderboard_stats_card_rect(win_w: u32, win_h: u32) -> Rect {
        let dialog = Self::leaderboard_dialog_rect(win_w, win_h);
        Rect::new(dialog.x(), dialog.y(), Self::LEADERBOARD_WIDTH, Self::STATS_CARD_HEIGHT)
    }

    /// Returns the Rects for the two action buttons (Save Stats Image, Close) flush with the trophy frame.
    pub fn leaderboard_buttons(win_w: u32, win_h: u32) -> (Rect, Rect) {
        let dialog = Self::leaderboard_dialog_rect(win_w, win_h);
        let btn_w: u32 = 320;
        let btn_h: u32 = 42;
        let btn_y = dialog.y() + Self::STATS_CARD_HEIGHT as i32 + 12;
        let btn1 = Rect::new(dialog.x(), btn_y, btn_w, btn_h);
        let btn2 = Rect::new(dialog.x() + 340, btn_y, btn_w, btn_h);
        (btn1, btn2)
    }

    /// Renders the redesigned Trophy & Stats panel matching the modern web-based card layout.
    pub fn render_leaderboard(&mut self, user_name: &str, is_saved_active: bool, lang: Language) {
        self.draw_overlay_backdrop();

        let leaderboard = Leaderboard::load(user_name);
        let shuffle_state = ShuffleState::load(user_name);
        let trophy_state = TrophyState::load(user_name);
        let user_progress = UserProgress::load(user_name);

        let current_streak = shuffle_state.consecutive_days;
        let best_streak = shuffle_state.best_streak.max(current_streak);
        let highest_level = if user_progress.max_completed_level > 0 {
            user_progress.max_completed_level
        } else {
            1
        };

        // Total score: highest of career score, leaderboard entry score, or calculated progress
        let last_match_score = leaderboard.entries.first().map(|e| e.score as u64).unwrap_or(0);
        let total_score = trophy_state.total_career_score.max(last_match_score);

        let (win_w, win_h) = self.window_size();
        let dialog = Self::leaderboard_dialog_rect(win_w, win_h);
        let stats_card = Self::leaderboard_stats_card_rect(win_w, win_h);

        // Stats card container (midnight blue background with subtle depth border, excluding bottom buttons)
        self.canvas.set_draw_color(Color::RGB(11, 19, 41));
        self.canvas.fill_rect(stats_card).ok();
        self.canvas.set_draw_color(Color::RGB(28, 48, 80));
        self.canvas.draw_rect(stats_card).ok();
        let inner_border = Rect::new(stats_card.x() + 1, stats_card.y() + 1, stats_card.width() - 2, stats_card.height() - 2);
        self.canvas.set_draw_color(Color::RGB(19, 31, 55));
        self.canvas.draw_rect(inner_border).ok();

        let center_x = dialog.x() + dialog.width() as i32 / 2;

        // -------------------------------------------------------------
        // TOP HEADER: Pill Badge + Title + Subtitle
        // -------------------------------------------------------------
        let pill_w: u32 = 136;
        let pill_h: u32 = 22;
        let pill_x = dialog.x() + ((dialog.width() - pill_w) / 2) as i32;
        let pill_y = dialog.y() + 10;
        let pill_rect = Rect::new(pill_x, pill_y, pill_w, pill_h);
        self.draw_rounded_card(pill_rect, Color::RGB(23, 37, 69), Color::RGB(56, 110, 180));
        self.draw_mini_tile_icon(pill_x + 8, pill_y + 2);
        self.draw_bitmap_text("xMahjong", pill_x + 30, pill_y + 4, 2, Color::RGB(147, 197, 253));

        // Title: 🏆 TROPHIES & STATS (or localized)
        let title_text = i18n::t(lang, "trophies_title");
        let title_scale = 3u32;
        let title_w = self.text_width(title_text, title_scale);
        let icon_w = 22;
        let gap = 10;
        let total_title_w = icon_w + gap + title_w as i32;
        let title_start_x = dialog.x() + (dialog.width() as i32 - total_title_w) / 2;
        let title_y = dialog.y() + 38;

        self.draw_trophy_icon(title_start_x + icon_w / 2, title_y + 11, 22);
        self.draw_text_rendered(title_text, title_start_x + icon_w + gap + 1, title_y + 1, title_scale, Color::RGB(100, 75, 10));
        self.draw_text_rendered(title_text, title_start_x + icon_w + gap, title_y, title_scale, Color::RGB(251, 191, 36));

        // Subtitle
        let subtitle = i18n::t(lang, "trophies_sub");
        self.draw_text_centered(subtitle, center_x, dialog.y() + 66, 1, Color::RGB(148, 163, 184));

        // -------------------------------------------------------------
        // SECTION 1: CAREER OVERVIEW
        // -------------------------------------------------------------
        let s1_title = i18n::t(lang, "career_overview");
        let s1_y = dialog.y() + 84;
        self.draw_chart_icon(dialog.x() + 30, s1_y + 4, 14);
        self.draw_text_rendered(s1_title, dialog.x() + 44, s1_y, 1, Color::RGB(148, 163, 184));
        self.canvas.set_draw_color(Color::RGB(30, 48, 75));
        self.canvas.draw_line(
            sdl2::rect::Point::new(dialog.x() + 152, s1_y + 4),
            sdl2::rect::Point::new(dialog.x() + dialog.width() as i32 - 25, s1_y + 4),
        ).ok();

        let card_w: u32 = 295;
        let card_h: u32 = 74;
        let card_bg = Color::RGB(17, 27, 49);
        let card_border = Color::RGB(35, 56, 93);
        let s1_card_y = s1_y + 14;

        // Card 1: HIGHEST LEVEL COMPLETED
        let c1_x = dialog.x() + 25;
        let c1_center = c1_x + (card_w as i32) / 2;
        let c1_rect = Rect::new(c1_x, s1_card_y, card_w, card_h);
        self.draw_rounded_card(c1_rect, card_bg, card_border);
        self.draw_mountain_icon(c1_center, s1_card_y + 10, 14);

        let lbl1 = i18n::t(lang, "highest_level_completed");
        self.draw_text_centered(lbl1, c1_center, s1_card_y + 20, 1, Color::RGB(148, 163, 184));

        let val1 = format!("{}: {}", i18n::t(lang, "level"), highest_level);
        self.draw_text_centered(&val1, c1_center, s1_card_y + 32, 3, Color::RGB(250, 204, 21));

        let sub1 = i18n::t(lang, "out_of_1000");
        self.draw_text_centered(sub1, c1_center, s1_card_y + 58, 1, Color::RGB(100, 116, 139));

        // Card 2: TOTAL SCORE CURRENTLY
        let c2_x = dialog.x() + 340;
        let c2_center = c2_x + (card_w as i32) / 2;
        let c2_rect = Rect::new(c2_x, s1_card_y, card_w, card_h);
        self.draw_rounded_card(c2_rect, card_bg, card_border);
        self.draw_star_icon(c2_center, s1_card_y + 10, 7, Color::RGB(250, 204, 21));

        let lbl2 = i18n::t(lang, "total_score_curr");
        self.draw_text_centered(lbl2, c2_center, s1_card_y + 20, 1, Color::RGB(148, 163, 184));

        let val2 = Self::format_number_commas(total_score);
        self.draw_text_centered(&val2, c2_center, s1_card_y + 32, 3, Color::RGB(56, 189, 248));

        let sub2 = i18n::t(lang, "accum_points");
        self.draw_text_centered(sub2, c2_center, s1_card_y + 58, 1, Color::RGB(100, 116, 139));

        // -------------------------------------------------------------
        // SECTION 2: DAILY CONSISTENCY STREAKS
        // -------------------------------------------------------------
        let s2_title = i18n::t(lang, "daily_consistency");
        let s2_y = s1_card_y + card_h as i32 + 10;
        self.draw_flame_icon(dialog.x() + 30, s2_y + 4, 12);
        self.draw_text_rendered(s2_title, dialog.x() + 44, s2_y, 1, Color::RGB(148, 163, 184));
        self.canvas.set_draw_color(Color::RGB(30, 48, 75));
        self.canvas.draw_line(
            sdl2::rect::Point::new(dialog.x() + 215, s2_y + 4),
            sdl2::rect::Point::new(dialog.x() + dialog.width() as i32 - 25, s2_y + 4),
        ).ok();

        let s2_card_y = s2_y + 14;

        // Card 3: CURRENT DAY STREAK
        let c3_x = dialog.x() + 25;
        let c3_center = c3_x + (card_w as i32) / 2;
        let c3_rect = Rect::new(c3_x, s2_card_y, card_w, card_h);
        self.draw_rounded_card(c3_rect, card_bg, card_border);
        self.draw_flame_icon(c3_center, s2_card_y + 10, 14);

        let lbl3 = i18n::t(lang, "current_day_streak");
        self.draw_text_centered(lbl3, c3_center, s2_card_y + 20, 1, Color::RGB(148, 163, 184));

        let val3 = format!("{} {}", current_streak, if current_streak == 1 { i18n::t(lang, "day") } else { i18n::t(lang, "days") });
        self.draw_text_centered(&val3, c3_center, s2_card_y + 32, 3, Color::RGB(251, 146, 60));

        let sub3 = i18n::t(lang, "active_today");
        self.draw_text_centered(sub3, c3_center, s2_card_y + 58, 1, Color::RGB(100, 116, 139));

        // Card 4: BEST STREAK RECORD
        let c4_x = dialog.x() + 340;
        let c4_center = c4_x + (card_w as i32) / 2;
        let c4_rect = Rect::new(c4_x, s2_card_y, card_w, card_h);
        self.draw_rounded_card(c4_rect, card_bg, card_border);
        self.draw_crown_icon(c4_center, s2_card_y + 10, 14);

        let lbl4 = i18n::t(lang, "best_streak_record");
        self.draw_text_centered(lbl4, c4_center, s2_card_y + 20, 1, Color::RGB(148, 163, 184));

        let val4 = format!("{} {}", best_streak, if best_streak == 1 { i18n::t(lang, "day") } else { i18n::t(lang, "days") });
        self.draw_text_centered(&val4, c4_center, s2_card_y + 32, 3, Color::RGB(251, 146, 60));

        let sub4 = i18n::t(lang, "consecutive_days");
        self.draw_text_centered(sub4, c4_center, s2_card_y + 58, 1, Color::RGB(100, 116, 139));

        // Daily Gift Banner Box
        let banner_y = s2_card_y + card_h as i32 + 6;
        let banner_w: u32 = 610;
        let banner_h: u32 = 22;
        let banner_rect = Rect::new(dialog.x() + 25, banner_y, banner_w, banner_h);
        self.draw_rounded_card(banner_rect, Color::RGB(9, 44, 32), Color::RGB(16, 185, 129));
        
        self.draw_gift_icon(dialog.x() + 40, banner_y + 11, 12);
        let banner_text = i18n::t(lang, "daily_gift_granted");
        self.draw_text_rendered(banner_text, dialog.x() + 54, banner_y + 7, 1, Color::RGB(52, 211, 153));

        // -------------------------------------------------------------
        // SECTION 3: MASTERY & CLEAN CLEARANCES
        // -------------------------------------------------------------
        let s3_title = i18n::t(lang, "mastery_clean");
        let s3_y = banner_y + banner_h as i32 + 10;
        self.draw_trophy_icon(dialog.x() + 30, s3_y + 4, 12);
        self.draw_text_rendered(s3_title, dialog.x() + 44, s3_y, 1, Color::RGB(148, 163, 184));
        self.canvas.set_draw_color(Color::RGB(30, 48, 75));
        self.canvas.draw_line(
            sdl2::rect::Point::new(dialog.x() + 235, s3_y + 4),
            sdl2::rect::Point::new(dialog.x() + dialog.width() as i32 - 25, s3_y + 4),
        ).ok();

        let s3_card_y = s3_y + 14;
        let card3_w: u32 = 196;
        let card3_h: u32 = 74;

        // Card 5: NO HINTS
        let c5_x = dialog.x() + 25;
        let c5_center = c5_x + (card3_w as i32) / 2;
        let c5_rect = Rect::new(c5_x, s3_card_y, card3_w, card3_h);
        self.draw_rounded_card(c5_rect, card_bg, card_border);
        self.draw_bulb_icon(c5_center, s3_card_y + 10, 14);

        let lbl5 = i18n::t(lang, "no_hints");
        self.draw_text_centered(lbl5, c5_center, s3_card_y + 20, 1, Color::RGB(148, 163, 184));

        let val5 = format!("{}", trophy_state.no_hints_count);
        self.draw_text_centered(&val5, c5_center, s3_card_y + 32, 3, Color::RGB(74, 222, 128));

        let sub5 = i18n::t(lang, "zero_hints");
        self.draw_text_centered(sub5, c5_center, s3_card_y + 58, 1, Color::RGB(100, 116, 139));

        // Card 6: NO UNDOS
        let c6_x = dialog.x() + 232;
        let c6_center = c6_x + (card3_w as i32) / 2;
        let c6_rect = Rect::new(c6_x, s3_card_y, card3_w, card3_h);
        self.draw_rounded_card(c6_rect, card_bg, card_border);
        self.draw_undo_icon(c6_center, s3_card_y + 10, 14);

        let lbl6 = i18n::t(lang, "no_undos");
        self.draw_text_centered(lbl6, c6_center, s3_card_y + 20, 1, Color::RGB(148, 163, 184));

        let val6 = format!("{}", trophy_state.no_undos_count);
        self.draw_text_centered(&val6, c6_center, s3_card_y + 32, 3, Color::RGB(192, 132, 252));

        let sub6 = i18n::t(lang, "zero_undos");
        self.draw_text_centered(sub6, c6_center, s3_card_y + 58, 1, Color::RGB(100, 116, 139));

        // Card 7: NO LIVES USED (Zero shuffles)
        let c7_x = dialog.x() + 439;
        let c7_center = c7_x + (card3_w as i32) / 2;
        let c7_rect = Rect::new(c7_x, s3_card_y, card3_w, card3_h);
        self.draw_rounded_card(c7_rect, card_bg, card_border);
        self.draw_heart_icon(c7_center, s3_card_y + 10, 14, Color::RGB(244, 63, 94));

        let lbl7 = i18n::t(lang, "no_lives_used");
        self.draw_text_centered(lbl7, c7_center, s3_card_y + 20, 1, Color::RGB(148, 163, 184));

        let val7 = format!("{}", trophy_state.no_shuffles_count);
        self.draw_text_centered(&val7, c7_center, s3_card_y + 32, 3, Color::RGB(56, 189, 248));

        let sub7 = i18n::t(lang, "zero_shuffles");
        self.draw_text_centered(sub7, c7_center, s3_card_y + 58, 1, Color::RGB(100, 116, 139));

        // -------------------------------------------------------------
        // SECTION 4: ACTION BUTTONS (Save Stats Image & Close)
        // -------------------------------------------------------------
        let (btn1_rect, btn2_rect) = Self::leaderboard_buttons(win_w, win_h);
        let btn_y = btn1_rect.y();
        let btn1_x = btn1_rect.x();
        let btn_w = btn1_rect.width();
        let btn_h = btn1_rect.height();

        // Left button: 💾 SAVE STATS IMAGE
        let btn1_bg = if is_saved_active { Color::RGB(16, 185, 129) } else { Color::RGB(79, 70, 229) };
        let btn1_border = if is_saved_active { Color::RGB(52, 211, 153) } else { Color::RGB(129, 140, 248) };
        self.draw_rounded_card(btn1_rect, btn1_bg, btn1_border);

        if is_saved_active {
            let label = "✓ SAVED!";
            self.draw_text_centered(label, btn1_x + (btn_w as i32) / 2, btn_y + (btn_h as i32 - 14) / 2, 2, Color::RGB(255, 255, 255));
        } else {
            let label = i18n::t(lang, "save_stats_image");
            let icon_w: i32 = 16;
            let gap: i32 = 10;
            let text_scale = if self.text_width(label, 2) as i32 + icon_w + gap <= btn_w as i32 - 20 { 2u32 } else { 1u32 };
            let text_w = self.text_width(label, text_scale) as i32;
            let text_h = (7 * text_scale) as i32;
            let total_content_w = icon_w + gap + text_w;
            let start_x = btn1_x + (btn_w as i32 - total_content_w) / 2;
            self.draw_floppy_icon(start_x + icon_w / 2, btn_y + (btn_h as i32) / 2, 16);
            self.draw_text_rendered(label, start_x + icon_w + gap, btn_y + (btn_h as i32 - text_h) / 2, text_scale, Color::RGB(255, 255, 255));
        }

        // Right button: ✖ CLOSE
        let btn2_x = btn2_rect.x();
        let btn2_w = btn2_rect.width();
        let btn2_h = btn2_rect.height();
        self.draw_rounded_card(btn2_rect, Color::RGB(2, 132, 199), Color::RGB(56, 189, 248));

        let label2 = i18n::t(lang, "close");
        let icon_w: i32 = 12;
        let gap: i32 = 10;
        let text_scale = if self.text_width(label2, 2) as i32 + icon_w + gap <= btn2_w as i32 - 20 { 2u32 } else { 1u32 };
        let text_w = self.text_width(label2, text_scale) as i32;
        let text_h = (7 * text_scale) as i32;
        let total_content_w = icon_w + gap + text_w;
        let start_x = btn2_x + (btn2_w as i32 - total_content_w) / 2;
        self.draw_cross_icon(start_x + icon_w / 2, btn_y + (btn2_h as i32) / 2, 12, Color::RGB(255, 255, 255));
        self.draw_text_rendered(label2, start_x + icon_w + gap, btn_y + (btn2_h as i32 - text_h) / 2, text_scale, Color::RGB(255, 255, 255));
    }

    /// Renders the level select screen allowing the user to browse and replay any unlocked level.
    pub fn render_level_select(&mut self, user_name: &str, progress: &UserProgress, page: usize, selected_level: u32, lang: Language) {
        self.draw_overlay_backdrop();

        let dialog_w: u32 = 720;
        let dialog_h: u32 = 590;
        let dialog = self.draw_dialog_box(dialog_w, dialog_h);

        // Title (centered, scale 3)
        let title = i18n::t(lang, "select_level");
        let center_x = dialog.x() + dialog.width() as i32 / 2;
        self.draw_text_centered(
            title,
            center_x,
            dialog.y() + 16,
            3,
            Color::RGB(255, 215, 0),
        );

        // Header info: completed count & page
        let completed_count = progress.completed_levels.len().max(progress.max_completed_level as usize);
        let total_pages = 40; // 1000 / 25
        let current_page = page.min(total_pages - 1);
        let page_info = i18n::t_param(lang, "page_info", &[("current", &(current_page + 1).to_string()), ("total", &total_pages.to_string())]);
        let info_text = format!("USER: {}   COMPLETED: {}/1000   {}", user_name, completed_count, page_info);
        self.draw_text_centered(
            &info_text,
            center_x,
            dialog.y() + 50,
            2,
            Color::RGB(150, 220, 255),
        );

        // Phase Quick Jump Tabs
        let tabs: &[(&str, usize)] = &[
            (i18n::t(lang, "phase_penguin_short"), 0),
            (i18n::t(lang, "phase_dog_short"), 0),
            (i18n::t(lang, "phase_space_short"), 0),
            (i18n::t(lang, "phase_endgame_short"), 2),
            (i18n::t(lang, "phase_grandmaster_short"), 4),
        ];
        let tab_w: u32 = 124;
        let tab_h: u32 = 28;
        let tab_gap: i32 = 8;
        let tabs_total_w = (tab_w as i32 * tabs.len() as i32) + (tab_gap * (tabs.len() as i32 - 1));
        let tab_start_x = dialog.x() + (dialog.width() as i32 - tabs_total_w) / 2;
        let tab_y = dialog.y() + 78;

        for (i, (tab_label, target_page)) in tabs.iter().enumerate() {
            let tx = tab_start_x + i as i32 * (tab_w as i32 + tab_gap);
            let is_active_phase = current_page == *target_page;
            let tab_color = if is_active_phase {
                Color::RGB(60, 130, 180)
            } else {
                Color::RGB(40, 50, 70)
            };
            self.draw_labeled_button(tx, tab_y, tab_w, tab_h, tab_color, tab_label);
        }

        // 5x5 Grid of Levels (25 per page)
        let grid_cols = 5;
        let grid_rows = 5;
        let cell_w: u32 = 116;
        let cell_h: u32 = 54;
        let cell_gap_x: i32 = 12;
        let cell_gap_y: i32 = 10;
        let grid_total_w = (cell_w as i32 * grid_cols) + (cell_gap_x * (grid_cols - 1));
        let grid_start_x = dialog.x() + (dialog.width() as i32 - grid_total_w) / 2;
        let grid_start_y = dialog.y() + 116;

        let page_start_level = (current_page * 25 + 1) as u32;
        let play_text = i18n::t(lang, "play");

        for row in 0..grid_rows {
            for col in 0..grid_cols {
                let idx = row * grid_cols + col;
                let level = page_start_level + idx as u32;
                if level > 1000 {
                    continue;
                }

                let cx = grid_start_x + col * (cell_w as i32 + cell_gap_x);
                let cy = grid_start_y + row * (cell_h as i32 + cell_gap_y);

                let is_completed = progress.is_level_completed(level);
                let is_unlocked = progress.is_level_unlocked(level);
                let is_selected = level == selected_level;

                let cell_rect = Rect::new(cx, cy, cell_w, cell_h);

                // Background and text colors
                let (bg_color, border_color, text_color, status_color, status_text) = if is_completed {
                    (
                        Color::RGB(25, 70, 55),
                        Color::RGB(50, 150, 100),
                        Color::RGB(255, 255, 255),
                        Color::RGB(100, 240, 140),
                        "CLEARED",
                    )
                } else if is_unlocked {
                    (
                        Color::RGB(30, 60, 110),
                        Color::RGB(70, 130, 220),
                        Color::RGB(200, 230, 255),
                        Color::RGB(100, 200, 255),
                        play_text,
                    )
                } else {
                    (
                        Color::RGB(28, 30, 38),
                        Color::RGB(50, 55, 68),
                        Color::RGB(100, 105, 120),
                        Color::RGB(80, 85, 95),
                        "LOCKED",
                    )
                };

                // Draw cell background
                self.canvas.set_draw_color(bg_color);
                self.canvas.fill_rect(cell_rect).ok();
                self.canvas.set_draw_color(border_color);
                self.canvas.draw_rect(cell_rect).ok();

                // Draw level number
                let lvl_str = format!("{}: {}", i18n::t(lang, "level"), level);
                let cell_center_x = cx + (cell_w as i32) / 2;
                self.draw_text_centered(
                    &lvl_str,
                    cell_center_x,
                    cy + 8,
                    1,
                    text_color,
                );

                // Draw status text (scale 1)
                self.draw_text_centered(
                    status_text,
                    cell_center_x,
                    cy + 34,
                    1,
                    status_color,
                );

                // Highlight border if selected
                if is_selected {
                    let sel_outer = Rect::new(cx - 2, cy - 2, cell_w + 4, cell_h + 4);
                    self.canvas.set_draw_color(Color::RGB(255, 255, 255));
                    self.canvas.draw_rect(sel_outer).ok();
                    let sel_inner = Rect::new(cx - 1, cy - 1, cell_w + 2, cell_h + 2);
                    self.canvas.draw_rect(sel_inner).ok();
                }
            }
        }

        // Bottom Navigation Bar
        let nav_y = dialog.y() + 444;
        let nav_btn_w: u32 = 130;
        let nav_btn_h: u32 = 38;

        // < PREV button
        let prev_x = grid_start_x;
        let prev_color = if current_page > 0 {
            Color::RGB(50, 100, 160)
        } else {
            Color::RGB(40, 45, 55)
        };
        let prev_label = format!("< {}", i18n::t(lang, "prev_page"));
        self.draw_labeled_button(prev_x, nav_y, nav_btn_w, nav_btn_h, prev_color, &prev_label);

        // Page indicator in middle
        let page_str = i18n::t_param(lang, "page_info", &[("current", &(current_page + 1).to_string()), ("total", &total_pages.to_string())]);
        self.draw_text_centered(
            &page_str,
            center_x,
            nav_y + 10,
            2,
            Color::RGB(200, 220, 240),
        );

        // NEXT > button
        let next_x = grid_start_x + grid_total_w - nav_btn_w as i32;
        let next_color = if current_page + 1 < total_pages {
            Color::RGB(50, 100, 160)
        } else {
            Color::RGB(40, 45, 55)
        };
        let next_label = format!("{} >", i18n::t(lang, "next_page"));
        self.draw_labeled_button(next_x, nav_y, nav_btn_w, nav_btn_h, next_color, &next_label);

        // BACK button
        let back_w: u32 = 160;
        let back_h: u32 = 40;
        let back_x = dialog.x() + (dialog.width() as i32 - back_w as i32) / 2;
        let back_y = dialog.y() + 494;
        let back_str = i18n::t(lang, "back_btn");
        self.draw_labeled_button(back_x, back_y, back_w, back_h, Color::RGB(100, 100, 100), back_str);

        // Footer hint
        let hint = "ARROWS NAVIGATE  ENTER PLAY  PGUP/DN PAGE  ESC BACK";
        let hint_w = hint.chars().count() as i32 * 6;
        self.draw_bitmap_text(
            hint,
            dialog.x() + (dialog.width() as i32 - hint_w) / 2,
            dialog.y() + 550,
            1,
            Color::RGB(130, 130, 150),
        );
    }

    /// Renders a beautiful daily play streak achievement popup.
    pub fn render_daily_streak_popup(&mut self, streak: u32, lang: Language) {
        self.draw_overlay_backdrop();

        let dialog = self.draw_dialog_box(380, 240);
        let center_x = dialog.x() + dialog.width() as i32 / 2;

        // Neon cyan title
        let title = i18n::t(lang, "daily_streak_title");
        self.draw_text_centered(
            title,
            center_x,
            dialog.y() + 25,
            2,
            Color::RGB(0, 255, 200),
        );

        // Subtitle
        let subtitle = i18n::t(lang, "daily_streak_sub");
        self.draw_text_centered(
            subtitle,
            center_x,
            dialog.y() + 75,
            1,
            Color::RGB(180, 180, 220),
        );

        // Day count
        let day_text = format!("{}: {}", i18n::t(lang, "day"), streak);
        self.draw_text_centered(
            &day_text,
            center_x,
            dialog.y() + 115,
            3,
            Color::RGB(255, 215, 0), // Gold
        );

        // Button
        let btn_w: u32 = 180;
        let btn_h: u32 = 40;
        let btn_x = dialog.x() + ((dialog.width() - btn_w) / 2) as i32;
        let btn_y = dialog.y() + 180;
        let continue_str = i18n::t(lang, "continue_btn");
        self.draw_labeled_button(btn_x, btn_y, btn_w, btn_h, Color::RGB(0, 150, 150), continue_str);
    }

    /// Renders the no-moves notification with options to shuffle or start a new game.
    pub fn render_no_moves(&mut self, selected: usize, lang: Language) {
        self.draw_overlay_backdrop();

        let dialog = self.draw_dialog_box(320, 220);
        let center_x = dialog.x() + dialog.width() as i32 / 2;

        // "NO MOVES!" title
        let title = i18n::t(lang, "no_more_moves");
        self.draw_text_centered(
            title,
            center_x,
            dialog.y() + 22,
            2,
            Color::RGB(255, 120, 80),
        );

        // Explanatory text
        let sub = i18n::t(lang, "out_of_lives_sub");
        self.draw_text_centered(
            sub,
            center_x,
            dialog.y() + 68,
            1,
            Color::RGB(180, 180, 200),
        );

        let btn_w: u32 = 200;
        let btn_h: u32 = 44;
        let btn_x = dialog.x() + ((dialog.width() - btn_w) / 2) as i32;

        let shuffle_str = i18n::t(lang, "shuffle");
        let new_game_str = i18n::t(lang, "new_game");

        let buttons: &[(i32, Color, &str)] = &[
            (110, Color::RGB(120, 60, 160), shuffle_str),
            (164, Color::RGB(50, 140, 70), new_game_str),
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
    pub fn render_hint_suggestion(&mut self, lang: Language) {
        let (win_w, win_h) = self.window_size();

        let text = i18n::t(lang, "hint_suggestion");
        let scale: u32 = 2;
        let text_w = self.text_width(text, scale);
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
        self.draw_text_rendered(text, text_x, text_y, scale, Color::RGBA(180, 220, 255, 220));
    }

    /// Renders the game over dialog when no moves and no shuffles remain.
    pub fn render_game_over(&mut self, score: u32, time_seconds: u32, hints_used: u32, shuffles_used: u32, level: u32, selected: usize, lang: Language) {
        self.draw_overlay_backdrop();

        let dialog = self.draw_dialog_box(380, 374);
        let center_x = dialog.x() + dialog.width() as i32 / 2;

        // "GAME OVER" title (red)
        let title = i18n::t(lang, "out_of_lives");
        self.draw_text_centered(
            title,
            center_x,
            dialog.y() + 20,
            2,
            Color::RGB(255, 80, 80),
        );

        // Explanatory text
        let sub = i18n::t(lang, "out_of_lives_sub");
        self.draw_text_centered(
            sub,
            center_x,
            dialog.y() + 62,
            1,
            Color::RGB(180, 180, 200),
        );

        // Stats display
        let level_text = format!("{}:  {}", i18n::t(lang, "level"), level);
        self.draw_text_rendered(
            &level_text,
            dialog.x() + 50,
            dialog.y() + 100,
            2,
            Color::RGB(200, 200, 220),
        );

        let score_text = format!("{}:  {}", i18n::t(lang, "score"), score);
        self.draw_text_rendered(
            &score_text,
            dialog.x() + 50,
            dialog.y() + 124,
            2,
            Color::RGB(255, 200, 50),
        );

        let minutes = time_seconds / 60;
        let seconds = time_seconds % 60;
        let time_text = format!("{}:   {:02}:{:02}", i18n::t(lang, "time"), minutes, seconds);
        self.draw_text_rendered(
            &time_text,
            dialog.x() + 50,
            dialog.y() + 148,
            2,
            Color::RGB(100, 220, 100),
        );

        let hints_text = format!("{}:  {}", i18n::t(lang, "hints"), hints_used);
        self.draw_text_rendered(
            &hints_text,
            dialog.x() + 50,
            dialog.y() + 172,
            2,
            Color::RGB(150, 180, 255),
        );

        let shuffles_text = format!("{}:  {}", i18n::t(lang, "lives"), shuffles_used);
        self.draw_text_rendered(
            &shuffles_text,
            dialog.x() + 200,
            dialog.y() + 172,
            2,
            Color::RGB(180, 130, 255),
        );

        let btn_w: u32 = 220;
        let btn_h: u32 = 44;
        let btn_x = dialog.x() + ((dialog.width() - btn_w) / 2) as i32;

        let save_score_str = i18n::t(lang, "save_score");
        let new_game_str = i18n::t(lang, "new_game");
        let wait_shuffle_str = i18n::t(lang, "wait_for_shuffle");

        let buttons: &[(i32, Color, &str)] = &[
            (210, Color::RGB(180, 140, 30), save_score_str),
            (264, Color::RGB(50, 140, 70), new_game_str),
            (318, Color::RGB(80, 100, 180), wait_shuffle_str),
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

    /// Renders the player selection and name entry dialog.
    pub fn render_name_entry(&mut self, state: &NameEntryState, lang: Language) {
        self.draw_overlay_backdrop();

        let is_startup = state.score == 0 && state.time_seconds == 0;

        if !is_startup {
            // High score screen (for games completed that qualified for leaderboard)
            let dialog = self.draw_dialog_box(400, 280);

            let title = i18n::t(lang, "trophies_title");
            let title_w = title.chars().count() as i32 * 6 * 2;
            self.draw_text_rendered(
                title,
                dialog.x() + (dialog.width() as i32 - title_w) / 2,
                dialog.y() + 18,
                2,
                Color::RGB(255, 215, 0),
            );

            let score_text = format!("{}:  {}", i18n::t(lang, "score"), state.score);
            self.draw_text_rendered(
                &score_text,
                dialog.x() + 130,
                dialog.y() + 58,
                2,
                Color::RGB(255, 200, 50),
            );

            let minutes = state.time_seconds / 60;
            let seconds = state.time_seconds % 60;
            let time_text = format!("{}:  {:02}:{:02}", i18n::t(lang, "time"), minutes, seconds);
            self.draw_text_rendered(
                &time_text,
                dialog.x() + 130,
                dialog.y() + 82,
                2,
                Color::RGB(100, 200, 100),
            );

            let enter_label = i18n::t(lang, "enter_name");
            self.draw_text_rendered(
                enter_label,
                dialog.x() + 40,
                dialog.y() + 118,
                2,
                Color::RGB(200, 200, 220),
            );

            let input_x = dialog.x() + 40;
            let input_y = dialog.y() + 148;
            let input_w: u32 = 320;
            let input_h: u32 = 36;
            let input_rect = Rect::new(input_x, input_y, input_w, input_h);

            self.canvas.set_draw_color(Color::RGB(25, 25, 35));
            self.canvas.fill_rect(input_rect).ok();
            self.canvas.set_draw_color(Color::RGB(100, 160, 220));
            self.canvas.draw_rect(input_rect).ok();

            if !state.text.is_empty() {
                let display_name: String = state.text.to_uppercase();
                self.draw_text_rendered(
                    &display_name,
                    input_x + 6,
                    input_y + 10,
                    2,
                    Color::RGB(220, 220, 240),
                );
            }

            let char_width = 12i32;
            let cursor_x = input_x + 6 + (state.text.chars().count() as i32 * char_width).min((input_w as i32) - 14);
            let cursor_rect = Rect::new(cursor_x, input_y + 8, 2, input_h - 16);
            self.canvas.set_draw_color(Color::RGB(200, 220, 255));
            self.canvas.fill_rect(cursor_rect).ok();

            self.draw_bitmap_text(
                "ENTER TO SUBMIT  ESC TO SKIP",
                dialog.x() + 52,
                dialog.y() + 198,
                1,
                Color::RGB(140, 140, 160),
            );

            let char_count = state.text.chars().count();
            let count_text = format!("{}/20", char_count);
            let count_color = if char_count == 0 {
                Color::RGB(200, 80, 80)
            } else {
                Color::RGB(100, 200, 100)
            };
            self.draw_bitmap_text(
                &count_text,
                dialog.x() + 320,
                dialog.y() + 248,
                1,
                count_color,
            );
            return;
        }

        // --- Startup / Switch User Dialog ---
        if state.profiles.is_empty() {
            // Case A: No existing saved profiles
            let dialog = self.draw_dialog_box(420, 300);

            // Title
            let title = "xMahjong";
            let title_w = title.chars().count() as i32 * 6 * 3;
            self.draw_text_rendered(
                title,
                dialog.x() + (dialog.width() as i32 - title_w) / 2,
                dialog.y() + 24,
                3,
                Color::RGB(255, 215, 0),
            );

            let sub = i18n::t(lang, "enter_name");
            let sub_w = sub.chars().count() as i32 * 6 * 2;
            self.draw_text_rendered(
                sub,
                dialog.x() + (dialog.width() as i32 - sub_w) / 2,
                dialog.y() + 75,
                2,
                Color::RGB(200, 200, 220),
            );

            let input_x = dialog.x() + 40;
            let input_y = dialog.y() + 130;
            let input_w: u32 = 340;
            let input_h: u32 = 40;
            let input_rect = Rect::new(input_x, input_y, input_w, input_h);

            self.canvas.set_draw_color(Color::RGB(25, 25, 35));
            self.canvas.fill_rect(input_rect).ok();
            self.canvas.set_draw_color(Color::RGB(100, 160, 220));
            self.canvas.draw_rect(input_rect).ok();

            if !state.text.is_empty() {
                let display_name: String = state.text.to_uppercase();
                self.draw_text_rendered(
                    &display_name,
                    input_x + 8,
                    input_y + 12,
                    2,
                    Color::RGB(220, 220, 240),
                );
            }

            let char_width = 12i32;
            let cursor_x = input_x + 8 + (state.text.chars().count() as i32 * char_width).min((input_w as i32) - 14);
            let cursor_rect = Rect::new(cursor_x, input_y + 8, 2, input_h - 16);
            self.canvas.set_draw_color(Color::RGB(200, 220, 255));
            self.canvas.fill_rect(cursor_rect).ok();

            let instructions = i18n::t(lang, "press_enter_to_start");
            self.draw_text_centered(
                instructions,
                dialog.x() + (dialog.width() as i32) / 2,
                dialog.y() + 195,
                1,
                Color::RGB(140, 140, 160),
            );

            let char_count = state.text.chars().count();
            let count_text = format!("{}/20", char_count);
            let count_color = if char_count == 0 {
                Color::RGB(200, 80, 80)
            } else {
                Color::RGB(100, 200, 100)
            };
            self.draw_bitmap_text(
                &count_text,
                dialog.x() + 340,
                dialog.y() + 250,
                1,
                count_color,
            );
        } else {
            // Case B: Existing profiles exist
            let dialog_w: u32 = 480;
            let dialog_h: u32 = 500;
            let dialog = self.draw_dialog_box(dialog_w, dialog_h);

            // Title
            let title = "xMahjong";
            let title_w = title.chars().count() as i32 * 6 * 3;
            self.draw_text_rendered(
                title,
                dialog.x() + (dialog.width() as i32 - title_w) / 2,
                dialog.y() + 16,
                3,
                Color::RGB(255, 215, 0),
            );

            // Subtitle
            let subtitle = i18n::t(lang, "select_player");
            self.draw_text_centered(
                subtitle,
                dialog.x() + (dialog.width() as i32) / 2,
                dialog.y() + 48,
                1,
                Color::RGB(160, 200, 230),
            );

            // Section 1 Header: Existing Players
            let section1 = i18n::t(lang, "select_player");
            self.draw_text_rendered(
                section1,
                dialog.x() + 30,
                dialog.y() + 72,
                1,
                Color::RGB(255, 215, 0),
            );

            // Pagination info (if more than 1 page)
            let total_pages = state.total_pages();
            if total_pages > 1 {
                let page_info = i18n::t_param(lang, "page_info", &[("current", &(state.page + 1).to_string()), ("total", &total_pages.to_string())]);
                self.draw_text_rendered(
                    &page_info,
                    dialog.x() + 320,
                    dialog.y() + 72,
                    1,
                    Color::RGB(180, 190, 210),
                );

                // Prev/Next buttons
                let prev_color = if state.page > 0 { Color::RGB(50, 90, 140) } else { Color::RGB(35, 45, 60) };
                let next_color = if state.page + 1 < total_pages { Color::RGB(50, 90, 140) } else { Color::RGB(35, 45, 60) };
                self.draw_labeled_button(dialog.x() + 270, dialog.y() + 68, 42, 18, prev_color, "<");
                self.draw_labeled_button(dialog.x() + 408, dialog.y() + 68, 42, 18, next_color, ">");
            }

            // Cards for profiles on current page
            let start_idx = state.page_start_index();
            let end_idx = state.page_end_index();
            let card_w: u32 = 420;
            let card_h: u32 = 54;
            let card_x = dialog.x() + 30;
            let start_y = dialog.y() + 92;
            let card_spacing = 62i32;

            for (i, p_idx) in (start_idx..end_idx).enumerate() {
                let profile = &state.profiles[p_idx];
                let is_selected = p_idx == state.selected_index;
                let cy = start_y + (i as i32) * card_spacing;
                let card_rect = Rect::new(card_x, cy, card_w, card_h);

                // Card background and border
                let (bg_color, border_color) = if is_selected {
                    (Color::RGB(35, 65, 105), Color::RGB(100, 200, 255))
                } else {
                    (Color::RGB(25, 30, 42), Color::RGB(55, 65, 85))
                };

                self.canvas.set_draw_color(bg_color);
                self.canvas.fill_rect(card_rect).ok();
                self.canvas.set_draw_color(border_color);
                self.canvas.draw_rect(card_rect).ok();

                if is_selected {
                    // Double border for high visibility
                    let inner_rect = Rect::new(card_x + 1, cy + 1, card_w - 2, card_h - 2);
                    self.canvas.set_draw_color(Color::RGB(255, 255, 255));
                    self.canvas.draw_rect(inner_rect).ok();

                    // Selection arrow ">"
                    self.draw_bitmap_text(">", card_x + 8, cy + 18, 2, Color::RGB(255, 215, 0));
                }

                // Profile Name (uppercase, scale 2)
                let name_display = profile.name.to_uppercase();
                let name_x = card_x + 26;
                let name_color = if is_selected { Color::RGB(255, 255, 255) } else { Color::RGB(210, 220, 235) };
                self.draw_text_rendered(&name_display, name_x, cy + 14, 2, name_color);

                // Badges on the right
                let mut badge_right_x = card_x + card_w as i32 - 12;

                // Streak badge if > 1
                if profile.streak > 1 {
                    let streak_str = format!("STRK:{}", profile.streak);
                    let s_w = streak_str.len() as i32 * 6;
                    badge_right_x -= s_w + 10;
                    self.draw_bitmap_text(&streak_str, badge_right_x, cy + 34, 1, Color::RGB(255, 180, 50));
                }

                // Save or Progress Badge
                if profile.has_save {
                    let badge_text = i18n::t_param(lang, "resume_lvl", &[("level", &profile.save_level.to_string())]);
                    let b_w = self.text_width(&badge_text, 1) as i32;
                    let b_rect_w = (b_w + 12) as u32;
                    let b_x = card_x + card_w as i32 - b_rect_w as i32 - 12;
                    let b_rect = Rect::new(b_x, cy + 10, b_rect_w, 20);

                    self.canvas.set_draw_color(Color::RGB(30, 90, 55));
                    self.canvas.fill_rect(b_rect).ok();
                    self.canvas.set_draw_color(Color::RGB(60, 180, 110));
                    self.canvas.draw_rect(b_rect).ok();
                    self.draw_text_rendered(&badge_text, b_x + 6, cy + 14, 1, Color::RGB(160, 255, 190));
                } else if profile.max_completed_level > 0 {
                    let badge_text = format!("LVL {}/1000", profile.max_completed_level);
                    let b_w = badge_text.len() as i32 * 6;
                    let b_rect_w = (b_w + 12) as u32;
                    let b_x = card_x + card_w as i32 - b_rect_w as i32 - 12;
                    let b_rect = Rect::new(b_x, cy + 10, b_rect_w, 20);

                    self.canvas.set_draw_color(Color::RGB(30, 55, 95));
                    self.canvas.fill_rect(b_rect).ok();
                    self.canvas.set_draw_color(Color::RGB(65, 120, 200));
                    self.canvas.draw_rect(b_rect).ok();
                    self.draw_bitmap_text(&badge_text, b_x + 6, cy + 14, 1, Color::RGB(160, 210, 255));
                } else {
                    let badge_text = i18n::t(lang, "new");
                    let b_w = self.text_width(badge_text, 1) as i32;
                    let b_rect_w = (b_w + 12) as u32;
                    let b_x = card_x + card_w as i32 - b_rect_w as i32 - 12;
                    let b_rect = Rect::new(b_x, cy + 10, b_rect_w, 20);

                    self.canvas.set_draw_color(Color::RGB(40, 45, 60));
                    self.canvas.fill_rect(b_rect).ok();
                    self.canvas.set_draw_color(Color::RGB(80, 90, 110));
                    self.canvas.draw_rect(b_rect).ok();
                    self.draw_text_rendered(badge_text, b_x + 6, cy + 14, 1, Color::RGB(180, 190, 210));
                }
            }

            // Section 2: Create New Player
            let sep_text = i18n::t(lang, "or_start_new_player");
            self.draw_text_centered(
                sep_text,
                dialog.x() + (dialog.width() as i32) / 2,
                dialog.y() + 290,
                1,
                Color::RGB(120, 130, 155),
            );

            let is_input_focused = state.is_new_player_selected();

            // Text input box
            let input_x = dialog.x() + 30;
            let input_y = dialog.y() + 320;
            let input_w: u32 = 310;
            let input_h: u32 = 38;
            let input_rect = Rect::new(input_x, input_y, input_w, input_h);

            let (in_bg, in_border) = if is_input_focused {
                (Color::RGB(20, 32, 50), Color::RGB(100, 200, 255))
            } else {
                (Color::RGB(20, 22, 30), Color::RGB(55, 65, 80))
            };

            self.canvas.set_draw_color(in_bg);
            self.canvas.fill_rect(input_rect).ok();
            self.canvas.set_draw_color(in_border);
            self.canvas.draw_rect(input_rect).ok();

            if is_input_focused {
                let inner_in = Rect::new(input_x + 1, input_y + 1, input_w - 2, input_h - 2);
                self.canvas.set_draw_color(Color::RGB(255, 255, 255));
                self.canvas.draw_rect(inner_in).ok();
            }

            if !state.text.is_empty() {
                let display_name: String = state.text.to_uppercase();
                self.draw_text_rendered(
                    &display_name,
                    input_x + 8,
                    input_y + 11,
                    2,
                    Color::RGB(240, 240, 255),
                );
            } else if !is_input_focused {
                let placeholder = i18n::t(lang, "enter_name");
                self.draw_text_rendered(
                    placeholder,
                    input_x + 8,
                    input_y + 13,
                    1,
                    Color::RGB(90, 100, 120),
                );
            }

            // Blinking cursor
            if is_input_focused {
                let char_width = 12i32;
                let cursor_x = input_x + 8 + (state.text.chars().count() as i32 * char_width).min((input_w as i32) - 16);
                let cursor_rect = Rect::new(cursor_x, input_y + 8, 2, input_h - 16);
                self.canvas.set_draw_color(Color::RGB(200, 220, 255));
                self.canvas.fill_rect(cursor_rect).ok();
            }

            // Start button
            let btn_x = dialog.x() + 350;
            let btn_y = dialog.y() + 320;
            let btn_w: u32 = 100;
            let btn_h: u32 = 38;
            let btn_color = if state.is_valid() {
                Color::RGB(45, 140, 75)
            } else {
                Color::RGB(40, 60, 50)
            };
            let start_label = i18n::t(lang, "play");
            self.draw_labeled_button(btn_x, btn_y, btn_w, btn_h, btn_color, start_label);

            // Character count indicator (e.g., "3/20")
            let char_count = state.text.chars().count();
            let count_text = format!("{}/20", char_count);
            let count_color = if char_count == 0 {
                Color::RGB(140, 140, 160)
            } else {
                Color::RGB(100, 200, 100)
            };
            self.draw_bitmap_text(
                &count_text,
                dialog.x() + 290,
                dialog.y() + 364,
                1,
                count_color,
            );

            // Instructions footer
            let instr1 = i18n::t(lang, "player_select_nav");
            self.draw_text_centered(
                instr1,
                dialog.x() + (dialog.width() as i32) / 2,
                dialog.y() + 410,
                1,
                Color::RGB(170, 180, 200),
            );

            let instr2 = i18n::t(lang, "player_select_click");
            self.draw_text_centered(
                instr2,
                dialog.x() + (dialog.width() as i32) / 2,
                dialog.y() + 438,
                1,
                Color::RGB(110, 180, 220),
            );

            let instr3 = i18n::t(lang, "esc_quit");
            self.draw_text_centered(
                instr3,
                dialog.x() + (dialog.width() as i32) / 2,
                dialog.y() + 466,
                1,
                Color::RGB(130, 135, 150),
            );
        }
    }

    /// Renders the update available dialog.
    pub fn render_update_dialog(&mut self, current_version: &str, latest_version: &str, lang: Language) {
        self.draw_overlay_backdrop();

        let dialog = self.draw_dialog_box(360, 200);
        let center_x = dialog.x() + dialog.width() as i32 / 2;

        // "UPDATE AVAILABLE" title
        let title = i18n::t(lang, "update_available");
        self.draw_text_centered(
            title,
            center_x,
            dialog.y() + 20,
            2,
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
        let download_label = i18n::t(lang, "download");
        self.draw_labeled_button(btn_start_x, btn_y, btn_w, btn_h, Color::RGB(40, 140, 60), download_label);

        // Not Now button (gray)
        let later_label = i18n::t(lang, "later");
        self.draw_labeled_button(
            btn_start_x + btn_w as i32 + btn_spacing,
            btn_y,
            btn_w,
            btn_h,
            Color::RGB(100, 100, 110),
            later_label,
        );
    }

    /// Renders the quit confirmation dialog.
    pub fn render_quit_confirmation(&mut self, lang: Language) {
        self.draw_overlay_backdrop();

        let dialog = self.draw_dialog_box(320, 180);
        let center_x = dialog.x() + dialog.width() as i32 / 2;

        // "QUIT GAME?" title
        let title = i18n::t(lang, "quit_game");
        self.draw_text_centered(
            title,
            center_x,
            dialog.y() + 24,
            2,
            Color::RGB(255, 160, 160),
        );

        // Subtitle / Prompt
        let sub = i18n::t(lang, "quit_prompt");
        self.draw_text_centered(
            sub,
            center_x,
            dialog.y() + 64,
            1,
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
        let yes_label = i18n::t(lang, "yes");
        self.draw_labeled_button(btn_start_x, btn_y, btn_w, btn_h, Color::RGB(180, 50, 50), yes_label);

        // No button (gray) with label
        let no_label = i18n::t(lang, "no");
        self.draw_labeled_button(
            btn_start_x + btn_w as i32 + btn_spacing,
            btn_y,
            btn_w,
            btn_h,
            Color::RGB(100, 100, 110),
            no_label,
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
    match ch {
        'a' => Some(&[0b00000, 0b01110, 0b00001, 0b01111, 0b10001, 0b10011, 0b01101]),
        'b' => Some(&[0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b11110]),
        'c' => Some(&[0b00000, 0b00000, 0b01110, 0b10000, 0b10000, 0b10001, 0b01110]),
        'd' => Some(&[0b00001, 0b00001, 0b01101, 0b10011, 0b10001, 0b10001, 0b01111]),
        'e' => Some(&[0b00000, 0b00000, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110]),
        'f' => Some(&[0b00110, 0b01001, 0b01000, 0b11110, 0b01000, 0b01000, 0b01000]),
        'g' => Some(&[0b00000, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110]),
        'h' => Some(&[0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b10001]),
        'i' => Some(&[0b00100, 0b00000, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110]),
        'j' => Some(&[0b00010, 0b00000, 0b00110, 0b00010, 0b00010, 0b10010, 0b01100]),
        'k' => Some(&[0b10000, 0b10000, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010]),
        'l' => Some(&[0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
        'm' => Some(&[0b00000, 0b00000, 0b11010, 0b10101, 0b10101, 0b10001, 0b10001]),
        'n' => Some(&[0b00000, 0b00000, 0b11110, 0b10001, 0b10001, 0b10001, 0b10001]),
        'o' => Some(&[0b00000, 0b00000, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110]),
        'p' => Some(&[0b00000, 0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000]),
        'q' => Some(&[0b00000, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001]),
        'r' => Some(&[0b00000, 0b00000, 0b10110, 0b11001, 0b10000, 0b10000, 0b10000]),
        's' => Some(&[0b00000, 0b00000, 0b01111, 0b10000, 0b01110, 0b00001, 0b11110]),
        't' => Some(&[0b01000, 0b01000, 0b11110, 0b01000, 0b01000, 0b01001, 0b00110]),
        'u' => Some(&[0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b10011, 0b01101]),
        'v' => Some(&[0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100]),
        'w' => Some(&[0b00000, 0b00000, 0b10001, 0b10101, 0b10101, 0b11011, 0b01010]),
        'x' => Some(&[0b00000, 0b00000, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001]),
        'y' => Some(&[0b00000, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110]),
        'z' => Some(&[0b00000, 0b00000, 0b11111, 0b00010, 0b00100, 0b01000, 0b11111]),
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
        '&' => Some(&[0b01100, 0b10010, 0b01100, 0b01010, 0b10001, 0b10010, 0b01101]),
        '*' => Some(&[0b00000, 0b10101, 0b01110, 0b11111, 0b01110, 0b10101, 0b00000]),
        '%' => Some(&[0b11001, 0b11010, 0b00100, 0b01000, 0b01011, 0b10011, 0b00000]),
        '[' => Some(&[0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110]),
        ']' => Some(&[0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110]),
        '<' => Some(&[0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010]),
        '>' => Some(&[0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000]),
        '•' | '·' => Some(&[0b00000, 0b00000, 0b00100, 0b01110, 0b00100, 0b00000, 0b00000]),
        '✓' => Some(&[0b00000, 0b00001, 0b00010, 0b10100, 0b01000, 0b00000, 0b00000]),
        'Á' | 'À' | 'Â' | 'Ä' | 'Ã' | 'Å' | 'Ą' => Some(&[0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'ą' => Some(&[0b00100, 0b00000, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111]),
        'É' | 'È' | 'Ê' | 'Ë' | 'Ę' => Some(&[0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111]),
        'é' | 'è' | 'ê' | 'ë' | 'ę' => Some(&[0b00100, 0b00000, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110]),
        'Í' | 'Ì' | 'Î' | 'Ï' | 'İ' => Some(&[0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
        'í' | 'ì' | 'î' | 'ï' | 'ı' => Some(&[0b00100, 0b00000, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110]),
        'Ó' | 'Ò' | 'Ô' | 'Ö' | 'Õ' => Some(&[0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' => Some(&[0b00100, 0b00000, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110]),
        'Ú' | 'Ù' | 'Û' | 'Ü' => Some(&[0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
        'ú' | 'ù' | 'û' | 'ü' => Some(&[0b00100, 0b00000, 0b10001, 0b10001, 0b10001, 0b10011, 0b01101]),
        'Ñ' | 'Ń' => Some(&[0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001]),
        'ñ' | 'ń' => Some(&[0b01010, 0b00000, 0b11110, 0b10001, 0b10001, 0b10001, 0b10001]),
        'Ç' | 'Ć' => Some(&[0b01110, 0b10001, 0b10000, 0b10000, 0b10001, 0b01110, 0b00100]),
        'ç' | 'ć' => Some(&[0b00000, 0b01110, 0b10000, 0b10000, 0b01110, 0b00100, 0b00000]),
        'Ş' | 'Ś' => Some(&[0b01110, 0b10001, 0b10000, 0b01110, 0b00001, 0b10001, 0b01110]),
        'ş' | 'ś' => Some(&[0b00000, 0b01111, 0b10000, 0b01110, 0b00001, 0b11110, 0b00100]),
        'Ğ' => Some(&[0b01010, 0b00000, 0b01110, 0b10000, 0b10111, 0b10001, 0b01110]),
        'ğ' => Some(&[0b01010, 0b00000, 0b01111, 0b10001, 0b01111, 0b00001, 0b01110]),
        'Ł' => Some(&[0b10000, 0b10100, 0b11110, 0b10000, 0b10000, 0b10000, 0b11111]),
        'ł' => Some(&[0b01100, 0b00100, 0b01110, 0b00100, 0b00100, 0b00100, 0b01110]),
        'Ź' | 'Ż' => Some(&[0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111]),
        'ź' | 'ż' => Some(&[0b00100, 0b00000, 0b11111, 0b00010, 0b00100, 0b01000, 0b11111]),
        'ß' => Some(&[0b01100, 0b10010, 0b11100, 0b10010, 0b10010, 0b11100, 0b10000]),
        '¡' => Some(&[0b00100, 0b00000, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100]),
        '¿' => Some(&[0b00100, 0b00000, 0b00100, 0b01100, 0b10000, 0b10001, 0b01110]),
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
