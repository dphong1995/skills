import AppKit
import Testing
@testable import ClaudeSessionsKit

private let scr = NSRect(x: 0, y: 0, width: 1512, height: 982)

@Test func anchorNearScreenRegions() {
    #expect(Anchor.near(CGPoint(x: 1400, y: 950), in: scr) == Anchor(h: .trailing, up: false))
    #expect(Anchor.near(CGPoint(x: 100, y: 50), in: scr) == Anchor(h: .leading, up: true))
    #expect(Anchor.near(CGPoint(x: 756, y: 950), in: scr) == Anchor(h: .center, up: false))
}

@Test(arguments: [Anchor(h: .leading, up: false), Anchor(h: .center, up: true), Anchor(h: .trailing, up: true)])
func canvasPillRoundTrip(a: Anchor) {
    let o = CGPoint(x: 600, y: 300)
    let f = Layout.canvasFrame(pillOrigin: o, anchor: a)
    #expect(Layout.pill(in: f, size: Layout.collapsed, anchor: a).origin == o)
}

@Test func draggedPastCornerIsClampedAndExpandedStaysOnScreen() {
    let a = Anchor(h: .trailing, up: false)
    let corner = Layout.canvasFrame(pillOrigin: Layout.clamp(pillOrigin: CGPoint(x: 1500, y: 975), to: scr), anchor: a)
    let open = Layout.pill(in: corner, size: Layout.expanded((0..<6).map { session("s\($0)", "idle", "hi") }), anchor: a)
    #expect(scr.contains(open))
}

private func session(_ id: String, _ status: String, _ text: String?) -> Session {
    Session(id: id, name: id, cwd: "/x", status: status, waitingFor: nil, since: nil, lastReply: text)
}

@Test func rowHeights() {
    #expect(Layout.height(of: session("a", "busy", "hi")) == 36)
    #expect(Layout.height(of: session("a", "idle", "hi")) == 64)
    #expect(Layout.height(of: session("a", "idle", nil)) == 36)
}

@Test func expandedSumsHeights() {
    #expect(Layout.expanded([session("a", "busy", nil), session("b", "idle", "hi")]).height == CGFloat(146))
    #expect(Layout.expanded([]).height == CGFloat(82))
}

@Test func isDragThreshold() {
    let o = CGPoint.zero
    #expect(!Layout.isDrag(from: o, to: CGPoint(x: 3, y: 0)))
    #expect(!Layout.isDrag(from: o, to: CGPoint(x: 4, y: 0)))
    #expect(Layout.isDrag(from: o, to: CGPoint(x: 3, y: 3)))
    #expect(Layout.isDrag(from: o, to: CGPoint(x: 5, y: 0)))
}

@Test func sizeUsesMeasuredHeight() {
    let store = Store(dir: makeTempDir(), projects: makeTempDir())
    store.hovering = true
    store.measuredHeight = 200
    #expect(Layout.size(store).height == 200)
    store.measuredHeight = 5000
    #expect(Layout.size(store).height == Layout.canvas.height - 2 * Layout.margin)
    store.measuredHeight = nil
    #expect(Layout.size(store).height == Layout.expanded(store.sessions).height)
}
