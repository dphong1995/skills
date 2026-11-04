import Testing
import Foundation
@testable import ClaudeSessionsKit

private let be = "t6engineering/talo6-dct-backend"
private let fe = "t6engineering/talo6-dct-frontend"
private let t0 = Date(timeIntervalSince1970: 1_800_000_000)

private func prRef(_ id: Int, repo: String = be) -> PRRef {
    let parts = repo.split(separator: "/")
    return PRRef(workspace: String(parts[0]), repo: String(parts[1]), id: id)
}

private func target(_ linkRefs: [PRRef] = [], branch: String? = nil, cwd: String = "/x") -> PRTarget {
    PRTarget(sessionId: "s", linkRefs: linkRefs, cwd: cwd, branch: branch)
}

private func monitor(_ api: FakeBitbucket, remote: String? = nil) -> PRMonitor {
    PRMonitor(api: api, queue: nil, remote: { _ in remote })
}

private func withCommit(_ pr: [String: Any], _ hash: String) -> [String: Any] {
    var o = pr
    var source = o["source"] as! [String: Any]
    source["commit"] = ["hash": hash]
    o["source"] = source
    return o
}

@Suite struct PRMonitorTests {
    @Test func linkRefsShownAfterFirstCycle() {
        let api = fake(open: [prJSON(id: 7571, reviewers: 3)])
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
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

    @Test func newLinkRefTriggersCycle() {
        let api = fake(open: [prJSON(id: 7571), prJSON(id: 6874, repo: fe)])
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
        #expect(api.requests(containing: "/pullrequests/6874") == 0)
        m.update([target([prRef(7571), prRef(6874, repo: fe)])], at: t0 + 5)
        #expect(api.requests(containing: "/pullrequests/6874") > 0)
        #expect(m.takeSnapshot().prs["s"]?.map(\.ref.id) == [7571, 6874])
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

    @Test func foreignWorkspaceRefAskedOnce() {
        let api = fake(open: [prJSON(id: 7571)])
        let m = monitor(api)
        let refs = [prRef(7571), prRef(1, repo: "acme/app")]
        m.update([target(refs)], at: t0)
        m.update([target(refs)], at: t0 + 61)
        #expect(api.requests(containing: "repositories/acme/app/pullrequests/1") == 1)
        #expect(m.takeSnapshot().prs["s"]?.map(\.ref.id) == [7571])
    }

    @Test func closedOnFirstSightHidden() {
        let api = fake()
        api.set("/talo6-dct-backend/pullrequests/7567?", json: prJSON(id: 7567, state: "MERGED"))
        let m = monitor(api)
        let refs = [prRef(7567)]
        m.update([target(refs)], at: t0)
        #expect((m.takeSnapshot().prs["s"] ?? []).isEmpty)
        m.update([target(refs)], at: t0 + 120)
        #expect(api.requests(containing: "pullrequests/7567") == 1)
    }

    @Test func statusesOnlyWhenNeeded() {
        let api = fake(open: [prJSON(id: 7571)])
        let m = monitor(api)
        let refs = [prRef(7571)]
        m.update([target(refs)], at: t0)
        #expect(api.requests(containing: "/statuses") == 1)
        m.update([target(refs)], at: t0 + 61)
        #expect(api.requests(containing: "/statuses") == 1)
        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [withCommit(prJSON(id: 7571), "def456")]])
        m.update([target(refs)], at: t0 + 122)
        #expect(api.requests(containing: "/statuses") == 2)
    }

    @Test func commentsOnlyWhenCountChanges() {
        let api = fake(open: [prJSON(id: 7571, comments: 2)])
        api.set("/comments", json: ["values": [comment("{tin}", "Tin Tran", "2026-11-04T10:00:00.000000+00:00")]])
        let m = monitor(api)
        let refs = [prRef(7571)]
        m.update([target(refs)], at: t0)
        #expect(api.requests(containing: "/comments") == 1)
        #expect(m.takeSnapshot().events.isEmpty)
        m.update([target(refs)], at: t0 + 61)
        #expect(api.requests(containing: "/comments") == 1)
        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [prJSON(id: 7571, comments: 3)]])
        api.set("/comments", json: ["values": [comment("{tin}", "Tin Tran", "2026-11-04T11:00:00.000000+00:00")]])
        m.update([target(refs)], at: t0 + 122)
        #expect(api.requests(containing: "/comments") == 2)
        #expect(m.takeSnapshot().events == [.comment])
    }

    @Test func eventRefsMatchEvents() {
        let api = fake(open: [prJSON(id: 7571, comments: 2)])
        api.set("/comments", json: ["values": [comment("{tin}", "Tin Tran", "2026-11-04T10:00:00.000000+00:00")]])
        let m = monitor(api)
        let refs = [prRef(7571)]
        m.update([target(refs)], at: t0)
        #expect(m.takeSnapshot().eventRefs.isEmpty)
        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [prJSON(id: 7571, comments: 3)]])
        api.set("/comments", json: ["values": [comment("{tin}", "Tin Tran", "2026-11-04T11:00:00.000000+00:00")]])
        m.update([target(refs)], at: t0 + 61)
        #expect(m.takeSnapshot().eventRefs == [PRRef(workspace: "t6engineering", repo: "talo6-dct-backend", id: 7571)])
        #expect(m.takeSnapshot().eventRefs.isEmpty)
        m.update([target(refs)], at: t0 + 122)
        #expect(m.takeSnapshot().eventRefs.isEmpty)
    }

    @Test func failedCycleKeepsStatuses() {
        let api = fake(open: [prJSON(id: 7571, reviewers: 2)])
        let m = monitor(api)
        let refs = [prRef(7571)]
        m.update([target(refs)], at: t0)
        let good = m.takeSnapshot()
        api.failAll = .http(403)
        m.update([target(refs)], at: t0 + 61)
        let after = m.takeSnapshot()
        #expect(after.prs == good.prs)
        #expect(after.lastSuccess == t0)
        #expect(after.events.isEmpty)
    }

    @Test func cycleWaitsForInterval() {
        let api = fake(open: [prJSON(id: 7571), prJSON(id: 7572)])
        let m = monitor(api)
        let refs = [prRef(7571)]
        m.update([target(refs)], at: t0)
        let first = api.requests.count
        #expect(first > 0)
        m.update([target([prRef(7571), prRef(7572)])], at: t0 + 5)
        let second = api.requests.count
        #expect(second > first)
        m.update([target(refs)], at: t0 + 30)
        #expect(api.requests.count == second)
        m.update([target(refs)], at: t0 + 70)
        #expect(api.requests.count > second)
    }

    @Test func takeSnapshotDrainsEvents() {
        let api = fake(open: [prJSON(id: 7571, reviewers: 1)])
        let m = monitor(api)
        let refs = [prRef(7571)]
        m.update([target(refs)], at: t0)
        #expect(m.takeSnapshot().events.isEmpty)
        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [prJSON(id: 7571, reviewers: 2)]])
        m.update([target(refs)], at: t0 + 61)
        #expect(m.takeSnapshot().events == [.approved])
        #expect(m.takeSnapshot().events.isEmpty)
    }

    @Test func droppedSessionForgotten() {
        let api = fake(open: [prJSON(id: 7571)])
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
        #expect(m.takeSnapshot().prs["s"] != nil)
        m.update([], at: t0 + 1)
        m.update([], at: t0 + 32)
        #expect(m.takeSnapshot().prs["s"] == nil)
    }

    @Test func briefAbsenceKeepsState() {
        let api = fake(open: [prJSON(id: 7571)])
        let m = monitor(api)
        let refs = [prRef(7571)]
        m.update([target(refs)], at: t0)
        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": []])
        api.set("/talo6-dct-backend/pullrequests/7571?", json: prJSON(id: 7571, state: "MERGED"))
        m.update([], at: t0 + 1)
        m.update([target(refs)], at: t0 + 2)
        m.update([target(refs)], at: t0 + 61)
        #expect(m.takeSnapshot().prs["s"]?.map(\.state) == ["MERGED"])
    }

    @Test func foreignWorkspaceOpenListFailureSkipsOnlyThatWorkspace() {
        let api = fake(open: [prJSON(id: 7571)])
        let m = monitor(api)
        let refs = [prRef(7571), prRef(1, repo: "acme/app")]
        m.update([target(refs)], at: t0)
        let snap = m.takeSnapshot()
        #expect(snap.prs["s"]?.map(\.ref.id) == [7571])
        #expect(snap.lastSuccess == t0)
    }

    @Test func everyOpenListForbiddenFailsCycle() {
        let api = fake(open: [prJSON(id: 7571)])
        let m = monitor(api)
        let refs = [prRef(7571), prRef(1, repo: "acme/app")]
        m.update([target(refs)], at: t0)
        let good = m.takeSnapshot()
        api.routes["/2.0/workspaces/t6engineering/pullrequests/"] = .failure(.http(403))
        m.update([target(refs)], at: t0 + 61)
        let after = m.takeSnapshot()
        #expect(after.prs == good.prs)
        #expect(after.lastSuccess == t0)
    }

    @Test func failedStatusesAfterPushKeepsOldStatusThenRetries() {
        let api = fake(open: [prJSON(id: 7571)])
        let m = monitor(api)
        let refs = [prRef(7571)]
        m.update([target(refs)], at: t0)
        let good = m.takeSnapshot()
        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [withCommit(prJSON(id: 7571), "def456")]])
        api.routes["/statuses"] = .failure(.http(500))
        m.update([target(refs)], at: t0 + 61)
        #expect(m.takeSnapshot().prs == good.prs)
        api.set("/statuses", json: ["values": [["state": "FAILED"]]])
        let before = api.requests(containing: "/statuses")
        m.update([target(refs)], at: t0 + 122)
        #expect(api.requests(containing: "/statuses") == before + 1)
        let p = m.takeSnapshot().prs["s"]?.first
        #expect(p?.pipeline == .failed && p?.commit == "def456")
    }

    @Test func nilRemoteRetriedAfterFiveMinutes() {
        final class Counter { var n = 0 }
        let c = Counter()
        let m = PRMonitor(api: fake(), queue: nil, remote: { _ in c.n += 1; return nil })
        m.update([target()], at: t0)
        m.update([target()], at: t0 + 100)
        #expect(c.n == 1)
        m.update([target()], at: t0 + 301)
        #expect(c.n == 2)
    }

    @Test func accountParsedFromKeychainAttributes() {
        let out = "keychain: \"/x/login.keychain-db\"\nattributes:\n    \"acct\"<blob>=\"phong@example.com\"\n    \"svce\"<blob>=\"bitbucket-api-token\"\n"
        #expect(BitbucketClient.account(fromAttributes: out) == "phong@example.com")
        #expect(BitbucketClient.account(fromAttributes: "    \"acct\"<blob>=<NULL>\n") == nil)
    }
}
