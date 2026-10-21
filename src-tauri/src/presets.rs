//! Presets: one click sets lighting, smart lighting, Pomodoro, the anime
//! screen and the knob together for a situation (work, focus, calls…).
//!
//! Every part is optional and the user picks which parts to apply. Personal
//! settings stay untouched: the weather city, upload consent, the music
//! service, app profiles and key mappings are never changed by a preset.
use crate::anime_screen::AnimeConfig;
use crate::mac_controls::Bindings;
use crate::pomodoro::PomodoroConfig;
use crate::smart_lighting::{effect, solid, AppLighting, SmartConfig};
use ak820_anime::Character;
use ak820_protocol::commands::lighting::{LightingConfig, Mode};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SmartPatch {
    pub teams_enabled: bool,
    pub telegram_enabled: bool,
    pub telegram_color: &'static str,
    pub telegram_flashes: u8,
    pub apps_enabled: bool,
    /// Added to (or replacing) the user's rules for the same apps.
    pub app_rules: Vec<AppLighting>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnimePatch {
    pub character: Character,
    pub auto: bool,
    pub scenes: Vec<&'static str>,
    pub music_minutes: u32,
    pub ambient_minutes: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Preset {
    pub id: &'static str,
    pub icon: &'static str,
    pub name: &'static str,
    pub name_ru: &'static str,
    pub description: &'static str,
    pub description_ru: &'static str,
    pub lighting: Option<LightingConfig>,
    pub smart: Option<SmartPatch>,
    pub pomodoro: Option<PomodoroConfig>,
    pub anime: Option<AnimePatch>,
    pub knob: Option<Bindings>,
}

fn knob(single: &str, double: &str, triple: &str) -> Bindings {
    Bindings {
        single: single.into(),
        double: double.into(),
        triple: triple.into(),
        rotation: "volume".into(),
        shift_rotation: "seek".into(),
    }
}
fn rule(bundle_id: &str, name: &str, lighting: LightingConfig) -> AppLighting {
    AppLighting {
        bundle_id: bundle_id.into(),
        name: name.into(),
        lighting,
    }
}
fn pomodoro(
    focus: u32,
    short: u32,
    long: u32,
    rounds: u32,
    auto_focus: bool,
    card: bool,
) -> PomodoroConfig {
    PomodoroConfig {
        focus_minutes: focus,
        short_break_minutes: short,
        long_break_minutes: long,
        rounds_before_long_break: rounds,
        auto_start_breaks: true,
        auto_start_focus: auto_focus,
        anime_screen: card,
        ..PomodoroConfig::default()
    }
}
fn rainbow() -> LightingConfig {
    let mut config = effect(Mode::Spectrum, "FFFFFF", 4, 3);
    config.color_mode = 1;
    config
}

pub fn library() -> Vec<Preset> {
    vec![
        Preset {
            id: "work",
            icon: "💻",
            name: "Work",
            name_ru: "Работа",
            description: "Cool white lighting, colours for GoLand, DBeaver and Postman, Teams and Telegram alerts, 50/10 Pomodoro on a triple click, weather and music on the screen.",
            description_ru: "Холодный белый свет, свои цвета для GoLand, DBeaver и Postman, сигналы Teams и Telegram, помодоро 50/10 на тройной клик, погода и музыка на экране.",
            lighting: Some(solid("E8F0FF", 3)),
            smart: Some(SmartPatch {
                teams_enabled: true,
                telegram_enabled: true,
                telegram_color: "0088FF",
                telegram_flashes: 2,
                apps_enabled: true,
                app_rules: vec![
                    rule("com.jetbrains.goland", "GoLand", solid("7B61FF", 3)),
                    rule("org.jkiss.dbeaver.core.product", "DBeaver", solid("00C2A8", 3)),
                    rule("com.postmanlabs.mac", "Postman", solid("FF6C37", 3)),
                ],
            }),
            pomodoro: Some(pomodoro(50, 10, 20, 3, false, true)),
            anime: Some(AnimePatch {
                character: Character::Yuki,
                auto: true,
                scenes: vec!["daily", "weather", "now_playing"],
                music_minutes: 2,
                ambient_minutes: 30,
            }),
            knob: Some(knob("toggle", "next", "pomodoro")),
        },
        Preset {
            id: "focus",
            icon: "🎯",
            name: "Deep focus",
            name_ru: "Глубокий фокус",
            description: "Dim warm light, Telegram silenced (Teams calls still show), classic 25/5 Pomodoro that starts the next round by itself, one click starts or pauses it.",
            description_ru: "Тёплый приглушённый свет, Telegram не мигает (звонки Teams видны), классическое помодоро 25/5 с автозапуском раундов, один клик — старт/пауза.",
            lighting: Some(solid("FF9A3C", 2)),
            smart: Some(SmartPatch {
                teams_enabled: true,
                telegram_enabled: false,
                telegram_color: "0088FF",
                telegram_flashes: 1,
                apps_enabled: false,
                app_rules: Vec::new(),
            }),
            pomodoro: Some(pomodoro(25, 5, 15, 4, true, true)),
            anime: Some(AnimePatch {
                character: Character::Kuro,
                auto: true,
                scenes: vec!["daily", "weather"],
                music_minutes: 2,
                ambient_minutes: 60,
            }),
            knob: Some(knob("pomodoro", "toggle", "next")),
        },
        Preset {
            id: "calls",
            icon: "🎧",
            name: "Calls",
            name_ru: "Созвоны",
            description: "Soft light, incoming Teams calls pulse, a knob click mutes the microphone and a double click mutes the sound. Telegram flashes once.",
            description_ru: "Мягкий свет, входящие Teams пульсируют, клик колёсика выключает микрофон, двойной — звук. Telegram мигает один раз.",
            lighting: Some(solid("FFFFFF", 2)),
            smart: Some(SmartPatch {
                teams_enabled: true,
                telegram_enabled: true,
                telegram_color: "0066CC",
                telegram_flashes: 1,
                apps_enabled: false,
                app_rules: Vec::new(),
            }),
            pomodoro: None,
            anime: Some(AnimePatch {
                character: Character::Hikari,
                auto: true,
                scenes: vec!["daily", "weather"],
                music_minutes: 2,
                ambient_minutes: 30,
            }),
            knob: Some(knob("mic_mute", "mute", "toggle")),
        },
        Preset {
            id: "music",
            icon: "🎵",
            name: "Music",
            name_ru: "Музыка",
            description: "Rainbow lighting, the screen follows the current track every 2 minutes, click to play/pause, double/triple click for next/previous, Shift+turn seeks second by second.",
            description_ru: "Радужная подсветка, экран показывает текущий трек и обновляется каждые 2 минуты, клик — пауза, двойной/тройной — следующий/предыдущий, Shift+поворот — перемотка по секунде.",
            lighting: Some(rainbow()),
            smart: Some(SmartPatch {
                teams_enabled: true,
                telegram_enabled: true,
                telegram_color: "0088FF",
                telegram_flashes: 2,
                apps_enabled: false,
                app_rules: Vec::new(),
            }),
            pomodoro: None,
            anime: Some(AnimePatch {
                character: Character::Sakura,
                auto: true,
                scenes: vec!["now_playing", "daily", "weather"],
                music_minutes: 2,
                ambient_minutes: 30,
            }),
            knob: Some(knob("toggle", "next", "previous")),
        },
        Preset {
            id: "gaming",
            icon: "🎮",
            name: "Gaming",
            name_ru: "Игры",
            description: "Bright red ripples, no Telegram flashes in the middle of a fight (Teams calls still pulse), the screen stays put, the knob keeps volume and music.",
            description_ru: "Яркие красные волны, Telegram не мигает посреди боя (звонки Teams пульсируют), экран не меняется, колёсико — громкость и музыка.",
            lighting: Some(effect(Mode::Ripples, "FF0040", 5, 4)),
            smart: Some(SmartPatch {
                teams_enabled: true,
                telegram_enabled: false,
                telegram_color: "0088FF",
                telegram_flashes: 1,
                apps_enabled: false,
                app_rules: Vec::new(),
            }),
            pomodoro: None,
            anime: Some(AnimePatch {
                character: Character::Kuro,
                auto: false,
                scenes: vec!["daily", "weather"],
                music_minutes: 2,
                ambient_minutes: 60,
            }),
            knob: Some(knob("toggle", "next", "previous")),
        },
        Preset {
            id: "night",
            icon: "🌙",
            name: "Night",
            name_ru: "Ночь",
            description: "Barely-there amber light, a single dim Telegram flash, the night greeting card changing every hour.",
            description_ru: "Едва заметный янтарный свет, одна тусклая вспышка Telegram, ночная открытка раз в час.",
            lighting: Some(solid("FF6A00", 1)),
            smart: Some(SmartPatch {
                teams_enabled: true,
                telegram_enabled: true,
                telegram_color: "003A80",
                telegram_flashes: 1,
                apps_enabled: false,
                app_rules: Vec::new(),
            }),
            pomodoro: None,
            anime: Some(AnimePatch {
                character: Character::Kuro,
                auto: true,
                scenes: vec!["daily", "weather", "now_playing"],
                music_minutes: 5,
                ambient_minutes: 60,
            }),
            knob: Some(knob("toggle", "next", "previous")),
        },
    ]
}

pub fn find(id: &str) -> Option<Preset> {
    library().into_iter().find(|p| p.id == id)
}

/// Applies a patch onto the user's smart-lighting settings, keeping colours
/// they chose for Teams and app rules for other apps.
pub fn patch_smart(mut config: SmartConfig, patch: &SmartPatch) -> SmartConfig {
    config.teams_enabled = patch.teams_enabled;
    config.telegram_enabled = patch.telegram_enabled;
    config.telegram_color = patch.telegram_color.into();
    config.telegram_flashes = patch.telegram_flashes;
    config.apps_enabled = patch.apps_enabled;
    for rule in &patch.app_rules {
        config.app_rules.retain(|r| r.bundle_id != rule.bundle_id);
        config.app_rules.push(rule.clone());
    }
    config
}

/// Applies a patch onto the anime settings, keeping city and consent.
pub fn patch_anime(mut config: AnimeConfig, patch: &AnimePatch) -> AnimeConfig {
    config.character = patch.character.clone();
    config.auto = patch.auto && config.consent;
    config.rotation = patch.scenes.iter().map(|s| s.to_string()).collect();
    config.music_minutes = patch.music_minutes;
    config.ambient_minutes = patch.ambient_minutes;
    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_is_valid_and_unique() {
        let presets = library();
        let mut ids = std::collections::BTreeSet::new();
        for p in &presets {
            assert!(ids.insert(p.id), "{}", p.id);
            if let Some(smart) = &p.smart {
                patch_smart(SmartConfig::default(), smart)
                    .validate()
                    .unwrap();
            }
            if let Some(pomodoro) = &p.pomodoro {
                pomodoro.validate().unwrap();
            }
            if let Some(anime) = &p.anime {
                patch_anime(AnimeConfig::default(), anime)
                    .validate()
                    .unwrap();
            }
            if let Some(lighting) = &p.lighting {
                ak820_protocol::legacy::lighting_packet(lighting).unwrap();
            }
        }
        assert!(presets.len() >= 6);
    }

    #[test]
    fn patches_keep_personal_settings() {
        let mut anime = AnimeConfig {
            city: "Almaty".into(),
            consent: false,
            ..AnimeConfig::default()
        };
        let patch = find("music").unwrap().anime.unwrap();
        anime = patch_anime(anime, &patch);
        assert_eq!(anime.city, "Almaty");
        assert!(!anime.auto, "no automatic uploads without consent");
        let mut smart = SmartConfig::default();
        smart
            .app_rules
            .push(rule("com.example", "Example", solid("123456", 2)));
        let smart = patch_smart(smart, &find("work").unwrap().smart.unwrap());
        assert_eq!(smart.app_rules.len(), 4);
        // Only presets with an explicit description mute anything.
        for p in library() {
            if let Some(k) = &p.knob {
                let mutes = [&k.single, &k.double, &k.triple]
                    .iter()
                    .any(|a| a.contains("mute"));
                assert_eq!(mutes, p.id == "calls", "{}", p.id);
            }
        }
    }
}
