import Testing
import Foundation
@testable import ClaudeSessionsKit

private let fixtures = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("Fixtures")

private func loadJSON(_ name: String) -> Any {
    try! JSONSerialization.jsonObject(with: Data(contentsOf: fixtures.appendingPathComponent(name)))
}

private func parseDate(_ s: String) -> Date? {
    let f = ISO8601DateFormatter()
    f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    return f.date(from: s) ?? ISO8601DateFormatter().date(from: s)
}

/// Feeds one recorded PR through a `PRMonitor` and records what the card would show.
func replayOutcome(_ pr: [String: Any]) -> [String: Any] {
    let repo = pr["repo"] as! String
    let id = pr["id"] as! Int
    let comments = pr["comments"] as! [[String: Any]]
    let statuses = pr["statuses"] as! [[String: Any]]
    let newest = (comments + statuses).flatMap { [$0["created_on"], $0["updated_on"]] }
        .compactMap { ($0 as? String).flatMap(parseDate) }.max() ?? Date(timeIntervalSince1970: 1_800_000_000)
    let now = newest.addingTimeInterval(60)

    var open = prJSON(id: id, repo: repo, state: "OPEN", comments: pr["commentCount"] as! Int)
    open["source"] = ["branch": ["name": "b"], "repository": ["full_name": repo], "commit": ["hash": pr["commit"] as! String]]
    open["participants"] = pr["participants"]
    let api = fake(open: [open])
    api.set("/2.0/user", json: ["uuid": pr["author"] as! String])
    api.set("/pullrequests/\(id)/statuses", json: ["values": statuses])
    api.set("/pullrequests/\(id)/comments", json: ["values": comments])

    let m = PRMonitor(api: api, queue: nil, remote: { _ in nil })
    m.update([PRTarget(sessionId: "s", linkRefs: [prRef(id, repo: repo)], cwd: "/x", branch: nil)], at: now)
    let shown = m.takeSnapshot().prs["s"]?.first
    var out: [String: Any] = ["label": NSNull(), "pending": NSNull(), "total": NSNull(), "hidesAskClaude": false]
    if let shown {
        if let rabbit = shown.rabbit { out["label"] = CodeRabbit.label(rabbit, threads: shown.rabbitThreads) }
        if let t = shown.rabbitThreads { out["pending"] = t.pending; out["total"] = t.total }
        out["hidesAskClaude"] = CodeRabbit.hidesAskClaude(shown.rabbit, threads: shown.rabbitThreads)
    }
    return out
}

@Suite struct RabbitReplayTests {
    @Test func replayMatchesExpected() throws {
        let prs = (loadJSON("rabbit-replay.json") as! [String: Any])["prs"] as! [[String: Any]]
        var actual: [String: Any] = [:]
        for pr in prs {
            let key = pr["key"] as! String
            actual[key] = replayOutcome(pr)
            // Same data with nobody approving, so the threads/summary/awaiting-approval paths are reached.
            var noApprovers = pr
            noApprovers["participants"] = (pr["participants"] as! [[String: Any]]).map {
                var p = $0
                p["approved"] = false
                p["state"] = NSNull()
                return p
            }
            actual["\(key)/noApprovers"] = replayOutcome(noApprovers)
        }

        let expectedURL = fixtures.appendingPathComponent("rabbit-replay-expected.json")
        if ProcessInfo.processInfo.environment["UPDATE_REPLAY"] == "1" {
            try JSONSerialization.data(withJSONObject: actual, options: [.prettyPrinted, .sortedKeys]).write(to: expectedURL)
            return
        }
        let expected = loadJSON("rabbit-replay-expected.json") as! [String: [String: Any]]
        var diffs: [String] = []
        for key in Set(actual.keys).union(expected.keys).sorted() {
            let old = expected[key].map { NSDictionary(dictionary: $0).description.replacingOccurrences(of: "\n", with: " ") } ?? "missing"
            let new = (actual[key] as? [String: Any]).map { NSDictionary(dictionary: $0).description.replacingOccurrences(of: "\n", with: " ") } ?? "missing"
            if old != new { diffs.append("\(key): \(old) -> \(new)") }
        }
        let report = diffs.joined(separator: "\n")
        if !diffs.isEmpty { Issue.record(Comment(rawValue: "\(diffs.count) PRs differ:\n\(report)")) }
    }
}
