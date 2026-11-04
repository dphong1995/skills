import Foundation
import Testing
@testable import ClaudeSessionsKit

@Test func tasksMissingDirIsNil() {
    #expect(Tasks.read(makeTempDir().appendingPathComponent("nope")) == nil)
}

@Test func tasksEmptyDirIsNil() {
    let dir = makeTempDir()
    try! Data().write(to: dir.appendingPathComponent(".highwatermark"))
    try! Data().write(to: dir.appendingPathComponent(".lock"))
    #expect(Tasks.read(dir) == nil)
}

@Test func tasksAllDoneIsNil() {
    let dir = makeTempDir()
    writeTask(dir, session: "", id: 1, status: "completed", subject: "A")
    writeTask(dir, session: "", id: 2, status: "completed", subject: "B")
    #expect(Tasks.read(dir) == nil)
}

@Test func tasksPrefersInProgress() {
    let dir = makeTempDir()
    writeTask(dir, session: "", id: 1, status: "completed", subject: "Phase 1")
    writeTask(dir, session: "", id: 2, status: "in_progress", subject: "Phase 2", activeForm: "Implementing phase 2")
    writeTask(dir, session: "", id: 3, status: "pending", subject: "Phase 3")
    writeTask(dir, session: "", id: 10, status: "in_progress", subject: "Phase 10", activeForm: "Implementing phase 10")
    #expect(Tasks.read(dir) == Todo(done: 1, total: 4, current: "Implementing phase 2"))
}

@Test func tasksInProgressWithoutActiveFormUsesSubject() {
    let dir = makeTempDir()
    writeTask(dir, session: "", id: 1, status: "in_progress", subject: "Phase 1")
    #expect(Tasks.read(dir) == Todo(done: 0, total: 1, current: "Phase 1"))
}

@Test func tasksFallsBackToFirstPending() {
    let dir = makeTempDir()
    writeTask(dir, session: "", id: 1, status: "completed", subject: "Phase 1")
    writeTask(dir, session: "", id: 2, status: "pending", subject: "Phase 2")
    writeTask(dir, session: "", id: 3, status: "pending", subject: "Phase 3")
    #expect(Tasks.read(dir) == Todo(done: 1, total: 3, current: "Phase 2"))
}

@Test func tasksSkipsBrokenFile() {
    let dir = makeTempDir()
    writeTask(dir, session: "", id: 1, status: "pending", subject: "Phase 1")
    try! Data(#"{"id":"2","sub"#.utf8).write(to: dir.appendingPathComponent("2.json"))
    #expect(Tasks.read(dir) == Todo(done: 0, total: 1, current: "Phase 1"))
}

@Test func tasksIgnoresUnknownStatus() {
    let dir = makeTempDir()
    writeTask(dir, session: "", id: 1, status: "pending", subject: "Phase 1")
    writeTask(dir, session: "", id: 2, status: "deleted", subject: "Phase 2")
    #expect(Tasks.read(dir) == Todo(done: 0, total: 1, current: "Phase 1"))
}
