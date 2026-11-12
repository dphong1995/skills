//! The header of the expanded card: the quota rings with the reset or forecast time, a hairline, then the count groups, all at the
//! right end so a notch never covers them.

use super::pill::{draw_groups, quota_token, ring};
use super::{Canvas, Ctx};
use crate::theme;
use claude_sessions_core::layout::{CARD_W, HEADER_HEIGHT, PANEL_PAD, groups_width};
use claude_sessions_core::quota::Quota;
use claude_sessions_core::scene::{Align, Node, Point, Rect, ThemeColor, Weight};

const RING: f64 = 13.0;
const GAP: f64 = 5.0;
/// Space between the counts and the right edge of the node.
const PAD_RIGHT: f64 = 4.0;
/// Space either side of a divider.
const DIVIDER_MARGIN: f64 = 5.0;

/// A ring is green until the quota warns, then takes the level colour.
fn ring_token(p: i64) -> ThemeColor {
    match quota_token(p) {
        ThemeColor::Text => ThemeColor::Green,
        other => other,
    }
}

/// Draws one ring, then its label, percent and (for the five-hour quota) the reset or forecast; returns where it ends.
fn quota_part(ctx: &Ctx, c: &mut Canvas, x: f64, cy: f64, label: &str, p: i64, note: Note) -> f64 {
    let mut x = x;
    ring(c, Point::new(x + RING / 2.0, cy), 5.25, p, ring_token(p));
    x += RING + GAP;
    let text = |c: &mut Canvas, x: f64, t: &str, font, color| {
        let w = ctx.text_w(t, &font);
        c.text(
            Rect::new(x, cy - 8.0, w + 2.0, 16.0),
            t,
            font,
            theme::solid(color),
            1,
            Align::Left,
        );
        x + w + GAP
    };
    x = text(c, x, label, theme::QUOTA_FONT, ThemeColor::TextSecondary);
    x = text(
        c,
        x,
        &format!("{p}%"),
        theme::QUOTA_BOLD_FONT,
        quota_token(p),
    );
    match note {
        Note::None => {}
        Note::Reset(t) => x = text(c, x, &t, theme::QUOTA_FONT, ThemeColor::TextFaint),
        Note::Forecast(t) => {
            c.symbol(
                "exclamationmark.triangle.fill",
                Rect::new(x, cy - 7.0, 12.0, 14.0),
                theme::solid(ThemeColor::Red),
                Weight::Medium,
            );
            x = text(c, x + 16.0, &t, theme::QUOTA_FONT, ThemeColor::Red);
        }
    }
    x - GAP
}

enum Note {
    None,
    Reset(String),
    Forecast(String),
}

/// Which optional parts of the quota block are drawn: the note after the 5h percent, the weekly quota.
#[derive(Clone, Copy)]
struct Parts {
    note: bool,
    weekly: bool,
}

/// Draws the quota block starting at `x0`; returns where it ends.
fn draw_quota(ctx: &Ctx, c: &mut Canvas, q: &Quota, cy: f64, x0: f64, parts: Parts) -> f64 {
    let clock = ctx.view.clock;
    let note = match (parts.note, q.forecast) {
        (false, _) => Note::None,
        (true, Some(f)) => Note::Forecast(format!("hết ~{}", clock(f))),
        (true, None) => Note::Reset(format!("· reset {}", clock(q.five_hour_reset))),
    };
    let mut x = quota_part(ctx, c, x0, cy, "5h", q.five_hour, note);
    if parts.weekly {
        x += GAP + DIVIDER_MARGIN;
        c.vhairline(x, cy - 5.0, 10.0);
        x += 0.5 + DIVIDER_MARGIN + GAP;
        x = quota_part(ctx, c, x, cy, "Tuần", q.weekly, Note::None);
    }
    x
}

pub fn node(ctx: &mut Ctx, y: f64) -> Node {
    let snap = ctx.view.snap;
    let cy = HEADER_HEIGHT / 2.0;
    let mut c = Canvas::default();
    let counts_w = groups_width(snap);
    let counts_x = CARD_W - PAD_RIGHT - counts_w;
    draw_groups(&mut c, snap, counts_x, cy);
    if let Some(q) = &snap.quota {
        // The block ends before the hairline that separates it from the counts; drop the reset note, then the weekly
        // quota, until it fits in the right half of the header, left of that hairline.
        let (right, line_x) = if counts_w > 0.0 {
            let line_x = counts_x - DIVIDER_MARGIN - 0.5;
            (line_x - DIVIDER_MARGIN, Some(line_x))
        } else {
            (CARD_W - PAD_RIGHT, None)
        };
        let width = |parts: Parts| draw_quota(ctx, &mut Canvas::default(), q, cy, 0.0, parts);
        let parts = [(true, true), (false, true), (false, false)]
            .map(|(note, weekly)| Parts { note, weekly })
            .into_iter()
            .find(|p| width(*p) <= right - CARD_W / 2.0)
            .unwrap_or(Parts {
                note: false,
                weekly: false,
            });
        if let Some(x) = line_x {
            c.vhairline(x, cy - 5.0, 10.0);
        }
        draw_quota(ctx, &mut c, q, cy, right - width(parts), parts);
    }
    ctx.node("header", Rect::new(PANEL_PAD, y, CARD_W, HEADER_HEIGHT), c)
}
