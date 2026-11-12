//! The question block: per question the prompt, one row per option (radio, label, description, number key), the `Khác…` row
//! (plus its field while open), then the countdown with `⌘↩` at the left and the primary `Gửi` at the right. A sent or fallback
//! question is one note line.
//! Positions follow `layout::question_height`.

use super::{Canvas, Ctx};
use crate::hits::Target;
use crate::theme;
use claude_sessions_core::feed::{self, BlockState, FeedQuestion};
use claude_sessions_core::layout::{
    BLOCK_GAP, BODY_GAP, BUTTON_H, CARD_INSET, FIELD_H, NOTE_H, OPTION_ROW_H, OTHER_FIELD_H,
    OTHER_ROW_H, QUESTION_GAP, QUESTION_PROMPT_H, TEXT_W, other_open, question_state,
};
use claude_sessions_core::scene::{Align, Rect, ThemeColor};
use claude_sessions_core::store::{SendKey, SendKind, Session};
use std::collections::HashMap;

/// `block` is the rect the card layout placed for `Block::Question`, relative to the card.
pub fn draw(ctx: &mut Ctx, c: &mut Canvas, s: &Session, block: Rect) {
    let Some(q) = &s.question else {
        return;
    };
    match question_state(s, q) {
        BlockState::Sent => c.label(
            Rect::new(CARD_INSET, block.y + BODY_GAP, TEXT_W, NOTE_H),
            "Đã gửi",
            theme::SUB_FONT,
            theme::solid(ThemeColor::Text),
            Align::Left,
        ),
        BlockState::Fallback => {
            let r = Rect::new(
                CARD_INSET,
                block.y + BODY_GAP,
                ctx.button_w("Trả lời trong cmux"),
                NOTE_H,
            );
            c.button(
                r,
                "Trả lời trong cmux",
                theme::solid(ThemeColor::Fill),
                theme::solid(ThemeColor::Text),
                None,
            );
            c.hit(r, Target::Jump(s.id.clone()));
        }
        BlockState::Form => form(ctx, c, s, q, block),
    }
}

fn form(ctx: &mut Ctx, c: &mut Canvas, s: &Session, q: &FeedQuestion, block: Rect) {
    let view = ctx.view;
    let answers = view
        .panel
        .answers
        .get(&q.request_id)
        .map_or(&[][..], Vec::as_slice);
    let empty = HashMap::new();
    let others = view.panel.others.get(&q.request_id).unwrap_or(&empty);
    let open = other_open(s, view.panel);
    let mut y = block.y + BODY_GAP;
    for (i, item) in q.questions.iter().enumerate() {
        if i > 0 {
            y += QUESTION_GAP;
        }
        let mut x = CARD_INSET;
        // The prompt is one 17 pt line with 4 pt under it.
        let line = QUESTION_PROMPT_H - 4.0;
        if let Some(h) = &item.header {
            let w = ctx.text_w(h, &theme::CAPTION_FONT) + 12.0;
            let chip = Rect::new(x, y + 1.0, w, 16.0);
            c.rrect(chip, 8.0, Some(theme::solid(ThemeColor::Fill)), None);
            c.label(
                chip,
                h,
                theme::CAPTION_FONT,
                theme::solid(ThemeColor::TextSecondary),
                Align::Center,
            );
            x += w + 6.0;
        }
        c.label(
            Rect::new(x, y, CARD_INSET + TEXT_W - x, line),
            &item.prompt,
            theme::REPLY_FONT,
            theme::solid(ThemeColor::Text),
            Align::Left,
        );
        y += QUESTION_PROMPT_H;
        let picked = answers.get(i).map_or(&[][..], Vec::as_slice);
        // Rows reach 6 pt past the text column on both sides, as in a list.
        let row_x = CARD_INSET - 6.0;
        let row_w = TEXT_W + 12.0;
        for (j, o) in item.options.iter().enumerate() {
            let on = picked.contains(&o.label);
            let row = Rect::new(row_x, y, row_w, OPTION_ROW_H);
            radio(
                c,
                Rect::new(row.x + 8.0, y + (OPTION_ROW_H - 14.0) / 2.0, 14.0, 14.0),
                on,
            );
            // Number key at the right, then label and description sharing the line; the description takes what the label
            // leaves and is cut with `…`.
            let key_w = 14.0;
            if let Some(n) = feed::option_number(q, i, j) {
                c.label(
                    Rect::new(row.x + row.w - 8.0 - key_w, y, key_w, OPTION_ROW_H),
                    &n.to_string(),
                    theme::CAPTION_FONT,
                    theme::solid(ThemeColor::TextFaint),
                    Align::Right,
                );
            }
            let text_x = row.x + 31.0;
            let text_w = row.w - 31.0 - 8.0 - key_w - 8.0;
            let label_w = ctx.text_w(&o.label, &theme::ROW_FONT).ceil().min(text_w);
            c.text(
                Rect::new(text_x, y, label_w, OPTION_ROW_H),
                &o.label,
                theme::ROW_FONT,
                theme::solid(ThemeColor::Text),
                1,
                Align::Left,
            );
            if let Some(d) = &o.description {
                c.tip(row, d);
                let dx = label_w + 8.0;
                if dx < text_w {
                    c.text(
                        Rect::new(text_x + dx, y, text_w - dx, OPTION_ROW_H),
                        d,
                        theme::META_FONT,
                        theme::solid(ThemeColor::TextSecondary),
                        1,
                        Align::Left,
                    );
                }
            }
            c.hit(
                row,
                Target::PickOption {
                    session: s.id.clone(),
                    question: i,
                    label: o.label.clone(),
                },
            );
            y += OPTION_ROW_H;
        }
        let typed = others.get(&i).map(|t| t.trim()).filter(|t| !t.is_empty());
        let other_row = Rect::new(row_x, y, row_w, OTHER_ROW_H);
        radio(
            c,
            Rect::new(
                other_row.x + 8.0,
                y + (OTHER_ROW_H - 14.0) / 2.0,
                14.0,
                14.0,
            ),
            typed.is_some(),
        );
        let label = match typed {
            Some(t) if open != Some(i) => format!("Khác: {t}"),
            _ => "Khác…".to_string(),
        };
        c.text(
            Rect::new(other_row.x + 31.0, y, other_row.w - 31.0 - 8.0, OTHER_ROW_H),
            &label,
            theme::ROW_FONT,
            theme::solid(ThemeColor::Text),
            1,
            Align::Left,
        );
        c.hit(
            other_row,
            Target::OpenOther {
                session: s.id.clone(),
                question: i,
            },
        );
        y += OTHER_ROW_H;
        if open == Some(i) {
            c.field(
                format!("other-{}#{i}", s.id),
                Rect::new(CARD_INSET, y + 2.0, TEXT_W, FIELD_H),
                "Khác…",
                others.get(&i).cloned().unwrap_or_default(),
                false,
            );
            y += OTHER_FIELD_H;
        }
    }

    y += BLOCK_GAP;
    let send_w = ctx.button_w("Gửi") - 24.0;
    let send = Rect::new(CARD_INSET + TEXT_W - send_w, y, send_w, BUTTON_H);
    let can = feed::complete(q, answers, others);
    let state = view
        .snap
        .send_state(&SendKey::new(&s.id, SendKind::Answer), view.now);
    let fill = if can {
        theme::solid(ThemeColor::Accent)
    } else {
        theme::tint(ThemeColor::Accent, 0.35)
    };
    if c.button(send, "Gửi", fill, theme::solid(ThemeColor::White), state) && can {
        c.hit(send, Target::Answer(s.id.clone()));
    }
    let countdown = feed::countdown(q, view.now);
    // Measured from the countdown with every digit as `0`, so `⌘↩` stays put while the digits tick.
    let template = countdown.replace(|c: char| c.is_ascii_digit(), "0");
    let cw = ctx.text_w(&template, &theme::HINT_FONT) + 4.0;
    c.label(
        Rect::new(CARD_INSET, y, cw, BUTTON_H),
        &countdown,
        theme::HINT_FONT,
        theme::solid(ThemeColor::TextFaint),
        Align::Left,
    );
    c.label(
        Rect::new(CARD_INSET + cw + 8.0, y, 40.0, BUTTON_H),
        "⌘↩",
        theme::HINT_FONT,
        theme::solid(ThemeColor::TextFaint),
        Align::Left,
    );
}

/// A 14 pt radio: a faint ring, or the `Accent` disc with a white dot once chosen.
fn radio(c: &mut Canvas, rect: Rect, on: bool) {
    if on {
        c.rrect(rect, 7.0, Some(theme::solid(ThemeColor::Accent)), None);
        c.rrect(
            Rect::new(rect.x + 4.5, rect.y + 4.5, 5.0, 5.0),
            2.5,
            Some(theme::solid(ThemeColor::White)),
            None,
        );
    } else {
        c.rrect(rect, 7.0, None, Some(theme::solid(ThemeColor::TextFaint)));
    }
}
