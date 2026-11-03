import Foundation
import Testing
@testable import ClaudeSessionsKit

// Own pid keeps fake sessions alive (kill(pid, 0)); 999_999 is a dead one.
private let me = getpid()

@Test func emptyDirHasNoSessions() {
    let store = Store(dir: makeSessionsDir())
    store.refresh(at: Date())
    #expect(store.sessions.isEmpty)
}

@Test func deadPidIsFilteredAndFirstSightIsQuiet() {
    let dir = makeSessionsDir()
    let store = Store(dir: dir)
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 42, name: "talo6-dct-frontend-d7", cwd: frontend)
    writeSession(dir, "b", pid: me, status: "busy", ageSec: 300, name: "talo6-dct-backend-3f", cwd: backend)
    writeSession(dir, "dead", pid: 999_999, status: "idle", ageSec: 10, name: "dead", cwd: "/tmp")
    store.refresh(at: Date())
    #expect(store.sessions.count == 2)
    #expect(store.attention.isEmpty && !store.expanded)
}

@Test func busyToIdleFlagsAttentionPeeksAndExpires() {
    let dir = makeSessionsDir()
    let store = Store(dir: dir)
    let t0 = Date()
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 42, name: "talo6-dct-frontend-d7", cwd: frontend)
    writeSession(dir, "b", pid: me, status: "busy", ageSec: 300, name: "talo6-dct-backend-3f", cwd: backend)
    store.refresh(at: t0)

    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: frontend)
    store.refresh(at: t0.addingTimeInterval(1))
    #expect(store.attention == ["a"])
    #expect(store.expanded)
    #expect(store.sessions.first?.id == "a")

    store.refresh(at: t0.addingTimeInterval(7))
    #expect(!store.expanded)
    #expect(store.attention == ["a"])

    store.refresh(at: t0.addingTimeInterval(30))
    #expect(store.attention.isEmpty)
}

@Test func waitingOrderingAndLabels() {
    let dir = makeSessionsDir()
    let store = Store(dir: dir)
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
    #expect(store.attention == ["a", "b"])
    #expect(store.sessions.map(\.id) == ["c", "b", "a", "d"])
    #expect(store.sessions.first { $0.id == "b" }.map(label) == "chờ duyệt")
    #expect(store.sessions.first { $0.id == "c" }.map(label) == "chờ trả lời")
}

@Test func hoverExpands() {
    let store = Store(dir: makeSessionsDir())
    store.hovering = true
    #expect(store.expanded)
}

@Test func secondTurnFinishingPeeksAgain() {
    let dir = makeSessionsDir()
    let store = Store(dir: dir)
    let t0 = Date()
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: frontend)
    store.refresh(at: t0)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: frontend)
    store.refresh(at: t0.addingTimeInterval(1))
    store.refresh(at: t0.addingTimeInterval(30))
    #expect(store.attention.isEmpty && !store.expanded)

    writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: frontend)
    store.refresh(at: t0.addingTimeInterval(31))
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: frontend)
    store.refresh(at: t0.addingTimeInterval(32))
    #expect(store.attention == ["a"] && store.expanded)
}

private func writeTranscript(_ projects: URL, cwd: String, id: String, lines: [String]) -> URL {
    let d = projects.appendingPathComponent(Transcript.projectDirName(cwd))
    try! FileManager.default.createDirectory(at: d, withIntermediateDirectories: true)
    let f = d.appendingPathComponent("\(id).jsonl")
    try! Data(lines.joined(separator: "\n").utf8).write(to: f)
    return f
}

@Test func lastReplyOnIdle() {
    let dir = makeSessionsDir(), projects = makeSessionsDir()
    let store = Store(dir: dir, projects: projects)
    _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [assistantLine("all done")])
    writeSession(dir, "a", pid: me, status: "busy", ageSec: 5, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.sessions.first?.lastReply == nil)
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    #expect(store.sessions.first?.lastReply == "all done")
}

@Test func noRereadWhenUnchanged() {
    let dir = makeSessionsDir(), projects = makeSessionsDir()
    let store = Store(dir: dir, projects: projects)
    let f = writeTranscript(projects, cwd: frontend, id: "a", lines: [assistantLine("aaaa")])
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    try! Data(assistantLine("bbbb").utf8).write(to: f)
    store.refresh(at: Date())
    #expect(store.sessions.first?.lastReply == "aaaa")
}

@Test func rereadsWhenTranscriptGrows() {
    let dir = makeSessionsDir(), projects = makeSessionsDir()
    let store = Store(dir: dir, projects: projects)
    let f = writeTranscript(projects, cwd: frontend, id: "a", lines: [assistantLine("first")])
    writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "a", cwd: frontend)
    store.refresh(at: Date())
    try! Data([assistantLine("first"), assistantLine("a longer final text")].joined(separator: "\n").utf8).write(to: f)
    store.refresh(at: Date())
    #expect(store.sessions.first?.lastReply == "a longer final text")
}
