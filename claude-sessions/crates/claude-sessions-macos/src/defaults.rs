//! `NSUserDefaults` behind the core `KeyValueStore`, with the property-list types the Swift widget wrote: dictionaries of
//! arrays of doubles, doubles, bools, strings, string dictionaries and `NSData`.

use claude_sessions_core::kv::KeyValueStore;
use objc2::encode::Encoding;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AnyThread, Message};
use objc2_foundation::{NSArray, NSData, NSDictionary, NSNumber, NSString, NSUserDefaults};
use serde_json::{Map, Number, Value};

/// The standard defaults of the app, or a named suite (tests use a throwaway one).
pub struct UserDefaultsStore {
    defaults: Retained<NSUserDefaults>,
}

impl UserDefaultsStore {
    /// `None` is `UserDefaults.standard`; `Some(name)` is `UserDefaults(suiteName:)`, which keeps another domain.
    pub fn new(suite: Option<&str>) -> Option<Self> {
        let defaults = match suite {
            None => NSUserDefaults::standardUserDefaults(),
            Some(name) => NSUserDefaults::initWithSuiteName(
                NSUserDefaults::alloc(),
                Some(&NSString::from_str(name)),
            )?,
        };
        Some(Self { defaults })
    }

    /// Removes a whole suite domain (test clean-up).
    pub fn remove_suite(name: &str) {
        NSUserDefaults::standardUserDefaults()
            .removePersistentDomainForName(&NSString::from_str(name));
    }

    fn object(&self, key: &str) -> Option<Retained<AnyObject>> {
        self.defaults.objectForKey(&NSString::from_str(key))
    }

    fn set_object(&self, key: &str, value: &AnyObject) {
        // SAFETY: `value` is a property-list object (`NSData`, `NSString`, `NSNumber`, `NSArray`, `NSDictionary`).
        unsafe {
            self.defaults
                .setObject_forKey(Some(value), &NSString::from_str(key));
        }
    }
}

/// The system alert sound (`NSSound.beep()`).
pub fn beep() {
    objc2_app_kit::NSBeep();
}

fn is_bool(n: &NSNumber) -> bool {
    n.encoding() == Encoding::Char
}

fn to_value(object: &AnyObject) -> Option<Value> {
    if let Some(s) = object.downcast_ref::<NSString>() {
        return Some(Value::String(s.to_string()));
    }
    if let Some(n) = object.downcast_ref::<NSNumber>() {
        return Some(if is_bool(n) {
            Value::Bool(n.as_bool())
        } else {
            match n.encoding() {
                Encoding::Float | Encoding::Double => Value::Number(Number::from_f64(n.as_f64())?),
                Encoding::UChar
                | Encoding::UShort
                | Encoding::UInt
                | Encoding::ULong
                | Encoding::ULongLong => Value::Number(n.as_u64().into()),
                _ => Value::Number(n.as_i64().into()),
            }
        });
    }
    if let Some(a) = object.downcast_ref::<NSArray>() {
        return a
            .iter()
            .map(|o| to_value(&o))
            .collect::<Option<Vec<_>>>()
            .map(Value::Array);
    }
    if let Some(d) = object.downcast_ref::<NSDictionary>() {
        return dict_to_map(d).map(Value::Object);
    }
    None
}

fn dict_to_map(d: &NSDictionary) -> Option<Map<String, Value>> {
    let (keys, values) = d.to_vecs();
    keys.iter()
        .zip(values.iter())
        .map(|(k, v)| Some((k.downcast_ref::<NSString>()?.to_string(), to_value(v)?)))
        .collect()
}

fn any<T: Message>(object: Retained<T>) -> Retained<AnyObject> {
    // SAFETY: every Objective-C object is an `AnyObject`.
    unsafe { Retained::cast_unchecked(object) }
}

fn to_object(value: &Value) -> Option<Retained<AnyObject>> {
    Some(match value {
        Value::String(s) => any(NSString::from_str(s)),
        Value::Bool(b) => any(NSNumber::new_bool(*b)),
        Value::Number(n) => {
            let number = if let Some(i) = n.as_i64().filter(|_| !n.is_f64()) {
                NSNumber::new_i64(i)
            } else {
                NSNumber::new_f64(n.as_f64()?)
            };
            any(number)
        }
        Value::Array(items) => {
            let objects = items.iter().map(to_object).collect::<Option<Vec<_>>>()?;
            any(NSArray::from_retained_slice(&objects))
        }
        Value::Object(map) => any(map_to_dict(map)?),
        Value::Null => return None,
    })
}

fn map_to_dict(map: &Map<String, Value>) -> Option<Retained<NSDictionary<NSString, AnyObject>>> {
    let keys: Vec<Retained<NSString>> = map.keys().map(|k| NSString::from_str(k)).collect();
    let key_refs: Vec<&NSString> = keys.iter().map(|k| &**k).collect();
    let objects = map.values().map(to_object).collect::<Option<Vec<_>>>()?;
    Some(NSDictionary::from_retained_objects(&key_refs, &objects))
}

impl KeyValueStore for UserDefaultsStore {
    fn get_bytes(&self, key: &str) -> Option<Vec<u8>> {
        Some(self.object(key)?.downcast_ref::<NSData>()?.to_vec())
    }

    fn set_bytes(&self, key: &str, value: &[u8]) {
        self.set_object(key, &NSData::with_bytes(value));
    }

    fn get_string(&self, key: &str) -> Option<String> {
        Some(self.object(key)?.downcast_ref::<NSString>()?.to_string())
    }

    fn set_string(&self, key: &str, value: &str) {
        self.set_object(key, &NSString::from_str(value));
    }

    fn get_f64(&self, key: &str) -> Option<f64> {
        let object = self.object(key)?;
        let n = object.downcast_ref::<NSNumber>()?;
        (!is_bool(n)).then(|| n.as_f64())
    }

    fn set_f64(&self, key: &str, value: f64) {
        self.set_object(key, &NSNumber::new_f64(value));
    }

    fn get_bool(&self, key: &str) -> Option<bool> {
        Some(self.object(key)?.downcast_ref::<NSNumber>()?.as_bool())
    }

    fn set_bool(&self, key: &str, value: bool) {
        self.set_object(key, &NSNumber::new_bool(value));
    }

    fn get_dict(&self, key: &str) -> Option<Map<String, Value>> {
        let object = self.object(key)?;
        dict_to_map(object.downcast_ref::<NSDictionary>()?)
    }

    fn set_dict(&self, key: &str, value: &Map<String, Value>) {
        if let Some(dict) = map_to_dict(value) {
            self.set_object(key, &dict);
        }
    }

    fn remove(&self, key: &str) {
        self.defaults.removeObjectForKey(&NSString::from_str(key));
    }
}
