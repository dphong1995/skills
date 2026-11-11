//! Start at login through `SMAppService` (Swift `main.swift:215`).

use std::path::Path;

use objc2_service_management::{SMAppService, SMAppServiceStatus};

/// Whether the app may register itself: not under `CLAUDE_SESSIONS_ROOT`, and only when the running bundle is under
/// `~/Applications` (a dev or test build must never leave a login item behind).
pub fn allowed(root_env: bool, bundle: &Path, home: &Path) -> bool {
    !root_env && bundle.starts_with(home.join("Applications"))
}

/// Registers the app as a login item unless it already is, when `allowed` for the running bundle. Never called by tests.
pub fn ensure_registered() {
    let root_env = std::env::var_os("CLAUDE_SESSIONS_ROOT").is_some();
    let (Some(exe), Some(home)) = (
        std::env::current_exe().ok(),
        std::env::var_os("HOME").map(std::path::PathBuf::from),
    ) else {
        return;
    };
    // `<bundle>.app/Contents/MacOS/<exe>`
    let Some(bundle) = exe.ancestors().nth(3) else {
        return;
    };
    if bundle.extension().is_none_or(|e| e != "app") || !allowed(root_env, bundle, &home) {
        return;
    }
    // SAFETY: `mainAppService` and `status` have no preconditions; `register` only talks to the system.
    unsafe {
        let service = SMAppService::mainAppService();
        if service.status() != SMAppServiceStatus::Enabled {
            let _ = service.registerAndReturnError();
        }
    }
}
