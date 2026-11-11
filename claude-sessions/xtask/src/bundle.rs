use std::path::{Path, PathBuf};
use std::process::Command;

fn sh(cmd: &mut Command) -> Result<(), String> {
    let status = cmd.status().map_err(|e| format!("{cmd:?}: {e}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("{cmd:?} failed"))
}

/// Builds the release binary and wraps it into `target/bundle/ClaudeSessions.app` for the host target, ad-hoc signed.
pub fn run_bundle(root: &Path) -> Result<PathBuf, String> {
    sh(Command::new("cargo")
        .args(["build", "--release", "-p", "claude-sessions"])
        .current_dir(root))?;
    let app = root.join("target/bundle/ClaudeSessions.app");
    let _ = std::fs::remove_dir_all(&app);
    let macos = app.join("Contents/MacOS");
    std::fs::create_dir_all(&macos).map_err(|e| e.to_string())?;
    std::fs::copy(
        root.join("target/release/ClaudeSessions"),
        macos.join("ClaudeSessions"),
    )
    .map_err(|e| e.to_string())?;
    std::fs::copy(
        root.join("crates/claude-sessions/resources/Info.plist"),
        app.join("Contents/Info.plist"),
    )
    .map_err(|e| e.to_string())?;
    sh(Command::new("codesign")
        .args(["--force", "-s", "-"])
        .arg(&app))?;
    Ok(app)
}

pub fn run() -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("no workspace root")?;
    let app = run_bundle(root)?;
    println!("{}", app.display());
    Ok(())
}
