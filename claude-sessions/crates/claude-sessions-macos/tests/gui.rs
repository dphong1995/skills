//! GUI-crate tests: `harness = false`. Set `CLAUDE_SESSIONS_GUI_TESTS=1` to run them.
//!
//! `defaults_reads_swift_values` uses only the throwaway suite `claude-sessions-test-<pid>` (removed at the end, also on
//! failure); it never touches the real `local.claude-sessions` domain and opens no window.

use std::process::Command;

use claude_sessions_core::kv::KeyValueStore;
use claude_sessions_macos::defaults::UserDefaultsStore;
use serde_json::{Map, Value, json};

const CLOSED_SESSIONS: &[u8] =
    include_bytes!("../../claude-sessions-core/tests/fixtures/closed-sessions-swift.json");
const HOTKEY_COMBO: &[u8] =
    include_bytes!("../../claude-sessions-core/tests/fixtures/hotkey-combo-swift.json");

/// The plist Swift's `UserDefaults.set` writes for these values: doubles are `real`, `[String: [Double]]` a dict of real arrays.
const SWIFT_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>pillOrigins</key>
	<dict>
		<key>UUID-A</key>
		<array>
			<real>12</real>
			<real>34</real>
		</array>
	</dict>
	<key>pillX</key>
	<real>100</real>
	<key>pillY</key>
	<real>200</real>
	<key>pillWidth</key>
	<real>232</real>
	<key>bgOpacity</key>
	<real>0.5</real>
	<key>muted</key>
	<true/>
	<key>prPrompt</key>
	<string>x {url}</string>
	<key>jiraProjects</key>
	<dict>
		<key>ABC</key>
		<string>acme.atlassian.net</string>
	</dict>
	<key>hotkeyNext</key>
	<string>off</string>
</dict>
</plist>
"#;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn defaults(args: &[&str]) -> Result<String, String> {
    let out = Command::new("/usr/bin/defaults")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "defaults {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn defaults_reads_swift_values(suite: &str) -> Result<(), String> {
    let plist = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{suite}.plist"));
    std::fs::write(&plist, SWIFT_PLIST).map_err(|e| e.to_string())?;
    defaults(&["import", suite, plist.to_str().ok_or("path")?])?;
    std::fs::remove_file(&plist).ok();
    defaults(&[
        "write",
        suite,
        "closedSessions",
        "-data",
        &hex(CLOSED_SESSIONS),
    ])?;
    defaults(&["write", suite, "hotkeyOpen", "-data", &hex(HOTKEY_COMBO)])?;
    let swift_export = defaults(&["export", suite, "-"])?;

    let store = UserDefaultsStore::new(Some(suite)).ok_or("suite")?;
    let check = |ok: bool, what: &str| {
        if ok {
            Ok(())
        } else {
            Err(format!("read {what}"))
        }
    };
    check(
        store.get_dict("pillOrigins") == json!({"UUID-A": [12.0, 34.0]}).as_object().cloned(),
        "pillOrigins",
    )?;
    check(store.get_f64("pillX") == Some(100.0), "pillX")?;
    check(store.get_f64("pillY") == Some(200.0), "pillY")?;
    check(store.get_f64("pillWidth") == Some(232.0), "pillWidth")?;
    check(store.get_f64("bgOpacity") == Some(0.5), "bgOpacity")?;
    check(store.get_bool("muted") == Some(true), "muted")?;
    check(
        store.get_string("prPrompt").as_deref() == Some("x {url}"),
        "prPrompt",
    )?;
    check(
        store.get_dict("jiraProjects") == json!({"ABC": "acme.atlassian.net"}).as_object().cloned(),
        "jiraProjects",
    )?;
    check(
        store.get_bytes("closedSessions").as_deref() == Some(CLOSED_SESSIONS),
        "closedSessions",
    )?;
    check(
        store.get_bytes("hotkeyOpen").as_deref() == Some(HOTKEY_COMBO),
        "hotkeyOpen",
    )?;
    check(
        store.get_string("hotkeyNext").as_deref() == Some("off"),
        "hotkeyNext",
    )?;
    // A value of another kind reads as absent.
    check(store.get_f64("muted").is_none(), "muted as f64")?;
    check(store.get_string("pillX").is_none(), "pillX as string")?;
    check(store.get_dict("nothing").is_none(), "absent key")?;

    // Write each value through Rust: the exported plist has the same types as the Swift-typed one.
    for key in [
        "pillOrigins",
        "pillX",
        "pillY",
        "pillWidth",
        "bgOpacity",
        "muted",
        "prPrompt",
        "jiraProjects",
        "closedSessions",
        "hotkeyOpen",
        "hotkeyNext",
    ] {
        store.remove(key);
    }
    let origins: Map<String, Value> = json!({"UUID-A": [12.0, 34.0]})
        .as_object()
        .cloned()
        .unwrap();
    let jira: Map<String, Value> = json!({"ABC": "acme.atlassian.net"})
        .as_object()
        .cloned()
        .unwrap();
    store.set_dict("pillOrigins", &origins);
    store.set_f64("pillX", 100.0);
    store.set_f64("pillY", 200.0);
    store.set_f64("pillWidth", 232.0);
    store.set_f64("bgOpacity", 0.5);
    store.set_bool("muted", true);
    store.set_string("prPrompt", "x {url}");
    store.set_dict("jiraProjects", &jira);
    store.set_bytes("closedSessions", CLOSED_SESSIONS);
    store.set_bytes("hotkeyOpen", HOTKEY_COMBO);
    store.set_string("hotkeyNext", "off");
    let rust_export = defaults(&["export", suite, "-"])?;
    if rust_export != swift_export {
        return Err(format!(
            "plist types differ\n--- swift ---\n{swift_export}\n--- rust ---\n{rust_export}"
        ));
    }
    Ok(())
}

fn main() {
    if std::env::var_os("CLAUDE_SESSIONS_GUI_TESTS").is_none() {
        println!("gui tests skipped (set CLAUDE_SESSIONS_GUI_TESTS=1)");
        return;
    }
    let suite = format!("claude-sessions-test-{}", std::process::id());
    let result = defaults_reads_swift_values(&suite);
    UserDefaultsStore::remove_suite(&suite);
    let _ = defaults(&["delete", &suite]);
    // The domain leaves an empty plist behind; it belongs to the throwaway suite only.
    if let Some(home) = std::env::var_os("HOME") {
        let _ = std::fs::remove_file(
            std::path::Path::new(&home).join(format!("Library/Preferences/{suite}.plist")),
        );
    }
    match result {
        Ok(()) => println!("test defaults_reads_swift_values ... ok"),
        Err(e) => {
            println!("test defaults_reads_swift_values ... FAILED: {e}");
            std::process::exit(1);
        }
    }
}
