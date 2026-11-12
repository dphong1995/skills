//! The collapsed pill: one node with the counts per group (new, `Cần bạn` badge, `Xong`, `Đang chạy`).

use super::{Canvas, Ctx};
use crate::theme;
use claude_sessions_core::layout::{
    PILL_GAP, PILL_HEIGHT, PILL_KEY, PILL_PAD, collapsed_width, pill_group_width, pill_groups,
};
use claude_sessions_core::quota::QuotaLevel;
use claude_sessions_core::scene::{Align, Material, Node, Point, Rect, Shadow, ThemeColor, Weight};
use claude_sessions_core::store::Snapshot;

pub const KEY: &str = PILL_KEY;

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

/// A quota ring of `radius` around `center`: the faint track, then the arc swept by `percent` in `token`.
pub fn ring(c: &mut Canvas, center: Point, radius: f64, percent: i64, token: ThemeColor) {
    c.arc(
        center,
        radius,
        2.0,
        0.0,
        360.0,
        theme::tint(ThemeColor::Text, 0.15),
    );
    c.arc(
        center,
        radius,
        2.0,
        0.0,
        ring_sweep(percent),
        theme::solid(token),
    );
}

/// Draws the count groups left to right from `x`, centred on `cy`.
pub fn draw_groups(c: &mut Canvas, snap: &Snapshot, x: f64, cy: f64) {
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
}

pub fn node(ctx: &mut Ctx) -> Node {
    let snap = ctx.view.snap;
    let w = collapsed_width(snap);
    let cy = PILL_HEIGHT / 2.0;
    let mut c = Canvas::default();
    // 1.0 at the opacity the pill was designed for; the tint, the edge and the blur behind all scale with it.
    let k = (snap.bg_opacity / theme::CARD_OPACITY).clamp(0.0, 1.0);
    // The thin light border sits inside the node: a stroke centred on the edge would be half clipped away.
    c.rrect(
        Rect::new(0.5, 0.5, w - 1.0, PILL_HEIGHT - 1.0),
        (PILL_HEIGHT - 1.0) / 2.0,
        Some(theme::tint(ThemeColor::Glass, k)),
        Some(theme::tint(ThemeColor::GlassEdge, k)),
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
    draw_groups(&mut c, snap, PILL_PAD, cy);
    let mut node = ctx.node(KEY, Rect::new(0.0, 0.0, w, PILL_HEIGHT), c);
    node.material = Some(Material::Glass {
        corner: theme::PILL_RADIUS,
        alpha: k,
    });
    // The shadow is kept at zero alpha when the opacity is 0: that avoids clearing and setting the layer's shadow path again
    // when the opacity changes.
    node.shadow = Some(Shadow {
        color: theme::tint(
            theme::PILL_SHADOW.color.token,
            theme::PILL_SHADOW.color.alpha * k,
        ),
        ..theme::PILL_SHADOW
    });
    node
}
