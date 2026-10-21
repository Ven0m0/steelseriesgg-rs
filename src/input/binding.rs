//! Validated key bindings.
//!
//! Deliberately only two actions: remap one key to another, or emit a chord. Macros, text typing,
//! command launching and profile switching are not supported on purpose (smaller attack surface).

use std::collections::HashMap;

use super::keys::InputKey;
use crate::error::{Error, Result};

/// Longest chord a single key may emit.
pub const MAX_COMBO_LEN: usize = 4;

/// What a bound key does.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    /// Act as another key.
    Key(InputKey),
    /// Press the keys in order on press, release them in reverse order on release.
    Combo(Vec<InputKey>),
}

impl Action {
    /// The keys this action presses, in press order.
    pub fn outputs(&self) -> Vec<InputKey> {
        match self {
            Action::Key(key) => vec![*key],
            Action::Combo(keys) => keys.clone(),
        }
    }
}

/// Source key to action, validated on construction.
#[derive(Clone, Debug, Default)]
pub struct BindingSet {
    map: HashMap<InputKey, Action>,
}

impl BindingSet {
    /// Build from `(source, action)` pairs, rejecting duplicate sources (e.g. two aliases of one key),
    /// self-mappings, empty or over-long chords and chords that repeat a key.
    pub fn from_pairs(pairs: impl IntoIterator<Item = (InputKey, Action)>) -> Result<Self> {
        let mut map = HashMap::new();
        for (source, action) in pairs {
            validate(source, &action)?;
            if map.insert(source, action).is_some() {
                return Err(Error::InvalidConfig(format!(
                    "key '{source}' is bound more than once (aliases of the same key?)"
                )));
            }
        }
        Ok(BindingSet { map })
    }

    /// The action bound to `key`, if any.
    pub fn get(&self, key: InputKey) -> Option<&Action> {
        self.map.get(&key)
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Bindings sorted by source key code, for stable display.
    pub fn sorted(&self) -> Vec<(InputKey, &Action)> {
        let mut all: Vec<_> = self.map.iter().map(|(k, a)| (*k, a)).collect();
        all.sort_by_key(|(k, _)| *k);
        all
    }

    /// Every key some binding can emit (the virtual device must be able to produce these).
    pub fn output_keys(&self) -> Vec<InputKey> {
        let mut keys: Vec<InputKey> = self.map.values().flat_map(Action::outputs).collect();
        keys.sort();
        keys.dedup();
        keys
    }
}

fn validate(source: InputKey, action: &Action) -> Result<()> {
    match action {
        Action::Key(target) if *target == source => {
            Err(Error::InvalidConfig(format!("key '{source}' is bound to itself")))
        }
        Action::Key(_) => Ok(()),
        Action::Combo(keys) => {
            if keys.is_empty() {
                return Err(Error::InvalidConfig(format!("key '{source}' has an empty combo")));
            }
            if keys.len() > MAX_COMBO_LEN {
                return Err(Error::InvalidConfig(format!(
                    "combo for '{source}' has {} keys; the limit is {MAX_COMBO_LEN}",
                    keys.len()
                )));
            }
            if keys.as_slice() == [source] {
                return Err(Error::InvalidConfig(format!("key '{source}' is bound to itself")));
            }
            for (i, key) in keys.iter().enumerate() {
                if keys[..i].contains(key) {
                    return Err(Error::InvalidConfig(format!(
                        "combo for '{source}' repeats key '{key}'"
                    )));
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(name: &str) -> InputKey {
        InputKey::parse(name).unwrap()
    }

    #[test]
    fn accepts_key_and_combo() {
        let set = BindingSet::from_pairs([
            (k("caps_lock"), Action::Key(k("esc"))),
            (k("f13"), Action::Combo(vec![k("left_ctrl"), k("c")])),
        ])
        .unwrap();
        assert_eq!(set.len(), 2);
        assert_eq!(set.get(k("caps_lock")), Some(&Action::Key(k("esc"))));
        assert_eq!(set.output_keys().len(), 3);
    }

    #[test]
    fn rejects_self_binding() {
        assert!(BindingSet::from_pairs([(k("a"), Action::Key(k("a")))]).is_err());
        assert!(BindingSet::from_pairs([(k("a"), Action::Combo(vec![k("a")]))]).is_err());
    }

    #[test]
    fn allows_combo_that_contains_source_among_others() {
        assert!(BindingSet::from_pairs([(k("a"), Action::Combo(vec![k("left_ctrl"), k("a")]))]).is_ok());
    }

    #[test]
    fn rejects_empty_long_and_repeating_combos() {
        assert!(BindingSet::from_pairs([(k("a"), Action::Combo(vec![]))]).is_err());
        let long = vec![k("b"), k("c"), k("d"), k("e"), k("f")];
        assert!(BindingSet::from_pairs([(k("a"), Action::Combo(long))]).is_err());
        assert!(BindingSet::from_pairs([(k("a"), Action::Combo(vec![k("b"), k("b")]))]).is_err());
    }

    #[test]
    fn rejects_duplicate_sources() {
        let err = BindingSet::from_pairs([(k("esc"), Action::Key(k("a"))), (k("escape"), Action::Key(k("b")))])
            .unwrap_err()
            .to_string();
        assert!(err.contains("more than once"), "{err}");
    }
}
