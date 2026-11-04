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
    writeTask(dir, id: 1, status: "completed", subject: "A")
    writeTask(dir, id: 2, status: "completed", subject: "B")
    #expect(Tasks.read(dir) == nil)
}

@Test func tasksPrefersInProgress() {
    let dir = makeTempDir()
    writeTask(dir, id: 1, status: "completed", subject: "Phase 1")
    writeTask(dir, id: 2, status: "in_progress", subject: "Phase 2", activeForm: "Implementing phase 2")
    writeTask(dir, id: 3, status: "pending", subject: "Phase 3")
    writeTask(dir, id: 10, status: "in_progress", subject: "Phase 10", activeForm: "Implementing phase 10")
    #expect(Tasks.read(dir) == Todo(done: 1, total: 4, current: "Implementing phase 2"))
}

@Test func tasksInProgressWithoutActiveFormUsesSubject() {
    let dir = makeTempDir()
    writeTask(dir, id: 1, status: "in_progress", subject: "Phase 1")
    #expect(Tasks.read(dir) == Todo(done: 0, total: 1, current: "Phase 1"))
}

@Test func tasksFallsBackToFirstPending() {
    let dir = makeTempDir()
    writeTask(dir, id: 1, status: "completed", subject: "Phase 1")
    writeTask(dir, id: 2, status: "pending", subject: "Phase 2")
    writeTask(dir, id: 3, status: "pending", subject: "Phase 3")
    #expect(Tasks.read(dir) == Todo(done: 1, total: 3, current: "Phase 2"))
}

@Test func tasksSkipsBrokenFile() {
    let dir = makeTempDir()
    writeTask(dir, id: 1, status: "pending", subject: "Phase 1")
    try! Data(#"{"id":"2","sub"#.utf8).write(to: dir.appendingPathComponent("2.json"))
    #expect(Tasks.read(dir) == Todo(done: 0, total: 1, current: "Phase 1"))
}

@Test func tasksIgnoresUnknownStatus() {
    let dir = makeTempDir()
    writeTask(dir, id: 1, status: "pending", subject: "Phase 1")
    writeTask(dir, id: 2, status: "deleted", subject: "Phase 2")
    #expect(Tasks.read(dir) == Todo(done: 0, total: 1, current: "Phase 1"))
}

@Test func tasksEmptyTextIsNotCurrent() {
    let dir = makeTempDir()
    writeTask(dir, id: 1, status: "in_progress", subject: "Phase 1", activeForm: "")
    #expect(Tasks.read(dir) == Todo(done: 0, total: 1, current: "Phase 1"))
    let dir2 = makeTempDir()
    writeTask(dir2, id: 1, status: "pending", subject: "")
    #expect(Tasks.read(dir2) == Todo(done: 0, total: 1, current: nil))
}

@Test func tasksKeepsNonNumericId() {
    let dir = makeTempDir()
    writeTask(dir, id: 1, status: "completed", subject: "A")
    try! Data(#"{"id":"abc","subject":"B","status":"pending"}"#.utf8).write(to: dir.appendingPathComponent("b.json"))
    try! Data(#"{"subject":"C","status":"pending"}"#.utf8).write(to: dir.appendingPathComponent("c.json"))
    #expect(Tasks.read(dir) == Todo(done: 1, total: 3, current: "B"))
}

@Test func todoLabel() {
    #expect(Todo(done: 1, total: 3, current: "Phase 2").label == "☑ 1/3 · Phase 2")
    #expect(Todo(done: 1, total: 3, current: nil).label == "☑ 1/3")
}
