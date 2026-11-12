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
    // Only this test touches CMUX_WORKSPACE_ID; no other test reads it.
    unsafe { std::env::set_var("CMUX_WORKSPACE_ID", "caller-workspace") };
    let out = run_process(
        Path::new("/bin/sh"),
        &[
            "-c",
            r#"echo "${CMUX_WORKSPACE_ID-unset}|${CMUX_SURFACE_ID-unset}|${HOME-unset}""#,
        ],
        Duration::from_secs(5),
        true,
        None,
    );
    unsafe { std::env::remove_var("CMUX_WORKSPACE_ID") };
    let home = std::env::var("HOME").unwrap();
    assert_eq!(out, Some(format!("unset|unset|{home}\n")));
}
