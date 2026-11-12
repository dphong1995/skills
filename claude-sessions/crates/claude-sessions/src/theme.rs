//! Design tokens of the redesign, from `plans/2026-11-11-claude-sessions-rust/mockups/src/base.css`: colours (resolved per
//! appearance by `ThemeColor`), radii, spacing and font sizes. Dark and light share one layout.

use claude_sessions_core::layout::PILL_HEIGHT;
use claude_sessions_core::scene::{Color, FontSpec, Point, Shadow, ThemeColor, Weight};
use claude_sessions_core::store::Session;

pub use claude_sessions_core::layout::{REPLY_FONT, STATE_FONT, SUB_FONT, TITLE_FONT};

/// Swift's `Layout.cardOpacity`: the opaque end of the background opacity menu, at which the pill looks as designed.
pub const CARD_OPACITY: f64 = 0.92;

pub const PANEL_RADIUS: f64 = 20.0;
pub const PILL_RADIUS: f64 = PILL_HEIGHT / 2.0;
/// The group a run of rows sits in.
pub const GROUP_RADIUS: f64 = 13.0;
pub const CARD_RADIUS: f64 = 12.0;
pub const BUTTON_RADIUS: f64 = 7.0;
pub const CMD_RADIUS: f64 = 8.0;
pub const CHIP_RADIUS: f64 = 6.0;
pub const ICON: f64 = 16.0;

/// Soft shadows cast into the window's transparent margin (`layout::MARGIN`): radius + vertical offset must fit it.
pub const PANEL_SHADOW: Shadow = Shadow {
    color: Color {
        token: ThemeColor::Black,
        alpha: 0.30,
    },
    radius: 11.0,
    offset: Point { x: 0.0, y: 3.0 },
    corner: PANEL_RADIUS,
};
pub const PILL_SHADOW: Shadow = Shadow {
    color: Color {
        token: ThemeColor::Black,
        alpha: 0.22,
    },
    radius: 6.0,
    offset: Point { x: 0.0, y: 2.0 },
    corner: PILL_RADIUS,
};

pub const fn solid(token: ThemeColor) -> Color {
    token.opaque()
}

pub const fn tint(token: ThemeColor, alpha: f64) -> Color {
    Color { token, alpha }
}

pub const fn font(size: f64, weight: Weight) -> FontSpec {
    FontSpec {
        size,
        weight,
        mono: false,
    }
}

pub const COUNT_FONT: FontSpec = font(12.0, Weight::Semibold);
pub const QUOTA_FONT: FontSpec = font(11.0, Weight::Regular);
pub const QUOTA_BOLD_FONT: FontSpec = font(11.0, Weight::Semibold);
pub const SECTION_FONT: FontSpec = font(11.0, Weight::Medium);
/// The title of a `Vừa đóng` row.
pub const ROW_FONT: FontSpec = font(12.5, Weight::Medium);
pub const HINT_FONT: FontSpec = font(11.0, Weight::Regular);
pub const TIME_FONT: FontSpec = font(11.0, Weight::Regular);
pub const META_FONT: FontSpec = font(11.5, Weight::Regular);
pub const BUTTON_FONT: FontSpec = font(12.0, Weight::Semibold);
pub const CAPTION_FONT: FontSpec = font(10.5, Weight::Regular);
pub const META_MONO_FONT: FontSpec = FontSpec {
    size: 11.5,
    weight: Weight::Regular,
    mono: true,
};
pub const MONO_FONT: FontSpec = FontSpec {
    size: 12.0,
    weight: Weight::Regular,
    mono: true,
};

/// The row/card fill (`--card`).
pub fn card_fill() -> Color {
    tint(ThemeColor::Row, 0.92)
}

pub fn card_hover_fill() -> Color {
    tint(ThemeColor::Text, 0.09)
}

/// Laid over the card fill of a card with a fresh Claude mark.
pub fn card_fresh_fill() -> Color {
    tint(ThemeColor::Yellow, 0.04)
}

pub fn selection_ring() -> Color {
    tint(ThemeColor::Text, 0.85)
}

/// Colour of a `color_status`: waiting red, done orange, running blue.
pub fn status_token(color_status: &str) -> ThemeColor {
    match color_status {
        "waiting" => ThemeColor::Red,
        "idle" => ThemeColor::Orange,
        _ => ThemeColor::Blue,
    }
}

pub fn status_color(s: &Session) -> Color {
    solid(status_token(s.color_status()))
}

/// The status icon of a card (SF Symbol names as Swift `Theme.Symbols.status`, filled-circle variants as in the mockups).
pub fn status_symbol(s: &Session) -> &'static str {
    if s.status == "waiting" {
        return if s.is_permission_prompt() {
            "exclamationmark.circle.fill"
        } else {
            "questionmark.circle.fill"
        };
    }
    if s.waits_in_background() {
        return "hourglass.circle.fill";
    }
    match s.shown_status() {
        "idle" => "checkmark.circle.fill",
        _ => "circle.dotted",
    }
}

/// Colour of a link or Jira status tone.
pub fn tone_token(tone: Option<claude_sessions_core::store::ItemTone>) -> ThemeColor {
    use claude_sessions_core::store::ItemTone::*;
    match tone {
        None | Some(Todo) => ThemeColor::TextSecondary,
        Some(Doing) | Some(Open) => ThemeColor::Blue,
        Some(Done) => ThemeColor::Green,
        Some(Merged) => ThemeColor::Purple,
        Some(Closed) => ThemeColor::Red,
    }
}
