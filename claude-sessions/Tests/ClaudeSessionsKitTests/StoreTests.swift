import Foundation
import AppKit
import Testing
@testable import ClaudeSessionsKit

@Test func emptyDirHasNoSessions() {
    let store = makeStore(dir: makeTempDir())
    store.refresh(at: Date())
    #expect(store.sessions.isEmpty)
}

@Test func deadPidIsFilteredAndFirstSightIsQuiet() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 42, name: "talo6-dct-frontend-d7", cwd: frontend)
    writeSession(dir, "b", pid: me, status: "busy", ageSec: 300, name: "talo6-dct-backend-3f", cwd: backend)
    writeSession(dir, "dead", pid: 999_999, status: "idle", ageSec: 10, name: "dead", cwd: "/tmp")
    store.refresh(at: Date())
    #expect(store.sessions.count == 2)
    #expect(!store.peeking)
}

@Test func busyToIdlePeeks() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    let t0 = Date()
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 42, name: "talo6-dct-frontend-d7", cwd: frontend)
    writeSession(dir, "b", pid: me, status: "busy", ageSec: 300, name: "talo6-dct-backend-3f", cwd: backend)
    store.refresh(at: t0)

    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: frontend)
    store.refresh(at: t0.addingTimeInterval(1))
    store.applyAlert(transitions: store.transitions, prChime: store.prChime, excluding: [], at: t0.addingTimeInterval(1))
    #expect(PanelState().expanded(store))
    #expect(store.chime == .done)
    #expect(store.sessions.first?.id == "a")

    store.refresh(at: t0.addingTimeInterval(7))
    #expect(!store.peeking)
}

@Test func waitingOrdering() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    let t0 = Date()
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 42, name: "talo6-dct-frontend-d7", cwd: frontend)
    writeSession(dir, "b", pid: me, status: "busy", ageSec: 300, name: "talo6-dct-backend-3f", cwd: backend)
    store.refresh(at: t0)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: frontend)
    store.refresh(at: t0.addingTimeInterval(1))

    writeSession(dir, "b", pid: me, status: "waiting", waitingFor: "permission prompt", ageSec: 0, name: "talo6-dct-backend-3f", cwd: backend)
    writeSession(dir, "c", pid: me, status: "waiting", waitingFor: "input needed", ageSec: 75, name: "ocr-service-a1", cwd: "/Users/phong/ocr-service")
    writeSession(dir, "d", pid: me, status: "busy", ageSec: 3700, name: "talo-v2-9c", cwd: "/Users/phong/talos-v2")
    store.refresh(at: t0.addingTimeInterval(8))
    #expect(store.sessions.map(\.id) == ["c", "b", "a", "d"])
    #expect(store.sessions.first { $0.id == "b" }?.isPermissionPrompt == true)
    #expect(store.sessions.first { $0.id == "c" }?.isPermissionPrompt == false)
}

@Test func waitingOrder() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    let t0 = Date()
    writeSession(dir, "idleUnseen", pid: me, status: "idle", ageSec: 10, name: "a", cwd: frontend)
    writeSession(dir, "waiting", pid: me, status: "waiting", waitingFor: "input needed", ageSec: 20, name: "b", cwd: backend)
    writeSession(dir, "perm", pid: me, status: "waiting", waitingFor: "permission prompt", ageSec: 30, name: "c", cwd: backend)
    writeSession(dir, "busy", pid: me, status: "busy", ageSec: 40, name: "d", cwd: backend)
    writeSession(dir, "idleSeen", pid: me, status: "idle", ageSec: 50, name: "e", cwd: backend)
    store.refresh(at: t0)
    store.unseen = ["idleUnseen": NewMark(claude: true), "busy": NewMark(claude: true), "waiting": NewMark(claude: true)]
    #expect(store.waitingOrder() == ["perm", "waiting", "idleUnseen"])
}

@Test func hoverExpands() {
    let store = makeStore(dir: makeTempDir())
    let panel = PanelState()
    panel.hovering = true
    #expect(panel.expanded(store))
}

@Test func secondTurnFinishingPeeksAgain() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    let t0 = Date()
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: frontend)
    store.refresh(at: t0)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: frontend)
    store.refresh(at: t0.addingTimeInterval(1))
    store.refresh(at: t0.addingTimeInterval(30))
    #expect(!store.peeking)

    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: frontend)
    store.refresh(at: t0.addingTimeInterval(31))
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: frontend)
    store.refresh(at: t0.addingTimeInterval(32))
    store.applyAlert(transitions: store.transitions, prChime: store.prChime, excluding: [], at: t0.addingTimeInterval(32))
    #expect(store.peeking)
}

@Test func transitionsRecorded() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    let t0 = Date()
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    writeSession(dir, "b", pid: me, status: "busy", ageSec: 0, name: "b", cwd: backend)
    store.refresh(at: t0)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    writeSession(dir, "b", pid: me, status: "waiting", ageSec: 0, name: "b", cwd: backend)
    store.refresh(at: t0.addingTimeInterval(1))
    #expect(store.transitions == ["a": .done, "b": .needsYou])
    #expect(!store.peeking)
    #expect(store.chime == .needsYou)
    store.refresh(at: t0.addingTimeInterval(2))
    #expect(store.transitions.isEmpty)
}

@Test func alertExcludesFocused() {
    let r = Store.alert(transitions: ["a": .done], prChime: nil, excluding: ["a"])
    #expect(r.chime == nil)
    #expect(!r.peek)
}

@Test func alertKeepsOtherSessions() {
    let r = Store.alert(transitions: ["a": .needsYou, "b": .done], prChime: nil, excluding: ["a"])
    #expect(r.chime == .done)
    #expect(r.peek)
}

@Test func alertKeepsPRChime() {
    let r = Store.alert(transitions: ["a": .done], prChime: .needsYou, excluding: ["a"])
    #expect(r.chime == .needsYou)
    #expect(!r.peek)
}

@Test func alertNeedsYouWins() {
    let r = Store.alert(transitions: ["a": .done, "b": .needsYou], prChime: .done, excluding: [])
    #expect(r.chime == .needsYou)
    #expect(r.peek)
}

@Test func applyAlertPeeks() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    let t0 = Date()
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: t0)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: t0.addingTimeInterval(1))
    let t = t0.addingTimeInterval(1)
    #expect(store.applyAlert(transitions: store.transitions, prChime: store.prChime, excluding: ["a"], at: t) == nil)
    #expect(!store.peeking)
    #expect(store.applyAlert(transitions: store.transitions, prChime: store.prChime, excluding: [], at: t) == .done)
    #expect(store.peeking)
}

@Test func lastReplyOnIdle() {
    let dir = makeTempDir(), projects = makeTempDir()
    let store = makeStore(dir: dir, projects: projects)
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [assistantLine("all done")])
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 5, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.sessions.first?.lastReply == nil)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.sessions.first?.lastReply == "all done")
}

@Test func noRereadWhenUnchanged() {
    let dir = makeTempDir(), projects = makeTempDir()
    let store = makeStore(dir: dir, projects: projects)
    let f = writeTranscript(projects, cwd: frontend, id: "a", lines: [assistantLine("aaaa")])
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    try! Data(assistantLine("bbbb").utf8).write(to: f)
    store.refresh(at: Date())
    #expect(store.sessions.first?.lastReply == "aaaa")
}

@Test func rereadsWhenTranscriptGrows() {
    let dir = makeTempDir(), projects = makeTempDir()
    let store = makeStore(dir: dir, projects: projects)
    let f = writeTranscript(projects, cwd: frontend, id: "a", lines: [assistantLine("first")])
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    try! Data([assistantLine("first"), assistantLine("a longer final text")].joined(separator: "\n").utf8).write(to: f)
    store.refresh(at: Date())
    #expect(store.sessions.first?.lastReply == "a longer final text")
}

@Test func pendingOnlyForPermissionPrompt() {
    let dir = makeTempDir(), projects = makeTempDir()
    let store = makeStore(dir: dir, projects: projects)
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [toolUseLine(id: "t1", name: "Bash", input: ["command": "npm test"])])
    writeSession(dir, "a", pid: me, status: "waiting", waitingFor: "permission prompt", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.sessions.first?.pending == .one(Pending(toolId: "t1", tool: "Bash", text: "npm test")))
    writeSession(dir, "a", pid: me, status: "waiting", waitingFor: "input needed", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.sessions.first?.pending == nil)
}

private func approvableStore() -> (Store, URL, URL, URL) {
    let dir = makeTempDir(), projects = makeTempDir()
    let store = makeStore(dir: dir, projects: projects)
    let f = writeTranscript(projects, cwd: frontend, id: "a", lines: [toolUseLine(id: "t1", name: "Bash", input: ["command": "npm test"])])
    return (store, dir, projects, f)
}

@Test func stillValidRejectsChangedState() {
    let (store, dir, _, f) = approvableStore()
    writeSession(dir, "a", pid: me, status: "waiting", waitingFor: "permission prompt", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.stillValid(.approve("a", toolId: "t1")))
    #expect(!store.stillValid(.approve("a", toolId: "t2")))

    // Same state, a different tool open: only the toolId differs.
    try! Data([toolUseLine(id: "t1", name: "Bash", input: ["command": "npm test"]), toolResultLine(id: "t1"),
               toolUseLine(id: "t2", name: "Bash", input: ["command": "ls"])].joined(separator: "\n").utf8).write(to: f)
    writeSession(dir, "a", pid: me, status: "waiting", waitingFor: "permission prompt", ageSec: 1, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(!store.stillValid(.approve("a", toolId: "t1")))
    #expect(store.stillValid(.approve("a", toolId: "t2")))

    try! Data([toolUseLine(id: "t1", name: "Bash", input: ["command": "npm test"]), toolResultLine(id: "t1")].joined(separator: "\n").utf8).write(to: f)
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(!store.stillValid(.approve("a", toolId: "t1")))
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(!store.stillValid(.approve("a", toolId: "t1")))
    #expect(store.stillValid(.jump("zzz")))
}

@Test func askClaudeValidOnlyWhenIdle() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    let ask = WidgetAction.askClaude("a", prompt: "x")
    for (status, waitingFor, ok) in [("idle", nil, true), ("busy", nil, false), ("waiting", "input needed", false), ("waiting", "permission prompt", false)] as [(String, String?, Bool)] {
        writeSession(dir, "a", pid: me, status: status, waitingFor: waitingFor, ageSec: 0, name: "a", cwd: frontend)
        store.refresh(at: Date())
        #expect(store.stillValid(ask) == ok)
    }
    #expect(!store.stillValid(.askClaude("zzz", prompt: "x")))
}

@Test func replyNeedsIdle() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    let reply = WidgetAction.reply("a", text: "hi")
    for (status, waitingFor, ok) in [("idle", nil, true), ("busy", nil, false), ("waiting", "input needed", false), ("waiting", "permission prompt", false)] as [(String, String?, Bool)] {
        writeSession(dir, "a", pid: me, status: status, waitingFor: waitingFor, ageSec: 0, name: "a", cwd: frontend)
        store.refresh(at: Date())
        #expect(store.stillValid(reply) == ok)
    }
    #expect(!store.stillValid(.reply("zzz", text: "hi")))
}

@Test func stopNeedsBusy() {
    let dir = makeTempDir(), projects = makeTempDir()
    let store = makeStore(dir: dir, projects: projects)
    let stop = WidgetAction.stop("a")
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.stillValid(stop))
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(!store.stillValid(stop))
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [userLine("hi"), assistantLine("done"), turnDurationLine(background: 1)].map { $0 + "\n" })
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(!store.stillValid(stop))
    #expect(!store.stillValid(.stop("zzz")))
}

@Test func askClaudeInvalidWhenWaitingAfterTurnEnd() {
    let dir = makeTempDir(), projects = makeTempDir()
    let store = makeStore(dir: dir, projects: projects)
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [userLine("hi"), assistantLine("done"), turnDurationLine(background: 1)])
    writeSession(dir, "a", pid: me, status: "waiting", waitingFor: "input needed", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(!store.stillValid(.askClaude("a", prompt: "x")))
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.stillValid(.askClaude("a", prompt: "x")))
}

@Test func askClaudeInvalidWhenBusyEndedThenWaiting() {
    let dir = makeTempDir(), projects = makeTempDir()
    let store = makeStore(dir: dir, projects: projects)
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [userLine("hi"), assistantLine("done"), turnDurationLine(background: 1)].map { $0 + "\n" })
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.stillValid(.askClaude("a", prompt: "x")))
    writeSession(dir, "a", pid: me, status: "waiting", waitingFor: "input needed", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(!store.stillValid(.askClaude("a", prompt: "x")))
}

@Test func allowsFalseWhenSessionGone() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    let e = Store.Editing(id: "a", kind: .deny, toolId: "t1")
    try! FileManager.default.removeItem(at: dir.appendingPathComponent("a.json"))
    store.refresh(at: Date())
    #expect(!store.allows(e))
}

@Test func editingKeepsExpanded() {
    let store = makeStore(dir: makeTempDir())
    let panel = PanelState()
    panel.editing = Store.Editing(id: "a", kind: .deny, toolId: "t1")
    #expect(panel.expanded(store))
}

@Test func hiddenAfter30sEmpty() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    let t0 = Date()
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 5, name: "a", cwd: frontend)
    store.refresh(at: t0)
    #expect(!store.hidden)
    try? FileManager.default.removeItem(at: dir.appendingPathComponent("a.json"))
    store.refresh(at: t0.addingTimeInterval(1))
    #expect(!store.hidden)
    store.refresh(at: t0.addingTimeInterval(30))
    #expect(!store.hidden)
    store.refresh(at: t0.addingTimeInterval(31))
    #expect(store.hidden)
}

@Test func shownWhenSessionAppears() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    let t0 = Date()
    store.refresh(at: t0.addingTimeInterval(-60))
    store.refresh(at: t0)
    #expect(store.hidden)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 5, name: "a", cwd: frontend)
    store.refresh(at: t0.addingTimeInterval(1))
    #expect(!store.hidden && store.emptySince == nil)
}

@Test func startHiddenHidesImmediately() {
    let store = makeStore(dir: makeTempDir(), startHidden: true)
    store.refresh(at: Date())
    #expect(store.hidden)
    let dir = makeTempDir()
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 5, name: "a", cwd: frontend)
    let live = makeStore(dir: dir, startHidden: true)
    live.refresh(at: Date())
    #expect(!live.hidden)
}

@Test func defaultStartsVisible() {
    let store = makeStore(dir: makeTempDir())
    store.refresh(at: Date())
    #expect(!store.hidden)
}

@Test func surfaceDroppedWhenPidChanges() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    store.surfaces["a"] = "S"
    store.refresh(at: Date())
    #expect(store.surfaces["a"] == "S")
    #expect(store.sessions.map(\.id) == ["a"])
    let other = getppid()
    writeSession(dir, "a", pid: other, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.sessions.map(\.id) == ["a"])
    #expect(store.surfaces["a"] == nil)
}

@Test func statusFlipWithUnchangedFileSizeIsSeen() {
    let (store, dir, _, _) = approvableStore()
    let t = Date()
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend, updatedAt: t)
    store.refresh(at: Date())
    #expect(store.sessions.first?.pending == nil)
    writeSession(dir, "a", pid: me, status: "waiting", waitingFor: "permission prompt", ageSec: 0, name: "a", cwd: frontend, updatedAt: t)
    store.refresh(at: Date())
    #expect(store.sessions.first?.pending == .one(Pending(toolId: "t1", tool: "Bash", text: "npm test")))
}

@Test func allowsFalseWhenToolChanges() {
    let (store, dir, _, f) = approvableStore()
    writeSession(dir, "a", pid: me, status: "waiting", waitingFor: "permission prompt", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    let e = Store.Editing(id: "a", kind: .deny, toolId: "t1")
    store.refresh(at: Date())
    #expect(store.allows(e))
    try! Data([toolUseLine(id: "t1", name: "Bash", input: ["command": "npm test"]), toolResultLine(id: "t1"),
               toolUseLine(id: "t2", name: "Bash", input: ["command": "ls"])].joined(separator: "\n").utf8).write(to: f)
    writeSession(dir, "a", pid: me, status: "waiting", waitingFor: "permission prompt", ageSec: 1, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(!store.allows(e))
}

// One conversation open in two processes (resumed in a second pane): one row, the newest process, and its cached surface stays.
@Test func sameSessionInTwoProcessesShowsNewestOnce() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "new", cwd: frontend)
    let old: [String: Any] = ["pid": getppid(), "sessionId": "a", "name": "old", "cwd": frontend, "status": "idle",
                              "statusUpdatedAt": (Date().timeIntervalSince1970 - 600) * 1000]
    try! JSONSerialization.data(withJSONObject: old).write(to: dir.appendingPathComponent("old.json"))
    store.refresh(at: Date())
    #expect(store.sessions.map(\.name) == ["new"])
    store.surfaces["a"] = "S"
    store.refresh(at: Date())
    #expect(store.surfaces["a"] == "S")
}

@Test func titleOrderUserBeatsAITitleBeatsPrompt() {
    let dir = makeTempDir(), projects = makeTempDir()
    let store = makeStore(dir: dir, projects: projects)
    let t0 = Date()
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [userLine("sửa widget")])
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "front-d7", cwd: frontend, updatedAt: t0)
    store.refresh(at: t0)
    #expect(store.sessions.first?.title == "sửa widget")

    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [userLine("sửa widget"), aiTitleLine("widget-fix")])
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "front-d7", cwd: frontend, updatedAt: t0.addingTimeInterval(1))
    store.refresh(at: t0.addingTimeInterval(1))
    #expect(store.sessions.first?.title == "widget-fix")

    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "tên tôi", cwd: frontend, updatedAt: t0.addingTimeInterval(1), nameSource: "user")
    store.refresh(at: t0.addingTimeInterval(2))
    #expect(store.sessions.first?.title == "tên tôi")
}

@Test func aiTitleStaysAfterLeavingTail() {
    let dir = makeTempDir(), projects = makeTempDir()
    let store = makeStore(dir: dir, projects: projects)
    let t0 = Date()
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [aiTitleLine("t1"), assistantLine("x")])
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "front-d7", cwd: frontend, updatedAt: t0)
    store.refresh(at: t0)
    #expect(store.sessions.first?.title == "t1")

    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [assistantLine("x"), assistantLine("a longer reply")])
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "front-d7", cwd: frontend, updatedAt: t0.addingTimeInterval(1))
    store.refresh(at: t0.addingTimeInterval(1))
    #expect(store.sessions.first?.title == "t1")
}

@Test func busySessionHasPromptTitleAndBranch() {
    let dir = makeTempDir(), projects = makeTempDir()
    let store = makeStore(dir: dir, projects: projects, branch: { $0 == frontend ? "main" : nil })
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [userLine("chạy test")])
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 5, name: "front-d7", cwd: frontend)
    store.refresh(at: Date())
    let s = store.sessions[0]
    #expect(s.title == "chạy test")
    #expect(s.subtitle == "talo6-dct-frontend · main")
    #expect(s.lastReply == nil)
}

@Test func titleAppearsAfterTranscriptCreated() {
    let dir = makeTempDir(), projects = makeTempDir()
    let store = makeStore(dir: dir, projects: projects)
    let t0 = Date()
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "front-d7", cwd: frontend, updatedAt: t0)
    store.refresh(at: t0)
    #expect(store.sessions[0].title == "front-d7")

    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [userLine("chạy test")])
    store.refresh(at: t0)
    #expect(store.sessions[0].title == "front-d7")

    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "front-d7", cwd: frontend, updatedAt: t0.addingTimeInterval(1))
    store.refresh(at: t0.addingTimeInterval(1))
    #expect(store.sessions[0].title == "chạy test")
}

@Test func sortsByTitle() {
    let dir = makeTempDir(), projects = makeTempDir()
    let store = makeStore(dir: dir, projects: projects)
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [userLine("zzz")])
    _ = writeTranscript(projects, cwd: backend, id: "b", lines: [userLine("aaa")])
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 5, name: "a", cwd: frontend)
    writeSession(dir, "b", pid: me, status: "idle", ageSec: 5, name: "b", cwd: backend)
    store.refresh(at: Date())
    #expect(store.sessions.map(\.id) == ["b", "a"])
}

@Test func todoForBusySession() {
    let dir = makeTempDir(), tasks = makeTempDir()
    let store = makeStore(dir: dir, tasks: tasks)
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 5, name: "a", cwd: frontend)
    writeTask(tasks, session: "a", id: 1, status: "in_progress", subject: "Phase 1", activeForm: "Doing phase 1")
    store.refresh(at: Date())
    #expect(store.sessions.first?.todo == Todo(done: 0, total: 1, current: "Doing phase 1"))
}

@Test func todoUpdatesWhenTaskChanges() {
    let dir = makeTempDir(), tasks = makeTempDir()
    let store = makeStore(dir: dir, tasks: tasks)
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 5, name: "a", cwd: frontend)
    writeTask(tasks, session: "a", id: 1, status: "in_progress", subject: "Phase 1")
    store.refresh(at: Date())
    #expect(store.sessions.first?.todo?.done == 0)
    writeTask(tasks, session: "a", id: 1, status: "completed", subject: "Phase 1")
    writeTask(tasks, session: "a", id: 2, status: "pending", subject: "Phase 2")
    store.refresh(at: Date())
    #expect(store.sessions.first?.todo == Todo(done: 1, total: 2, current: "Phase 2"))
}

@Test func todoGoneWhenFilesDeleted() throws {
    let dir = makeTempDir(), tasks = makeTempDir()
    let store = makeStore(dir: dir, tasks: tasks)
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 5, name: "a", cwd: frontend)
    writeTask(tasks, session: "a", id: 1, status: "in_progress", subject: "Phase 1")
    store.refresh(at: Date())
    #expect(store.sessions.first?.todo != nil)
    try FileManager.default.removeItem(at: tasks.appendingPathComponent("a/1.json"))
    store.refresh(at: Date())
    #expect(store.sessions.first?.todo == nil)
}

@Test func sessionJSONSameSizeRewriteIsSeen() throws {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    let t = Date()
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend, updatedAt: t)
    store.refresh(at: t)
    #expect(store.sessions.first?.status == "idle")
    // Same byte size ("idle" -> "busy" padded), written to a temp file and renamed like Claude Code does.
    let file = dir.appendingPathComponent("a.json")
    let old = try Data(contentsOf: file)
    var text = String(decoding: old, as: UTF8.self).replacingOccurrences(of: "\"idle\"", with: "\"busy\"")
    text += String(repeating: " ", count: old.count - text.utf8.count)
    #expect(text.utf8.count == old.count)
    let tmp = dir.appendingPathComponent("a.tmp")
    try text.write(to: tmp, atomically: false, encoding: .utf8)
    _ = try FileManager.default.replaceItemAt(file, withItemAt: tmp)
    store.refresh(at: t.addingTimeInterval(1))
    #expect(store.sessions.first?.status == "busy")
}

@Test func taskRewriteInPlaceIsSeen() {
    let dir = makeTempDir(), tasks = makeTempDir()
    let store = makeStore(dir: dir, tasks: tasks)
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 5, name: "a", cwd: frontend)
    writeTask(tasks, session: "a", id: 1, status: "pending", subject: "Phase 1")
    store.refresh(at: Date())
    #expect(store.sessions.first?.todo == Todo(done: 0, total: 1, current: "Phase 1"))
    // Same file name, same size, rewritten in place: the directory does not change.
    writeTask(tasks, session: "a", id: 1, status: "pending", subject: "Phase 2")
    store.refresh(at: Date())
    #expect(store.sessions.first?.todo == Todo(done: 0, total: 1, current: "Phase 2"))
}

@Test func branchFollowsCheckoutWhileBusy() {
    let dir = makeTempDir(), projects = makeTempDir()
    var heads = [frontend: "main"]
    let store = makeStore(dir: dir, projects: projects, branch: { heads[$0] })
    let t0 = Date()
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [userLine("x"), assistantLine("y")])
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend, updatedAt: t0)
    store.refresh(at: t0)
    #expect(store.sessions[0].subtitle.hasSuffix("· main"))

    heads[frontend] = "feat"
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend, updatedAt: t0.addingTimeInterval(1))
    store.refresh(at: t0.addingTimeInterval(1))
    #expect(store.sessions[0].subtitle.hasSuffix("· feat"))
}

@Test func branchComesFromCwdNotTranscript() {
    let dir = makeTempDir(), projects = makeTempDir()
    let store = makeStore(dir: dir, projects: projects, branch: { $0 == frontend ? "master" : nil })
    store.jiraProjects = ["T6IR": jiraHost]
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [jsonLine(["type": "user", "gitBranch": "bugfix/T6IR-30", "message": ["content": "hello"]]), ""])
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    store.refresh(at: Date())
    #expect(store.sessions[0].subtitle == "talo6-dct-frontend · master")
    #expect(labels(store) == [])
}

@Test func branchTicketNotAButton() {
    let dir = makeTempDir(), projects = makeTempDir()
    let store = makeStore(dir: dir, projects: projects, branch: { _ in "bugfix/T6IR-30" })
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [userLine("https://\(jiraHost)/browse/T6IR-10"), ""])
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    store.refresh(at: Date())
    #expect(labels(store) == ["T6IR-10"])
    #expect(store.sessions[0].menu.map(\.title).contains("Nhánh") == false)
}

@Test func titleCutAt200() {
    let dir = makeTempDir(), projects = makeTempDir()
    let store = makeStore(dir: dir, projects: projects)
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [userLine(String(repeating: "t", count: 260))])
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 5, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.sessions.first?.title == String(repeating: "t", count: 200) + "…")
}

@Test func replyCutAt600WithLabels() {
    let dir = makeTempDir(), projects = makeTempDir()
    let store = makeStore(dir: dir, projects: projects)
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [assistantLine("PR https://github.com/a/b/pull/9 " + String(repeating: "x", count: 1000))])
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    let reply = store.sessions.first?.lastReply
    #expect(reply?.hasPrefix("PR PR #9 ") == true)
    #expect(reply?.hasSuffix("…") == true)
    #expect(reply?.count == 601)
}

private let jiraHost = "talosix.atlassian.net"

/// One idle session "a" whose transcript holds `lines`.
private func scanStore(_ lines: [String], status: String = "idle") -> (Store, URL) {
    let dir = makeTempDir(), projects = makeTempDir()
    let store = makeStore(dir: dir, projects: projects)
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: lines + [""])
    writeSession(dir, "a", pid: me, status: status, ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    return (store, dir)
}

private func labels(_ store: Store) -> [String]? { store.sessions.first?.links.map(\.label) }

@Test func ticketButtonOrder() {
    let (store, _) = scanStore([
        userLine("https://\(jiraHost)/browse/T6IR-10"),
        userLine("T6IR-11"),
        assistantLine("T6IR-20"),
        assistantLine("T6IR-21")])
    #expect(labels(store) == ["T6IR-10", "T6IR-11", "T6IR-21", "T6IR-20"])
}

@Test func onlyJiraAndPRButtons() {
    let (store, _) = scanStore([assistantLine("https://example.com/x https://docs.coderabbit.ai https://github.com/a/b/pull/1")])
    #expect(labels(store) == ["PR #1"])
}

@Test func unknownPrefixDropped() {
    let (store, _) = scanStore([userLine("AC-1 UTF-8 https://\(jiraHost)/browse/T6IR-1")])
    #expect(labels(store) == ["T6IR-1"])
}

@Test func bareKeyShownOnceProjectLearned() {
    let (store, _) = scanStore([userLine("T6IR-5"), assistantLine("see https://\(jiraHost)/browse/T6IR-1")])
    #expect(labels(store) == ["T6IR-5", "T6IR-1"])
    #expect(store.jiraProjects["T6IR"] == jiraHost)
    #expect(store.sessions.first?.links.first?.url.absoluteString == "https://\(jiraHost)/browse/T6IR-5")
}

@Test func ticketButtonCacheInvalidates() {
    let dir = makeTempDir(), projects = makeTempDir()
    let store = makeStore(dir: dir, projects: projects)
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [userLine("T6IR-5"), ""])
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    func links(_ id: String) -> [String]? { store.sessions.first { $0.id == id }?.links.map(\.label) }
    store.refresh(at: Date())
    store.refresh(at: Date())
    #expect(links("a") == [])
    // Another session teaches the T6IR prefix: the bare key appears.
    _ = writeTranscript(projects, cwd: backend, id: "b", lines: [userLine("https://\(jiraHost)/browse/T6IR-1"), ""])
    writeSession(dir, "b", pid: me, status: "idle", ageSec: 0, name: "b", cwd: backend)
    store.refresh(at: Date())
    store.refresh(at: Date())
    #expect(links("a") == ["T6IR-5"])
    #expect(links("b") == ["T6IR-1"])
}

@Test func ticketButtonsSurviveBusy() {
    let (store, dir) = scanStore([userLine("/cook https://\(jiraHost)/browse/T6IR-14728"), assistantLine("PR https://github.com/a/b/pull/1")])
    let idle = labels(store)
    #expect(idle == ["T6IR-14728", "PR #1"])
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(labels(store) == idle)
}

@Test func noButtonCap() {
    let keys = (1...6).map { "T6IR-\($0)" }
    let (store, _) = scanStore([userLine("https://\(jiraHost)/browse/T6IR-1 " + keys.joined(separator: " "))])
    #expect(labels(store) == keys)
}

@Test func titleCutInsideUrlShowsLabelAndKeepsLink() {
    let dir = makeTempDir(), projects = makeTempDir()
    let store = makeStore(dir: dir, projects: projects)
    let pr = "https://bitbucket.org/talosix/very-long-repository-name/pull-requests/6867/overview?tab=activity&filter=" + String(repeating: "z", count: 80)
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [userLine(String(repeating: "a", count: 120) + " " + pr), ""])
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    let s = store.sessions.first
    #expect(s?.title == String(repeating: "a", count: 120) + " PR #6867")
    #expect(s?.links.map(\.label) == ["PR #6867"])
}

// Running-tool line

private struct ActivityRig {
    let dir = makeTempDir(), projects = makeTempDir()
    let store: Store
    let file: URL
    init(_ lines: [String]) {
        store = makeStore(dir: dir, projects: projects)
        file = writeTranscript(projects, cwd: frontend, id: "a", lines: lines.map { $0 + "\n" })
        busy()
    }
    func busy() { writeSession(dir, "a", pid: me, status: "busy", ageSec: 5, name: "n", cwd: frontend) }
    func idle() { writeSession(dir, "a", pid: me, status: "idle", ageSec: 5, name: "n", cwd: frontend) }
    func refresh() { store.refresh(at: Date()) }
    var activity: String? { store.sessions.first?.activity }
    var session: Session? { store.sessions.first }
    func append(_ s: String) { ClaudeSessionsKitTests.append(file, s) }
}

private func bash(_ c: String) -> String { toolUseLine(id: UUID().uuidString, name: "Bash", input: ["command": c]) }

@Test func activityFromExistingTranscript() {
    let r = ActivityRig([userLine("hi"), bash("swift test")])
    r.refresh()
    #expect(r.activity == "Bash: swift test")
}

// Two processes with one sessionId: the idle one must not clear the busy row's cached activity, whatever order the files are read in.
@Test(arguments: [("a-busy", "z-idle"), ("z-busy", "a-idle")])
func activitySurvivesDuplicateSessionIdOrder(busyFile: String, idleFile: String) {
    let dir = makeTempDir(), projects = makeTempDir()
    let store = makeStore(dir: dir, projects: projects)
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [bash("swift test") + "\n"])
    func write(_ file: String, pid: Int32, status: String, ageSec: Double) {
        let o: [String: Any] = ["pid": pid, "sessionId": "a", "name": file, "cwd": frontend, "status": status,
                                "statusUpdatedAt": (Date().timeIntervalSince1970 - ageSec) * 1000]
        try! JSONSerialization.data(withJSONObject: o).write(to: dir.appendingPathComponent("\(file).json"))
    }
    write(busyFile, pid: me, status: "busy", ageSec: 1)
    write(idleFile, pid: getppid(), status: "idle", ageSec: 600)
    for _ in 0..<2 {
        store.refresh(at: Date())
        #expect(store.sessions.map(\.name) == [busyFile])
        #expect(store.sessions.first?.activity == "Bash: swift test")
    }
}

@Test func busyCardCountsAgents() {
    func agent(_ id: String) -> String { toolUseLine(id: id, name: "Agent", input: ["description": "review \(id)"]) }
    let r = ActivityRig([userLine("hi"), agent("a1"), agent("a2"), agent("a3"), toolResultLine(id: "a1")])
    r.refresh()
    #expect(r.session?.agents == ["review a2", "review a3"])
    r.append(toolResultLine(id: "a2") + "\n")
    r.refresh()
    #expect(r.session?.agents == ["review a3"])
    r.idle()
    r.refresh()
    #expect(r.session?.agents == [])
}

private let askX = WidgetAction.askClaude("a", prompt: "x")

@Test func backgroundWaitShown() {
    let r = ActivityRig([userLine("hi"), bash("swift test"), assistantLine("done"), turnDurationLine(background: 3, workflows: 0)])
    r.refresh()
    #expect(r.session?.backgroundAgents == 3)
    #expect(r.session?.shownStatus == "idle")
    #expect(r.activity == nil)
    #expect(r.store.stillValid(askX))
}

@Test func backgroundWaitEndsOnNewTurn() {
    let r = ActivityRig([userLine("hi"), assistantLine("done"), turnDurationLine(background: 3)])
    r.refresh()
    #expect(r.session?.backgroundAgents == 3)
    r.append(userLine("task-notification") + "\n" + bash("ls") + "\n")
    r.refresh()
    #expect(r.session?.backgroundAgents == nil)
    #expect(r.session?.shownStatus == "busy")
    #expect(r.activity == "Bash: ls")
    #expect(!r.store.stillValid(askX))
}

@Test func endedClearsOldToolLineForNextTurn() {
    let r = ActivityRig([userLine("hi"), bash("swift test"), assistantLine("done"), turnDurationLine(background: 1)])
    r.refresh()
    r.append(userLine("next") + "\n")
    r.refresh()
    #expect(r.session?.backgroundAgents == nil)
    #expect(r.activity == nil)
}

@Test func backgroundWaitShowsLastReply() {
    let r = ActivityRig([userLine("hi"), assistantLine("all done"), turnDurationLine(background: 2)])
    r.refresh()
    #expect(r.session?.backgroundAgents == 2)
    #expect(r.session?.lastReply == "all done")
    #expect(r.session?.isFinished == true)
}

@Test func backgroundWaitSurvivesMetadataOnlyAppend() {
    let r = ActivityRig([userLine("hi"), assistantLine("done"), turnDurationLine(background: 3)])
    r.refresh()
    r.append(jsonLine(["type": "last-prompt", "sessionId": "a"]) + "\n")
    r.refresh()
    #expect(r.session?.backgroundAgents == 3)
}

@Test func sidechainLinesKeepBackgroundWait() {
    let r = ActivityRig([userLine("hi"), assistantLine("done"), turnDurationLine(background: 2)])
    r.refresh()
    r.append(toolUseLine(id: "s", name: "Bash", input: ["command": "ls"], sidechain: true) + "\n" + assistantLine("side", sidechain: true) + "\n")
    r.refresh()
    #expect(r.session?.backgroundAgents == 2)
    #expect(r.activity == nil)
    #expect(r.store.stillValid(askX))
}

@Test func turnEndedWithoutBackgroundCounts() {
    let r = ActivityRig([userLine("hi"), assistantLine("done"), turnDurationLine()])
    r.refresh()
    #expect(r.session?.backgroundAgents == 0)
    #expect(r.session?.shownStatus == "idle")
    #expect(r.store.stillValid(askX))
}

@Test func backgroundWaitClearedWhenSessionNotBusy() {
    let r = ActivityRig([userLine("hi"), assistantLine("done"), turnDurationLine(background: 1)])
    r.refresh()
    r.idle()
    r.refresh()
    #expect(r.session?.backgroundAgents == nil)
}

@Test func activityNoReadWhenUnchanged() {
    let r = ActivityRig([bash("swift test")])
    r.refresh()
    let reads = r.store.activityReads
    #expect(reads == 1)
    r.refresh()
    #expect(r.store.activityReads == reads)
    #expect(r.activity == "Bash: swift test")
}

@Test func activityUpdatesOnAppend() {
    let r = ActivityRig([bash("swift test")])
    r.refresh()
    let reads = r.store.activityReads
    r.append(toolUseLine(id: "e", name: "Edit", input: ["file_path": "/a/File.swift"]) + "\n")
    r.refresh()
    #expect(r.activity == "Edit File.swift")
    #expect(r.store.activityReads == reads + 1)
}

@Test func activityWaitsForCompleteLine() {
    let r = ActivityRig([bash("swift test")])
    r.refresh()
    let line = toolUseLine(id: "e", name: "Edit", input: ["file_path": "/a/File.swift"])
    let half = line.index(line.startIndex, offsetBy: line.count / 2)
    r.append(String(line[..<half]))
    r.refresh()
    #expect(r.activity == "Bash: swift test")
    r.append(String(line[half...]) + "\n")
    r.refresh()
    #expect(r.activity == "Edit File.swift")
}

@Test func activityResetsWhenFileShrinks() {
    let r = ActivityRig([bash(String(repeating: "x", count: 100)), bash("swift test")])
    r.refresh()
    try! Data((toolUseLine(id: "g", name: "Grep", input: ["pattern": "foo"]) + "\n").utf8).write(to: r.file)
    r.refresh()
    #expect(r.activity == "Grep: foo")
}

@Test func activityClearedWhenNotBusy() {
    let r = ActivityRig([bash("swift test")])
    r.refresh()
    #expect(r.activity == "Bash: swift test")
    r.idle()
    r.refresh()
    #expect(r.activity == nil)
    r.busy()
    r.refresh()
    #expect(r.activity == nil)
}

@Test func activityReadsAtMostWindow() {
    let filler = userLine(String(repeating: "x", count: 1_000))
    let r = ActivityRig([bash("old")] + Array(repeating: filler, count: 400))
    r.refresh()
    #expect(r.activity == nil)
    #expect(r.store.activityReads == 1)
}

@Test func chimeSilentOnFirstRefresh() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    writeSession(dir, "b", pid: me, status: "waiting", waitingFor: "permission prompt", ageSec: 0, name: "b", cwd: backend)
    store.refresh(at: Date())
    #expect(store.chime == nil)
}

@Test func chimeDoneOnBusyToIdle() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    let t0 = Date()
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: t0)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: t0.addingTimeInterval(1))
    #expect(store.chime == .done)
}

@Test func chimeNeedsYouOnBusyToWaiting() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    let t0 = Date()
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: t0)
    writeSession(dir, "a", pid: me, status: "waiting", waitingFor: "permission prompt", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: t0.addingTimeInterval(1))
    #expect(store.chime == .needsYou)
}

@Test func chimeNeedsYouWins() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    let t0 = Date()
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    writeSession(dir, "b", pid: me, status: "busy", ageSec: 0, name: "b", cwd: backend)
    store.refresh(at: t0)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    writeSession(dir, "b", pid: me, status: "waiting", waitingFor: "permission prompt", ageSec: 0, name: "b", cwd: backend)
    store.refresh(at: t0.addingTimeInterval(1))
    #expect(store.chime == .needsYou)
}

@Test func chimeResetsNextRefresh() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    let t0 = Date()
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: t0)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: t0.addingTimeInterval(1))
    #expect(store.chime == .done)
    store.refresh(at: t0.addingTimeInterval(2))
    #expect(store.chime == nil)
}

@Test func bgOpacityDefault() {
    let store = makeStore(dir: makeTempDir())
    #expect(store.bgOpacity == 0.25)
    #expect(Layout.bgOpacities == [0, 0.25, 0.5, 0.75, 0.92])
}

// MARK: - Unseen marks

private func markedStore() -> (Store, URL, Date) {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    let t0 = Date()
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 5, name: "a", cwd: frontend)
    store.refresh(at: t0)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: t0.addingTimeInterval(1))
    return (store, dir, t0)
}

@Test func unseenOnBusyToIdleAndWaiting() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    let t0 = Date()
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 5, name: "a", cwd: frontend)
    writeSession(dir, "b", pid: me, status: "busy", ageSec: 5, name: "b", cwd: backend)
    writeSession(dir, "c", pid: me, status: "idle", ageSec: 5, name: "c", cwd: backend)
    store.refresh(at: t0)
    #expect(store.unseen.isEmpty)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    writeSession(dir, "b", pid: me, status: "waiting", waitingFor: "permission prompt", ageSec: 0, name: "b", cwd: backend)
    store.refresh(at: t0.addingTimeInterval(1))
    #expect(Set(store.unseen.keys) == ["a", "b"])
}

@Test func unseenKeptAcrossRefreshes() {
    let (store, _, t0) = markedStore()
    for i in 1...3 { store.refresh(at: t0.addingTimeInterval(1 + Double(i) * 30)) }
    #expect(Set(store.unseen.keys) == ["a"])
}

@Test func unseenClearedOnSwitchToBusy() {
    let (store, dir, t0) = markedStore()
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: t0.addingTimeInterval(2))
    #expect(store.unseen.isEmpty)
}

@Test func unseenDroppedWhenSessionDies() {
    let (store, dir, t0) = markedStore()
    writeSession(dir, "a", pid: 999_999, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: t0.addingTimeInterval(2))
    #expect(store.unseen["a"] == nil)
}

@Test func markSeenClears() {
    let (store, _, _) = markedStore()
    store.markSeen("zzz")
    #expect(Set(store.unseen.keys) == ["a"])
    store.markSeen("a")
    #expect(store.unseen.isEmpty)
}

@Test func newCountCountsLiveOnly() {
    let (store, _, _) = markedStore()
    store.unseen["dead"] = NewMark(claude: true)
    #expect(store.newCount == 1)
}

@Test func noteFocusedNeedsTwoConsecutive() {
    let (store, _, _) = markedStore()
    store.surfaces["a"] = "S1"
    store.noteFocused("S1")
    #expect(Set(store.unseen.keys) == ["a"])
    store.noteFocused("s1")
    #expect(store.unseen.isEmpty)
}

@Test func noteFocusedNilResets() {
    let (store, _, _) = markedStore()
    store.surfaces["a"] = "S1"
    store.noteFocused("S1")
    store.noteFocused(nil)
    store.noteFocused("S1")
    #expect(Set(store.unseen.keys) == ["a"])
}

@Test func surfaceMissDroppedWhenPidChanges() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    store.surfaceMisses.insert("a")
    store.refresh(at: Date())
    #expect(store.surfaceMisses == ["a"])
    writeSession(dir, "a", pid: getppid(), status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.surfaceMisses.isEmpty)
    store.surfaceMisses.insert("a")
    writeSession(dir, "a", pid: 999_999, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.surfaceMisses.isEmpty)
}

// MARK: - Pull requests

private let prRepo = "t6engineering/talo6-dct-backend"
private let prT0 = Date(timeIntervalSince1970: 1_800_000_000)

private func prReply(_ ids: [Int], extra: String = "") -> String {
    assistantLine(ids.map { "https://bitbucket.org/\(prRepo)/pull-requests/\($0)" }.joined(separator: " ") + extra)
}

/// One idle session `a` whose last reply is `lines`, with a PR monitor on the fake.
private func prStore(_ api: FakeBitbucket, reply: String) -> (Store, URL, URL) {
    let dir = makeTempDir(), projects = makeTempDir()
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [reply, ""])
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    let monitor = PRMonitor(api: api, queue: nil, remote: { _ in nil })
    return (makeStore(dir: dir, projects: projects, prs: monitor), dir, projects)
}

private func prWithReviewer(id: Int, approved: Bool, comments: Int = 0) -> [String: Any] {
    var o = prJSON(id: id, comments: comments)
    o["participants"] = [["role": "REVIEWER", "approved": approved, "state": approved ? "approved" : NSNull(), "user": ["uuid": "{r}"]]]
    return o
}

@Test func prsAttachedToSession() {
    let api = fake(open: [7571, 6874, 1, 2].map { prJSON(id: $0) })
    let (store, _, _) = prStore(api, reply: prReply([7571, 6874, 1, 2]))
    store.refresh(at: prT0)
    #expect(store.sessions.first?.prs.map(\.ref.id) == [7571, 6874, 1])
}

@Test func prLinkButtonDropped() {
    let reply = prReply([7571], extra: " https://example.com/x")
    let api = fake(open: [prJSON(id: 7571)])
    let (store, _, _) = prStore(api, reply: reply)
    store.refresh(at: prT0)
    #expect(store.sessions.first?.links.isEmpty == true)

    let failing = fake()
    failing.failAll = .transport
    let (other, _, _) = prStore(failing, reply: reply)
    other.refresh(at: prT0)
    #expect(other.sessions.first?.prs.isEmpty == true)
    #expect(other.sessions.first?.links.count == 1)
}

@Test func prEventChimesAndPeeks() {
    let api = fake(open: [prWithReviewer(id: 7571, approved: false)])
    let (store, dir, _) = prStore(api, reply: prReply([7571]))
    store.refresh(at: prT0)
    #expect(store.chime == nil)
    #expect(!store.peeking)

    api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [prWithReviewer(id: 7571, approved: false, comments: 1)]])
    api.set("/comments", json: ["values": [comment("{tin}", "Tin Tran", "2026-11-04T10:00:00.000000+00:00")]])
    store.refresh(at: prT0 + 61)
    #expect(store.chime == .needsYou)
    #expect(store.peeking)

    api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [prWithReviewer(id: 7571, approved: true, comments: 1)]])
    store.refresh(at: prT0 + 122)
    #expect(store.chime == .done)

    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: prT0 + 130)
    api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [prWithReviewer(id: 7571, approved: true, comments: 2)]])
    api.set("/comments", json: ["values": [comment("{tin}", "Tin Tran", "2026-11-04T11:00:00.000000+00:00")]])
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: prT0 + 200)
    #expect(store.chime == .needsYou)
}

@Test func refreshSetsPrChimeForPrEventsAndClearsIt() {
    let api = fake(open: [prWithReviewer(id: 7571, approved: false)])
    let (store, _, _) = prStore(api, reply: prReply([7571]))
    store.refresh(at: prT0)
    #expect(store.prChime == nil)

    api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [prWithReviewer(id: 7571, approved: false, comments: 1)]])
    api.set("/comments", json: ["values": [comment("{tin}", "Tin Tran", "2026-11-04T10:00:00.000000+00:00")]])
    store.refresh(at: prT0 + 61)
    #expect(store.prChime == .needsYou)

    store.refresh(at: prT0 + 62)
    #expect(store.prChime == nil)
}

@Test func prStaleAfterFiveMinutes() {
    let api = fake(open: [prJSON(id: 7571)])
    let (store, _, _) = prStore(api, reply: prReply([7571]))
    store.refresh(at: prT0)
    #expect(store.prLastSuccess == prT0)
    api.failAll = .transport
    store.refresh(at: prT0 + 61)
    store.refresh(at: prT0 + 299)
    #expect(!store.prStale)
    store.refresh(at: prT0 + 301)
    #expect(store.prStale)
    #expect(store.prLastSuccess == prT0)
}

@Test func prEventMarksSession() {
    let api = fake(open: [prWithReviewer(id: 7571, approved: false)])
    let (store, _, _) = prStore(api, reply: prReply([7571]))
    store.refresh(at: prT0)
    #expect(store.unseen.isEmpty)
    api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [prWithReviewer(id: 7571, approved: false, comments: 1)]])
    api.set("/comments", json: ["values": [comment("{tin}", "Tin Tran", "2026-11-04T10:00:00.000000+00:00")]])
    store.refresh(at: prT0 + 61)
    #expect(Set(store.unseen.keys) == ["a"])
}

@Test func prEventMarksEverySessionWithThePR() {
    let api = fake(open: [prWithReviewer(id: 7571, approved: false)])
    let (store, dir, projects) = prStore(api, reply: prReply([7571]))
    _ = writeTranscript(projects, cwd: backend, id: "b", lines: [prReply([7571]), ""])
    writeSession(dir, "b", pid: me, status: "idle", ageSec: 0, name: "b", cwd: backend)
    store.refresh(at: prT0)
    #expect(store.unseen.isEmpty)
    api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [prWithReviewer(id: 7571, approved: false, comments: 1)]])
    api.set("/comments", json: ["values": [comment("{tin}", "Tin Tran", "2026-11-04T10:00:00.000000+00:00")]])
    store.refresh(at: prT0 + 61)
    #expect(Set(store.unseen.keys) == ["a", "b"])
}

@Test func prEventMarksBusySession() {
    let api = fake(open: [prWithReviewer(id: 7571, approved: false)])
    let (store, dir, _) = prStore(api, reply: prReply([7571]))
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: prT0)
    #expect(store.unseen.isEmpty)
    api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [prWithReviewer(id: 7571, approved: false, comments: 1)]])
    api.set("/comments", json: ["values": [comment("{tin}", "Tin Tran", "2026-11-04T10:00:00.000000+00:00")]])
    store.refresh(at: prT0 + 61)
    #expect(Set(store.unseen.keys) == ["a"])
}

@Test func prMarkSurvivesSwitchToBusy() {
    let api = fake(open: [prWithReviewer(id: 7571, approved: false)])
    let (store, dir, _) = prStore(api, reply: prReply([7571]))
    store.refresh(at: prT0)
    store.markSeen("a")
    api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [prWithReviewer(id: 7571, approved: false, comments: 1)]])
    api.set("/comments", json: ["values": [comment("{tin}", "Tin Tran", "2026-11-04T10:00:00.000000+00:00")]])
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: prT0 + 61)
    #expect(Set(store.unseen.keys) == ["a"])
    store.refresh(at: prT0 + 62)
    #expect(Set(store.unseen.keys) == ["a"])
}

@Test func prEventMarksOnlyThatPR() {
    let api = fake(open: [prWithReviewer(id: 7571, approved: false), prWithReviewer(id: 7572, approved: false)])
    let (store, _, _) = prStore(api, reply: prReply([7571, 7572]))
    store.refresh(at: prT0)
    #expect(store.unseen.isEmpty)
    api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [prWithReviewer(id: 7571, approved: false), prWithReviewer(id: 7572, approved: false, comments: 1)]])
    api.set("/comments", json: ["values": [comment("{tin}", "Tin Tran", "2026-11-04T10:00:00.000000+00:00")]])
    store.refresh(at: prT0 + 61)
    #expect(store.unseen["a"] == NewMark(claude: false, prs: [prRef(7572)]))
}

@Test func claudeAndPRMarkTogether() {
    let api = fake(open: [prWithReviewer(id: 7571, approved: false)])
    let (store, dir, _) = prStore(api, reply: prReply([7571]))
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: prT0)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [prWithReviewer(id: 7571, approved: false, comments: 1)]])
    api.set("/comments", json: ["values": [comment("{tin}", "Tin Tran", "2026-11-04T10:00:00.000000+00:00")]])
    store.refresh(at: prT0 + 61)
    #expect(store.unseen["a"] == NewMark(claude: true, prs: [prRef(7571)]))
}

@Test func claudeOnlyMark() {
    let (store, _, _) = markedStore()
    #expect(store.unseen["a"] == NewMark(claude: true))
}

@Test func prMarkDroppedWhenPRGone() {
    let api = fake(open: [prWithReviewer(id: 7571, approved: false)])
    let (store, _, _) = prStore(api, reply: prReply([7571]))
    store.refresh(at: prT0)
    store.unseen["a"] = NewMark(prs: [prRef(9999)])
    store.refresh(at: prT0 + 1)
    #expect(store.unseen["a"] == nil)
    #expect(store.newCount == 0)
}

@Test func prMarkOnHiddenFourthPR() {
    let ids = [7571, 7572, 7573, 7574]
    let api = fake(open: ids.map { prWithReviewer(id: $0, approved: false) })
    let (store, _, _) = prStore(api, reply: prReply(ids))
    store.refresh(at: prT0)
    #expect(store.unseen.isEmpty)
    api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": ids.map { prWithReviewer(id: $0, approved: false, comments: $0 == 7574 ? 1 : 0) }])
    api.set("/comments", json: ["values": [comment("{tin}", "Tin Tran", "2026-11-04T10:00:00.000000+00:00")]])
    store.refresh(at: prT0 + 61)
    #expect(store.unseen["a"]?.prs == [prRef(7574)])
    #expect(!store.sessions[0].prs.map(\.ref).contains(prRef(7574)))
}

@Test func claudeMarkSurvivesPRPrune() {
    let api = fake(open: [prWithReviewer(id: 7571, approved: false)])
    let (store, _, _) = prStore(api, reply: prReply([7571]))
    store.refresh(at: prT0)
    store.unseen["a"] = NewMark(claude: true, prs: [prRef(9999)])
    store.refresh(at: prT0 + 1)
    #expect(store.unseen["a"] == NewMark(claude: true))
}

@Test func prMarkClearedOnSwitchToBusy() {
    let api = fake(open: [prWithReviewer(id: 7571, approved: false)])
    let (store, dir, _) = prStore(api, reply: prReply([7571]))
    store.refresh(at: prT0)
    store.unseen["a"] = NewMark(prs: [prRef(7571)])
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: prT0 + 1)
    #expect(store.unseen["a"] == nil)
}

// MARK: - Link menu

private let menuTitle = "Export CSV thiếu cột site"

private func menuPR(_ id: Int, _ title: String) -> [String: Any] {
    var o = prJSON(id: id)
    o["title"] = title
    return o
}

/// One idle session `a`: the user mentions T6IR-2 then T6IR-3, Claude T6IR-4 and four PRs.
private func menuStore(extra: [String] = [], jira: FakeJira? = nil, prs: Bool = true) -> (Store, FakeJira, FakeBitbucket) {
    let fakeJira = jira ?? FakeJira()
    if jira == nil { fakeJira.issues["T6IR-2"] = (menuTitle, "To Do") }
    let api = fake(open: [menuPR(7571, "Fix site scope filter"), menuPR(6874, "Second"), menuPR(1, "Third"), menuPR(2, "Fourth")])
    let dir = makeTempDir(), projects = makeTempDir()
    _ = writeTranscript(projects, cwd: frontend, id: "a",
                        lines: [userLine("T6IR-2"), userLine("T6IR-3"), assistantLine("T6IR-4"),
                                prReply([7571, 6874, 1, 2])] + extra + [""])
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    let store = makeStore(dir: dir, projects: projects,
                          prs: prs ? PRMonitor(api: api, queue: nil, remote: { _ in nil }) : nil,
                          jira: JiraMonitor(api: fakeJira, queue: nil))
    store.jiraProjects = ["T6IR": jiraHost]
    store.refresh(at: prT0)
    return (store, fakeJira, api)
}

private func section(_ s: Session?, _ title: String) -> [String]? { s?.menu.first { $0.title == title }?.items.map(\.text) }

@Test func menuSectionsInOrder() {
    let (store, _, _) = menuStore()
    let s = store.sessions.first
    #expect(s?.menu.map(\.title) == ["Bạn nhắc", "Claude nhắc", "PR"])
    #expect(s?.menu.dropLast().map { $0.items.map(\.url.lastPathComponent) } == [["T6IR-2", "T6IR-3"], ["T6IR-4"]])
    #expect(section(s, "Bạn nhắc") == ["T6IR-2  \(menuTitle)  ·  To Do", "T6IR-3"])
}

@Test func menuListsAllPRs() {
    let (store, _, _) = menuStore()
    let s = store.sessions.first
    #expect(s?.prs.count == 3)
    #expect(section(s, "PR")?.count == 4)
    #expect(section(s, "PR")?.first == "PR #7571  Fix site scope filter  ·  mở")
    func status(_ state: String, _ title: String?) -> PRStatus {
        PRStatus(ref: PRRef(workspace: "w", repo: "r", id: 5), url: URL(string: "https://x.y/5")!, state: state, reviewers: 0,
                 approved: 0, changesRequested: 0, comments: 0, commit: nil, sourceRepo: nil, branch: nil, title: title)
    }
    let merged = status("MERGED", "Done")
    let declined = status("DECLINED", nil)
    #expect(Store.linkText(merged) == "PR #5  Done  ·  đã merge")
    #expect(Store.linkText(declined) == "PR #5  ·  đã đóng")
}

@Test func menuUntrackedPRLink() {
    let (store, _, _) = menuStore(extra: [assistantLine("https://github.com/a/b/pull/12")])
    #expect(section(store.sessions.first, "PR")?.last == "PR #12")
}

@Test func menuTitleCutAt60() {
    let jira = FakeJira()
    jira.issues["T6IR-2"] = (String(repeating: "x", count: 70), "Done")
    let (store, _, _) = menuStore(jira: jira)
    #expect(section(store.sessions.first, "Bạn nhắc")?.first == "T6IR-2  \(String(repeating: "x", count: 60))…  ·  Done")
}

@Test func menuTitleWithoutStatus() {
    let jira = FakeJira()
    jira.issues["T6IR-2"] = ("Only title", nil)
    let (store, _, _) = menuStore(jira: jira)
    #expect(section(store.sessions.first, "Bạn nhắc")?.first == "T6IR-2  Only title")
}

private let iconURL = URL(string: "https://talosix.atlassian.net/avatar/1?size=medium")!

private func png() -> Data {
    let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 1, pixelsHigh: 1, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true,
                               isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    return rep.representation(using: .png, properties: [:])!
}

private func typedJira() -> FakeJira {
    let jira = FakeJira()
    jira.issues["T6IR-2"] = (menuTitle, "Done")
    jira.types["T6IR-2"] = ("Bug", iconURL.absoluteString)
    jira.categories["T6IR-2"] = "done"
    return jira
}

@Test func menuItemsCarryTicketFields() {
    let (store, _, _) = menuStore(jira: typedJira())
    let item = store.sessions.first?.menu.first { $0.title == "Bạn nhắc" }?.items.first
    #expect(item?.kind == .ticket)
    #expect(item?.code == "T6IR-2")
    #expect(item?.title == menuTitle)
    #expect(item?.status == "Done")
    #expect(item?.tone == .done)
    #expect(item?.iconURL == iconURL)
    #expect(item?.help == "T6IR-2 · \(menuTitle) · Done")
}

@Test func menuItemsCarryPRFields() {
    let (store, _, _) = menuStore(extra: [assistantLine("https://github.com/a/b/pull/12")])
    let items = store.sessions.first?.menu.first { $0.title == "PR" }?.items ?? []
    #expect(items.first?.kind == .pr)
    #expect(items.first?.code == "#7571")
    #expect(items.first?.status == "mở")
    #expect(items.first?.tone == .open)
    #expect(items.first?.help == "Fix site scope filter")
    #expect(items.last?.kind == .link)
    #expect(items.last?.code == "PR #12")
    #expect(items.last?.tone == nil)
    let merged = Store.menu([], issues: [:], prs: [PRStatus(ref: PRRef(workspace: "w", repo: "r", id: 5), url: URL(string: "https://x.y/5")!, state: "MERGED", reviewers: 0, approved: 0, changesRequested: 0, comments: 0, commit: nil, sourceRepo: nil, branch: nil, title: "T")]).first?.items.first
    #expect(merged?.code == "#5")
    #expect(merged?.status == "đã merge")
    #expect(merged?.tone == .merged)
}

@Test func sessionLinkCarriesIconAndTone() {
    let (store, _, _) = menuStore(jira: typedJira())
    let link = store.sessions.first?.links.first { $0.label == "T6IR-2" }
    #expect(link?.iconURL == iconURL)
    #expect(link?.tone == .done)

    let (bare, _, _) = menuStore(jira: {
        let j = FakeJira()
        j.fail = .transport
        return j
    }())
    let none = bare.sessions.first?.links.first { $0.label == "T6IR-2" }
    #expect(none != nil)
    #expect(none?.iconURL == nil && none?.tone == nil && none?.help == nil)
}

@Test func storeDecodesIssueIcons() {
    let jira = typedJira()
    jira.iconData[iconURL] = png()
    let (store, _, _) = menuStore(jira: jira)
    #expect(store.issueIcons[iconURL] != nil)

    let bad = typedJira()
    bad.iconData[iconURL] = Data([1, 2, 3])
    let (store2, _, _) = menuStore(jira: bad)
    #expect(store2.issueIcons[iconURL] == nil)

    // Decoding happens once per URL: a later valid image for the same URL is ignored.
    let (store3, _, _) = menuStore()
    store3.decodeIcons([iconURL: Data([1, 2, 3])])
    store3.decodeIcons([iconURL: png()])
    #expect(store3.issueIcons[iconURL] == nil)
}

@Test func menuNeedsJiraWhenNotOk() {
    let (store, _, _) = menuStore()
    #expect(store.sessions.first?.menuNeedsJira == false)

    let failing = FakeJira()
    failing.fail = .http(401)
    let (rejected, _, _) = menuStore(jira: failing)
    #expect(rejected.sessions.first?.menuNeedsJira == true)

    let dir = makeTempDir(), projects = makeTempDir()
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [userLine("T6IR-2"), ""])
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    let bare = makeStore(dir: dir, projects: projects)
    bare.jiraProjects = ["T6IR": jiraHost]
    bare.refresh(at: prT0)
    #expect(bare.jiraMonitor == nil)
    #expect(bare.sessions.first?.menuNeedsJira == true)

    let (noTickets, _, _) = prStore(fake(open: [prJSON(id: 7571)]), reply: prReply([7571]))
    noTickets.refresh(at: prT0)
    #expect(noTickets.sessions.first?.menuNeedsJira == false)
}

@Test func menuCountExcludesFooter() {
    let (store, _, _) = menuStore(jira: {
        let j = FakeJira()
        j.fail = .transport
        return j
    }())
    let s = store.sessions.first
    #expect(s?.menuNeedsJira == true)
    #expect(s?.menuCount == 2 + 1 + 4)
}

@Test func menuRowForPROnlySession() {
    let (store, _, _) = prStore(fake(open: [prJSON(id: 7571)]), reply: prReply([7571]))
    store.refresh(at: prT0)
    let s = store.sessions.first
    #expect(s?.links.isEmpty == true)
    #expect(s?.prs.count == 1)
    #expect(s?.menu.map(\.title) == ["PR"])
}

@Test func jiraTargetsOnlyKnownHosts() {
    let (_, jira, _) = menuStore(extra: [userLine("X9-1 please")])
    #expect(!jira.calls.isEmpty)
    #expect(jira.calls.allSatisfy { $0.host == jiraHost })
    #expect(jira.calls.flatMap(\.keys).allSatisfy { !$0.hasPrefix("X9") })
}

@Test func jiraTargetsSkipForeignHosts() {
    let jira = FakeJira()
    let dir = makeTempDir(), projects = makeTempDir()
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [userLine("T6IR-2"), ""])
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    let store = makeStore(dir: dir, projects: projects, jira: JiraMonitor(api: jira, queue: nil))
    store.jiraProjects = ["T6IR": "evil.example"]
    store.accounts[.jira] = "pluu@talosix.com"
    store.refresh(at: prT0)
    #expect(jira.calls.isEmpty)
    #expect(store.connectionText(.jira) == "Jira: ✗ chưa biết site Jira")
}

@Test func learnedAtlassianHostReplacesForeignOne() {
    func learn(stored: String) -> String? {
        let dir = makeTempDir(), projects = makeTempDir()
        _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [userLine("https://talosix.atlassian.net/browse/T6IR-2"), ""])
        writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
        let store = makeStore(dir: dir, projects: projects)
        store.jiraProjects = ["T6IR": stored]
        store.refresh(at: prT0)
        return store.jiraProjects["T6IR"]
    }
    #expect(learn(stored: "evil.example") == "talosix.atlassian.net")
    #expect(learn(stored: "other.atlassian.net") == "other.atlassian.net")
}

@Test func ticketButtonHelp() {
    let (store, _, _) = menuStore()
    let links = store.sessions.first?.links
    #expect(links?.first { $0.label == "T6IR-2" }?.help == "T6IR-2 · \(menuTitle) · To Do")
    #expect(links?.first { $0.label == "T6IR-3" }?.help == nil)
}

@Test func connectionTexts() {
    let email = "pluu@talosix.com"
    let (bare, _, _) = menuStore(prs: false)
    #expect(bare.connectionText(.bitbucket) == "Bitbucket: ✗ chưa có token")

    let api = fake(open: [prJSON(id: 7571)])
    let (store, _, _) = prStore(api, reply: prReply([7571]))
    store.accounts[.bitbucket] = email
    #expect(store.connectionText(.bitbucket) == "Bitbucket: ✗ chưa có token")  // no refresh yet
    store.refresh(at: prT0)
    #expect(store.connectionText(.bitbucket) == "Bitbucket: ✓ \(email)")

    func text(failing e: BitbucketError) -> String {
        let f = fake(open: [prJSON(id: 7571)])
        f.failAll = e
        let (s, _, _) = prStore(f, reply: prReply([7571]))
        s.accounts[.bitbucket] = email
        s.refresh(at: prT0)
        return s.connectionText(.bitbucket)
    }
    #expect(text(failing: .http(401)) == "Bitbucket: ✗ token bị từ chối (401)")
    #expect(text(failing: .transport) == "Bitbucket: ✗ không kết nối được")

    // Idle monitor that never called: still checking.
    let (idle, _, _) = prStore(fake(), reply: assistantLine("hi"))
    idle.accounts[.bitbucket] = email
    idle.refresh(at: prT0)
    #expect(idle.connectionText(.bitbucket) == "Bitbucket: \(email) (đang kiểm tra)")

    let noSite = makeStore(dir: makeTempDir())
    noSite.accounts[.jira] = email
    noSite.refresh(at: prT0)
    #expect(noSite.connectionText(.jira) == "Jira: ✗ chưa biết site Jira")

    let (withJira, _, _) = menuStore()
    withJira.accounts[.jira] = email
    withJira.refresh(at: prT0.addingTimeInterval(1))
    #expect(withJira.connectionText(.jira) == "Jira: ✓ \(email)")
    let (noToken, _, _) = menuStore(prs: false)
    noToken.jiraMonitor = nil
    noToken.refresh(at: prT0.addingTimeInterval(1))
    #expect(noToken.connectionText(.jira) == "Jira: ✗ chưa có token")
}

@Test func exitedButUnreapedPidIsFiltered() throws {
    // A child that exited but was not waited for: kill(pid, 0) still returns 0 (like a Claude stuck exiting under its parent).
    var pid: pid_t = 0
    let argv: [UnsafeMutablePointer<CChar>?] = [strdup("/usr/bin/true"), nil]
    defer { argv.forEach { free($0) } }
    #expect(posix_spawn(&pid, "/usr/bin/true", nil, nil, argv, nil) == 0)
    defer { var st: Int32 = 0; waitpid(pid, &st, 0) }
    var info = siginfo_t()
    #expect(waitid(P_PID, id_t(pid), &info, WEXITED | WNOWAIT) == 0)
    #expect(kill(pid, 0) == 0)
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    writeSession(dir, "live", pid: me, status: "idle", ageSec: 0, name: "live", cwd: frontend)
    writeSession(dir, "zombie", pid: pid, status: "idle", ageSec: 0, name: "zombie", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.sessions.map(\.id) == ["live"])
}

/// A live child process to stand in for Claude; `endProcess` kills and reaps it, so its pid is really gone.
private func spawnSleeper() -> pid_t {
    var pid: pid_t = 0
    let argv: [UnsafeMutablePointer<CChar>?] = [strdup("/bin/sleep"), strdup("60"), nil]
    defer { argv.forEach { free($0) } }
    posix_spawn(&pid, "/bin/sleep", nil, nil, argv, nil)
    return pid
}

private func endProcess(_ pid: pid_t) {
    kill(pid, SIGKILL)
    var st: Int32 = 0
    waitpid(pid, &st, 0)
}

@Test func deadSessionGoesToClosed() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir, branch: { _ in "feat/x" })
    writeSession(dir, "dead", pid: 999_999, status: "idle", ageSec: 10, name: "dead", cwd: "/tmp")
    store.refresh(at: Date())
    #expect(store.closed.items.isEmpty)
    let child = spawnSleeper()
    writeSession(dir, "a", pid: child, status: "idle", ageSec: 10, name: "frontend", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.closed.items.isEmpty)
    endProcess(child)
    store.refresh(at: Date())
    let c = store.closed.items.first
    #expect(c?.id == "a")
    #expect(c?.title == "frontend")
    #expect(c?.cwd == frontend)
    #expect(c?.branch == "feat/x")
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 10, name: "frontend", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.closed.items.isEmpty)
}

@Test func closedPersistsAcrossStores() {
    let suite = "closed-\(UUID().uuidString)"
    let defaults = UserDefaults(suiteName: suite)!
    defer { defaults.removePersistentDomain(forName: suite) }
    let dir = makeTempDir()
    let store = Store(dir: dir, projects: makeTempDir(), tasks: makeTempDir(), scanner: TranscriptScanner(queue: nil), branch: { _ in nil }, closedStore: defaults)
    let child = spawnSleeper()
    writeSession(dir, "a", pid: child, status: "idle", ageSec: 10, name: "frontend", cwd: frontend)
    store.refresh(at: Date())
    endProcess(child)
    store.refresh(at: Date())
    let again = Store(dir: makeTempDir(), projects: makeTempDir(), tasks: makeTempDir(), scanner: TranscriptScanner(queue: nil), branch: { _ in nil }, closedStore: defaults)
    #expect(again.closed.items.map(\.id) == ["a"])
}

private func feedStore(_ items: [[String: Any]]) -> (Store, URL, FakeFeedTransport) {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    let t = FakeFeedTransport()
    t.items = items
    store.feedMonitor = FeedMonitor(transport: t, queue: nil)
    return (store, dir, t)
}

@Test func questionAttachesToTheSessionOfItsId() {
    let (store, dir, _) = feedStore([feedItem(requestId: "R1", sessionId: "b")])
    writeSession(dir, "a", pid: me, status: "waiting", waitingFor: "input needed", ageSec: 0, name: "a", cwd: frontend)
    writeSession(dir, "b", pid: me, status: "waiting", waitingFor: "input needed", ageSec: 0, name: "b", cwd: backend)
    store.refresh(at: Date())
    #expect(store.sessions.first { $0.id == "b" }?.question?.requestId == "R1")
    #expect(store.sessions.first { $0.id == "a" }?.question == nil)
    // An unknown session id attaches to nobody, even when the pid is shared.
    let (s2, d2, _) = feedStore([feedItem(requestId: "R2", sessionId: "other-id")])
    writeSession(d2, "a", pid: me, status: "waiting", waitingFor: "input needed", ageSec: 0, name: "a", cwd: frontend)
    s2.refresh(at: Date())
    #expect(s2.sessions.allSatisfy { $0.question == nil })
}

@Test func questionPollsOnlyForBusyOrAskingSessions() {
    let (store, dir, t) = feedStore([feedItem()])
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(t.calls.isEmpty)
    writeSession(dir, "a", pid: me, status: "waiting", waitingFor: "permission prompt", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(t.calls.isEmpty)
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(t.calls.count == 1)
}

@Test func questionCountsAsWaiting() {
    let (store, dir, t) = feedStore([])
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    let t0 = Date()
    store.refresh(at: t0)
    #expect(store.unseen["a"] == nil && store.transitions.isEmpty)
    // The question shows up while the session still reads `busy`.
    t.items = [feedItem(requestId: "R1", sessionId: "a")]
    store.refresh(at: t0.addingTimeInterval(1))
    #expect(store.unseen["a"]?.claude == true)
    #expect(store.transitions["a"] == .needsYou)
    #expect(store.chime == .needsYou)
    let chime = store.applyAlert(transitions: store.transitions, prChime: store.prChime, excluding: [], at: t0.addingTimeInterval(1))
    #expect(chime == .needsYou)
    #expect(store.peeking)
    // The same question is announced once; the later busy -> waiting flip stays quiet.
    store.refresh(at: t0.addingTimeInterval(2))
    #expect(store.transitions.isEmpty)
    writeSession(dir, "a", pid: me, status: "waiting", waitingFor: "input needed", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: t0.addingTimeInterval(3))
    #expect(store.transitions.isEmpty && store.chime == nil)
    #expect(store.unseen["a"]?.claude == true)
}

@Test func questionNotAnnouncedWhenAlreadyThereAtFirstSight() {
    let (store, dir, _) = feedStore([feedItem(requestId: "R1", sessionId: "a")])
    writeSession(dir, "a", pid: me, status: "waiting", waitingFor: "input needed", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.unseen["a"] == nil && store.transitions.isEmpty)
}

@Test func sentMarkShownUntilSessionRunsAgain() {
    let (store, dir, _) = feedStore([feedItem(requestId: "R1", sessionId: "a")])
    writeSession(dir, "a", pid: me, status: "waiting", waitingFor: "input needed", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    store.noteSent(session: "a", requestId: "R1")
    #expect(store.sent["a"] == "R1")
    store.refresh(at: Date())
    #expect(store.sent["a"] == "R1")
    // It stays while the session reads `busy` (the reply is still being taken up) and goes once it is idle.
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.sent["a"] == "R1")
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.sent["a"] == nil)
}

@Test func failedSendFallsBackAndIsKeptLikeSent() {
    let (store, dir, _) = feedStore([feedItem(requestId: "R1", sessionId: "a")])
    writeSession(dir, "a", pid: me, status: "waiting", waitingFor: "input needed", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    let s = store.sessions[0]
    #expect(store.blockState(of: s) == .form)
    store.noteSent(session: "a", requestId: "R1")
    #expect(store.blockState(of: store.sessions[0]) == .sent)
    store.noteSendFailed(session: "a", requestId: "R1")
    #expect(store.sent["a"] == nil && store.failed["a"] == "R1")
    #expect(store.blockStates["a"] == .fallback)
    #expect(Layout.expanded(store.sessions, blocks: store.blockStates).height < Layout.expanded(store.sessions).height)
    store.refresh(at: Date())
    #expect(store.failed["a"] == "R1")
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.failed["a"] == nil)
}

@Test func closeIsFoundWhenTheFileGoesBeforeTheProcess() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    let child = spawnSleeper()
    writeSession(dir, "a", pid: child, status: "idle", ageSec: 10, name: "a", cwd: frontend)
    store.refresh(at: Date())
    // Claude deletes the file while the process is still exiting.
    try? FileManager.default.removeItem(at: dir.appendingPathComponent("a.json"))
    store.refresh(at: Date())
    #expect(store.sessions.isEmpty)
    #expect(store.closed.items.isEmpty)
    endProcess(child)
    store.refresh(at: Date())
    #expect(store.closed.items.map(\.id) == ["a"])
}

@Test func clearKeepsPidSoNothingIsClosed() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 10, name: "a", cwd: frontend)
    store.refresh(at: Date())
    // `/clear`: the same process, a new session id.
    try? FileManager.default.removeItem(at: dir.appendingPathComponent("a.json"))
    writeSession(dir, "b", pid: me, status: "idle", ageSec: 10, name: "b", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.sessions.map(\.id) == ["b"])
    #expect(store.closed.items.isEmpty)
}

@Test func replyAndDenyDraftsAreKeptApart() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 10, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(Store.draftKey("a", .reply) != Store.draftKey("a", .deny))
    store.drafts[Store.draftKey("a", .reply)] = "half a reply"
    #expect(store.drafts[Store.draftKey("a", .deny)] == nil)
    // Both survive a refresh while the session lives, and go with it.
    store.refresh(at: Date())
    #expect(store.drafts[Store.draftKey("a", .reply)] == "half a reply")
    try? FileManager.default.removeItem(at: dir.appendingPathComponent("a.json"))
    store.refresh(at: Date())
    #expect(store.drafts.isEmpty)
}

@Test func answerValidOnlyWhileItsQuestionIsLive() {
    let (store, dir, t) = feedStore([feedItem(requestId: "R1", sessionId: "a")])
    writeSession(dir, "a", pid: me, status: "waiting", waitingFor: "input needed", ageSec: 0, name: "a", cwd: frontend)
    let t0 = Date()
    store.refresh(at: t0)
    let ok = WidgetAction.answer("a", requestId: "R1", answers: [["Red"]], other: [:])
    #expect(store.stillValid(ok))
    #expect(!store.stillValid(.answer("a", requestId: "R2", answers: [["Red"]], other: [:])))
    #expect(!store.stillValid(.answer("zzz", requestId: "R1", answers: [["Red"]], other: [:])))
    #expect(store.allows(Store.Editing(id: "a", kind: .answer)))
    t.items = []
    store.refresh(at: t0.addingTimeInterval(1))
    #expect(!store.stillValid(ok))
    #expect(!store.allows(Store.Editing(id: "a", kind: .answer)))
}
