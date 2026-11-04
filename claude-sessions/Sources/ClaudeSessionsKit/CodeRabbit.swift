import Foundation

public enum Rabbit: Equatable { case reviewing, reviewed(findings: Int?), approved, skipped, failed }

enum CodeRabbit {
    static let marker = "coderabbit.ai/#html-comment"

    static func isStatus(_ v: [String: Any]) -> Bool {
        (v["key"] as? String)?.hasPrefix("coderabbitai/") == true || v["name"] as? String == "CodeRabbit"
    }

    static func isComment(_ v: [String: Any]) -> Bool {
        ((v["content"] as? [String: Any])?["raw"] as? String)?.contains(marker) == true
    }

    /// The newest CodeRabbit status of `commit`; `findings` is filled in by `findings(fromComments:since:)`.
    static func state(fromStatuses data: Data, commit: String?) -> (rabbit: Rabbit, since: Date)? {
        guard let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let values = o["values"] as? [[String: Any]] else { return nil }
        func stamp(_ v: [String: Any], _ key: String) -> Date? { (v[key] as? String).flatMap(PullRequests.date) }
        let newest = values.filter { v in
            guard isStatus(v) else { return false }
            guard let commit, let hash = (v["commit"] as? [String: Any])?["hash"] as? String else { return true }
            return hash.hasPrefix(commit)
        }.max { (stamp($0, "updated_on") ?? .distantPast) < (stamp($1, "updated_on") ?? .distantPast) }
        guard let s = newest, let state = s["state"] as? String else { return nil }
        let since = stamp(s, "created_on") ?? stamp(s, "updated_on") ?? .distantPast
        let text = (s["description"] as? String ?? "").lowercased()
        func has(_ words: [String]) -> Bool { words.contains { text.contains($0) } }
        switch state {
        case "INPROGRESS": return (.reviewing, since)
        case "FAILED": return (.failed, since)
        case "STOPPED": return (.skipped, since)
        case "SUCCESSFUL":
            if has(["failed", "could not", "incomplete", "not published", "error"]) { return (.failed, since) }
            if has(["skipped", "paused", "no files", "too many files", "rate limited", "no reviewable"]) { return (.skipped, since) }
            if text.contains("approv") { return (.approved, since) }
            return (.reviewed(findings: nil), since)
        default: return nil
        }
    }

    private static let actionable = try! NSRegularExpression(pattern: #"Actionable comments posted: (\d+)"#)

    enum Findings: Equatable { case notFound, stale, count(Int) }

    /// Count from the newest summary comment (marked, with a count line); `.stale` when it predates `since`,
    /// `.notFound` when this page of comments holds no summary.
    static func findings(fromComments data: Data, since: Date) -> Findings {
        guard let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let values = o["values"] as? [[String: Any]] else { return .notFound }
        for v in values where (v["deleted"] as? Bool) != true && isComment(v) {
            let raw = (v["content"] as? [String: Any])?["raw"] as? String ?? ""
            let count: Int
            let ns = raw as NSString
            if let m = actionable.firstMatch(in: raw, range: NSRange(location: 0, length: ns.length)),
               let n = Int(ns.substring(with: m.range(at: 1))) {
                count = n
            } else if raw.contains("No actionable comments") {
                count = 0
            } else {
                continue
            }
            let dates = ["created_on", "updated_on"].compactMap { (v[$0] as? String).flatMap(PullRequests.date) }
            return dates.contains { $0 >= since } ? .count(count) : .stale
        }
        return .notFound
    }

    static func label(_ r: Rabbit) -> String {
        switch r {
        case .reviewing: return "🐰 đang review"
        case .reviewed(let n): return n.map { $0 > 0 } == true ? "🐰 \(n!) góp ý" : "🐰 xong"
        case .approved: return "🐰 đã duyệt"
        case .skipped: return "🐰 bỏ qua"
        case .failed: return "🐰 lỗi"
        }
    }

    static func isFinal(_ r: Rabbit) -> Bool {
        switch r {
        case .reviewed, .approved, .failed: return true
        case .reviewing, .skipped: return false
        }
    }

    /// `r` without its findings count, to tell a new outcome from a recount.
    static func kind(_ r: Rabbit?) -> Rabbit? {
        if case .reviewed = r { return .reviewed(findings: nil) }
        return r
    }
}
