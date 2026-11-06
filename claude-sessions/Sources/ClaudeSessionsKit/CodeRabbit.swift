import Foundation

public enum Rabbit: Equatable { case reviewing, reviewed(findings: Int?), approved, skipped, failed, awaitingApproval, nudged }

struct RabbitFacts: Equatable {
    /// uuid of the author of the newest marked comment.
    var rabbitUser: String?
    /// Every top-level inline CodeRabbit finding has been answered.
    var allAnswered: Bool
    /// The newest approve command is newer than every top-level CodeRabbit comment.
    var nudgePending: Bool
}

enum CodeRabbit {
    static let marker = "coderabbit.ai/#html-comment"

    private static func isStatus(_ v: [String: Any]) -> Bool {
        (v["key"] as? String)?.hasPrefix("coderabbitai/") == true || v["name"] as? String == "CodeRabbit"
    }

    private static func raw(_ v: [String: Any]) -> String { (v["content"] as? [String: Any])?["raw"] as? String ?? "" }

    private static func stamp(_ v: [String: Any], _ key: String) -> Date? { (v[key] as? String).flatMap(ISODate.date) }

    static func isComment(_ v: [String: Any]) -> Bool { raw(v).contains(marker) }

    /// The newest CodeRabbit status of `commit`; `findings` is filled in by `findings(fromComments:since:)`.
    static func state(fromStatuses data: Data, commit: String?) -> (rabbit: Rabbit, since: Date)? {
        guard let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let values = o["values"] as? [[String: Any]] else { return nil }
        let newest = values.filter { v in
            guard isStatus(v) else { return false }
            guard let commit, let hash = (v["commit"] as? [String: Any])?["hash"] as? String else { return true }
            return hash.hasPrefix(commit)
        }.max { (stamp($0, "updated_on") ?? .distantPast) < (stamp($1, "updated_on") ?? .distantPast) }
        guard let s = newest, let state = s["state"] as? String else { return nil }
        // CodeRabbit's INPROGRESS status lingers after its summary is posted, so its start is its last update.
        let since = (state == "INPROGRESS" ? stamp(s, "updated_on") : nil) ?? stamp(s, "created_on") ?? stamp(s, "updated_on") ?? .distantPast
        let text = (s["description"] as? String ?? "").lowercased()
        func has(_ words: [String]) -> Bool { words.contains { text.contains($0) } }
        switch state {
        case "INPROGRESS": return (.reviewing, since)
        case "FAILED": return (.failed, since)
        case "STOPPED": return (.skipped, since)
        case "SUCCESSFUL":
            if text.hasPrefix("approve command performed") {
                return (has(["approval completed", "already approved"]) ? .approved : .reviewed(findings: nil), since)
            }
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
    /// `.notFound` when this page of comments holds no summary. `createdOnly` ignores `updated_on`, which CodeRabbit
    /// bumps when it edits an old summary.
    static func findings(fromComments data: Data, since: Date, createdOnly: Bool = false) -> Findings {
        guard let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let values = o["values"] as? [[String: Any]] else { return .notFound }
        for v in values where (v["deleted"] as? Bool) != true && isComment(v) {
            let raw = raw(v)
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
            let dates = (createdOnly ? ["created_on"] : ["created_on", "updated_on"]).compactMap { stamp(v, $0) }
            return dates.contains { $0 >= since } ? .count(count) : .stale
        }
        return .notFound
    }

    static func label(_ r: Rabbit) -> String {
        switch r {
        case .reviewing: return "🐰 đang review"
        case .reviewed(let n?) where n > 0: return "🐰 \(n) góp ý"
        case .reviewed: return "🐰 xong"
        case .approved: return "🐰 đã duyệt"
        case .skipped: return "🐰 bỏ qua"
        case .failed: return "🐰 lỗi"
        case .awaitingApproval: return "🐰 chờ duyệt"
        case .nudged: return "🐰 đang nhắc"
        }
    }

    static func isFinal(_ r: Rabbit) -> Bool {
        switch r {
        case .reviewed, .approved, .failed, .awaitingApproval: return true
        case .reviewing, .skipped, .nudged: return false
        }
    }

    private static let nudgeCommand = try! NSRegularExpression(pattern: #"@coderabbit(ai)?\s+approve"#, options: .caseInsensitive)

    /// What the full comment list says about the CodeRabbit account, its inline threads and a pending approve command.
    static func facts(from values: [[String: Any]]) -> RabbitFacts {
        let live = values.filter { ($0["deleted"] as? Bool) != true }
        func created(_ v: [String: Any]) -> Date { stamp(v, "created_on") ?? .distantPast }
        func parent(_ v: [String: Any]) -> Int? { (v["parent"] as? [String: Any])?["id"] as? Int }
        let marked = live.filter(isComment)
        let rabbitUser = marked.max { created($0) < created($1) }
            .flatMap { ($0["user"] as? [String: Any])?["uuid"] as? String }

        var children: [Int: [[String: Any]]] = [:]
        for v in live { if let p = parent(v) { children[p, default: []].append(v) } }
        let allAnswered = marked.filter { $0["inline"] != nil && parent($0) == nil }.allSatisfy { root in
            var thread = [root]
            var i = 0
            while i < thread.count {
                if let id = thread[i]["id"] as? Int { thread += children[id] ?? [] }
                i += 1
            }
            guard thread.contains(where: { !isComment($0) }), let newest = thread.max(by: { created($0) < created($1) }) else { return false }
            return isComment(newest)
        }

        let command = live.filter { v in
            let r = raw(v)
            return !isComment(v) && nudgeCommand.firstMatch(in: r, range: NSRange(location: 0, length: (r as NSString).length)) != nil
        }.map(created).max()
        let topLevel = marked.filter { parent($0) == nil && $0["inline"] == nil }.map(created).max()
        let nudgePending = command.map { c in topLevel.map { c > $0 } ?? true } ?? false
        return RabbitFacts(rabbitUser: rabbitUser, allAnswered: allAnswered, nudgePending: nudgePending)
    }

    /// The displayed state from the raw status, the PR's approvers and the comment facts.
    static func resolve(_ status: Rabbit?, approvers: Set<String>, facts: RabbitFacts?) -> Rabbit? {
        if let u = facts?.rabbitUser, approvers.contains(u) { return .approved }
        if status == .reviewing { return .reviewing }
        switch status {
        case .failed?, .skipped?: return status
        case .reviewed?, .approved?:
            guard let facts, facts.rabbitUser != nil else { return status }
            if facts.allAnswered { return facts.nudgePending ? .nudged : .awaitingApproval }
            if case .approved? = status { return .reviewed(findings: nil) }
            return status
        default: return status
        }
    }

    /// `r` without its findings count, to tell a new outcome from a recount.
    static func kind(_ r: Rabbit?) -> Rabbit? {
        if case .reviewed = r { return .reviewed(findings: nil) }
        return r
    }
}
