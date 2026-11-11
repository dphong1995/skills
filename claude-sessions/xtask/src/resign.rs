use std::process::Command;

use crate::dist::{build_dmg, dist_dir, entitlements, root, sh, write_sha256};

const PROFILE: &str = "claude-sessions";

pub fn run(args: &[String]) -> Result<(), String> {
    let identity = match args {
        [flag, name] if flag == "--identity" && !name.is_empty() => name.as_str(),
        _ => return Err("usage: cargo xtask resign --identity <name>".into()),
    };
    let root = root()?;
    let dist = dist_dir(root);
    let app = dist.join("ClaudeSessions.app");
    if !app.is_dir() {
        return Err(format!(
            "{} not found: run `cargo xtask dist` first",
            app.display()
        ));
    }
    sh(Command::new("codesign")
        .args([
            "--force",
            "--deep",
            "--options",
            "runtime",
            "--timestamp",
            "-s",
            identity,
            "--entitlements",
        ])
        .arg(entitlements(root))
        .arg(&app))?;
    build_dmg(root)?;
    let dmg = dist.join("ClaudeSessions.dmg");

    let has_profile = Command::new("xcrun")
        .args(["notarytool", "history", "--keychain-profile", PROFILE])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if has_profile {
        println!("resign: Developer ID branch (signed with a hardened runtime, notarizing)");
        sh(Command::new("xcrun")
            .args([
                "notarytool",
                "submit",
                "--keychain-profile",
                PROFILE,
                "--wait",
            ])
            .arg(&dmg))?;
        sh(Command::new("xcrun").args(["stapler", "staple"]).arg(&dmg))?;
        write_sha256(&dist)?;
    } else {
        println!("resign: signed only, no notarytool profile `{PROFILE}`: not notarized");
    }
    Ok(())
}
