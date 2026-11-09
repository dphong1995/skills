import Testing
import Foundation
@testable import ClaudeSessionsKit

private func data(_ values: [[String: Any]]) -> Data { try! JSONSerialization.data(withJSONObject: ["values": values]) }

private func rabbit(_ state: String, _ description: String) -> Rabbit? {
    CodeRabbit.state(fromStatuses: data([rabbitStatus(state: state, description: description)]), commit: "abc123")?.rabbit
}

@Suite struct CodeRabbitTests {
    @Test func stateTable() {
        #expect(rabbit("INPROGRESS", "Review in progress") == .reviewing)
        #expect(rabbit("FAILED", "Review failed") == .failed)
        #expect(rabbit("SUCCESSFUL", "Review failed") == .failed)
        #expect(rabbit("SUCCESSFUL", "Review skipped") == .skipped)
        #expect(rabbit("SUCCESSFUL", "Review rate limited") == .skipped)
        #expect(rabbit("SUCCESSFUL", "No reviewable changes") == .skipped)
        #expect(rabbit("STOPPED", "Stopped") == .skipped)
        #expect(rabbit("SUCCESSFUL", "Approve command performed: Comments resolved. Approval completed") == .approved)
        #expect(rabbit("SUCCESSFUL", "Approve command performed: Approval skipped: already approved") == .approved)
        #expect(rabbit("SUCCESSFUL", "Approve command performed: Approval skipped: durable Cloud recovery is unavailable") == .reviewed(findings: nil))
        #expect(rabbit("SUCCESSFUL", "Review completed") == .reviewed(findings: nil))

        let other: [String: Any] = ["key": "pipelines", "name": "Pipeline", "state": "SUCCESSFUL", "commit": ["hash": "abc123"]]
        #expect(CodeRabbit.state(fromStatuses: data([other]), commit: "abc123") == nil)
        let old = rabbitStatus(description: "Review completed", commit: "old999")
        #expect(CodeRabbit.state(fromStatuses: data([old]), commit: "abc123") == nil)
    }

    @Test func newestStatusWins() {
        let older = rabbitStatus(state: "INPROGRESS", description: "Review in progress", created: "2026-11-03T03:00:00.000000+00:00")
        let newer = rabbitStatus(description: "Review completed", created: "2026-11-03T04:00:00.000000+00:00")
        let s = CodeRabbit.state(fromStatuses: data([older, newer]), commit: "abc123")
        #expect(s?.rabbit == .reviewed(findings: nil))
        #expect(s?.since == Date(timeIntervalSince1970: 1_793_678_400))
    }

    @Test func inProgressSinceIsUpdatedOn() {
        let s = rabbitStatus(state: "INPROGRESS", description: "Review in progress", created: "2026-11-03T03:00:00.000000+00:00",
                             updated: "2026-11-03T04:00:00.000000+00:00")
        #expect(CodeRabbit.state(fromStatuses: data([s]), commit: "abc123")?.since == Date(timeIntervalSince1970: 1_793_678_400))
    }

    @Test func createdOnlyIgnoresEditedSummary() {
        let since = Date(timeIntervalSince1970: 1_793_678_400)
        var edited = rabbitComment("**Actionable comments posted: 2**", at: "2026-11-03T03:00:00.000000+00:00")
        edited["updated_on"] = "2026-11-03T04:05:00.000000+00:00"
        #expect(CodeRabbit.findings(fromComments: data([edited]), since: since) == .count(2))
        #expect(CodeRabbit.findings(fromComments: data([edited]), since: since, createdOnly: true) == .stale)
        let fresh = rabbitComment("**Actionable comments posted: 2**", at: "2026-11-03T04:05:00.000000+00:00")
        #expect(CodeRabbit.findings(fromComments: data([fresh]), since: since, createdOnly: true) == .count(2))
    }

    @Test func findingsFromSummary() {
        let since = Date(timeIntervalSince1970: 1_793_678_400) // 2026-11-03T04:00:00Z
        let summary = rabbitComment("**Actionable comments posted: 2**", at: "2026-11-03T04:05:00.000000+00:00")
        let inline = rabbitComment("nit: rename this", at: "2026-11-03T04:06:00.000000+00:00")
        #expect(CodeRabbit.findings(fromComments: data([inline, summary]), since: since) == .count(2))

        let none = rabbitComment("No actionable comments were generated in the recent review.", at: "2026-11-03T04:05:00.000000+00:00")
        #expect(CodeRabbit.findings(fromComments: data([none]), since: since) == .count(0))

        let stale = rabbitComment("**Actionable comments posted: 2**", at: "2026-11-03T03:00:00.000000+00:00")
        #expect(CodeRabbit.findings(fromComments: data([stale]), since: since) == .stale)
        #expect(CodeRabbit.findings(fromComments: data([]), since: since) == .notFound)
        #expect(CodeRabbit.findings(fromComments: data([inline]), since: since) == .notFound)
    }

    @Test func labels() {
        #expect(CodeRabbit.label(.reviewed(findings: 2)) == "2 góp ý")
        #expect(CodeRabbit.label(.reviewed(findings: 0)) == "xong")
        #expect(CodeRabbit.label(.reviewed(findings: nil)) == "xong")
        #expect(CodeRabbit.label(.reviewing) == "đang review")
        #expect(CodeRabbit.label(.approved) == "đã duyệt")
        #expect(CodeRabbit.label(.skipped) == "bỏ qua")
        #expect(CodeRabbit.label(.failed) == "lỗi")
        #expect(CodeRabbit.label(.awaitingApproval) == "chờ duyệt")
        #expect(CodeRabbit.label(.nudged) == "đang nhắc")
        #expect(CodeRabbit.isFinal(.awaitingApproval) && !CodeRabbit.isFinal(.nudged))
        #expect(CodeRabbit.isFinal(.reviewed(findings: nil)) && CodeRabbit.isFinal(.approved) && CodeRabbit.isFinal(.failed))
        #expect(!CodeRabbit.isFinal(.reviewing) && !CodeRabbit.isFinal(.skipped))
    }

    private static let t1 = "2026-11-04T10:00:00.000000+00:00"
    private static let t2 = "2026-11-04T10:01:00.000000+00:00"
    private static let t3 = "2026-11-04T10:02:00.000000+00:00"
    private static let t4 = "2026-11-04T10:03:00.000000+00:00"

    private var answered: [[String: Any]] {
        [rabbitInline(id: 1, "nit", at: Self.t1),
         humanReply(id: 2, parent: 1, "won't fix", at: Self.t2),
         rabbitInline(id: 3, "ok, withdrawn", at: Self.t3, parent: 1)]
    }

    private func command(_ raw: String = "@coderabbitai approve", at: String) -> [String: Any] {
        humanReply(id: 10, parent: nil, raw, at: at, inline: false)
    }

    @Test func factsWithdrawnThreadIsAnswered() {
        let f = CodeRabbit.facts(from: answered)
        #expect(f.allAnswered && f.rabbitUser == "{tin}" && !f.nudgePending)
    }

    @Test func factsUnansweredThread() {
        #expect(!CodeRabbit.facts(from: [rabbitInline(id: 1, "nit", at: Self.t1)]).allAnswered)
        let f = CodeRabbit.facts(from: [rabbitInline(id: 1, "nit", at: Self.t1), humanReply(id: 2, parent: 1, "why?", at: Self.t2)])
        #expect(!f.allAnswered)
    }

    @Test func factsNoFindings() {
        let f = CodeRabbit.facts(from: [rabbitComment("**Actionable comments posted: 0**", at: Self.t1)])
        #expect(f.allAnswered && f.rabbitUser == "{tin}")
    }

    @Test func factsNudgePending() {
        let base = answered + [rabbitComment("summary", at: Self.t1)]
        #expect(CodeRabbit.facts(from: base + [command(at: Self.t4)]).nudgePending)
        #expect(CodeRabbit.facts(from: base + [command("@CodeRabbit  Approve", at: Self.t4)]).nudgePending)
        #expect(!CodeRabbit.facts(from: base + [command(at: Self.t1)]).nudgePending)
        #expect(!CodeRabbit.facts(from: base).nudgePending)
    }

    @Test func nudgeSurvivesInlineReply() {
        let reply = rabbitInline(id: 4, "still fine", at: "2026-11-04T10:04:00.000000+00:00", parent: 1)
        let f = CodeRabbit.facts(from: answered + [rabbitComment("summary", at: Self.t1), command(at: Self.t4), reply])
        #expect(f.nudgePending && f.allAnswered)
    }

    @Test func nudgeEndsOnActionPerformed() {
        let done = rabbitComment("Action performed", at: "2026-11-04T10:05:00.000000+00:00")
        #expect(!CodeRabbit.facts(from: answered + [command(at: Self.t4), done]).nudgePending)
    }

    @Test func factsIgnoresDeleted() {
        var gone = rabbitInline(id: 9, "nit", at: Self.t1)
        gone["deleted"] = true
        #expect(CodeRabbit.facts(from: answered + [gone]).allAnswered)
    }

    @Test func factsCountsThreads() {
        let f = CodeRabbit.facts(from: [
            rabbitInline(id: 1, "a", at: Self.t1),
            rabbitInline(id: 4, "b", at: Self.t1), humanReply(id: 5, parent: 4, "ok", at: Self.t2), rabbitInline(id: 6, "done", at: Self.t3, parent: 4),
            rabbitInline(id: 7, "c", at: Self.t1), humanReply(id: 8, parent: 7, "why", at: Self.t3)])
        #expect(f.threads == RabbitThreads(pending: 2, total: 3) && !f.allAnswered)
    }

    @Test func factsDeletedRootNotCounted() {
        var gone = rabbitInline(id: 9, "nit", at: Self.t1)
        gone["deleted"] = true
        let f = CodeRabbit.facts(from: answered + [gone])
        #expect(f.threads == RabbitThreads(pending: 0, total: 1))
    }

    @Test func labelRemaining() {
        #expect(CodeRabbit.label(.reviewed(findings: 5), threads: RabbitThreads(pending: 2, total: 5)) == "còn 2/5")
        #expect(CodeRabbit.label(.reviewed(findings: 5), threads: RabbitThreads(pending: 0, total: 0)) == "5 góp ý")
        #expect(CodeRabbit.label(.reviewed(findings: 5), threads: nil) == "5 góp ý")
    }

    @Test func hidesAskClaudeTable() {
        let none = RabbitThreads(pending: 0, total: 3), empty = RabbitThreads(pending: 0, total: 0), some = RabbitThreads(pending: 2, total: 5)
        #expect(CodeRabbit.hidesAskClaude(.awaitingApproval, threads: none))
        #expect(CodeRabbit.hidesAskClaude(.nudged, threads: none))
        #expect(CodeRabbit.hidesAskClaude(.approved, threads: none))
        #expect(CodeRabbit.hidesAskClaude(.reviewed(findings: 0), threads: empty))
        #expect(!CodeRabbit.hidesAskClaude(.reviewed(findings: 5), threads: some))
        #expect(!CodeRabbit.hidesAskClaude(.reviewing, threads: none))
        #expect(!CodeRabbit.hidesAskClaude(.awaitingApproval, threads: nil))
        #expect(!CodeRabbit.hidesAskClaude(.failed, threads: none))
        #expect(!CodeRabbit.hidesAskClaude(.skipped, threads: none))
        #expect(!CodeRabbit.hidesAskClaude(nil, threads: none))
    }

    @Test func resolveOrder() {
        let known = RabbitFacts(rabbitUser: "{tin}", threads: RabbitThreads(pending: 1, total: 1), nudgePending: false)
        var answeredFacts = known; answeredFacts.threads = RabbitThreads(pending: 0, total: 1)
        var nudgedFacts = answeredFacts; nudgedFacts.nudgePending = true
        let tin: Set<String> = ["{tin}"]
        #expect(CodeRabbit.resolve(.reviewing, approvers: tin, facts: known) == .approved)
        #expect(CodeRabbit.resolve(.reviewing, approvers: [], facts: known) == .reviewing)
        #expect(CodeRabbit.resolve(.failed, approvers: tin, facts: known) == .approved)
        #expect(CodeRabbit.resolve(.skipped, approvers: tin, facts: known) == .approved)
        #expect(CodeRabbit.resolve(.failed, approvers: [], facts: known) == .failed)
        #expect(CodeRabbit.resolve(.skipped, approvers: [], facts: answeredFacts) == .skipped)
        let unknown = RabbitFacts(rabbitUser: nil, threads: RabbitThreads(pending: 0, total: 1), nudgePending: false)
        #expect(CodeRabbit.resolve(.reviewed(findings: 2), approvers: [], facts: unknown) == .reviewed(findings: 2))
        #expect(CodeRabbit.resolve(.approved, approvers: [], facts: nil) == .approved)
        #expect(CodeRabbit.resolve(.reviewed(findings: 2), approvers: [], facts: answeredFacts) == .awaitingApproval)
        #expect(CodeRabbit.resolve(.reviewed(findings: 2), approvers: [], facts: nudgedFacts) == .nudged)
        #expect(CodeRabbit.resolve(.reviewed(findings: 2), approvers: [], facts: known) == .reviewed(findings: 2))
        #expect(CodeRabbit.resolve(.approved, approvers: [], facts: known) == .reviewed(findings: nil))
        #expect(CodeRabbit.resolve(nil, approvers: tin, facts: known) == .approved)
        #expect(CodeRabbit.resolve(nil, approvers: [], facts: known) == nil)
    }

    private static let since = "2026-11-03T04:00:00.000000+00:00"
    private static let now = Date(timeIntervalSince1970: 1_793_682_000)

    private func inputs(_ status: [String: Any], comments: [[String: Any]]? = nil, summary: [String: Any]? = nil) -> RabbitInputs {
        RabbitInputs(commit: "abc123", statuses: [status], comments: comments, summary: summary, approvers: [], me: nil)
    }

    @Test func deriveReviewingWithFreshSummaryIsReviewed() {
        let status = rabbitStatus(state: "INPROGRESS", description: "Review in progress", created: Self.since)
        let summary = rabbitComment("**Actionable comments posted: 2**", at: "2026-11-03T04:01:00.000000+00:00")
        let v = CodeRabbit.derive(inputs(status, summary: summary), now: Self.now)
        #expect(v.rabbit == .reviewed(findings: 2))
        #expect(v.status == .reviewed(findings: 2))
    }

    @Test func deriveIgnoresSummaryBeforeSince() {
        let status = rabbitStatus(state: "INPROGRESS", description: "Review in progress", created: Self.since)
        let summary = rabbitComment("**Actionable comments posted: 2**", at: "2026-11-03T03:59:00.000000+00:00")
        let v = CodeRabbit.derive(inputs(status, summary: summary), now: Self.now)
        #expect(v.rabbit == .reviewing)
    }

    @Test func deriveUsesCachedSummaryWhenCommentsLackIt() {
        let status = rabbitStatus(description: "Review completed", created: Self.since)
        let summary = rabbitComment("**Actionable comments posted: 8**", at: "2026-11-03T04:01:00.000000+00:00")
        let v = CodeRabbit.derive(inputs(status, comments: [], summary: summary), now: Self.now)
        #expect(v.rabbit == .reviewed(findings: 8))
    }

    @Test func deriveWithoutCommentsHasNoThreads() {
        let status = rabbitStatus(description: "Review completed", created: Self.since)
        let v = CodeRabbit.derive(inputs(status), now: Self.now)
        #expect(v.threads == nil)
        #expect(v.rabbit == .reviewed(findings: nil))
    }

    @Test func deriveIsPure() {
        let status = rabbitStatus(description: "Review completed", created: Self.since)
        let summary = rabbitComment("**Actionable comments posted: 1**", at: "2026-11-03T04:01:00.000000+00:00")
        let i = inputs(status, comments: [summary])
        #expect(CodeRabbit.derive(i, now: Self.now) == CodeRabbit.derive(i, now: Self.now))
    }
}
