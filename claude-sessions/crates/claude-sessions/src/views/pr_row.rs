//! One PR row: `⑂ #n`, the parts of `text::pr_parts` as icon + number, the CodeRabbit chip and the `Nhắc duyệt` /
//! `Nhờ Claude` buttons at the end. Positions follow `layout::pr_row_height`.

use super::{Canvas, Ctx};
use crate::hits::Target;
use crate::theme;
use claude_sessions_core::code_rabbit::{self, Rabbit};
use claude_sessions_core::layout::{CARD_INSET, TEXT_W};
use claude_sessions_core::scene::{Align, Rect, ThemeColor, Weight};
use claude_sessions_core::store::{SendKey, SendKind, SendState, Session};
use claude_sessions_core::text::{self, PartKind, PartTint, PrPart};

/// Height of the row's box inside its slot; the rest is the gap to the next row.
const GAP: f64 = 7.0;
const BTN_H: f64 = 22.0;
const PART_GAP: f64 = 14.0;

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
        PartKind::Person => Some((None, p.text.clone(), ThemeColor::TextFaint)),
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
    let row = Rect::new(CARD_INSET, block.y, TEXT_W, block.h - GAP);
    let fill = if fresh {
        theme::tint(ThemeColor::Purple, 0.14)
    } else {
        theme::tint(ThemeColor::Text, 0.05)
    };
    c.rrect(row, 6.0, Some(fill), None);
    let cy = row.y + row.h / 2.0;

    // Right end: the buttons, the CodeRabbit chip left of them.
    let mut right = row.x + row.w - 6.0;
    let pr_key = |kind| {
        let mut k = SendKey::new(&s.id, kind);
        k.pr = Some(pr.ref_.clone());
        snap.send_state(&k, view.now)
    };
    if pr.is_open() {
        let nudge = pr_key(SendKind::Nudge);
        if pr.rabbit == Some(Rabbit::AwaitingApproval) {
            right = button(ctx, c, "Nhắc duyệt", nudge, true, right, cy, || {
                Target::Nudge {
                    session: s.id.clone(),
                    pr: pr.ref_.clone(),
                }
            });
        } else if let Some(state) = nudge {
            right = state_line(ctx, c, state, right, cy);
        }
        let ask = pr_key(SendKind::AskClaude);
        if !code_rabbit::hides_ask_claude(
            pr.rabbit.as_ref(),
            pr.rabbit_threads,
            pr.rabbit_others_pending,
        ) {
            // Only while Claude can take a message; an earlier send keeps showing its result.
            right = button(
                ctx,
                c,
                "Nhờ Claude",
                ask,
                s.can_ask_claude(),
                right,
                cy,
                || Target::AskClaude {
                    session: s.id.clone(),
                    pr: pr.ref_.clone(),
                },
            );
        } else if let Some(state) = ask {
            right = state_line(ctx, c, state, right, cy);
        }
    }
    let mut parts = text::pr_parts(
        pr,
        view.now,
        snap.pr_stale.then_some(snap.pr_last_success).flatten(),
    );
    if let Some(i) = parts.iter().position(|p| p.kind == PartKind::Rabbit) {
        let p = parts.remove(i);
        if let Some(r) = &pr.rabbit {
            let w = ctx.text_w(&p.text, &theme::STATE_FONT) + 28.0;
            let chip = Rect::new(right - w, cy - 10.0, w, 20.0);
            let color = rabbit_color(r);
            c.rrect(
                chip,
                theme::CHIP_RADIUS,
                Some(theme::tint(color, 0.2)),
                None,
            );
            c.symbol(
                "hare",
                Rect::new(chip.x + 6.0, chip.y + 4.0, 12.0, 12.0),
                theme::solid(color),
                Weight::Regular,
            );
            c.label(
                Rect::new(chip.x + 20.0, chip.y, chip.w - 24.0, 20.0),
                &p.text,
                theme::STATE_FONT,
                theme::solid(color),
                Align::Left,
            );
            right = chip.x - 8.0;
        }
    }

    // The flow on the left: what does not fit goes by drop rank, lowest first.
    let mut x = row.x + 10.0;
    if fresh {
        c.symbol(
            "sparkle",
            Rect::new(x, cy - 7.0, 14.0, 14.0),
            theme::solid(ThemeColor::Purple),
            Weight::Bold,
        );
        x += 22.0;
    }
    let width = |p: &PrPart| match (&p.kind, flow(p)) {
        (PartKind::State(_), _) => ctx.text_w(&p.text, &theme::STATE_FONT) + 16.0,
        (_, Some((icon, text, _))) => {
            ctx.text_w(&text, &theme::BUTTON_FONT) + if icon.is_some() { 16.0 } else { 0.0 }
        }
        _ => 0.0,
    };
    let avail = right - x - 8.0;
    let total = |rank: u8| -> f64 {
        parts
            .iter()
            .filter(|p| p.drop_rank == 0 || p.drop_rank > rank)
            .map(|p| width(p) + PART_GAP)
            .sum()
    };
    let rank = (0..=2).find(|r| total(*r) <= avail).unwrap_or(2);
    for p in parts
        .iter()
        .filter(|p| p.drop_rank == 0 || p.drop_rank > rank)
    {
        let w = width(p);
        if let PartKind::State(tone) = p.kind {
            let color = theme::tone_token(Some(tone));
            let chip = Rect::new(x, cy - 10.0, w, 20.0);
            c.rrect(
                chip,
                theme::CHIP_RADIUS,
                Some(theme::tint(color, 0.2)),
                None,
            );
            c.label(
                chip,
                &p.text,
                theme::STATE_FONT,
                theme::solid(color),
                Align::Center,
            );
        } else if let Some((icon, text, color)) = flow(p) {
            let mut tx = x;
            if let Some(icon) = icon {
                c.symbol(
                    icon,
                    Rect::new(x, cy - 6.0, 12.0, 12.0),
                    theme::solid(color),
                    Weight::Regular,
                );
                tx += 16.0;
            }
            let font = if p.kind == PartKind::Pr {
                theme::BUTTON_FONT
            } else {
                theme::META_FONT
            };
            c.label(
                Rect::new(tx, cy - 8.0, w, 16.0),
                &text,
                font,
                theme::solid(color),
                Align::Left,
            );
        }
        x += w + PART_GAP;
    }
}

/// A button at the right end whose left edge becomes the new `right`. `usable` false dims it and takes its hit away unless a
/// send state is showing.
#[allow(clippy::too_many_arguments)]
fn button(
    ctx: &Ctx,
    c: &mut Canvas,
    label: &str,
    state: Option<SendState>,
    usable: bool,
    right: f64,
    cy: f64,
    target: impl FnOnce() -> Target,
) -> f64 {
    let w = ctx.button_w(label);
    let rect = Rect::new(right - w, cy - BTN_H / 2.0, w, BTN_H);
    let dim = !usable && state.is_none();
    let fill = if dim {
        theme::tint(ThemeColor::Button, 0.4)
    } else {
        theme::solid(ThemeColor::Button)
    };
    let text = theme::tint(ThemeColor::Text, if dim { 0.4 } else { 1.0 });
    let clickable = c.button(rect, label, fill, text, state);
    if clickable && usable {
        c.hit(rect, target());
    }
    rect.x - 8.0
}

/// The result of a send whose button is gone (the nudge moved CodeRabbit on) as one line.
fn state_line(ctx: &Ctx, c: &mut Canvas, state: SendState, right: f64, cy: f64) -> f64 {
    let (text, color) = match state {
        SendState::Sending => ("Đang gửi…", ThemeColor::TextSecondary),
        SendState::Sent => ("Đã gửi ✓", ThemeColor::Green),
        SendState::Failed => ("Lỗi — bấm thử lại", ThemeColor::Red),
    };
    let w = ctx.text_w(text, &theme::META_FONT) + 4.0;
    c.label(
        Rect::new(right - w, cy - 8.0, w, 16.0),
        text,
        theme::META_FONT,
        theme::solid(color),
        Align::Right,
    );
    right - w - 8.0
}
