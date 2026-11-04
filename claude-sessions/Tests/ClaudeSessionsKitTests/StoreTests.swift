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
    #expect(store.unseen.isEmpty && !store.expanded)
}

@Test func busyToIdleMarksUnseenUntilSeen() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    let t0 = Date()
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 42, name: "talo6-dct-frontend-d7", cwd: frontend)
    writeSession(dir, "b", pid: me, status: "busy", ageSec: 300, name: "talo6-dct-backend-3f", cwd: backend)
    store.refresh(at: t0)

    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: frontend)
    store.refresh(at: t0.addingTimeInterval(1))
    #expect(store.unseen == ["a"])
    #expect(store.expanded)
    #expect(store.sessions.first?.id == "a")

    store.refresh(at: t0.addingTimeInterval(7))
    #expect(!store.expanded)
    #expect(store.unseen == ["a"])

    store.refresh(at: t0.addingTimeInterval(30))
    #expect(store.unseen == ["a"])
    store.markSeen("a")
    #expect(store.unseen.isEmpty)
}

@Test func waitingOrderingAndLabels() {
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
    #expect(store.unseen == ["a", "b"])
    #expect(store.sessions.map(\.id) == ["c", "b", "a", "d"])
    #expect(store.sessions.first { $0.id == "b" }.map(label) == "chờ duyệt")
    #expect(store.sessions.first { $0.id == "c" }.map(label) == "chờ trả lời")
}

@Test func hoverExpands() {
    let store = makeStore(dir: makeTempDir())
    store.hovering = true
    #expect(store.expanded)
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
    store.markSeen("a")
    #expect(store.unseen.isEmpty && !store.expanded)

    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: frontend)
    store.refresh(at: t0.addingTimeInterval(31))
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: frontend)
    store.refresh(at: t0.addingTimeInterval(32))
    #expect(store.unseen == ["a"] && store.expanded)
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

@Test func unseenClearedOnBusy() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    let t0 = Date()
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: t0)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: t0.addingTimeInterval(1))
    #expect(store.unseen == ["a"])
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: t0.addingTimeInterval(2))
    #expect(store.unseen.isEmpty)
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
    #expect(store.stillValid(.jump("zzz")))
}

@Test func editingClearedWhenSessionGone() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    store.editing = Store.Editing(id: "a", kind: .deny, toolId: "t1")
    try! FileManager.default.removeItem(at: dir.appendingPathComponent("a.json"))
    store.refresh(at: Date())
    #expect(store.editing == nil && !store.expanded)
}

@Test func editingKeepsExpanded() {
    let store = makeStore(dir: makeTempDir())
    store.hovering = false
    store.editing = Store.Editing(id: "a", kind: .deny, toolId: "t1")
    #expect(store.expanded)
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
    let store = Store(dir: makeTempDir(), projects: makeTempDir(), startHidden: true)
    store.refresh(at: Date())
    #expect(store.hidden)
    let dir = makeTempDir()
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 5, name: "a", cwd: frontend)
    let live = Store(dir: dir, projects: makeTempDir(), startHidden: true)
    live.refresh(at: Date())
    #expect(!live.hidden)
}

@Test func defaultStartsVisible() {
    let store = Store(dir: makeTempDir(), projects: makeTempDir(), startHidden: false)
    store.refresh(at: Date())
    #expect(!store.hidden)
}

@Test func surfaceDroppedWhenPidChanges() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    store.surfaces["a"] = "S"
    store.surfaceMisses.insert("a")
    store.refresh(at: Date())
    #expect(store.surfaces["a"] == "S" && store.surfaceMisses.contains("a"))
    #expect(store.sessions.map(\.id) == ["a"])
    let other = getppid()
    writeSession(dir, "a", pid: other, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.sessions.map(\.id) == ["a"])
    #expect(store.surfaces["a"] == nil && store.surfaceMisses.isEmpty)
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

@Test func editingDenyClosedWhenToolChanges() {
    let (store, dir, _, f) = approvableStore()
    writeSession(dir, "a", pid: me, status: "waiting", waitingFor: "permission prompt", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    store.editing = Store.Editing(id: "a", kind: .deny, toolId: "t1")
    store.refresh(at: Date())
    #expect(store.editing != nil)
    try! Data([toolUseLine(id: "t1", name: "Bash", input: ["command": "npm test"]), toolResultLine(id: "t1"),
               toolUseLine(id: "t2", name: "Bash", input: ["command": "ls"])].joined(separator: "\n").utf8).write(to: f)
    writeSession(dir, "a", pid: me, status: "waiting", waitingFor: "permission prompt", ageSec: 1, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.editing == nil)
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
