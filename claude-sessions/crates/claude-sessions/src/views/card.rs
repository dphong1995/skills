//! One session card: status icon, title with the state text beside it, folder line, reply / task / activity lines and the
//! right column (time, `Trả lời…` or `Dừng`). Blocks the card reserves for 6b (approval, question, PR rows, chips) are drawn by
//! their own modules at the rect `card_layout` placed them.

use super::{Canvas, Ctx, approval, chips, pr_row, question};
use crate::hits::{STOP_ARM_SECS, Target};
use crate::theme;
use claude_sessions_core::layout::{
    Block, CARD_INSET, CARD_PAD_RIGHT, CARD_PAD_Y, CARD_W, CardLayout, PANEL_PAD, REPLY_GAP,
    RIGHT_COLUMN, TEXT_W, state_label,
};
use claude_sessions_core::scene::{Align, Node, Rect, ThemeColor, Weight};
use claude_sessions_core::store::{SendKey, SendKind, Session};
use claude_sessions_core::text;
use claude_sessions_core::time::secs_since;

pub fn key(id: &str) -> String {
    format!("card-{id}")
}

/// `folder · branch`, plus the running agent count.
pub fn subtitle(s: &Session) -> String {
    let mut t = s.folder();
    if let Some(b) = s.branch.as_deref().filter(|b| !b.is_empty()) {
        t.push_str(" · ");
        t.push_str(b);
    }
    match s.agent_count() {
        0 => {}
        n => t.push_str(&format!(" · {n} agent")),
    }
    t
}

pub fn node(ctx: &mut Ctx, s: &Session, y: f64, layout: &CardLayout) -> Node {
    let view = ctx.view;
    let (snap, panel) = (view.snap, view.panel);
    let mut c = Canvas::default();
    let fresh = snap.unseen.get(&s.id).is_some_and(|m| m.claude);
    let selected = panel.keyboard && panel.selected.as_deref() == Some(&s.id);
    let hovered = panel.hovered_row.as_deref() == Some(&s.id);
    let fill = if fresh {
        theme::card_fresh_fill()
    } else if hovered {
        theme::card_hover_fill()
    } else {
        theme::card_fill()
    };
    c.rrect(
        Rect::new(0.0, 0.0, CARD_W, layout.height),
        theme::CARD_RADIUS,
        Some(fill),
        selected.then(theme::selection_ring),
    );

    // Status icon; a running card shows the spinner instead.
    let icon = Rect::new(12.0, 11.0, theme::ICON, theme::ICON);
    if s.color_status() == "busy" && !s.waits_in_background() {
        c.spinner = Some(icon);
    } else {
        c.symbol(
            theme::status_symbol(s),
            icon,
            theme::status_color(s),
            Weight::Regular,
        );
    }

    // Title, new mark, state text.
    let mut tx = CARD_INSET;
    if fresh {
        c.symbol(
            "sparkle",
            Rect::new(tx, CARD_PAD_Y + 2.0, 14.0, 14.0),
            theme::solid(ThemeColor::Yellow),
            Weight::Bold,
        );
        tx += 18.0;
    }
    c.text(
        Rect::new(tx, CARD_PAD_Y, layout.title_w, layout.title_h),
        s.title(),
        theme::TITLE_FONT,
        theme::solid(ThemeColor::Text),
        2,
        Align::Left,
    );
    let title_w = view
        .measure
        .measure(s.title(), &theme::TITLE_FONT, layout.title_w, 2)
        .w;
    let label = state_label(s);
    let state_w = ctx.text_w(&label, &theme::STATE_FONT);
    c.text(
        Rect::new(tx + title_w + 6.0, CARD_PAD_Y + 2.0, state_w + 2.0, 15.0),
        &label,
        theme::STATE_FONT,
        theme::status_color(s),
        1,
        Align::Left,
    );
    c.text(
        Rect::new(
            CARD_INSET,
            CARD_PAD_Y + layout.title_h,
            TEXT_W - RIGHT_COLUMN,
            15.0,
        ),
        &subtitle(s),
        theme::SUB_FONT,
        theme::solid(ThemeColor::TextSecondary),
        1,
        Align::Left,
    );

    for p in &layout.blocks {
        let block = Rect::new(0.0, p.y, CARD_W, p.h);
        match p.block {
            Block::Head => {}
            Block::Approval => approval::draw(ctx, &mut c, s, block),
            Block::Question => question::draw(ctx, &mut c, s, block),
            Block::Reply => {
                if let Some(reply) = s.last_reply.as_deref() {
                    c.text(
                        Rect::new(CARD_INSET, p.y + REPLY_GAP, TEXT_W, p.h - REPLY_GAP),
                        reply,
                        theme::REPLY_FONT,
                        theme::tint(ThemeColor::Text, 0.85),
                        2,
                        Align::Left,
                    );
                }
            }
            Block::Todo => {
                if let Some(todo) = &s.todo {
                    let track = Rect::new(CARD_INSET, p.y + 12.0, 44.0, 4.0);
                    c.rrect(track, 2.0, Some(theme::tint(ThemeColor::Text, 0.10)), None);
                    let done = if todo.total == 0 {
                        0.0
                    } else {
                        todo.done as f64 / todo.total as f64
                    };
                    if done > 0.0 {
                        c.rrect(
                            Rect::new(track.x, track.y, track.w * done.min(1.0), 4.0),
                            2.0,
                            Some(theme::solid(ThemeColor::Blue)),
                            None,
                        );
                    }
                    c.text(
                        Rect::new(
                            CARD_INSET + 52.0,
                            p.y + 5.0,
                            TEXT_W - 52.0 - RIGHT_COLUMN,
                            15.0,
                        ),
                        &todo.label(),
                        theme::META_FONT,
                        theme::solid(ThemeColor::TextSecondary),
                        1,
                        Align::Left,
                    );
                }
            }
            Block::Activity => {
                if let Some(a) = s.activity.as_deref() {
                    c.symbol(
                        "chevron.right",
                        Rect::new(CARD_INSET, p.y + 8.0, 9.0, 10.0),
                        theme::solid(ThemeColor::TextSecondary),
                        Weight::Bold,
                    );
                    c.text(
                        Rect::new(
                            CARD_INSET + 14.0,
                            p.y + 5.0,
                            TEXT_W - 14.0 - RIGHT_COLUMN,
                            15.0,
                        ),
                        a,
                        theme::META_FONT,
                        theme::solid(ThemeColor::TextSecondary),
                        1,
                        Align::Left,
                    );
                }
            }
            Block::Pr(i) => pr_row::draw(ctx, &mut c, s, i, block),
            Block::Chips => chips::draw_chips(ctx, &mut c, s, block),
            Block::LinkList => chips::draw_link_list(ctx, &mut c, s, block),
            Block::Editor => reply_editor(&mut c, s, ctx, p.y),
        }
    }

    right_column(ctx, &mut c, s);
    // The card body last: controls above come first in hit order.
    c.hit(
        Rect::new(0.0, 0.0, CARD_W, layout.height),
        Target::Jump(s.id.clone()),
    );
    ctx.node(
        &key(&s.id),
        Rect::new(PANEL_PAD, y, CARD_W, layout.height),
        c,
    )
}

/// The reply field and its `Gửi` button, whose face shows the send state.
fn reply_editor(c: &mut Canvas, s: &Session, ctx: &Ctx, y: f64) {
    let state = ctx
        .view
        .snap
        .send_state(&SendKey::new(&s.id, SendKind::Reply), ctx.view.now);
    let btn = Rect::new(CARD_W - CARD_PAD_RIGHT - 64.0, y + 8.0, 64.0, 28.0);
    c.rrect(
        btn,
        theme::BUTTON_RADIUS,
        Some(theme::solid(ThemeColor::Button)),
        None,
    );
    c.fields.push(claude_sessions_core::scene::Field {
        key: format!("reply-{}", s.id),
        rect: Rect::new(CARD_INSET, y + 8.0, btn.x - CARD_INSET - 8.0, 28.0),
        text: String::new(),
        placeholder: "Nhắn cho Claude…".into(),
    });
    let label = match state {
        None => "Gửi",
        Some(claude_sessions_core::store::SendState::Sending) => "Đang gửi…",
        Some(claude_sessions_core::store::SendState::Sent) => "Đã gửi",
        Some(claude_sessions_core::store::SendState::Failed) => "Lỗi",
    };
    c.text(
        Rect::new(btn.x, btn.y + 6.0, btn.w, 16.0),
        label,
        theme::BUTTON_FONT,
        theme::solid(ThemeColor::Text),
        1,
        Align::Center,
    );
    if state != Some(claude_sessions_core::store::SendState::Sending) {
        c.hit(btn, Target::ReplySend(s.id.clone()));
    }
}

/// Time (or `bấm lại` while `Dừng` is armed) and the quick button under it.
fn right_column(ctx: &Ctx, c: &mut Canvas, s: &Session) {
    let (snap, panel, now) = (ctx.view.snap, ctx.view.panel, ctx.view.now);
    let armed = s.shown_status() == "busy"
        && panel
            .stop_armed
            .get(&s.id)
            .is_some_and(|t| secs_since(now, *t) < STOP_ARM_SECS);
    let col_x = CARD_W - CARD_PAD_RIGHT - 56.0;
    if armed {
        c.text(
            Rect::new(col_x, CARD_PAD_Y, 56.0, 15.0),
            "bấm lại",
            theme::STATE_FONT,
            theme::solid(ThemeColor::Orange),
            1,
            Align::Right,
        );
    } else {
        c.text(
            Rect::new(col_x, CARD_PAD_Y, 56.0, 15.0),
            &text::elapsed(s.since, now),
            theme::TIME_FONT,
            theme::solid(ThemeColor::TextFaint),
            1,
            Align::Right,
        );
    }
    let (symbol, target) = if s.can_ask_claude() {
        ("arrowshape.turn.up.left", Target::Reply(s.id.clone()))
    } else if s.shown_status() == "busy" {
        ("stop.fill", Target::Stop(s.id.clone()))
    } else {
        return;
    };
    let in_cmux = !snap.surface_misses.contains(&s.id);
    let slot = snap
        .send_state(&SendKey::new(&s.id, SendKind::Reply), now)
        .or_else(|| snap.send_state(&SendKey::new(&s.id, SendKind::Stop), now));
    let rect = Rect::new(CARD_W - CARD_PAD_RIGHT - 26.0, 28.0, 26.0, 26.0);
    let fill = if armed {
        theme::tint(ThemeColor::Orange, 0.35)
    } else {
        theme::tint(ThemeColor::Text, 0.08)
    };
    c.rrect(rect, theme::BUTTON_RADIUS, Some(fill), None);
    let mut color = theme::solid(ThemeColor::TextSecondary);
    if !in_cmux {
        color = theme::tint(ThemeColor::TextSecondary, 0.4);
    }
    let clickable = c.send_glyph(rect, slot, symbol, color);
    if clickable && in_cmux {
        c.hit(rect, target);
    }
}
