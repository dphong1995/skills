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

@Test func migrateShiftsByAnchor() {
    #expect(Layout.migrate(pillX: 1280, from: 232, to: 124, anchor: Anchor(h: .trailing)) == 1388)
    #expect(Layout.migrate(pillX: 600, from: 232, to: 124, anchor: Anchor(h: .center)) == 654)
    #expect(Layout.migrate(pillX: 40, from: 232, to: 124, anchor: Anchor(h: .leading)) == 40)
}

@Test func migrateNoOpWhenWidthUnchanged() {
    #expect(Layout.migrate(pillX: 1388, from: 124, to: 124, anchor: Anchor(h: .trailing)) == 1388)
}

@Test func collapsedWidth() {
    #expect(Layout.collapsed == CGSize(width: 104, height: 32))
}

private func session(_ id: String, _ status: String, _ text: String?) -> Session {
    Session(id: id, name: id, cwd: "/x", status: status, waitingFor: nil, since: nil, lastReply: text)
}

@Test func rowHeights() {
    #expect(Layout.height(of: session("a", "busy", "hi")) == 36)
    #expect(Layout.height(of: session("a", "idle", "hi")) == 92)
    #expect(Layout.height(of: session("a", "idle", nil)) == 36)
    var longTitle = session("a", "busy", nil)
    longTitle.firstPrompt = String(repeating: "x", count: 100)
    #expect(Layout.height(of: longTitle) == 52)
    var withTodo = session("a", "busy", nil)
    withTodo.todo = Todo(done: 1, total: 4, current: "x")
    #expect(Layout.height(of: withTodo) == CGFloat(36 + 15))
    var withActivity = session("a", "busy", nil)
    withActivity.activity = "Bash: ls"
    #expect(Layout.height(of: withActivity) == CGFloat(36 + 15))
    var withLinks = session("a", "idle", "hi")
    withLinks.links = [SessionLink(label: "PR #1", url: URL(string: "https://x.y/pull/1")!)]
    #expect(Layout.height(of: withLinks) == CGFloat(92 + 22))
}

@Test func expandedSumsHeights() {
    #expect(Layout.expanded([session("a", "busy", nil), session("b", "idle", "hi")]).height == CGFloat(174))
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
    let store = makeStore(dir: makeTempDir())
    let panel = PanelState()
    panel.hovering = true
    panel.measuredHeight = 200
    #expect(Layout.size(store, panel).height == 200)
    panel.measuredHeight = 5000
    #expect(Layout.size(store, panel).height == Layout.canvas.height - 2 * Layout.margin)
    panel.measuredHeight = nil
    #expect(Layout.size(store, panel).height == Layout.expanded(store.sessions).height)
}

@Test func commandFitThresholds() {
    #expect(Layout.commandFit("npm test") == .short)
    // 4 lines, 240 characters including the 3 newlines
    let four = [59, 59, 59, 60].map { String(repeating: "a", count: $0) }.joined(separator: "\n")
    #expect(four.count == 240 && Layout.commandFit(four) == .short)
    #expect(Layout.commandFit(Array(repeating: "a", count: 5).joined(separator: "\n")) == .long)
    #expect(Layout.commandFit(String(repeating: "a", count: 241)) == .long)
    #expect(Layout.commandFit(Array(repeating: "a", count: 13).joined(separator: "\n")) == .tooLong)
    #expect(Layout.commandFit(String(repeating: "a", count: 1201)) == .tooLong)
}

@Test func dragOriginFollowsPointer() {
    #expect(Layout.dragOrigin(start: CGPoint(x: 100, y: 200), mouseDown: CGPoint(x: 150, y: 230), mouse: CGPoint(x: 10, y: 900)) == CGPoint(x: -40, y: 870))
}

@Test func settledPillOriginKeepsDroppedCard() {
    #expect(Layout.settledPillOrigin(card: NSRect(x: 0, y: 500, width: 520, height: 400), screen: scr) == CGPoint(x: 0, y: 868))
    #expect(Layout.settledPillOrigin(card: NSRect(x: 992, y: 582, width: 520, height: 400), screen: scr) == CGPoint(x: 1408, y: 950))
    #expect(Layout.settledPillOrigin(card: NSRect(x: 0, y: 20, width: 520, height: 400), screen: scr) == CGPoint(x: 0, y: 20))
}

@Test func settledPillOriginCollapsedIsIdentity() {
    for o in [CGPoint(x: 1388, y: 950), CGPoint(x: 100, y: 50), CGPoint(x: 694, y: 950)] {
        let r = NSRect(origin: o, size: Layout.collapsed)
        #expect(Layout.settledPillOrigin(card: r, screen: scr) == o)
    }
}

private let ext = NSRect(x: 1512, y: 0, width: 2560, height: 1440)

@Test func clampDragStopsVisibleCardAtEdges() {
    let size = Layout.canvas
    let down = Anchor(h: .trailing, up: false)
    let up = Anchor(h: .trailing, up: true)
    func pillRect(_ o: CGPoint, _ v: CGSize, _ a: Anchor) -> NSRect { Layout.pill(in: NSRect(origin: o, size: size), size: v, anchor: a) }
    let c = Layout.collapsed
    let inside = CGPoint(x: 600, y: 300)
    // Past maxX: pill maxX is 50 beyond the screen edge.
    let right = CGPoint(x: scr.maxX + 50 + Layout.margin - size.width, y: inside.y)
    #expect(pillRect(Layout.clampDrag(windowOrigin: right, windowSize: size, visible: c, anchor: down, screen: scr), c, down).maxX == scr.maxX)
    let left = CGPoint(x: -50 - (size.width - Layout.margin - c.width), y: inside.y)
    #expect(pillRect(Layout.clampDrag(windowOrigin: left, windowSize: size, visible: c, anchor: down, screen: scr), c, down).minX == scr.minX)
    let top = CGPoint(x: inside.x, y: scr.maxY + 50 + Layout.margin - size.height)
    #expect(pillRect(Layout.clampDrag(windowOrigin: top, windowSize: size, visible: c, anchor: down, screen: scr), c, down).maxY == scr.maxY)
    let bottom = CGPoint(x: inside.x, y: -50 - Layout.margin)
    #expect(pillRect(Layout.clampDrag(windowOrigin: bottom, windowSize: size, visible: c, anchor: up, screen: scr), c, up).minY == scr.minY)
    let card = CGSize(width: 520, height: 400)
    #expect(pillRect(Layout.clampDrag(windowOrigin: right, windowSize: size, visible: card, anchor: down, screen: scr), card, down).maxX == scr.maxX)
    #expect(Layout.clampDrag(windowOrigin: inside, windowSize: size, visible: c, anchor: down, screen: scr) == inside)
}

@Test func relativeOriginKeepsEdgeDistances() {
    let t = Anchor(h: .trailing, up: false)
    #expect(Layout.relativeOrigin(CGPoint(x: scr.maxX - 104 - 16, y: scr.maxY - 32 - 10), from: scr, to: ext, anchor: t)
        == CGPoint(x: ext.maxX - 120, y: ext.maxY - 42))
    let l = Anchor(h: .leading, up: true)
    #expect(Layout.relativeOrigin(CGPoint(x: scr.minX + 20, y: scr.minY + 30), from: scr, to: ext, anchor: l)
        == CGPoint(x: ext.minX + 20, y: ext.minY + 30))
    let c = Anchor(h: .center, up: false)
    #expect(Layout.relativeOrigin(CGPoint(x: scr.midX + 40, y: scr.maxY - 50), from: scr, to: ext, anchor: c)
        == CGPoint(x: ext.midX + 40, y: ext.maxY - 50))
}

@Test func relativeOriginClampsToTarget() {
    let o = CGPoint(x: ext.maxX - 2000, y: 100)
    let r = Layout.relativeOrigin(o, from: ext, to: scr, anchor: Anchor(h: .trailing, up: true))
    #expect(r == Layout.clamp(pillOrigin: r, to: scr))
    #expect(r.x >= scr.minX && r.x <= scr.maxX - Layout.collapsed.width)
    #expect(r.y >= scr.minY && r.y <= scr.maxY - Layout.collapsed.height)
}

@Test func legacyOffsetPicksScreenContainingCentre() {
    let r = Layout.legacyOffset(x: 1512 + 300, y: 200, width: Layout.collapsed.width, screens: [scr, ext])
    #expect(r?.index == 1)
    #expect(r?.offset == [300, 200])
}

@Test func legacyOffsetFallsBackToFirstScreenAndClamps() {
    let r = Layout.legacyOffset(x: 9000, y: 9000, width: Layout.collapsed.width, screens: [scr, ext])
    #expect(r?.index == 0)
    #expect(r?.offset == [Double(scr.maxX - Layout.collapsed.width), Double(scr.maxY - Layout.collapsed.height)])
    #expect(Layout.legacyOffset(x: 0, y: 0, width: 104, screens: []) == nil)
}

@Test func legacyOffsetShiftsLegacyWidth() {
    let x: CGFloat = scr.maxX - 232 - 14
    let r = Layout.legacyOffset(x: x, y: 800, width: 232, screens: [scr])
    #expect(r?.offset == [Double(x + 232 - Layout.collapsed.width), 800])
}

@Test func offsetRoundTripsAcrossOriginChange() {
    let o = CGPoint(x: 1512 + 300, y: 900)
    let off = Layout.offset(of: o, in: ext)
    #expect(off == [300, 900])
    #expect(Layout.origin(offset: off, in: NSRect(x: 0, y: 0, width: 2560, height: 1440)) == CGPoint(x: 300, y: 900))
    #expect(Layout.origin(offset: [1], in: ext) == nil)
}
