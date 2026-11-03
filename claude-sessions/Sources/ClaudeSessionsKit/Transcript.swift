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

enum Transcript {
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

    static func lastReply(in data: Data) -> String? {
        for line in data.split(separator: 0x0A).reversed() {
            guard let o = try? JSONSerialization.jsonObject(with: Data(line)) as? [String: Any],
                  o["type"] as? String == "assistant",
                  (o["isSidechain"] as? Bool) != true,
                  let content = (o["message"] as? [String: Any])?["content"] as? [[String: Any]]
            else { continue }
            let text = content.compactMap { $0["type"] as? String == "text" ? $0["text"] as? String : nil }
                .joined(separator: " ")
            let collapsed = text.split(whereSeparator: \.isWhitespace).joined(separator: " ")
            if !collapsed.isEmpty { return String(collapsed.prefix(300)) }
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
        let key: String
        switch first.name {
        case "Bash": key = "command"
        case "WebFetch": key = "url"
        case "WebSearch": key = "query"
        case "Glob", "Grep": key = "pattern"
        default: key = "file_path"
        }
        return .one(Pending(toolId: first.id, tool: first.name, text: first.input[key] as? String ?? ""))
    }
}
