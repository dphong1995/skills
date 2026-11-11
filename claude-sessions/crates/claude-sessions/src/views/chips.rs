//! Ticket chips with the menu count (`Block::Chips`, `layout::CHIPS_H`) and the inline link list (`Block::LinkList`,
//! `layout::link_list_height`). The Jira icon is a tinted placeholder until `DrawCmd` can draw an image.

use super::{Canvas, Ctx};
use crate::hits::Target;
use crate::theme;
use claude_sessions_core::credentials::Service;
use claude_sessions_core::layout::{CARD_INSET, LINK_ROW_H, LINK_SECTION_H, TEXT_W};
use claude_sessions_core::scene::{Align, Rect, ThemeColor, Weight};
use claude_sessions_core::store::{LinkKind, Session};

const CHIP_H: f64 = 22.0;

pub fn draw_chips(ctx: &mut Ctx, c: &mut Canvas, s: &Session, block: Rect) {
    let view = ctx.view;
    let y = block.y + (block.h - CHIP_H) / 2.0;
    let mut x = CARD_INSET;
    for link in &s.links {
        let icon = if link.icon_url.is_some() { 18.0 } else { 0.0 };
        let w = ctx.text_w(&link.label, &theme::META_FONT) + 16.0 + icon;
        let chip = Rect::new(x, y, w, CHIP_H);
        c.rrect(
            chip,
            theme::CHIP_RADIUS,
            Some(theme::solid(ThemeColor::Button)),
            None,
        );
        if link.icon_url.is_some() {
            c.rrect(
                Rect::new(x + 7.0, y + 5.0, 12.0, 12.0),
                3.0,
                Some(theme::tint(theme::tone_token(link.tone), 0.6)),
                None,
            );
        }
        c.label(
            Rect::new(x + 8.0 + icon, y, w - 16.0 - icon, CHIP_H),
            &link.label,
            theme::META_FONT,
            theme::solid(ThemeColor::Text),
            Align::Left,
        );
        c.hit(
            chip,
            Target::Link {
                session: s.id.clone(),
                url: link.url.clone(),
            },
        );
        x += w + 6.0;
    }
    // The menu count; purple while a PR it does not list yet is new.
    let open = view.panel.open_menus.contains(&s.id);
    let hidden_mark = view
        .snap
        .unseen
        .get(&s.id)
        .is_some_and(|m| m.prs.iter().any(|r| !s.prs.iter().any(|p| &p.ref_ == r)));
    let count = s.menu_count().to_string();
    let w = ctx.text_w(&count, &theme::META_FONT) + 34.0;
    let chip = Rect::new(x, y, w, CHIP_H);
    let fill = if open {
        theme::tint(ThemeColor::Text, 0.22)
    } else if hidden_mark {
        theme::tint(ThemeColor::Purple, 0.35)
    } else {
        theme::solid(ThemeColor::Button)
    };
    c.rrect(chip, theme::CHIP_RADIUS, Some(fill), None);
    c.symbol(
        "list.bullet",
        Rect::new(x + 8.0, y + 5.0, 12.0, 12.0),
        theme::solid(ThemeColor::TextSecondary),
        Weight::Medium,
    );
    c.label(
        Rect::new(x + 24.0, y, w - 28.0, CHIP_H),
        &count,
        theme::META_FONT,
        theme::solid(ThemeColor::Text),
        Align::Left,
    );
    c.hit(chip, Target::Menu(s.id.clone()));
}

pub fn draw_link_list(ctx: &mut Ctx, c: &mut Canvas, s: &Session, block: Rect) {
    let mut y = block.y;
    for section in &s.menu {
        c.text(
            Rect::new(CARD_INSET + 4.0, y + 2.0, TEXT_W, 14.0),
            &section.title,
            theme::CAPTION_FONT,
            theme::solid(ThemeColor::TextFaint),
            1,
            Align::Left,
        );
        y += LINK_SECTION_H;
        for item in &section.items {
            let row = Rect::new(CARD_INSET, y, TEXT_W, LINK_ROW_H);
            let icon = Rect::new(row.x + 4.0, y + 4.0, 14.0, 14.0);
            match item.kind {
                LinkKind::Ticket => {
                    if item.icon_url.is_some() {
                        c.rrect(
                            Rect::new(icon.x + 1.0, icon.y + 1.0, 12.0, 12.0),
                            3.0,
                            Some(theme::tint(theme::tone_token(item.tone), 0.6)),
                            None,
                        );
                    }
                }
                LinkKind::Pr => c.symbol(
                    "arrow.triangle.branch",
                    icon,
                    theme::solid(ThemeColor::TextSecondary),
                    Weight::Regular,
                ),
                LinkKind::Link => c.symbol(
                    "link",
                    icon,
                    theme::solid(ThemeColor::TextSecondary),
                    Weight::Regular,
                ),
            }
            let code_w = ctx.text_w(&item.code, &theme::META_MONO_FONT) + 2.0;
            c.label(
                Rect::new(row.x + 24.0, y, code_w, LINK_ROW_H),
                &item.code,
                theme::META_MONO_FONT,
                theme::solid(ThemeColor::TextFaint),
                Align::Left,
            );
            let status_w = item
                .status
                .as_deref()
                .map_or(0.0, |t| ctx.text_w(t, &theme::STATE_FONT) + 16.0);
            if let Some(title) = &item.title {
                let tx = row.x + 24.0 + code_w + 6.0;
                c.label(
                    Rect::new(
                        tx,
                        y,
                        (row.x + row.w - status_w - 12.0 - tx).max(0.0),
                        LINK_ROW_H,
                    ),
                    title,
                    theme::META_FONT,
                    theme::solid(ThemeColor::Text),
                    Align::Left,
                );
            }
            if let Some(status) = &item.status {
                let color = theme::tone_token(item.tone);
                let tag = Rect::new(row.x + row.w - status_w - 4.0, y + 3.0, status_w, 16.0);
                c.rrect(tag, 4.0, Some(theme::tint(color, 0.2)), None);
                c.label(
                    tag,
                    status,
                    theme::STATE_FONT,
                    theme::solid(color),
                    Align::Center,
                );
            }
            c.hit(
                row,
                Target::Link {
                    session: s.id.clone(),
                    url: item.url.clone(),
                },
            );
            y += LINK_ROW_H;
        }
    }
    if s.menu_needs_jira {
        let row = Rect::new(CARD_INSET, y, TEXT_W, LINK_ROW_H);
        c.rrect(
            Rect::new(
                row.x,
                y + 2.0,
                ctx.button_w("Kết nối Jira…"),
                LINK_ROW_H - 4.0,
            ),
            theme::BUTTON_RADIUS,
            Some(theme::solid(ThemeColor::Button)),
            None,
        );
        c.label(
            Rect::new(
                row.x,
                y + 2.0,
                ctx.button_w("Kết nối Jira…"),
                LINK_ROW_H - 4.0,
            ),
            "Kết nối Jira…",
            theme::BUTTON_FONT,
            theme::solid(ThemeColor::Text),
            Align::Center,
        );
        c.hit(
            Rect::new(row.x, y, ctx.button_w("Kết nối Jira…"), LINK_ROW_H),
            Target::Connect(Service::Jira),
        );
    }
}
