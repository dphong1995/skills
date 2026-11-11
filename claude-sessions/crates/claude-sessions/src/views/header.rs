//! The header of the expanded card: title, quota bars with the reset or forecast time, and the count groups.

use super::pill::{draw_groups, groups_width, quota_token};
use super::{Canvas, Ctx};
use crate::theme;
use claude_sessions_core::layout::{CARD_W, HEADER_HEIGHT, PANEL_PAD};
use claude_sessions_core::quota::Quota;
use claude_sessions_core::scene::{Align, Color, Node, Rect, ThemeColor, Weight};

const BAR_W: f64 = 54.0;
const BAR_H: f64 = 5.0;
const GAP: f64 = 6.0;

enum Piece {
    Label(String, Color),
    Bar(i64),
    Warn(String),
}

fn pieces(ctx: &Ctx, q: &Quota) -> Vec<(Piece, f64)> {
    let clock = ctx.view.clock;
    let faint = theme::solid(ThemeColor::TextFaint);
    let label = |t: &str| {
        (
            Piece::Label(t.to_string(), faint),
            ctx.text_w(t, &theme::QUOTA_FONT),
        )
    };
    let pct = |p: i64| {
        let t = format!("{p}%");
        let w = ctx.text_w(&t, &theme::QUOTA_FONT);
        (Piece::Label(t, theme::solid(quota_token(p))), w)
    };
    let mid = match q.forecast {
        Some(f) => {
            let t = format!("hết ~{}", clock(f));
            let w = ctx.text_w(&t, &theme::QUOTA_FONT) + 16.0;
            (Piece::Warn(t), w)
        }
        None => label(&format!("reset {}", clock(q.five_hour_reset))),
    };
    vec![
        label("5h"),
        (Piece::Bar(q.five_hour), BAR_W),
        pct(q.five_hour),
        mid,
        label("Tuần"),
        (Piece::Bar(q.weekly), BAR_W),
        pct(q.weekly),
    ]
}

pub fn node(ctx: &mut Ctx, y: f64) -> Node {
    let snap = ctx.view.snap;
    let cy = HEADER_HEIGHT / 2.0;
    let mut c = Canvas::default();
    c.text(
        Rect::new(8.0, cy - 9.0, 160.0, 18.0),
        "Claude",
        theme::HEADER_TITLE_FONT,
        theme::solid(ThemeColor::TextSecondary),
        1,
        Align::Left,
    );
    let counts_w = groups_width(snap);
    let counts_x = CARD_W - 8.0 - counts_w;
    draw_groups(&mut c, snap, counts_x, cy);
    if let Some(q) = &snap.quota {
        let parts = pieces(ctx, q);
        let total: f64 = parts.iter().map(|p| p.1).sum::<f64>() + GAP * (parts.len() - 1) as f64;
        let mut x = counts_x - if counts_w > 0.0 { 14.0 } else { 0.0 } - total;
        for (piece, w) in parts {
            match piece {
                Piece::Label(t, color) => c.text(
                    Rect::new(x, cy - 8.0, w + 2.0, 16.0),
                    &t,
                    theme::QUOTA_FONT,
                    color,
                    1,
                    Align::Left,
                ),
                Piece::Bar(p) => {
                    let track = Rect::new(x, cy - BAR_H / 2.0, BAR_W, BAR_H);
                    c.rrect(track, 2.5, Some(theme::tint(ThemeColor::Text, 0.10)), None);
                    let fill = BAR_W * (p.clamp(0, 100) as f64) / 100.0;
                    if fill > 0.0 {
                        c.rrect(
                            Rect::new(x, track.y, fill, BAR_H),
                            2.5,
                            Some(theme::solid(quota_token_bar(p))),
                            None,
                        );
                    }
                }
                Piece::Warn(t) => {
                    c.symbol(
                        "exclamationmark.triangle.fill",
                        Rect::new(x, cy - 7.0, 12.0, 14.0),
                        theme::solid(ThemeColor::Red),
                        Weight::Medium,
                    );
                    c.text(
                        Rect::new(x + 16.0, cy - 8.0, w, 16.0),
                        &t,
                        theme::QUOTA_FONT,
                        theme::solid(ThemeColor::Red),
                        1,
                        Align::Left,
                    );
                }
            }
            x += w + GAP;
        }
    }
    ctx.node("header", Rect::new(PANEL_PAD, y, CARD_W, HEADER_HEIGHT), c)
}

/// Bars are green until the quota warns (mockup `.bar i`), then take the level colour.
fn quota_token_bar(p: i64) -> ThemeColor {
    match quota_token(p) {
        ThemeColor::Text => ThemeColor::Green,
        other => other,
    }
}
