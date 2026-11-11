use std::path::{Path, PathBuf};
use std::process::Command;

const TARGETS: [&str; 2] = ["aarch64-apple-darwin", "x86_64-apple-darwin"];
const ENTITLEMENTS: &str = "crates/claude-sessions/resources/ClaudeSessions.entitlements";

pub fn root() -> Result<&'static Path, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| "no workspace root".to_string())
}

pub fn sh(cmd: &mut Command) -> Result<(), String> {
    let status = cmd.status().map_err(|e| format!("{cmd:?}: {e}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("{cmd:?} failed"))
}

pub fn entitlements(root: &Path) -> PathBuf {
    root.join(ENTITLEMENTS)
}

pub fn dist_dir(root: &Path) -> PathBuf {
    root.join("target/dist")
}

/// `hdiutil create` from a folder holding the app and an `Applications` symlink, then the `.sha256` file.
pub fn build_dmg(root: &Path) -> Result<(), String> {
    let dist = dist_dir(root);
    let stage = dist.join("dmg-root");
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage).map_err(|e| e.to_string())?;
    sh(Command::new("ditto")
        .arg(dist.join("ClaudeSessions.app"))
        .arg(stage.join("ClaudeSessions.app")))?;
    std::os::unix::fs::symlink("/Applications", stage.join("Applications"))
        .map_err(|e| e.to_string())?;
    let dmg = dist.join("ClaudeSessions.dmg");
    let _ = std::fs::remove_file(&dmg);
    sh(Command::new("hdiutil")
        .args([
            "create",
            "-quiet",
            "-volname",
            "ClaudeSessions",
            "-srcfolder",
        ])
        .arg(&stage)
        .args(["-ov", "-format", "UDZO"])
        .arg(&dmg))?;
    std::fs::remove_dir_all(&stage).map_err(|e| e.to_string())?;
    write_sha256(&dist)
}

pub fn write_sha256(dist: &Path) -> Result<(), String> {
    let out = Command::new("shasum")
        .args(["-a", "256", "ClaudeSessions.dmg"])
        .current_dir(dist)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err("shasum failed".into());
    }
    std::fs::write(dist.join("ClaudeSessions.dmg.sha256"), out.stdout).map_err(|e| e.to_string())
}

fn valid_version(v: &str) -> bool {
    v.starts_with(|c: char| c.is_ascii_digit())
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'))
}

pub fn run(args: &[String]) -> Result<(), String> {
    let version = match args {
        [flag, v] if flag == "--version" && valid_version(v) => v.as_str(),
        _ => return Err("usage: cargo xtask dist --version X.Y.Z[-rc.N]".into()),
    };
    let root = root()?;
    let dist = dist_dir(root);
    let _ = std::fs::remove_dir_all(&dist);
    let app = dist.join("ClaudeSessions.app");
    let macos = app.join("Contents/MacOS");
    let resources = app.join("Contents/Resources");
    std::fs::create_dir_all(&macos).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&resources).map_err(|e| e.to_string())?;

    for target in TARGETS {
        sh(Command::new("cargo")
            .args(["build", "--release", "--locked", "-p", "claude-sessions"])
            .args(["--target", target])
            .current_dir(root))?;
    }
    let mut lipo = Command::new("lipo");
    lipo.arg("-create");
    for target in TARGETS {
        lipo.arg(root.join(format!("target/{target}/release/ClaudeSessions")));
    }
    sh(lipo.arg("-output").arg(macos.join("ClaudeSessions")))?;

    let plist = app.join("Contents/Info.plist");
    std::fs::copy(
        root.join("crates/claude-sessions/resources/Info.plist"),
        &plist,
    )
    .map_err(|e| e.to_string())?;
    for key in ["CFBundleShortVersionString", "CFBundleVersion"] {
        sh(Command::new("plutil")
            .args(["-replace", key, "-string", version])
            .arg(&plist))?;
    }

    sh(Command::new("cargo")
        .args(["about", "generate", "--locked", "-c", "about.toml", "-o"])
        .arg(resources.join("THIRD_PARTY_LICENSES.txt"))
        .arg("about.hbs")
        .current_dir(root))?;

    sh(Command::new("codesign")
        .args(["--force", "--deep", "-s", "-", "--entitlements"])
        .arg(entitlements(root))
        .arg(&app))?;
    build_dmg(root)?;
    println!("{}", dist.join("ClaudeSessions.dmg").display());
    Ok(())
}
