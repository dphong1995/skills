//! Where each display's pill sits: `pillOrigins` (display UUID to the pill origin's offset from that screen's corner), the
//! one-time move of the old `pillX`/`pillY`/`pillWidth` keys, and the panel frame for a pill origin (Swift
//! `syncPanels`, `migrateLegacyPillOrigin`, `place`).

use std::collections::BTreeMap;

use claude_sessions_core::kv::KeyValueStore;
use claude_sessions_core::layout::{
    Anchor, COLLAPSED, canvas_frame, clamp, legacy_offset, near, offset, origin, pill_centre,
    relative_origin,
};
use claude_sessions_core::scene::{Point, Rect};
use claude_sessions_macos::screens::KeyedScreen;
use serde_json::{Map, Value};

pub const ORIGINS_KEY: &str = "pillOrigins";
/// The width of the pill in builds that wrote no `pillWidth`.
const LEGACY_PILL_WIDTH: f64 = 232.0;

/// A pill placed on a screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    /// The pill origin after clamping to the screen.
    pub origin: Point,
    /// What `pillOrigins` stores for it.
    pub offset: [f64; 2],
    pub anchor: Anchor,
    /// The panel frame (the whole canvas around the pill).
    pub frame: Rect,
}

/// The panel frame for a pill origin `saved` on `screen`, as Swift's `place`.
pub fn panel_frame(saved: Point, screen: Rect) -> Placed {
    let o = clamp(saved, screen);
    let anchor = near(pill_centre(o), screen);
    Placed {
        origin: o,
        offset: offset(o, screen),
        anchor,
        frame: canvas_frame(o, anchor),
    }
}

/// The saved offsets; `[String: [Double]]` as a whole, so one malformed entry empties it, as Swift's cast does.
pub fn load_origins(kv: &dyn KeyValueStore) -> BTreeMap<String, Vec<f64>> {
    let Some(dict) = kv.get_dict(ORIGINS_KEY) else {
        return BTreeMap::new();
    };
    dict.iter()
        .map(|(key, value)| {
            let numbers = value
                .as_array()?
                .iter()
                .map(Value::as_f64)
                .collect::<Option<Vec<_>>>()?;
            Some((key.clone(), numbers))
        })
        .collect::<Option<_>>()
        .unwrap_or_default()
}

pub fn save_origins(kv: &dyn KeyValueStore, origins: &BTreeMap<String, Vec<f64>>) {
    let dict: Map<String, Value> = origins
        .iter()
        .map(|(key, offset)| (key.clone(), Value::from(offset.clone())))
        .collect();
    kv.set_dict(ORIGINS_KEY, &dict);
}

/// Saves the offset of a pill the user moved (`place(save: true)`).
pub fn save_origin(kv: &dyn KeyValueStore, key: &str, offset: [f64; 2]) {
    let mut all = load_origins(kv);
    all.insert(key.to_string(), offset.to_vec());
    save_origins(kv, &all);
}

/// Moves `pillX`/`pillY`/`pillWidth` into `pillOrigins` once; the legacy keys go as soon as `pillOrigins` exists.
pub fn migrate_legacy(kv: &dyn KeyValueStore, screens: &[KeyedScreen]) {
    let Some(x) = kv.get_f64("pillX") else {
        return;
    };
    if kv.get_dict(ORIGINS_KEY).is_none() {
        let width = kv.get_f64("pillWidth").unwrap_or(LEGACY_PILL_WIDTH);
        let frames: Vec<Rect> = screens.iter().map(|s| s.frame).collect();
        if let Some((index, offset)) =
            legacy_offset(x, kv.get_f64("pillY").unwrap_or(0.0), width, &frames)
        {
            save_origins(
                kv,
                &BTreeMap::from([(screens[index].key.clone(), offset.to_vec())]),
            );
        }
    }
    if kv.get_dict(ORIGINS_KEY).is_some() {
        for key in ["pillX", "pillY", "pillWidth"] {
            kv.remove(key);
        }
    }
}

/// One placement per screen, in order. A screen with a saved offset uses it; a new one is mapped from the first screen that
/// has one (keeping its distance to the anchored edges) or goes to the top right of its visible frame; both are saved.
pub fn sync(kv: &dyn KeyValueStore, screens: &[KeyedScreen]) -> Vec<Placed> {
    let mut origins = load_origins(kv);
    let mut changed = false;
    let mut placed = Vec::with_capacity(screens.len());
    for screen in screens {
        let saved = origins
            .get(&screen.key)
            .and_then(|o| origin(o, screen.frame));
        let (pill, save) = if let Some(o) = saved {
            (o, false)
        } else if let Some(mapped) = screens
            .iter()
            .find(|s| origins.contains_key(&s.key))
            .and_then(|reference| {
                let ref_origin = origin(&origins[&reference.key], reference.frame)?;
                let anchor = near(pill_centre(ref_origin), reference.frame);
                Some(relative_origin(
                    ref_origin,
                    reference.frame,
                    screen.frame,
                    anchor,
                ))
            })
        {
            (mapped, true)
        } else {
            let v = screen.visible_frame;
            (
                Point::new(
                    v.max_x() - COLLAPSED.w - 16.0,
                    v.max_y() - COLLAPSED.h - 10.0,
                ),
                true,
            )
        };
        let p = panel_frame(pill, screen.frame);
        if save {
            origins.insert(screen.key.clone(), p.offset.to_vec());
            changed = true;
        }
        placed.push(p);
    }
    if changed {
        save_origins(kv, &origins);
    }
    placed
}
