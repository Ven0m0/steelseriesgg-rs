//! Key-name table for the remapper.
//!
//! Codes are the Linux `input-event-codes.h` values, so the table needs no evdev dependency and
//! the config parser and tests run on every platform.

use std::fmt;

use crate::error::{Error, Result};

/// A Linux input key code (`KEY_*`).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InputKey(u16);

impl InputKey {
    pub const LEFT_CTRL: InputKey = InputKey(29);
    pub const LEFT_SHIFT: InputKey = InputKey(42);
    pub const RIGHT_SHIFT: InputKey = InputKey(54);
    pub const RIGHT_CTRL: InputKey = InputKey(97);

    /// Wrap a raw kernel key code.
    pub const fn from_code(code: u16) -> Self {
        InputKey(code)
    }

    /// The raw kernel key code.
    pub const fn code(self) -> u16 {
        self.0
    }

    /// Parse a key name, case-insensitively; `-` and `_` are interchangeable.
    pub fn parse(name: &str) -> Result<Self> {
        let norm = name.trim().to_ascii_lowercase().replace('-', "_");
        KEY_TABLE
            .iter()
            .find(|(n, _)| *n == norm)
            .map(|(_, code)| InputKey(*code))
            .ok_or_else(|| Error::InvalidConfig(format!("unknown key name '{name}'")))
    }

    /// Canonical name (first table entry for the code), if the key is in the table.
    pub fn name(self) -> Option<&'static str> {
        KEY_TABLE.iter().find(|(_, code)| *code == self.0).map(|(n, _)| *n)
    }
}

impl fmt::Display for InputKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => f.write_str(name),
            None => write!(f, "key_{}", self.0),
        }
    }
}

/// Name to code. The first entry for a code is its canonical name; later entries are aliases.
const KEY_TABLE: &[(&str, u16)] = &[
    ("esc", 1),
    ("escape", 1),
    ("1", 2),
    ("2", 3),
    ("3", 4),
    ("4", 5),
    ("5", 6),
    ("6", 7),
    ("7", 8),
    ("8", 9),
    ("9", 10),
    ("0", 11),
    ("minus", 12),
    ("equal", 13),
    ("backspace", 14),
    ("tab", 15),
    ("q", 16),
    ("w", 17),
    ("e", 18),
    ("r", 19),
    ("t", 20),
    ("y", 21),
    ("u", 22),
    ("i", 23),
    ("o", 24),
    ("p", 25),
    ("left_brace", 26),
    ("right_brace", 27),
    ("enter", 28),
    ("return", 28),
    ("left_ctrl", 29),
    ("a", 30),
    ("s", 31),
    ("d", 32),
    ("f", 33),
    ("g", 34),
    ("h", 35),
    ("j", 36),
    ("k", 37),
    ("l", 38),
    ("semicolon", 39),
    ("apostrophe", 40),
    ("grave", 41),
    ("left_shift", 42),
    ("backslash", 43),
    ("z", 44),
    ("x", 45),
    ("c", 46),
    ("v", 47),
    ("b", 48),
    ("n", 49),
    ("m", 50),
    ("comma", 51),
    ("dot", 52),
    ("slash", 53),
    ("right_shift", 54),
    ("kp_asterisk", 55),
    ("left_alt", 56),
    ("space", 57),
    ("caps_lock", 58),
    ("capslock", 58),
    ("f1", 59),
    ("f2", 60),
    ("f3", 61),
    ("f4", 62),
    ("f5", 63),
    ("f6", 64),
    ("f7", 65),
    ("f8", 66),
    ("f9", 67),
    ("f10", 68),
    ("num_lock", 69),
    ("scroll_lock", 70),
    ("kp7", 71),
    ("kp8", 72),
    ("kp9", 73),
    ("kp_minus", 74),
    ("kp4", 75),
    ("kp5", 76),
    ("kp6", 77),
    ("kp_plus", 78),
    ("kp1", 79),
    ("kp2", 80),
    ("kp3", 81),
    ("kp0", 82),
    ("kp_dot", 83),
    ("102nd", 86),
    ("f11", 87),
    ("f12", 88),
    ("kp_enter", 96),
    ("right_ctrl", 97),
    ("kp_slash", 98),
    ("print_screen", 99),
    ("sysrq", 99),
    ("right_alt", 100),
    ("home", 102),
    ("up", 103),
    ("page_up", 104),
    ("left", 105),
    ("right", 106),
    ("end", 107),
    ("down", 108),
    ("page_down", 109),
    ("insert", 110),
    ("delete", 111),
    ("mute", 113),
    ("volume_down", 114),
    ("volume_up", 115),
    ("kp_equal", 117),
    ("pause", 119),
    ("left_meta", 125),
    ("left_super", 125),
    ("right_meta", 126),
    ("right_super", 126),
    ("menu", 127),
    ("next_song", 163),
    ("play_pause", 164),
    ("previous_song", 165),
    ("stop_cd", 166),
    ("f13", 183),
    ("f14", 184),
    ("f15", 185),
    ("f16", 186),
    ("f17", 187),
    ("f18", 188),
    ("f19", 189),
    ("f20", 190),
    ("f21", 191),
    ("f22", 192),
    ("f23", 193),
    ("f24", 194),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_names() {
        assert_eq!(InputKey::parse("a").unwrap().code(), 30);
        assert_eq!(InputKey::parse("caps_lock").unwrap().code(), 58);
        assert_eq!(InputKey::parse("esc").unwrap().code(), 1);
        assert_eq!(InputKey::parse("0").unwrap().code(), 11);
        assert_eq!(InputKey::parse("f24").unwrap().code(), 194);
        assert_eq!(InputKey::parse("left_ctrl").unwrap(), InputKey::LEFT_CTRL);
        assert_eq!(InputKey::parse("right_alt").unwrap().code(), 100);
    }

    #[test]
    fn parsing_is_case_insensitive_and_dash_tolerant() {
        assert_eq!(
            InputKey::parse("Caps_Lock").unwrap(),
            InputKey::parse("caps_lock").unwrap()
        );
        assert_eq!(InputKey::parse(" LEFT-CTRL ").unwrap(), InputKey::LEFT_CTRL);
    }

    #[test]
    fn aliases_resolve_to_the_same_key() {
        assert_eq!(InputKey::parse("escape").unwrap(), InputKey::parse("esc").unwrap());
        assert_eq!(InputKey::parse("capslock").unwrap().name(), Some("caps_lock"));
    }

    #[test]
    fn unknown_name_is_an_error_naming_the_offender() {
        let err = InputKey::parse("capslokc").unwrap_err().to_string();
        assert!(err.contains("capslokc"), "{err}");
        assert!(InputKey::parse("").is_err());
        assert!(InputKey::parse("f25").is_err());
    }

    #[test]
    fn every_letter_digit_and_f_key_is_present() {
        for c in 'a'..='z' {
            assert!(InputKey::parse(&c.to_string()).is_ok(), "{c}");
        }
        for c in '0'..='9' {
            assert!(InputKey::parse(&c.to_string()).is_ok(), "{c}");
        }
        for n in 1..=24 {
            assert!(InputKey::parse(&format!("f{n}")).is_ok(), "f{n}");
        }
    }

    #[test]
    fn table_has_no_duplicate_names() {
        for (i, (name, _)) in KEY_TABLE.iter().enumerate() {
            assert!(KEY_TABLE[i + 1..].iter().all(|(n, _)| n != name), "duplicate {name}");
        }
    }

    #[test]
    fn display_uses_canonical_name_or_code() {
        assert_eq!(InputKey::parse("escape").unwrap().to_string(), "esc");
        assert_eq!(InputKey::from_code(500).to_string(), "key_500");
    }
}
