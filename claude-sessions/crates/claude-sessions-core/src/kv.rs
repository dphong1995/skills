//! Persistence behind a trait: `claude-sessions-macos` implements it over `NSUserDefaults`, tests use `MemoryStore`.

use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A string-keyed store of the property-list value kinds the widget saves. Setters take `&self`: `NSUserDefaults`
/// is a shared object, and `MemoryStore` locks internally.
pub trait KeyValueStore {
    fn get_bytes(&self, key: &str) -> Option<Vec<u8>>;
    fn set_bytes(&self, key: &str, value: &[u8]);
    fn get_string(&self, key: &str) -> Option<String>;
    fn set_string(&self, key: &str, value: &str);
    fn get_f64(&self, key: &str) -> Option<f64>;
    fn set_f64(&self, key: &str, value: f64);
    fn get_bool(&self, key: &str) -> Option<bool>;
    fn set_bool(&self, key: &str, value: bool);
    /// A dictionary of property-list values (strings, numbers, arrays of numbers).
    fn get_dict(&self, key: &str) -> Option<Map<String, Value>>;
    fn set_dict(&self, key: &str, value: &Map<String, Value>);
    fn remove(&self, key: &str);
}

#[derive(Clone, Debug, PartialEq)]
enum Entry {
    Bytes(Vec<u8>),
    Str(String),
    F64(f64),
    Bool(bool),
    Dict(Map<String, Value>),
}

/// An in-memory store; clones share the same storage.
#[derive(Clone, Default)]
pub struct MemoryStore {
    entries: Arc<Mutex<HashMap<String, Entry>>>,
}

impl MemoryStore {
    fn get(&self, key: &str) -> Option<Entry> {
        self.entries.lock().unwrap().get(key).cloned()
    }

    fn set(&self, key: &str, entry: Entry) {
        self.entries.lock().unwrap().insert(key.to_string(), entry);
    }
}

impl KeyValueStore for MemoryStore {
    fn get_bytes(&self, key: &str) -> Option<Vec<u8>> {
        match self.get(key)? {
            Entry::Bytes(b) => Some(b),
            _ => None,
        }
    }

    fn set_bytes(&self, key: &str, value: &[u8]) {
        self.set(key, Entry::Bytes(value.to_vec()));
    }

    fn get_string(&self, key: &str) -> Option<String> {
        match self.get(key)? {
            Entry::Str(s) => Some(s),
            _ => None,
        }
    }

    fn set_string(&self, key: &str, value: &str) {
        self.set(key, Entry::Str(value.to_string()));
    }

    fn get_f64(&self, key: &str) -> Option<f64> {
        match self.get(key)? {
            Entry::F64(n) => Some(n),
            _ => None,
        }
    }

    fn set_f64(&self, key: &str, value: f64) {
        self.set(key, Entry::F64(value));
    }

    fn get_bool(&self, key: &str) -> Option<bool> {
        match self.get(key)? {
            Entry::Bool(b) => Some(b),
            _ => None,
        }
    }

    fn set_bool(&self, key: &str, value: bool) {
        self.set(key, Entry::Bool(value));
    }

    fn get_dict(&self, key: &str) -> Option<Map<String, Value>> {
        match self.get(key)? {
            Entry::Dict(d) => Some(d),
            _ => None,
        }
    }

    fn set_dict(&self, key: &str, value: &Map<String, Value>) {
        self.set(key, Entry::Dict(value.clone()));
    }

    fn remove(&self, key: &str) {
        self.entries.lock().unwrap().remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_round_trip_by_kind() {
        let kv = MemoryStore::default();
        kv.set_f64("a", 1.5);
        kv.set_bool("b", true);
        kv.set_string("c", "x");
        assert_eq!(kv.get_f64("a"), Some(1.5));
        assert_eq!(kv.get_bool("b"), Some(true));
        assert_eq!(kv.get_string("c").as_deref(), Some("x"));
        assert_eq!(kv.get_string("a"), None);
        kv.remove("a");
        assert_eq!(kv.get_f64("a"), None);
    }
}
