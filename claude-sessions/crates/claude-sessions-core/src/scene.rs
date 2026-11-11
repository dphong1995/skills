//! The scene: pure data that views produce and the AppKit layer draws. Origin is top-left (flipped), units are points.

use std::collections::HashMap;
use std::sync::Arc;

pub use crate::geometry::{Point, Rect, Size};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appearance {
    Dark,
    Light,
}

/// Colour tokens; each resolves to an RGBA value per appearance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeColor {
    Panel,
    PanelStroke,
    Row,
    RowAttention,
    Code,
    Button,
    Text,
    TextSecondary,
    /// Timestamps, section titles and other third-level text.
    TextFaint,
    /// Label on a filled accent (the green `Cho phép` button).
    OnAccent,
    /// White in both appearances (the count inside the red badge).
    White,
    Black,
    Red,
    Orange,
    Green,
    Blue,
    Yellow,
    Purple,
}

impl ThemeColor {
    pub fn rgba(self, appearance: Appearance) -> [f64; 4] {
        let dark = appearance == Appearance::Dark;
        let pick = |d: [f64; 4], l: [f64; 4]| if dark { d } else { l };
        match self {
            Self::Panel => pick([0.110, 0.110, 0.118, 0.96], [0.965, 0.965, 0.97, 0.95]),
            Self::PanelStroke => pick([1.0, 1.0, 1.0, 0.10], [0.0, 0.0, 0.0, 0.12]),
            Self::Row => pick([1.0, 1.0, 1.0, 0.07], [0.0, 0.0, 0.0, 0.05]),
            Self::RowAttention => pick([1.0, 0.75, 0.2, 0.10], [1.0, 0.6, 0.0, 0.12]),
            Self::Code => pick([0.0, 0.0, 0.0, 0.35], [0.0, 0.0, 0.0, 0.07]),
            Self::Button => pick([1.0, 1.0, 1.0, 0.14], [0.0, 0.0, 0.0, 0.08]),
            Self::Text => pick([1.0, 1.0, 1.0, 0.92], [0.0, 0.0, 0.0, 0.88]),
            Self::TextSecondary => pick([0.65, 0.65, 0.67, 1.0], [0.0, 0.0, 0.0, 0.58]),
            Self::TextFaint => pick([0.42, 0.43, 0.46, 1.0], [0.0, 0.0, 0.0, 0.38]),
            Self::OnAccent => pick([0.03, 0.15, 0.06, 1.0], [1.0, 1.0, 1.0, 1.0]),
            Self::White => [1.0, 1.0, 1.0, 1.0],
            Self::Black => [0.0, 0.0, 0.0, 1.0],
            Self::Red => pick([1.0, 0.38, 0.35, 1.0], [0.85, 0.15, 0.12, 1.0]),
            Self::Orange => pick([1.0, 0.72, 0.25, 1.0], [0.85, 0.5, 0.0, 1.0]),
            Self::Green => pick([0.2, 0.84, 0.35, 1.0], [0.1, 0.62, 0.2, 1.0]),
            Self::Blue => pick([0.35, 0.65, 1.0, 1.0], [0.0, 0.4, 0.9, 1.0]),
            Self::Yellow => pick([1.0, 0.82, 0.1, 1.0], [0.8, 0.6, 0.0, 1.0]),
            Self::Purple => pick([0.75, 0.48, 1.0, 1.0], [0.55, 0.25, 0.85, 1.0]),
        }
    }

    pub const fn opaque(self) -> Color {
        Color {
            token: self,
            alpha: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub token: ThemeColor,
    /// Multiplied into the token's own alpha.
    pub alpha: f64,
}

impl Color {
    pub fn resolve(self, appearance: Appearance) -> [f64; 4] {
        let [r, g, b, a] = self.token.rgba(appearance);
        [r, g, b, a * self.alpha]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    Regular,
    Medium,
    Semibold,
    Bold,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontSpec {
    pub size: f64,
    pub weight: Weight,
    pub mono: bool,
}

/// A stretch of `Rich` text; the flags change the base font, the colour stays the command's.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub mono: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DrawCmd {
    RoundedRect {
        rect: Rect,
        radius: f64,
        fill: Option<Color>,
        stroke: Option<Color>,
    },
    /// `max_lines` 0 = unlimited; text beyond it is cut with an ellipsis.
    Text {
        rect: Rect,
        text: String,
        font: FontSpec,
        color: Color,
        max_lines: u32,
        align: Align,
    },
    /// `Text` made of runs that differ in weight, slant or monospacing (markdown); `font` is the base every run starts from.
    Rich {
        rect: Rect,
        runs: Vec<Run>,
        font: FontSpec,
        color: Color,
        max_lines: u32,
        align: Align,
    },
    /// An SF Symbol by name.
    Symbol {
        name: String,
        rect: Rect,
        color: Color,
        weight: Weight,
    },
    Line {
        from: Point,
        to: Point,
        color: Color,
        width: f64,
    },
    /// A stroked arc of the circle around `center`. Angles are in degrees from 12 o'clock, clockwise; `end` > `start`.
    Arc {
        center: Point,
        radius: f64,
        width: f64,
        start: f64,
        end: f64,
        color: Color,
    },
    /// The picture stored under `key` in `Scene::images`, scaled into `rect`; nothing while the key is absent or undecodable.
    Image { rect: Rect, key: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HitId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cursor {
    Arrow,
    Pointer,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub rect: Rect,
    pub id: HitId,
    pub cursor: Cursor,
}

/// A tooltip over a rect of its node.
#[derive(Clone, Debug, PartialEq)]
pub struct Tip {
    pub rect: Rect,
    pub text: String,
}

/// A native, editable text field laid over the scene.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub key: String,
    pub rect: Rect,
    pub text: String,
    pub placeholder: String,
}

/// Stable per pill/header/section/card/row so the renderer can tell what changed.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeKey(pub String);

impl From<&str> for NodeKey {
    fn from(key: &str) -> Self {
        Self(key.to_owned())
    }
}

/// A soft drop shadow under a node's rounded rect; `offset.y` positive is down in the flipped scene.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub color: Color,
    pub radius: f64,
    pub offset: Point,
    /// Corner radius of the shape that casts it.
    pub corner: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub key: NodeKey,
    /// In scene points; draw commands are relative to the node's own top-left.
    pub frame: Rect,
    pub cmds: Vec<DrawCmd>,
    /// Relative to the node's top-left.
    pub hits: Vec<Hit>,
    pub fields: Vec<Field>,
    /// The loading spinner's rect relative to the node's top-left, if it shows one.
    pub spinner: Option<Rect>,
    /// Hover texts, relative to the node's top-left.
    pub tips: Vec<Tip>,
    /// `Some(content size)` makes this node a scroll viewport (its `frame`): every node after it in `Scene::nodes` lives in the
    /// scrolled content, keeping its scene position, so content point = scene point - the viewport's origin.
    pub scroll: Option<Size>,
    /// Cast by the node's whole frame (a rounded rect of radius `corner`), outside it.
    pub shadow: Option<Shadow>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    pub size: Size,
    pub nodes: Vec<Node>,
    /// Encoded picture bytes (PNG, JPEG...) by the keys of `DrawCmd::Image`.
    pub images: HashMap<String, Arc<Vec<u8>>>,
}

#[derive(Debug, PartialEq)]
pub struct NodeUpdate<'a> {
    pub node: &'a Node,
    pub frame_changed: bool,
    pub cmds_changed: bool,
}

#[derive(Debug, Default, PartialEq)]
pub struct SceneDiff<'a> {
    pub added: Vec<&'a Node>,
    pub removed: Vec<&'a NodeKey>,
    /// Nodes present in both scenes that differ in any way; unchanged nodes are absent.
    pub updated: Vec<NodeUpdate<'a>>,
}

/// What changed from `old` to `new`, by node key.
pub fn changed_nodes<'a>(old: &'a Scene, new: &'a Scene) -> SceneDiff<'a> {
    let before: HashMap<&NodeKey, &Node> = old.nodes.iter().map(|n| (&n.key, n)).collect();
    let mut diff = SceneDiff::default();
    for node in &new.nodes {
        match before.get(&node.key) {
            None => diff.added.push(node),
            Some(prev) if *prev != node => diff.updated.push(NodeUpdate {
                node,
                frame_changed: prev.frame != node.frame,
                cmds_changed: prev.cmds != node.cmds,
            }),
            Some(_) => {}
        }
    }
    let after: HashMap<&NodeKey, ()> = new.nodes.iter().map(|n| (&n.key, ())).collect();
    diff.removed = old
        .nodes
        .iter()
        .map(|n| &n.key)
        .filter(|k| !after.contains_key(k))
        .collect();
    diff
}

/// `cmd` with every colour replaced by one fixed colour: two commands that are equal this way differ only in colour.
fn without_colors(cmd: &DrawCmd) -> DrawCmd {
    let c = Color {
        token: ThemeColor::Text,
        alpha: 1.0,
    };
    let mut cmd = cmd.clone();
    match &mut cmd {
        DrawCmd::RoundedRect { fill, stroke, .. } => {
            *fill = fill.map(|_| c);
            *stroke = stroke.map(|_| c);
        }
        DrawCmd::Text { color, .. }
        | DrawCmd::Rich { color, .. }
        | DrawCmd::Symbol { color, .. }
        | DrawCmd::Line { color, .. }
        | DrawCmd::Arc { color, .. } => *color = c,
        DrawCmd::Image { .. } => {}
    }
    cmd
}

/// The two command lists are the same drawing in different colours (a status change), so the renderer may cross-fade them.
pub fn colors_only_changed(old: &[DrawCmd], new: &[DrawCmd]) -> bool {
    old != new
        && old.len() == new.len()
        && old
            .iter()
            .zip(new)
            .all(|(a, b)| without_colors(a) == without_colors(b))
}

/// The rects of the `name` symbols in `new` that `old` did not draw at the same place: a mark that just appeared.
pub fn appeared_symbols(old: &[DrawCmd], new: &[DrawCmd], name: &str) -> Vec<Rect> {
    let rects = |cmds: &[DrawCmd]| -> Vec<Rect> {
        cmds.iter()
            .filter_map(|c| match c {
                DrawCmd::Symbol { name: n, rect, .. } if n == name => Some(*rect),
                _ => None,
            })
            .collect()
    };
    let before = rects(old);
    rects(new)
        .into_iter()
        .filter(|r| !before.contains(r))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(key: &str, x: f64, text: &str) -> Node {
        Node {
            key: key.into(),
            frame: Rect::new(x, 0.0, 10.0, 10.0),
            cmds: vec![DrawCmd::Text {
                rect: Rect::new(0.0, 0.0, 10.0, 10.0),
                text: text.into(),
                font: FontSpec {
                    size: 12.0,
                    weight: Weight::Regular,
                    mono: false,
                },
                color: ThemeColor::Text.opaque(),
                max_lines: 1,
                align: Align::Left,
            }],
            hits: vec![],
            fields: vec![],
            spinner: None,
            tips: vec![],
            scroll: None,
            shadow: None,
        }
    }

    fn scene(nodes: Vec<Node>) -> Scene {
        Scene {
            size: Size { w: 100.0, h: 100.0 },
            nodes,
            images: HashMap::new(),
        }
    }

    #[test]
    fn identical_scenes_have_no_changes() {
        let s = scene(vec![node("a", 0.0, "x"), node("b", 5.0, "y")]);
        assert_eq!(changed_nodes(&s, &s.clone()), SceneDiff::default());
    }

    #[test]
    fn reports_added_removed_and_what_changed() {
        let old = scene(vec![
            node("a", 0.0, "x"),
            node("b", 0.0, "y"),
            node("c", 0.0, "z"),
        ]);
        let new = scene(vec![
            node("a", 0.0, "x"),
            node("b", 7.0, "y"),
            node("c", 0.0, "changed"),
            node("d", 0.0, "n"),
        ]);
        let diff = changed_nodes(&old, &new);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].key, "d".into());
        assert!(diff.removed.is_empty());
        let flags: Vec<_> = diff
            .updated
            .iter()
            .map(|u| (u.node.key.0.as_str(), u.frame_changed, u.cmds_changed))
            .collect();
        assert_eq!(flags, [("b", true, false), ("c", false, true)]);
        let gone = changed_nodes(&new, &old);
        assert_eq!(gone.removed, [&NodeKey::from("d")]);
    }
    fn symbol(name: &str, x: f64, token: ThemeColor) -> DrawCmd {
        DrawCmd::Symbol {
            name: name.into(),
            rect: Rect::new(x, 0.0, 10.0, 10.0),
            color: Color { token, alpha: 1.0 },
            weight: Weight::Regular,
        }
    }

    #[test]
    fn a_recolour_is_colors_only() {
        let a = [symbol("circle", 0.0, ThemeColor::Text)];
        let b = [symbol("circle", 0.0, ThemeColor::Green)];
        assert!(colors_only_changed(&a, &b));
        assert!(!colors_only_changed(&a, &a));
        assert!(!colors_only_changed(
            &a,
            &[symbol("circle", 5.0, ThemeColor::Green)]
        ));
        assert!(!colors_only_changed(&a, &[]));
    }

    #[test]
    fn only_a_new_symbol_appears() {
        let old = [symbol("sparkle", 0.0, ThemeColor::Text)];
        let new = [
            symbol("sparkle", 0.0, ThemeColor::Text),
            symbol("sparkle", 20.0, ThemeColor::Text),
            symbol("circle", 40.0, ThemeColor::Text),
        ];
        assert_eq!(
            appeared_symbols(&old, &new, "sparkle"),
            vec![Rect::new(20.0, 0.0, 10.0, 10.0)]
        );
        assert!(appeared_symbols(&new, &new, "sparkle").is_empty());
    }
}
