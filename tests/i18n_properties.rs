//! Property and unit tests for internationalization (i18n) across all 12 supported languages.

use xmahjong::i18n::{self, Language};
use xmahjong::storage::Settings;

#[test]
fn test_all_12_languages_present() {
    let all = Language::all();
    assert_eq!(all.len(), 12, "Must support exactly 12 languages");
    let codes: Vec<&str> = all.iter().map(|l| l.code()).collect();
    assert_eq!(
        codes,
        vec!["en", "es", "fr", "de", "it", "pt", "nl", "pl", "tr", "ta", "ja", "zh"]
    );
}

#[test]
fn test_language_from_code_roundtrip() {
    for &lang in Language::all() {
        let code = lang.code();
        let parsed = Language::from_code(code);
        assert_eq!(parsed, Some(lang), "Failed code roundtrip for {:?}", lang);

        // Case-insensitivity test
        let upper = code.to_uppercase();
        assert_eq!(Language::from_code(&upper), Some(lang));
    }
}

#[test]
fn test_language_cycle_all_12() {
    let start = Language::En;
    let mut current = start;
    for _ in 0..12 {
        current = current.next();
    }
    assert_eq!(current, start, "Cycling 12 times should return to start language");
}

#[test]
fn test_language_serde_roundtrip() {
    for &lang in Language::all() {
        let json = serde_json::to_string(&lang).expect("Failed to serialize Language");
        let deserialized: Language = serde_json::from_str(&json).expect("Failed to deserialize Language");
        assert_eq!(deserialized, lang);
    }
}

#[test]
fn test_settings_language_serde() {
    let settings = Settings {
        muted: true,
        language: Language::Pt,
    };
    let json = serde_json::to_string(&settings).expect("Failed to serialize Settings");
    assert!(json.contains("\"language\":\"pt\""));

    let loaded: Settings = serde_json::from_str(&json).expect("Failed to deserialize Settings");
    assert_eq!(loaded.language, Language::Pt);
    assert!(loaded.muted);
}

#[test]
fn test_locale_detection() {
    assert_eq!(Language::from_code("pt_BR.UTF-8"), Some(Language::Pt));
    assert_eq!(Language::from_code("pt-BR"), Some(Language::Pt));
    assert_eq!(Language::from_code("es_ES"), Some(Language::Es));
    assert_eq!(Language::from_code("fr_FR"), Some(Language::Fr));
    assert_eq!(Language::from_code("de_DE"), Some(Language::De));
    assert_eq!(Language::from_code("it_IT"), Some(Language::It));
    assert_eq!(Language::from_code("nl_NL"), Some(Language::Nl));
    assert_eq!(Language::from_code("pl_PL"), Some(Language::Pl));
    assert_eq!(Language::from_code("tr_TR"), Some(Language::Tr));
    assert_eq!(Language::from_code("ta_IN"), Some(Language::Ta));
    assert_eq!(Language::from_code("ja_JP"), Some(Language::Ja));
    assert_eq!(Language::from_code("zh_CN"), Some(Language::Zh));
    assert_eq!(Language::from_code("en_US"), Some(Language::En));
    assert_eq!(Language::from_code("unknown_LOCALE"), None);
}

#[test]
fn test_brazilian_portuguese_translations() {
    let pt = Language::Pt;
    assert_eq!(pt.name(), "Português");
    assert_eq!(pt.display_label(), "Português");
    assert_eq!(i18n::t(pt, "level"), "Nível");
    assert_eq!(i18n::t(pt, "score"), "Pontos");
    assert_eq!(i18n::t(pt, "lives"), "Vidas");
    assert_eq!(i18n::t(pt, "hints"), "Dicas");
    assert_eq!(i18n::t(pt, "tiles"), "Peças");
    assert_eq!(i18n::t(pt, "time"), "Tempo");
    assert_eq!(i18n::t(pt, "undo"), "Desfazer");
    assert_eq!(i18n::t(pt, "shuffle"), "Embaralhar");
    assert_eq!(i18n::t(pt, "paused"), "PAUSADO");
    assert_eq!(i18n::t(pt, "shortcuts"), "ATALHOS");
    assert_eq!(i18n::t(pt, "achievements"), "CONQUISTAS");
    assert_eq!(i18n::t(pt, "select_level"), "SELECIONAR NÍVEL");
    assert_eq!(i18n::t(pt, "quit_game"), "SAIR DO JOGO?");
    assert_eq!(i18n::t(pt, "yes"), "SIM");
    assert_eq!(i18n::t(pt, "no"), "NÃO");
    assert_eq!(i18n::t(pt, "theme_auto"), "Auto");
}

#[test]
fn test_key_parity_across_all_languages() {
    let keys = [
        "level", "theme", "score", "lives", "hints", "tiles", "time", "buy_lives",
        "undo", "hint", "shuffle", "retry", "new", "theme_auto", "theme_tiles",
        "theme_dogs", "theme_space", "theme_ocean", "phase_penguin", "phase_bear",
        "phase_dog", "phase_space", "phase_endgame", "phase_grandmaster",
        "phase_penguin_short", "phase_bear_short", "phase_dog_short",
        "phase_space_short", "phase_endgame_short", "phase_grandmaster_short",
        "victory_title", "all_levels_complete", "victory_sub", "level_score",
        "total_score", "level_time", "total_time", "first_clear_bonus", "replay_clear",
        "next_level", "replay_level", "new_game_l1", "new_game", "pause_title",
        "pause_sub", "resume_game", "retry_current_level", "view_trophies",
        "out_of_lives", "out_of_lives_sub", "no_more_moves", "no_more_moves_sub",
        "use_life_shuffle", "use_life_shuffle_cnt", "trophies_title", "trophies_sub",
        "career_overview", "highest_level_completed", "out_of_1000", "total_score_curr",
        "accum_points", "daily_consistency", "current_day_streak", "best_streak_record",
        "consecutive_days", "active_today", "played_yesterday", "streak_reset",
        "daily_gift_granted", "mastery_clean", "no_hints", "no_undos", "no_lives_used",
        "zero_hints", "zero_undos", "zero_shuffles", "save_stats_image", "close",
        "day", "days", "none_yet", "save_image", "back_to_stats", "stats_snapshot",
        "stats_snapshot_sub", "select_level", "page_info", "prev_page", "next_page",
        "back_btn", "menu", "esc_menu", "paused", "levels", "shortcuts", "achievements",
        "mode_label", "language_label", "about", "switch_user", "save_quit",
        "esc_resume_ctrl_s_save", "keyboard_shortcuts", "quit_game", "quit_prompt",
        "yes", "no", "select_player", "enter_name", "guest", "delete", "play",
        "update_available", "update_sub", "download", "later", "save_score",
        "wait_for_shuffle", "hint_suggestion", "daily_streak_title", "daily_streak_sub",
        "continue_btn", "press_enter_to_start", "or_start_new_player", "resume_lvl",
        "player_select_nav", "player_select_click", "esc_quit",
    ];

    for &lang in Language::all() {
        for &key in &keys {
            let val = i18n::t(lang, key);
            assert!(
                !val.is_empty(),
                "Translation for key '{}' in language '{:?}' must not be empty",
                key,
                lang
            );
            // It should not fallback to key name
            assert_ne!(
                val, key,
                "Translation for key '{}' in language '{:?}' returned raw key (missing translation)",
                key, lang
            );
        }
    }
}

#[test]
fn test_parameter_substitution_all_languages() {
    for &lang in Language::all() {
        let vic = i18n::t_param(lang, "victory_title", &[("level", "42")]);
        assert!(vic.contains("42"), "victory_title in {:?} should contain '42': '{}'", lang, vic);

        let page = i18n::t_param(lang, "page_info", &[("current", "3"), ("total", "40")]);
        assert!(page.contains('3') && page.contains("40"), "page_info in {:?} should contain '3' and '40': '{}'", lang, page);

        let lang_lbl = i18n::t_param(lang, "language_label", &[("lang", lang.display_label().as_str())]);
        assert!(lang_lbl.contains(lang.display_label().as_str()), "language_label in {:?} should contain display label: '{}'", lang, lang_lbl);
    }
}
