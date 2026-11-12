//! Ticket chips with the menu count (`Block::Chips`, `layout::CHIPS_H`) and the inline link list (`Block::LinkList`,
//! `layout::link_list_height`). The Jira icon is the fetched picture, a tinted placeholder until its bytes are in the snapshot.

use super::{Canvas, Ctx};
use crate::hits::Target;
use crate::theme;
use claude_sessions_core::credentials::Service;
use claude_sessions_core::layout::{
    BLOCK_GAP, CARD_INSET, CHIP_H, LINK_ROW_H, LINK_SECTION_H, TEXT_W,
};
use claude_sessions_core::scene::{Align, Rect, ThemeColor, Weight};
use claude_sessions_core::store::{ItemTone, LinkKind, Session, Snapshot};

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
    let count = s.menu_count().to_string();
    let menu_w = ctx.text_w(&count, &theme::META_FONT) + 34.0;
    let widths: Vec<f64> = s
        .links
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
    for (link, &w) in s.links.iter().zip(&widths).take(shown) {
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
    // The menu count; purple while a PR it does not list yet is new.
    let open = view.panel.open_menus.contains(&s.id);
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
                    if let Some(u) = &item.icon_url {
                        icon_or_placeholder(
                            ctx.view.snap,
                            c,
                            Rect::new(icon.x + 1.0, icon.y + 1.0, 12.0, 12.0),
                            u,
                            item.tone,
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
            let status_w = item.status.as_deref().map_or(0.0, |t| {
                // At most 40% of the row, so a long status never leaves the title only an ellipsis.
                (ctx.text_w(t, &theme::STATE_FONT) + 16.0).min(0.4 * row.w)
            });
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
                c.rrect(tag, 4.0, Some(theme::tint(color, 0.18)), None);
                c.label(
                    tag,
                    status,
                    theme::STATE_FONT,
                    theme::solid(color),
                    Align::Center,
                );
            }
            c.tip(row, item.help.as_deref().unwrap_or(&item.text));
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
            (LINK_ROW_H - 4.0) / 2.0,
            Some(theme::solid(ThemeColor::Fill)),
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
