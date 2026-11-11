use std::process::Command;

use crate::dist::{build_dmg, dist_dir, entitlements, root, sh, write_sha256};

const PROFILE: &str = "claude-sessions";

pub fn run(args: &[String]) -> Result<(), String> {
    let (identity, notarize) = match args {
        [flag, name] if flag == "--identity" && !name.is_empty() => (name.as_str(), false),
        [flag, name, n] if flag == "--identity" && !name.is_empty() && n == "--notarize" => {
            (name.as_str(), true)
        }
        _ => return Err("usage: cargo xtask resign --identity <name> [--notarize]".into()),
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

    if notarize {
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
        println!("resign: signed only (no --notarize): not notarized");
    }
    Ok(())
}
