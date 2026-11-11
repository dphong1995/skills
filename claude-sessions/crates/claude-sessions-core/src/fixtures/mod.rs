//! Snapshot states for the static views, behind the crate feature `fixtures` (off by default). Every state is built
//! through a real `Store` from session, transcript and task files written into the temp dir the caller passes in, as
//! Swift `SnapshotTests.swift` builds them; nothing here touches `~/.claude`, the Keychain or the network.
//!
//! The state names are the Swift names (`07-mixed-hover`); the functions replace `-` with `_`. The snapshot holds
//! timestamps only, so a few states also need the clock the views run at: `view_now`.

mod fakes;
mod states;

pub use fakes::*;
pub use states::*;

use crate::panel_state::PanelState;
use crate::store::{Paths, Snapshot, Store, StoreOptions};
use crate::time::unix_secs;
use crate::transcript::project_dir_name;
use crate::transcript_scanner::TranscriptScanner;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const FRONTEND: &str = "/Users/phong/talo6-dct-frontend";
pub const BACKEND: &str = "/Users/phong/talo6-dct-backend";
pub const OCR: &str = "/Users/phong/ocr-service";
pub const TALOS: &str = "/Users/phong/talos-v2";

/// The clock text the snapshots show (`HH:mm`), whatever the time zone.
pub const CLOCK: &str = "09:41";

/// The fixed `now` of every state: 2026-11-03 04:00:00 UTC, just after the PR comment times the CodeRabbit fixtures use.
pub fn fixed_now() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_793_678_400)
}

/// A state: the snapshot and the pill's panel state, from files written into `dir`, at `now`.
pub type StateFn = fn(&Path, SystemTime) -> (Snapshot, PanelState);

/// Our own pid keeps fake sessions alive (`kill(pid, 0)`).
fn me() -> i32 {
    rustix::process::getpid().as_raw_nonzero().get()
}

/// The store setup of every state: transcripts and tasks under the temp dir, no repo reads.
fn options() -> StoreOptions {
    StoreOptions {
        scanner: TranscriptScanner::new(false),
        branch: Some(Box::new(|_| None)),
        ..StoreOptions::default()
    }
}

/// A store over `dir` and the panel of the pill being built.
struct Fx {
    dir: PathBuf,
    now: SystemTime,
    store: Store,
    panel: PanelState,
}

impl Fx {
    fn new(dir: &Path, now: SystemTime) -> Fx {
        Fx::with(dir, now, options())
    }

    fn with(dir: &Path, now: SystemTime, options: StoreOptions) -> Fx {
        let paths = Paths {
            sessions: dir.to_path_buf(),
            projects: dir.join("_projects"),
            tasks: dir.join("_tasks"),
        };
        Fx {
            dir: dir.to_path_buf(),
            now,
            store: Store::new(paths, options),
            panel: PanelState::new(),
        }
    }

    fn at(&self, secs: f64) -> SystemTime {
        if secs >= 0.0 {
            self.now + Duration::from_secs_f64(secs)
        } else {
            self.now - Duration::from_secs_f64(-secs)
        }
    }

    fn refresh(&mut self, secs: f64) {
        let at = self.at(secs);
        self.store.refresh(at);
    }

    /// A registry file; the status changed `age` seconds before `now`.
    fn session(&self, id: &str, status: &str, age: f64, name: &str, cwd: &str) {
        self.session_with(id, me(), status, None, age, name, cwd, None);
    }

    fn waiting(&self, id: &str, waiting_for: &str, age: f64, name: &str, cwd: &str) {
        self.session_with(id, me(), "waiting", Some(waiting_for), age, name, cwd, None);
    }

    #[allow(clippy::too_many_arguments)]
    fn session_with(
        &self,
        id: &str,
        pid: i32,
        status: &str,
        waiting_for: Option<&str>,
        age: f64,
        name: &str,
        cwd: &str,
        name_source: Option<&str>,
    ) {
        let updated = unix_secs(self.now) - age;
        let mut o = json!({"pid": pid, "sessionId": id, "name": name, "cwd": cwd, "status": status,
            "statusUpdatedAt": updated * 1000.0});
        if let Some(w) = waiting_for {
            o["waitingFor"] = json!(w);
        }
        if let Some(n) = name_source {
            o["nameSource"] = json!(n);
        }
        fs::create_dir_all(&self.dir).unwrap();
        fs::write(
            self.dir.join(format!("{id}.json")),
            serde_json::to_vec(&o).unwrap(),
        )
        .unwrap();
    }

    fn transcript(&self, cwd: &str, id: &str, lines: &[String]) {
        let d = self.dir.join("_projects").join(project_dir_name(cwd));
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join(format!("{id}.jsonl")), lines.join("\n")).unwrap();
    }

    /// `<tasks>/<session>/<id>.json` like Claude Code's TaskCreate.
    fn task(&self, session: &str, id: i64, status: &str, subject: &str, active_form: Option<&str>) {
        let d = self.dir.join("_tasks").join(session);
        fs::create_dir_all(&d).unwrap();
        let mut o = json!({"id": id.to_string(), "subject": subject, "status": status, "blocks": [], "blockedBy": []});
        if let Some(a) = active_form {
            o["activeForm"] = json!(a);
        }
        fs::write(
            d.join(format!("{id}.json")),
            serde_json::to_vec(&o).unwrap(),
        )
        .unwrap();
    }

    fn hover(&mut self) {
        self.panel.hovering = true;
    }

    fn done(self) -> (Snapshot, PanelState) {
        (self.store.snapshot(), self.panel)
    }
}

fn json_line(o: Value) -> String {
    serde_json::to_string(&o).unwrap()
}

fn assistant_line(text: &str) -> String {
    json_line(json!({"type": "assistant", "isSidechain": false,
        "message": {"content": [{"type": "text", "text": text}]}}))
}

fn user_line(text: &str) -> String {
    json_line(json!({"type": "user", "message": {"content": text}}))
}

fn command_line(name: &str) -> String {
    let message = &name[1..];
    user_line(&format!(
        "<command-name>{name}</command-name>\n<command-message>{message}</command-message>\n<command-args></command-args>"
    ))
}

fn tool_use_line(id: &str, name: &str, input: Value) -> String {
    json_line(json!({"type": "assistant", "isSidechain": false,
        "message": {"content": [{"type": "tool_use", "id": id, "name": name, "input": input}]}}))
}

/// The main-turn end marker with `background` agents still pending.
fn turn_duration_line(background: i64) -> String {
    json_line(
        json!({"type": "system", "subtype": "turn_duration", "isSidechain": false, "durationMs": 10,
        "pendingBackgroundAgentCount": background}),
    )
}

/// The clock the views run at in the state `name`: `now`, except where the state is built later in time.
pub fn view_now(name: &str, now: SystemTime) -> SystemTime {
    let secs = match name {
        "22-pr-lines" => 61.0,
        "22-pr-lines-stale" => 420.0,
        "50-send-sending" | "51-send-sent" | "52-send-failed" => 2.0,
        "question-5-countdown" | "question-6-sent" => 95.0,
        "question-7-fallback" => 100.0,
        _ => 0.0,
    };
    now + Duration::from_secs_f64(secs)
}
