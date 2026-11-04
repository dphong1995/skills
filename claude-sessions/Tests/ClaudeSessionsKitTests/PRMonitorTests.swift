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
        // A finished CodeRabbit review: with none (nil) the statuses are asked again every cycle.
        api.set("/statuses", json: ["values": [["state": "SUCCESSFUL"], rabbitStatus(description: "Review completed")]])
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

    private static let started = "2026-11-03T03:33:15.111541+00:00"
    private static let later = "2026-11-03T03:40:00.000000+00:00"
    private static let pipelinePass: [String: Any] = ["state": "SUCCESSFUL", "commit": ["hash": "abc123"]]

    private func setStatuses(_ api: FakeBitbucket, _ rabbit: [String: Any]?) {
        api.set("/statuses", json: ["values": [Self.pipelinePass] + (rabbit.map { [$0] } ?? [])])
    }

    private func rabbitOf(_ m: PRMonitor) -> Rabbit? { m.takeSnapshot().prs["s"]?.first?.rabbit }

    @Test func rabbitAssigned() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        setStatuses(api, rabbitStatus(description: "Review completed", created: Self.started))
        api.set("/comments", json: ["values": [rabbitComment("**Actionable comments posted: 2**", at: Self.later)]])
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
        let p = m.takeSnapshot().prs["s"]?.first
        #expect(p?.rabbit == .reviewed(findings: 2))
        #expect(p?.pipeline == .passed)
        #expect(p?.lastOther == nil)
    }

    @Test func statusesRefetchedWhileRabbitPendingOrCommentsChange() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        setStatuses(api, rabbitStatus(state: "INPROGRESS", description: "Review in progress", created: Self.started))
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
        var n = api.requests(containing: "/statuses")
        m.update([target([prRef(7571)])], at: t0 + 61)
        #expect(api.requests(containing: "/statuses") == n + 1)

        setStatuses(api, rabbitStatus(description: "Review completed", created: Self.started))
        m.update([target([prRef(7571)])], at: t0 + 122)
        n = api.requests(containing: "/statuses")
        #expect(n == 3)
        m.update([target([prRef(7571)])], at: t0 + 183)
        #expect(api.requests(containing: "/statuses") == n)

        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [prJSON(id: 7571, comments: 2)]])
        m.update([target([prRef(7571)])], at: t0 + 244)
        #expect(api.requests(containing: "/statuses") == n + 1)
    }

    @Test func commentsRefetchedWhenRabbitFinishes() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        setStatuses(api, rabbitStatus(state: "INPROGRESS", description: "Review in progress", created: Self.started))
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
        #expect(rabbitOf(m) == .reviewing)
        let n = api.requests(containing: "/comments")

        setStatuses(api, rabbitStatus(description: "Review completed", created: Self.started))
        api.set("/comments", json: ["values": [rabbitComment("**Actionable comments posted: 2**", at: Self.later)]])
        m.update([target([prRef(7571)])], at: t0 + 61)
        #expect(api.requests(containing: "/comments") == n + 1)
        #expect(rabbitOf(m) == .reviewed(findings: 2))
    }

    private static let start = "2026-11-04T10:00:00.000000+00:00"
    private static let summaryAt = "2026-11-04T10:12:00.000000+00:00"

    private func inProgress(updated: String = start) -> [String: Any] {
        rabbitStatus(state: "INPROGRESS", description: "Review in progress", created: Self.start, updated: updated)
    }

    private func bump(_ api: FakeBitbucket, comments: Int) {
        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [prJSON(id: 7571, comments: comments)]])
    }

    /// INPROGRESS status, then a summary posted after its start and one more comment.
    private func overriddenMonitor(_ api: FakeBitbucket, summary: String = "**Actionable comments posted: 1**") -> PRMonitor {
        setStatuses(api, inProgress())
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
        _ = m.takeSnapshot()
        api.set("/comments", json: ["values": [rabbitComment(summary, at: Self.summaryAt)]])
        bump(api, comments: 2)
        m.update([target([prRef(7571)])], at: t0 + 61)
        return m
    }

    @Test func summaryEndsInProgressReview() {
        let m = overriddenMonitor(fake(open: [prJSON(id: 7571, comments: 1)]))
        let snap = m.takeSnapshot()
        #expect(rabbitOf(m) == .reviewed(findings: 1))
        #expect(CodeRabbit.label(.reviewed(findings: 1)) == "🐰 1 góp ý")
        #expect(snap.events == [.rabbitNeedsYou])
    }

    @Test func overrideKeptWhileStatusStaysInProgress() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        let m = overriddenMonitor(api)
        _ = m.takeSnapshot()
        bump(api, comments: 3)
        m.update([target([prRef(7571)])], at: t0 + 122)
        #expect(rabbitOf(m) == .reviewed(findings: 1))
        #expect(m.takeSnapshot().events.isEmpty)
    }

    @Test func overrideKeptWhenOnlyStatusesAreRefetched() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        api.set("/statuses", json: ["values": [inProgress()]]) // no pipeline: statuses stay polled inside the new-commit window
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
        api.set("/comments", json: ["values": [rabbitComment("**Actionable comments posted: 1**", at: Self.summaryAt)]])
        bump(api, comments: 2)
        m.update([target([prRef(7571)])], at: t0 + 61)
        _ = m.takeSnapshot()
        let statuses = api.requests(containing: "/statuses")
        let comments = api.requests(containing: "/comments")
        m.update([target([prRef(7571)])], at: t0 + 122)
        #expect(api.requests(containing: "/statuses") == statuses + 1)
        #expect(api.requests(containing: "/comments") == comments)
        #expect(rabbitOf(m) == .reviewed(findings: 1))
        #expect(m.takeSnapshot().events.isEmpty)
    }

    @Test func summaryFoundOnOwedFetchAtFirstSightDoesNotChime() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        setStatuses(api, inProgress())
        api.routes["/comments"] = .failure(.transport)
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
        _ = m.takeSnapshot()
        api.set("/comments", json: ["values": [rabbitComment("**Actionable comments posted: 1**", at: Self.summaryAt)]])
        m.update([target([prRef(7571)])], at: t0 + 61)
        let snap = m.takeSnapshot()
        #expect(snap.prs["s"]?.first?.rabbit == .reviewed(findings: 1))
        #expect(snap.events.isEmpty)
    }

    @Test func overrideSurvivesStatusFlippingToSuccessful() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        let m = overriddenMonitor(api)
        _ = m.takeSnapshot()
        setStatuses(api, rabbitStatus(description: "Review completed", created: Self.start, updated: "2026-11-04T10:30:00.000000+00:00"))
        bump(api, comments: 3)
        m.update([target([prRef(7571)])], at: t0 + 122)
        #expect(rabbitOf(m) == .reviewed(findings: 1))
        #expect(m.takeSnapshot().events.isEmpty)
    }

    @Test func summaryBeforeInProgressStartDoesNotEndReview() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        setStatuses(api, inProgress())
        api.set("/comments", json: ["values": [rabbitComment("**Actionable comments posted: 1**", at: "2026-11-04T09:00:00.000000+00:00")]])
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
        _ = m.takeSnapshot()
        bump(api, comments: 2)
        m.update([target([prRef(7571)])], at: t0 + 61)
        #expect(rabbitOf(m) == .reviewing)
        #expect(m.takeSnapshot().events.isEmpty)
    }

    @Test func newerInProgressStartRestartsReview() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        let m = overriddenMonitor(api)
        _ = m.takeSnapshot()
        setStatuses(api, inProgress(updated: "2026-11-04T10:40:00.000000+00:00"))
        bump(api, comments: 3)
        m.update([target([prRef(7571)])], at: t0 + 122)
        #expect(rabbitOf(m) == .reviewing)
    }

    @Test func noActionableSummaryEndsInProgressReview() {
        let m = overriddenMonitor(fake(open: [prJSON(id: 7571, comments: 1)]), summary: "No actionable comments were generated.")
        let snap = m.takeSnapshot()
        #expect(rabbitOf(m) == .reviewed(findings: 0))
        #expect(snap.events == [.rabbitDone])
    }

    @Test func firstSightOfReviewedPRWithInProgressStatusDoesNotChime() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        setStatuses(api, inProgress())
        api.set("/comments", json: ["values": [rabbitComment("**Actionable comments posted: 1**", at: Self.summaryAt)]])
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
        #expect(rabbitOf(m) == .reviewed(findings: 1))
        #expect(m.takeSnapshot().events.isEmpty)
    }

    @Test func rabbitChimesOncePerReview() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        setStatuses(api, rabbitStatus(description: "Review completed", created: Self.started))
        api.set("/comments", json: ["values": [rabbitComment("**Actionable comments posted: 2**", at: Self.later)]])
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
        #expect(m.takeSnapshot().events.isEmpty)
        m.update([target([prRef(7571)])], at: t0 + 61)
        #expect(m.takeSnapshot().events.isEmpty)

        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [withCommit(prJSON(id: 7571, comments: 1), "def456")]])
        api.set("/statuses", json: ["values": [rabbitStatus(description: "Review completed", commit: "def456", created: "2026-11-03T05:00:00.000000+00:00")]])
        api.set("/comments", json: ["values": [rabbitComment("No actionable comments were generated.", at: "2026-11-03T05:10:00.000000+00:00")]])
        m.update([target([prRef(7571)])], at: t0 + 122)
        let snap = m.takeSnapshot()
        #expect(snap.events == [.rabbitDone])
        #expect(snap.eventRefs == [prRef(7571)])
        m.update([target([prRef(7571)])], at: t0 + 183)
        #expect(m.takeSnapshot().events.isEmpty)
    }

    @Test func rabbitChimesWhenReviewEndsBetweenPolls() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        setStatuses(api, rabbitStatus(description: "Review completed", created: Self.started))
        api.set("/comments", json: ["values": [rabbitComment("No actionable comments were generated.", at: Self.later)]])
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
        #expect(rabbitOf(m) == .reviewed(findings: 0))

        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [withCommit(prJSON(id: 7571, comments: 1), "def456")]])
        api.set("/statuses", json: ["values": [rabbitStatus(description: "Review completed", commit: "def456", created: "2026-11-03T05:00:00.000000+00:00")]])
        api.set("/comments", json: ["values": [rabbitComment("**Actionable comments posted: 1**", at: "2026-11-03T05:10:00.000000+00:00")]])
        m.update([target([prRef(7571)])], at: t0 + 61)
        #expect(m.takeSnapshot().events == [.rabbitNeedsYou])
    }

    @Test func findingsKeptWhenSummaryMissingFromCommentsPage() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        setStatuses(api, rabbitStatus(description: "Review completed", created: Self.started))
        api.set("/comments", json: ["values": [rabbitComment("**Actionable comments posted: 8**", at: Self.later)]])
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
        #expect(rabbitOf(m) == .reviewed(findings: 8))

        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [prJSON(id: 7571, comments: 2)]])
        api.set("/comments", json: ["values": [rabbitComment("nit: rename this", at: Self.later)]])
        m.update([target([prRef(7571)])], at: t0 + 61)
        let snap = m.takeSnapshot()
        #expect(snap.prs["s"]?.first?.rabbit == .reviewed(findings: 8))
        #expect(snap.events.isEmpty)
    }

    @Test func firstFailedStatusesFetchIsNotAChimeLater() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        api.routes["/statuses"] = .failure(.transport)
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
        setStatuses(api, rabbitStatus(description: "Review completed", created: Self.started))
        api.set("/comments", json: ["values": [rabbitComment("**Actionable comments posted: 2**", at: Self.later)]])
        m.update([target([prRef(7571)])], at: t0 + 61)
        let snap = m.takeSnapshot()
        #expect(snap.prs["s"]?.first?.rabbit == .reviewed(findings: 2))
        #expect(snap.events.isEmpty)

        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [withCommit(prJSON(id: 7571, comments: 1), "def456")]])
        api.set("/statuses", json: ["values": [rabbitStatus(description: "Review completed", commit: "def456", created: "2026-11-03T05:00:00.000000+00:00")]])
        api.set("/comments", json: ["values": [rabbitComment("No actionable comments were generated.", at: "2026-11-03T05:10:00.000000+00:00")]])
        m.update([target([prRef(7571)])], at: t0 + 122)
        #expect(m.takeSnapshot().events == [.rabbitDone])
    }

    @Test func oldReviewAfterTwoTransientFailuresDoesNotChime() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        api.routes["/statuses"] = .failure(.transport)
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
        setStatuses(api, rabbitStatus(description: "Review completed", created: Self.started))
        api.routes["/comments"] = .failure(.transport)
        m.update([target([prRef(7571)])], at: t0 + 61)
        api.set("/comments", json: ["values": [rabbitComment("**Actionable comments posted: 2**", at: Self.later)]])
        m.update([target([prRef(7571)])], at: t0 + 122)
        let snap = m.takeSnapshot()
        #expect(snap.prs["s"]?.first?.rabbit == .reviewed(findings: 2))
        #expect(snap.events.isEmpty)
    }

    @Test func reviewedReviewIsSlowPolledUntilItFlipsToApproved() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        setStatuses(api, rabbitStatus(description: "Review completed", created: Self.started))
        api.set("/comments", json: ["values": [rabbitComment("**Actionable comments posted: 1**", at: Self.later)]])
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
        #expect(rabbitOf(m) == .reviewed(findings: 1))
        _ = m.takeSnapshot()
        let n = api.requests(containing: "/statuses")
        m.update([target([prRef(7571)])], at: t0 + 299)
        #expect(api.requests(containing: "/statuses") == n)
        setStatuses(api, rabbitStatus(description: "Review approved", created: Self.started))
        m.update([target([prRef(7571)])], at: t0 + 700)
        #expect(api.requests(containing: "/statuses") == n + 1)
        let snap = m.takeSnapshot()
        #expect(snap.prs["s"]?.first?.rabbit == .approved)
        #expect(CodeRabbit.label(.approved) == "🐰 đã duyệt")
        #expect(snap.events == [.rabbitDone])
    }

    @Test func settledRabbitKindsAreNotSlowPolled() {
        let cases: [(String, String)] = [("SUCCESSFUL", "Review approved"), ("SUCCESSFUL", "Review skipped"), ("FAILED", "Review failed")]
        for (state, description) in cases {
            let api = fake(open: [prJSON(id: 7571, comments: 1)])
            setStatuses(api, rabbitStatus(state: state, description: description, created: Self.started))
            let m = monitor(api)
            m.update([target([prRef(7571)])], at: t0)
            let n = api.requests(containing: "/statuses")
            m.update([target([prRef(7571)])], at: t0 + 700)
            m.update([target([prRef(7571)])], at: t0 + 1400)
            #expect(api.requests(containing: "/statuses") == n, "\(description)")
        }
    }

    @Test func slowPollAfterOverrideWithStatusStillInProgressChangesNothing() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        let m = overriddenMonitor(api)
        _ = m.takeSnapshot()
        let statuses = api.requests(containing: "/statuses")
        let comments = api.requests(containing: "/comments")
        m.update([target([prRef(7571)])], at: t0 + 700)
        #expect(api.requests(containing: "/statuses") == statuses + 1)
        #expect(api.requests(containing: "/comments") == comments)
        #expect(rabbitOf(m) == .reviewed(findings: 1))
        #expect(m.takeSnapshot().events.isEmpty)
    }

    @Test func closedPRIsNeverSlowPolled() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        setStatuses(api, rabbitStatus(description: "Review completed", created: Self.started))
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": []])
        api.set("/talo6-dct-backend/pullrequests/7571?", json: prJSON(id: 7571, state: "MERGED"))
        m.update([target([prRef(7571)])], at: t0 + 61)
        let n = api.requests(containing: "/statuses")
        m.update([target([prRef(7571)])], at: t0 + 700)
        #expect(api.requests(containing: "/statuses") == n)
    }

    @Test func statusAppearingAfterWindowIsPickedUpByTheSlowPoll() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        api.set("/statuses", json: ["values": []])
        let m = monitor(api)
        let refs = [prRef(7571)]
        m.update([target(refs)], at: t0)
        m.update([target(refs)], at: t0 + 599)
        let n = api.requests(containing: "/statuses")
        api.set("/statuses", json: ["values": [Self.pipelinePass]])
        m.update([target(refs)], at: t0 + 660)
        m.update([target(refs)], at: t0 + 800)
        #expect(api.requests(containing: "/statuses") == n)
        m.update([target(refs)], at: t0 + 900)
        #expect(api.requests(containing: "/statuses") == n + 1)
        #expect(m.takeSnapshot().prs["s"]?.first?.pipeline == .passed)
    }

    @Test func statusesFailingThroughTheWindowAreFetchedAfterIt() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        api.routes["/statuses"] = .failure(.transport)
        let m = monitor(api)
        let refs = [prRef(7571)]
        m.update([target(refs)], at: t0)
        m.update([target(refs)], at: t0 + 599)
        setStatuses(api, nil)
        let n = api.requests(containing: "/statuses")
        m.update([target(refs)], at: t0 + 660)
        #expect(api.requests(containing: "/statuses") == n + 1)
        #expect(m.takeSnapshot().prs["s"]?.first?.pipeline == .passed)
        m.update([target(refs)], at: t0 + 721)
        #expect(api.requests(containing: "/statuses") == n + 1)
    }

    @Test func statusesNotPolledForeverWithoutAnyStatuses() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        api.set("/statuses", json: ["values": []])
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
        m.update([target([prRef(7571)])], at: t0 + 61)
        let n = api.requests(containing: "/statuses")
        m.update([target([prRef(7571)])], at: t0 + 660 + 61)
        #expect(api.requests(containing: "/statuses") == n + 1)
        m.update([target([prRef(7571)])], at: t0 + 660 + 122)
        m.update([target([prRef(7571)])], at: t0 + 660 + 183)
        #expect(api.requests(containing: "/statuses") == n + 1)

        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [withCommit(prJSON(id: 7571, comments: 1), "def456")]])
        m.update([target([prRef(7571)])], at: t0 + 660 + 244)
        #expect(api.requests(containing: "/statuses") == n + 2)
    }

    @Test func newFinalKindOnSameCommitChimesAfterCommentsOwed() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        setStatuses(api, rabbitStatus(state: "FAILED", description: "Review failed", created: Self.started))
        api.routes["/comments"] = .failure(.transport)
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
        #expect(rabbitOf(m) == .failed)
        _ = m.takeSnapshot()

        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [prJSON(id: 7571, comments: 2)]])
        setStatuses(api, rabbitStatus(description: "Review completed", created: Self.later))
        api.set("/comments", json: ["values": [rabbitComment("**Actionable comments posted: 2**", at: Self.later)]])
        m.update([target([prRef(7571)])], at: t0 + 61)
        #expect(m.takeSnapshot().events == [.rabbitNeedsYou])
    }

    @Test func newReviewOnSameCommitAfterUnsavedReviewingCycleStillChimes() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        setStatuses(api, rabbitStatus(description: "Review completed", created: Self.started))
        api.routes["/comments"] = .failure(.transport)
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
        _ = m.takeSnapshot()

        // A re-review starts and finishes between two saved cycles: the in-progress cycle fails on comments and is not saved.
        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [prJSON(id: 7571, comments: 2)]])
        setStatuses(api, rabbitStatus(state: "INPROGRESS", description: "Review in progress", created: Self.later))
        m.update([target([prRef(7571)])], at: t0 + 61)
        setStatuses(api, rabbitStatus(description: "Review completed", created: "2026-11-03T04:00:00.000000+00:00"))
        api.set("/comments", json: ["values": [rabbitComment("**Actionable comments posted: 2**", at: "2026-11-03T04:05:00.000000+00:00")]])
        m.update([target([prRef(7571)])], at: t0 + 122)
        #expect(m.takeSnapshot().events == [.rabbitNeedsYou])
    }

    @Test func failedFirstCommentsFetchIsRetried() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        setStatuses(api, rabbitStatus(description: "Review completed", created: Self.started))
        api.routes["/comments"] = .failure(.transport)
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
        #expect(rabbitOf(m) == .reviewed(findings: nil))
        let n = api.requests(containing: "/comments")

        api.set("/comments", json: ["values": [rabbitComment("**Actionable comments posted: 2**", at: Self.later)]])
        m.update([target([prRef(7571)])], at: t0 + 61)
        #expect(api.requests(containing: "/comments") == n + 1)
        let snap = m.takeSnapshot()
        #expect(snap.prs["s"]?.first?.rabbit == .reviewed(findings: 2))
        #expect(snap.events.isEmpty)
    }

    @Test func statusesNotPolledForeverWithoutRabbit() {
        let api = fake(open: [prJSON(id: 7571, comments: 1)])
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
        m.update([target([prRef(7571)])], at: t0 + 61)
        #expect(api.requests(containing: "/statuses") == 2)
        m.update([target([prRef(7571)])], at: t0 + 660 + 61)
        #expect(api.requests(containing: "/statuses") == 3)
        m.update([target([prRef(7571)])], at: t0 + 660 + 122)
        m.update([target([prRef(7571)])], at: t0 + 660 + 183)
        #expect(api.requests(containing: "/statuses") == 3)
    }

    @Test func rabbitCommentDoesNotChime() {
        let api = fake(open: [prJSON(id: 7571, comments: 0)])
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
        _ = m.takeSnapshot()
        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [prJSON(id: 7571, comments: 1)]])
        api.set("/comments", json: ["values": [rabbitComment("**Actionable comments posted: 1**", at: Self.later)]])
        m.update([target([prRef(7571)])], at: t0 + 61)
        let snap = m.takeSnapshot()
        #expect(snap.events.isEmpty)
        #expect(snap.prs["s"]?.first?.lastOther == nil)
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

    @Test func connectionOkAfterCycle() {
        let m = monitor(fake())
        m.update([target([prRef(7571)])], at: t0)
        #expect(m.takeSnapshot().connection == .ok)
    }

    @Test func connectionRejectedOn401() {
        let api = fake()
        api.failAll = .http(401)
        let m = monitor(api)
        m.update([target([prRef(7571)])], at: t0)
        #expect(m.takeSnapshot().connection == .rejected)
    }

    @Test func connectionIgnoresPerPRErrors() {
        let api = fake(open: [prJSON(id: 7571, reviewers: 1)])
        api.routes["/2.0/workspaces/t6engineering/pullrequests/9999"] = .failure(.http(404))
        let m = monitor(api)
        m.update([target([prRef(7571), prRef(9999)])], at: t0)
        #expect(m.takeSnapshot().connection == .ok)
    }
}
