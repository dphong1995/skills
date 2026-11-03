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
    static let canvas = CGSize(width: 420 + margin * 2, height: 640)
    static let rowHeight: CGFloat = 36
    static let replyRowHeight: CGFloat = 64
    static let headerHeight: CGFloat = 32
    public static let collapsed = CGSize(width: 190, height: headerHeight)
    static let dragThreshold: CGFloat = 4

    static func isDrag(from a: CGPoint, to b: CGPoint) -> Bool {
        hypot(b.x - a.x, b.y - a.y) > dragThreshold
    }

    static func height(of s: Session) -> CGFloat {
        s.isFinished && s.lastReply != nil ? replyRowHeight : rowHeight
    }

    static func expanded(_ sessions: [Session]) -> CGSize {
        let rows = sessions.isEmpty ? rowHeight : sessions.reduce(0) { $0 + height(of: $1) }
        return CGSize(width: 420, height: headerHeight + 4 + rows + 10)
    }

    public static func size(_ store: Store) -> CGSize {
        guard store.expanded else { return collapsed }
        let h = store.measuredHeight ?? expanded(store.sessions).height
        return CGSize(width: 420, height: min(h, canvas.height - 2 * margin))
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
