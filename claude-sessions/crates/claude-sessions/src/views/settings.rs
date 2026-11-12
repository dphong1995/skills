//! The Settings window as a scene: a glass panel with a fixed title bar and the groups below it (they scroll when the content
//! is taller than `max_h`). Pure like the other views; the clicks are `Target::Settings` and the text fields are native.

use super::{Built, Canvas, Targets, node_of};
use crate::hits::Target;
use crate::hotkey::{Hotkeys, Which};
use crate::settings::BG_OPACITIES;
use crate::settings_model::{
    CheckState, PROMPT_KEY, SITE_KEY, SettingsAction, SettingsModel, email_key, status_line,
    token_key,
};
use crate::theme;
use claude_sessions_core::credentials::Service;
use claude_sessions_core::layout::TextMeasure;
use claude_sessions_core::scene::{
    Align, Field, FieldKind, FontSpec, Material, Node, Point, Rect, Scene, Size, ThemeColor, Weight,
};
use claude_sessions_core::store::Store;
use std::collections::HashMap;

pub const WIDTH: f64 = 520.0;
const PAD: f64 = 20.0;
const GROUP_W: f64 = WIDTH - 2.0 * PAD;
const TITLE_BAR_H: f64 = 44.0;
const TITLE_H: f64 = 28.0;
const ROW_H: f64 = 40.0;
const GAP: f64 = 12.0;
const INNER: f64 = 12.0;
const BOX_H: f64 = 26.0;
const LABEL_W: f64 = 60.0;
const BUTTON_H: f64 = 28.0;

const ROW_FONT: FontSpec = theme::font(13.0, Weight::Regular);
const NAME_FONT: FontSpec = theme::font(13.0, Weight::Semibold);

pub struct SettingsView<'a> {
    pub model: &'a SettingsModel,
    pub store: &'a Store,
    pub hotkeys: &'a Hotkeys,
    pub measure: &'a dyn TextMeasure,
    pub max_h: f64,
}

fn text_w(v: &SettingsView, text: &str, font: &FontSpec) -> f64 {
    v.measure.measure(text, font, f64::MAX / 4.0, 1).w
}

fn act(a: SettingsAction) -> Target {
    Target::Settings(a)
}

fn group(c: &mut Canvas, y: f64, h: f64) {
    c.rrect(
        Rect::new(0.0, y, GROUP_W, h),
        theme::GROUP_RADIUS,
        Some(theme::solid(ThemeColor::Group)),
        None,
    );
}

fn section_title(c: &mut Canvas, text: &str) {
    c.text(
        Rect::new(8.0, TITLE_H - 18.0, GROUP_W - 8.0, 14.0),
        text,
        theme::SECTION_FONT,
        theme::solid(ThemeColor::TextSecondary),
        1,
        Align::Left,
    );
}

fn row_label(c: &mut Canvas, y: f64, text: &str, color: ThemeColor) {
    c.label(
        Rect::new(INNER, y, 200.0, ROW_H),
        text,
        ROW_FONT,
        theme::solid(color),
        Align::Left,
    );
}

/// A node holding one canvas, `h` tall, at `y` of the scene.
fn node(targets: &mut Targets, key: &str, y: f64, h: f64, c: Canvas) -> Node {
    node_of(key, Rect::new(PAD, y, GROUP_W, h), c, targets)
}

fn background(size: Size, targets: &mut Targets) -> Node {
    let mut c = Canvas::default();
    // As the expanded card: the tint is part of the node, the blur behind comes from `material`.
    c.rrect(
        Rect::new(0.5, 0.5, size.w - 1.0, size.h - 1.0),
        theme::PANEL_RADIUS - 0.5,
        Some(theme::solid(ThemeColor::Glass)),
        Some(theme::solid(ThemeColor::GlassEdge)),
    );
    let mut n = node_of(
        "settings-panel",
        Rect::new(0.0, 0.0, size.w, size.h),
        c,
        targets,
    );
    n.shadow = Some(theme::PANEL_SHADOW);
    n.material = Some(Material::Glass {
        corner: theme::PANEL_RADIUS,
        alpha: 1.0,
    });
    n
}

fn title_bar(targets: &mut Targets) -> Node {
    let mut c = Canvas::default();
    c.rrect(
        Rect::new(16.0, (TITLE_BAR_H - 12.0) / 2.0, 12.0, 12.0),
        6.0,
        Some(theme::solid(ThemeColor::Red)),
        None,
    );
    c.hit(
        Rect::new(12.0, (TITLE_BAR_H - 20.0) / 2.0, 20.0, 20.0),
        act(SettingsAction::Close),
    );
    c.label(
        Rect::new(0.0, 0.0, WIDTH, TITLE_BAR_H),
        "Cài đặt",
        NAME_FONT,
        theme::solid(ThemeColor::Text),
        Align::Center,
    );
    node_of(
        "settings-title",
        Rect::new(0.0, 0.0, WIDTH, TITLE_BAR_H),
        c,
        targets,
    )
}

fn appearance(v: &SettingsView) -> (Canvas, f64) {
    let mut c = Canvas::default();
    section_title(&mut c, "GIAO DIỆN");
    group(&mut c, TITLE_H, 2.0 * ROW_H);
    let y = TITLE_H;
    row_label(&mut c, y, "Độ đậm nền", ThemeColor::Text);
    let selected = BG_OPACITIES
        .iter()
        .enumerate()
        .min_by(|a, b| {
            (a.1 - v.store.bg_opacity)
                .abs()
                .total_cmp(&(b.1 - v.store.bg_opacity).abs())
        })
        .map_or(0, |(i, _)| i);
    let labels = ["0%", "25%", "50%", "75%", "92%"];
    let w = 216.0;
    c.segmented(
        Rect::new(GROUP_W - INNER - w, y + (ROW_H - BOX_H) / 2.0, w, BOX_H),
        &labels,
        selected,
        BG_OPACITIES
            .iter()
            .map(|o| act(SettingsAction::SetOpacity(*o)))
            .collect(),
    );
    c.hairline(INNER, y + ROW_H, GROUP_W - 2.0 * INNER);
    row_label(&mut c, y + ROW_H, "Âm thanh thông báo", ThemeColor::Text);
    let (sw, sh) = (44.0, 26.0);
    c.switch(
        Rect::new(GROUP_W - INNER - sw, y + ROW_H + (ROW_H - sh) / 2.0, sw, sh),
        !v.store.muted,
        act(SettingsAction::ToggleMute),
    );
    (c, TITLE_H + 2.0 * ROW_H)
}

fn hotkeys(v: &SettingsView) -> (Canvas, f64) {
    let mut c = Canvas::default();
    section_title(&mut c, "PHÍM TẮT");
    group(&mut c, TITLE_H, 2.0 * ROW_H);
    let rows = [
        (Which::Open, "Mở widget"),
        (Which::Next, "Nhảy tới session chờ"),
    ];
    let mut conflicts = Vec::new();
    for (i, (which, label)) in rows.into_iter().enumerate() {
        let y = TITLE_H + i as f64 * ROW_H;
        if i > 0 {
            c.hairline(INNER, y, GROUP_W - 2.0 * INNER);
        }
        row_label(&mut c, y, label, ThemeColor::Text);
        let choice = v.hotkeys.choice(which);
        let value = choice.combo().map_or("Tắt", |k| k.label.as_str());
        let failed = v.hotkeys.failed.contains(which.name());
        if failed {
            conflicts.push(value.to_string());
        }
        let w = (text_w(v, value, &theme::BUTTON_FONT) + 44.0).max(72.0);
        let rect = Rect::new(GROUP_W - INNER - w, y + (ROW_H - BOX_H) / 2.0, w, BOX_H);
        c.rrect(rect, 8.0, Some(theme::solid(ThemeColor::Fill)), None);
        let color = if failed {
            ThemeColor::Red
        } else {
            ThemeColor::Text
        };
        c.label(
            Rect::new(rect.x + 10.0, rect.y, rect.w - 32.0, rect.h),
            value,
            theme::BUTTON_FONT,
            theme::solid(color),
            Align::Left,
        );
        c.symbol(
            "chevron.up.chevron.down",
            Rect::new(rect.x + rect.w - 20.0, rect.y + 7.0, 12.0, 12.0),
            theme::solid(ThemeColor::TextSecondary),
            Weight::Medium,
        );
        c.hit(rect, act(SettingsAction::PickHotkey(which)));
    }
    let mut h = TITLE_H + 2.0 * ROW_H;
    for combo in conflicts {
        c.text(
            Rect::new(8.0, h + 6.0, GROUP_W - 16.0, 14.0),
            &format!("{combo} đang bị app khác dùng"),
            theme::CAPTION_FONT,
            theme::solid(ThemeColor::Red),
            1,
            Align::Left,
        );
        h += 18.0;
    }
    if h > TITLE_H + 2.0 * ROW_H {
        h += 6.0;
    }
    (c, h)
}

fn prompt(v: &SettingsView) -> (Canvas, f64) {
    let mut c = Canvas::default();
    section_title(&mut c, "PROMPT NHỜ CLAUDE");
    let h = 12.0 + 76.0 + 32.0;
    group(&mut c, TITLE_H, h);
    let area = Rect::new(INNER, TITLE_H + 12.0, GROUP_W - 2.0 * INNER, 76.0);
    c.rrect(
        area,
        theme::CMD_RADIUS,
        Some(theme::solid(ThemeColor::Code)),
        Some(theme::tint(ThemeColor::Separator, 1.0)),
    );
    c.fields.push(Field {
        key: PROMPT_KEY.to_string(),
        kind: FieldKind::Multiline,
        rect: Rect::new(area.x + 6.0, area.y + 6.0, area.w - 12.0, area.h - 12.0),
        text: v.model.prompt.clone(),
        placeholder: String::new(),
    });
    let row = Rect::new(INNER, area.y + area.h, GROUP_W - 2.0 * INNER, 32.0);
    c.label(
        Rect::new(row.x + 4.0, row.y, 280.0, row.h),
        "{url} = link PR, {id} = số PR",
        theme::CAPTION_FONT,
        theme::solid(ThemeColor::TextFaint),
        Align::Left,
    );
    let link = "Khôi phục mặc định";
    let w = text_w(v, link, &theme::BUTTON_FONT) + 8.0;
    let rect = Rect::new(row.x + row.w - w, row.y, w, row.h);
    c.label(
        rect,
        link,
        theme::BUTTON_FONT,
        theme::solid(ThemeColor::Accent),
        Align::Right,
    );
    c.hit(rect, act(SettingsAction::PromptReset));
    (c, TITLE_H + h)
}

fn connect_title() -> (Canvas, f64) {
    let mut c = Canvas::default();
    section_title(&mut c, "KẾT NỐI");
    (c, TITLE_H)
}

/// A labelled one-line field row: a rounded box and the native field in it.
fn field_row(c: &mut Canvas, y: f64, label: &str, field: Field) {
    row_label(c, y, label, ThemeColor::TextSecondary);
    let x = INNER + LABEL_W;
    let rect = Rect::new(x, y + (ROW_H - BOX_H) / 2.0, GROUP_W - INNER - x, BOX_H);
    c.rrect(
        rect,
        8.0,
        Some(theme::solid(ThemeColor::Code)),
        Some(theme::tint(ThemeColor::Separator, 1.0)),
    );
    c.fields.push(Field {
        rect: Rect::new(rect.x + 8.0, rect.y + 4.0, rect.w - 16.0, rect.h - 8.0),
        ..field
    });
}

fn button(
    c: &mut Canvas,
    rect: Rect,
    label: &str,
    fill: ThemeColor,
    text: ThemeColor,
    live: bool,
    target: Target,
) {
    let (fill, text) = if live {
        (theme::solid(fill), theme::solid(text))
    } else {
        (theme::tint(fill, 0.5), theme::tint(text, 0.5))
    };
    c.capsule(rect, Some(fill));
    c.label(rect, label, theme::BUTTON_FONT, text, Align::Center);
    if live {
        c.hit(rect, target);
    }
}

fn service(v: &SettingsView, s: Service) -> (Canvas, f64) {
    let mut c = Canvas::default();
    let form = v.model.form(s);
    let jira = s == Service::Jira;
    let busy = matches!(form.check, CheckState::Checking | CheckState::Saving);
    let shows_result = form.check != CheckState::Idle;
    let h =
        52.0 + ROW_H * if jira { 3.0 } else { 2.0 } + if shows_result { 24.0 } else { 0.0 } + 44.0;
    group(&mut c, 0.0, h);

    // Header: icon, name, status.
    let (letter, tint) = if jira {
        ("J", ThemeColor::Blue)
    } else {
        ("B", ThemeColor::Indigo)
    };
    let icon = Rect::new(INNER, 12.0, 28.0, 28.0);
    c.rrect(icon, 7.0, Some(theme::solid(tint)), None);
    c.label(
        icon,
        letter,
        theme::font(14.0, Weight::Bold),
        theme::solid(ThemeColor::White),
        Align::Center,
    );
    let text_x = icon.x + icon.w + 12.0;
    c.text(
        Rect::new(text_x, 8.0, GROUP_W - text_x - INNER, 18.0),
        s.display_name(),
        NAME_FONT,
        theme::solid(ThemeColor::Text),
        1,
        Align::Left,
    );
    let (status, color) = status_line(v.model, v.store, s);
    c.text(
        Rect::new(text_x, 27.0, GROUP_W - text_x - INNER, 14.0),
        &status,
        theme::META_FONT,
        theme::solid(color),
        1,
        Align::Left,
    );
    let mut y = 52.0;
    c.hairline(INNER, y, GROUP_W - 2.0 * INNER);

    field_row(
        &mut c,
        y,
        "Email",
        Field {
            key: email_key(s),
            kind: FieldKind::Line,
            rect: Rect::default(),
            text: form.email.clone(),
            placeholder: "ban@cong-ty.com".to_string(),
        },
    );
    y += ROW_H;
    if jira {
        c.hairline(INNER, y, GROUP_W - 2.0 * INNER);
        field_row(
            &mut c,
            y,
            "Site",
            Field {
                key: SITE_KEY.to_string(),
                kind: FieldKind::Line,
                rect: Rect::default(),
                text: v.model.site.clone(),
                placeholder: v
                    .store
                    .jira_guess()
                    .unwrap_or_else(|| "ten-cong-ty.atlassian.net".to_string()),
            },
        );
        y += ROW_H;
    }
    c.hairline(INNER, y, GROUP_W - 2.0 * INNER);
    field_row(
        &mut c,
        y,
        "Token",
        Field {
            key: token_key(s, form.token_gen),
            kind: FieldKind::Secure,
            rect: Rect::default(),
            text: String::new(),
            placeholder: if v.store.accounts.contains_key(&s) {
                "Đã lưu · để trống để giữ"
            } else {
                "API token"
            }
            .to_string(),
        },
    );
    y += ROW_H;

    if shows_result {
        let (text, color) = match &form.check {
            CheckState::Checking | CheckState::Saving => {
                let site = if jira {
                    v.model.effective_site(v.store).unwrap_or_default()
                } else {
                    "api.bitbucket.org".to_string()
                };
                (format!("Đang gọi {site}…"), ThemeColor::TextSecondary)
            }
            CheckState::Ok(t) => (t.clone(), ThemeColor::Green),
            CheckState::Failed(t) => (t.clone(), ThemeColor::Red),
            CheckState::Idle => unreachable!("an idle form shows no result"),
        };
        c.text(
            Rect::new(
                INNER + LABEL_W,
                y + 4.0,
                GROUP_W - INNER - LABEL_W - INNER,
                14.0,
            ),
            &text,
            theme::META_FONT,
            theme::solid(color),
            1,
            Align::Left,
        );
        y += 24.0;
    }

    // Buttons: the link at the left, `Kiểm tra` and `Lưu` at the right.
    let by = y + (44.0 - BUTTON_H) / 2.0;
    let link = "Tạo token…";
    let lw = text_w(v, link, &theme::BUTTON_FONT) + 8.0;
    let link_rect = Rect::new(INNER, by, lw, BUTTON_H);
    c.label(
        link_rect,
        link,
        theme::BUTTON_FONT,
        theme::solid(ThemeColor::Accent),
        Align::Left,
    );
    c.hit(link_rect, act(SettingsAction::OpenTokenPage));
    let width = |normal: &str, flight: &str| {
        (text_w(v, normal, &theme::BUTTON_FONT).max(text_w(v, flight, &theme::BUTTON_FONT)) + 28.0)
            .max(96.0)
    };
    let save_w = width("Lưu", "Đang lưu…");
    let check_w = width("Kiểm tra", "Đang kiểm tra…");
    let save_rect = Rect::new(GROUP_W - INNER - save_w, by, save_w, BUTTON_H);
    let check_rect = Rect::new(save_rect.x - 8.0 - check_w, by, check_w, BUTTON_H);
    let check_label = if form.check == CheckState::Checking {
        "Đang kiểm tra…"
    } else {
        "Kiểm tra"
    };
    let save_label = if form.check == CheckState::Saving {
        "Đang lưu…"
    } else {
        "Lưu"
    };
    button(
        &mut c,
        check_rect,
        check_label,
        ThemeColor::Fill,
        ThemeColor::Text,
        !busy,
        act(SettingsAction::Check(s)),
    );
    button(
        &mut c,
        save_rect,
        save_label,
        ThemeColor::Accent,
        ThemeColor::White,
        !busy,
        act(SettingsAction::Save(s)),
    );
    (c, h)
}

pub fn build(v: &SettingsView) -> Built {
    let mut targets = Targets::default();
    let sections = [
        ("settings-appearance", appearance(v)),
        ("settings-hotkeys", hotkeys(v)),
        ("settings-prompt", prompt(v)),
        ("settings-connect", connect_title()),
        ("settings-jira", service(v, Service::Jira)),
        ("settings-bitbucket", service(v, Service::Bitbucket)),
    ];
    let mut y = TITLE_BAR_H;
    let mut body = Vec::new();
    for (key, (c, h)) in sections {
        body.push(node(&mut targets, key, y, h, c));
        // The service groups follow their title closely.
        y += h + if key == "settings-connect" { 0.0 } else { GAP };
    }
    let content_h = y + PAD - GAP;
    let size = Size::new(WIDTH, content_h.min(v.max_h));
    let mut nodes = vec![background(size, &mut targets), title_bar(&mut targets)];
    if content_h > size.h {
        let mut scroll = node_of(
            "settings-scroll",
            Rect::new(0.0, TITLE_BAR_H, WIDTH, size.h - TITLE_BAR_H),
            Canvas::default(),
            &mut targets,
        );
        scroll.scroll = Some(Size::new(WIDTH, content_h - TITLE_BAR_H));
        nodes.push(scroll);
    }
    nodes.extend(body);
    Built {
        scene: Scene {
            size,
            nodes,
            images: HashMap::new(),
            grow_origin: Point::default(),
        },
        targets,
    }
}
