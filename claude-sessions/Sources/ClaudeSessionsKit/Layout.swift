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
    static let expandedWidth: CGFloat = 520
    static let canvas = CGSize(width: expandedWidth + margin * 2, height: 640)
    static var maxExpandedHeight: CGFloat { canvas.height - 2 * margin }
    static let rowHeight: CGFloat = 36
    static let replyRowHeight: CGFloat = 92
    static let todoHeight: CGFloat = 15
    static let headerHeight: CGFloat = 32
    /// Padding inside the list's scroll content, on the header's side and the far side; `listHeight` is measured with it.
    static let listPaddingNear: CGFloat = 4
    static let listPaddingFar: CGFloat = 10
    static let listPadding = listPaddingNear + listPaddingFar
    static var maxListHeight: CGFloat { maxExpandedHeight - headerHeight }
    public static let collapsed = CGSize(width: 104, height: headerHeight)
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

    /// Pill origin after a drop: the dropped card stays put and the pill takes the card's corner for the anchor its centre implies.
    public static func settledPillOrigin(card: NSRect, screen: NSRect) -> CGPoint {
        // pill(in:) expects the canvas, which is the card grown by `margin`.
        pill(in: card.insetBy(dx: -margin, dy: -margin), size: collapsed,
             anchor: Anchor.near(CGPoint(x: card.midX, y: card.midY), in: screen)).origin
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

    static func height(of s: Session) -> CGFloat {
        (s.isFinished && s.lastReply != nil ? replyRowHeight : rowHeight) + (s.todo != nil ? todoHeight : 0) + (s.activity != nil ? todoHeight : 0)
            + (s.title.count > 60 ? 16 : 0) + (s.links.isEmpty ? 0 : 22)
    }

    static func expanded(_ sessions: [Session]) -> CGSize {
        let rows = sessions.isEmpty ? rowHeight : sessions.reduce(0) { $0 + height(of: $1) }
        return CGSize(width: expandedWidth, height: headerHeight + listPadding + rows)
    }

    public static func size(_ store: Store) -> CGSize {
        guard store.expanded else { return collapsed }
        let h = store.measuredHeight ?? expanded(store.sessions).height
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
}
