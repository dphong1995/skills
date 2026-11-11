//! The redesigned widget as pure functions of `(&Snapshot, &PanelState, &dyn TextMeasure)` (plus the clock) returning a `Scene`.
//! Heights and positions come only from `claude_sessions_core::layout` (`expanded_layout`, `card_layout`, the block helpers);
//! these views place nodes with them and never add heights up themselves. No I/O, no `Store`.
//!
//! Hit order inside a node: the renderer takes the first hit whose rect contains the point, so a control's hit is added before
//! the hit of the card body that contains it.

pub mod approval;
pub mod card;
pub mod chips;
pub mod closed;
pub mod header;
pub mod pill;
pub mod pr_row;
pub mod question;
pub mod section;

use crate::hits::{Target, Targets};
use crate::theme;
use claude_sessions_core::layout::{self, Item, TextMeasure};
use claude_sessions_core::panel_state::PanelState;
use claude_sessions_core::scene::{
    Align, Color, Cursor, DrawCmd, Field, FontSpec, Hit, Node, NodeKey, Rect, Run, Scene, Size,
    ThemeColor, Weight,
};
use claude_sessions_core::store::{SendState, Snapshot};
use std::time::SystemTime;

/// Everything a view reads. `now` and `clock` (local `HH:mm`) are inputs because the snapshot holds timestamps only.
pub struct View<'a> {
    pub snap: &'a Snapshot,
    pub panel: &'a PanelState,
    pub measure: &'a dyn TextMeasure,
    pub now: SystemTime,
    pub clock: &'a dyn Fn(SystemTime) -> String,
}

/// A scene and what its hit ids mean.
#[derive(Clone, Debug)]
pub struct Built {
    pub scene: Scene,
    pub targets: Targets,
}

/// Draw commands, hits and fields of one node, relative to the node's top-left.
#[derive(Default)]
pub struct Canvas {
    pub cmds: Vec<DrawCmd>,
    pub hits: Vec<(Rect, Target)>,
    pub fields: Vec<Field>,
    pub spinner: Option<Rect>,
}

impl Canvas {
    pub fn rrect(&mut self, rect: Rect, radius: f64, fill: Option<Color>, stroke: Option<Color>) {
        self.cmds.push(DrawCmd::RoundedRect {
            rect,
            radius,
            fill,
            stroke,
        });
    }

    pub fn text(
        &mut self,
        rect: Rect,
        text: &str,
        font: FontSpec,
        color: Color,
        max_lines: u32,
        align: Align,
    ) {
        self.cmds.push(DrawCmd::Text {
            rect,
            text: text.to_string(),
            font,
            color,
            max_lines,
            align,
        });
    }

    /// `text` as markdown runs.
    pub fn rich(
        &mut self,
        rect: Rect,
        runs: Vec<Run>,
        font: FontSpec,
        color: Color,
        max_lines: u32,
        align: Align,
    ) {
        self.cmds.push(DrawCmd::Rich {
            rect,
            runs,
            font,
            color,
            max_lines,
            align,
        });
    }

    /// A one-line text vertically centred in `rect`.
    pub fn label(&mut self, rect: Rect, text: &str, font: FontSpec, color: Color, align: Align) {
        let h = 16.0;
        self.text(
            Rect::new(rect.x, rect.y + (rect.h - h) / 2.0, rect.w, h),
            text,
            font,
            color,
            1,
            align,
        );
    }

    /// A labelled button wearing its send state: sending (dimmed, spinner or none, no hit), sent (green), failed (red).
    /// Returns whether it is clickable.
    pub fn button(
        &mut self,
        rect: Rect,
        label: &str,
        fill: Color,
        text: Color,
        state: Option<SendState>,
    ) -> bool {
        let (label, fill, text) = match state {
            None => (label, fill, text),
            Some(SendState::Sending) => (
                "Đang gửi…",
                theme::tint(ThemeColor::Button, 0.5),
                theme::tint(ThemeColor::Text, 0.5),
            ),
            Some(SendState::Sent) => (
                "Đã gửi ✓",
                theme::solid(ThemeColor::Green),
                theme::solid(ThemeColor::OnAccent),
            ),
            Some(SendState::Failed) => (
                "Lỗi — bấm thử lại",
                theme::solid(ThemeColor::Red),
                theme::solid(ThemeColor::White),
            ),
        };
        self.rrect(rect, theme::BUTTON_RADIUS, Some(fill), None);
        let mut face = rect;
        if state == Some(SendState::Sending) && self.spinner.is_none() {
            self.spinner = Some(Rect::new(
                rect.x + 8.0,
                rect.y + (rect.h - 12.0) / 2.0,
                12.0,
                12.0,
            ));
            face.x += 14.0;
            face.w -= 14.0;
        }
        self.label(face, label, theme::BUTTON_FONT, text, Align::Center);
        state != Some(SendState::Sending)
    }

    pub fn symbol(&mut self, name: &str, rect: Rect, color: Color, weight: Weight) {
        self.cmds.push(DrawCmd::Symbol {
            name: name.to_string(),
            rect,
            color,
            weight,
        });
    }

    pub fn hit(&mut self, rect: Rect, target: Target) {
        self.hits.push((rect, target));
    }

    /// An icon button's face for its send state: the symbol, a spinner (or `ellipsis` when the node's spinner is taken) while
    /// sending, a check after `Sent`, a warning after `Failed`. Returns whether the control is clickable (not in flight).
    pub fn send_glyph(
        &mut self,
        rect: Rect,
        state: Option<SendState>,
        symbol: &str,
        color: Color,
    ) -> bool {
        let glyph = Rect::new(
            rect.x + (rect.w - 14.0) / 2.0,
            rect.y + (rect.h - 14.0) / 2.0,
            14.0,
            14.0,
        );
        match state {
            None => self.symbol(symbol, glyph, color, Weight::Medium),
            Some(SendState::Sending) => {
                if self.spinner.is_none() {
                    self.spinner = Some(glyph);
                } else {
                    self.symbol("ellipsis", glyph, color, Weight::Medium);
                }
            }
            Some(SendState::Sent) => self.symbol(
                "checkmark",
                glyph,
                theme::solid(ThemeColor::Green),
                Weight::Bold,
            ),
            Some(SendState::Failed) => self.symbol(
                "exclamationmark.triangle.fill",
                glyph,
                theme::solid(ThemeColor::Red),
                Weight::Medium,
            ),
        }
        state != Some(SendState::Sending)
    }
}

/// Registers hit targets while nodes are made.
pub struct Ctx<'a> {
    pub view: &'a View<'a>,
    pub targets: Targets,
}

impl Ctx<'_> {
    pub fn node(&mut self, key: &str, frame: Rect, canvas: Canvas) -> Node {
        let hits = canvas
            .hits
            .into_iter()
            .map(|(rect, target)| Hit {
                rect,
                id: self.targets.add(target),
                cursor: Cursor::Pointer,
            })
            .collect();
        Node {
            key: NodeKey(key.to_string()),
            frame,
            cmds: canvas.cmds,
            hits,
            fields: canvas.fields,
            spinner: canvas.spinner,
        }
    }

    /// The width of a button labelled `label`: text and padding, at least wide enough for its send-state faces.
    pub fn button_w(&self, label: &str) -> f64 {
        (self.text_w(label, &theme::BUTTON_FONT) + 28.0).max(96.0)
    }

    /// The width `text` takes on one line.
    pub fn text_w(&self, text: &str, font: &FontSpec) -> f64 {
        self.view.measure.measure(text, font, f64::MAX / 4.0, 1).w
    }
}

/// The scene of the panel: the pill while collapsed, else the expanded card.
pub fn build(view: &View) -> Built {
    let mut ctx = Ctx {
        view,
        targets: Targets::default(),
    };
    let scene = if view.panel.expanded(view.snap.peeking) {
        expanded(&mut ctx)
    } else {
        Scene {
            size: Size::new(layout::collapsed_width(view.snap), layout::HEADER_HEIGHT),
            nodes: vec![pill::node(&mut ctx)],
        }
    };
    Built {
        scene,
        targets: ctx.targets,
    }
}

pub fn scene(view: &View) -> Scene {
    build(view).scene
}

fn expanded(ctx: &mut Ctx) -> Scene {
    let view = ctx.view;
    let l = layout::expanded_layout(view.snap, view.panel, view.measure);
    let mut bg = Canvas::default();
    bg.rrect(
        Rect::new(0.0, 0.0, l.size.w, l.size.h),
        theme::PANEL_RADIUS,
        Some(theme::solid(ThemeColor::Panel)),
        Some(theme::solid(ThemeColor::PanelStroke)),
    );
    let mut nodes = vec![ctx.node("panel", Rect::new(0.0, 0.0, l.size.w, l.size.h), bg)];
    for item in &l.items {
        match item {
            Item::Header { y } => nodes.push(header::node(ctx, *y)),
            Item::Section { section, count, y } => {
                nodes.push(section::node(ctx, *section, *count, *y))
            }
            Item::Card { id, y, layout } => {
                if let Some(s) = view.snap.sessions.iter().find(|s| &s.id == id) {
                    nodes.push(card::node(ctx, s, *y, layout));
                }
            }
            Item::Empty { y } => nodes.push(section::empty_node(ctx, *y)),
            Item::Closed { y, h } => nodes.push(closed::node(ctx, *y, *h)),
        }
    }
    Scene {
        size: l.size,
        nodes,
    }
}
