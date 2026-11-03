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

    try! Data([toolUseLine(id: "t1", name: "Bash", input: ["command": "npm test"]), toolResultLine(id: "t1")].joined(separator: "\n").utf8).write(to: f)
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(!store.stillValid(.approve("a", toolId: "t1")))
    #expect(!store.stillValid(.reply("a", text: "x")))
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.stillValid(.reply("a", text: "x")))
    #expect(store.stillValid(.jump("zzz")))
}

@Test func editingClearedWhenSessionGone() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    store.editing = Store.Editing(id: "a", kind: .reply)
    try! FileManager.default.removeItem(at: dir.appendingPathComponent("a.json"))
    store.refresh(at: Date())
    #expect(store.editing == nil && !store.expanded)
}

@Test func editingClearedWhenReplyNotAllowed() {
    let dir = makeTempDir()
    let store = makeStore(dir: dir)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    store.editing = Store.Editing(id: "a", kind: .reply)
    store.refresh(at: Date())
    #expect(store.editing != nil)
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.editing == nil)
}

@Test func editingKeepsExpanded() {
    let store = makeStore(dir: makeTempDir())
    store.hovering = false
    store.editing = Store.Editing(id: "a", kind: .reply)
    #expect(store.expanded)
}
