//! The `Vừa đóng N` disclosure row and its open list: one group with a row per closed session (`layout::closed_height`) and a
//! separator between rows. A row opens the session (`Target::Resume`) and shows `Đang mở…` / `Lỗi — đã copy lệnh` while its send
//! is in flight or failed. A row whose session left processes running is orange-dotted, says `N đang chạy · 886 MB ›` at its right
//! end (one hit that opens its child rows) and has the kill control; the title row counts the leftovers while collapsed.

use super::{Canvas, Ctx, proc_row, section};
use crate::hits::Target;
use crate::theme;
use claude_sessions_core::layout::{
    CARD_PAD_RIGHT, CARD_W, CLOSED_CHILD_H, CLOSED_ITEM_H, CLOSED_ROW_H, KEY_HINT_H, PANEL_PAD,
    closed_rows,
};
use claude_sessions_core::procs::{SessionProcs, format_bytes};
use claude_sessions_core::scene::{Align, Node, Rect, ThemeColor, Weight};
use claude_sessions_core::store::{ClosedRow, SendKey, SendKind, SendState};
use claude_sessions_core::text;
use std::time::{Duration, SystemTime};

/// The label column of the panel: the group titles, the disclosure row and the hint start here.
const TEXT_X: f64 = 8.0;
const CHEVRON_W: f64 = 9.0;

/// The key hint line under the list while keyboard mode is on.
pub fn key_hint_node(ctx: &mut Ctx, y: f64) -> Node {
    let mut c = Canvas::default();
    c.label(
        Rect::new(TEXT_X, 0.0, CARD_W - 2.0 * TEXT_X, KEY_HINT_H),
        "↑↓ chọn thẻ · ↩ nhảy tới pane · R trả lời · Esc đóng",
        theme::HINT_FONT,
        theme::solid(ThemeColor::TextFaint),
        Align::Left,
    );
    ctx.node("key-hint", Rect::new(PANEL_PAD, y, CARD_W, KEY_HINT_H), c)
}

/// The key of the hover region of the title row (a row id, never a session id).
const TITLE_HOVER: &str = "closed-title";
/// Where the title of a leftover row starts: after its dot.
const DOT_TITLE_X: f64 = 26.0;
/// The right edge of the `N đang chạy · 886 MB` summary; the chevron sits after it, then the kill control.
const SUMMARY_RIGHT: f64 = CARD_W - CARD_PAD_RIGHT - theme::ROW_BUTTON - 8.0 - 14.0;

/// The node of the list and, before it, an empty node per row with processes (and one for the title row): the app finds the
/// hovered row by the `card-<id>` key of a node, and those rows are drawn inside the one `closed` node.
pub fn nodes(ctx: &mut Ctx, y: f64, h: f64) -> Vec<Node> {
    let (snap, panel) = (ctx.view.snap, ctx.view.panel);
    let mut out = vec![ctx.node(
        &super::card::key(TITLE_HOVER),
        Rect::new(PANEL_PAD, y, CARD_W, CLOSED_ROW_H),
        Canvas::default(),
    )];
    if panel.closed_open {
        for (row, (ry, rh)) in snap.closed_rows.iter().zip(closed_rows(snap, panel)) {
            if row.procs.is_some() {
                out.push(ctx.node(
                    &super::card::key(&row.id),
                    Rect::new(PANEL_PAD, y + ry, CARD_W, rh),
                    Canvas::default(),
                ));
            }
        }
    }
    out.push(node(ctx, y, h));
    out
}

fn micros_since_epoch(us: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_micros(us)
}

/// The text after the title of a row: `đóng 12p · folder`, `chạy từ 26h` for a session the widget never saw.
fn row_sub(ctx: &Ctx, row: &ClosedRow) -> String {
    let now = ctx.view.now;
    let mut sub = match (row.closed_at, &row.procs) {
        (Some(at), _) => format!("đóng {}", text::closed_ago(at, now)),
        (None, Some(p)) => format!(
            "chạy từ {}",
            text::closed_ago(micros_since_epoch(p.started_us), now)
        ),
        (None, None) => String::new(),
    };
    let folder = if row.closed_at.is_some() || row.folder != row.title {
        row.folder.as_str()
    } else {
        ""
    };
    if !folder.is_empty() {
        sub.push_str(" · ");
        sub.push_str(folder);
    }
    sub
}

fn title_row(ctx: &Ctx, c: &mut Canvas) {
    let (snap, panel) = (ctx.view.snap, ctx.view.panel);
    let open = panel.closed_open;
    // Same baseline as the group titles.
    let top = CLOSED_ROW_H - 18.0;
    c.symbol(
        if open {
            "chevron.down"
        } else {
            "chevron.right"
        },
        Rect::new(TEXT_X, top + 1.0, CHEVRON_W, 12.0),
        theme::solid(ThemeColor::TextSecondary),
        Weight::Bold,
    );
    let x = TEXT_X + CHEVRON_W + 5.0;
    section::draw_title(ctx, c, x, CLOSED_ROW_H, "Vừa đóng", snap.closed_rows.len());
    let left = snap.leftover_count();
    if left > 0 && !open {
        let used = ctx.text_w("Vừa đóng", &theme::SECTION_FONT)
            + ctx.text_w(&snap.closed_rows.len().to_string(), &theme::SECTION_FONT)
            + 12.0;
        let text = format!("· {left} đang chạy");
        let w = ctx.text_w(&text, &theme::SECTION_FONT).ceil();
        c.text(
            Rect::new(x + used, top, w + 2.0, 14.0),
            &text,
            theme::SECTION_FONT,
            theme::solid(ThemeColor::Orange),
            1,
            Align::Left,
        );
    }
}

/// The summary of a leftover row: `3 đang chạy · 886 MB`, with `CPU cao` in red instead of the bytes.
fn summary(ctx: &Ctx, c: &mut Canvas, y: f64, p: &SessionProcs) -> f64 {
    let lead = format!("{} đang chạy · ", p.row_count());
    let (tail, color) = if p.cpu_high() {
        ("CPU cao".to_string(), ThemeColor::Red)
    } else {
        (format_bytes(p.rss()), ThemeColor::TextFaint)
    };
    let (w1, w2) = (
        ctx.text_w(&lead, &theme::PROC_FONT).ceil(),
        ctx.text_w(&tail, &theme::PROC_FONT).ceil(),
    );
    let x2 = SUMMARY_RIGHT - w2;
    let x1 = x2 - w1;
    c.label(
        Rect::new(x1, y, w1, CLOSED_ITEM_H),
        &lead,
        theme::PROC_FONT,
        theme::solid(ThemeColor::TextSecondary),
        Align::Left,
    );
    c.label(
        Rect::new(x2, y, w2, CLOSED_ITEM_H),
        &tail,
        theme::PROC_FONT,
        theme::solid(color),
        Align::Left,
    );
    x1
}

/// The first line of a row: title, `đóng 12p · folder` (or the resume state), and for a leftover row the dot, the summary and
/// the kill control.
fn row_line(ctx: &Ctx, c: &mut Canvas, row: &ClosedRow, y: f64) {
    let snap = ctx.view.snap;
    let state = snap.send_state(&SendKey::new(&row.id, SendKind::Resume), ctx.view.now);
    let line = Rect::new(0.0, y, CARD_W, CLOSED_ITEM_H);
    let (title_x, right) = match &row.procs {
        Some(p) => {
            c.rrect(
                Rect::new(12.0, y + 12.0, 6.0, 6.0),
                3.0,
                Some(theme::solid(ThemeColor::Orange)),
                None,
            );
            let n = p.row_count();
            let word = if n == 1 {
                "Tắt".to_string()
            } else {
                format!("Tắt {n}")
            };
            let key = format!("all:{}", row.id);
            let hover = proc_row::active(ctx, &row.id);
            let replaced = proc_row::kill_control(ctx, c, y, (&row.id, &key), &word, hover);
            let x1 = if replaced {
                // The control's face covers the summary; keep its room so the title does not jump.
                SUMMARY_RIGHT - 120.0
            } else {
                let x1 = summary(ctx, c, y, p);
                let chevron = Rect::new(SUMMARY_RIGHT + 6.0, y + 9.0, 8.0, 12.0);
                c.symbol(
                    if ctx.view.panel.open_procs.contains(&row.id) {
                        "chevron.down"
                    } else {
                        "chevron.right"
                    },
                    chevron,
                    theme::solid(ThemeColor::TextSecondary),
                    Weight::Bold,
                );
                c.hit(
                    Rect::new(
                        x1 - 6.0,
                        y,
                        chevron.max_x() + 6.0 - (x1 - 6.0),
                        CLOSED_ITEM_H,
                    ),
                    Target::ToggleProcs(row.id.clone()),
                );
                x1
            };
            (DOT_TITLE_X, x1 - 8.0)
        }
        None => (12.0, 0.0),
    };
    let limit = match &row.procs {
        Some(_) => (right - title_x).max(0.0),
        None => CARD_W / 2.0,
    };
    let title_w = ctx.text_w(&row.title, &theme::ROW_FONT).min(limit);
    c.label(
        Rect::new(title_x, y, title_w, CLOSED_ITEM_H),
        &row.title,
        theme::ROW_FONT,
        theme::solid(ThemeColor::Text),
        Align::Left,
    );
    let sub_x = title_x + title_w + 8.0;
    let resume_face = match state {
        Some(SendState::Sending) => Some(("Đang mở…", ThemeColor::TextSecondary)),
        Some(SendState::Failed) => Some(("Lỗi — đã copy lệnh", ThemeColor::Red)),
        _ => None,
    };
    if row.procs.is_some() {
        // The resume state takes the place of `đóng 12p · folder`: the right end belongs to the summary.
        let (label, color) = resume_face.map_or_else(
            || (row_sub(ctx, row), ThemeColor::TextFaint),
            |(l, c)| (l.to_string(), c),
        );
        c.label(
            Rect::new(sub_x, y, (right - sub_x).max(0.0), CLOSED_ITEM_H),
            &label,
            theme::META_FONT,
            theme::solid(color),
            Align::Left,
        );
    } else {
        let mut sub = row.folder.clone();
        if let Some(b) = row.session.branch.as_deref().filter(|b| !b.is_empty()) {
            sub.push_str(" · ");
            sub.push_str(b);
        }
        c.label(
            Rect::new(sub_x, y, CARD_W / 3.0, CLOSED_ITEM_H),
            &sub,
            theme::META_FONT,
            theme::solid(ThemeColor::TextFaint),
            Align::Left,
        );
        let (label, color) = resume_face.map_or_else(
            || {
                let ago = row
                    .closed_at
                    .map_or(String::new(), |at| text::closed_ago(at, ctx.view.now));
                (format!("đóng {ago}"), ThemeColor::TextFaint)
            },
            |(l, c)| (l.to_string(), c),
        );
        c.label(
            Rect::new(CARD_W - 12.0 - 160.0, y, 160.0, CLOSED_ITEM_H),
            &label,
            theme::TIME_FONT,
            theme::solid(color),
            Align::Right,
        );
    }
    if state != Some(SendState::Sending) {
        c.hit(line, Target::Resume(row.id.clone()));
    }
}

pub fn node(ctx: &mut Ctx, y: f64, h: f64) -> Node {
    let snap = ctx.view.snap;
    let panel = ctx.view.panel;
    let mut c = Canvas::default();
    title_row(ctx, &mut c);
    if snap
        .closed_rows
        .iter()
        .filter(|r| r.procs.is_some())
        .count()
        >= 2
    {
        let n = snap.leftover_count();
        proc_row::kill_control(
            ctx,
            &mut c,
            0.0,
            ("", "all"),
            &format!("Tắt cả {n}"),
            proc_row::active(ctx, TITLE_HOVER),
        );
    }
    c.hit(
        Rect::new(0.0, 0.0, CARD_W, CLOSED_ROW_H),
        Target::ToggleClosed,
    );
    if panel.closed_open {
        c.rrect(
            Rect::new(0.0, CLOSED_ROW_H, CARD_W, h - CLOSED_ROW_H),
            theme::GROUP_RADIUS,
            Some(theme::solid(ThemeColor::Group)),
            None,
        );
        for (i, (row, (y, _))) in snap
            .closed_rows
            .iter()
            .zip(closed_rows(snap, panel))
            .enumerate()
        {
            if i > 0 {
                c.hairline(12.0, y, CARD_W - 12.0);
            }
            row_line(ctx, &mut c, row, y);
            if let (Some(p), true) = (&row.procs, panel.open_procs.contains(&row.id)) {
                for (j, g) in p.groups().enumerate() {
                    proc_row::draw(
                        ctx,
                        &mut c,
                        g,
                        y + CLOSED_ITEM_H + j as f64 * CLOSED_CHILD_H,
                        DOT_TITLE_X,
                        &row.id,
                        false,
                    );
                }
            }
        }
    }
    ctx.node("closed", Rect::new(PANEL_PAD, y, CARD_W, h), c)
}
