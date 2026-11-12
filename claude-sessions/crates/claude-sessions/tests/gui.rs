//! GUI tests: `harness = false`, so they run on the main thread. Set `CLAUDE_SESSIONS_GUI_TESTS=1` to run them.
//! Scenes are rendered offscreen (no window, no display). Vibrancy never appears there: the panel shows its fallback fill.
//!
//! `snapshots` renders every core fixture state through the real views at scale 2 into `target/snapshots/<state>.png` and
//! compares it with `tests/snapshots/<state>.png`. A pixel differs when a channel is more than `CHANNEL_TOLERANCE`/255 off;
//! the state fails when more than `MAX_DIFFERING` of its pixels differ, which absorbs font anti-aliasing noise between
//! macOS builds but not a moved or recoloured element. `UPDATE_SNAPSHOTS=1` rewrites the references.

use std::path::{Path, PathBuf};

use claude_sessions::hits::Target;
use claude_sessions::views::{Built, View, build};
use claude_sessions_core::fixtures::{CLOCK, STATES, fixed_now, view_now};
use claude_sessions_core::scene::{Appearance, HitId, Point, Rect, Scene};
use claude_sessions_macos::text::CoreTextMeasure;
use claude_sessions_macos::ui::{
    WidgetPanelHandle, compare_png, read_png_pixel, render_png, testing,
};
use std::cell::RefCell;
use std::rc::Rc;

const SCALE: f64 = 2.0;
const CHANNEL_TOLERANCE: u8 = 2;
const MAX_DIFFERING: f64 = 0.005;

fn built_of(name: &str, build_state: claude_sessions_core::fixtures::StateFn) -> Built {
    let dir = tempfile::Builder::new()
        .prefix("claude-sessions-snapshot-")
        .tempdir()
        .expect("temp dir");
    let now = fixed_now();
    let (snap, panel) = build_state(dir.path(), now);
    let clock = |_: std::time::SystemTime| CLOCK.to_string();
    build(&View {
        snap: &snap,
        panel: &panel,
        measure: &CoreTextMeasure,
        now: view_now(name, now),
        clock: &clock,
    })
}

fn scene_of(name: &str, build_state: claude_sessions_core::fixtures::StateFn) -> Scene {
    built_of(name, build_state).scene
}

fn snapshots_out() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR"))
        .parent()
        .expect("target dir")
        .join("snapshots")
}

fn snapshots() -> Result<(), String> {
    let out = snapshots_out();
    let refs = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots");
    let update = std::env::var_os("UPDATE_SNAPSHOTS").is_some();
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&refs).map_err(|e| e.to_string())?;
    let mut failures = Vec::new();
    for (name, build_state) in STATES {
        let scene = scene_of(name, *build_state);
        let appearance = if name.starts_with("light-") {
            Appearance::Light
        } else {
            Appearance::Dark
        };
        let png = out.join(format!("{name}.png"));
        render_png(&scene, SCALE, appearance, &png)?;
        let reference = refs.join(format!("{name}.png"));
        if update {
            std::fs::copy(&png, &reference).map_err(|e| e.to_string())?;
            continue;
        }
        if !reference.exists() {
            failures.push(format!(
                "{name}: no reference; run with UPDATE_SNAPSHOTS=1 to create {}",
                reference.display()
            ));
            continue;
        }
        let diff = compare_png(&png, &reference, CHANNEL_TOLERANCE)?;
        let ratio = diff.differing as f64 / diff.total.max(1) as f64;
        if diff.size_a != diff.size_b || ratio > MAX_DIFFERING {
            failures.push(format!(
                "{name}: {}/{} pixels differ ({:.2}%), sizes {:?} vs {:?}; see {}",
                diff.differing,
                diff.total,
                ratio * 100.0,
                diff.size_a,
                diff.size_b,
                png.display()
            ));
        }
    }
    println!(
        "snapshots: {} states in {}{}",
        STATES.len(),
        out.display(),
        if update { " (references updated)" } else { "" }
    );
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n  "))
    }
}

/// The mixed hover state renders to a PNG of the scene's size that is not one flat colour.
fn mixed_hover_renders_to_png() -> Result<(), String> {
    let (name, build_state) = STATES
        .iter()
        .find(|(n, _)| *n == "07-mixed-hover")
        .ok_or("no 07-mixed-hover state")?;
    let scene = scene_of(name, *build_state);
    let png = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("mixed-hover.png");
    render_png(&scene, SCALE, Appearance::Dark, &png)?;
    let (size, _) = read_png_pixel(&png, 0, 0)?;
    let want = (
        ((scene.size.w + 2.0 * claude_sessions_core::layout::MARGIN) * SCALE) as usize,
        ((scene.size.h + 2.0 * claude_sessions_core::layout::MARGIN) * SCALE) as usize,
    );
    if size != want {
        return Err(format!("PNG is {size:?} pixels, want {want:?}"));
    }
    let mut colours = Vec::new();
    for i in 1..20 {
        let (_, c) = read_png_pixel(&png, size.0 * i / 20, size.1 * i / 20)?;
        if !colours.contains(&c.map(|v| (v * 255.0) as u8)) {
            colours.push(c.map(|v| (v * 255.0) as u8));
        }
    }
    if colours.len() < 2 {
        return Err("the render is one flat colour".into());
    }
    println!("mixed_hover_renders_to_png: {}", png.display());
    Ok(())
}

/// Phase 2 height targets with the real `CoreTextMeasure`: the mixed hover panel and the two-question card.
fn heights_meet_polish_targets() -> Result<(), String> {
    // The grouped rows at 400 pt measure 358 pt: the next 10 pt up.
    const TARGET: f64 = 360.0;
    let (_, f) = STATES
        .iter()
        .find(|(n, _)| *n == "07-mixed-hover")
        .ok_or("no 07-mixed-hover state")?;
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let (snap, panel) = f(dir.path(), fixed_now());
    let h = claude_sessions_core::layout::expanded_size(&snap, &panel, &CoreTextMeasure).h;
    if h > TARGET {
        return Err(format!("07-mixed-hover is {h} pt tall, want <= {TARGET}"));
    }
    let (name, f) = STATES
        .iter()
        .find(|(n, _)| *n == "question-4-two-questions")
        .ok_or("no question-4 state")?;
    let built = built_of(name, *f);
    let card = built
        .scene
        .nodes
        .iter()
        .find(|n| {
            n.hits
                .iter()
                .any(|h| matches!(built.targets.get(h.id), Some(Target::PickOption { .. })))
        })
        .ok_or("no card with option rows")?;
    if card.frame.h > TARGET {
        return Err(format!(
            "the question card is {} pt tall, want <= {TARGET}",
            card.frame.h
        ));
    }
    Ok(())
}

/// The Edit items answer ⌘X, ⌘C, ⌘V, ⌘A and ⌘Z (Swift `editMenuHandlesClipboardShortcuts`).
fn edit_menu_handles_clipboard_shortcuts() -> Result<(), String> {
    if !claude_sessions_macos::menu::install_main_menu() {
        return Err("no main menu (not the main thread?)".into());
    }
    for key in ["x", "c", "v", "a", "z"] {
        if !claude_sessions_macos::menu::handles_command_key(key) {
            return Err(format!("Cmd-{key} is not handled"));
        }
    }
    Ok(())
}

/// A point on the first card body that no control above it covers, in scene points, and the session it belongs to.
fn card_body_point(built: &Built) -> Result<(Point, String), String> {
    let (id, session) = built
        .targets
        .0
        .iter()
        .find_map(|(id, t)| match t {
            Target::Jump(s) => Some((*id, s.clone())),
            _ => None,
        })
        .ok_or("no Jump target in the scene")?;
    for node in &built.scene.nodes {
        let Some(hit) = node.hits.iter().find(|h| h.id == id) else {
            continue;
        };
        let r = hit.rect;
        for gy in 1..20 {
            for gx in 1..20 {
                let local = Point::new(
                    r.x + r.w * f64::from(gx) / 20.0,
                    r.y + r.h * f64::from(gy) / 20.0,
                );
                let first = node.hits.iter().find(|h| h.rect.contains(local));
                if first.is_some_and(|h| h.id == id) {
                    return Ok((
                        Point::new(node.frame.x + local.x, node.frame.y + local.y),
                        session,
                    ));
                }
            }
        }
    }
    Err("the card body is fully covered by controls".into())
}

/// A press and release on a card body reaches the panel's handler as that card's `Jump`; the panel is never ordered on
/// screen and the events are dispatched to its view, not posted (Swift `ClickTests`).
fn click_row(dragged: bool) -> Result<(), String> {
    let (name, build_state) = STATES
        .iter()
        .find(|(n, _)| *n == "07-mixed-hover")
        .ok_or("no 07-mixed-hover state")?;
    let built = built_of(name, *build_state);
    let (at, session) = card_body_point(&built)?;
    let clicked: Rc<RefCell<Vec<HitId>>> = Rc::default();
    let sink = clicked.clone();
    let size = built.scene.size;
    let window = WidgetPanelHandle::new(
        Rect::new(0.0, 0.0, size.w, size.h),
        Rc::new(move |event| {
            if let claude_sessions_macos::ui::UiEvent::Click(id) = event {
                sink.borrow_mut().push(id);
            }
        }),
    )
    .ok_or("no panel (not the main thread?)")?;
    window.set_scene(&built.scene);
    if !testing::click(&window.view(), at, dragged) {
        return Err("could not make mouse events".into());
    }
    window.close();
    let got: Vec<Target> = clicked
        .borrow()
        .iter()
        .filter_map(|id| built.targets.get(*id).cloned())
        .collect();
    if got == [Target::Jump(session.clone())] {
        Ok(())
    } else {
        Err(format!("clicked {got:?}, want Jump({session})"))
    }
}

fn tap_records_session_id() -> Result<(), String> {
    click_row(false)
}

fn small_move_still_clicks() -> Result<(), String> {
    click_row(true)
}

type Case = fn() -> Result<(), String>;

fn main() {
    if std::env::var_os("CLAUDE_SESSIONS_GUI_TESTS").is_none() {
        println!("gui tests skipped (set CLAUDE_SESSIONS_GUI_TESTS=1)");
        return;
    }
    let cases: [(&str, Case); 6] = [
        ("mixed_hover_renders_to_png", mixed_hover_renders_to_png),
        ("heights_meet_polish_targets", heights_meet_polish_targets),
        ("snapshots", snapshots),
        (
            "edit_menu_handles_clipboard_shortcuts",
            edit_menu_handles_clipboard_shortcuts,
        ),
        ("tap_records_session_id", tap_records_session_id),
        ("small_move_still_clicks", small_move_still_clicks),
    ];
    let mut failed = false;
    for (name, case) in cases {
        match case() {
            Ok(()) => println!("test {name} ... ok"),
            Err(e) => {
                println!("test {name} ... FAILED: {e}");
                failed = true;
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}
