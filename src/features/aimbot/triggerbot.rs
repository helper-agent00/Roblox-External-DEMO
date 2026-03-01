use std::time::Instant;

use crate::config::Config;
use crate::utils::input::Input;

/// Triggerbot auto-fire system.
pub struct Triggerbot {
    target_acquired_time: Option<Instant>,
    last_target_name: String,
    last_fire_time: Instant,
    /// Randomized delay jitter PRNG state
    rng_state: u32,
    /// Current randomized delay
    current_delay_ms: u64,
    /// Consecutive shots on same target
    consecutive_shots: u32,
}

impl Triggerbot {
    pub fn new() -> Self {
        Self {
            target_acquired_time: None,
            last_target_name: String::new(),
            last_fire_time: Instant::now(),
            rng_state: 0xBEEFCAFE,
            current_delay_ms: 0,
            consecutive_shots: 0,
        }
    }

    /// Fast xorshift PRNG
    #[inline]
    fn fast_rand(&mut self) -> f32 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 17;
        self.rng_state ^= self.rng_state << 5;
        (self.rng_state as f32) / (u32::MAX as f32)
    }

    /// Called each frame with aim assist's current locked target.
    pub fn apply(&mut self, config: &Config, locked_target_name: Option<&str>) {
        if !config.triggerbot.enabled {
            self.target_acquired_time = None;
            self.consecutive_shots = 0;
            return;
        }

        let target_name = match locked_target_name {
            Some(name) => name,
            None => {
                self.target_acquired_time = None;
                self.last_target_name.clear();
                self.consecutive_shots = 0;
                return;
            }
        };

        let now = Instant::now();

        // adaptive fire cooldown
        let base_cooldown: u64 = 40;
        let burst_penalty: u64 = if self.consecutive_shots > 3 { 15 } else { 0 };
        let min_fire_cooldown_ms = base_cooldown + burst_penalty;
        if self.last_fire_time.elapsed().as_millis() < min_fire_cooldown_ms as u128 {
            return;
        }

        // track target acquisition time (for confirmation delay)
        if self.target_acquired_time.is_none() || self.last_target_name != target_name {
            self.target_acquired_time = Some(now);
            self.last_target_name = target_name.to_string();
            self.consecutive_shots = 0;
            let jitter = (self.fast_rand() * 0.4 - 0.2) * config.triggerbot.delay_ms;
            self.current_delay_ms = (config.triggerbot.delay_ms + jitter).max(0.0) as u64;
        }

        // Check if we've held target long enough
        let time_on_target = self.target_acquired_time
            .map(|t| now.duration_since(t).as_millis() as u64)
            .unwrap_or(0);

        if time_on_target >= self.current_delay_ms {
            Input::click_mouse();
            self.last_fire_time = now;
            self.consecutive_shots += 1;

            // reset for next shot with fresh randomized delay
            self.target_acquired_time = Some(now);
            let jitter = (self.fast_rand() * 0.3 - 0.15) * config.triggerbot.delay_ms;
            self.current_delay_ms = (config.triggerbot.delay_ms + jitter).max(0.0) as u64;
        }
    }
}
