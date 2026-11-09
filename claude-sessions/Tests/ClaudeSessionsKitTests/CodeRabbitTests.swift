import Testing
import Foundation
@testable import ClaudeSessionsKit

private func rabbit(_ state: String, _ description: String) -> Rabbit? {
    CodeRabbit.state(fromValues: [rabbitStatus(state: state, description: description)], commit: "abc123")?.rabbit
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
        #expect(rabbit("SUCCESSFUL", "Approve command performed: Approval skipped: durable Cloud recovery is unavailable") == .approvalFailed(reason: "durable Cloud recovery is unavailable"))
        #expect(rabbit("SUCCESSFUL", "Review completed") == .reviewed(findings: nil))

        let other: [String: Any] = ["key": "pipelines", "name": "Pipeline", "state": "SUCCESSFUL", "commit": ["hash": "abc123"]]
        #expect(CodeRabbit.state(fromValues: [other], commit: "abc123") == nil)
        let old = rabbitStatus(description: "Review completed", commit: "old999")
        #expect(CodeRabbit.state(fromValues: [old], commit: "abc123") == nil)
    }

    @Test func newestStatusWins() {
        let older = rabbitStatus(state: "INPROGRESS", description: "Review in progress", created: "2026-11-03T03:00:00.000000+00:00")
        let newer = rabbitStatus(description: "Review completed", created: "2026-11-03T04:00:00.000000+00:00")
        let s = CodeRabbit.state(fromValues: [older, newer], commit: "abc123")
        #expect(s?.rabbit == .reviewed(findings: nil))
        #expect(s?.since == Date(timeIntervalSince1970: 1_793_678_400))
    }

    @Test func inProgressSinceIsUpdatedOn() {
        let s = rabbitStatus(state: "INPROGRESS", description: "Review in progress", created: "2026-11-03T03:00:00.000000+00:00",
                             updated: "2026-11-03T04:00:00.000000+00:00")
        #expect(CodeRabbit.state(fromValues: [s], commit: "abc123")?.since == Date(timeIntervalSince1970: 1_793_678_400))
    }

    @Test func createdOnlyIgnoresEditedSummary() {
        let since = Date(timeIntervalSince1970: 1_793_678_400)
        var edited = rabbitComment("**Actionable comments posted: 2**", at: "2026-11-03T03:00:00.000000+00:00")
        edited["updated_on"] = "2026-11-03T04:05:00.000000+00:00"
        #expect(CodeRabbit.findings(fromValues: [edited], since: since) == 2)
        #expect(CodeRabbit.findings(fromValues: [edited], since: since, createdOnly: true) == nil)
        let fresh = rabbitComment("**Actionable comments posted: 2**", at: "2026-11-03T04:05:00.000000+00:00")
        #expect(CodeRabbit.findings(fromValues: [fresh], since: since, createdOnly: true) == 2)
    }

    @Test func findingsFromSummary() {
        let since = Date(timeIntervalSince1970: 1_793_678_400) // 2026-11-03T04:00:00Z
        let summary = rabbitComment("**Actionable comments posted: 2**", at: "2026-11-03T04:05:00.000000+00:00")
        let inline = rabbitComment("nit: rename this", at: "2026-11-03T04:06:00.000000+00:00")
        #expect(CodeRabbit.findings(fromValues: [inline, summary], since: since) == 2)

        let none = rabbitComment("No actionable comments were generated in the recent review.", at: "2026-11-03T04:05:00.000000+00:00")
        #expect(CodeRabbit.findings(fromValues: [none], since: since) == 0)

        let stale = rabbitComment("**Actionable comments posted: 2**", at: "2026-11-03T03:00:00.000000+00:00")
        #expect(CodeRabbit.findings(fromValues: [stale], since: since) == nil)
        #expect(CodeRabbit.findings(fromValues: [], since: since) == nil)
        #expect(CodeRabbit.findings(fromValues: [inline], since: since) == nil)
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
        #expect(CodeRabbit.hidesAskClaude(.awaitingApproval, threads: none, othersPending: false))
        #expect(CodeRabbit.hidesAskClaude(.nudged, threads: none, othersPending: false))
        #expect(CodeRabbit.hidesAskClaude(.approved, threads: none, othersPending: false))
        #expect(CodeRabbit.hidesAskClaude(.reviewed(findings: 0), threads: empty, othersPending: false))
        #expect(!CodeRabbit.hidesAskClaude(.reviewed(findings: 5), threads: some, othersPending: false))
        #expect(!CodeRabbit.hidesAskClaude(.reviewing, threads: none, othersPending: false))
        #expect(!CodeRabbit.hidesAskClaude(.awaitingApproval, threads: nil, othersPending: false))
        #expect(!CodeRabbit.hidesAskClaude(.failed, threads: none, othersPending: false))
        #expect(!CodeRabbit.hidesAskClaude(.skipped, threads: none, othersPending: false))
        #expect(!CodeRabbit.hidesAskClaude(nil, threads: none, othersPending: false))
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

    // MARK: Rules 7-10

    private static let tc = Date(timeIntervalSince1970: 1_793_682_000) // 2026-11-03T05:00:00Z
    private static func at(_ seconds: TimeInterval) -> String {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f.string(from: tc.addingTimeInterval(seconds))
    }

    private func derived(statuses: [[String: Any]], comments: [[String: Any]], now: TimeInterval, me: String? = nil, author: String? = nil) -> RabbitView {
        CodeRabbit.derive(RabbitInputs(commit: "abc123", statuses: statuses, comments: comments, summary: nil, approvers: [], me: me, author: author),
                          now: Self.tc.addingTimeInterval(now))
    }

    private var reviewed: [String: Any] { rabbitStatus(description: "Review completed", created: Self.at(-600)) }
    private var approveCommand: [String: Any] { humanReply(id: 10, parent: nil, "@coderabbitai approve", at: Self.at(0), inline: false) }

    @Test func approvalSkippedIsApprovalFailed() {
        let status = rabbitStatus(description: "Approve command performed: Comments resolved. Approval skipped: durable Cloud recovery is unavailable")
        let v = derived(statuses: [status], comments: [], now: 60)
        #expect(v.rabbit == .approvalFailed(reason: "durable Cloud recovery is unavailable"))
        #expect(v.rabbit.map { CodeRabbit.label($0) } == "duyệt lỗi")
        let already = rabbitStatus(description: "Approve command performed: Approval skipped: already approved")
        #expect(derived(statuses: [already], comments: [], now: 60).rabbit == .approved)
    }

    private var failedComment: [String: Any] { rabbitComment("Comments resolved, but approval failed.", at: Self.at(30)) }

    @Test func approvalFailedFromComment() {
        let v = derived(statuses: [reviewed], comments: [approveCommand, failedComment], now: 60)
        #expect(v.rabbit == .approvalFailed(reason: "Comments resolved, but approval failed."))
    }

    @Test func approvalFailedCommentSurvivesApproveStatusUpdate() {
        let status = rabbitStatus(description: "Approve command performed: Comments resolved.", created: Self.at(-600), updated: Self.at(60))
        let v = derived(statuses: [status], comments: [approveCommand, failedComment], now: 90)
        #expect(v.rabbit == .approvalFailed(reason: "Comments resolved, but approval failed."))
    }

    @Test func approvalFailedCommentClearedByNewReview() {
        let fresh = rabbitStatus(description: "Review completed", created: Self.at(120))
        let v = derived(statuses: [reviewed, fresh], comments: [approveCommand, failedComment], now: 150)
        #expect(v.rabbit == .awaitingApproval)
    }

    @Test func newerCommandSupersedesApprovalFailed() {
        let again = humanReply(id: 11, parent: nil, "@coderabbitai approve", at: Self.at(90), inline: false)
        let v = derived(statuses: [reviewed], comments: [approveCommand, failedComment, again], now: 100)
        #expect(v.rabbit == .nudged)
    }

    private var answeredThread: [[String: Any]] {
        [rabbitInline(id: 1, "nit", at: Self.at(-500)), humanReply(id: 2, parent: 1, "fixed", at: Self.at(-400)),
         rabbitInline(id: 3, "ok", at: Self.at(-300), parent: 1)]
    }

    @Test func nudgeEndsOnStatusUpdate() {
        let comments = answeredThread + [approveCommand]
        #expect(derived(statuses: [reviewed], comments: comments, now: 40).rabbit == .nudged)
        let updated = rabbitStatus(description: "Review completed", created: Self.at(-600), updated: Self.at(30))
        #expect(derived(statuses: [updated], comments: comments, now: 40).rabbit == .awaitingApproval)
    }

    @Test func nudgeTimesOutAfter600s() {
        let comments = answeredThread + [approveCommand]
        #expect(derived(statuses: [reviewed], comments: comments, now: 599).rabbit == .nudged)
        #expect(derived(statuses: [reviewed], comments: comments, now: 601).rabbit == .awaitingApproval)
    }

    @Test func addressedRootIsDone() {
        let root = rabbitInline(id: 1, "✅ Addressed in commits 6ae36ec to e0eaca7", at: Self.at(-300))
        #expect(derived(statuses: [reviewed], comments: [root], now: 60).threads == RabbitThreads(pending: 0, total: 1))
        let later = humanReply(id: 2, parent: 1, "thanks, but also...", at: Self.at(-100))
        #expect(derived(statuses: [reviewed], comments: [root, later], now: 60).threads == RabbitThreads(pending: 0, total: 1))
    }

    @Test func othersPendingIgnoresAuthorAndMe() {
        func pending(_ comments: [[String: Any]], me: String? = "{me}") -> Bool {
            CodeRabbit.facts(from: comments, me: me, author: "{a}").othersPending
        }
        let r = humanReply(id: 1, parent: nil, uuid: "{r}", "please rename", at: Self.at(0))
        #expect(pending([r]))
        #expect(pending([r, humanReply(id: 2, parent: 1, uuid: "{a}", "done", at: Self.at(10))]))
        #expect(!pending([r, humanReply(id: 2, parent: 1, uuid: "{me}", "done", at: Self.at(10))]))
        #expect(!pending([humanReply(id: 1, parent: nil, uuid: "{a}", "note", at: Self.at(0))]))
        #expect(!pending([humanReply(id: 1, parent: nil, uuid: "{r}", "@coderabbitai review", at: Self.at(0))]))
        #expect(!pending([r], me: nil))
        // The CodeRabbit account's own unmarked comments are not a reviewer's.
        let bot = rabbitComment("summary", at: Self.at(-10))
        #expect(!pending([bot, humanReply(id: 1, parent: nil, uuid: "{tin}", "reply from the bot account", at: Self.at(0))]))
    }

    @Test func hidesAskClaudeKeepsButtonForOthers() {
        let none = RabbitThreads(pending: 0, total: 0)
        #expect(CodeRabbit.hidesAskClaude(.awaitingApproval, threads: none, othersPending: false))
        #expect(!CodeRabbit.hidesAskClaude(.awaitingApproval, threads: none, othersPending: true))
        #expect(CodeRabbit.hidesAskClaude(.approvalFailed(reason: "x"), threads: none, othersPending: false))
    }
}
