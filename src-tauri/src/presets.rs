//! Presets: one click sets the lighting and the knob gestures together for
//! a situation (work, focus, calls…).
//!
//! Every part is optional and the user picks which parts to apply. Personal
//! settings stay untouched: the music service, app profiles and key
//! mappings are never changed by a preset.
use crate::base_lighting::{effect, solid};
use crate::mac_controls::Bindings;
use ak820_protocol::commands::lighting::{LightingConfig, Mode};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Preset {
    pub id: &'static str,
    pub icon: &'static str,
    pub name: &'static str,
    pub name_ru: &'static str,
    pub description: &'static str,
    pub description_ru: &'static str,
    pub lighting: Option<LightingConfig>,
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
            description: "Cool white lighting; the knob plays or pauses, skips and goes back.",
            description_ru:
                "Холодный белый свет; колёсико ставит на паузу, листает вперёд и назад.",
            lighting: Some(solid("E8F0FF", 3)),
            knob: Some(knob("toggle", "next", "previous")),
        },
        Preset {
            id: "focus",
            icon: "🎯",
            name: "Deep focus",
            name_ru: "Глубокий фокус",
            description:
                "Dim warm light; a click pauses the music, a double click mutes the sound.",
            description_ru:
                "Приглушённый тёплый свет; клик ставит музыку на паузу, двойной выключает звук.",
            lighting: Some(solid("FF9A3C", 2)),
            knob: Some(knob("toggle", "mute", "next")),
        },
        Preset {
            id: "calls",
            icon: "🎧",
            name: "Calls",
            name_ru: "Созвоны",
            description:
                "Soft white light; a click mutes the microphone, a double click the sound.",
            description_ru: "Мягкий белый свет; клик выключает микрофон, двойной — звук.",
            lighting: Some(solid("FFFFFF", 2)),
            knob: Some(knob("mic_mute", "mute", "toggle")),
        },
        Preset {
            id: "music",
            icon: "🎵",
            name: "Music",
            name_ru: "Музыка",
            description: "Rainbow lighting; the knob controls the player.",
            description_ru: "Радужная подсветка; колёсико управляет плеером.",
            lighting: Some(rainbow()),
            knob: Some(knob("toggle", "next", "previous")),
        },
        Preset {
            id: "gaming",
            icon: "🎮",
            name: "Gaming",
            name_ru: "Игры",
            description: "Bright red ripples; the knob controls the player.",
            description_ru: "Яркие красные волны; колёсико управляет плеером.",
            lighting: Some(effect(Mode::Ripples, "FF0040", 5, 4)),
            knob: Some(knob("toggle", "next", "previous")),
        },
        Preset {
            id: "night",
            icon: "🌙",
            name: "Night",
            name_ru: "Ночь",
            description: "Very dim amber light; the knob controls the player.",
            description_ru: "Очень тусклый янтарный свет; колёсико управляет плеером.",
            lighting: Some(solid("FF6A00", 1)),
            knob: Some(knob("toggle", "next", "previous")),
        },
    ]
}

pub fn find(id: &str) -> Option<Preset> {
    library().into_iter().find(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_unique_and_use_known_knob_actions() {
        let all = library();
        let ids: std::collections::BTreeSet<_> = all.iter().map(|p| p.id).collect();
        assert_eq!(ids.len(), all.len());
        let known = [
            "toggle", "next", "previous", "open", "mute", "mic_mute", "none",
        ];
        for p in &all {
            let k = p.knob.as_ref().unwrap();
            for action in [&k.single, &k.double, &k.triple] {
                assert!(known.contains(&action.as_str()), "{} uses {action}", p.id);
            }
        }
        assert!(find("work").is_some() && find("nope").is_none());
    }
}
