import AppKit
import Testing
@testable import ClaudeSessionsKit

private let scr = NSRect(x: 0, y: 0, width: 1512, height: 982)

@Test func anchorNearScreenRegions() {
    #expect(Anchor.near(CGPoint(x: 1400, y: 950), in: scr) == Anchor(h: .trailing, up: false))
    #expect(Anchor.near(CGPoint(x: 100, y: 50), in: scr) == Anchor(h: .leading, up: true))
    #expect(Anchor.near(CGPoint(x: 756, y: 950), in: scr) == Anchor(h: .trailing, up: false))
    #expect(Anchor.near(CGPoint(x: 600, y: 950), in: scr) == Anchor(h: .leading, up: false))
    #expect(Anchor.near(CGPoint(x: 697, y: 950), in: scr) == Anchor(h: .trailing, up: false))
    #expect(Anchor.near(CGPoint(x: 696, y: 950), in: scr) == Anchor(h: .leading, up: false))
}

@Test func anchorNearMiddleWithoutRoomOpensRight() {
    let narrow = NSRect(x: 0, y: 0, width: 1280, height: 800)
    #expect(Anchor.near(CGPoint(x: 640, y: 700), in: narrow) == Anchor(h: .leading, up: false))
}

@Test func anchorNearUsesScreenMinX() {
    #expect(Anchor.near(CGPoint(x: ext.minX + 900, y: ext.maxY - 50), in: ext) == Anchor(h: .trailing, up: false))
    let narrow = NSRect(x: 1512, y: 0, width: 1280, height: 800)
    #expect(Anchor.near(CGPoint(x: 1512 + 600, y: 750), in: narrow) == Anchor(h: .leading, up: false))
}

@Test(arguments: [Anchor(h: .leading, up: false), Anchor(h: .leading, up: true), Anchor(h: .trailing, up: true)])
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
    #expect(Layout.migrate(pillX: 40, from: 232, to: 124, anchor: Anchor(h: .leading)) == 40)
}

@Test func migrateNoOpWhenWidthUnchanged() {
    #expect(Layout.migrate(pillX: 1388, from: 124, to: 124, anchor: Anchor(h: .trailing)) == 1388)
}

@Test func collapsedWidth() {
    #expect(Layout.collapsed == CGSize(width: 126, height: 32))
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
    withLinks.menu = [LinkSection(title: "PR", items: [LinkItem(text: "PR #1", url: URL(string: "https://x.y/pull/1")!)])]
    #expect(Layout.height(of: withLinks) == CGFloat(92 + 22))
}

@Test func heightCountsMenuRowWithoutLinks() {
    var s = session("a", "busy", nil)
    s.menu = [LinkSection(title: "PR", items: [LinkItem(text: "PR #1  ·  mở", url: URL(string: "https://x.y/pull/1")!)])]
    s.prs = [prStatus(id: 1)]
    #expect(s.links.isEmpty)
    #expect(Layout.height(of: s) == CGFloat(36 + 22 + 22))
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
    #expect(Layout.settledPillOrigin(card: NSRect(x: 992, y: 582, width: 520, height: 400), screen: scr) == CGPoint(x: 1386, y: 950))
    #expect(Layout.settledPillOrigin(card: NSRect(x: 0, y: 20, width: 520, height: 400), screen: scr) == CGPoint(x: 0, y: 20))
}

@Test func settledPillOriginCollapsedIsIdentity() {
    for o in [CGPoint(x: 1370, y: 950), CGPoint(x: 100, y: 50), CGPoint(x: 694, y: 950)] {
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
    let card = CGSize(width: Layout.expandedWidth, height: 400)
    #expect(pillRect(Layout.clampDrag(windowOrigin: right, windowSize: size, visible: card, anchor: down, screen: scr), card, down).maxX == scr.maxX)
    #expect(Layout.clampDrag(windowOrigin: inside, windowSize: size, visible: c, anchor: down, screen: scr) == inside)
}

@Test func relativeOriginKeepsEdgeDistances() {
    let t = Anchor(h: .trailing, up: false)
    #expect(Layout.relativeOrigin(CGPoint(x: scr.maxX - 126 - 16, y: scr.maxY - 32 - 10), from: scr, to: ext, anchor: t)
        == CGPoint(x: ext.maxX - 142, y: ext.maxY - 42))
    let l = Anchor(h: .leading, up: true)
    #expect(Layout.relativeOrigin(CGPoint(x: scr.minX + 20, y: scr.minY + 30), from: scr, to: ext, anchor: l)
        == CGPoint(x: ext.minX + 20, y: ext.minY + 30))
    let ld = Anchor(h: .leading, up: false)
    #expect(Layout.relativeOrigin(CGPoint(x: scr.minX + 40, y: scr.maxY - 50), from: scr, to: ext, anchor: ld)
        == CGPoint(x: ext.minX + 40, y: ext.maxY - 50))
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

private func prStatus(id: Int, state: String = "OPEN", reviewers: Int = 0, approved: Int = 0, changes: Int = 0, comments: Int = 0,
                      pipeline: Pipeline? = nil, lastOther: PRComment? = nil) -> PRStatus {
    PRStatus(ref: PRRef(workspace: "w", repo: "r", id: id), url: URL(string: "https://bitbucket.org/w/r/pull-requests/\(id)")!,
             state: state, reviewers: reviewers, approved: approved, changesRequested: changes, comments: comments,
             commit: nil, sourceRepo: nil, branch: nil, pipeline: pipeline, lastOther: lastOther)
}

@Test func prLineText() {
    let now = Date(timeIntervalSince1970: 1_800_000_000)
    let pr = prStatus(id: 7571, reviewers: 5, changes: 1, comments: 23, pipeline: .failed,
                      lastOther: PRComment(name: "Tin Tran", at: now - 180))
    #expect(prLine(pr, now: now, stale: now - 420) == "PR #7571 · ✗ 1 yêu cầu sửa · ✓ 0/5 · pipeline ✗ · 💬 23 · Tin Tran 3m · cũ 7m")
    #expect(prLine(pr, now: now, stale: nil) == "PR #7571 · ✗ 1 yêu cầu sửa · ✓ 0/5 · pipeline ✗ · 💬 23 · Tin Tran 3m")
    #expect(prLine(prStatus(id: 9), now: now, stale: nil) == "PR #9")
    #expect(prLine(prStatus(id: 9, pipeline: .passed), now: now, stale: nil) == "PR #9 · pipeline ✓")
    #expect(prLine(prStatus(id: 9, pipeline: .running), now: now, stale: nil) == "PR #9 · pipeline …")
    #expect(prLine(prStatus(id: 7567, state: "MERGED", reviewers: 2), now: now, stale: nil) == "PR #7567 · đã merge")
    #expect(prLine(prStatus(id: 7567, state: "DECLINED"), now: now, stale: nil) == "PR #7567 · đã đóng")
}

@Test func heightCountsPRLines() {
    var s = session("a", "idle", "hi")
    let base = Layout.height(of: s)
    s.prs = [prStatus(id: 1), prStatus(id: 2)]
    #expect(Layout.height(of: s) == base + 44)
    s.prs = [prStatus(id: 1, state: "MERGED"), prStatus(id: 2)]
    #expect(Layout.height(of: s) == base + 40)
}

@Test func editMenuHandlesClipboardShortcuts() {
    let menu = editMainMenu()
    for key in ["x", "c", "v", "a", "z"] {
        let e = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: .command, timestamp: 0, windowNumber: 0,
                                 context: nil, characters: key, charactersIgnoringModifiers: key, isARepeat: false, keyCode: 0)!
        #expect(menu.performKeyEquivalent(with: e), "Cmd-\(key)")
    }
}

@Test func prLineShowsRabbit() {
    let now = Date(timeIntervalSince1970: 1_800_000_000)
    var pr = prStatus(id: 9, pipeline: .passed)
    pr.rabbit = .reviewed(findings: 2)
    #expect(prLine(pr, now: now, stale: nil) == "PR #9 · pipeline ✓ · 🐰 2 góp ý")
    var closed = prStatus(id: 9, state: "MERGED")
    closed.rabbit = .reviewed(findings: 2)
    #expect(!prLine(closed, now: now, stale: nil).contains("🐰"))
}

@Test func expandedHeightGrowsWithOpenList() {
    var s = session("a", "idle", "hi")
    let url = URL(string: "https://x.y/1")!
    s.menu = [LinkSection(title: "Nhánh", items: [LinkItem(text: "A", url: url)]),
              LinkSection(title: "PR", items: [LinkItem(text: "B", url: url), LinkItem(text: "C", url: url)])]
    #expect(Layout.expanded([s], open: [s.id]).height == Layout.expanded([s]).height + 3 * 22 + 2 * 16)
}
