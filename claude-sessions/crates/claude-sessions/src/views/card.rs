//! One session row of an inset grouped list: status glyph, title, the state word and folder line, reply / task / activity
//! lines and the right column (time, and `Trả lời` / `Dừng` while the row is hovered or selected). Rows of one section are
//! drawn as one rounded group, each row knowing its place in it. Blocks (approval, question, PR rows, chips) are drawn by their
//! own modules at the rect `card_layout` placed them.

use super::{Canvas, Ctx, RowPos, approval, chips, pr_row, proc_row, question};
use crate::hits::{Target, is_armed};
use crate::theme;
use claude_sessions_core::layout::{
    BLOCK_GAP, Block, CARD_INSET, CARD_PAD_RIGHT, CARD_PAD_Y, CARD_W, CLOSED_CHILD_H, CardLayout,
    FIELD_H, PANEL_PAD, REPLY_GAP, RIGHT_COLUMN, SPARKLE_W, SUB_H, TEXT_W, TITLE_LINE_H,
    state_label,
};
use claude_sessions_core::procs::{GroupKind, HEAVY_BYTES, SessionProcs, format_bytes};
use claude_sessions_core::scene::{Align, Node, Rect, ThemeColor, Weight};
use claude_sessions_core::store::{SendKey, SendKind, Session};
use claude_sessions_core::text;

/// Space before each part of the process tail, and the chevron at its end.
const SUFFIX_GAP: f64 = 4.0;
const SUFFIX_CHEVRON_W: f64 = 8.0;

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

/// The quiet tail of the folder line of a live card with processes, as `(text, colour)` parts: `· :5173 :8080` (`:5173 +2`
/// from three ports on, `· 2 lệnh nền` without ports), `· 2,6 GB` from `HEAVY_BYTES`, `· CPU cao`. Empty when the session
/// has nothing worth a line.
pub fn procs_suffix(p: &SessionProcs) -> Vec<(String, ThemeColor)> {
    let heavy = p.rss() >= HEAVY_BYTES;
    if !p.has_suffix() {
        return Vec::new();
    }
    let mut parts = Vec::new();
    let mut ports: Vec<u16> = p.commands.iter().flat_map(|g| g.ports.clone()).collect();
    ports.sort_unstable();
    ports.dedup();
    let first = match (ports.len(), p.commands.len()) {
        (0, 0) => None,
        (0, n) => Some(format!("{n} lệnh nền")),
        (1 | 2, _) => Some(
            ports
                .iter()
                .map(|port| format!(":{port}"))
                .collect::<Vec<_>>()
                .join(" "),
        ),
        (n, _) => Some(format!(":{} +{}", ports[0], n - 1)),
    };
    if let Some(t) = first {
        parts.push((format!("· {t}"), ThemeColor::TextSecondary));
    }
    if heavy {
        parts.push((format!("· {}", format_bytes(p.rss())), ThemeColor::Orange));
    }
    if p.cpu_high() {
        parts.push(("· CPU cao".to_string(), ThemeColor::Red));
    }
    parts
}

pub fn node(ctx: &mut Ctx, s: &Session, y: f64, layout: &CardLayout, pos: RowPos) -> Node {
    let view = ctx.view;
    let (snap, panel) = (view.snap, view.panel);
    let mut c = Canvas::default();
    let fresh = snap.unseen.get(&s.id).is_some_and(|m| m.claude);
    let selected = panel.keyboard && panel.selected.as_deref() == Some(&s.id);
    let hovered = panel.hovered_row.as_deref() == Some(&s.id);
    c.row_shape(CARD_W, layout.height, pos, theme::solid(ThemeColor::Group));
    if hovered {
        c.row_shape(
            CARD_W,
            layout.height,
            pos,
            theme::solid(ThemeColor::RowHover),
        );
    }
    if fresh {
        c.row_shape(CARD_W, layout.height, pos, theme::card_fresh_fill());
    }
    if !pos.first {
        c.hairline(CARD_INSET, 0.0, CARD_W - CARD_INSET);
    }
    if selected {
        c.rrect(
            Rect::new(1.5, 1.5, CARD_W - 3.0, layout.height - 3.0),
            theme::GROUP_RADIUS - 2.0,
            None,
            Some(theme::selection_ring()),
        );
    }

    // Status glyph; a running row shows the spinner instead.
    let icon = Rect::new(10.0, CARD_PAD_Y, theme::ICON, theme::ICON);
    if s.color_status() == "busy" && !s.waits_in_background() {
        c.spinner = Some(icon);
    } else {
        // A solid circle with the plain glyph on it: the hierarchical `.circle.fill` symbols draw their glyph dim.
        let glyph = theme::status_symbol(s).trim_end_matches(".circle.fill");
        c.rrect(icon, theme::ICON / 2.0, Some(theme::status_color(s)), None);
        c.symbol(
            glyph,
            Rect::new(icon.x + 5.0, icon.y + 5.0, icon.w - 10.0, icon.h - 10.0),
            theme::solid(ThemeColor::White),
            Weight::Bold,
        );
    }

    // Title, new mark.
    let mut tx = CARD_INSET;
    if fresh {
        c.symbol(
            "sparkle",
            Rect::new(tx, CARD_PAD_Y + 4.0, 12.0, 12.0),
            theme::solid(ThemeColor::Yellow),
            Weight::Bold,
        );
        tx += SPARKLE_W;
    }
    let text_h = layout.title_text_h.min(layout.title_h);
    c.text(
        Rect::new(
            tx,
            CARD_PAD_Y + ((layout.title_h - text_h) / 2.0).floor(),
            layout.title_w,
            text_h,
        ),
        s.title(),
        theme::TITLE_FONT,
        theme::solid(ThemeColor::Text),
        2,
        Align::Left,
    );

    // The folder line: the state word in its colour, then `· folder · branch`.
    let sub = Rect::new(
        CARD_INSET,
        CARD_PAD_Y + layout.title_h,
        TEXT_W - RIGHT_COLUMN,
        SUB_H,
    );
    let label = state_label(s);
    let (rest_x, rest) = if label.is_empty() {
        (sub.x, subtitle(s))
    } else {
        let w = ctx.text_w(&label, &theme::SUB_EM_FONT).ceil();
        c.text(
            Rect::new(sub.x, sub.y, w.min(sub.w), SUB_H),
            &label,
            theme::SUB_EM_FONT,
            theme::status_color(s),
            1,
            Align::Left,
        );
        (sub.x + w + 4.0, format!("· {}", subtitle(s)))
    };
    let avail = (sub.x + sub.w - rest_x).max(0.0);
    // The process tail keeps its room; the folder part gives way first.
    let procs = view.snap.procs.get(&s.id);
    let suffix = procs.map(procs_suffix).unwrap_or_default();
    let widths: Vec<f64> = suffix
        .iter()
        .map(|(t, _)| ctx.text_w(t, &theme::SUB_FONT).ceil())
        .collect();
    let suffix_w = if suffix.is_empty() {
        0.0
    } else {
        widths.iter().map(|w| w + SUFFIX_GAP).sum::<f64>() + SUFFIX_CHEVRON_W
    };
    let rest_w = (avail - suffix_w).max(0.0);
    if rest_w > 0.0 {
        c.text(
            Rect::new(rest_x, sub.y, rest_w, SUB_H),
            &rest,
            theme::SUB_FONT,
            theme::solid(ThemeColor::TextSecondary),
            1,
            Align::Left,
        );
    }
    if !suffix.is_empty() {
        let start = rest_x + ctx.text_w(&rest, &theme::SUB_FONT).ceil().min(rest_w) + SUFFIX_GAP;
        let mut x = start;
        for ((text, color), w) in suffix.iter().zip(&widths) {
            c.text(
                Rect::new(x, sub.y, *w, SUB_H),
                text,
                theme::SUB_FONT,
                theme::solid(*color),
                1,
                Align::Left,
            );
            x += w + SUFFIX_GAP;
        }
        let open = view.panel.open_procs.contains(&s.id);
        c.symbol(
            if open {
                "chevron.down"
            } else {
                "chevron.right"
            },
            Rect::new(x - 1.0, sub.y + 3.0, 8.0, 9.0),
            theme::solid(ThemeColor::TextSecondary),
            Weight::Bold,
        );
        c.hit(
            Rect::new(start - 2.0, sub.y - 2.0, x - start + 12.0, SUB_H + 4.0),
            Target::ToggleProcs(s.id.clone()),
        );
    }
    if s.agent_count() > 0 {
        // The ` · N agent` tail lists the running agents.
        let full = ctx.text_w(&rest, &theme::SUB_FONT);
        let tail = ctx.text_w(&format!(" · {} agent", s.agent_count()), &theme::SUB_FONT);
        let end = (rest_x + full).min(rest_x + rest_w);
        let from = (rest_x + full - tail).min(rest_x + rest_w);
        c.tip(
            Rect::new(from, sub.y, end - from, sub.h),
            &s.agents.join("\n"),
        );
    }

    for p in &layout.blocks {
        let block = Rect::new(0.0, p.y, CARD_W, p.h);
        match p.block {
            Block::Head => {}
            Block::Approval => approval::draw(ctx, &mut c, s, block, layout.approval),
            Block::SentLine => {
                if let Some(state) = view.snap.approval_send(&s.id) {
                    c.sent_line(
                        Rect::new(CARD_INSET, p.y + 4.0, TEXT_W, 16.0),
                        state,
                        Align::Left,
                    );
                }
            }
            Block::Question => question::draw(ctx, &mut c, s, block),
            Block::Reply => {
                if let Some(reply) = s.last_reply.as_deref() {
                    c.rich(
                        Rect::new(CARD_INSET, p.y + REPLY_GAP, TEXT_W, p.h - REPLY_GAP),
                        text::markdown_runs(reply),
                        theme::REPLY_FONT,
                        theme::tint(ThemeColor::Text, 0.85),
                        2,
                        Align::Left,
                    );
                }
            }
            Block::Todo => {
                if let Some(todo) = &s.todo {
                    // The line sits 7 pt under what is above it.
                    let line = Rect::new(CARD_INSET, p.y + 7.0, TEXT_W, SUB_H);
                    let track = Rect::new(line.x, line.y + (SUB_H - 4.0) / 2.0, 56.0, 4.0);
                    c.rrect(track, 2.0, Some(theme::solid(ThemeColor::Fill)), None);
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
                        Rect::new(line.x + 64.0, line.y, line.w - 64.0, SUB_H),
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
                    let line = Rect::new(CARD_INSET, p.y + 7.0, TEXT_W, SUB_H);
                    c.symbol(
                        "chevron.right",
                        Rect::new(line.x, line.y + 3.0, 8.0, 9.0),
                        theme::solid(ThemeColor::TextSecondary),
                        Weight::Bold,
                    );
                    c.text(
                        Rect::new(line.x + 14.0, line.y, line.w - 14.0, SUB_H),
                        a,
                        theme::META_MONO_FONT,
                        theme::solid(ThemeColor::TextSecondary),
                        1,
                        Align::Left,
                    );
                }
            }
            Block::Procs => {
                if let Some(procs) = view.snap.procs.get(&s.id) {
                    for (i, g) in procs.groups().enumerate() {
                        let live = g.kind == GroupKind::ClaudeAndMcp;
                        proc_row::draw(
                            ctx,
                            &mut c,
                            g,
                            p.y + i as f64 * CLOSED_CHILD_H,
                            CARD_INSET,
                            &s.id,
                            live,
                        );
                    }
                }
            }
            Block::Prs => pr_row::draw(ctx, &mut c, s, block),
            Block::Chips => chips::draw_chips(ctx, &mut c, s, block),
            Block::LinkList => chips::draw_link_list(ctx, &mut c, s, block),
            Block::Editor => reply_editor(&mut c, s, ctx, p.y),
        }
    }

    right_column(ctx, &mut c, s, hovered || selected);
    // The row body last: controls above come first in hit order.
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

/// The reply field with the round send button inside it, whose face shows the send state.
fn reply_editor(c: &mut Canvas, s: &Session, ctx: &Ctx, y: f64) {
    let state = ctx
        .view
        .snap
        .send_state(&SendKey::new(&s.id, SendKind::Reply), ctx.view.now);
    let capsule = Rect::new(CARD_INSET, y + BLOCK_GAP, TEXT_W, FIELD_H);
    c.field(
        format!("reply-{}", s.id),
        capsule,
        "Nhắn cho Claude…",
        String::new(),
        true,
    );
    if let Some(btn) = c.round_send(capsule, state) {
        c.hit(btn, Target::ReplySend(s.id.clone()));
    }
}

/// The time at the right end of the title line and, while the row is `active` (hovered or keyboard-selected), the quick
/// button before it: `Trả lời` on a finished row, `Dừng` on a running one. An armed `Dừng` is the red capsule `Dừng?`, in the
/// time's place, and stays until it lapses; a send in flight or done shows its face on the button whether or not the row is
/// active.
fn right_column(ctx: &Ctx, c: &mut Canvas, s: &Session, active: bool) {
    let (snap, panel, now) = (ctx.view.snap, ctx.view.panel, ctx.view.now);
    let right = CARD_W - CARD_PAD_RIGHT;
    let quick = if s.can_ask_claude() {
        Some(("arrowshape.turn.up.left", Target::Reply(s.id.clone())))
    } else if s.shown_status() == "busy" {
        Some(("stop.fill", Target::Stop(s.id.clone())))
    } else {
        None
    };
    let slot = quick.as_ref().and_then(|_| {
        snap.send_state(&SendKey::new(&s.id, SendKind::Reply), now)
            .or_else(|| snap.send_state(&SendKey::new(&s.id, SendKind::Stop), now))
    });
    let in_cmux = !snap.surface_misses.contains(&s.id);
    let tip = if !in_cmux {
        "Chỉ dùng được trong cmux"
    } else if s.can_ask_claude() {
        "Trả lời…"
    } else {
        "Dừng (bấm hai lần)"
    };
    let armed = s.shown_status() == "busy" && is_armed(&panel.stop_armed, &s.id, now);
    if let Some((_, target)) = &quick
        && armed
        && slot.is_none()
    {
        // The capsule takes the place of the time, inside the right column the title leaves free.
        let rect = proc_row::capsule(
            ctx,
            c,
            (right, CARD_PAD_Y),
            "Dừng?",
            ThemeColor::Red,
            ThemeColor::White,
        );
        c.tip(rect, tip);
        if in_cmux {
            c.hit(rect, target.clone());
        }
        return;
    }
    let time = text::elapsed(s.since, now);
    let time_w = ctx.text_w(&time, &theme::TIME_FONT).ceil();
    c.label(
        Rect::new(right - time_w, CARD_PAD_Y, time_w, TITLE_LINE_H),
        &time,
        theme::TIME_FONT,
        theme::solid(ThemeColor::TextFaint),
        Align::Right,
    );
    let Some((symbol, target)) = quick else {
        return;
    };
    if !(active || slot.is_some()) {
        return;
    }
    let rect = Rect::new(
        right - time_w - 6.0 - theme::ROW_BUTTON,
        CARD_PAD_Y,
        theme::ROW_BUTTON,
        theme::ROW_BUTTON,
    );
    if slot.is_none() {
        c.capsule(rect, Some(theme::solid(ThemeColor::Fill)));
    }
    let mut color = theme::solid(ThemeColor::TextSecondary);
    if !in_cmux {
        color = theme::tint(ThemeColor::TextSecondary, 0.4);
    }
    c.tip(rect, tip);
    let clickable = c.send_glyph(rect, slot, symbol, color);
    if clickable && in_cmux {
        c.hit(rect, target);
    }
}
