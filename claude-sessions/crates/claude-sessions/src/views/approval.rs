//! The approval block: the command box, `Cho phép` / `Từ chối…` and, while denying, the reason field with its `Gửi`. A long
//! command shows `Xem đủ` instead of the buttons, a too-long one and an unclear pending a line that jumps to cmux (the
//! buttons only ever sit under a command shown whole). The look comes from `layout::approval_look`; positions follow
//! `ApprovalLook::height`. The buttons wear their send state (`Canvas::button`); once the approval is
//! sent the card has no pending command any more and the block is gone with it.

use super::{Canvas, Ctx};
use crate::hits::Target;
use crate::theme;
use claude_sessions_core::layout::{
    ApprovalLook, ApprovalRow, BLOCK_GAP, BUTTON_H, CARD_INSET, CMD_LINE_H, CMD_MARGIN, CMD_PAD,
    FIELD_H, NOTE_H, TEXT_W, approval_look, command_box_height,
};
use claude_sessions_core::panel_state::EditKind;
use claude_sessions_core::scene::{Align, Field, Rect, ThemeColor};
use claude_sessions_core::store::{SendKey, SendKind, Session};

/// `block` is the rect the card layout placed for `Block::Approval`, relative to the card.
pub fn draw(ctx: &mut Ctx, c: &mut Canvas, s: &Session, block: Rect) {
    let Some(pending) = s.pending.as_ref() else {
        return;
    };
    let view = ctx.view;
    let jump = |ctx: &mut Ctx, c: &mut Canvas, y: f64, label: &str| {
        let r = Rect::new(CARD_INSET, y, ctx.button_w(label), NOTE_H);
        c.button(
            r,
            label,
            theme::solid(ThemeColor::Button),
            theme::solid(ThemeColor::Text),
            None,
        );
        c.hit(r, Target::Jump(s.id.clone()));
    };
    let (lines, row) = match approval_look(pending, view.snap, view.measure) {
        ApprovalLook::Unclear => {
            jump(ctx, c, block.y + BLOCK_GAP, "Cần duyệt trong cmux");
            return;
        }
        ApprovalLook::Command { lines, row } => (lines, row),
    };
    let Some(p) = s.approvable() else {
        return;
    };
    let cmd = Rect::new(
        CARD_INSET,
        block.y + CMD_MARGIN,
        TEXT_W,
        command_box_height(lines),
    );
    c.rrect(
        cmd,
        theme::CMD_RADIUS,
        Some(theme::solid(ThemeColor::Code)),
        None,
    );
    c.text(
        Rect::new(
            cmd.x + 10.0,
            cmd.y + CMD_PAD,
            cmd.w - 20.0,
            CMD_LINE_H * lines as f64,
        ),
        &p.text,
        theme::MONO_FONT,
        theme::solid(ThemeColor::Text),
        lines as u32,
        Align::Left,
    );

    let send = |kind| {
        let mut key = SendKey::new(&s.id, kind);
        key.detail = Some(p.tool_id.clone());
        view.snap.send_state(&key, view.now)
    };
    let row_y = cmd.y + cmd.h + BLOCK_GAP;
    match row {
        ApprovalRow::Buttons => {}
        ApprovalRow::JumpTooLong => {
            jump(ctx, c, row_y, "Lệnh dài, bấm để duyệt trong cmux");
            return;
        }
        ApprovalRow::ShowFull => {
            let r = Rect::new(CARD_INSET, row_y, ctx.button_w("Xem đủ") + 22.0, BUTTON_H);
            c.button(
                r,
                "Xem đủ",
                theme::solid(ThemeColor::Button),
                theme::solid(ThemeColor::Text),
                None,
            );
            c.hit(
                r,
                Target::ShowCommand {
                    session: s.id.clone(),
                    tool_id: p.tool_id.clone(),
                },
            );
            return;
        }
    }
    let approve_w = ctx.button_w("Cho phép") + 22.0;
    let approve = Rect::new(CARD_INSET, row_y, approve_w, BUTTON_H);
    let deny = Rect::new(
        approve.x + approve.w + 8.0,
        row_y,
        ctx.button_w("Từ chối…") + 22.0,
        BUTTON_H,
    );
    let approve_state = send(SendKind::Approve);
    let deny_state = send(SendKind::Deny);
    let target = |t: fn(String, String) -> Target| t(s.id.clone(), p.tool_id.clone());
    if c.button(
        approve,
        "Cho phép",
        theme::solid(ThemeColor::Green),
        theme::solid(ThemeColor::OnAccent),
        approve_state,
    ) {
        c.hit(
            approve,
            target(|session, tool_id| Target::Approve { session, tool_id }),
        );
    }
    if c.button(
        deny,
        "Từ chối…",
        theme::solid(ThemeColor::Button),
        theme::solid(ThemeColor::Text),
        deny_state,
    ) {
        c.hit(
            deny,
            target(|session, tool_id| Target::Deny { session, tool_id }),
        );
    }
    if approve_state.is_none() {
        key_hint(c, approve, "A", ThemeColor::OnAccent);
    }
    if deny_state.is_none() {
        key_hint(c, deny, "D", ThemeColor::TextFaint);
    }

    let denying = view
        .panel
        .editing
        .as_ref()
        .is_some_and(|e| e.id == s.id && e.kind == EditKind::Deny);
    if denying {
        let y = row_y + BUTTON_H + BLOCK_GAP;
        let send_btn = Rect::new(
            CARD_INSET + TEXT_W - 64.0,
            y + (FIELD_H - BUTTON_H) / 2.0,
            64.0,
            BUTTON_H,
        );
        c.fields.push(Field {
            key: format!("deny-{}", s.id),
            rect: Rect::new(CARD_INSET, y, send_btn.x - CARD_INSET - 8.0, FIELD_H),
            text: String::new(),
            placeholder: "Lý do (để trống = chỉ từ chối)".into(),
        });
        if c.button(
            send_btn,
            "Gửi",
            theme::solid(ThemeColor::Button),
            theme::solid(ThemeColor::Text),
            deny_state,
        ) {
            c.hit(
                send_btn,
                target(|session, tool_id| Target::DenySend { session, tool_id }),
            );
        }
    }
}

/// The keyboard letter at the right end of a button, as in the mockup.
fn key_hint(c: &mut Canvas, button: Rect, letter: &str, color: ThemeColor) {
    let r = Rect::new(button.x + button.w - 24.0, button.y + 5.0, 14.0, 14.0);
    c.rrect(r, 3.0, None, Some(theme::tint(color, 0.5)));
    c.label(
        r,
        letter,
        theme::font(9.0, claude_sessions_core::scene::Weight::Semibold),
        theme::tint(color, 0.8),
        Align::Center,
    );
}
