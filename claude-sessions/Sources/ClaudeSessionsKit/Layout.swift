import AppKit
import SwiftUI

public enum HAnchor { case leading, center, trailing }

public struct Anchor: Equatable {
    public var h: HAnchor = .trailing
    public var up = false

    var alignment: Alignment {
        switch (h, up) {
        case (.leading, false): .topLeading
        case (.center, false): .top
        case (.trailing, false): .topTrailing
        case (.leading, true): .bottomLeading
        case (.center, true): .bottom
        case (.trailing, true): .bottomTrailing
        }
    }

    public static func near(_ p: CGPoint, in s: NSRect) -> Anchor {
        let third = s.width / 3
        let h: HAnchor = p.x < s.minX + third ? .leading : p.x > s.maxX - third ? .trailing : .center
        return Anchor(h: h, up: p.y < s.midY)
    }
}
public enum Layout {
    static let margin: CGFloat = 14
    static let expandedWidth: CGFloat = 760
    static let canvas = CGSize(width: expandedWidth + margin * 2, height: 640)
    static var maxExpandedHeight: CGFloat { canvas.height - 2 * margin }
    static let rowHeight: CGFloat = 36
    /// How long the mouse must rest on a fresh row before its mark clears.
    static let seenDwell: Duration = .seconds(1)
    static let replyRowHeight: CGFloat = 92
    static let todoHeight: CGFloat = 15
    static let headerHeight: CGFloat = 32
    /// Padding inside the list's scroll content, on the header's side and the far side; `listHeight` is measured with it.
    static let listPaddingNear: CGFloat = 4
    static let listPaddingFar: CGFloat = 10
    static let listPadding = listPaddingNear + listPaddingFar
    static var maxListHeight: CGFloat { maxExpandedHeight - headerHeight }
    public static let collapsed = CGSize(width: 126, height: headerHeight)
    /// Background opacity of the expanded card; the last of `bgOpacities`.
    static let cardOpacity: Double = 0.92
    public static let bgOpacities: [Double] = [0, 0.25, 0.5, 0.75, cardOpacity]
    static let dragThreshold: CGFloat = 4

    /// Shifts a saved pill x when the collapsed width changed, keeping the anchored edge (or centre) in place.
    public static func migrate(pillX x: CGFloat, from old: CGFloat, to new: CGFloat, anchor: Anchor) -> CGFloat {
        switch anchor.h {
        case .trailing: x + (old - new)
        case .center: x + (old - new) / 2
        case .leading: x
        }
    }
    static func dragOrigin(start: CGPoint, mouseDown: CGPoint, mouse: CGPoint) -> CGPoint {
        CGPoint(x: start.x + mouse.x - mouseDown.x, y: start.y + mouse.y - mouseDown.y)
    }

    /// Pill origin after a drop: the pill stays where it was shown (in the window at `clamped`, the exact origin given
    /// to the window; its frame is rounded to whole points, so it must not be read back), except that on an axis where
    /// the drag moved toward a screen edge and pushed the card against it (`clamped` short of `raw`) it goes to that edge.
    /// `place` then re-anchors it.
    public static func settledPillOrigin(start: CGPoint, raw: CGPoint, clamped: CGPoint, size: CGSize, anchor: Anchor, screen s: NSRect) -> CGPoint {
        var o = pill(in: NSRect(origin: clamped, size: size), size: collapsed, anchor: anchor).origin
        if raw.x < clamped.x && raw.x < start.x { o.x = s.minX } else if raw.x > clamped.x && raw.x > start.x { o.x = s.maxX - collapsed.width }
        if raw.y < clamped.y && raw.y < start.y { o.y = s.minY } else if raw.y > clamped.y && raw.y > start.y { o.y = s.maxY - collapsed.height }
        return o
    }

    static func isDrag(from a: CGPoint, to b: CGPoint) -> Bool {
        hypot(b.x - a.x, b.y - a.y) > dragThreshold
    }

    public enum Fit { case short, long, tooLong }

    static func commandFit(_ s: String) -> Fit {
        let lines = s.split(separator: "\n", omittingEmptySubsequences: false).count
        if lines > 12 || s.count > 1200 { return .tooLong }
        return lines <= 4 && s.count <= 240 ? .short : .long
    }

    static let linkRowHeight: CGFloat = 22
    static let linkSectionHeight: CGFloat = 16
    static let cardGap: CGFloat = 4

    static func height(of s: Session, open: Bool = false) -> CGFloat {
        (s.isFinished && s.lastReply != nil ? replyRowHeight : rowHeight) + (s.todo != nil ? todoHeight : 0) + (s.activity != nil ? todoHeight : 0)
            + (s.title.count > 60 ? 16 : 0) + (s.menu.isEmpty ? 0 : 22) + s.prs.reduce(0) { $0 + ($1.isOpen ? 22 : 18) }
            + (open ? CGFloat(s.menuCount) * linkRowHeight + CGFloat(s.menu.count) * linkSectionHeight + (s.menuNeedsJira ? linkRowHeight : 0) : 0)
    }

    static func expanded(_ sessions: [Session], open: Set<String> = []) -> CGSize {
        let rows = sessions.isEmpty ? rowHeight : sessions.reduce(0) { $0 + height(of: $1, open: open.contains($1.id)) }
        return CGSize(width: expandedWidth, height: headerHeight + listPadding + rows + cardGap * CGFloat(max(sessions.count - 1, 0)))
    }

    public static func size(_ store: Store, _ panel: PanelState) -> CGSize {
        guard panel.expanded(store) else { return collapsed }
        let h = panel.measuredHeight ?? expanded(store.sessions, open: panel.openMenus).height
        return CGSize(width: expandedWidth, height: min(h, maxExpandedHeight))
    }

    public static func pill(in f: NSRect, size: CGSize, anchor: Anchor) -> NSRect {
        let x: CGFloat
        switch anchor.h {
        case .leading: x = f.minX + margin
        case .center: x = f.midX - size.width / 2
        case .trailing: x = f.maxX - margin - size.width
        }
        let y = anchor.up ? f.minY + margin : f.maxY - margin - size.height
        return NSRect(x: x, y: y, width: size.width, height: size.height)
    }

    public static func canvasFrame(pillOrigin o: CGPoint, anchor: Anchor) -> NSRect {
        let c = collapsed
        let x: CGFloat
        switch anchor.h {
        case .leading: x = o.x - margin
        case .center: x = o.x + c.width / 2 - canvas.width / 2
        case .trailing: x = o.x + c.width + margin - canvas.width
        }
        let y = anchor.up ? o.y - margin : o.y + c.height + margin - canvas.height
        return NSRect(x: x, y: y, width: canvas.width, height: canvas.height)
    }

    public static func clamp(pillOrigin o: CGPoint, to s: NSRect) -> CGPoint {
        CGPoint(x: min(max(o.x, s.minX), s.maxX - collapsed.width),
                y: min(max(o.y, s.minY), s.maxY - collapsed.height))
    }

    /// Window origin shifted by the smallest dx, dy that keeps the visible pill or card inside `screen`.
    public static func clampDrag(windowOrigin: CGPoint, windowSize: CGSize, visible: CGSize, anchor: Anchor, screen: NSRect) -> CGPoint {
        let r = pill(in: NSRect(origin: windowOrigin, size: windowSize), size: visible, anchor: anchor)
        let dx = r.minX < screen.minX ? screen.minX - r.minX : r.maxX > screen.maxX ? screen.maxX - r.maxX : 0
        let dy = r.minY < screen.minY ? screen.minY - r.minY : r.maxY > screen.maxY ? screen.maxY - r.maxY : 0
        return CGPoint(x: windowOrigin.x + dx, y: windowOrigin.y + dy)
    }

    /// A collapsed pill origin on screen `a` mapped to screen `b`, keeping the distance to the anchored edges.
    public static func relativeOrigin(_ o: CGPoint, from a: NSRect, to b: NSRect, anchor: Anchor) -> CGPoint {
        let x: CGFloat
        switch anchor.h {
        case .trailing: x = b.maxX - (a.maxX - o.x)
        case .leading: x = b.minX + (o.x - a.minX)
        case .center: x = b.midX + (o.x - a.midX)
        }
        let y = anchor.up ? b.minY + (o.y - a.minY) : b.maxY - (a.maxY - o.y)
        return clamp(pillOrigin: CGPoint(x: x, y: y), to: b)
    }

    public static func pillCentre(_ o: CGPoint) -> CGPoint {
        CGPoint(x: o.x + collapsed.width / 2, y: o.y + collapsed.height / 2)
    }

    /// A pill origin saved by the old single-pill keys, as the screen (index into `screens`) holding it and its offset there.
    /// A pill saved at another width is shifted to the current one; a pill on no screen goes to the first.
    public static func legacyOffset(x: CGFloat, y: CGFloat, width: CGFloat, screens: [NSRect]) -> (index: Int, offset: [Double])? {
        guard !screens.isEmpty else { return nil }
        let centre = CGPoint(x: x + width / 2, y: y + collapsed.height / 2)
        let index = screens.firstIndex { $0.contains(centre) } ?? 0
        let f = screens[index]
        var x = x
        if width != collapsed.width {
            x = migrate(pillX: x, from: width, to: collapsed.width, anchor: Anchor.near(centre, in: f))
        }
        return (index, offset(of: clamp(pillOrigin: CGPoint(x: x, y: y), to: f), in: f))
    }

    public static func offset(of o: CGPoint, in screen: NSRect) -> [Double] {
        [o.x - screen.minX, o.y - screen.minY]
    }

    public static func origin(offset: [Double], in screen: NSRect) -> CGPoint? {
        offset.count == 2 ? CGPoint(x: screen.minX + offset[0], y: screen.minY + offset[1]) : nil
    }
}
