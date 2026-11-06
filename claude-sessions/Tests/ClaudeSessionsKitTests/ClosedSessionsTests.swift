import Foundation
import Testing
@testable import ClaudeSessionsKit

private func closed(_ id: String, at: Date) -> ClosedSession {
    ClosedSession(id: id, title: "t \(id)", cwd: "/tmp/x", branch: "main", closedAt: at)
}

@Test func addCapsAtFive() {
    let now = Date()
    var c = ClosedSessions()
    for i in 0..<7 { c.add(closed("s\(i)", at: now), now: now) }
    #expect(c.items.count == 5)
    #expect(c.items.map(\.id) == ["s6", "s5", "s4", "s3", "s2"])
}

@Test func dropsAfter24h() {
    let now = Date()
    var c = ClosedSessions()
    c.add(closed("old", at: now.addingTimeInterval(-23 * 3600)), now: now)
    c.add(closed("new", at: now), now: now)
    #expect(c.items.map(\.id) == ["new", "old"])
    c.prune(now: now.addingTimeInterval(2 * 3600))
    #expect(c.items.map(\.id) == ["new"])
    c.add(closed("stale", at: now.addingTimeInterval(-25 * 3600)), now: now)
    #expect(c.items.map(\.id) == ["new"])
}

@Test func noDuplicateIds() {
    let now = Date()
    var c = ClosedSessions()
    c.add(closed("a", at: now), now: now)
    c.add(closed("b", at: now), now: now)
    c.add(closed("a", at: now.addingTimeInterval(5)), now: now.addingTimeInterval(5))
    #expect(c.items.map(\.id) == ["a", "b"])
}

@Test func revivedIsRemoved() {
    let now = Date()
    var c = ClosedSessions()
    for id in ["a", "b", "c"] { c.add(closed(id, at: now), now: now) }
    c.revive(["a", "c", "zzz"])
    #expect(c.items.map(\.id) == ["b"])
}

@Test func codableRoundTrip() throws {
    let a = ClosedSession(id: "a", title: "Tiêu đề", cwd: "/tmp/it's", branch: nil, closedAt: Date())
    let b = closed("b", at: Date().addingTimeInterval(-60))
    let data = try JSONEncoder().encode([a, b])
    #expect(try JSONDecoder().decode([ClosedSession].self, from: data) == [a, b])
}
