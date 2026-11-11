//! The question block: per question the prompt, one row per option (numbered badge, label, description), the
//! `Khác…` row (plus its field while open), then `Gửi`, the countdown and `⌘↩`. A sent or fallback question is one note line.
//! Positions follow `layout::question_height`.

use super::{Canvas, Ctx};
use crate::hits::Target;
use crate::theme;
use claude_sessions_core::feed::{self, BlockState, FeedQuestion};
use claude_sessions_core::layout::{
    BLOCK_GAP, BUTTON_H, CARD_INSET, FIELD_H, NOTE_H, OPTION_ROW_H, OTHER_ROW_H, QUESTION_GAP,
    QUESTION_PROMPT_H, TEXT_W, other_open, question_state,
};
use claude_sessions_core::scene::{Align, Field, Rect, ThemeColor, Weight};
use claude_sessions_core::store::{SendKey, SendKind, Session};
use std::collections::HashMap;

/// `block` is the rect the card layout placed for `Block::Question`, relative to the card.
pub fn draw(ctx: &mut Ctx, c: &mut Canvas, s: &Session, block: Rect) {
    let Some(q) = &s.question else {
        return;
    };
    match question_state(s, q) {
        BlockState::Sent => c.label(
            Rect::new(CARD_INSET, block.y + BLOCK_GAP, TEXT_W, NOTE_H),
            "Đã gửi",
            theme::SUB_FONT,
            theme::solid(ThemeColor::Text),
            Align::Left,
        ),
        BlockState::Fallback => {
            let r = Rect::new(
                CARD_INSET,
                block.y + BLOCK_GAP,
                ctx.button_w("Trả lời trong cmux"),
                NOTE_H,
            );
            c.button(
                r,
                "Trả lời trong cmux",
                theme::solid(ThemeColor::Button),
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
    let mut y = block.y + BLOCK_GAP;
    for (i, item) in q.questions.iter().enumerate() {
        if i > 0 {
            y += QUESTION_GAP;
        }
        let mut x = CARD_INSET;
        if let Some(h) = &item.header {
            let w = ctx.text_w(h, &theme::CAPTION_FONT) + 12.0;
            let chip = Rect::new(x, y + 3.0, w, 16.0);
            c.rrect(chip, 8.0, Some(theme::solid(ThemeColor::Button)), None);
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
            Rect::new(x, y, CARD_INSET + TEXT_W - x, QUESTION_PROMPT_H),
            &item.prompt,
            theme::SUB_FONT,
            theme::solid(ThemeColor::Text),
            Align::Left,
        );
        y += QUESTION_PROMPT_H;
        let picked = answers.get(i).map_or(&[][..], Vec::as_slice);
        for (j, o) in item.options.iter().enumerate() {
            let on = picked.contains(&o.label);
            let row = Rect::new(CARD_INSET, y, TEXT_W, OPTION_ROW_H);
            // A picked row is blue-tinted with a blue border and badge; the others are bare rows with a quiet badge.
            if on {
                c.rrect(
                    Rect::new(row.x, y + 1.0, row.w, OPTION_ROW_H - 2.0),
                    7.0,
                    Some(theme::tint(ThemeColor::Blue, 0.20)),
                    Some(theme::tint(ThemeColor::Blue, 0.60)),
                );
            }
            let badge = Rect::new(row.x + 8.0, y + (OPTION_ROW_H - 18.0) / 2.0, 18.0, 18.0);
            c.rrect(
                badge,
                5.0,
                Some(if on {
                    theme::solid(ThemeColor::Blue)
                } else {
                    theme::tint(ThemeColor::Text, 0.10)
                }),
                None,
            );
            if let Some(n) = feed::option_number(q, i, j) {
                c.label(
                    badge,
                    &n.to_string(),
                    theme::CAPTION_FONT,
                    theme::solid(if on {
                        ThemeColor::White
                    } else {
                        ThemeColor::TextSecondary
                    }),
                    Align::Center,
                );
            }
            // Label and description share the one line; the description takes what the label leaves and is cut with `…`.
            let text_x = row.x + 34.0;
            let text_w = row.w - 42.0;
            let label_w = ctx.text_w(&o.label, &theme::BUTTON_FONT).ceil().min(text_w);
            c.text(
                Rect::new(text_x, y, label_w, OPTION_ROW_H),
                &o.label,
                theme::BUTTON_FONT,
                theme::solid(ThemeColor::Text),
                1,
                Align::Left,
            );
            if let Some(d) = &o.description {
                c.tip(row, d);
                let dx = label_w + 10.0;
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
        let other_row = Rect::new(CARD_INSET, y, TEXT_W, OTHER_ROW_H);
        c.symbol(
            "pencil",
            Rect::new(
                other_row.x + 8.0,
                y + (OTHER_ROW_H - 14.0) / 2.0,
                14.0,
                14.0,
            ),
            theme::solid(ThemeColor::TextSecondary),
            Weight::Regular,
        );
        let label = match typed {
            Some(t) if open != Some(i) => format!("Khác: {t}"),
            _ => "Khác…".to_string(),
        };
        c.text(
            Rect::new(other_row.x + 34.0, y, other_row.w - 42.0, OTHER_ROW_H),
            &label,
            theme::BUTTON_FONT,
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
            c.fields.push(Field {
                key: format!("other-{}#{i}", s.id),
                rect: Rect::new(CARD_INSET + 34.0, y, TEXT_W - 34.0, FIELD_H),
                text: others.get(&i).cloned().unwrap_or_default(),
                placeholder: "Khác…".into(),
            });
            y += FIELD_H;
        }
    }

    y += BLOCK_GAP;
    let send = Rect::new(CARD_INSET, y, ctx.button_w("Gửi") - 24.0, BUTTON_H);
    let can = feed::complete(q, answers, others);
    let state = view
        .snap
        .send_state(&SendKey::new(&s.id, SendKind::Answer), view.now);
    let fill = if can {
        theme::solid(ThemeColor::Green)
    } else {
        theme::tint(ThemeColor::Green, 0.35)
    };
    if c.button(send, "Gửi", fill, theme::solid(ThemeColor::OnAccent), state) && can {
        c.hit(send, Target::Answer(s.id.clone()));
    }
    let countdown = feed::countdown(q, view.now);
    let cw = ctx.text_w(&countdown, &theme::META_FONT) + 4.0;
    c.label(
        Rect::new(send.x + send.w + 10.0, y, cw, BUTTON_H),
        &countdown,
        theme::META_FONT,
        theme::solid(ThemeColor::TextSecondary),
        Align::Left,
    );
    c.label(
        Rect::new(send.x + send.w + 18.0 + cw, y, 40.0, BUTTON_H),
        "⌘↩",
        theme::META_FONT,
        theme::solid(ThemeColor::TextFaint),
        Align::Left,
    );
}
