//! GUI tests: `harness = false`, so they run on the main thread. Set `CLAUDE_SESSIONS_GUI_TESTS=1` to run them.
//! Scenes are rendered offscreen (no window, no display); phase 6 builds its snapshots on this.

use std::path::PathBuf;

use claude_sessions::fake;
use claude_sessions_core::scene::{Appearance, ThemeColor};
use claude_sessions_macos::text::CoreTextMeasure;
use claude_sessions_macos::ui::{read_png_pixel, render_png};

fn over(top: [f64; 4], bottom: [f64; 4]) -> [f64; 4] {
    let alpha = top[3] + bottom[3] * (1.0 - top[3]);
    let channel = |i: usize| (top[i] * top[3] + bottom[i] * bottom[3] * (1.0 - top[3])) / alpha;
    [channel(0), channel(1), channel(2), alpha]
}

fn fake_scene_renders_to_png() -> Result<(), String> {
    let scene = fake::expanded_scene(&fake::cards(), 0, &CoreTextMeasure);
    let png = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("fake-expanded.png");
    render_png(&scene, 2.0, Appearance::Dark, &png)?;

    let (size, _) = read_png_pixel(&png, 0, 0)?;
    let want = ((scene.size.w * 2.0) as usize, (scene.size.h * 2.0) as usize);
    if size != want {
        return Err(format!("PNG is {size:?} pixels, want {want:?}"));
    }

    // Right padding of the first card, where nothing but its fill is drawn.
    let row = scene
        .nodes
        .iter()
        .find(|n| n.key.0.starts_with("row-"))
        .ok_or("no card in the scene")?;
    let x = ((row.frame.x + row.frame.w - 4.0) * 2.0) as usize;
    let y = ((row.frame.y + row.frame.h / 2.0) * 2.0) as usize;
    let (_, got) = read_png_pixel(&png, x, y)?;
    let expected = over(
        ThemeColor::RowAttention.rgba(Appearance::Dark),
        ThemeColor::Panel.rgba(Appearance::Dark),
    );
    if got.iter().zip(expected).any(|(g, e)| (g - e).abs() > 0.02) {
        return Err(format!("card pixel {got:?}, want about {expected:?}"));
    }
    println!("fake_scene_renders_to_png: {}", png.display());
    Ok(())
}

type Case = fn() -> Result<(), String>;

fn main() {
    if std::env::var_os("CLAUDE_SESSIONS_GUI_TESTS").is_none() {
        println!("gui tests skipped (set CLAUDE_SESSIONS_GUI_TESTS=1)");
        return;
    }
    let cases: [(&str, Case); 1] = [("fake_scene_renders_to_png", fake_scene_renders_to_png)];
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
