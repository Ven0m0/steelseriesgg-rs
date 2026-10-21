//! Pure remapping state machine. No I/O and no clock: callers pass `now`, so every timing rule
//! (the escape chord) is unit-testable.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::binding::{Action, BindingSet};
use super::keys::InputKey;
use crate::error::{Error, Result};

/// Physical keys that, held together for `RemapperConfig::escape_hold`, stop the engine.
pub const ESCAPE_CHORD: [InputKey; 4] = [
    InputKey::LEFT_CTRL,
    InputKey::RIGHT_CTRL,
    InputKey::LEFT_SHIFT,
    InputKey::RIGHT_SHIFT,
];

/// Key event value, as in the kernel's `EV_KEY`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyValue {
    Release,
    Press,
    Repeat,
}

impl KeyValue {
    /// Translate a raw evdev value; unknown values give `None`.
    pub fn from_evdev(value: i32) -> Option<Self> {
        match value {
            0 => Some(KeyValue::Release),
            1 => Some(KeyValue::Press),
            2 => Some(KeyValue::Repeat),
            _ => None,
        }
    }

    pub fn to_evdev(self) -> i32 {
        match self {
            KeyValue::Release => 0,
            KeyValue::Press => 1,
            KeyValue::Repeat => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyEvent {
    pub key: InputKey,
    pub value: KeyValue,
}

/// A key event the engine must emit on the virtual device.
pub type Effect = KeyEvent;

#[derive(Clone, Copy, Debug)]
pub struct RemapperConfig {
    /// How long the escape chord must be held.
    pub escape_hold: Duration,
}

impl Default for RemapperConfig {
    fn default() -> Self {
        RemapperConfig {
            escape_hold: Duration::from_secs(2),
        }
    }
}

#[derive(Debug)]
pub struct Remapper {
    bindings: BindingSet,
    /// Physical key to the keys its press emitted, in press order. Release and repeat use this,
    /// never the bindings, so a binding change mid-press cannot leave a key stuck.
    pressed: HashMap<InputKey, Vec<InputKey>>,
    /// Output keys currently down on the virtual device with a holder count, in press order.
    down: Vec<(InputKey, u32)>,
    chord_held: [bool; ESCAPE_CHORD.len()],
    chord_deadline: Option<Instant>,
    escape_hold: Duration,
    stopped: bool,
}

impl Remapper {
    pub fn new(bindings: &BindingSet, config: RemapperConfig) -> Result<Self> {
        if bindings.is_empty() {
            return Err(Error::InvalidConfig("no key bindings configured".to_string()));
        }
        if config.escape_hold.is_zero() {
            return Err(Error::InvalidConfig("escape hold time must be above zero".to_string()));
        }
        Ok(Remapper {
            bindings: bindings.clone(),
            pressed: HashMap::new(),
            down: Vec::new(),
            chord_held: [false; ESCAPE_CHORD.len()],
            chord_deadline: None,
            escape_hold: config.escape_hold,
            stopped: false,
        })
    }

    /// Swap the bindings. Keys already held keep releasing what they pressed.
    pub fn replace_bindings(&mut self, bindings: &BindingSet) {
        self.bindings = bindings.clone();
    }

    /// False once the escape chord has fired; the engine must then stop.
    pub fn is_active(&self) -> bool {
        !self.stopped
    }

    /// When `tick` next has work to do.
    pub fn next_deadline(&self) -> Option<Instant> {
        if self.stopped { None } else { self.chord_deadline }
    }

    /// Fire the escape chord if its hold time has elapsed.
    pub fn tick(&mut self, now: Instant) -> Vec<Effect> {
        match self.chord_deadline {
            Some(deadline) if !self.stopped && now >= deadline => {
                self.stopped = true;
                self.chord_deadline = None;
                self.release_all()
            }
            _ => Vec::new(),
        }
    }

    pub fn handle(&mut self, event: KeyEvent, now: Instant) -> Vec<Effect> {
        let mut out = self.tick(now);
        if self.stopped {
            return out;
        }
        self.track_chord(event, now);
        match event.value {
            KeyValue::Press if !self.pressed.contains_key(&event.key) => self.press(event.key, &mut out),
            KeyValue::Press | KeyValue::Repeat => self.repeat(event.key, &mut out),
            KeyValue::Release => self.release(event.key, &mut out),
        }
        out
    }

    /// Release every output key still down, most recent first, and forget all held keys.
    pub fn release_all(&mut self) -> Vec<Effect> {
        let out = self
            .down
            .iter()
            .rev()
            .map(|(key, _)| KeyEvent {
                key: *key,
                value: KeyValue::Release,
            })
            .collect();
        self.down.clear();
        self.pressed.clear();
        out
    }

    fn track_chord(&mut self, event: KeyEvent, now: Instant) {
        let Some(i) = ESCAPE_CHORD.iter().position(|k| *k == event.key) else {
            return;
        };
        match event.value {
            KeyValue::Press => self.chord_held[i] = true,
            KeyValue::Release => self.chord_held[i] = false,
            KeyValue::Repeat => {}
        }
        if !self.chord_held.iter().all(|held| *held) {
            self.chord_deadline = None;
        } else if self.chord_deadline.is_none() {
            self.chord_deadline = Some(now + self.escape_hold);
        }
    }

    fn press(&mut self, key: InputKey, out: &mut Vec<Effect>) {
        let outputs = self.bindings.get(key).map(Action::outputs).unwrap_or_else(|| vec![key]);
        for output in &outputs {
            match self.down.iter_mut().find(|(k, _)| k == output) {
                Some((_, count)) => *count += 1,
                None => {
                    self.down.push((*output, 1));
                    out.push(KeyEvent {
                        key: *output,
                        value: KeyValue::Press,
                    });
                }
            }
        }
        self.pressed.insert(key, outputs);
    }

    /// Repeats follow the press: the last key of a chord repeats, like holding the last key typed.
    fn repeat(&self, key: InputKey, out: &mut Vec<Effect>) {
        if let Some(last) = self.pressed.get(&key).and_then(|outputs| outputs.last()) {
            out.push(KeyEvent {
                key: *last,
                value: KeyValue::Repeat,
            });
        }
    }

    fn release(&mut self, key: InputKey, out: &mut Vec<Effect>) {
        // A release for a key the virtual device never pressed (held before the grab) is dropped.
        let Some(outputs) = self.pressed.remove(&key) else {
            return;
        };
        for output in outputs.iter().rev() {
            let Some(i) = self.down.iter().position(|(k, _)| k == output) else {
                continue;
            };
            self.down[i].1 -= 1;
            if self.down[i].1 == 0 {
                self.down.remove(i);
                out.push(KeyEvent {
                    key: *output,
                    value: KeyValue::Release,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(name: &str) -> InputKey {
        InputKey::parse(name).unwrap()
    }

    fn ev(name: &str, value: KeyValue) -> KeyEvent {
        KeyEvent { key: k(name), value }
    }

    fn fx(name: &str, value: KeyValue) -> Effect {
        ev(name, value)
    }

    fn remapper(pairs: Vec<(&str, Action)>) -> Remapper {
        let set = BindingSet::from_pairs(pairs.into_iter().map(|(n, a)| (k(n), a))).unwrap();
        Remapper::new(&set, RemapperConfig::default()).unwrap()
    }

    fn caps_to_esc() -> Remapper {
        remapper(vec![("caps_lock", Action::Key(k("esc")))])
    }

    #[test]
    fn plain_remap_press_repeat_release() {
        let mut r = caps_to_esc();
        let t = Instant::now();
        assert_eq!(
            r.handle(ev("caps_lock", KeyValue::Press), t),
            vec![fx("esc", KeyValue::Press)]
        );
        assert_eq!(
            r.handle(ev("caps_lock", KeyValue::Repeat), t),
            vec![fx("esc", KeyValue::Repeat)]
        );
        assert_eq!(
            r.handle(ev("caps_lock", KeyValue::Release), t),
            vec![fx("esc", KeyValue::Release)]
        );
    }

    #[test]
    fn unbound_keys_pass_through() {
        let mut r = caps_to_esc();
        let t = Instant::now();
        assert_eq!(r.handle(ev("a", KeyValue::Press), t), vec![fx("a", KeyValue::Press)]);
        assert_eq!(r.handle(ev("a", KeyValue::Repeat), t), vec![fx("a", KeyValue::Repeat)]);
        assert_eq!(
            r.handle(ev("a", KeyValue::Release), t),
            vec![fx("a", KeyValue::Release)]
        );
    }

    #[test]
    fn combo_presses_in_order_and_releases_in_reverse() {
        let mut r = remapper(vec![(
            "f13",
            Action::Combo(vec![k("left_ctrl"), k("left_shift"), k("t")]),
        )]);
        let t = Instant::now();
        assert_eq!(
            r.handle(ev("f13", KeyValue::Press), t),
            vec![
                fx("left_ctrl", KeyValue::Press),
                fx("left_shift", KeyValue::Press),
                fx("t", KeyValue::Press)
            ]
        );
        assert_eq!(
            r.handle(ev("f13", KeyValue::Repeat), t),
            vec![fx("t", KeyValue::Repeat)]
        );
        assert_eq!(
            r.handle(ev("f13", KeyValue::Release), t),
            vec![
                fx("t", KeyValue::Release),
                fx("left_shift", KeyValue::Release),
                fx("left_ctrl", KeyValue::Release)
            ]
        );
    }

    #[test]
    fn no_stuck_key_when_bindings_change_mid_press() {
        let mut r = caps_to_esc();
        let t = Instant::now();
        r.handle(ev("caps_lock", KeyValue::Press), t);
        let other = BindingSet::from_pairs([(k("caps_lock"), Action::Key(k("tab")))]).unwrap();
        r.replace_bindings(&other);
        assert_eq!(
            r.handle(ev("caps_lock", KeyValue::Release), t),
            vec![fx("esc", KeyValue::Release)]
        );
        assert_eq!(
            r.handle(ev("caps_lock", KeyValue::Press), t),
            vec![fx("tab", KeyValue::Press)]
        );
        assert_eq!(
            r.handle(ev("caps_lock", KeyValue::Release), t),
            vec![fx("tab", KeyValue::Release)]
        );
    }

    #[test]
    fn two_sources_for_one_output_release_only_when_both_are_up() {
        let mut r = remapper(vec![("a", Action::Key(k("x"))), ("b", Action::Key(k("x")))]);
        let t = Instant::now();
        assert_eq!(r.handle(ev("a", KeyValue::Press), t), vec![fx("x", KeyValue::Press)]);
        assert!(r.handle(ev("b", KeyValue::Press), t).is_empty());
        assert!(r.handle(ev("a", KeyValue::Release), t).is_empty());
        assert_eq!(
            r.handle(ev("b", KeyValue::Release), t),
            vec![fx("x", KeyValue::Release)]
        );
    }

    #[test]
    fn stray_release_and_orphan_repeat_are_dropped() {
        let mut r = caps_to_esc();
        let t = Instant::now();
        assert!(r.handle(ev("a", KeyValue::Release), t).is_empty());
        assert!(r.handle(ev("a", KeyValue::Repeat), t).is_empty());
    }

    #[test]
    fn release_all_releases_everything_newest_first() {
        let mut r = remapper(vec![("f13", Action::Combo(vec![k("left_ctrl"), k("c")]))]);
        let t = Instant::now();
        r.handle(ev("f13", KeyValue::Press), t);
        r.handle(ev("a", KeyValue::Press), t);
        assert_eq!(
            r.release_all(),
            vec![
                fx("a", KeyValue::Release),
                fx("c", KeyValue::Release),
                fx("left_ctrl", KeyValue::Release)
            ]
        );
        assert!(r.release_all().is_empty());
        // The physical releases that follow must not emit anything.
        assert!(r.handle(ev("f13", KeyValue::Release), t).is_empty());
        assert!(r.handle(ev("a", KeyValue::Release), t).is_empty());
    }

    fn press_chord(r: &mut Remapper, t: Instant) {
        for key in ["left_ctrl", "right_ctrl", "left_shift", "right_shift"] {
            r.handle(ev(key, KeyValue::Press), t);
        }
    }

    #[test]
    fn escape_chord_fires_at_two_seconds_not_before() {
        let mut r = caps_to_esc();
        let t = Instant::now();
        press_chord(&mut r, t);
        assert_eq!(r.next_deadline(), Some(t + Duration::from_secs(2)));
        assert!(r.tick(t + Duration::from_millis(1900)).is_empty());
        assert!(r.is_active());
        let released = r.tick(t + Duration::from_secs(2));
        assert_eq!(released.len(), 4);
        assert!(released.iter().all(|e| e.value == KeyValue::Release));
        assert!(!r.is_active());
        assert_eq!(r.next_deadline(), None);
        assert!(
            r.handle(ev("a", KeyValue::Press), t + Duration::from_secs(3))
                .is_empty()
        );
    }

    #[test]
    fn escape_chord_fires_from_handle_when_deadline_passed() {
        let mut r = caps_to_esc();
        let t = Instant::now();
        press_chord(&mut r, t);
        let out = r.handle(ev("a", KeyValue::Press), t + Duration::from_millis(2500));
        assert_eq!(out.len(), 4);
        assert!(!r.is_active());
    }

    #[test]
    fn releasing_a_chord_key_cancels_the_chord() {
        let mut r = caps_to_esc();
        let t = Instant::now();
        press_chord(&mut r, t);
        r.handle(ev("right_shift", KeyValue::Release), t + Duration::from_millis(1000));
        assert_eq!(r.next_deadline(), None);
        assert!(r.tick(t + Duration::from_secs(5)).is_empty());
        assert!(r.is_active());
        // Pressing it again restarts the full hold.
        let t2 = t + Duration::from_secs(6);
        r.handle(ev("right_shift", KeyValue::Press), t2);
        assert_eq!(r.next_deadline(), Some(t2 + Duration::from_secs(2)));
    }

    #[test]
    fn chord_keys_are_detected_physically_even_when_remapped() {
        let mut r = remapper(vec![("left_ctrl", Action::Key(k("a")))]);
        let t = Instant::now();
        press_chord(&mut r, t);
        assert!(r.next_deadline().is_some());
    }

    #[test]
    fn new_rejects_empty_bindings_and_zero_hold() {
        assert!(Remapper::new(&BindingSet::default(), RemapperConfig::default()).is_err());
        let set = BindingSet::from_pairs([(k("a"), Action::Key(k("b")))]).unwrap();
        let zero = RemapperConfig {
            escape_hold: Duration::ZERO,
        };
        assert!(Remapper::new(&set, zero).is_err());
    }

    #[test]
    fn key_value_round_trips_evdev_numbers() {
        for v in [KeyValue::Release, KeyValue::Press, KeyValue::Repeat] {
            assert_eq!(KeyValue::from_evdev(v.to_evdev()), Some(v));
        }
        assert_eq!(KeyValue::from_evdev(3), None);
    }
}
