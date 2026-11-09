import Foundation

public enum Rabbit: Equatable { case reviewing, reviewed(findings: Int?), approved, skipped, failed, awaitingApproval, nudged, approvalFailed(reason: String) }

struct RabbitFacts: Equatable {
    /// uuid of the author of the newest marked comment.
    var rabbitUser: String?
    /// CodeRabbit inline threads (a live top-level inline comment and its descendants), and those not answered:
    /// no human reply, or the newest comment is not CodeRabbit's.
    var threads: RabbitThreads
    /// Every top-level inline CodeRabbit finding has been answered.
    var allAnswered: Bool { threads.pending == 0 }
    /// The newest approve command is newer than every top-level CodeRabbit comment.
    var nudgePending: Bool
    /// When the newest approve command was posted.
    var command: Date? = nil
    /// When CodeRabbit last posted a top-level comment saying the approval failed, and its line saying so.
    var approvalFailedAt: Date? = nil
    var approvalFailedReason: String? = nil
    /// A reviewer (not `me`, not the PR author, not the CodeRabbit account) has a comment `me` has not answered in its thread.
    var othersPending = false
}

/// The raw data `derive` reads; `statuses` and `comments` are nil until fetched.
struct RabbitInputs {
    var commit: String?
    var statuses: [[String: Any]]?
    var comments: [[String: Any]]?
    /// The newest summary comment seen for `commit`, kept for when `comments` no longer holds it.
    var summary: [String: Any]?
    var approvers: Set<String>
    var me: String?
    var author: String? = nil
    /// When the widget itself posted the approve command; the 600 s timeout counts from the later of that and the command's
    /// own time, so a clock that runs ahead of Bitbucket's cannot end it early. Commands first seen in a fetch get no floor.
    var nudgedAt: Date? = nil
}

struct RabbitView: Equatable {
    /// The displayed state.
    var rabbit: Rabbit?
    /// The state from the statuses and summary alone.
    var status: Rabbit?
    var since: Date?
    var threads: RabbitThreads?
    var othersPending = false
}

enum CodeRabbit {
    static let marker = "coderabbit.ai/#html-comment"

    private static func isStatus(_ v: [String: Any]) -> Bool {
        (v["key"] as? String)?.hasPrefix("coderabbitai/") == true || v["name"] as? String == "CodeRabbit"
    }

    private static func raw(_ v: [String: Any]) -> String { (v["content"] as? [String: Any])?["raw"] as? String ?? "" }

    private static func stamp(_ v: [String: Any], _ key: String) -> Date? { (v[key] as? String).flatMap(ISODate.date) }

    static func isComment(_ v: [String: Any]) -> Bool { raw(v).contains(marker) }

    /// The newest CodeRabbit status of `commit`.
    static func newestStatus(in values: [[String: Any]], commit: String?) -> [String: Any]? {
        values.filter { v in
            guard isStatus(v) else { return false }
            guard let commit, let hash = (v["commit"] as? [String: Any])?["hash"] as? String else { return true }
            return hash.hasPrefix(commit)
        }.max { (stamp($0, "updated_on") ?? .distantPast) < (stamp($1, "updated_on") ?? .distantPast) }
    }

    static func state(fromValues values: [[String: Any]], commit: String?) -> (rabbit: Rabbit, since: Date)? {
        newestStatus(in: values, commit: commit).flatMap(state(fromStatus:))
    }

    /// The `findings` of the result stay nil here; `findings(fromValues:since:createdOnly:)` fills them in.
    static func state(fromStatus s: [String: Any]) -> (rabbit: Rabbit, since: Date)? {
        guard let state = s["state"] as? String else { return nil }
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
                let description = s["description"] as? String ?? ""
                if let skipped = description.range(of: "Approval skipped: ", options: .caseInsensitive), !text.contains("already approved") {
                    return (.approvalFailed(reason: String(description[skipped.upperBound...]).trimmingCharacters(in: .whitespacesAndNewlines)), since)
                }
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

    /// The finding count of a summary comment, nil for any other comment.
    private static func summaryCount(_ v: [String: Any]) -> Int? {
        guard (v["deleted"] as? Bool) != true && isComment(v) else { return nil }
        let raw = raw(v)
        let ns = raw as NSString
        if let m = actionable.firstMatch(in: raw, range: NSRange(location: 0, length: ns.length)),
           let n = Int(ns.substring(with: m.range(at: 1))) {
            return n
        }
        return raw.contains("No actionable comments") ? 0 : nil
    }

    /// The first (newest, in `sort=-created_on` order) summary comment of `values`.
    static func newestSummary(in values: [[String: Any]]) -> [String: Any]? {
        values.first { summaryCount($0) != nil }
    }

    /// Count from the newest summary comment (marked, with a count line); nil when there is none or it predates `since`.
    /// `createdOnly` ignores `updated_on`, which CodeRabbit bumps when it edits an old summary.
    static func findings(fromValues values: [[String: Any]], since: Date, createdOnly: Bool = false) -> Int? {
        guard let v = newestSummary(in: values), let count = summaryCount(v) else { return nil }
        let dates = (createdOnly ? ["created_on"] : ["created_on", "updated_on"]).compactMap { stamp(v, $0) }
        return dates.contains { $0 >= since } ? count : nil
    }

    private static let nudgeTimeout: TimeInterval = 600

    /// Pure: the same inputs always give the same view.
    static func derive(_ i: RabbitInputs, now: Date) -> RabbitView {
        var facts = i.comments.map { CodeRabbit.facts(from: $0, me: i.me, author: i.author) }
        var status: Rabbit?
        var since: Date?
        var statusUpdated: Date?
        var statusDescription = ""
        if let values = i.statuses, let newest = newestStatus(in: values, commit: i.commit), let fresh = state(fromStatus: newest) {
            since = fresh.since
            statusUpdated = stamp(newest, "updated_on")
            statusDescription = ((newest["description"] as? String) ?? "").lowercased()
            let pool = (i.comments ?? []) + (i.summary.map { [$0] } ?? [])
            var r = fresh.rabbit
            // An approve command newer than the failed approval starts over: the answer to it is still to come.
            if case .approvalFailed = r, let c = facts?.command, let u = statusUpdated, c > u { r = .reviewed(findings: nil) }
            if r == .reviewing {
                // The summary is out; the status lags behind it.
                if let n = findings(fromValues: pool, since: fresh.since, createdOnly: true) { r = .reviewed(findings: n) }
            } else if case .reviewed = r {
                if let n = findings(fromValues: pool, since: fresh.since) { r = .reviewed(findings: n) }
            }
            status = r
        }
        // The newest approve command ends once CodeRabbit answered in a comment (kept by `facts`), updated its status, or 600 s passed.
        if let command = facts?.command, facts?.nudgePending == true,
           (statusUpdated.map { $0 > command } ?? false) || now.timeIntervalSince(max(command, i.nudgedAt ?? .distantPast)) >= nudgeTimeout {
            facts?.nudgePending = false
        }
        var shown = resolve(status, approvers: i.approvers, facts: facts)
        // The failure CodeRabbit wrote in a comment after the newest command stays until a new review (a status that is not
        // an approve command) or a newer command replaces it.
        if case .reviewed? = status, let f = facts, let command = f.command, let failed = f.approvalFailedAt, failed > command,
           !(statusUpdated.map { $0 > failed } ?? false) || statusDescription.hasPrefix("approve command performed") {
            switch shown {
            case .reviewed?, .awaitingApproval?: shown = .approvalFailed(reason: f.approvalFailedReason ?? "approval failed")
            default: break
            }
        }
        return RabbitView(rabbit: shown, status: status, since: since, threads: facts?.threads, othersPending: facts?.othersPending ?? false)
    }

    static func label(_ r: Rabbit, threads: RabbitThreads? = nil) -> String {
        if case .reviewed = r, let t = threads, t.total > 0, t.pending > 0 { return "còn \(t.pending)/\(t.total)" }
        switch r {
        case .reviewing: return "đang review"
        case .reviewed(let n?) where n > 0: return "\(n) góp ý"
        case .reviewed: return "xong"
        case .approved: return "đã duyệt"
        case .skipped: return "bỏ qua"
        case .failed: return "lỗi"
        case .awaitingApproval: return "chờ duyệt"
        case .nudged: return "đang nhắc"
        case .approvalFailed: return "duyệt lỗi"
        }
    }

    /// `Nhờ Claude` has nothing to ask while CodeRabbit is done and every thread is answered.
    static func hidesAskClaude(_ r: Rabbit?, threads: RabbitThreads?, othersPending: Bool) -> Bool {
        guard let threads, threads.pending == 0, !othersPending else { return false }
        switch r {
        case .reviewed?, .awaitingApproval?, .nudged?, .approved?, .approvalFailed?: return true
        default: return false
        }
    }

    static func isFinal(_ r: Rabbit) -> Bool {
        switch r {
        case .reviewed, .approved, .failed, .awaitingApproval, .approvalFailed: return true
        case .reviewing, .skipped, .nudged: return false
        }
    }

    private static let mention = try! NSRegularExpression(pattern: #"@coderabbit(ai)?\b"#, options: .caseInsensitive)

    private static let nudgeCommand = try! NSRegularExpression(pattern: #"@coderabbit(ai)?\s+approve"#, options: .caseInsensitive)

    /// What the full comment list says about the CodeRabbit account, its inline threads and a pending approve command.
    static func facts(from values: [[String: Any]], me: String? = nil, author: String? = nil) -> RabbitFacts {
        let live = values.filter { ($0["deleted"] as? Bool) != true }
        func created(_ v: [String: Any]) -> Date { stamp(v, "created_on") ?? .distantPast }
        func parent(_ v: [String: Any]) -> Int? { (v["parent"] as? [String: Any])?["id"] as? Int }
        func uuid(_ v: [String: Any]) -> String? { (v["user"] as? [String: Any])?["uuid"] as? String }
        func isTopLevel(_ v: [String: Any]) -> Bool { parent(v) == nil && v["inline"] == nil }
        let marked = live.filter(isComment)
        let rabbitUser = marked.max { created($0) < created($1) }.flatMap(uuid)

        // Each comment's thread root, found once; comments without an id stand alone.
        var parentOf: [Int: Int] = [:]
        for v in live { if let id = v["id"] as? Int, let p = parent(v) { parentOf[id] = p } }
        var threads: [Int: [[String: Any]]] = [:]
        var alone: [[String: Any]] = []
        for v in live {
            guard var id = v["id"] as? Int else { alone.append(v); continue }
            while let p = parentOf[id] { id = p }
            threads[id, default: []].append(v)
        }

        let roots = marked.filter { $0["inline"] != nil && parent($0) == nil }
        let pending = roots.filter { root in
            // CodeRabbit rewrites a root it has dealt with to `✅ Addressed in commit …`.
            if raw(root).contains("Addressed in commit") { return false }
            let thread = (root["id"] as? Int).flatMap { threads[$0] } ?? [root]
            guard thread.contains(where: { !isComment($0) }), let newest = thread.max(by: { created($0) < created($1) }) else { return true }
            return !isComment(newest)
        }.count

        let command = live.filter { v in
            let r = raw(v)
            return !isComment(v) && nudgeCommand.firstMatch(in: r, range: NSRange(location: 0, length: (r as NSString).length)) != nil
        }.map(created).max()
        let topLevel = marked.filter(isTopLevel).map(created).max()
        let nudgePending = command.map { c in topLevel.map { c > $0 } ?? true } ?? false
        let failed = marked.filter { isTopLevel($0) && raw($0).range(of: "approval failed", options: .caseInsensitive) != nil }
            .max { created($0) < created($1) }
        let approvalFailedReason = failed.flatMap { v in
            raw(v).split(whereSeparator: \.isNewline).first { $0.range(of: "approval failed", options: .caseInsensitive) != nil }
                .map { $0.trimmingCharacters(in: .whitespaces) }
        }

        // Reviewer comments `me` has not answered: nothing from `me` after them in their thread.
        var othersPending = false
        if let me {
            func unanswered(_ thread: [[String: Any]], _ answered: (Date) -> Bool) -> Bool {
                thread.contains { v in
                    let user = uuid(v)
                    let r = raw(v)
                    guard let user, user != me, user != author, user != rabbitUser, !isComment(v),
                          mention.firstMatch(in: r, range: NSRange(location: 0, length: (r as NSString).length)) == nil else { return false }
                    return !answered(created(v))
                }
            }
            othersPending = unanswered(alone) { _ in false } || threads.values.contains { thread in
                let latestMe = thread.filter { uuid($0) == me }.map(created).max()
                return unanswered(thread) { at in latestMe.map { $0 > at } ?? false }
            }
        }
        return RabbitFacts(rabbitUser: rabbitUser, threads: RabbitThreads(pending: pending, total: roots.count), nudgePending: nudgePending,
                           command: command, approvalFailedAt: failed.map(created), approvalFailedReason: approvalFailedReason,
                           othersPending: othersPending)
    }

    /// The displayed state from the raw status, the PR's approvers and the comment facts.
    static func resolve(_ status: Rabbit?, approvers: Set<String>, facts: RabbitFacts?) -> Rabbit? {
        if let u = facts?.rabbitUser, approvers.contains(u) { return .approved }
        if status == .reviewing { return .reviewing }
        switch status {
        case .failed?, .skipped?, .approvalFailed?: return status
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
