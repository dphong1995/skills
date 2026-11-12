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

/// The icon and text of a part drawn on the meta line; `None` for the parts drawn elsewhere (the number, the CodeRabbit
/// label and the state word).
fn flow(p: &PrPart) -> Option<(Option<&'static str>, String, ThemeColor)> {
    let tint = |t: Option<PartTint>, base| match t {
        Some(PartTint::Red) => ThemeColor::Red,
        Some(PartTint::Green) => ThemeColor::Green,
        None => base,
    };
    match p.kind {
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
        PartKind::Clock => Some((None, p.text.clone(), ThemeColor::TextFaint)),
        PartKind::Pr | PartKind::State(_) | PartKind::Rabbit => None,
    }
}

/// The meta line of `pr` in the order checks, comments, CodeRabbit, reviewer, staleness.
fn meta_parts(pr: &PRStatus, parts: &[PrPart]) -> Vec<Meta> {
    let mut out: Vec<Meta> = Vec::new();
    let mut rabbit = None;
    for p in parts {
        if let PartKind::State(tone) = p.kind {
            out.push(Meta {
                icon: None,
                text: p.text.clone(),
                color: theme::tone_token(Some(tone)),
                drop_rank: 0,
                help: None,
                rabbit: false,
            });
        } else if p.kind == PartKind::Rabbit {
            let color = pr
                .rabbit
                .as_ref()
                .map_or(ThemeColor::TextFaint, rabbit_color);
            rabbit = Some(Meta {
                icon: Some("hare"),
                text: p.text.clone(),
                color,
                drop_rank: p.drop_rank,
                help: p.help.clone(),
                rabbit: true,
            });
        } else if let Some((icon, text, color)) = flow(p) {
            let m = Meta {
                icon,
                text,
                color,
                drop_rank: p.drop_rank,
                help: p.help.clone(),
                rabbit: false,
            };
            // CodeRabbit sits after the comments and before the reviewer.
            if p.kind == PartKind::Person
                && let Some(r) = rabbit.take()
            {
                out.push(r);
            }
            out.push(m);
        }
    }
    out.extend(rabbit);
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
    // The inner edge: the scene strokes 1 pt, so it sits fully inside the list.
    c.rrect(
        Rect::new(list.x + 0.5, list.y + 0.5, list.w - 1.0, list.h - 1.0),
        PR_LIST_RADIUS - 0.5,
        None,
        Some(theme::solid(ThemeColor::InsetEdge)),
    );
}

/// What one button slot of an item shows.
struct Slot {
    kind: SendKind,
    /// A button (else the result line of a send whose button is gone).
    button: bool,
    state: Option<SendState>,
}

fn item(ctx: &Ctx, c: &mut Canvas, s: &Session, pr: &PRStatus, it: Rect, place: (bool, bool)) {
    let view = ctx.view;
    let snap = view.snap;
    let fresh = snap
        .unseen
        .get(&s.id)
        .is_some_and(|m| m.prs.contains(&pr.ref_));
    let open = pr.is_open();
    if fresh {
        // Rounded like the list on its outer corners; the inner ones are covered by the neighbour's edge.
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

    // Badge.
    let (badge_color, badge_symbol) = if open {
        (ThemeColor::Green, "arrow.triangle.branch")
    } else if pr.state == "MERGED" {
        (ThemeColor::Indigo, "arrow.triangle.merge")
    } else {
        (ThemeColor::Red, "xmark")
    };
    let badge = Rect::new(it.x + 10.0, it.y + (it.h - BADGE) / 2.0, BADGE, BADGE);
    c.rrect(badge, 6.0, Some(theme::tint(badge_color, 0.16)), None);
    c.symbol(
        badge_symbol,
        Rect::new(badge.x + 4.0, badge.y + 4.0, 12.0, 12.0),
        theme::solid(badge_color),
        Weight::Regular,
    );

    // Buttons, right to left from the edge; `Nhắc duyệt` is the left one.
    let pr_key = |kind| {
        let mut k = SendKey::new(&s.id, kind);
        k.pr = Some(pr.ref_.clone());
        snap.send_state(&k, view.now)
    };
    let (ask, nudge) = pr_buttons(pr);
    let slots: Vec<Slot> = [(SendKind::Nudge, nudge), (SendKind::AskClaude, ask)]
        .into_iter()
        .filter_map(|(kind, button)| {
            let state = pr_key(kind);
            (open && (button || state.is_some())).then_some(Slot {
                kind,
                button,
                state,
            })
        })
        .collect();
    let label = |k: SendKind| {
        if k == SendKind::Nudge {
            "Nhắc duyệt"
        } else {
            "Nhờ Claude"
        }
    };
    let width = |sl: &Slot| {
        if sl.button {
            ctx.button_w(label(sl.kind))
        } else {
            ctx.text_w(
                send_label(sl.state.unwrap_or(SendState::Sent)),
                &theme::META_FONT,
            ) + 4.0
        }
    };
    let total: f64 =
        slots.iter().map(width).sum::<f64>() + BTN_GAP * slots.len().saturating_sub(1) as f64;
    let right = it.x + it.w - BTN_EDGE;
    let area_left = right - total;
    let mut x = area_left;
    for sl in &slots {
        let w = width(sl);
        if sl.button {
            let rect = Rect::new(x, it.y + (it.h - PR_BUTTON_H) / 2.0, w, PR_BUTTON_H);
            let target = |pr: &PRStatus| {
                if sl.kind == SendKind::Nudge {
                    Target::Nudge {
                        session: s.id.clone(),
                        pr: pr.ref_.clone(),
                    }
                } else {
                    Target::AskClaude {
                        session: s.id.clone(),
                        pr: pr.ref_.clone(),
                    }
                }
            };
            button(
                c,
                label(sl.kind),
                sl.state,
                // Only while Claude can take a message; an earlier send keeps showing its result.
                sl.kind == SendKind::Nudge || s.can_ask_claude(),
                sl.kind == SendKind::Nudge,
                rect,
                || target(pr),
            );
        } else if let Some(state) = sl.state {
            c.sent_line(
                Rect::new(x, it.y + (it.h - 16.0) / 2.0, w, 16.0),
                state,
                Align::Left,
            );
        }
        x += w + BTN_GAP;
    }
    // Text ends before the buttons; with none it keeps the same margin as the buttons would.
    let limit = if slots.is_empty() {
        right
    } else {
        area_left - TEXT_BTN_GAP
    };
    let link_w = if slots.is_empty() {
        it.w
    } else {
        area_left - it.x
    };

    // Meta line.
    let parts = text::pr_parts(pr, view.now, snap.pr_stale_since);
    let meta = meta_parts(pr, &parts);
    let text_x = it.x + TEXT_X;
    let line_one_y = if meta.is_empty() {
        it.y + (it.h - LINE_ONE_H) / 2.0
    } else {
        it.y + LINE_ONE_Y
    };
    let mut help_tip = false;
    if !meta.is_empty() {
        let shown = |m: &Meta, rank: u8| m.drop_rank == 0 || m.drop_rank > rank;
        let part_w = |m: &Meta, word: bool| {
            let text = rabbit_text(m, word);
            ctx.text_w(&text, &theme::META_FONT) + if m.icon.is_some() { ICON_W } else { 0.0 }
        };
        let total = |rank: u8, word: bool| -> f64 {
            let ws: Vec<f64> = meta
                .iter()
                .filter(|m| shown(m, rank))
                .map(|m| part_w(m, word))
                .collect();
            ws.iter().sum::<f64>() + PART_GAP * ws.len().saturating_sub(1) as f64
        };
        // Drop by rank (clock, then person), then the word `CodeRabbit `.
        let (rank, word) = [(0, true), (1, true), (2, true), (2, false)]
            .into_iter()
            .find(|(r, w)| total(*r, *w) <= limit - text_x)
            .unwrap_or((2, false));
        let mut px = text_x;
        for m in meta.iter().filter(|m| shown(m, rank)) {
            let room = limit - px;
            if room <= 0.0 {
                break;
            }
            let w = part_w(m, word).min(room);
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
                &rabbit_text(m, word),
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
    }

    // Title line.
    let mut lx = text_x;
    if fresh {
        c.symbol(
            "sparkle",
            Rect::new(lx, line_one_y + 2.0, 12.0, 12.0),
            theme::solid(ThemeColor::Indigo),
            Weight::Bold,
        );
        lx += 18.0;
    }
    let number = format!("#{}", pr.ref_.id);
    let number_w = ctx.text_w(&number, &theme::PR_FONT);
    c.text(
        Rect::new(lx, line_one_y, number_w, LINE_ONE_H),
        &number,
        theme::PR_FONT,
        theme::solid(ThemeColor::Text),
        1,
        Align::Left,
    );
    let title = pr.title.as_deref().filter(|t| !t.is_empty());
    if let Some(title) = title {
        let tx = lx + number_w + 6.0;
        if limit - tx > 0.0 {
            c.text(
                Rect::new(tx, line_one_y, limit - tx, LINE_ONE_H),
                title,
                theme::PR_TITLE_FONT,
                theme::solid(if open {
                    ThemeColor::Text
                } else {
                    ThemeColor::TextSecondary
                }),
                1,
                Align::Left,
            );
        }
    }

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

/// The text of a meta part: the CodeRabbit label wears its name unless the line is too crowded for it.
fn rabbit_text(m: &Meta, word: bool) -> String {
    if m.rabbit && word {
        format!("{RABBIT_WORD}{}", m.text)
    } else {
        m.text.clone()
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
