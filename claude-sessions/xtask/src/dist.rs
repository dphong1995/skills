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
    // `+meta` and `-rc.1000`+ would silently map to build `.0`: reject them so the tag fails loudly.
    let rc_ok = match v.find("-rc.") {
        Some(i) => v[i + 4..].parse::<u32>().is_ok_and(|n| n < 1000),
        None => true,
    };
    v.starts_with(|c: char| c.is_ascii_digit())
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
        && rc_ok
}

/// Numeric `CFBundleVersion` (macOS accepts only dot-separated integers): `X.Y.Z` plus a build number.
/// `X.Y.Z-rc.N` -> `X.Y.Z.N`, a plain `X.Y.Z` -> `X.Y.Z.1000` (so it sorts after its release candidates),
/// any other suffix (dry runs) -> `X.Y.Z.0`. The full string stays in `CFBundleShortVersionString`.
fn bundle_version(v: &str) -> String {
    let (base, suffix) = match v.find(['-', '+']) {
        Some(i) => (&v[..i], Some(&v[i..])),
        None => (v, None),
    };
    let build = match suffix {
        None => 1000,
        Some(s) => s
            .strip_prefix("-rc.")
            .and_then(|n| n.parse::<u32>().ok())
            .filter(|n| *n < 1000)
            .unwrap_or(0),
    };
    let base: Vec<&str> = base.split('.').collect();
    let base = if base
        .iter()
        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
        && base.len() == 3
    {
        base.join(".")
    } else {
        "0.0.0".to_string()
    };
    format!("{base}.{build}")
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
        let mut build = Command::new("cargo");
        build
            .args(["build", "--release", "--locked", "-p", "claude-sessions"])
            .args(["--target", target])
            .current_dir(root);
        if std::env::var_os("MACOSX_DEPLOYMENT_TARGET").is_none() {
            build.env("MACOSX_DEPLOYMENT_TARGET", "14.0");
        }
        sh(&mut build)?;
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
    for (key, value) in [
        ("CFBundleShortVersionString", version.to_string()),
        ("CFBundleVersion", bundle_version(version)),
    ] {
        sh(Command::new("plutil")
            .args(["-replace", key, "-string", &value])
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

#[cfg(test)]
mod tests {
    use super::{bundle_version, valid_version};

    #[test]
    fn valid_version_rejects_forms_that_map_to_build_zero() {
        for ok in ["2.0.0", "2.0.0-rc.1", "2.0.0-rc.999", "0.0.0-dryrun"] {
            assert!(valid_version(ok), "{ok}");
        }
        for bad in [
            "2.0.0-rc.1000",
            "2.0.0-rc.1+meta",
            "2.0.0+meta",
            "2.0.0-rc.x",
            "2.0.0-rc.",
            "v2.0.0",
        ] {
            assert!(!valid_version(bad), "{bad}");
        }
    }

    #[test]
    fn bundle_version_is_numeric() {
        assert_eq!(bundle_version("2.0.0-rc.1"), "2.0.0.1");
        assert_eq!(bundle_version("2.0.0"), "2.0.0.1000");
        assert_eq!(bundle_version("0.0.0-dryrun"), "0.0.0.0");
        assert_eq!(bundle_version("2.1+meta"), "0.0.0.0");
    }
}
