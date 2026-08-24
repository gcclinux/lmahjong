//! Audio manager module.
//!
//! Manages sound effect playback via SDL2_mixer with graceful
//! degradation when audio hardware is unavailable.

use sdl2::mixer::{self, Chunk, InitFlag};

/// Manages game audio: loading sound effects, playback, and mute state.
///
/// Degrades gracefully when SDL2_mixer initialization fails or sound
/// files are unavailable — all `play_*` methods become no-ops.
pub struct AudioManager {
    select_sound: Option<Chunk>,
    deselect_sound: Option<Chunk>,
    match_sound: Option<Chunk>,
    error_sound: Option<Chunk>,
    victory_sound: Option<Chunk>,
    shuffle_sound: Option<Chunk>,
    muted: bool,
    _initialized: bool,
}

impl AudioManager {
    /// Initializes the audio subsystem and loads sound effects.
    ///
    /// Returns a functional `AudioManager` even if audio hardware is
    /// unavailable or sound files are missing. In that case, all playback
    /// methods silently do nothing.
    pub fn new() -> Self {
        // Try to initialize SDL2_mixer. If it fails, degrade gracefully.
        let initialized = mixer::open_audio(44100, mixer::AUDIO_S16LSB, 2, 1024).is_ok();
        if initialized {
            let _ = mixer::init(InitFlag::OGG);
            mixer::allocate_channels(8);
        }

        // Try to load sound effects (will be None if files don't exist or audio not initialized)
        let select_sound = Self::load_sound_by_name("select");
        let deselect_sound = Self::load_sound_by_name("deselect");
        let match_sound = Self::load_sound_by_name("match");
        let error_sound = Self::load_sound_by_name("error");
        let victory_sound = Self::load_sound_by_name("victory");
        let shuffle_sound = Self::load_sound_by_name("shuffle");

        AudioManager {
            select_sound,
            deselect_sound,
            match_sound,
            error_sound,
            victory_sound,
            shuffle_sound,
            muted: false,
            _initialized: initialized,
        }
    }

    /// Attempts to load a sound file given its base name (e.g. "select").
    /// Checks for .wav and .ogg extensions across common asset locations.
    fn load_sound_by_name(name: &str) -> Option<Chunk> {
        let candidates = [
            format!("assets/sounds/{}.wav", name),
            format!("assets/sounds/{}.ogg", name),
            format!("../assets/sounds/{}.wav", name),
            format!("../assets/sounds/{}.ogg", name),
            format!("../../assets/sounds/{}.wav", name),
            format!("../../assets/sounds/{}.ogg", name),
        ];

        for path in &candidates {
            if let Ok(chunk) = Chunk::from_file(path) {
                return Some(chunk);
            }
        }

        // Also check relative to executable if available
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                for ext in &["wav", "ogg"] {
                    let path = dir.join("assets").join("sounds").join(format!("{}.{}", name, ext));
                    if let Ok(chunk) = Chunk::from_file(&path) {
                        return Some(chunk);
                    }
                    let path = dir.join("../../assets").join("sounds").join(format!("{}.{}", name, ext));
                    if let Ok(chunk) = Chunk::from_file(&path) {
                        return Some(chunk);
                    }
                }
            }
        }

        None
    }

    /// Plays the tile select sound effect.
    /// No-op if muted or sound is unavailable.
    pub fn play_select(&self) {
        self.play(&self.select_sound);
    }

    /// Plays the tile deselect sound effect.
    /// No-op if muted or sound is unavailable.
    pub fn play_deselect(&self) {
        self.play(&self.deselect_sound);
    }

    /// Plays the match success sound effect.
    /// No-op if muted or sound is unavailable.
    pub fn play_match(&self) {
        self.play(&self.match_sound);
    }

    /// Plays the error/mismatch sound effect.
    /// No-op if muted or sound is unavailable.
    pub fn play_error(&self) {
        self.play(&self.error_sound);
    }

    /// Plays the victory fanfare sound effect.
    /// No-op if muted or sound is unavailable.
    pub fn play_victory(&self) {
        self.play(&self.victory_sound);
    }

    /// Plays the shuffle sound effect.
    /// No-op if muted or sound is unavailable.
    pub fn play_shuffle(&self) {
        self.play(&self.shuffle_sound);
    }

    /// Internal playback helper. Skips if muted or chunk is None.
    fn play(&self, chunk: &Option<Chunk>) {
        if self.muted {
            return;
        }
        if let Some(ref sound) = chunk {
            let _ = sdl2::mixer::Channel::all().play(sound, 0);
        }
    }

    /// Toggles the mute state. When muted, all playback is suppressed.
    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
    }

    /// Returns `true` if audio is currently muted.
    pub fn is_muted(&self) -> bool {
        self.muted
    }

    /// Sets the mute state explicitly.
    pub fn set_mute(&mut self, muted: bool) {
        self.muted = muted;
    }
}

impl Default for AudioManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create an AudioManager without SDL2 initialization.
    /// This avoids needing audio hardware for unit tests.
    fn make_test_manager() -> AudioManager {
        AudioManager {
            select_sound: None,
            deselect_sound: None,
            match_sound: None,
            error_sound: None,
            victory_sound: None,
            shuffle_sound: None,
            muted: false,
            _initialized: false,
        }
    }

    #[test]
    fn test_initial_mute_state_is_false() {
        let manager = make_test_manager();
        assert!(!manager.is_muted());
    }

    #[test]
    fn test_toggle_mute_enables_mute() {
        let mut manager = make_test_manager();
        manager.toggle_mute();
        assert!(manager.is_muted());
    }

    #[test]
    fn test_toggle_mute_twice_disables_mute() {
        let mut manager = make_test_manager();
        manager.toggle_mute();
        manager.toggle_mute();
        assert!(!manager.is_muted());
    }

    #[test]
    fn test_set_mute_true() {
        let mut manager = make_test_manager();
        manager.set_mute(true);
        assert!(manager.is_muted());
    }

    #[test]
    fn test_set_mute_false() {
        let mut manager = make_test_manager();
        manager.set_mute(true);
        manager.set_mute(false);
        assert!(!manager.is_muted());
    }

    #[test]
    fn test_set_mute_idempotent() {
        let mut manager = make_test_manager();
        manager.set_mute(true);
        manager.set_mute(true);
        assert!(manager.is_muted());
        manager.set_mute(false);
        manager.set_mute(false);
        assert!(!manager.is_muted());
    }

    #[test]
    fn test_play_methods_do_not_panic_when_no_sound() {
        let manager = make_test_manager();
        // These should all be no-ops without panicking
        manager.play_select();
        manager.play_deselect();
        manager.play_match();
        manager.play_error();
        manager.play_victory();
        manager.play_shuffle();
    }

    #[test]
    fn test_play_methods_do_not_panic_when_muted() {
        let mut manager = make_test_manager();
        manager.set_mute(true);
        // These should all be no-ops without panicking
        manager.play_select();
        manager.play_deselect();
        manager.play_match();
        manager.play_error();
        manager.play_victory();
        manager.play_shuffle();
    }
}
