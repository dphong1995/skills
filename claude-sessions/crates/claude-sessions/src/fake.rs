//! Fake data and the two fake views for the phase 1 gate. Deleted in phase 9 when the real `Snapshot` replaces it.

use claude_sessions_core::layout::TextMeasure;
use claude_sessions_core::scene::{
    Align, Color, Cursor, DrawCmd, Field, FontSpec, Hit, HitId, Node, Point, Rect, Scene, Size,
    ThemeColor, Weight,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Waiting,
    Busy,
    Idle,
}

#[derive(Clone, Debug)]
pub struct Card {
    pub title: &'static str,
    pub detail: &'static str,
    pub status: Status,
}

pub fn cards() -> Vec<Card> {
    vec![
        Card {
            title: "talo6-dct-backend",
            detail: "Cần duyệt: Bash swift test --filter ParserTests",
            status: Status::Waiting,
        },
        Card {
            title: "skills-repo",
            detail: "Đang chạy: Edit main.swift",
            status: Status::Busy,
        },
        Card {
            title: "claude-sessions",
            detail: "Trả lời nhanh cho session này",
            status: Status::Idle,
        },
        Card {
            title: "dotfiles",
            detail: "Xong: cập nhật README",
            status: Status::Idle,
        },
    ]
}

/// Flips one card per call, round robin: the "state change every 10 s" of the gate.
pub fn rotate(cards: &mut [Card], n: u64) {
    let i = (n as usize) % cards.len();
    cards[i].status = match cards[i].status {
        Status::Waiting => Status::Idle,
        Status::Busy => Status::Waiting,
        Status::Idle => Status::Busy,
    };
}

/// The fixed panel canvas: the pill and the expanded card both sit at its top-right.
pub const CANVAS: Size = Size { w: 380.0, h: 500.0 };
const PILL: Size = Size { w: 232.0, h: 40.0 };
pub const PILL_HIT: HitId = HitId(1);
const CARD_HIT_BASE: u32 = 10;
const MARGIN: f64 = 10.0;

fn font(size: f64, weight: Weight) -> FontSpec {
    FontSpec {
        size,
        weight,
        mono: false,
    }
}

fn text(
    rect: Rect,
    text: &str,
    font: FontSpec,
    color: ThemeColor,
    max_lines: u32,
    align: Align,
) -> DrawCmd {
    DrawCmd::Text {
        rect,
        text: text.to_owned(),
        font,
        color: color.opaque(),
        max_lines,
        align,
    }
}

fn node(key: &str, frame: Rect, cmds: Vec<DrawCmd>) -> Node {
    Node {
        key: key.into(),
        frame,
        cmds,
        hits: vec![],
        fields: vec![],
        spinner: None,
    }
}

fn panel_background(frame: Rect) -> DrawCmd {
    DrawCmd::RoundedRect {
        rect: Rect::new(0.0, 0.0, frame.w, frame.h),
        radius: 14.0,
        fill: Some(ThemeColor::Panel.opaque()),
        stroke: Some(ThemeColor::PanelStroke.opaque()),
    }
}

/// The collapsed pill: one node, at the top-right of the canvas.
pub fn pill_scene(cards: &[Card]) -> Scene {
    let waiting = cards.iter().filter(|c| c.status == Status::Waiting).count();
    let label = format!("{} phiên · {} chờ bạn", cards.len(), waiting);
    let frame = Rect::new(CANVAS.w - PILL.w, 0.0, PILL.w, PILL.h);
    let dot = if waiting > 0 {
        ThemeColor::Red
    } else {
        ThemeColor::Green
    };
    let mut pill = node(
        "card",
        frame,
        vec![
            panel_background(frame),
            DrawCmd::Symbol {
                name: "circle.fill".into(),
                rect: Rect::new(14.0, 13.0, 14.0, 14.0),
                color: dot.opaque(),
                weight: Weight::Regular,
            },
            text(
                Rect::new(36.0, 12.0, frame.w - 48.0, 18.0),
                &label,
                font(13.0, Weight::Medium),
                ThemeColor::Text,
                1,
                Align::Left,
            ),
        ],
    );
    pill.hits.push(Hit {
        rect: Rect::new(0.0, 0.0, frame.w, frame.h),
        id: PILL_HIT,
        cursor: Cursor::Pointer,
    });
    Scene {
        size: CANVAS,
        nodes: vec![pill],
    }
}

fn section_title(status: Status) -> (&'static str, ThemeColor) {
    match status {
        Status::Waiting => ("CẦN BẠN", ThemeColor::Red),
        Status::Idle => ("XONG", ThemeColor::Orange),
        Status::Busy => ("ĐANG CHẠY", ThemeColor::Blue),
    }
}

fn status_symbol(status: Status) -> Option<(&'static str, ThemeColor)> {
    match status {
        Status::Waiting => Some(("exclamationmark.circle.fill", ThemeColor::Red)),
        Status::Idle => Some(("checkmark.circle.fill", ThemeColor::Orange)),
        Status::Busy => None,
    }
}

/// The expanded card: header, three sections of cards and a reply field. `elapsed` drives the time labels.
pub fn expanded_scene(cards: &[Card], elapsed: u64, measure: &dyn TextMeasure) -> Scene {
    let width = CANVAS.w;
    let mut nodes = vec![node("card", Rect::default(), vec![])];
    let mut header = node(
        "header",
        Rect::new(0.0, 0.0, width, PILL.h),
        vec![
            text(
                Rect::new(16.0, 11.0, 120.0, 20.0),
                "Claude",
                font(15.0, Weight::Semibold),
                ThemeColor::TextSecondary,
                1,
                Align::Left,
            ),
            text(
                Rect::new(width - 160.0, 13.0, 144.0, 16.0),
                &format!("{} phiên", cards.len()),
                font(12.0, Weight::Regular),
                ThemeColor::TextSecondary,
                1,
                Align::Right,
            ),
        ],
    );
    header.hits.push(Hit {
        rect: Rect::new(0.0, 0.0, width, PILL.h),
        id: PILL_HIT,
        cursor: Cursor::Arrow,
    });
    nodes.push(header);

    let mut y = PILL.h;
    for status in [Status::Waiting, Status::Idle, Status::Busy] {
        let group: Vec<(usize, &Card)> = cards
            .iter()
            .enumerate()
            .filter(|(_, c)| c.status == status)
            .collect();
        if group.is_empty() {
            continue;
        }
        let (title, color) = section_title(status);
        nodes.push(node(
            &format!("section-{title}"),
            Rect::new(0.0, y, width, 24.0),
            vec![
                DrawCmd::Symbol {
                    name: "circle.fill".into(),
                    rect: Rect::new(18.0, 8.0, 8.0, 8.0),
                    color: color.opaque(),
                    weight: Weight::Regular,
                },
                text(
                    Rect::new(34.0, 4.0, 200.0, 16.0),
                    &format!("{title} · {}", group.len()),
                    font(11.0, Weight::Semibold),
                    ThemeColor::TextSecondary,
                    1,
                    Align::Left,
                ),
            ],
        ));
        y += 24.0;
        for (i, card) in group {
            let row_w = width - 2.0 * MARGIN;
            let detail_w = row_w - 52.0;
            let detail_h = measure
                .measure(card.detail, &font(12.0, Weight::Regular), detail_w, 2)
                .h;
            let row_h = 36.0 + detail_h + 10.0;
            let fill = if status == Status::Waiting {
                ThemeColor::RowAttention
            } else {
                ThemeColor::Row
            };
            let mut cmds = vec![
                DrawCmd::RoundedRect {
                    rect: Rect::new(0.0, 0.0, row_w, row_h),
                    radius: 10.0,
                    fill: Some(fill.opaque()),
                    stroke: None,
                },
                text(
                    Rect::new(40.0, 10.0, row_w - 110.0, 18.0),
                    card.title,
                    font(13.0, Weight::Semibold),
                    ThemeColor::Text,
                    1,
                    Align::Left,
                ),
                text(
                    Rect::new(row_w - 64.0, 11.0, 52.0, 16.0),
                    &format!("{}s", 13 + 7 * i as u64 + elapsed),
                    font(12.0, Weight::Regular),
                    ThemeColor::TextSecondary,
                    1,
                    Align::Right,
                ),
                text(
                    Rect::new(40.0, 32.0, detail_w, detail_h),
                    card.detail,
                    font(12.0, Weight::Regular),
                    ThemeColor::TextSecondary,
                    2,
                    Align::Left,
                ),
            ];
            if let Some((name, tint)) = status_symbol(status) {
                cmds.push(DrawCmd::Symbol {
                    name: name.into(),
                    rect: Rect::new(12.0, 9.0, 20.0, 20.0),
                    color: Color {
                        token: tint,
                        alpha: 1.0,
                    },
                    weight: Weight::Semibold,
                });
            }
            let mut row = node(
                &format!("row-{}", card.title),
                Rect::new(MARGIN, y, row_w, row_h),
                cmds,
            );
            row.hits.push(Hit {
                rect: Rect::new(0.0, 0.0, row_w, row_h),
                id: HitId(CARD_HIT_BASE + i as u32),
                cursor: Cursor::Pointer,
            });
            if status == Status::Busy {
                row.spinner = Some(Rect::new(12.0, 9.0, 20.0, 20.0));
            }
            nodes.push(row);
            y += row_h + 6.0;
        }
        y += 4.0;
    }

    let mut reply = node(
        "reply",
        Rect::new(MARGIN, y, width - 2.0 * MARGIN, 28.0),
        vec![],
    );
    reply.fields.push(Field {
        key: "reply".into(),
        rect: Rect::new(0.0, 0.0, width - 2.0 * MARGIN, 28.0),
        text: String::new(),
        placeholder: "Trả lời nhanh…".into(),
    });
    nodes.push(reply);
    y += 28.0 + MARGIN;

    let frame = Rect::new(0.0, 0.0, width, y);
    nodes[0] = node("card", frame, vec![panel_background(frame)]);
    Scene {
        size: CANVAS,
        nodes,
    }
}

/// The card's frame in a scene, for the pointer test.
pub fn card_frame(scene: &Scene) -> Option<Rect> {
    scene
        .nodes
        .iter()
        .find(|n| n.key.0 == "card")
        .map(|n| n.frame)
}

/// Where a point lies relative to the card, in scene points.
pub fn card_contains(scene: &Scene, p: Point) -> bool {
    card_frame(scene).is_some_and(|f| f.contains(p))
}
