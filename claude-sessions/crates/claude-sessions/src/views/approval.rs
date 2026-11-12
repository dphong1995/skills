//! The approval block: the command box, `Từ chối…` and the primary `Cho phép` at the right and, while denying, the round reason
//! field with its send button. A long
//! command shows `Xem đủ` instead of the buttons, a too-long one and an unclear pending a line that jumps to cmux (the
//! buttons only ever sit under a command shown whole). The look is the one `card_layout` computed; positions follow
//! `ApprovalLook::height`. The buttons wear their send state (`Canvas::button`); once the approval is
//! sent the card has no pending command any more and the block is gone with it.

use super::{Canvas, Ctx};
use crate::hits::Target;
use crate::theme;
use claude_sessions_core::layout::{
    ApprovalLook, ApprovalRow, BLOCK_GAP, BODY_GAP, BUTTON_H, CARD_INSET, CMD_FONT, CMD_LINE_H,
    CMD_MARGIN, CMD_PAD, FIELD_H, NOTE_H, TEXT_W, command_box_height,
};
use claude_sessions_core::panel_state::EditKind;
use claude_sessions_core::scene::{Align, Rect, ThemeColor};
use claude_sessions_core::store::{SendKey, SendKind, Session};

/// `block` is the rect the card layout placed for `Block::Approval`, relative to the card.
pub fn draw(ctx: &mut Ctx, c: &mut Canvas, s: &Session, block: Rect, look: Option<ApprovalLook>) {
    if s.pending.is_none() {
        return;
    }
    let view = ctx.view;
    // The button wears the jump's send state; a jump in flight takes no second click.
    let jump = |ctx: &mut Ctx, c: &mut Canvas, y: f64, label: &str| {
        let r = Rect::new(CARD_INSET, y, ctx.button_w(label), NOTE_H);
        let state = view
            .snap
            .send_state(&SendKey::new(&s.id, SendKind::Jump), view.now);
        if c.button(
            r,
            label,
            theme::solid(ThemeColor::Fill),
            theme::solid(ThemeColor::Text),
            state,
        ) {
            c.hit(r, Target::Jump(s.id.clone()));
        }
    };
    let (lines, row) = match look {
        None => return,
        Some(ApprovalLook::Unclear) => {
            jump(ctx, c, block.y + BODY_GAP, "Cần duyệt trong cmux");
            return;
        }
        Some(ApprovalLook::Command { lines, row }) => (lines, row),
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
        CMD_FONT,
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
                theme::solid(ThemeColor::Fill),
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
    // Right-aligned: `Từ chối…` then the primary `Cho phép` at the end. Hits are added approve first.
    let approve_w = ctx.button_w("Cho phép") + 22.0;
    let approve = Rect::new(CARD_INSET + TEXT_W - approve_w, row_y, approve_w, BUTTON_H);
    let deny_w = ctx.button_w("Từ chối…") + 22.0;
    let deny = Rect::new(approve.x - 6.0 - deny_w, row_y, deny_w, BUTTON_H);
    let approve_state = send(SendKind::Approve);
    let deny_state = send(SendKind::Deny);
    let target = |t: fn(String, String) -> Target| t(s.id.clone(), p.tool_id.clone());
    if c.keyed_button(
        approve,
        "Cho phép",
        theme::solid(ThemeColor::Accent),
        theme::solid(ThemeColor::White),
        approve_state,
        Some("A"),
    ) {
        c.hit(
            approve,
            target(|session, tool_id| Target::Approve { session, tool_id }),
        );
    }
    if c.keyed_button(
        deny,
        "Từ chối…",
        theme::solid(ThemeColor::Fill),
        theme::solid(ThemeColor::Text),
        deny_state,
        Some("D"),
    ) {
        c.hit(
            deny,
            target(|session, tool_id| Target::Deny { session, tool_id }),
        );
    }

    let denying = view
        .panel
        .editing
        .as_ref()
        .is_some_and(|e| e.id == s.id && e.kind == EditKind::Deny);
    if denying {
        let capsule = Rect::new(CARD_INSET, row_y + BUTTON_H + BLOCK_GAP, TEXT_W, FIELD_H);
        c.field(
            format!("deny-{}", s.id),
            capsule,
            "Lý do (để trống = chỉ từ chối)",
            String::new(),
            true,
        );
        if let Some(btn) = c.round_send(capsule, deny_state) {
            c.hit(
                btn,
                target(|session, tool_id| Target::DenySend { session, tool_id }),
            );
        }
    }
}
