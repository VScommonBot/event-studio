use serde::{Deserialize, Serialize};

pub const DEFAULT_SECONDS: f64 = 300.0;
pub const DEFAULT_WARNING: f64 = 60.0;
pub const MAX_SECONDS: f64 = 359999.0;
pub const OVERTIME_DELAY: f64 = 1.0;
/// The quick choices for the length of a presentation, in minutes (the buttons under the time).
pub const PRESET_MINUTES: [u32; 5] = [3, 5, 10, 15, 20];
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Phase {
    Ready,
    Running,
    Warning,
    End,
    Over,
}
#[derive(Debug, Clone)]
pub struct Clock {
    pub duration: f64,
    pub accumulated: f64,
    pub started: Option<f64>,
    pub qna: bool,
}
impl Clock {
    pub fn new(duration: f64) -> Self {
        Self { duration, accumulated: 0.0, started: None, qna: false }
    }
    pub fn elapsed(&self, now: f64) -> f64 {
        self.accumulated + self.started.map_or(0.0, |t| (now - t).max(0.0))
    }
    pub fn running(&self) -> bool {
        self.started.is_some()
    }
    pub fn toggle(&mut self, now: f64) {
        if self.running() {
            self.accumulated = self.elapsed(now);
            self.started = None;
        } else {
            self.started = Some(now);
        }
    }
    pub fn reset(&mut self, duration: f64) {
        *self = Self::new(duration);
    }
    pub fn adjust(&mut self, delta: f64, now: f64) {
        self.duration = (self.duration + delta).max(self.elapsed(now).min(self.duration)).clamp(0.0, MAX_SECONDS);
    }
    pub fn switch_qna(&mut self, seconds: f64, carry: bool, now: f64) {
        // A presentation that is already over is not switched by hand; whole seconds are carried.
        let remaining = self.duration - self.elapsed(now);
        if self.qna || remaining <= 0.0 {
            return;
        }
        let extra = if carry { remaining.ceil() } else { 0.0 };
        // A running timer keeps its place within the current second on a manual switch.
        let tick_fraction = if self.running() { self.elapsed(now).fract() } else { 0.0 };
        self.duration = (seconds + extra).clamp(0.0, MAX_SECONDS);
        self.accumulated = tick_fraction;
        self.started = Some(now);
        self.qna = true;
    }
    pub fn auto_qna(&mut self, seconds: f64, now: f64) {
        if !self.qna && self.running() && self.elapsed(now) >= self.duration + OVERTIME_DELAY {
            let overflow = self.elapsed(now) - self.duration - OVERTIME_DELAY;
            // The automatic switch is separate from the manual one and its guard.
            self.duration = seconds.clamp(0.0, MAX_SECONDS);
            self.started = Some(now);
            self.qna = true;
            self.accumulated = overflow;
        }
    }
    /// Seconds until the shown time next changes: the elapsed time passes a whole second (counting
    /// up) or what is left does (counting down, and past the end). Warning, end, overtime and the
    /// automatic Q&A switch all fall on those moments. `None` while the timer stands still.
    pub fn next_change(&self, now: f64) -> Option<f64> {
        if !self.running() {
            return None;
        }
        let e = self.elapsed(now);
        let up = 1.0 - e.fract();
        let down = (self.duration - e).rem_euclid(1.0);
        Some(up.min(if down > 0.0 { down } else { 1.0 }))
    }
    pub fn view(&self, now: f64, warn: f64, count_up: bool, overtime: bool, seconds_only: bool) -> View {
        let e = self.elapsed(now);
        let rem = self.duration - e;
        let phase = if rem <= -OVERTIME_DELAY && overtime {
            Phase::Over
        } else if rem <= 0.0 {
            Phase::End
        } else if warn > 0.0 && rem <= warn && (self.running() || e > 0.0) {
            Phase::Warning
        } else if self.running() || e > 0.0 {
            Phase::Running
        } else {
            Phase::Ready
        };
        let (n, plus) = if count_up {
            ((if overtime { e } else { e.min(self.duration) }).floor(), false)
        } else if rem > 0.0 {
            (rem.ceil(), false)
        } else if phase == Phase::Over {
            ((-rem).floor(), true)
        } else {
            (0.0, false)
        };
        let n = n.max(0.0) as u64;
        let mut text = if seconds_only { n.to_string() } else { format_seconds(n) };
        if plus {
            text.insert(0, '+');
        }
        let remaining = if self.duration > 0.0 { (rem / self.duration).clamp(0.0, 1.0) as f32 } else { 0.0 };
        // The graph shows what is left when counting down and what has passed when counting up
        // (as in VS Event Timer 1.1.0; a zero duration counts as all passed).
        let fill = if count_up {
            if self.duration > 0.0 {
                (e / self.duration).clamp(0.0, 1.0) as f32
            } else {
                1.0
            }
        } else {
            remaining
        };
        View { text, phase, remaining, fill, elapsed: e }
    }
}
pub struct View {
    pub text: String,
    pub phase: Phase,
    pub remaining: f32,
    pub fill: f32,
    pub elapsed: f64,
}
/// The preset length (in seconds) one step up (`step > 0`) or down from `current`: the nearest one
/// beyond it, or the end of the list when there is none; a length between two presets goes to the
/// one on that side.
pub fn step_preset(current: f64, step: i32) -> f64 {
    let secs = PRESET_MINUTES.map(|m| m as f64 * 60.0);
    let found = if step > 0 { secs.iter().find(|s| **s > current) } else { secs.iter().rev().find(|s| **s < current) };
    *found.unwrap_or(if step > 0 { &secs[secs.len() - 1] } else { &secs[0] })
}
pub fn format_seconds(n: u64) -> String {
    if n >= 3600 {
        format!("{}:{:02}:{:02}", n / 3600, n / 60 % 60, n % 60)
    } else {
        format!("{:02}:{:02}", n / 60, n % 60)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presets_step_up_and_down_and_stop_at_the_ends() {
        assert_eq!(step_preset(300.0, 1), 600.0);
        assert_eq!(step_preset(300.0, -1), 180.0);
        assert_eq!(step_preset(180.0, -1), 180.0);
        assert_eq!(step_preset(1200.0, 1), 1200.0);
        // Between two presets: the one on that side.
        assert_eq!(step_preset(400.0, 1), 600.0);
        assert_eq!(step_preset(400.0, -1), 300.0);
        assert_eq!(step_preset(0.0, -1), 180.0);
        assert_eq!(step_preset(9999.0, 1), 1200.0);
    }
    #[test]
    fn starts_at_five_minutes() {
        assert_eq!(Clock::new(DEFAULT_SECONDS).view(0.0, 60.0, false, true, false).text, "05:00");
    }
    #[test]
    fn pause_resume_has_no_drift() {
        let mut c = Clock::new(300.0);
        c.toggle(10.0);
        c.toggle(11.25);
        assert_eq!(c.elapsed(999.0), 1.25);
        c.toggle(1000.0);
        assert_eq!(c.elapsed(1002.5), 3.75);
    }
    #[test]
    fn warning_and_overtime_boundaries() {
        let mut c = Clock::new(300.0);
        c.toggle(0.0);
        assert_eq!(c.view(240.0, 60.0, false, true, false).phase, Phase::Warning);
        assert_eq!(c.view(300.0, 60.0, false, true, false).text, "00:00");
        assert_eq!(c.view(300.99, 60.0, false, true, false).phase, Phase::End);
        assert_eq!(c.view(301.0, 60.0, false, true, false).text, "+00:01");
    }
    #[test]
    fn countdown_ceil_countup_floor() {
        let mut c = Clock::new(300.0);
        c.toggle(0.0);
        assert_eq!(c.view(0.2, 60.0, false, true, false).text, "05:00");
        assert_eq!(c.view(0.2, 60.0, true, true, false).text, "00:00");
    }
    #[test]
    fn disabled_overtime_clamps() {
        let mut c = Clock::new(1.0);
        c.toggle(0.0);
        assert_eq!(c.view(5.0, 0.0, false, false, false).text, "00:00");
        assert_eq!(c.view(5.0, 0.0, true, false, false).text, "00:01");
    }
    #[test]
    fn qna_carry_and_repeat_guard() {
        let mut c = Clock::new(300.0);
        c.toggle(0.0);
        c.switch_qna(300.0, true, 120.0);
        assert_eq!(c.duration, 480.0);
        c.switch_qna(300.0, true, 130.0);
        assert_eq!(c.duration, 480.0);
    }
    #[test]
    fn qna_auto_retains_zero_interval() {
        let mut c = Clock::new(300.0);
        c.toggle(0.0);
        c.auto_qna(300.0, 300.5);
        assert!(!c.qna);
        c.auto_qna(300.0, 303.0);
        assert!(c.qna);
        assert_eq!(c.elapsed(303.0), 2.0);
    }
    #[test]
    fn adjustment_never_goes_below_elapsed() {
        let mut c = Clock::new(300.0);
        c.toggle(0.0);
        c.adjust(-500.0, 42.0);
        assert_eq!(c.duration, 42.0);
    }
    #[test]
    fn zero_duration_safe() {
        assert_eq!(Clock::new(0.0).view(0.0, 0.0, false, true, false).remaining, 0.0);
    }
    #[test]
    fn hour_and_seconds_display() {
        assert_eq!(format_seconds(3661), "1:01:01");
        assert_eq!(Clock::new(300.0).view(0.0, 60.0, false, true, true).text, "300");
    }
    #[test]
    fn qna_manual_rejects_zero_and_expired_pt() {
        for now in [300.0, 300.5, 301.0] {
            let mut c = Clock::new(300.0);
            c.toggle(0.0);
            c.switch_qna(300.0, true, now);
            assert!(!c.qna);
        }
    }
    #[test]
    fn qna_carries_integer_seconds_and_keeps_running_tick() {
        let mut c = Clock::new(300.0);
        c.toggle(0.0);
        c.switch_qna(300.0, true, 120.25);
        assert_eq!(c.duration, 480.0);
        assert_eq!(c.elapsed(120.25), 0.25);
        assert_eq!(c.view(121.0, 60.0, false, true, false).text, "07:59");
    }
    #[test]
    fn qna_paused_switch_starts_a_new_tick() {
        let mut c = Clock::new(300.0);
        c.toggle(0.0);
        c.toggle(120.25);
        c.switch_qna(300.0, false, 900.0);
        assert!(c.running());
        assert_eq!(c.elapsed(900.0), 0.0);
        assert_eq!(c.duration, 300.0);
    }
    #[test]
    fn qna_auto_exact_boundary_and_zero_qna() {
        let mut c = Clock::new(300.0);
        c.toggle(0.0);
        c.auto_qna(0.0, 301.0);
        assert!(c.qna);
        assert_eq!(c.view(301.0, 60.0, false, true, false).text, "00:00");
        assert_eq!(c.view(302.0, 60.0, false, true, false).text, "+00:01");
    }
    #[test]
    fn next_change_is_the_next_whole_second() {
        let mut c = Clock::new(300.0);
        assert_eq!(c.next_change(5.0), None);
        c.toggle(10.0);
        assert_eq!(c.next_change(10.0), Some(1.0));
        assert_eq!(c.next_change(10.25), Some(0.75));
        assert_eq!(c.next_change(310.5), Some(0.5), "past the end the overtime counts on");
        c.toggle(311.0);
        assert_eq!(c.next_change(400.0), None);
        // A length that is not whole seconds (after a carried Q&A tick): what is left reaches a whole second first.
        let mut c = Clock::new(10.5);
        c.toggle(0.0);
        assert_eq!(c.next_change(0.25), Some(0.25));
        assert_eq!(c.next_change(0.75), Some(0.25));
    }
    #[test]
    fn reset_clears_qna() {
        let mut c = Clock::new(3.0);
        c.switch_qna(4.0, false, 0.0);
        c.reset(300.0);
        assert!(!c.qna);
        assert!(!c.running());
        assert_eq!(c.duration, 300.0);
    }
    // As in VS Event Timer 1.1.0: the bar and ring show what is left counting down, what has passed counting up.
    #[test]
    fn graph_drains_counting_down_and_fills_counting_up() {
        let mut c = Clock::new(300.0);
        assert_eq!(c.view(0.0, 60.0, false, true, false).fill, 1.0);
        assert_eq!(c.view(0.0, 60.0, true, true, false).fill, 0.0);
        c.toggle(0.0);
        assert_eq!(c.view(75.0, 60.0, false, true, false).fill, 0.75);
        assert_eq!(c.view(75.0, 60.0, true, true, false).fill, 0.25);
        assert_eq!(c.view(400.0, 60.0, false, true, false).fill, 0.0);
        assert_eq!(c.view(400.0, 60.0, true, true, false).fill, 1.0);
        assert_eq!(c.view(75.0, 60.0, true, true, false).remaining, 0.75);
        assert_eq!(Clock::new(0.0).view(0.0, 0.0, false, true, false).fill, 0.0);
        assert_eq!(Clock::new(0.0).view(0.0, 0.0, true, true, false).fill, 1.0);
    }
}
