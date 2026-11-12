//! The PRs of a session as one inset list: a rounded dark well of 42 pt items, one per PR. An item has the state badge, a
//! title line (`#n` and the title), a meta line (`text::pr_parts`: checks, comments, CodeRabbit, last reviewer, staleness) and
//! on open PRs the `Nhắc duyệt` / `Nhờ Claude` buttons at the right end. Positions follow `layout::prs_height`.

use super::{Canvas, Ctx, send_label};
use crate::hits::Target;
use crate::theme;
use claude_sessions_core::code_rabbit::{Rabbit, pr_buttons};
use claude_sessions_core::layout::{
    BLOCK_GAP, CARD_INSET, PR_BUTTON_H, PR_ITEM_H, PR_LIST_RADIUS, TEXT_W,
};
use claude_sessions_core::pull_requests::PRStatus;
use claude_sessions_core::scene::{Align, Rect, ThemeColor, Weight};
use claude_sessions_core::store::{SendKey, SendKind, SendState, Session};
use claude_sessions_core::text::{self, PartKind, PartTint, PrPart};
use std::borrow::Cow;

/// Text starts here inside an item (after the 20 pt badge at x 10).
const TEXT_X: f64 = 38.0;
/// Space between the parts of the meta line.
const PART_GAP: f64 = 8.0;
/// Space between the buttons.
const BTN_GAP: f64 = 6.0;
/// The buttons end this far from the list's right edge.
const BTN_EDGE: f64 = 10.0;
/// Space between the text and the buttons.
const TEXT_BTN_GAP: f64 = 8.0;
const BADGE: f64 = 20.0;
const LINE_ONE_Y: f64 = 7.0;
const LINE_ONE_H: f64 = 16.0;
const LINE_TWO_Y: f64 = 24.0;
const LINE_TWO_H: f64 = 14.0;
/// An icon of the meta line and the space after it.
const ICON_W: f64 = 16.0;
/// The least text drawn beside an icon.
const MIN_TEXT_W: f64 = 8.0;
const RABBIT_WORD: &str = "CodeRabbit ";

fn rabbit_color(r: &Rabbit) -> ThemeColor {
    match r {
        Rabbit::Reviewed(Some(n)) if *n > 0 => ThemeColor::Orange,
        Rabbit::Reviewed(_) | Rabbit::Approved => ThemeColor::Green,
        Rabbit::Reviewing | Rabbit::AwaitingApproval | Rabbit::Nudged => ThemeColor::Blue,
        Rabbit::Failed | Rabbit::ApprovalFailed(_) => ThemeColor::Red,
        Rabbit::Skipped => ThemeColor::TextFaint,
    }
}

/// One part of the meta line: an optional icon and its text.
struct Meta {
    icon: Option<&'static str>,
    text: String,
    color: ThemeColor,
    drop_rank: u8,
    help: Option<String>,
    /// The CodeRabbit label: its text may lose the word `CodeRabbit `.
    rabbit: bool,
}

/// The part as drawn on the meta line; `None` for the number, which the title line draws.
fn flow(pr: &PRStatus, p: &PrPart) -> Option<Meta> {
    let tint = |t: Option<PartTint>, base| match t {
        Some(PartTint::Red) => ThemeColor::Red,
        Some(PartTint::Green) => ThemeColor::Green,
        None => base,
    };
    let meta = |icon, text, color| Meta {
        icon,
        text,
        color,
        drop_rank: p.drop_rank,
        help: p.help.clone(),
        rabbit: false,
    };
    match p.kind {
        PartKind::ChangesRequested => Some(meta(
            Some("xmark"),
            p.text.split(' ').next().unwrap_or("").to_string(),
            tint(p.tint, ThemeColor::Red),
        )),
        PartKind::Approved => Some(meta(
            Some("checkmark"),
            p.text.clone(),
            tint(p.tint, ThemeColor::TextSecondary),
        )),
        PartKind::Comments => Some(meta(
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
            Some(meta(None, text, ThemeColor::TextFaint))
        }
        PartKind::Clock => Some(meta(None, p.text.clone(), ThemeColor::TextFaint)),
        // The word stays grey: the state colour (merged: Indigo) is unreadable on a fresh item's tint.
        PartKind::State(_) => Some(Meta {
            drop_rank: 0,
            help: None,
            ..meta(None, p.text.clone(), ThemeColor::TextSecondary)
        }),
        PartKind::Rabbit => Some(Meta {
            rabbit: true,
            ..meta(
                Some("hare"),
                p.text.clone(),
                pr.rabbit
                    .as_ref()
                    .map_or(ThemeColor::TextFaint, rabbit_color),
            )
        }),
        PartKind::Pr => None,
    }
}

/// The meta line of `pr` in the order checks, comments, CodeRabbit, reviewer, staleness.
fn meta_parts(pr: &PRStatus, parts: &[PrPart]) -> Vec<Meta> {
    let mut out: Vec<Meta> = Vec::new();
    let (mut rabbit_at, mut tail_at) = (None, None);
    for p in parts {
        if let Some(m) = flow(pr, p) {
            match p.kind {
                PartKind::Rabbit => rabbit_at = Some(out.len()),
                PartKind::Person | PartKind::Clock => {
                    tail_at.get_or_insert(out.len());
                }
                _ => {}
            }
            out.push(m);
        }
    }
    // CodeRabbit sits after the comments and before the reviewer and the staleness, which always follow it in `parts`.
    if let Some(from) = rabbit_at {
        let rabbit = out.remove(from);
        out.insert(
            tail_at.map_or(out.len(), |t| if from < t { t - 1 } else { t }),
            rabbit,
        );
    }
    out
}

/// `block` is the rect placed for `Block::Prs`, relative to the card.
pub fn draw(ctx: &Ctx, c: &mut Canvas, s: &Session, block: Rect) {
    let n = s.prs.len();
    let list = Rect::new(
        CARD_INSET,
        block.y + BLOCK_GAP,
        TEXT_W,
        n as f64 * PR_ITEM_H,
    );
    c.rrect(
        list,
        PR_LIST_RADIUS,
        Some(theme::solid(ThemeColor::Inset)),
        None,
    );
    for (i, pr) in s.prs.iter().enumerate() {
        let it = Rect::new(list.x, list.y + i as f64 * PR_ITEM_H, list.w, PR_ITEM_H);
        item(ctx, c, s, pr, it, (i == 0, i + 1 == n));
    }
    // The inner edge: the scene strokes 1 pt, so it sits fully inside the list; half the token's alpha reads as 0.5 pt.
    c.rrect(
        Rect::new(list.x + 0.5, list.y + 0.5, list.w - 1.0, list.h - 1.0),
        PR_LIST_RADIUS - 0.5,
        None,
        Some(theme::tint(ThemeColor::InsetEdge, 0.5)),
    );
}

/// What one button slot of an item shows.
struct Slot {
    kind: SendKind,
    label: &'static str,
    primary: bool,
    target: Target,
    /// A button (else the result line of a send whose button is gone).
    button: bool,
    state: Option<SendState>,
    /// The measured width.
    w: f64,
}

/// `place` is (first, last) in the list.
fn item(ctx: &Ctx, c: &mut Canvas, s: &Session, pr: &PRStatus, it: Rect, place: (bool, bool)) {
    let view = ctx.view;
    let snap = view.snap;
    let fresh = snap
        .unseen
        .get(&s.id)
        .is_some_and(|m| m.prs.contains(&pr.ref_));
    if fresh {
        // The scene has no clip or per-corner radius: the first and last item round all four corners like the list, so the
        // inner corners of those two items show a notch of the list's fill.
        let radius = if place.0 || place.1 {
            PR_LIST_RADIUS
        } else {
            0.0
        };
        c.rrect(
            it,
            radius,
            Some(theme::tint(ThemeColor::Indigo, 0.10)),
            None,
        );
    }
    if !place.0 {
        c.hairline(it.x + TEXT_X, it.y, it.w - TEXT_X);
    }
    badge(c, pr, it);
    let area_left = buttons(ctx, c, s, pr, it);

    // Text ends before the buttons; with none it keeps the same margin as the buttons would.
    let limit = area_left.map_or(it.x + it.w - BTN_EDGE, |l| l - TEXT_BTN_GAP);
    let link_w = area_left.map_or(it.w, |l| l - it.x);
    let parts = text::pr_parts(pr, view.now, snap.pr_stale_since);
    let meta = meta_parts(pr, &parts);
    let text_x = it.x + TEXT_X;
    let line_one_y = if meta.is_empty() {
        it.y + (it.h - LINE_ONE_H) / 2.0
    } else {
        it.y + LINE_ONE_Y
    };
    let help_tip = meta_line(ctx, c, it, &meta, limit);
    let title = pr.title.as_deref().filter(|t| !t.is_empty());
    title_line(ctx, c, pr, title, fresh, (text_x, line_one_y), limit);

    // Hover text and the link: the whole item except the button area. The reason of a failed approval keeps its own tip, so
    // the item's tip then leaves the meta line.
    let tip = title
        .map(str::to_string)
        .unwrap_or_else(|| format!("PR #{}", pr.ref_.id));
    let tip_h = if help_tip { LINE_TWO_Y } else { it.h };
    c.tip(Rect::new(it.x, it.y, link_w, tip_h), &tip);
    c.hit(
        Rect::new(it.x, it.y, link_w, it.h),
        Target::Link {
            session: s.id.clone(),
            url: pr.url.clone(),
        },
    );
}

/// The state badge at the item's left.
fn badge(c: &mut Canvas, pr: &PRStatus, it: Rect) {
    let (color, symbol) = if pr.is_open() {
        (ThemeColor::Green, "arrow.triangle.branch")
    } else if pr.state == "MERGED" {
        (ThemeColor::Indigo, "arrow.triangle.merge")
    } else {
        (ThemeColor::Red, "xmark")
    };
    let badge = Rect::new(it.x + 10.0, it.y + (it.h - BADGE) / 2.0, BADGE, BADGE);
    c.rrect(badge, 6.0, Some(theme::tint(color, 0.16)), None);
    c.symbol(
        symbol,
        Rect::new(badge.x + 4.0, badge.y + 4.0, 12.0, 12.0),
        theme::solid(color),
        Weight::Regular,
    );
}

/// The label, whether it is the primary button and the hit of a button kind.
fn face(kind: SendKind, session: &str, pr: &PRStatus) -> (&'static str, bool, Target) {
    let (session, pr) = (session.to_string(), pr.ref_.clone());
    match kind {
        SendKind::Nudge => ("Nhắc duyệt", true, Target::Nudge { session, pr }),
        SendKind::AskClaude => ("Nhờ Claude", false, Target::AskClaude { session, pr }),
        _ => unreachable!("only Nudge and AskClaude are PR buttons"),
    }
}

/// Draws the buttons (or the result of a send whose button is gone), right to left from the edge, `Nhắc duyệt` the left
/// one; returns where the button area starts, `None` without any.
fn buttons(ctx: &Ctx, c: &mut Canvas, s: &Session, pr: &PRStatus, it: Rect) -> Option<f64> {
    if !pr.is_open() {
        return None;
    }
    let (ask, nudge) = pr_buttons(pr);
    let slots: Vec<Slot> = [(SendKind::Nudge, nudge), (SendKind::AskClaude, ask)]
        .into_iter()
        .filter_map(|(kind, button)| {
            let mut key = SendKey::new(&s.id, kind);
            key.pr = Some(pr.ref_.clone());
            let state = ctx.view.snap.send_state(&key, ctx.view.now);
            if !button && state.is_none() {
                return None;
            }
            let (label, primary, target) = face(kind, &s.id, pr);
            let w = if button {
                ctx.button_w(label)
            } else {
                ctx.text_w(
                    send_label(state.unwrap_or(SendState::Sent)),
                    &theme::META_FONT,
                ) + 4.0
            };
            Some(Slot {
                kind,
                label,
                primary,
                target,
                button,
                state,
                w,
            })
        })
        .collect();
    if slots.is_empty() {
        return None;
    }
    let total = slots.iter().map(|sl| sl.w).sum::<f64>() + BTN_GAP * (slots.len() - 1) as f64;
    let area_left = it.x + it.w - BTN_EDGE - total;
    let mut x = area_left;
    for sl in slots {
        if sl.button {
            let rect = Rect::new(x, it.y + (it.h - PR_BUTTON_H) / 2.0, sl.w, PR_BUTTON_H);
            button(
                c,
                sl.label,
                sl.state,
                // Only while Claude can take a message; an earlier send keeps showing its result. A primary button ignores
                // this: `Nhắc duyệt` is always usable.
                sl.kind == SendKind::Nudge || s.can_ask_claude(),
                sl.primary,
                rect,
                || sl.target,
            );
        } else if let Some(state) = sl.state {
            c.sent_line(
                Rect::new(x, it.y + (it.h - 16.0) / 2.0, sl.w, 16.0),
                state,
                Align::Left,
            );
        }
        x += sl.w + BTN_GAP;
    }
    Some(area_left)
}

/// The text of a meta part: the CodeRabbit label wears its name unless the line is too crowded for it.
fn part_text(m: &Meta, word: bool) -> Cow<'_, str> {
    if m.rabbit && word {
        Cow::Owned(format!("{RABBIT_WORD}{}", m.text))
    } else {
        Cow::Borrowed(&m.text)
    }
}

/// Draws the meta line of `it` up to `limit`; returns whether a part carries its own tip.
fn meta_line(ctx: &Ctx, c: &mut Canvas, it: Rect, meta: &[Meta], limit: f64) -> bool {
    let text_x = it.x + TEXT_X;
    let icon_w = |m: &Meta| if m.icon.is_some() { ICON_W } else { 0.0 };
    // Each part measured once with the word `CodeRabbit `, and the CodeRabbit part once without it.
    let widths: Vec<f64> = meta
        .iter()
        .map(|m| ctx.text_w(&part_text(m, true), &theme::META_FONT) + icon_w(m))
        .collect();
    let bare: Vec<f64> = meta
        .iter()
        .zip(&widths)
        .map(|(m, w)| {
            if m.rabbit {
                ctx.text_w(&m.text, &theme::META_FONT) + icon_w(m)
            } else {
                *w
            }
        })
        .collect();
    let shown = |m: &Meta, rank: u8| m.drop_rank == 0 || m.drop_rank > rank;
    let total = |rank: u8, word: bool| -> f64 {
        let ws: Vec<f64> = meta
            .iter()
            .enumerate()
            .filter(|(_, m)| shown(m, rank))
            .map(|(i, _)| if word { widths[i] } else { bare[i] })
            .collect();
        ws.iter().sum::<f64>() + PART_GAP * ws.len().saturating_sub(1) as f64
    };
    // Drop by rank (clock, then person), then the word `CodeRabbit `.
    let (rank, word) = [(0, true), (1, true), (2, true), (2, false)]
        .into_iter()
        .find(|(r, w)| total(*r, *w) <= limit - text_x)
        .unwrap_or((2, false));
    let mut help_tip = false;
    let mut px = text_x;
    for (i, m) in meta.iter().enumerate().filter(|(_, m)| shown(m, rank)) {
        let room = limit - px;
        // Nothing is drawn past the limit: an icon needs its own room and a little text beside it.
        let need = m.icon.map_or(0.0, |_| ICON_W + MIN_TEXT_W);
        if room <= 0.0 || room < need {
            break;
        }
        let w = (if word { widths[i] } else { bare[i] }).min(room);
        let line = Rect::new(px, it.y + LINE_TWO_Y, w, LINE_TWO_H);
        let mut tx = px;
        if let Some(icon) = m.icon {
            c.symbol(
                icon,
                Rect::new(px, line.y + (line.h - 12.0) / 2.0, 12.0, 12.0),
                theme::solid(m.color),
                Weight::Regular,
            );
            tx += ICON_W;
        }
        c.label(
            Rect::new(tx, line.y, (px + w - tx).max(0.0), line.h),
            &part_text(m, word),
            theme::META_FONT,
            theme::solid(m.color),
            Align::Left,
        );
        if let Some(help) = &m.help {
            c.tip(line, help);
            help_tip = true;
        }
        px += w + PART_GAP;
    }
    help_tip
}

/// Line one: [✦ when fresh] `#n` and the title, cut before `limit`. `at` is where the line starts.
fn title_line(
    ctx: &Ctx,
    c: &mut Canvas,
    pr: &PRStatus,
    title: Option<&str>,
    fresh: bool,
    at: (f64, f64),
    limit: f64,
) {
    let (mut lx, y) = at;
    if fresh {
        c.symbol(
            "sparkle",
            Rect::new(lx, y + 2.0, 12.0, 12.0),
            theme::solid(ThemeColor::Indigo),
            Weight::Bold,
        );
        lx += 18.0;
    }
    let number = format!("#{}", pr.ref_.id);
    let number_w = ctx.text_w(&number, &theme::PR_FONT);
    c.text(
        Rect::new(lx, y, number_w, LINE_ONE_H),
        &number,
        theme::PR_FONT,
        theme::solid(ThemeColor::Text),
        1,
        Align::Left,
    );
    if let Some(title) = title {
        let tx = lx + number_w + 6.0;
        if limit - tx > 0.0 {
            c.text(
                Rect::new(tx, y, limit - tx, LINE_ONE_H),
                title,
                theme::PR_TITLE_FONT,
                theme::solid(if pr.is_open() {
                    ThemeColor::Text
                } else {
                    ThemeColor::TextSecondary
                }),
                1,
                Align::Left,
            );
        }
    }
}

/// A capsule button of an item in `rect`. `usable` false dims it and takes its hit away unless a send state is showing;
/// `primary` is the blue call to action.
fn button(
    c: &mut Canvas,
    label: &str,
    state: Option<SendState>,
    usable: bool,
    primary: bool,
    rect: Rect,
    target: impl FnOnce() -> Target,
) {
    let dim = !usable && state.is_none();
    let (fill, text) = if primary {
        (
            theme::solid(ThemeColor::Accent),
            theme::solid(ThemeColor::White),
        )
    } else {
        (
            if dim {
                theme::tint(ThemeColor::Fill, 0.4)
            } else {
                theme::solid(ThemeColor::Fill)
            },
            theme::tint(ThemeColor::Text, if dim { 0.4 } else { 1.0 }),
        )
    };
    if !usable {
        c.tip(rect, "Chỉ gửi được khi Claude đã xong lượt");
    }
    let clickable = c.button(rect, label, fill, text, state);
    if clickable && usable {
        c.hit(rect, target());
    }
}
