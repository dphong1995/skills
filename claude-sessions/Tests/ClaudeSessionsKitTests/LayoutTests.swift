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
    #expect(Layout.collapsed == CGSize(width: 124, height: 32))
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
    store.hovering = true
    store.measuredHeight = 200
    #expect(Layout.size(store).height == 200)
    store.measuredHeight = 5000
    #expect(Layout.size(store).height == Layout.canvas.height - 2 * Layout.margin)
    store.measuredHeight = nil
    #expect(Layout.size(store).height == Layout.expanded(store.sessions).height)
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
    #expect(Layout.settledPillOrigin(card: NSRect(x: 992, y: 582, width: 520, height: 400), screen: scr) == CGPoint(x: 1388, y: 950))
    #expect(Layout.settledPillOrigin(card: NSRect(x: 0, y: 20, width: 520, height: 400), screen: scr) == CGPoint(x: 0, y: 20))
}

@Test func settledPillOriginCollapsedIsIdentity() {
    for o in [CGPoint(x: 1388, y: 950), CGPoint(x: 100, y: 50), CGPoint(x: 694, y: 950)] {
        let r = NSRect(origin: o, size: Layout.collapsed)
        #expect(Layout.settledPillOrigin(card: r, screen: scr) == o)
    }
}
