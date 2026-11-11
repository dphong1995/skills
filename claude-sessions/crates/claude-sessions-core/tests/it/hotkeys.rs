use claude_sessions_core::hotkeys::{
    Cycle, HotKeyChoice, HotKeyCombo, advance, next_choices, open_choices,
};
use claude_sessions_core::kv::{KeyValueStore, MemoryStore};
use std::collections::HashSet;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn t0() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_800_000_000)
}

fn ids(s: &[&str]) -> Vec<String> {
    s.iter().map(|s| s.to_string()).collect()
}

fn live(set: &[&str]) -> impl Fn(&str) -> bool {
    let set: HashSet<String> = set.iter().map(|s| s.to_string()).collect();
    move |id| set.contains(id)
}

#[test]
fn choices_defaults() {
    assert_eq!(open_choices()[0].label, "⌃⌥Space");
    assert_eq!(next_choices()[0].label, "⌃⌥→");
    let c = next_choices()[1].clone();
    let back = HotKeyCombo::from_defaults(&c.to_defaults()).unwrap();
    assert_eq!(back, c);
}

#[test]
fn cycle_index() {
    let t = t0();
    let order = ids(&["a", "b", "c"]);
    let is_live = live(&["a", "b", "c"]);
    let cycle = |id: &str| Cycle {
        order: order.clone(),
        id: id.to_string(),
        at: t,
    };
    let first = |c: Option<&Cycle>, fresh: &[String], dt: u64| {
        advance(c, fresh, &is_live, t + Duration::from_secs(dt)).map(|r| r.1)
    };
    assert_eq!(first(None, &order, 0).as_deref(), Some("a"));
    assert_eq!(first(Some(&cycle("a")), &order, 4).as_deref(), Some("b"));
    assert_eq!(first(Some(&cycle("c")), &order, 4).as_deref(), Some("a"));
    assert_eq!(first(Some(&cycle("a")), &order, 6).as_deref(), Some("a"));
    assert_eq!(first(Some(&cycle("gone")), &order, 1).as_deref(), Some("a"));
    assert_eq!(first(Some(&cycle("a")), &[], 0).as_deref(), Some("b"));
    assert_eq!(advance(None, &[], |_| false, t), None);
}

#[test]
fn cycle_with_every_id_dead_falls_back_to_fresh() {
    let t = t0();
    let c = Cycle {
        order: ids(&["x", "y"]),
        id: "x".into(),
        at: t,
    };
    let pick = advance(
        Some(&c),
        &ids(&["n1", "n2"]),
        live(&["n1", "n2"]),
        t + Duration::from_secs(2),
    )
    .unwrap();
    assert_eq!(pick.1, "n1");
    assert_eq!(pick.0, ids(&["n1", "n2"]));
}

#[test]
fn cycle_survives_session_leaving_fresh_order() {
    // W1 waits, I1 and I2 finished; jumping to I1 marks it seen, so a fresh order is [W1, I2] and no longer holds I1.
    let t = t0();
    let saved = ids(&["W1", "I1", "I2"]);
    let c = Cycle {
        order: saved.clone(),
        id: "I1".into(),
        at: t,
    };
    let after = |s: u64| t + Duration::from_secs(s);
    let pick = advance(
        Some(&c),
        &ids(&["W1", "I2"]),
        live(&["W1", "I1", "I2"]),
        after(2),
    )
    .unwrap();
    assert_eq!(pick.1, "I2");
    assert_eq!(pick.0, saved);
    // A session that disappeared is skipped.
    let skipped = advance(Some(&c), &ids(&["W1"]), live(&["W1", "I1"]), after(2)).unwrap();
    assert_eq!(skipped.1, "W1");
    // After 5 s the cycle starts over from a fresh order.
    let again = advance(
        Some(&c),
        &ids(&["W1", "I2"]),
        live(&["W1", "I1", "I2"]),
        after(6),
    )
    .unwrap();
    assert_eq!(again.1, "W1");
}

#[test]
fn decodes_swift_combo() {
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/hotkey-combo-swift.json"
    ))
    .unwrap();
    let c = HotKeyCombo::from_defaults(&bytes).unwrap();
    assert_eq!(c.key_code, 49);
    assert_eq!(c.modifiers, 0x800);
    assert_eq!(c.label, "⌥Space");
    assert_eq!(c, open_choices()[2]);
    // What Rust writes is the same document, read back by key.
    let v: serde_json::Value = serde_json::from_slice(&c.to_defaults()).unwrap();
    assert_eq!(v["keyCode"], 49);
    assert_eq!(v["modifiers"], 2048);
    assert_eq!(v["label"], "⌥Space");
}

#[test]
fn off_string_means_disabled() {
    let store = MemoryStore::default();
    let default = open_choices()[0].clone();
    // Absent key = the default.
    assert_eq!(
        HotKeyChoice::load(&store, "hotkeyOpen", &default),
        HotKeyChoice::On(default.clone())
    );
    HotKeyChoice::Off.save(&store, "hotkeyOpen");
    assert_eq!(store.get_string("hotkeyOpen").as_deref(), Some("off"));
    assert_eq!(
        HotKeyChoice::load(&store, "hotkeyOpen", &default),
        HotKeyChoice::Off
    );
    let other = next_choices()[1].clone();
    HotKeyChoice::On(other.clone()).save(&store, "hotkeyOpen");
    assert_eq!(
        HotKeyChoice::load(&store, "hotkeyOpen", &default),
        HotKeyChoice::On(other)
    );
    // Data that does not decode disables the hotkey, as Swift's `try?`.
    store.set_bytes("hotkeyOpen", b"garbage");
    assert_eq!(
        HotKeyChoice::load(&store, "hotkeyOpen", &default),
        HotKeyChoice::Off
    );
}
