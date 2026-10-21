//! Pomodoro timer with red keyboard lighting.
//!
//! Focus shows red that dims as time runs out and breathes during the last
//! minute; a pause breathes slowly. Breaks hand the keyboard back to the
//! other lighting layers. Phase changes flash red and post a notification.
use crate::smart_lighting::effect;
use ak820_protocol::commands::lighting::{LightingConfig, Mode};
use serde::{Deserialize, Serialize};

pub const CONFIG_FILE: &str = "pomodoro.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PomodoroConfig {
    pub focus_minutes: u32,
    pub short_break_minutes: u32,
    pub long_break_minutes: u32,
    pub rounds_before_long_break: u32,
    pub auto_start_breaks: bool,
    pub auto_start_focus: bool,
    pub color: String,
    pub notifications: bool,
    /// Uploads an anime card at each phase change (rewrites the stored GIF).
    pub anime_screen: bool,
}

impl Default for PomodoroConfig {
    fn default() -> Self {
        Self {
            focus_minutes: 25,
            short_break_minutes: 5,
            long_break_minutes: 15,
            rounds_before_long_break: 4,
            auto_start_breaks: true,
            auto_start_focus: false,
            color: "FF0000".into(),
            notifications: true,
            anime_screen: false,
        }
    }
}

impl PomodoroConfig {
    pub fn validate(&self) -> Result<(), String> {
        let minutes = 1..=180;
        if !minutes.contains(&self.focus_minutes)
            || !minutes.contains(&self.short_break_minutes)
            || !minutes.contains(&self.long_break_minutes)
            || !(1..=12).contains(&self.rounds_before_long_break)
            || ak820_protocol::commands::lighting::parse_hex_rgb(&self.color).is_none()
        {
            return Err("Pomodoro durations must be 1–180 minutes, rounds 1–12, colour hex".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Idle,
    Focus,
    ShortBreak,
    LongBreak,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Snapshot {
    pub phase: Phase,
    pub paused: bool,
    pub remaining_secs: u32,
    pub total_secs: u32,
    /// Completed focus sessions in the current cycle.
    pub round: u32,
    pub completed_today: u32,
}

/// What a tick or command asks the host to do.
#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    pub finished: Phase,
    pub next: Phase,
}

#[derive(Debug, Clone, Default)]
pub struct Timer {
    pub phase: Phase,
    pub paused: bool,
    pub remaining: u32,
    pub total: u32,
    pub round: u32,
    pub completed_today: u32,
    /// Set when a finished phase waits for the user to start the next one.
    pub waiting: Option<Phase>,
}

impl Timer {
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            phase: self.phase,
            paused: self.paused,
            remaining_secs: self.remaining,
            total_secs: self.total,
            round: self.round,
            completed_today: self.completed_today,
        }
    }

    fn begin(&mut self, phase: Phase, config: &PomodoroConfig) {
        let minutes = match phase {
            Phase::Focus => config.focus_minutes,
            Phase::ShortBreak => config.short_break_minutes,
            Phase::LongBreak => config.long_break_minutes,
            Phase::Idle => 0,
        };
        self.phase = phase;
        self.paused = false;
        self.total = minutes * 60;
        self.remaining = self.total;
        self.waiting = None;
    }

    pub fn start(&mut self, config: &PomodoroConfig) {
        let phase = self.waiting.unwrap_or(Phase::Focus);
        self.begin(phase, config);
    }
    pub fn pause(&mut self) {
        if self.phase != Phase::Idle {
            self.paused = true;
        }
    }
    pub fn resume(&mut self) {
        self.paused = false;
    }
    pub fn stop(&mut self) {
        let completed = self.completed_today;
        *self = Timer {
            completed_today: completed,
            ..Timer::default()
        };
    }
    /// Ends the current phase early, as if its time ran out.
    pub fn skip(&mut self, config: &PomodoroConfig) -> Option<Transition> {
        if self.phase == Phase::Idle {
            return None;
        }
        self.remaining = 0;
        self.finish(config)
    }

    /// Advances one second.
    pub fn tick(&mut self, config: &PomodoroConfig) -> Option<Transition> {
        if self.phase == Phase::Idle || self.paused {
            return None;
        }
        self.remaining = self.remaining.saturating_sub(1);
        if self.remaining > 0 {
            return None;
        }
        self.finish(config)
    }

    fn finish(&mut self, config: &PomodoroConfig) -> Option<Transition> {
        let finished = self.phase;
        let next = match finished {
            Phase::Focus => {
                self.round += 1;
                self.completed_today += 1;
                if self.round >= config.rounds_before_long_break {
                    self.round = 0;
                    Phase::LongBreak
                } else {
                    Phase::ShortBreak
                }
            }
            Phase::ShortBreak | Phase::LongBreak => Phase::Focus,
            Phase::Idle => return None,
        };
        let auto = if next == Phase::Focus {
            config.auto_start_focus
        } else {
            config.auto_start_breaks
        };
        if auto {
            self.begin(next, config);
        } else {
            self.phase = Phase::Idle;
            self.paused = false;
            self.remaining = 0;
            self.total = 0;
            self.waiting = Some(next);
        }
        Some(Transition { finished, next })
    }

    /// Red during focus, dimming from 5 to 1; breathing in the last minute
    /// or while paused. Breaks and idle leave the keyboard to other layers.
    pub fn lighting(&self, config: &PomodoroConfig) -> Option<LightingConfig> {
        if self.phase != Phase::Focus {
            return None;
        }
        if self.paused {
            return Some(effect(Mode::Breath, &config.color, 2, 1));
        }
        if self.remaining <= 60 {
            return Some(effect(Mode::Breath, &config.color, 5, 4));
        }
        let fraction = self.remaining as f32 / self.total.max(1) as f32;
        let brightness = (1.0 + (fraction * 4.0).ceil()).clamp(1.0, 5.0) as u8;
        Some(effect(Mode::Static, &config.color, brightness, 0))
    }

    /// "12:34" for the menu bar, or empty when idle.
    pub fn label(&self) -> String {
        if self.phase == Phase::Idle {
            return String::new();
        }
        let text = format!("{:02}:{:02}", self.remaining / 60, self.remaining % 60);
        if self.paused {
            format!("⏸ {text}")
        } else {
            text
        }
    }
}

pub fn phase_name(phase: Phase, russian: bool) -> &'static str {
    match (phase, russian) {
        (Phase::Focus, false) => "Focus",
        (Phase::Focus, true) => "Фокус",
        (Phase::ShortBreak, false) => "Short break",
        (Phase::ShortBreak, true) => "Короткий перерыв",
        (Phase::LongBreak, false) => "Long break",
        (Phase::LongBreak, true) => "Длинный перерыв",
        (Phase::Idle, false) => "Idle",
        (Phase::Idle, true) => "Остановлен",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quick() -> PomodoroConfig {
        PomodoroConfig {
            focus_minutes: 2,
            short_break_minutes: 1,
            long_break_minutes: 3,
            rounds_before_long_break: 2,
            ..PomodoroConfig::default()
        }
    }

    #[test]
    fn focus_then_breaks_then_long_break() {
        let config = quick();
        let mut timer = Timer::default();
        timer.start(&config);
        assert_eq!(timer.phase, Phase::Focus);
        assert_eq!(timer.remaining, 120);
        let mut transitions = Vec::new();
        for _ in 0..120 {
            if let Some(t) = timer.tick(&config) {
                transitions.push(t);
            }
        }
        assert_eq!(
            transitions,
            vec![Transition {
                finished: Phase::Focus,
                next: Phase::ShortBreak
            }]
        );
        assert_eq!(timer.phase, Phase::ShortBreak);
        // Focus does not auto-start by default.
        for _ in 0..60 {
            timer.tick(&config);
        }
        assert_eq!(
            (timer.phase, timer.waiting),
            (Phase::Idle, Some(Phase::Focus))
        );
        timer.start(&config);
        assert_eq!(timer.skip(&config).unwrap().next, Phase::LongBreak);
        assert_eq!(timer.round, 0);
        assert_eq!(timer.completed_today, 2);
    }

    #[test]
    fn pause_freezes_and_lighting_is_red_only_in_focus() {
        let config = quick();
        let mut timer = Timer::default();
        assert!(timer.lighting(&config).is_none());
        timer.start(&config);
        let full = timer.lighting(&config).unwrap();
        assert_eq!((full.color.as_str(), full.brightness), ("FF0000", 5));
        timer.pause();
        assert_eq!(timer.tick(&config), None);
        assert_eq!(timer.remaining, 120);
        assert_eq!(timer.lighting(&config).unwrap().mode, Mode::Breath);
        assert!(timer.label().starts_with('⏸'));
        timer.resume();
        for _ in 0..61 {
            timer.tick(&config);
        }
        let last_minute = timer.lighting(&config).unwrap();
        assert_eq!(last_minute.mode, Mode::Breath);
        timer.skip(&config);
        assert!(
            timer.lighting(&config).is_none(),
            "breaks leave the LEDs alone"
        );
        timer.stop();
        assert_eq!(timer.label(), "");
        assert_eq!(timer.completed_today, 1);
    }

    #[test]
    fn brightness_steps_down_over_the_session() {
        let config = PomodoroConfig::default();
        let mut timer = Timer::default();
        timer.start(&config);
        let mut levels = Vec::new();
        while timer.remaining > 61 {
            levels.push(timer.lighting(&config).unwrap().brightness);
            timer.tick(&config);
        }
        assert_eq!(levels.first(), Some(&5));
        assert!(levels.windows(2).all(|w| w[0] >= w[1]));
        assert!(*levels.last().unwrap() >= 1);
        assert!(PomodoroConfig {
            focus_minutes: 0,
            ..config
        }
        .validate()
        .is_err());
    }
}
