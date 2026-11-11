//! The collapsed pill: one node with the counts per group (new, `Cần bạn` badge, `Xong`, `Đang chạy`) and the quota ring from 80%.

use super::{Canvas, Ctx};
use crate::settings::BG_OPACITIES;
use crate::theme;
use claude_sessions_core::layout::{
    HEADER_HEIGHT, PILL_GAP, PILL_PAD, PILL_QUOTA_W, collapsed_width, pill_group_width, pill_groups,
};
use claude_sessions_core::quota::QuotaLevel;
use claude_sessions_core::scene::{Align, Node, Point, Rect, Shadow, ThemeColor, Weight};
use claude_sessions_core::store::Snapshot;

pub const KEY: &str = "pill";

/// The opacity at which the pill looks as designed (Swift `Layout.cardOpacity`): the last menu choice.
const FULL_OPACITY: f64 = BG_OPACITIES[BG_OPACITIES.len() - 1];

pub fn quota_token(percent: i64) -> ThemeColor {
    match QuotaLevel::level(percent) {
        QuotaLevel::Normal => ThemeColor::Text,
        QuotaLevel::Warn => ThemeColor::Orange,
        QuotaLevel::High => ThemeColor::Red,
    }
}

/// How far the quota ring is filled, in degrees clockwise from 12 o'clock.
pub fn ring_sweep(percent: i64) -> f64 {
    360.0 * percent.clamp(0, 100) as f64 / 100.0
}

/// Draws the count groups left to right from `x`, centred on `cy`; returns where they end.
pub fn draw_groups(c: &mut Canvas, snap: &Snapshot, x: f64, cy: f64) -> f64 {
    let mut x = x;
    let tokens = [
        ThemeColor::Yellow,
        ThemeColor::Red,
        ThemeColor::Orange,
        ThemeColor::Blue,
    ];
    let mut first = true;
    for (i, n) in pill_groups(snap).into_iter().enumerate() {
        if n == 0 {
            continue;
        }
        if !first {
            x += PILL_GAP;
        }
        first = false;
        let w = pill_group_width(i, n);
        let label = n.to_string();
        match i {
            0 => {
                c.symbol(
                    "sparkle",
                    Rect::new(x, cy - 7.0, 14.0, 14.0),
                    theme::solid(tokens[i]),
                    Weight::Bold,
                );
                c.text(
                    Rect::new(x + 18.0, cy - 8.0, w - 18.0, 16.0),
                    &label,
                    theme::COUNT_FONT,
                    theme::solid(tokens[i]),
                    1,
                    Align::Left,
                );
            }
            1 => {
                // A circle for one digit, a capsule for more.
                let badge = Rect::new(x, cy - 9.0, w, 18.0);
                c.rrect(badge, 9.0, Some(theme::solid(tokens[i])), None);
                c.text(
                    Rect::new(badge.x, cy - 8.0, badge.w, 16.0),
                    &label,
                    theme::COUNT_FONT,
                    theme::solid(ThemeColor::White),
                    1,
                    Align::Center,
                );
            }
            _ => {
                c.rrect(
                    Rect::new(x, cy - 3.5, 7.0, 7.0),
                    3.5,
                    Some(theme::solid(tokens[i])),
                    None,
                );
                c.text(
                    Rect::new(x + 12.0, cy - 8.0, w - 12.0, 16.0),
                    &label,
                    theme::COUNT_FONT,
                    theme::solid(ThemeColor::TextSecondary),
                    1,
                    Align::Left,
                );
            }
        }
        x += w;
    }
    x
}

pub fn node(ctx: &mut Ctx) -> Node {
    let snap = ctx.view.snap;
    let w = collapsed_width(snap);
    let cy = HEADER_HEIGHT / 2.0;
    let mut c = Canvas::default();
    let o = snap.bg_opacity;
    // The thin light border sits inside the node: a stroke centred on the edge would be half clipped away.
    c.rrect(
        Rect::new(0.5, 0.5, w - 1.0, HEADER_HEIGHT - 1.0),
        (HEADER_HEIGHT - 1.0) / 2.0,
        Some(theme::tint(ThemeColor::Panel, o)),
        Some(theme::tint(ThemeColor::Text, 0.18 * o / FULL_OPACITY)),
    );
    if snap.sessions.is_empty() {
        c.rrect(
            Rect::new(PILL_PAD, cy - 3.0, 6.0, 6.0),
            3.0,
            Some(theme::tint(ThemeColor::TextFaint, 1.0)),
            None,
        );
        c.text(
            Rect::new(PILL_PAD + 12.0, cy - 8.0, 20.0, 16.0),
            "–",
            theme::COUNT_FONT,
            theme::solid(ThemeColor::TextFaint),
            1,
            Align::Left,
        );
    }
    let mut x = draw_groups(&mut c, snap, PILL_PAD, cy);
    if let Some(p) = snap.quota.as_ref().and_then(|q| q.pill_percent()) {
        if x > PILL_PAD {
            x += PILL_GAP;
        }
        let token = quota_token(p);
        let ring = Point { x: x + 9.0, y: cy };
        c.arc(
            ring,
            7.0,
            2.0,
            0.0,
            360.0,
            theme::tint(ThemeColor::Text, 0.15),
        );
        c.arc(ring, 7.0, 2.0, 0.0, ring_sweep(p), theme::solid(token));
        c.text(
            Rect::new(x + 22.0, cy - 8.0, PILL_QUOTA_W - 22.0, 16.0),
            &format!("{p}%"),
            theme::COUNT_FONT,
            theme::solid(token),
            1,
            Align::Left,
        );
    }
    let mut node = ctx.node(KEY, Rect::new(0.0, 0.0, w, HEADER_HEIGHT), c);
    // A zero-alpha shadow at o = 0 is kept: the node stays the same shape at every opacity.
    node.shadow = Some(Shadow {
        color: theme::tint(
            theme::PILL_SHADOW.color.token,
            theme::PILL_SHADOW.color.alpha * o / FULL_OPACITY,
        ),
        ..theme::PILL_SHADOW
    });
    node
}
