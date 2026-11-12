//! One PR row in two tiers. Tier 1: `⑂ #n`, the parts of `text::pr_parts` as icon + number, the status label and the last
//! reviewer at the right end. Tier 2 (open PRs): the `Nhờ Claude` / `Nhắc duyệt` capsules and the CodeRabbit label at the right
//! end; with neither button (`layout::pr_has_buttons`) the row keeps tier 1 only and the label ends it. Positions follow
//! `layout::pr_row_height`.

use super::{Canvas, Ctx, send_label};
use crate::hits::Target;
use crate::theme;
use claude_sessions_core::code_rabbit::{self, Rabbit};
use claude_sessions_core::layout::{
    self, BLOCK_GAP, BUTTON_H, CARD_INSET, PR_GAP, PR_TIER_H, TEXT_W,
};
use claude_sessions_core::scene::{Align, FontSpec, Rect, ThemeColor, Weight};
use claude_sessions_core::store::{SendKey, SendKind, SendState, Session};
use claude_sessions_core::text::{self, PartKind, PartTint, PrPart};

const PART_GAP: f64 = 12.0;
/// Space between the buttons of tier 2.
const BTN_GAP: f64 = 6.0;

fn rabbit_color(r: &Rabbit) -> ThemeColor {
    match r {
        Rabbit::Reviewed(Some(n)) if *n > 0 => ThemeColor::Orange,
        Rabbit::Reviewed(_) | Rabbit::Approved => ThemeColor::Green,
        Rabbit::Reviewing | Rabbit::AwaitingApproval | Rabbit::Nudged => ThemeColor::Blue,
        Rabbit::Failed | Rabbit::ApprovalFailed(_) => ThemeColor::Red,
        Rabbit::Skipped => ThemeColor::TextFaint,
    }
}

/// The icon and text of a part drawn in the flow; `None` for the parts drawn elsewhere (state and CodeRabbit chips).
fn flow(p: &PrPart) -> Option<(Option<&'static str>, String, ThemeColor)> {
    let tint = |t: Option<PartTint>, base| match t {
        Some(PartTint::Red) => ThemeColor::Red,
        Some(PartTint::Green) => ThemeColor::Green,
        None => base,
    };
    match p.kind {
        PartKind::Pr => Some((
            Some("arrow.triangle.branch"),
            p.text.trim_start_matches("PR ").to_string(),
            ThemeColor::Text,
        )),
        PartKind::ChangesRequested => Some((
            Some("xmark"),
            p.text.split(' ').next().unwrap_or("").to_string(),
            tint(p.tint, ThemeColor::Red),
        )),
        PartKind::Approved => Some((
            Some("checkmark"),
            p.text.clone(),
            tint(p.tint, ThemeColor::TextSecondary),
        )),
        PartKind::Comments => Some((
            Some("bubble.left.fill"),
            p.text.clone(),
            ThemeColor::TextSecondary,
        )),
        // `Tin Tran 4m` is drawn `Tin Tran · 4m` (the text keeps its plain form for the one-line summary).
        PartKind::Person => {
            let text = match p.text.rsplit_once(' ') {
                Some((name, ago)) => format!("{name} · {ago}"),
                None => p.text.clone(),
            };
            Some((None, text, ThemeColor::TextFaint))
        }
        PartKind::Clock => Some((Some("clock"), p.text.clone(), ThemeColor::TextFaint)),
        PartKind::State(_) | PartKind::Rabbit => None,
    }
}

/// `block` is the rect placed for `Block::Pr(index)`, relative to the card.
pub fn draw(ctx: &mut Ctx, c: &mut Canvas, s: &Session, index: usize, block: Rect) {
    let Some(pr) = s.prs.get(index) else {
        return;
    };
    let view = ctx.view;
    let snap = view.snap;
    let fresh = snap
        .unseen
        .get(&s.id)
        .is_some_and(|m| m.prs.contains(&pr.ref_));
    let tier1 = Rect::new(CARD_INSET, block.y + PR_GAP, TEXT_W, PR_TIER_H);
    let cy = tier1.y + tier1.h / 2.0;
    let pr_key = |kind| {
        let mut k = SendKey::new(&s.id, kind);
        k.pr = Some(pr.ref_.clone());
        snap.send_state(&k, view.now)
    };

    // The CodeRabbit label is drawn in tier 2, so it leaves the flow of tier 1.
    let mut parts = text::pr_parts(pr, view.now, snap.pr_stale_since);
    let rabbit = parts
        .iter()
        .position(|p| p.kind == PartKind::Rabbit)
        .map(|i| parts.remove(i));

    // Tier 2: the buttons from the left, the CodeRabbit label at the right end.
    let tiered = layout::pr_has_buttons(s, pr, snap);
    if tiered {
        let tier2 = Rect::new(CARD_INSET, tier1.y + tier1.h + BLOCK_GAP, TEXT_W, BUTTON_H);
        let mut left = tier2.x;
        let ask = pr_key(SendKind::AskClaude);
        if !code_rabbit::hides_ask_claude(
            pr.rabbit.as_ref(),
            pr.rabbit_threads,
            pr.rabbit_others_pending,
        ) {
            // Only while Claude can take a message; an earlier send keeps showing its result.
            left = button(
                ctx,
                c,
                "Nhờ Claude",
                ask,
                s.can_ask_claude(),
                left,
                tier2,
                || Target::AskClaude {
                    session: s.id.clone(),
                    pr: pr.ref_.clone(),
                },
            );
        } else if let Some(state) = ask {
            left = state_line(ctx, c, state, left, tier2);
        }
        let nudge = pr_key(SendKind::Nudge);
        if pr.rabbit == Some(Rabbit::AwaitingApproval) {
            left = button(ctx, c, "Nhắc duyệt", nudge, true, left, tier2, || {
                Target::Nudge {
                    session: s.id.clone(),
                    pr: pr.ref_.clone(),
                }
            });
        } else if let Some(state) = nudge {
            left = state_line(ctx, c, state, left, tier2);
        }
        if let (Some(p), Some(r)) = (&rabbit, &pr.rabbit) {
            let y = tier2.y + (tier2.h - 20.0) / 2.0;
            rabbit_chip(ctx, c, &p.text, r, left, tier2.x + tier2.w, y, 20.0);
        }
    }

    // Tier 1: the flow on the left; what does not fit goes by drop rank, lowest first. The reviewer sits at the right end.
    let mut x = tier1.x;
    if fresh {
        c.symbol(
            "sparkle",
            Rect::new(x, cy - 6.0, 12.0, 12.0),
            theme::solid(ThemeColor::Indigo),
            Weight::Bold,
        );
        x += 18.0;
    }
    let width = |p: &PrPart| match (&p.kind, flow(p)) {
        (PartKind::State(_), _) => ctx.text_w(&p.text, &theme::STATE_FONT) + 14.0,
        (_, Some((icon, text, _))) => {
            ctx.text_w(&text, &flow_font(p)) + if icon.is_some() { 16.0 } else { 0.0 }
        }
        _ => 0.0,
    };
    let mut right = tier1.x + tier1.w;
    // Without the button tier the CodeRabbit label takes the right end of tier 1; the reviewer moves left of it.
    if let (false, Some(p), Some(r)) = (tiered, &rabbit, &pr.rabbit) {
        right = rabbit_chip(ctx, c, &p.text, r, x, right, tier1.y, tier1.h) - PART_GAP;
    }
    let total = |rank: u8| -> f64 {
        parts
            .iter()
            .filter(|p| p.drop_rank == 0 || p.drop_rank > rank)
            .map(|p| width(p) + PART_GAP)
            .sum()
    };
    let rank = (0..=2).find(|r| total(*r) <= right - x).unwrap_or(2);
    for p in parts
        .iter()
        .filter(|p| p.drop_rank == 0 || p.drop_rank > rank)
    {
        let w = width(p);
        let at = if p.kind == PartKind::Person {
            right - w
        } else {
            x
        };
        let help = if p.kind == PartKind::Pr {
            pr.title.as_deref()
        } else {
            p.help.as_deref()
        };
        c.tip(Rect::new(at, tier1.y, w, tier1.h), help.unwrap_or_default());
        if p.kind == PartKind::Pr {
            c.hit(
                Rect::new(at, tier1.y, w, tier1.h),
                Target::Link {
                    session: s.id.clone(),
                    url: pr.url.clone(),
                },
            );
        }
        if let PartKind::State(tone) = p.kind {
            let color = theme::tone_token(Some(tone));
            let chip = Rect::new(at, tier1.y, w, tier1.h);
            c.rrect(chip, 5.0, Some(theme::tag_fill(color)), None);
            c.label(
                chip,
                &p.text,
                theme::STATE_FONT,
                theme::solid(color),
                Align::Center,
            );
        } else if let Some((icon, text, color)) = flow(p) {
            let mut tx = at;
            if let Some(icon) = icon {
                c.symbol(
                    icon,
                    Rect::new(at, cy - 6.0, 12.0, 12.0),
                    theme::solid(color),
                    Weight::Regular,
                );
                tx += 16.0;
            }
            c.label(
                Rect::new(tx, tier1.y, w, tier1.h),
                &text,
                flow_font(p),
                theme::solid(color),
                Align::Left,
            );
        }
        if p.kind != PartKind::Person {
            x += w + PART_GAP;
        }
    }
}

/// The CodeRabbit label ending at `right`, no wider than `right - left`, `h` high from `y`; returns its left edge.
#[allow(clippy::too_many_arguments)]
fn rabbit_chip(
    ctx: &Ctx,
    c: &mut Canvas,
    text: &str,
    r: &Rabbit,
    left: f64,
    right: f64,
    y: f64,
    h: f64,
) -> f64 {
    let w = (ctx.text_w(text, &theme::STATE_FONT) + 28.0).min(right - left);
    let chip = Rect::new(right - w, y, w, h);
    let color = rabbit_color(r);
    c.rrect(chip, theme::CHIP_RADIUS, Some(theme::tag_fill(color)), None);
    c.symbol(
        "hare",
        Rect::new(chip.x + 6.0, y + (h - 12.0) / 2.0, 12.0, 12.0),
        theme::solid(color),
        Weight::Regular,
    );
    c.label(
        Rect::new(chip.x + 20.0, y, chip.w - 24.0, h),
        text,
        theme::STATE_FONT,
        theme::solid(color),
        Align::Left,
    );
    chip.x
}

/// The number is bold, the rest is the plain small text.
fn flow_font(p: &PrPart) -> FontSpec {
    if p.kind == PartKind::Pr {
        theme::PR_FONT
    } else {
        theme::META_FONT
    }
}

/// A capsule button of tier 2 from `left`; returns where the next one starts. `usable` false dims it and takes its hit away
/// unless a send state is showing.
#[allow(clippy::too_many_arguments)]
fn button(
    ctx: &Ctx,
    c: &mut Canvas,
    label: &str,
    state: Option<SendState>,
    usable: bool,
    left: f64,
    tier: Rect,
    target: impl FnOnce() -> Target,
) -> f64 {
    let w = ctx.button_w(label);
    let rect = Rect::new(left, tier.y, w, tier.h);
    let dim = !usable && state.is_none();
    let fill = if dim {
        theme::tint(ThemeColor::Fill, 0.4)
    } else {
        theme::solid(ThemeColor::Fill)
    };
    let text = theme::tint(ThemeColor::Text, if dim { 0.4 } else { 1.0 });
    if !usable {
        c.tip(rect, "Chỉ gửi được khi Claude đã xong lượt");
    }
    let clickable = c.button(rect, label, fill, text, state);
    if clickable && usable {
        c.hit(rect, target());
    }
    rect.x + rect.w + BTN_GAP
}

/// The result of a send whose button is gone (the nudge moved CodeRabbit on) as one line, from `left`.
fn state_line(ctx: &Ctx, c: &mut Canvas, state: SendState, left: f64, tier: Rect) -> f64 {
    let w = ctx.text_w(send_label(state), &theme::META_FONT) + 4.0;
    c.sent_line(
        Rect::new(left, tier.y + (tier.h - 16.0) / 2.0, w, 16.0),
        state,
        Align::Left,
    );
    left + w + BTN_GAP
}
