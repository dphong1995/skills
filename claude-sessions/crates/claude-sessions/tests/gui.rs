//! GUI tests: `harness = false`, so they run on the main thread. Set `CLAUDE_SESSIONS_GUI_TESTS=1` to run them.
//! Scenes are rendered offscreen (no window, no display). Vibrancy never appears there: the panel shows its fallback fill.
//!
//! `snapshots` renders every core fixture state through the real views at scale 2 into `target/snapshots/<state>.png` and
//! compares it with `tests/snapshots/<state>.png`. A pixel differs when a channel is more than `CHANNEL_TOLERANCE`/255 off;
//! the state fails when more than `MAX_DIFFERING` of its pixels differ, which absorbs font anti-aliasing noise between
//! macOS builds but not a moved or recoloured element. `UPDATE_SNAPSHOTS=1` rewrites the references.

use std::path::{Path, PathBuf};

use claude_sessions::views::{View, build};
use claude_sessions_core::fixtures::{CLOCK, STATES, fixed_now, view_now};
use claude_sessions_core::scene::{Appearance, Scene};
use claude_sessions_macos::text::CoreTextMeasure;
use claude_sessions_macos::ui::{compare_png, read_png_pixel, render_png};

const SCALE: f64 = 2.0;
const CHANNEL_TOLERANCE: u8 = 2;
const MAX_DIFFERING: f64 = 0.005;

fn scene_of(name: &str, build_state: claude_sessions_core::fixtures::StateFn) -> Scene {
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
    .scene
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
        (scene.size.w * SCALE) as usize,
        (scene.size.h * SCALE) as usize,
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

type Case = fn() -> Result<(), String>;

fn main() {
    if std::env::var_os("CLAUDE_SESSIONS_GUI_TESTS").is_none() {
        println!("gui tests skipped (set CLAUDE_SESSIONS_GUI_TESTS=1)");
        return;
    }
    let cases: [(&str, Case); 2] = [
        ("mixed_hover_renders_to_png", mixed_hover_renders_to_png),
        ("snapshots", snapshots),
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
