use claude_sessions_core::shell::run_process;
use std::path::Path;
use std::time::Duration;

#[test]
fn input_goes_to_stdin() {
    let out = run_process(
        Path::new("/bin/cat"),
        &[],
        Duration::from_secs(5),
        true,
        Some(b"secret-token"),
    );
    assert_eq!(out.as_deref(), Some("secret-token"));
    // The input is not an argument: `/bin/echo` ignores stdin and prints only its args.
    let out = run_process(
        Path::new("/bin/echo"),
        &["x"],
        Duration::from_secs(5),
        true,
        Some(b"secret-token"),
    );
    assert_eq!(out.as_deref(), Some("x\n"));
}

#[test]
fn require_success_fails_on_nonzero() {
    let f = Path::new("/usr/bin/false");
    assert_eq!(
        run_process(f, &[], Duration::from_secs(5), true, None),
        None
    );
    assert_eq!(
        run_process(f, &[], Duration::from_secs(5), false, None).as_deref(),
        Some("")
    );
}

#[test]
fn run_process_drops_cmux_caller_env() {
    // Only this test touches these CMUX_* names; no other test reads them.
    const NAMES: [&str; 4] = [
        "CMUX_WORKSPACE_ID",
        "CMUX_SURFACE_ID",
        "CMUX_TAB_ID",
        "CMUX_PANEL_ID",
    ];
    let previous: Vec<Option<std::ffi::OsString>> = NAMES.iter().map(std::env::var_os).collect();
    for name in NAMES {
        unsafe { std::env::set_var(name, format!("caller-{name}")) };
    }
    let out = run_process(
        Path::new("/bin/sh"),
        &[
            "-c",
            r#"echo "${CMUX_WORKSPACE_ID-unset}|${CMUX_SURFACE_ID-unset}|${CMUX_TAB_ID-unset}|${CMUX_PANEL_ID-unset}|${HOME-unset}""#,
        ],
        Duration::from_secs(5),
        true,
        None,
    );
    for (name, value) in NAMES.iter().zip(previous) {
        match value {
            Some(value) => unsafe { std::env::set_var(name, value) },
            None => unsafe { std::env::remove_var(name) },
        }
    }
    let home = std::env::var("HOME").unwrap();
    assert_eq!(out, Some(format!("unset|unset|unset|unset|{home}\n")));
}
