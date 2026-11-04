import Foundation

struct Pending: Equatable {
    let toolId: String
    let tool: String
    let text: String
}

enum PendingState: Equatable {
    case one(Pending)
    case unclear(count: Int)
}

public struct SessionLink: Equatable {
    public let label: String
    public let url: URL
}

enum Transcript {
    /// How much of a transcript's start is read for the first prompt and branch.
    static let headBytes = 65_536
    /// The most a refresh reads of a busy session's transcript for the running-tool line.
    static let activityWindow: UInt64 = 262_144
    static let approvable: Set<String> = ["Bash", "Edit", "Write", "MultiEdit", "NotebookEdit", "Read", "Glob", "Grep", "WebFetch", "WebSearch"]

    /// Claude Code names a project directory after the cwd with every non-alphanumeric char replaced by "-".
    static func projectDirName(_ cwd: String) -> String {
        String(cwd.unicodeScalars.map { s in
            (s.isASCII && (CharacterSet.alphanumerics.contains(s))) ? Character(s) : "-"
        })
    }

    static func find(sessionId: String, cwd: String, projects: URL) -> URL? {
        let fm = FileManager.default
        let direct = projects.appendingPathComponent(projectDirName(cwd)).appendingPathComponent("\(sessionId).jsonl")
        if fm.fileExists(atPath: direct.path) { return direct }
        let dirs = (try? fm.contentsOfDirectory(at: projects, includingPropertiesForKeys: nil)) ?? []
        return dirs.map { $0.appendingPathComponent("\(sessionId).jsonl") }.first { fm.fileExists(atPath: $0.path) }
    }

    static func tail(_ url: URL, bytes: Int = 262_144) -> Data {
        guard let h = try? FileHandle(forReadingFrom: url) else { return Data() }
        defer { try? h.close() }
        guard let end = try? h.seekToEnd() else { return Data() }
        try? h.seek(toOffset: end > UInt64(bytes) ? end - UInt64(bytes) : 0)
        return (try? h.readToEnd()) ?? Data()
    }

    /// The bytes `[from, to)` of the file.
    static func read(_ url: URL, from: UInt64, to: UInt64) -> Data {
        guard to > from, let h = try? FileHandle(forReadingFrom: url) else { return Data() }
        defer { try? h.close() }
        guard (try? h.seek(toOffset: from)) != nil else { return Data() }
        return (try? h.read(upToCount: Int(to - from))) ?? Data()
    }

    /// The first `bytes` of the file; when the file is longer, everything after the last newline is dropped.
    static func head(_ url: URL, bytes: Int = headBytes) -> Data {
        guard let h = try? FileHandle(forReadingFrom: url) else { return Data() }
        defer { try? h.close() }
        guard let d = try? h.read(upToCount: bytes + 1), !d.isEmpty else { return Data() }
        guard d.count > bytes else { return d }
        let cut = d.prefix(bytes)
        guard let nl = cut.lastIndex(of: 0x0A) else { return Data() }
        return Data(cut[cut.startIndex...nl])
    }

    static func stripReminders(_ text: String) -> String {
        var t = text
        while let a = t.range(of: "<system-reminder>"), let b = t.range(of: "</system-reminder>", range: a.upperBound..<t.endIndex) {
            t.removeSubrange(a.lowerBound..<b.upperBound)
        }
        return t
    }

    /// The Jira project of a ticket key: `T6IR` for `T6IR-12`.
    static func projectPrefix(of key: String) -> String? {
        key.split(separator: "-").first.map(String.init)
    }

    /// A one-line prompt for a row title, or nil when the text is not something the user typed.
    static func cleanPrompt(_ text: String) -> String? {
        var t = stripReminders(text).trimmingCharacters(in: .whitespacesAndNewlines)
        if t.hasPrefix("<local-command") || t.hasPrefix("Caveat:") { return nil }
        func inner(_ tag: String) -> String? {
            guard let a = t.range(of: "<\(tag)>"), let b = t.range(of: "</\(tag)>", range: a.upperBound..<t.endIndex) else { return nil }
            return String(t[a.upperBound..<b.lowerBound])
        }
        if t.hasPrefix("<command-"), let name = inner("command-name") {
            let args = inner("command-args")?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            t = args.isEmpty ? "" : "\(name) \(args)"
        }
        let collapsed = oneLine(t)
        if collapsed.isEmpty { return nil }
        return collapsed
    }

    /// `https?://` up to whitespace, brackets, parentheses or quotes; trailing punctuation is stripped by `urlRanges`.
    static let urlPattern = try! NSRegularExpression(pattern: #"https?://[^\s<>()\[\]"'`]+"#)
    private static let markdownLinkPattern = try! NSRegularExpression(pattern: #"\[([^\]]*)\]\((https?://[^\s)]+)\)"#)

    /// The URLs in `s` with their ranges, trailing `.,;:!?*` stripped.
    static func urlRanges(in s: String) -> [(range: Range<String.Index>, url: URL)] {
        let ns = s as NSString
        return urlPattern.matches(in: s, range: NSRange(location: 0, length: ns.length)).compactMap { m in
            var r = m.range
            while r.length > 0, ".,;:!?*".contains(ns.substring(with: NSRange(location: r.location + r.length - 1, length: 1))) {
                r.length -= 1
            }
            guard let range = Range(r, in: s), let url = URL(string: String(s[range])) else { return nil }
            return (range, url)
        }
    }

    private static let ticketPattern = try! NSRegularExpression(pattern: #"(?<![A-Za-z0-9-])([A-Z][A-Z0-9]{1,9})-(\d+)(?![A-Za-z0-9])"#)

    /// Every `KEY-123` in `text`, in order of appearance (a key repeated appears repeatedly).
    static func ticketKeys(in text: String) -> [String] {
        let ns = text as NSString
        return ticketPattern.matches(in: text, range: NSRange(location: 0, length: ns.length)).map { ns.substring(with: $0.range) }
    }

    /// The Jira key and host of a `/browse/KEY-N` path or a `selectedIssue=KEY-N` query item.
    static func jiraTicket(in url: URL) -> (key: String, host: String)? {
        guard let host = url.host else { return nil }
        let path = url.path
        if let m = jiraPattern.firstMatch(in: path, range: NSRange(path.startIndex..., in: path)),
           let r = Range(m.range(at: 1), in: path) {
            return (String(path[r]), host)
        }
        let item = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems?.first { $0.name == "selectedIssue" }?.value
        if let item, ticketKeys(in: item) == [item] { return (item, host) }
        return nil
    }

    static func isPullRequest(_ url: URL) -> Bool {
        let path = url.path
        return prPatterns.contains { $0.firstMatch(in: path, range: NSRange(path.startIndex..., in: path)) != nil }
    }

    /// Host plus the path up to the PR number: one key for `/pull-requests/12` and `/pull-requests/12/diff`.
    static func pullRequestKey(_ url: URL) -> String? {
        let path = url.path
        for re in prPatterns {
            if let m = re.firstMatch(in: path, range: NSRange(path.startIndex..., in: path)), let r = Range(m.range, in: path) {
                return (url.host ?? "") + path[..<r.upperBound]
            }
        }
        return nil
    }

    private static let prPatterns = [#"/pull-requests/(\d+)"#, #"/pull/(\d+)"#, #"/merge_requests/(\d+)"#]
        .map { try! NSRegularExpression(pattern: $0) }
    private static let jiraPattern = try! NSRegularExpression(pattern: #"/browse/([A-Z][A-Z0-9]{1,9}-\d+)"#)
    private static let issuePattern = try! NSRegularExpression(pattern: #"/issues/(\d+)"#)

    /// A short label for a link button or inline URL.
    static func label(for url: URL) -> String {
        let path = url.path
        func capture(_ re: NSRegularExpression) -> String? {
            guard let m = re.firstMatch(in: path, range: NSRange(path.startIndex..., in: path)),
                  let r = Range(m.range(at: 1), in: path) else { return nil }
            return String(path[r])
        }
        if let n = prPatterns.lazy.compactMap(capture).first { return "PR #\(n)" }
        if let k = capture(jiraPattern) { return k }
        if let n = capture(issuePattern) { return "#\(n)" }
        let host = url.host ?? url.absoluteString
        return host.hasPrefix("www.") ? String(host.dropFirst(4)) : host
    }

    /// The first `n` characters of `s`, plus `…` when `s` is longer.
    private static func cut(_ s: String, _ n: Int) -> String {
        s.count > n ? String(s.prefix(n)) + "…" : s
    }

    /// Markdown links become their text, bare URLs their label; then cut to `limit` characters plus `…`.
    static func displayText(_ text: String, limit: Int) -> String {
        let t = markdownLinkPattern.stringByReplacingMatches(in: text, range: NSRange(location: 0, length: (text as NSString).length), withTemplate: "$1")
        var out = ""
        var last = t.startIndex
        for (range, url) in urlRanges(in: t) {
            out += t[last..<range.lowerBound]
            out += label(for: url)
            last = range.upperBound
        }
        out += t[last...]
        return cut(out, limit)
    }

    /// Parses lazily, so a scan that stops at its first hit does not parse the rest.
    private static func objects(_ data: Data, reversed: Bool = false) -> some Sequence<[String: Any]> {
        let lines = data.split(separator: 0x0A)
        return (reversed ? AnySequence(lines.reversed()) : AnySequence(lines)).lazy
            .compactMap { try? JSONSerialization.jsonObject(with: Data($0)) as? [String: Any] }
    }

    static func text(of blocks: [[String: Any]]) -> String {
        blocks.compactMap { $0["type"] as? String == "text" ? $0["text"] as? String : nil }.joined(separator: " ")
    }

    private static func oneLine(_ s: String) -> String {
        s.split(whereSeparator: \.isWhitespace).joined(separator: " ")
    }

    static func firstPrompt(in data: Data) -> String? {
        for o in objects(data) {
            guard o["type"] as? String == "user", (o["isMeta"] as? Bool) != true, (o["isSidechain"] as? Bool) != true,
                  let content = (o["message"] as? [String: Any])?["content"]
            else { continue }
            let text: String
            if let s = content as? String {
                text = s
            } else if let blocks = content as? [[String: Any]] {
                text = Self.text(of: blocks)
            } else { continue }
            if let p = cleanPrompt(text) { return p }
        }
        return nil
    }

    static func lastAITitle(in data: Data) -> String? {
        for o in objects(data, reversed: true) where o["type"] as? String == "ai-title" {
            if let t = o["aiTitle"] as? String, !t.isEmpty { return t }
        }
        return nil
    }

    static func gitBranch(in data: Data, last: Bool) -> String? {
        for o in objects(data, reversed: last) {
            if let b = o["gitBranch"] as? String, !b.isEmpty { return b }
        }
        return nil
    }

    static func lastReply(in data: Data) -> String? {
        for line in data.split(separator: 0x0A).reversed() {
            guard let o = try? JSONSerialization.jsonObject(with: Data(line)) as? [String: Any],
                  o["type"] as? String == "assistant",
                  (o["isSidechain"] as? Bool) != true,
                  let content = (o["message"] as? [String: Any])?["content"] as? [[String: Any]]
            else { continue }
            let collapsed = oneLine(text(of: content))
            if !collapsed.isEmpty { return collapsed }
        }
        return nil
    }

    /// The tool calls still waiting for a result. Exactly one approvable call is `.one`; anything else open is `.unclear`.
    static func pending(in data: Data) -> PendingState? {
        var open: [(id: String, name: String, input: [String: Any])] = []
        for line in data.split(separator: 0x0A) {
            guard let o = try? JSONSerialization.jsonObject(with: Data(line)) as? [String: Any],
                  let content = (o["message"] as? [String: Any])?["content"] as? [[String: Any]]
            else { continue }
            switch o["type"] as? String {
            case "assistant" where (o["isSidechain"] as? Bool) != true:
                for c in content where c["type"] as? String == "tool_use" {
                    guard let id = c["id"] as? String, let name = c["name"] as? String else { continue }
                    open.append((id, name, c["input"] as? [String: Any] ?? [:]))
                }
            case "user":
                for c in content where c["type"] as? String == "tool_result" {
                    if let id = c["tool_use_id"] as? String { open.removeAll { $0.id == id } }
                }
            default: break
            }
        }
        guard let first = open.first else { return nil }
        guard open.count == 1, approvable.contains(first.name) else { return .unclear(count: open.count) }
        guard let text = first.input[inputKey(for: first.name)] as? String, !text.isEmpty else { return .unclear(count: 1) }
        return .one(Pending(toolId: first.id, tool: first.name, text: text))
    }

    /// The input field that says what a tool call acts on.
    static func inputKey(for tool: String) -> String {
        switch tool {
        case "Bash": "command"
        case "WebFetch": "url"
        case "WebSearch": "query"
        case "Glob", "Grep": "pattern"
        default: "file_path"
        }
    }

    /// One line for a tool call, cut at 120 characters plus `…`.
    static func toolSummary(name: String, input: [String: Any]) -> String {
        func field(_ key: String) -> String? {
            (input[key] as? String).map(oneLine).flatMap { $0.isEmpty ? nil : $0 }
        }
        let s: String
        switch name {
        case "Bash", "Glob", "Grep", "WebFetch", "WebSearch":
            s = field(inputKey(for: name)).map { "\(name): \($0)" } ?? name
        case "Read", "Edit", "Write", "MultiEdit", "NotebookEdit":
            s = field("file_path").map { "\(name) \(($0 as NSString).lastPathComponent)" } ?? name
        case "Agent", "Task":
            s = field("description").map { "Agent: \($0)" } ?? name
        case "Skill":
            s = field("skill").map { "Skill: \($0)" } ?? name
        case _ where name.hasPrefix("mcp__"):
            let parts = name.components(separatedBy: "__")
            s = parts.count > 2 ? parts.dropFirst(2).joined(separator: "__") : name
        default:
            s = name
        }
        return cut(s, 120)
    }

    /// The last main-thread tool call in `data`; only lines holding `"tool_use"` are parsed.
    static func lastToolUse(in data: Data) -> String? {
        let marker = Data("\"tool_use\"".utf8)
        for line in data.split(separator: 0x0A).reversed() where line.range(of: marker) != nil {
            guard let o = try? JSONSerialization.jsonObject(with: Data(line)) as? [String: Any],
                  o["type"] as? String == "assistant",
                  (o["isSidechain"] as? Bool) != true,
                  let content = (o["message"] as? [String: Any])?["content"] as? [[String: Any]],
                  let use = content.last(where: { $0["type"] as? String == "tool_use" }),
                  let name = use["name"] as? String
            else { continue }
            return toolSummary(name: name, input: use["input"] as? [String: Any] ?? [:])
        }
        return nil
    }
}
