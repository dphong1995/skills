import AppKit
import Testing
@testable import ClaudeSessionsKit

private let scr = NSRect(x: 0, y: 0, width: 1512, height: 982)

@Test func anchorNearScreenRegions() {
    #expect(Anchor.near(CGPoint(x: 1400, y: 950), in: scr) == Anchor(h: .trailing, up: false))
    #expect(Anchor.near(CGPoint(x: 100, y: 50), in: scr) == Anchor(h: .leading, up: true))
    #expect(Anchor.near(CGPoint(x: 756, y: 950), in: scr) == Anchor(h: .center, up: false))
    #expect(Anchor.near(CGPoint(x: ext.minX + 900, y: ext.maxY - 50), in: ext) == Anchor(h: .center, up: false))
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

@Test func centredCanvasAtInnerEdgeOfMiddleThirdStaysOnScreen() {
    let s = NSRect(x: 0, y: 0, width: 1280, height: 800)
    let centre = CGPoint(x: s.minX + s.width / 3 + 1, y: 400)
    let a = Anchor.near(centre, in: s)
    #expect(a.h == .center)
    let origin = CGPoint(x: centre.x - Layout.collapsed.width / 2, y: centre.y)
    let f = Layout.canvasFrame(pillOrigin: origin, anchor: a)
    #expect(f.minX >= s.minX)
    #expect(Layout.pill(in: f, size: CGSize(width: Layout.expandedWidth, height: 400), anchor: a).minX >= s.minX)
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
    #expect(Layout.collapsed == CGSize(width: 126, height: 32))
}

private func session(_ id: String, _ status: String, _ text: String?) -> Session {
    Session(id: id, name: id, cwd: "/x", status: status, waitingFor: nil, since: nil, lastReply: text)
}

@Test func rowHeights() {
    #expect(Layout.height(of: session("a", "busy", "hi")) == 50)
    #expect(Layout.height(of: session("a", "idle", "hi")) == 92)
    #expect(Layout.height(of: session("a", "idle", nil)) == 50)
    var longTitle = session("a", "busy", nil)
    longTitle.firstPrompt = String(repeating: "x", count: 100)
    #expect(Layout.height(of: longTitle) == 66)
    var withTodo = session("a", "busy", nil)
    withTodo.todo = Todo(done: 1, total: 4, current: "x")
    #expect(Layout.height(of: withTodo) == CGFloat(50 + 15))
    var withActivity = session("a", "busy", nil)
    withActivity.activity = "Bash: ls"
    #expect(Layout.height(of: withActivity) == CGFloat(50 + 15))
    var withLinks = session("a", "idle", "hi")
    withLinks.menu = [LinkSection(title: "PR", items: [LinkItem(text: "PR #1", url: URL(string: "https://x.y/pull/1")!)])]
    #expect(Layout.height(of: withLinks) == CGFloat(92 + 22))
}

@Test func titleWrapThreshold() {
    let short = session("a", "busy", nil)
    var fifty = short
    fifty.firstPrompt = String(repeating: "x", count: 50)
    var fortyEight = short
    fortyEight.firstPrompt = String(repeating: "x", count: 48)
    #expect(Layout.height(of: fifty) == Layout.height(of: short) + 16)
    #expect(Layout.height(of: fortyEight) == Layout.height(of: short))
}

@Test func heightCountsMenuRowWithoutLinks() {
    var s = session("a", "busy", nil)
    s.menu = [LinkSection(title: "PR", items: [LinkItem(text: "PR #1  ·  mở", url: URL(string: "https://x.y/pull/1")!)])]
    s.prs = [prStatus(id: 1)]
    #expect(s.links.isEmpty)
    #expect(Layout.height(of: s) == CGFloat(50 + 22 + 22))
}

@Test func expandedSumsHeights() {
    #expect(Layout.expanded([session("a", "busy", nil), session("b", "idle", "hi")]).height == CGFloat(192))
    #expect(Layout.expanded([]).height == CGFloat(96))
}

@Test func expandedAddsCardGaps() {
    let a = session("a", "busy", nil), b = session("b", "idle", "hi")
    #expect(Layout.expanded([a, b]).height == Layout.height(of: a) + Layout.height(of: b) + Layout.headerHeight + Layout.listPadding + 4)
    #expect(Layout.expanded([a]).height == Layout.height(of: a) + Layout.headerHeight + Layout.listPadding)
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

@Test func settledPillOriginKeepsDroppedPill() {
    let cases: [(Anchor, CGPoint)] = [
        (Anchor(h: .trailing), CGPoint(x: 850, y: 918)),
        (Anchor(h: .leading, up: true), CGPoint(x: 100, y: 50)),
        (Anchor(h: .center), CGPoint(x: 694, y: 918)),
        (Anchor(h: .trailing, up: true), CGPoint(x: 1300, y: 300)),
    ]
    for (a, o) in cases {
        let w = Layout.canvasFrame(pillOrigin: o, anchor: a)
        #expect(Layout.settledPillOrigin(start: w.origin, raw: w.origin, clamped: w.origin, size: w.size, anchor: a, screen: scr) == o)
    }
    // The anchor changes at drop (centre of the pill is in the middle third) but the pill does not move.
    #expect(Anchor.near(Layout.pillCentre(CGPoint(x: 850, y: 918)), in: scr).h == .center)
}

@Test func droppedPillStaysPutAndCardFitsAfterReanchor() {
    let o = CGPoint(x: 850, y: 918)
    let a = Anchor.near(Layout.pillCentre(o), in: scr)
    let canvas = Layout.canvasFrame(pillOrigin: o, anchor: a)
    #expect(Layout.pill(in: canvas, size: Layout.collapsed, anchor: a).origin == o)
    let card = Layout.pill(in: canvas, size: CGSize(width: Layout.expandedWidth, height: 400), anchor: a)
    #expect(card.minX >= scr.minX && card.maxX <= scr.maxX)
}

/// Drag the canvas of a pill at `o` by `(dx, dy)`, clamp it as the app does and settle.
private func drop(_ o: CGPoint, _ a: Anchor, dx: CGFloat, dy: CGFloat, height: CGFloat) -> (settled: CGPoint, shown: CGPoint) {
    let w0 = Layout.canvasFrame(pillOrigin: o, anchor: a)
    let raw = CGPoint(x: w0.origin.x + dx, y: w0.origin.y + dy)
    let clamped = Layout.clampDrag(windowOrigin: raw, windowSize: w0.size, visible: CGSize(width: Layout.expandedWidth, height: height), anchor: a, screen: scr)
    let w = NSRect(origin: clamped, size: w0.size)
    return (Layout.settledPillOrigin(start: w0.origin, raw: raw, clamped: clamped, size: w0.size, anchor: a, screen: scr),
            Layout.pill(in: w, size: Layout.collapsed, anchor: a).origin)
}

@Test func droppedFarFromThePillStillReachesTheLeftEdge() {
    let o = CGPoint(x: 1386, y: 950)
    let a = Anchor.near(Layout.pillCentre(o), in: scr)
    // Grabbed 630 pt left of the pill, cursor stopped at x = 0.
    let r = drop(o, a, dx: -(o.x - 630), dy: 0, height: 400)
    #expect(r.settled.x == 0)
    #expect(Anchor.near(Layout.pillCentre(r.settled), in: scr).h == .leading)
}

@Test func overshootingOnThePillReachesTheLeftEdge() {
    let o = CGPoint(x: 1386, y: 950)
    let a = Anchor.near(Layout.pillCentre(o), in: scr)
    #expect(drop(o, a, dx: -1400, dy: 0, height: 400).settled.x == 0)
}

@Test func droppedPastTheBottomReachesTheBottomEdge() {
    let o = CGPoint(x: 1300, y: 918)
    let a = Anchor(h: .trailing, up: false)
    let r = drop(o, a, dx: 0, dy: -1200, height: 550)
    #expect(r.settled.y == 0)
    #expect(Anchor.near(Layout.pillCentre(r.settled), in: scr).up == true)
}

@Test func dropInsideTheScreenKeepsThePillWhereShown() {
    let o = CGPoint(x: 850, y: 600)
    let a = Anchor.near(Layout.pillCentre(o), in: scr)
    let r = drop(o, a, dx: -100, dy: -50, height: 400)
    #expect(r.settled == r.shown)
}

@Test func horizontalDragOfAnOverflowingCardDoesNotSnapVertically() {
    let o = CGPoint(x: 1300, y: 480)
    let a = Anchor(h: .trailing, up: false)
    #expect(Anchor.near(Layout.pillCentre(o), in: scr) == a)
    let canvas = Layout.canvasFrame(pillOrigin: o, anchor: a)
    let raw = CGPoint(x: canvas.origin.x - 10, y: canvas.origin.y)
    let clamped = Layout.clampDrag(windowOrigin: raw, windowSize: canvas.size, visible: CGSize(width: Layout.expandedWidth, height: 600), anchor: a, screen: scr)
    let shown = Layout.pill(in: NSRect(origin: clamped, size: canvas.size), size: Layout.collapsed, anchor: a).origin
    let settled = Layout.settledPillOrigin(start: canvas.origin, raw: raw, clamped: clamped, size: canvas.size, anchor: a, screen: scr)
    #expect(settled.y == shown.y)
    #expect(settled.y != 0)
}

@Test func fractionalDropInsideTheScreenKeepsThePill() {
    let o = CGPoint(x: 850, y: 600)
    let a = Anchor.near(Layout.pillCentre(o), in: scr)
    let w0 = Layout.canvasFrame(pillOrigin: o, anchor: a)
    let raw = CGPoint(x: w0.origin.x - 100.4, y: w0.origin.y - 50.3)
    let clamped = Layout.clampDrag(windowOrigin: raw, windowSize: w0.size, visible: CGSize(width: Layout.expandedWidth, height: 400), anchor: a, screen: scr)
    let w = NSRect(origin: clamped, size: w0.size)
    let shown = Layout.pill(in: w, size: Layout.collapsed, anchor: a).origin
    #expect(abs(shown.x - 749.6) < 0.001 && abs(shown.y - 549.7) < 0.001)
    #expect(Layout.settledPillOrigin(start: w0.origin, raw: raw, clamped: clamped, size: w0.size, anchor: a, screen: scr) == shown)
    // A frame read back after AppKit's rounding (built by hand here) would not give the shown pill.
    let rounded = NSRect(origin: CGPoint(x: clamped.x.rounded(.down), y: clamped.y.rounded(.down)), size: w0.size)
    #expect(Layout.settledPillOrigin(start: w0.origin, raw: raw, clamped: rounded.origin, size: w0.size, anchor: a, screen: scr) != shown)
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

private func prStatus(id: Int, state: String = "OPEN", reviewers: Int = 0, approved: Int = 0, changes: Int = 0, comments: Int = 0,
                      lastOther: PRComment? = nil) -> PRStatus {
    PRStatus(ref: PRRef(workspace: "w", repo: "r", id: id), url: URL(string: "https://bitbucket.org/w/r/pull-requests/\(id)")!,
             state: state, reviewers: reviewers, approved: approved, changesRequested: changes, comments: comments,
             commit: nil, sourceRepo: nil, branch: nil, lastOther: lastOther)
}

@Test func prLineText() {
    let now = Date(timeIntervalSince1970: 1_800_000_000)
    let pr = prStatus(id: 7571, reviewers: 5, changes: 1, comments: 23,
                      lastOther: PRComment(name: "Tin Tran", at: now - 180))
    #expect(prLine(pr, now: now, stale: now - 420) == "PR #7571 · 1 yêu cầu sửa · 0/5 · 23 · Tin Tran 3m · cũ 7m")
    #expect(prLine(pr, now: now, stale: nil) == "PR #7571 · 1 yêu cầu sửa · 0/5 · 23 · Tin Tran 3m")
    #expect(prLine(prStatus(id: 9), now: now, stale: nil) == "PR #9")
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
    var pr = prStatus(id: 9)
    pr.rabbit = .reviewed(findings: 2)
    #expect(prLine(pr, now: now, stale: nil) == "PR #9 · 2 góp ý")
    var closed = prStatus(id: 9, state: "MERGED")
    closed.rabbit = .reviewed(findings: 2)
    #expect(!prLine(closed, now: now, stale: nil).contains("góp ý"))
}

@Test func expandedHeightGrowsWithOpenList() {
    var s = session("a", "idle", "hi")
    let url = URL(string: "https://x.y/1")!
    s.menu = [LinkSection(title: "Nhánh", items: [LinkItem(text: "A", url: url)]),
              LinkSection(title: "PR", items: [LinkItem(text: "B", url: url), LinkItem(text: "C", url: url)])]
    #expect(Layout.expanded([s], open: [s.id]).height == Layout.expanded([s]).height + 3 * 22 + 2 * 16)
}

@Test func closedGroupHeight() {
    let a = session("a", "idle", "hi")
    let base = Layout.expanded([a]).height
    #expect(Layout.expanded([a], closed: 0).height == base)
    let collapsed = Layout.expanded([a], closed: 2, closedOpen: false).height
    #expect(collapsed == base + Layout.cardGap + Layout.closedRowHeight)
    #expect(Layout.expanded([a], closed: 2, closedOpen: true).height == collapsed + 2 * Layout.closedRowHeight)
}

@Test func questionBlockHeight() {
    let one = feedQuestion()
    // prompt line + 2 options + Other row, then the Gửi row.
    #expect(Layout.otherFieldHeight == 22)
    let perQuestion = Layout.questionLineHeight + 2 * Layout.optionRowHeight + Layout.otherFieldHeight
    #expect(Layout.questionBlockHeight(one) == perQuestion + Layout.questionFooterHeight)
    let two = feedQuestion(questions: one.questions + one.questions)
    #expect(Layout.questionBlockHeight(two) == 2 * perQuestion + Layout.questionFooterHeight)
    var s = session("a", "waiting", "reply text")
    #expect(Layout.height(of: s) == 92)
    s.question = one
    s.questionLive = true
    // The block replaces the reply text's room.
    #expect(Layout.height(of: s) == Layout.rowHeight + Layout.questionBlockHeight(one))
    s.questionLive = false
    #expect(Layout.height(of: s) == Layout.rowHeight + Layout.questionFallbackHeight)
}

@Test func backgroundAgentsAddNoLine() {
    var s = session("a", "idle", "hi")
    let base = Layout.height(of: s)
    s.backgroundAgents = 2
    #expect(Layout.height(of: s) == base)
}

@Test func agentCountMergesForegroundAndBackground() {
    var s = session("a", "idle", "hi")
    #expect(s.agentCount == 0)
    s.backgroundAgents = 2
    #expect(s.agentCount == 2)
    s.backgroundAgents = nil
    s.agents = ["x", "y", "z"]
    #expect(s.agentCount == 3)
}

@Test func backgroundZeroAddsNoLine() {
    var s = session("a", "idle", "hi")
    let base = Layout.height(of: s)
    s.backgroundAgents = 0
    #expect(Layout.height(of: s) == base)
}
