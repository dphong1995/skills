//! The chimes: system sounds by name, as Swift's `playChime`.

use objc2_app_kit::NSSound;
use objc2_foundation::NSString;

use claude_sessions_core::pull_requests::Chime;

/// The system sound a chime plays: `Glass` when a session is done, `Funk` when one needs you.
pub fn sound_name(chime: Chime) -> &'static str {
    match chime {
        Chime::Done => "Glass",
        Chime::NeedsYou => "Funk",
    }
}

pub fn play(chime: Chime) {
    if let Some(sound) = NSSound::soundNamed(&NSString::from_str(sound_name(chime))) {
        sound.play();
    }
}
