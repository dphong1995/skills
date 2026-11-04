import Foundation

public struct PRRef: Hashable {
    public let workspace: String
    public let repo: String
    public let id: Int
    public init(workspace: String, repo: String, id: Int) {
        self.workspace = workspace; self.repo = repo; self.id = id
    }
}

public enum Pipeline: Equatable { case running, passed, failed }

public struct PRComment: Equatable {
    public let name: String
    public let at: Date
    public init(name: String, at: Date) { self.name = name; self.at = at }
}

public struct PRStatus: Equatable {
    public let ref: PRRef
    public let url: URL
    /// OPEN, MERGED, DECLINED or SUPERSEDED.
    public let state: String
    public let reviewers: Int
    public let approved: Int
    public let changesRequested: Int
    public let comments: Int
    public let commit: String?
    /// `full_name` of the source repository.
    public let sourceRepo: String?
    public let branch: String?
    public var pipeline: Pipeline?
    public var lastOther: PRComment?

    public var isOpen: Bool { state == "OPEN" }
}

public enum PREvent: Equatable { case comment, approved, changesRequested, merged, declined }

enum PullRequests {
    static let defaultPrompt = "Đọc các bình luận chưa xử lý và kết quả pipeline của PR {url}, sửa code theo đó, push, rồi trả lời từng bình luận."

    private static let linkRegex = try! NSRegularExpression(
        pattern: #"https://bitbucket\.org/([\w.-]+)/([\w.-]+)/pull-requests/(\d+)"#)

    static func refs(in text: String) -> [PRRef] {
        var seen = Set<PRRef>()
        var out: [PRRef] = []
        let ns = text as NSString
        for m in linkRegex.matches(in: text, range: NSRange(location: 0, length: ns.length)) {
            guard let id = Int(ns.substring(with: m.range(at: 3))) else { continue }
            let ref = PRRef(workspace: ns.substring(with: m.range(at: 1)), repo: ns.substring(with: m.range(at: 2)), id: id)
            if seen.insert(ref).inserted { out.append(ref) }
        }
        return out
    }

    /// `!` bash-mode and local-command output, which Claude Code stores as plain user lines.
    static func isShellOutput(_ s: String) -> Bool {
        let t = s.trimmingCharacters(in: .whitespacesAndNewlines)
        return ["<bash-stdout>", "<bash-stderr>", "<local-command-stdout>", "<local-command-stderr>"].contains { t.hasPrefix($0) }
    }

    private static let remoteRegex = try! NSRegularExpression(
        pattern: #"^(?:git@bitbucket\.org:|https://(?:[^@/]+@)?bitbucket\.org/)([\w.-]+)/([\w.-]+?)(?:\.git)?/?$"#)

    static func repo(fromRemote s: String) -> (workspace: String, repo: String)? {
        let t = s.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let m = remoteRegex.firstMatch(in: t, range: NSRange(location: 0, length: (t as NSString).length))
        else { return nil }
        let ns = t as NSString
        return (ns.substring(with: m.range(at: 1)), ns.substring(with: m.range(at: 2)))
    }

    static func status(from o: [String: Any]) -> PRStatus? {
        guard let href = ((o["links"] as? [String: Any])?["html"] as? [String: Any])?["href"] as? String,
              let ref = refs(in: href).first, let url = URL(string: href)
        else { return nil }
        let reviewers = (o["participants"] as? [[String: Any]] ?? []).filter { $0["role"] as? String == "REVIEWER" }
        let source = o["source"] as? [String: Any]
        return PRStatus(
            ref: ref, url: url, state: o["state"] as? String ?? "",
            reviewers: reviewers.count,
            approved: reviewers.filter { $0["approved"] as? Bool == true }.count,
            changesRequested: reviewers.filter { $0["state"] as? String == "changes_requested" }.count,
            comments: o["comment_count"] as? Int ?? 0,
            commit: (source?["commit"] as? [String: Any])?["hash"] as? String,
            sourceRepo: (source?["repository"] as? [String: Any])?["full_name"] as? String,
            branch: (source?["branch"] as? [String: Any])?["name"] as? String)
    }

    static func page(_ data: Data) -> (values: [PRStatus], next: String?)? {
        guard let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        let values = (o["values"] as? [[String: Any]] ?? []).compactMap(status(from:))
        return (values, o["next"] as? String)
    }

    /// Only statuses of `commit` count; statuses or a PR without a commit hash are kept.
    static func pipeline(fromStatuses data: Data, commit: String? = nil) -> Pipeline? {
        guard let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              var values = o["values"] as? [[String: Any]] else { return nil }
        if let commit {
            values = values.filter {
                guard let hash = ($0["commit"] as? [String: Any])?["hash"] as? String else { return true }
                return hash.hasPrefix(commit)
            }
        }
        let states = Set(values.compactMap { $0["state"] as? String })
        if states.contains("FAILED") { return .failed }
        if states.contains("INPROGRESS") { return .running }
        if states.contains("SUCCESSFUL") { return .passed }
        return nil
    }

    static func lastOther(fromComments data: Data, me: String) -> PRComment? {
        guard let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let values = o["values"] as? [[String: Any]] else { return nil }
        for v in values where (v["deleted"] as? Bool) != true {
            let user = v["user"] as? [String: Any]
            guard user?["uuid"] as? String != me else { continue }
            guard let name = user?["display_name"] as? String, let s = v["created_on"] as? String,
                  let at = date(s) else { return nil }
            return PRComment(name: name, at: at)
        }
        return nil
    }

    private static let fractionalDate: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f
    }()
    private static let plainDate = ISO8601DateFormatter()

    private static func date(_ s: String) -> Date? {
        fractionalDate.date(from: s) ?? plainDate.date(from: s)
    }

    static func events(old: PRStatus, new: PRStatus) -> [PREvent] {
        var out: [PREvent] = []
        if let at = new.lastOther?.at, old.lastOther.map({ at > $0.at }) ?? true { out.append(.comment) }
        if new.approved > old.approved { out.append(.approved) }
        if new.changesRequested > old.changesRequested { out.append(.changesRequested) }
        if old.isOpen && new.state == "MERGED" { out.append(.merged) }
        if old.isOpen && (new.state == "DECLINED" || new.state == "SUPERSEDED") { out.append(.declined) }
        return out
    }

    static func chime(for events: [PREvent]) -> Chime? {
        if events.contains(where: { $0 == .comment || $0 == .changesRequested || $0 == .declined }) { return .needsYou }
        if events.contains(where: { $0 == .approved || $0 == .merged }) { return .done }
        return nil
    }

    static func prompt(_ template: String, url: URL, id: Int) -> String {
        let t = template.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? defaultPrompt : template
        return t.replacingOccurrences(of: "{url}", with: url.absoluteString)
            .replacingOccurrences(of: "{id}", with: String(id))
            .replacingOccurrences(of: "\r\n", with: " ")
            .replacingOccurrences(of: "\n", with: " ")
    }
}
