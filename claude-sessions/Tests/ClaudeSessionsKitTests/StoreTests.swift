import Foundation
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

@Test func busySessionHasPromptTitleAndHeadBranch() {
    let dir = makeTempDir(), projects = makeTempDir()
    let store = makeStore(dir: dir, projects: projects)
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [userLine("chạy test", branch: "main")])
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

@Test func branchStaysWhileBusyAfterTail() {
    let dir = makeTempDir(), projects = makeTempDir()
    let store = makeStore(dir: dir, projects: projects)
    let t0 = Date()
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [userLine("x", branch: "main"), assistantLine("y", branch: "feat")])
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend, updatedAt: t0)
    store.refresh(at: t0)
    #expect(store.sessions[0].subtitle.hasSuffix("· feat"))

    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend, updatedAt: t0.addingTimeInterval(1))
    store.refresh(at: t0.addingTimeInterval(1))
    #expect(store.sessions[0].subtitle.hasSuffix("· feat"))
}

@Test func titleCutAt140() {
    let dir = makeTempDir(), projects = makeTempDir()
    let store = makeStore(dir: dir, projects: projects)
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [userLine(String(repeating: "t", count: 200))])
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 5, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.sessions.first?.title == String(repeating: "t", count: 140) + "…")
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
        userLine("https://\(jiraHost)/browse/T6IR-10", branch: "bugfix/T6IR-30"),
        userLine("T6IR-11"),
        assistantLine("T6IR-20"),
        assistantLine("T6IR-21")])
    #expect(labels(store) == ["T6IR-30", "T6IR-10", "T6IR-11", "T6IR-21", "T6IR-20"])
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
    let a = writeTranscript(projects, cwd: frontend, id: "a", lines: [userLine("T6IR-5", branch: "main"), ""])
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
    // A new branch puts its ticket first.
    append(a, userLine("x", branch: "bugfix/T6IR-30") + "\n")
    store.refresh(at: Date())
    #expect(links("a") == ["T6IR-30", "T6IR-5"])
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
    #expect(store.unseen == ["a", "b"])
}

@Test func unseenKeptAcrossRefreshes() {
    let (store, _, t0) = markedStore()
    for i in 1...3 { store.refresh(at: t0.addingTimeInterval(1 + Double(i) * 30)) }
    #expect(store.unseen == ["a"])
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
    #expect(!store.unseen.contains("a"))
}

@Test func markSeenClears() {
    let (store, _, _) = markedStore()
    store.markSeen("zzz")
    #expect(store.unseen == ["a"])
    store.markSeen("a")
    #expect(store.unseen.isEmpty)
}

@Test func newCountCountsLiveOnly() {
    let (store, _, _) = markedStore()
    store.unseen.insert("dead")
    #expect(store.newCount == 1)
}

@Test func noteFocusedNeedsTwoConsecutive() {
    let (store, _, _) = markedStore()
    store.surfaces["a"] = "S1"
    store.noteFocused("S1")
    #expect(store.unseen == ["a"])
    store.noteFocused("s1")
    #expect(store.unseen.isEmpty)
}

@Test func noteFocusedNilResets() {
    let (store, _, _) = markedStore()
    store.surfaces["a"] = "S1"
    store.noteFocused("S1")
    store.noteFocused(nil)
    store.noteFocused("S1")
    #expect(store.unseen == ["a"])
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
    #expect(store.unseen == ["a"])
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
    #expect(store.unseen == ["a", "b"])
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
    #expect(store.unseen == ["a"])
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
    #expect(store.unseen == ["a"])
    store.refresh(at: prT0 + 62)
    #expect(store.unseen == ["a"])
}
