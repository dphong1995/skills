import Testing
import Foundation
@testable import ClaudeSessionsKit

private let be = "t6engineering/talo6-dct-backend"
private let fe = "t6engineering/talo6-dct-frontend"
private let t0 = Date(timeIntervalSince1970: 1_800_000_000)

private func link(_ id: Int, repo: String = be) -> String {
    assistantLine("see https://bitbucket.org/\(repo)/pull-requests/\(id)")
}

private func target(_ transcript: URL? = nil, branch: String? = nil, cwd: String = "/x") -> PRTarget {
    PRTarget(sessionId: "s", transcript: transcript, cwd: cwd, branch: branch)
}

private func monitor(_ api: FakeBitbucket, remote: String? = nil) -> PRMonitor {
    PRMonitor(api: api, queue: nil, remote: { _ in remote })
}

private func transcript(_ lines: [String]) -> URL {
    writeTranscript(makeTempDir(), cwd: "/x", id: "s", lines: lines + [""])
}

/// Routes for the user, an open list and default passing pipeline / no comments.
private func fake(open: [[String: Any]] = []) -> FakeBitbucket {
    let api = FakeBitbucket()
    api.set("/2.0/user", json: ["uuid": "{me}"])
    api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": open])
    api.set("/statuses", json: ["values": [["state": "SUCCESSFUL"]]])
    api.set("/comments", json: ["values": []])
    return api
}

private func withCommit(_ pr: [String: Any], _ hash: String) -> [String: Any] {
    var o = pr
    var source = o["source"] as! [String: Any]
    source["commit"] = ["hash": hash]
    o["source"] = source
    return o
}

private func comment(_ uuid: String, _ name: String, _ at: String) -> [String: Any] {
    ["user": ["uuid": uuid, "display_name": name], "created_on": at, "deleted": false]
}

@Suite struct PRMonitorTests {
    @Test func linkRefsShownAfterFirstCycle() {
        let api = fake(open: [prJSON(id: 7571, reviewers: 3)])
        let m = monitor(api)
        m.update([target(transcript([link(7571)]))], at: t0)
        let snap = m.takeSnapshot()
        let prs = snap.prs["s"] ?? []
        #expect(prs.map(\.ref.id) == [7571])
        #expect(prs.first?.reviewers == 3)
        #expect(prs.first?.approved == 1)
        #expect(prs.first?.changesRequested == 1)
        #expect(prs.first?.pipeline == .passed)
        #expect(snap.lastSuccess == t0)
        #expect(snap.events.isEmpty)
    }

    @Test func scanFindsRefAfterLineCompletes() throws {
        let api = fake(open: [prJSON(id: 7571)])
        let m = monitor(api)
        let line = link(7571) + "\n"
        let cut = line.index(line.startIndex, offsetBy: line.count / 2)
        let url = writeTranscript(makeTempDir(), cwd: "/x", id: "s", lines: [String(line[..<cut])])
        m.update([target(url)], at: t0)
        #expect((m.takeSnapshot().prs["s"] ?? []).isEmpty)
        let h = try FileHandle(forWritingTo: url)
        try h.seekToEnd()
        try h.write(contentsOf: Data(line[cut...].utf8))
        try h.close()
        m.update([target(url)], at: t0 + 1)
        #expect(m.takeSnapshot().prs["s"]?.map(\.ref.id) == [7571])
    }

    @Test func firstScanReadsLargeFileInChunks() {
        let api = fake(open: [prJSON(id: 7571), prJSON(id: 7572)])
        let filler = assistantLine(String(repeating: "x", count: 1000))
        let lines = [link(7571)] + Array(repeating: filler, count: 1500) + [link(7572)]
        let url = transcript(lines)
        #expect((try! FileManager.default.attributesOfItem(atPath: url.path)[.size] as! UInt64) > 1 << 20)
        let m = monitor(api)
        m.update([target(url)], at: t0)
        #expect(m.takeSnapshot().prs["s"]?.map(\.ref.id) == [7571, 7572])
    }

    @Test func branchMatchUsesCwdRemote() {
        let branch = "bugfix/T6IR-14745"
        let api = fake(open: [prJSON(id: 6874, repo: fe, branch: branch), prJSON(id: 7571, repo: be, branch: branch)])
        let m = monitor(api, remote: "git@bitbucket.org:t6engineering/talo6-dct-frontend.git")
        m.update([target(branch: branch)], at: t0)
        #expect(m.takeSnapshot().prs["s"]?.map(\.ref.id) == [6874])
    }

    @Test func branchPRKeptAfterBranchChange() {
        let branch = "bugfix/T6IR-14745"
        let api = fake(open: [prJSON(id: 6874, repo: fe, branch: branch)])
        let m = monitor(api, remote: "git@bitbucket.org:t6engineering/talo6-dct-frontend.git")
        m.update([target(branch: branch)], at: t0)
        #expect(m.takeSnapshot().prs["s"]?.map(\.ref.id) == [6874])

        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": []])
        api.set("/talo6-dct-frontend/pullrequests/6874?", json: prJSON(id: 6874, repo: fe, branch: branch, state: "MERGED"))
        m.update([target(branch: "master")], at: t0 + 1)
        let snap = m.takeSnapshot()
        #expect(snap.prs["s"]?.map(\.state) == ["MERGED"])
        #expect(snap.events == [.merged])

        let before = api.requests(containing: "pullrequests/6874")
        m.update([target(branch: "master")], at: t0 + 120)
        #expect(api.requests(containing: "pullrequests/6874") == before)
        #expect(m.takeSnapshot().prs["s"]?.map(\.state) == ["MERGED"])
    }

    @Test func closedOnFirstSightHidden() {
        let api = fake()
        api.set("/talo6-dct-backend/pullrequests/7567?", json: prJSON(id: 7567, state: "MERGED"))
        let m = monitor(api)
        let url = transcript([link(7567)])
        m.update([target(url)], at: t0)
        #expect((m.takeSnapshot().prs["s"] ?? []).isEmpty)
        m.update([target(url)], at: t0 + 120)
        #expect(api.requests(containing: "pullrequests/7567") == 1)
    }

    @Test func statusesOnlyWhenNeeded() {
        let api = fake(open: [prJSON(id: 7571)])
        let m = monitor(api)
        let url = transcript([link(7571)])
        m.update([target(url)], at: t0)
        #expect(api.requests(containing: "/statuses") == 1)
        m.update([target(url)], at: t0 + 61)
        #expect(api.requests(containing: "/statuses") == 1)
        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [withCommit(prJSON(id: 7571), "def456")]])
        m.update([target(url)], at: t0 + 122)
        #expect(api.requests(containing: "/statuses") == 2)
    }

    @Test func commentsOnlyWhenCountChanges() {
        let api = fake(open: [prJSON(id: 7571, comments: 2)])
        api.set("/comments", json: ["values": [comment("{tin}", "Tin Tran", "2026-11-04T10:00:00.000000+00:00")]])
        let m = monitor(api)
        let url = transcript([link(7571)])
        m.update([target(url)], at: t0)
        #expect(api.requests(containing: "/comments") == 1)
        #expect(m.takeSnapshot().events.isEmpty)
        m.update([target(url)], at: t0 + 61)
        #expect(api.requests(containing: "/comments") == 1)
        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [prJSON(id: 7571, comments: 3)]])
        api.set("/comments", json: ["values": [comment("{tin}", "Tin Tran", "2026-11-04T11:00:00.000000+00:00")]])
        m.update([target(url)], at: t0 + 122)
        #expect(api.requests(containing: "/comments") == 2)
        #expect(m.takeSnapshot().events == [.comment])
    }

    @Test func failedCycleKeepsStatuses() {
        let api = fake(open: [prJSON(id: 7571, reviewers: 2)])
        let m = monitor(api)
        let url = transcript([link(7571)])
        m.update([target(url)], at: t0)
        let good = m.takeSnapshot()
        api.failAll = .http(403)
        m.update([target(url)], at: t0 + 61)
        let after = m.takeSnapshot()
        #expect(after.prs == good.prs)
        #expect(after.lastSuccess == t0)
        #expect(after.events.isEmpty)
    }

    @Test func cycleWaitsForInterval() throws {
        let api = fake(open: [prJSON(id: 7571), prJSON(id: 7572)])
        let m = monitor(api)
        let url = transcript([link(7571)])
        m.update([target(url)], at: t0)
        let first = api.requests.count
        #expect(first > 0)
        let h = try FileHandle(forWritingTo: url)
        try h.seekToEnd()
        try h.write(contentsOf: Data((link(7572) + "\n").utf8))
        try h.close()
        m.update([target(url)], at: t0 + 5)
        let second = api.requests.count
        #expect(second > first)
        m.update([target(url)], at: t0 + 30)
        #expect(api.requests.count == second)
        m.update([target(url)], at: t0 + 70)
        #expect(api.requests.count > second)
    }

    @Test func takeSnapshotDrainsEvents() {
        let api = fake(open: [prJSON(id: 7571, reviewers: 1)])
        let m = monitor(api)
        let url = transcript([link(7571)])
        m.update([target(url)], at: t0)
        #expect(m.takeSnapshot().events.isEmpty)
        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [prJSON(id: 7571, reviewers: 2)]])
        m.update([target(url)], at: t0 + 61)
        #expect(m.takeSnapshot().events == [.approved])
        #expect(m.takeSnapshot().events.isEmpty)
    }

    @Test func droppedSessionForgotten() {
        let api = fake(open: [prJSON(id: 7571)])
        let m = monitor(api)
        m.update([target(transcript([link(7571)]))], at: t0)
        #expect(m.takeSnapshot().prs["s"] != nil)
        m.update([], at: t0 + 1)
        #expect(m.takeSnapshot().prs["s"] == nil)
    }

    @Test func accountParsedFromKeychainAttributes() {
        let out = "keychain: \"/x/login.keychain-db\"\nattributes:\n    \"acct\"<blob>=\"phong@example.com\"\n    \"svce\"<blob>=\"bitbucket-api-token\"\n"
        #expect(BitbucketClient.account(fromAttributes: out) == "phong@example.com")
        #expect(BitbucketClient.account(fromAttributes: "    \"acct\"<blob>=<NULL>\n") == nil)
    }
}
