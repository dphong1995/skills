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
