import Foundation

public enum Transcript {
    /// Claude Code names a project directory after the cwd with every non-alphanumeric char replaced by "-".
    public static func projectDirName(_ cwd: String) -> String {
        String(cwd.unicodeScalars.map { s in
            (s.isASCII && (CharacterSet.alphanumerics.contains(s))) ? Character(s) : "-"
        })
    }

    public static func find(sessionId: String, cwd: String, projects: URL) -> URL? {
        let fm = FileManager.default
        let direct = projects.appendingPathComponent(projectDirName(cwd)).appendingPathComponent("\(sessionId).jsonl")
        if fm.fileExists(atPath: direct.path) { return direct }
        let dirs = (try? fm.contentsOfDirectory(at: projects, includingPropertiesForKeys: nil)) ?? []
        return dirs.map { $0.appendingPathComponent("\(sessionId).jsonl") }.first { fm.fileExists(atPath: $0.path) }
    }

    public static func tail(_ url: URL, bytes: Int = 262_144) -> Data {
        guard let h = try? FileHandle(forReadingFrom: url) else { return Data() }
        defer { try? h.close() }
        guard let end = try? h.seekToEnd() else { return Data() }
        try? h.seek(toOffset: end > UInt64(bytes) ? end - UInt64(bytes) : 0)
        return (try? h.readToEnd()) ?? Data()
    }

    public static func lastReply(in data: Data) -> String? {
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
}
