import Testing
import Foundation
@testable import ClaudeSessionsKit

private let ws = "t6engineering"
private func ref(_ repo: String, _ id: Int) -> PRRef { PRRef(workspace: ws, repo: repo, id: id) }
private func link(_ repo: String, _ id: Int) -> String { "https://bitbucket.org/\(ws)/\(repo)/pull-requests/\(id)" }
private func status(_ o: [String: Any]) -> PRStatus { PullRequests.status(from: o)! }
private func data(_ o: [String: Any]) -> Data { try! JSONSerialization.data(withJSONObject: o) }

@Suite struct PullRequestsTests {
    @Test func refsFromText() {
        let text = "see \(link("talo6-dct-backend", 7571)) and \(link("talo6-dct-frontend", 6874)), again \(link("talo6-dct-backend", 7571))"
        #expect(PullRequests.refs(in: text) == [ref("talo6-dct-backend", 7571), ref("talo6-dct-frontend", 6874)])
        #expect(PullRequests.refs(in: "https://github.com/a/b/pull/3").isEmpty)
    }

    @Test func remoteParsing() {
        let a = PullRequests.repo(fromRemote: "git@bitbucket.org:t6engineering/talo6-dct-backend.git\n")
        #expect(a?.workspace == ws && a?.repo == "talo6-dct-backend")
        let b = PullRequests.repo(fromRemote: "https://phong@bitbucket.org/t6engineering/talos-v2.git")
        #expect(b?.workspace == ws && b?.repo == "talos-v2")
        let c = PullRequests.repo(fromRemote: "https://bitbucket.org/t6engineering/talos-v2")
        #expect(c?.workspace == ws && c?.repo == "talos-v2")
        #expect(PullRequests.repo(fromRemote: "git@github.com:a/b.git") == nil)
    }

    @Test func statusFromListValue() {
        let s = status(prJSON(id: 7571, comments: 23, reviewers: 5))
        #expect(s.reviewers == 5 && s.approved == 1 && s.changesRequested == 1 && s.comments == 23)
        #expect(s.branch == "bugfix/T6IR-14744")
        #expect(s.sourceRepo == "t6engineering/talo6-dct-backend")
        #expect(s.isOpen && s.ref == ref("talo6-dct-backend", 7571) && s.commit == "abc123")
        #expect(s.pipeline == nil && s.lastOther == nil)
        #expect(PullRequests.status(from: ["state": "OPEN"]) == nil)
    }

    @Test func pageWithNext() {
        let p = PullRequests.page(data(["values": [prJSON(id: 1), prJSON(id: 2)], "next": "https://x/next"]))
        #expect(p?.values.count == 2 && p?.next == "https://x/next")
        #expect(PullRequests.page(data(["values": []]))?.next == nil)
        #expect(PullRequests.page(Data("nope".utf8)) == nil)
    }

    @Test func pipelineAggregation() {
        func p(_ states: [String]) -> Pipeline? {
            PullRequests.pipeline(fromStatuses: data(["values": states.map { ["state": $0] }]))
        }
        #expect(p(["FAILED", "SUCCESSFUL"]) == .failed)
        #expect(p(["INPROGRESS", "SUCCESSFUL"]) == .running)
        #expect(p(["SUCCESSFUL"]) == .passed)
        #expect(p([]) == nil)
    }

    @Test func pipelineIgnoresOldCommits() {
        let d = data(["values": [
            ["state": "FAILED", "commit": ["hash": "old111" + String(repeating: "0", count: 34)]],
            ["state": "SUCCESSFUL", "commit": ["hash": "abc123" + String(repeating: "0", count: 34)]],
        ]])
        #expect(PullRequests.pipeline(fromStatuses: d, commit: "abc123") == .passed)
        #expect(PullRequests.pipeline(fromStatuses: d, commit: nil) == .failed)
    }

    @Test func lastOtherSkipsMeAndDeleted() {
        func c(_ uuid: String, _ name: String, _ at: String, deleted: Bool = false) -> [String: Any] {
            ["created_on": at, "user": ["uuid": uuid, "display_name": name], "deleted": deleted]
        }
        let d = data(["values": [c("{me}", "Me", "2026-11-04T06:00:00.000000+00:00"),
                                 c("{x}", "Gone", "2026-11-04T05:30:00.000000+00:00", deleted: true),
                                 c("{t}", "Tin Tran", "2026-11-04T05:02:17.239112+00:00")]])
        let got = PullRequests.lastOther(fromComments: d, me: "{me}")
        #expect(got?.name == "Tin Tran")
        #expect(abs(got!.at.timeIntervalSince1970 - 1_793_768_537.239) < 0.01)
        let plain = PullRequests.lastOther(fromComments: data(["values": [c("{t}", "T", "2026-11-04T05:02:17Z")]]), me: "{me}")
        #expect(plain?.at == Date(timeIntervalSince1970: 1_793_768_537))
    }

    @Test func eventsEachKind() {
        let base = status(prJSON(id: 1, reviewers: 3))
        #expect(PullRequests.events(old: base, new: base) == [])
        var p = base; p.pipeline = .failed
        #expect(PullRequests.events(old: base, new: p) == [])

        var withComment = base; withComment.lastOther = PRComment(name: "A", at: Date(timeIntervalSince1970: 100))
        #expect(PullRequests.events(old: base, new: withComment) == [.comment])
        var newer = withComment; newer.lastOther = PRComment(name: "B", at: Date(timeIntervalSince1970: 200))
        #expect(PullRequests.events(old: withComment, new: newer) == [.comment])
        #expect(PullRequests.events(old: newer, new: withComment) == [])

        let fewer = status(prJSON(id: 1, reviewers: 1))   // 0 approved, 1 changesRequested
        let more = status(prJSON(id: 1, reviewers: 3))    // 1 approved, 1 changesRequested
        #expect(PullRequests.events(old: fewer, new: more) == [.approved])
        let none = status(prJSON(id: 1, reviewers: 0))
        #expect(PullRequests.events(old: none, new: fewer) == [.changesRequested])

        #expect(PullRequests.events(old: base, new: status(prJSON(id: 1, state: "MERGED", reviewers: 3))) == [.merged])
        #expect(PullRequests.events(old: base, new: status(prJSON(id: 1, state: "DECLINED", reviewers: 3))) == [.declined])
        #expect(PullRequests.events(old: base, new: status(prJSON(id: 1, state: "SUPERSEDED", reviewers: 3))) == [.declined])
        let merged = status(prJSON(id: 1, state: "MERGED", reviewers: 3))
        #expect(PullRequests.events(old: merged, new: status(prJSON(id: 1, state: "DECLINED", reviewers: 3))) == [])
    }

    @Test func chimePriority() {
        #expect(PullRequests.chime(for: [.approved, .comment]) == .needsYou)
        #expect(PullRequests.chime(for: [.merged]) == .done)
        #expect(PullRequests.chime(for: [.approved]) == .done)
        #expect(PullRequests.chime(for: [.declined]) == .needsYou)
        #expect(PullRequests.chime(for: []) == nil)
    }

    @Test func promptTemplate() {
        let url = URL(string: link("talo6-dct-backend", 7571))!
        #expect(PullRequests.prompt("Fix {url} (#{id})\nnow", url: url, id: 7571) == "Fix \(url.absoluteString) (#7571) now")
        let d = PullRequests.prompt("  ", url: url, id: 7571)
        #expect(d == PullRequests.defaultPrompt.replacingOccurrences(of: "{url}", with: url.absoluteString))
    }
}
