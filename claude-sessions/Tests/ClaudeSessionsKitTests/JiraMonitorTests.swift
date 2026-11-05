import Testing
import Foundation
@testable import ClaudeSessionsKit

private let t0 = Date(timeIntervalSince1970: 1_800_000_000)
private let host = "talosix.atlassian.net"

private func targets(_ keys: [String], host: String = host) -> [JiraTarget] {
    keys.map { JiraTarget(key: $0, host: host) }
}

@Suite struct JiraMonitorTests {
    @Test func connectionStartsChecking() {
        let m = JiraMonitor(api: FakeJira(), queue: nil)
        #expect(m.takeSnapshot().connection == .checking)
    }

    @Test func newKeysBatchedOnce() {
        let api = FakeJira()
        api.issues = ["A-1": ("one", "Open"), "A-2": ("two", nil), "A-3": ("three", "Done")]
        let m = JiraMonitor(api: api, queue: nil)
        m.update(targets(["A-1", "A-2", "A-3"]), at: t0)
        #expect(api.calls.count == 1)
        #expect(api.calls[0].keys.sorted() == ["A-1", "A-2", "A-3"])
        let s = m.takeSnapshot()
        #expect(s.issues["A-1"] == JiraIssue(title: "one", status: "Open"))
        #expect(s.issues["A-2"] == JiraIssue(title: "two", status: nil))
        #expect(s.connection == .ok)
    }

    @Test func cachedKeysNotRefetchedBefore900s() {
        let api = FakeJira()
        let m = JiraMonitor(api: api, queue: nil)
        m.update(targets(["A-1", "A-2"]), at: t0)
        m.update(targets(["A-1", "A-2"]), at: t0.addingTimeInterval(10))
        m.update(targets(["A-1", "A-2"]), at: t0.addingTimeInterval(899))
        #expect(api.calls.count == 1)
        m.update(targets(["A-1", "A-2"]), at: t0.addingTimeInterval(900))
        #expect(api.calls.count == 2)
        #expect(api.calls[1].keys.sorted() == ["A-1", "A-2"])
    }

    @Test func newKeyWaitsFiveSeconds() {
        let api = FakeJira()
        let m = JiraMonitor(api: api, queue: nil)
        m.update(targets(["A-1"]), at: t0)
        m.update(targets(["A-1", "A-2"]), at: t0.addingTimeInterval(2))
        #expect(api.calls.count == 1)
        m.update(targets(["A-1", "A-2"]), at: t0.addingTimeInterval(4.9))
        #expect(api.calls.count == 1)
        m.update(targets(["A-1", "A-2"]), at: t0.addingTimeInterval(5))
        #expect(api.calls.count == 2)
        #expect(api.calls[1].keys == ["A-2"])
    }

    @Test func splitsAt100() {
        let api = FakeJira()
        let keys = (1...150).map { "A-\($0)" }
        let m = JiraMonitor(api: api, queue: nil)
        m.update(targets(keys), at: t0)
        #expect(api.calls.map(\.keys.count) == [100, 50])
    }

    @Test func oneCallPerHost() {
        let api = FakeJira()
        let m = JiraMonitor(api: api, queue: nil)
        m.update(targets(["A-1"]) + targets(["B-1"], host: "other.atlassian.net"), at: t0)
        #expect(api.calls.count == 2)
        #expect(Set(api.calls.map(\.host)) == [host, "other.atlassian.net"])
    }

    @Test func missingKeyHasNilTitle() {
        let api = FakeJira()
        api.issues = ["A-1": ("one", "Open")]
        let m = JiraMonitor(api: api, queue: nil)
        m.update(targets(["A-1", "A-9"]), at: t0)
        let s = m.takeSnapshot()
        #expect(s.issues["A-9"] == JiraIssue(title: nil, status: nil))
        #expect(s.issues["A-1"]?.title == "one")
    }

    @Test func rejectedThenRecovers() {
        let api = FakeJira()
        api.issues = ["A-1": ("one", "Open")]
        api.fail = .http(401)
        let m = JiraMonitor(api: api, queue: nil)
        m.update(targets(["A-1"]), at: t0)
        #expect(m.takeSnapshot().connection == .rejected)
        m.update(targets(["A-1"]), at: t0.addingTimeInterval(59))
        #expect(api.calls.count == 1)
        api.fail = nil
        m.update(targets(["A-1"]), at: t0.addingTimeInterval(60))
        #expect(api.calls.count == 2)
        let s = m.takeSnapshot()
        #expect(s.connection == .ok)
        #expect(s.issues["A-1"]?.title == "one")
    }

    @Test func refreshFailureRetriesAfter60s() {
        let api = FakeJira()
        api.issues = ["A-1": ("one", "Open")]
        let m = JiraMonitor(api: api, queue: nil)
        m.update(targets(["A-1"]), at: t0)
        #expect(m.takeSnapshot().connection == .ok)
        api.fail = .http(401)
        m.update(targets(["A-1"]), at: t0.addingTimeInterval(900))
        #expect(m.takeSnapshot().connection == .rejected)
        #expect(api.calls.count == 2)
        api.fail = nil
        m.update(targets(["A-1"]), at: t0.addingTimeInterval(930))
        #expect(api.calls.count == 2)
        m.update(targets(["A-1"]), at: t0.addingTimeInterval(960))
        #expect(api.calls.count == 3)
        #expect(m.takeSnapshot().connection == .ok)
    }

    @Test func droppedAfterSessionGone() {
        let api = FakeJira()
        api.issues = ["A-1": ("one", "Open"), "A-2": ("two", "Open")]
        let m = JiraMonitor(api: api, queue: nil)
        m.update(targets(["A-1", "A-2"]), at: t0)
        m.update(targets(["A-1"]), at: t0.addingTimeInterval(10))
        #expect(m.takeSnapshot().issues["A-2"] != nil)
        m.update(targets(["A-1"]), at: t0.addingTimeInterval(41))
        #expect(m.takeSnapshot().issues["A-2"] == nil)
    }

    private static let iconA = URL(string: "https://talosix.atlassian.net/avatar/1?size=medium")!

    private func iconAPI(_ url: URL = iconA) -> FakeJira {
        let api = FakeJira()
        api.issues = ["A-1": ("one", "Open"), "A-2": ("two", "Open")]
        api.types = ["A-1": ("Story", url.absoluteString), "A-2": ("Story", url.absoluteString)]
        api.iconData = [url: Data([1, 2, 3])]
        return api
    }

    @Test func iconFetchedOncePerURL() {
        let api = iconAPI()
        let m = JiraMonitor(api: api, queue: nil)
        for dt in [0.0, 10, 20] { m.update(targets(["A-1", "A-2"]), at: t0.addingTimeInterval(dt)) }
        #expect(api.iconCalls.count == 1)
        #expect(m.takeSnapshot().icons[Self.iconA] == Data([1, 2, 3]))
    }

    @Test func iconRetriesAfter60sOnTick() {
        let api = iconAPI()
        api.iconFail = .transport
        let m = JiraMonitor(api: api, queue: nil)
        m.update(targets(["A-1"]), at: t0)
        api.iconFail = nil
        m.update(targets(["A-1"]), at: t0.addingTimeInterval(30))
        #expect(api.iconCalls.count == 1)
        m.update(targets(["A-1"]), at: t0.addingTimeInterval(60))
        #expect(api.iconCalls.count == 2)
        #expect(m.takeSnapshot().icons[Self.iconA] != nil)
        #expect(api.calls.count == 1)
    }

    @Test func iconSkipsForeignHost() {
        let foreign = URL(string: "https://cdn.example/x.png")!
        let api = iconAPI(foreign)
        let m = JiraMonitor(api: api, queue: nil)
        m.update(targets(["A-1"]), at: t0)
        m.update(targets(["A-1"]), at: t0.addingTimeInterval(120))
        #expect(api.iconCalls.isEmpty)
        #expect(m.takeSnapshot().icons.isEmpty)
    }

    @Test func iconSkipsNonHTTPS() {
        let plain = URL(string: "http://talosix.atlassian.net/avatar/1")!
        let api = iconAPI(plain)
        let m = JiraMonitor(api: api, queue: nil)
        m.update(targets(["A-1"]), at: t0)
        m.update(targets(["A-1"]), at: t0.addingTimeInterval(120))
        #expect(api.iconCalls.isEmpty)
        #expect(m.takeSnapshot().icons.isEmpty)
    }

    @Test func iconsSurviveForget() {
        let api = iconAPI()
        let m = JiraMonitor(api: api, queue: nil)
        m.update(targets(["A-1"]), at: t0)
        m.update([], at: t0.addingTimeInterval(1))
        m.update([], at: t0.addingTimeInterval(40))
        #expect(m.takeSnapshot().issues.isEmpty)
        #expect(m.takeSnapshot().icons[Self.iconA] != nil)
    }
}
