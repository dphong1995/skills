//! The real `EffectRunner`: cmux and AppleScript through `claude-sessions-macos`. Blocking cmux calls run on a short-lived
//! thread; AppleScript runs on a fresh main-queue turn. Results are posted back as `Completion`s.

use std::collections::VecDeque;
use std::path::PathBuf;

use claude_sessions_core::action::SendKey;
use claude_sessions_core::closed_sessions::ClosedSession;
use claude_sessions_core::cmux::{self, Key};
use claude_sessions_core::credentials::Service;
use claude_sessions_core::pull_requests::Chime;
use claude_sessions_macos::defaults::beep;
use claude_sessions_macos::{apple_script, main_queue, sound, workspace};
use url::Url;

use crate::actions::{Completion, EffectRunner, Pending, Poster};

/// Seconds an AppleScript call may take before it fails (Swift `CmuxJump.run` timeouts).
const SCRIPT_TIMEOUT: u32 = 5;

pub struct RealRunner {
    poster: Poster,
}

impl RealRunner {
    pub fn new(poster: Poster) -> Self {
        RealRunner { poster }
    }

    /// Runs `work` on a named thread; the result goes to `poster`.
    fn spawn(&self, work: impl FnOnce(&Poster) + Send + 'static) {
        let poster = self.poster.clone();
        let spawned = std::thread::Builder::new()
            .name("claude-sessions.cmux".into())
            .spawn(move || work(&poster));
        if let Err(e) = spawned {
            eprintln!("cmux thread: {e}");
        }
    }
}

impl EffectRunner for RealRunner {
    fn poster(&self) -> Poster {
        self.poster.clone()
    }

    fn beep(&mut self) {
        beep();
    }

    fn play(&mut self, chime: Chime) {
        sound::play(chime);
    }

    fn open_link(&mut self, url: &Url, allow_http: bool) {
        if allow_http {
            workspace::open_link_allow_http(url);
        } else {
            workspace::open_https(url);
        }
    }

    fn copy(&mut self, text: &str) {
        workspace::copy_to_pasteboard(text);
    }

    fn lookup_surface(&mut self, session: &str, pending: Pending) {
        let cli: Option<PathBuf> = workspace::cmux_cli_path();
        let session = session.to_string();
        self.spawn(move |poster| {
            let surface = cli.and_then(|cli| cmux::surface(&cli, &session));
            poster(Completion::Surface(pending, surface));
        });
    }

    fn jump(&mut self, surface: &str, key: Option<SendKey>) {
        let poster = self.poster.clone();
        apple_script::run_later(cmux::jump_script(surface), SCRIPT_TIMEOUT, move |ok| {
            poster(Completion::Jumped(key, ok))
        });
    }

    fn send_keys(&mut self, surface: &str, keys: Vec<Key>, pending: Pending) {
        let poster = self.poster.clone();
        send(
            surface.to_string(),
            keys.into(),
            Box::new(move |ok| poster(Completion::Sent(pending, ok))),
        );
    }

    fn resume(&mut self, closed: &ClosedSession, key: SendKey) {
        let cli = workspace::cmux_cli_path();
        let closed = closed.clone();
        self.spawn(move |poster| {
            let ok = cli.is_some_and(|cli| cmux::resume(&cli, &closed));
            poster(Completion::Resumed(closed, key, ok));
        });
    }

    fn connect(&mut self, service: Service) {
        // The app shows the dialog on a later main-queue turn, with no state borrowed.
        (self.poster)(Completion::OpenSettings(service));
    }
}

/// Types the keys one AppleScript call at a time, waiting out pauses on the main run loop; `done(false)` on the first error.
fn send(surface: String, mut keys: VecDeque<Key>, done: Box<dyn FnOnce(bool)>) {
    match keys.pop_front() {
        None => done(true),
        Some(Key::Pause(seconds)) => {
            main_queue::after(seconds, move || send(surface, keys, done));
        }
        Some(key) => {
            let script = cmux::script(&surface, &key);
            apple_script::run_later(script, SCRIPT_TIMEOUT, move |ok| {
                if ok {
                    send(surface, keys, done);
                } else {
                    done(false);
                }
            });
        }
    }
}
