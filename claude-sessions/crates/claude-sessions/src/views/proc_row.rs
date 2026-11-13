//! One process row of an open list: name, `:port` in the quieter grey right after it, the memory (or `CPU 590%` in red)
//! right-aligned, and the kill control at the right end. Rows sit 30 pt apart (`layout::CLOSED_CHILD_H`), indented to the text
//! column of what they belong to; there are no boxes, the list is plain text on the row's own fill.
//!
//! The kill control is one place with four faces: `✕` (only while the owner is hovered or selected), the red armed word
//! (`Tắt`, `Tắt N`, `Tắt cả N`), `Đang tắt…` while the kill runs, `Không tắt được` for 3 s after a failure.

use super::{Canvas, Ctx};
use crate::hits::{Target, is_armed};
use crate::theme;
use claude_sessions_core::layout::{CARD_PAD_RIGHT, CARD_W, CLOSED_CHILD_H};
use claude_sessions_core::procs::{GroupKind, ProcGroup, format_bytes};
use claude_sessions_core::scene::{Align, Rect, ThemeColor, Weight};
use claude_sessions_core::store::{SendKey, SendState};

/// The right edge of the memory column: the control's width and a gap in from the row's padding.
pub const VALUE_RIGHT: f64 = CARD_W - CARD_PAD_RIGHT - theme::ROW_BUTTON - 8.0;
/// Space between a name and its port.
const PORT_GAP: f64 = 8.0;

/// Whether the owner of the rows (a live card or a closed row, by id) is hovered or keyboard-selected: its `✕` shows.
pub fn active(ctx: &Ctx, owner: &str) -> bool {
    let panel = ctx.view.panel;
    panel.hovered_row.as_deref() == Some(owner)
        || (panel.keyboard && panel.selected.as_deref() == Some(owner))
}

/// A filled capsule with `label` centred in it, its right edge at `right` and its top at `y`: the armed faces (`Tắt?`, `Dừng?`)
/// and `Đang tắt…`. Returns its rect.
pub fn capsule(
    ctx: &Ctx,
    c: &mut Canvas,
    (right, y): (f64, f64),
    label: &str,
    fill: ThemeColor,
    text: ThemeColor,
) -> Rect {
    let w = (ctx.text_w(label, &theme::DANGER_FONT) + 18.0).ceil();
    let rect = Rect::new(right - w, y, w, theme::ROW_BUTTON);
    c.capsule(rect, Some(theme::solid(fill)));
    c.label(
        rect,
        label,
        theme::DANGER_FONT,
        theme::solid(text),
        Align::Center,
    );
    rect
}

/// The kill control of the row at `y` (30 pt tall). `session` is the card or closed-row id, `key` what the kill stands for,
/// `word` the armed face, `active` whether the idle `✕` shows. Returns whether the control shows a face that replaces the value beside it (armed, running, failed).
pub fn kill_control(
    ctx: &Ctx,
    c: &mut Canvas,
    y: f64,
    (session, key): (&str, &str),
    word: &str,
    active: bool,
) -> bool {
    let (snap, panel, now) = (ctx.view.snap, ctx.view.panel, ctx.view.now);
    let right = CARD_W - CARD_PAD_RIGHT;
    let btn_y = y + (CLOSED_CHILD_H - theme::ROW_BUTTON) / 2.0;
    let send = snap.send_state(&SendKey::kill(session, key), now);
    let armed = is_armed(&panel.kill_armed, key, now);
    let capsule = |c: &mut Canvas, label: &str, fill: ThemeColor, text: ThemeColor| {
        capsule(ctx, c, (right, btn_y), label, fill, text)
    };
    match send {
        // A kill that went through stays `Đang tắt…` until the next scan no longer lists the process.
        Some(SendState::Sending | SendState::Sent) => {
            capsule(c, "Đang tắt…", ThemeColor::Fill, ThemeColor::TextSecondary);
            true
        }
        Some(SendState::Failed) => {
            let label = "Không tắt được";
            let w = ctx.text_w(label, &theme::DANGER_FONT).ceil();
            let rect = Rect::new(right - w, btn_y, w, theme::ROW_BUTTON);
            c.label(
                rect,
                label,
                theme::DANGER_FONT,
                theme::solid(ThemeColor::Red),
                Align::Right,
            );
            c.hit(
                rect,
                Target::Kill {
                    session: session.to_string(),
                    key: key.to_string(),
                },
            );
            true
        }
        None if armed => {
            let rect = capsule(c, word, ThemeColor::Red, ThemeColor::White);
            c.hit(
                rect,
                Target::Kill {
                    session: session.to_string(),
                    key: key.to_string(),
                },
            );
            true
        }
        None if active => {
            let rect = Rect::new(
                right - theme::ROW_BUTTON,
                btn_y,
                theme::ROW_BUTTON,
                theme::ROW_BUTTON,
            );
            c.capsule(rect, Some(theme::solid(ThemeColor::Fill)));
            c.symbol(
                "xmark",
                Rect::new(rect.x + 6.0, rect.y + 6.0, 8.0, 8.0),
                theme::solid(ThemeColor::TextSecondary),
                Weight::Bold,
            );
            c.tip(rect, "Tắt (bấm hai lần)");
            c.hit(
                rect,
                Target::Kill {
                    session: session.to_string(),
                    key: key.to_string(),
                },
            );
            false
        }
        None => false,
    }
}

/// The row of `group` at `y`, its text starting at `x_indent`; `owner` is the card or closed-row id its kill control belongs
/// to. `read_only` draws no control and dims the name (the `Claude và MCP` row of a live session).
pub fn draw(
    ctx: &Ctx,
    c: &mut Canvas,
    group: &ProcGroup,
    y: f64,
    x_indent: f64,
    owner: &str,
    read_only: bool,
) {
    let replaced =
        !read_only && kill_control(ctx, c, y, (owner, &group.key), "Tắt", active(ctx, owner));
    let dim = group.kind == GroupKind::ClaudeAndMcp && read_only;
    let value_w = if replaced {
        0.0
    } else {
        let (text, color) = match group.cpu_percent {
            Some(p) => (format!("CPU {p:.0}%"), ThemeColor::Red),
            None => (format_bytes(group.rss), ThemeColor::TextFaint),
        };
        let w = ctx.text_w(&text, &theme::PROC_FONT).ceil();
        c.label(
            Rect::new(VALUE_RIGHT - w, y, w, CLOSED_CHILD_H),
            &text,
            theme::PROC_FONT,
            theme::solid(color),
            Align::Right,
        );
        w + 12.0
    };
    let limit = (VALUE_RIGHT - value_w - x_indent).max(0.0);
    let name_w = ctx.text_w(&group.name, &theme::PROC_FONT).ceil().min(limit);
    c.label(
        Rect::new(x_indent, y, name_w, CLOSED_CHILD_H),
        &group.name,
        theme::PROC_FONT,
        theme::solid(if dim {
            ThemeColor::TextSecondary
        } else {
            ThemeColor::Text
        }),
        Align::Left,
    );
    if !group.ports.is_empty() {
        let ports: Vec<String> = group.ports.iter().map(|p| format!(":{p}")).collect();
        let x = x_indent + name_w + PORT_GAP;
        c.label(
            Rect::new(x, y, (x_indent + limit - x).max(0.0), CLOSED_CHILD_H),
            &ports.join(" "),
            theme::PROC_FONT,
            theme::solid(ThemeColor::TextFaint),
            Align::Left,
        );
    }
    c.tip(Rect::new(0.0, y, CARD_W, CLOSED_CHILD_H), &group.command);
}
