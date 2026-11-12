//! Ticket chips with the menu count (`Block::Chips`, `layout::CHIPS_H`) and the inline link list (`Block::LinkList`,
//! `layout::link_list_height`): per section a header and an inset list of 38 pt two-line items, the chips hidden while it is
//! open. The Jira icon is the fetched picture, a tinted placeholder until its bytes are in the snapshot.

use super::pr_row::{BADGE, TEXT_X, inset_list, state_badge};
use super::{Canvas, Ctx};
use crate::hits::Target;
use crate::theme;
use claude_sessions_core::credentials::Service;
use claude_sessions_core::layout::{
    BLOCK_GAP, CARD_INSET, CHIP_H, LINK_ITEM_H, LINK_ROW_H, LINK_SECTION_H, TEXT_W,
};
use claude_sessions_core::scene::{Align, Rect, ThemeColor, Weight};
use claude_sessions_core::store::{ItemTone, LinkItem, LinkKind, Session, SessionLink, Snapshot};

/// The Jira icon of `url` when the snapshot holds its bytes, else a tinted square.
fn icon_or_placeholder(
    snap: &Snapshot,
    c: &mut Canvas,
    rect: Rect,
    url: &url::Url,
    tone: Option<ItemTone>,
) {
    if snap.issue_icons.contains_key(url) {
        c.image(rect, super::icon_key(url));
    } else {
        c.rrect(
            rect,
            3.0,
            Some(theme::tint(theme::tone_token(tone), 0.6)),
            None,
        );
    }
}

pub fn draw_chips(ctx: &mut Ctx, c: &mut Canvas, s: &Session, block: Rect) {
    let view = ctx.view;
    let y = block.y + BLOCK_GAP;
    let mut x = CARD_INSET;
    let open = view.panel.open_menus.contains(&s.id);
    // While the list is open it shows every ticket, so the chips give way to the one button that closes it.
    let links: &[SessionLink] = if open { &[] } else { &s.links };
    let count = if open {
        format!("Ẩn danh sách · {}", s.menu_count())
    } else {
        s.menu_count().to_string()
    };
    let menu_w = ctx.text_w(&count, &theme::META_FONT) + 34.0;
    let widths: Vec<f64> = links
        .iter()
        .map(|l| {
            let icon = if l.icon_url.is_some() { 18.0 } else { 0.0 };
            ctx.text_w(&l.label, &theme::META_FONT) + 16.0 + icon
        })
        .collect();
    // Chips that do not fit before the menu chip collapse into a `+N` chip.
    let right = CARD_INSET + TEXT_W;
    let shown = (0..=widths.len())
        .rev()
        .find(|&k| {
            let more = if k < widths.len() {
                ctx.text_w(&format!("+{}", widths.len() - k), &theme::META_FONT) + 22.0
            } else {
                -6.0
            };
            let used: f64 = widths[..k].iter().map(|w| w + 6.0).sum();
            CARD_INSET + used + more + 6.0 + menu_w <= right
        })
        .unwrap_or(0);
    for (link, &w) in links.iter().zip(&widths).take(shown) {
        let icon = if link.icon_url.is_some() { 18.0 } else { 0.0 };
        let chip = Rect::new(x, y, w, CHIP_H);
        c.rrect(
            chip,
            theme::CHIP_RADIUS,
            Some(theme::solid(ThemeColor::Fill)),
            None,
        );
        if let Some(u) = &link.icon_url {
            icon_or_placeholder(
                view.snap,
                c,
                Rect::new(x + 7.0, y + 4.0, 12.0, 12.0),
                u,
                link.tone,
            );
        }
        c.tip(chip, link.help.as_deref().unwrap_or_default());
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
    if shown < widths.len() {
        let label = format!("+{}", widths.len() - shown);
        let w = ctx.text_w(&label, &theme::META_FONT) + 22.0;
        let chip = Rect::new(x, y, w, CHIP_H);
        c.rrect(
            chip,
            theme::CHIP_RADIUS,
            Some(theme::solid(ThemeColor::Fill)),
            None,
        );
        c.label(
            chip,
            &label,
            theme::META_FONT,
            theme::solid(ThemeColor::TextSecondary),
            Align::Center,
        );
        c.tip(chip, "Xem đủ trong danh sách");
        c.hit(chip, Target::Menu(s.id.clone()));
        x += w + 6.0;
    }
    // The menu count; indigo while a PR it does not list yet is new.
    let hidden_mark = view
        .snap
        .unseen
        .get(&s.id)
        .is_some_and(|m| m.prs.iter().any(|r| !s.prs.iter().any(|p| &p.ref_ == r)));
    let w = menu_w;
    let chip = Rect::new(x, y, w, CHIP_H);
    let fill = if open {
        theme::tint(ThemeColor::Text, 0.22)
    } else if hidden_mark {
        theme::tint(ThemeColor::Indigo, 0.35)
    } else {
        theme::solid(ThemeColor::Fill)
    };
    c.rrect(chip, theme::CHIP_RADIUS, Some(fill), None);
    c.symbol(
        "list.bullet",
        Rect::new(x + 8.0, y + 4.0, 12.0, 12.0),
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
    let ctx = &*ctx;
    let mut y = block.y;
    for section in &s.menu {
        // The header band: 8 pt gap, 14 pt of text, 4 pt gap to the list.
        let title_w = ctx.text_w(&section.title, &theme::SECTION_FONT);
        let head = Rect::new(CARD_INSET + 4.0, y + 8.0, TEXT_W - 4.0, 14.0);
        c.label(
            Rect::new(head.x, head.y, title_w, head.h),
            &section.title,
            theme::SECTION_FONT,
            theme::solid(ThemeColor::TextSecondary),
            Align::Left,
        );
        c.label(
            Rect::new(head.x + title_w + 4.0, head.y, 40.0, head.h),
            &section.items.len().to_string(),
            theme::CAPTION_FONT,
            theme::solid(ThemeColor::TextFaint),
            Align::Left,
        );
        let list = Rect::new(
            CARD_INSET,
            y + LINK_SECTION_H,
            TEXT_W,
            section.items.len() as f64 * LINK_ITEM_H,
        );
        inset_list(c, list, |c| {
            for (i, item) in section.items.iter().enumerate() {
                let it = Rect::new(list.x, list.y + i as f64 * LINK_ITEM_H, list.w, LINK_ITEM_H);
                link_item(ctx, c, s, item, it, i == 0);
            }
        });
        y += LINK_SECTION_H + list.h;
    }
    if s.menu_needs_jira {
        y += BLOCK_GAP;
        let row = Rect::new(CARD_INSET, y, TEXT_W, LINK_ROW_H);
        let button = Rect::new(
            row.x,
            y + 2.0,
            ctx.button_w("Kết nối Jira…"),
            LINK_ROW_H - 4.0,
        );
        c.capsule(button, Some(theme::solid(ThemeColor::Fill)));
        c.label(
            button,
            "Kết nối Jira…",
            theme::BUTTON_FONT,
            theme::solid(ThemeColor::Text),
            Align::Center,
        );
        c.hit(
            Rect::new(row.x, y, button.w, LINK_ROW_H),
            Target::Connect(Service::Jira),
        );
    }
}

/// The state badge colour and symbol of a PR item, as in the PR list.
fn pr_badge(tone: Option<ItemTone>) -> (ThemeColor, &'static str) {
    match tone {
        Some(ItemTone::Merged) => (ThemeColor::Indigo, "arrow.triangle.merge"),
        Some(ItemTone::Closed) => (ThemeColor::Red, "xmark"),
        _ => (ThemeColor::Green, "arrow.triangle.branch"),
    }
}

/// One two-line item of `it`: badge, then the title over the code, a dot and the status; `first` has no hairline above.
fn link_item(ctx: &Ctx, c: &mut Canvas, s: &Session, item: &LinkItem, it: Rect, first: bool) {
    if !first {
        c.hairline(it.x + TEXT_X, it.y, it.w - TEXT_X);
    }
    let badge = Rect::new(it.x + 10.0, it.y + (it.h - BADGE) / 2.0, BADGE, BADGE);
    let mut dot = match item.tone {
        Some(_) => theme::tone_token(item.tone),
        None => ThemeColor::TextFaint,
    };
    match item.kind {
        LinkKind::Ticket => {
            c.rrect(badge, 6.0, Some(theme::solid(ThemeColor::Fill)), None);
            if let Some(u) = &item.icon_url {
                let icon = Rect::new(badge.x + 3.0, badge.y + 3.0, 14.0, 14.0);
                icon_or_placeholder(ctx.view.snap, c, icon, u, item.tone);
            }
        }
        LinkKind::Pr => {
            let (color, symbol) = pr_badge(item.tone);
            state_badge(c, badge, color, symbol);
            dot = color;
        }
        LinkKind::Link => {
            c.rrect(badge, 6.0, Some(theme::solid(ThemeColor::Fill)), None);
            c.symbol(
                "link",
                Rect::new(badge.x + 4.0, badge.y + 4.0, 12.0, 12.0),
                theme::solid(ThemeColor::TextSecondary),
                Weight::Regular,
            );
        }
    }

    let (x, right) = (it.x + TEXT_X, it.x + it.w - 10.0);
    let title = item.title.as_deref().filter(|t| !t.is_empty());
    let status = item.status.as_deref().filter(|t| !t.is_empty());
    let text = |c: &mut Canvas, y: f64, w: f64, t: &str| {
        c.label(
            Rect::new(x, y, w, 16.0),
            t,
            theme::PR_TITLE_FONT,
            theme::solid(ThemeColor::Text),
            Align::Left,
        );
    };
    match (title, status) {
        (None, None) => text(c, it.y + (it.h - 16.0) / 2.0, right - x, &item.code),
        _ => {
            // Line 1: the title, else the code. Line 2: the code only after a title, then the dot and the status.
            text(c, it.y + 5.0, right - x, title.unwrap_or(&item.code));
            let (y2, mut x2) = (it.y + 21.0, x);
            if title.is_some() {
                let w = ctx.text_w(&item.code, &theme::META_FONT).min(right - x);
                c.label(
                    Rect::new(x2, y2, w, 14.0),
                    &item.code,
                    theme::META_FONT,
                    theme::solid(ThemeColor::TextSecondary),
                    Align::Left,
                );
                x2 += w + 8.0;
            }
            if let Some(status) = status {
                c.rrect(
                    Rect::new(x2, y2 + 4.0, 6.0, 6.0),
                    3.0,
                    Some(theme::solid(dot)),
                    None,
                );
                x2 += 11.0;
                c.label(
                    Rect::new(x2, y2, (right - x2).max(0.0), 14.0),
                    status,
                    theme::META_FONT,
                    theme::solid(ThemeColor::TextSecondary),
                    Align::Left,
                );
            }
        }
    }
    c.tip(it, item.help.as_deref().unwrap_or(&item.text));
    c.hit(
        it,
        Target::Link {
            session: s.id.clone(),
            url: item.url.clone(),
        },
    );
}
