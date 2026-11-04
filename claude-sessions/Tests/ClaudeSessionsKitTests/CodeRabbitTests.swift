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
        #expect(CodeRabbit.label(.reviewed(findings: 2)) == "🐰 2 góp ý")
        #expect(CodeRabbit.label(.reviewed(findings: 0)) == "🐰 xong")
        #expect(CodeRabbit.label(.reviewed(findings: nil)) == "🐰 xong")
        #expect(CodeRabbit.label(.reviewing) == "🐰 đang review")
        #expect(CodeRabbit.label(.approved) == "🐰 đã duyệt")
        #expect(CodeRabbit.label(.skipped) == "🐰 bỏ qua")
        #expect(CodeRabbit.label(.failed) == "🐰 lỗi")
        #expect(CodeRabbit.isFinal(.reviewed(findings: nil)) && CodeRabbit.isFinal(.approved) && CodeRabbit.isFinal(.failed))
        #expect(!CodeRabbit.isFinal(.reviewing) && !CodeRabbit.isFinal(.skipped))
    }
}
